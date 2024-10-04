use std::io::{Read, Seek, SeekFrom, Write};

use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use num_traits::FromPrimitive;

use crate::{
    error::{Error, Result},
    spec::{DxgiFormat, TexFormat},
};

const DDS_MAGIC: i32 = 0x20534444;

const W_MAGIC_NUMBER_TEX: &[u8] = &[
    0x54, 0x45, 0x58, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x02, 0x00, 0x00, 0x00,
];
const TEX_FIXED_UNKN: &[u8] = &[
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
const TEX_OF_NEW_DDS: &[TexFormat] = &[
    TexFormat::DxgiFormatBc7Unorm,
    TexFormat::DxgiFormatBc7UnormSRGB,
    TexFormat::DxgiFormatBc6hUf16,
];
const TEX_WITH_4BPP: &[TexFormat] = &[
    TexFormat::DxgiFormatBc1Unorm,
    TexFormat::DxgiFormatBc1UnormSRGB,
    TexFormat::DxgiFormatBc4Unorm,
];
const TEX_WITH_16BPP: &[TexFormat] = &[TexFormat::DxgiFormatR8G8Unorm];

pub fn convert_to_tex<R>(reader: &mut R) -> Result<Vec<u8>>
where
    R: Read + Seek,
{
    // dds直接转换tex
    let magic = reader.read_i32::<LE>()?;
    if magic != DDS_MAGIC {
        return Err(Error::BadMagic(DDS_MAGIC, magic));
    }

    reader.seek(SeekFrom::Start(0x8))?;
    let dds_flag = reader.read_i32::<LE>()?;
    let is_raw = dds_flag & 0x8 == 0x8;
    let height = reader.read_i32::<LE>()?;
    let width = reader.read_i32::<LE>()?;

    reader.seek(SeekFrom::Start(0x1C))?;
    let mipmap_count = reader.read_i32::<LE>()?;

    reader.seek(SeekFrom::Start(0x54))?;
    let mut filetype_magic = [0u8; 4];
    reader.read_exact(&mut filetype_magic)?;
    let mut format = TexFormat::from_magic(&filetype_magic);
    if format == TexFormat::DxgiFormatUnknown {
        return Err(Error::UnknownTexFormat);
    }
    if format == TexFormat::DxgiFormatBc7Unorm {
        // 进一步读取DX10专有字段确定具体类型
        reader.seek(SeekFrom::Start(0x80))?;
        let dxgi_format_code = reader.read_i32::<LE>()?;
        let dxgi_format = DxgiFormat::from_i32(dxgi_format_code).ok_or(Error::UnknownTexFormat)?;
        format = dxgi_format.try_into()?;
    }

    if format.magic() == b"DX10" && !is_raw {
        reader.seek(SeekFrom::Start(0x94))?;
    } else {
        reader.seek(SeekFrom::Start(0x80))?;
    }

    let mut data = vec![];
    reader.read_to_end(&mut data)?;

    let mut out_tex = vec![];
    out_tex.write_all(W_MAGIC_NUMBER_TEX)?;
    out_tex.write_i32::<LE>(mipmap_count)?;
    out_tex.write_i32::<LE>(width)?;
    out_tex.write_i32::<LE>(height)?;
    out_tex.write_i32::<LE>(1)?;
    out_tex.write_i32::<LE>(format as i32)?;
    out_tex.write_all(TEX_FIXED_UNKN)?;

    if TEX_OF_NEW_DDS.contains(&format) {
        out_tex.write_i32::<LE>(1)?;
    } else {
        out_tex.write_i32::<LE>(0)?;
    }

    out_tex.write_all(&[0u8; 4 * 4])?;
    out_tex.write_all(
        &[-1_i32; 8]
            .into_iter()
            .flat_map(|x| x.to_le_bytes())
            .collect::<Vec<u8>>(),
    )?;
    out_tex.write_i32::<LE>(width)?;

    let is_full_width = is_raw || format == TexFormat::DxgiFormatR8G8Unorm;

    // repeat 3 times
    for _ in 0..3 {
        if is_full_width {
            out_tex.write_i16::<LE>(width as i16)?;
        } else {
            out_tex.write_i16::<LE>((width / 2) as i16)?;
        }
        out_tex.write_i16::<LE>(width as i16)?;
        out_tex.write_all(&[0u8; 4 * 2])?;
    }
    out_tex.write_all(&[0u8; 4 * 6])?;

    let mut cur_width: i32 = width;
    let mut cur_height: i32 = height;
    let mut base_loc: i32 = 0xb8 + mipmap_count * 8;
    for _ in 0..mipmap_count {
        out_tex.write_i32::<LE>(base_loc)?;
        out_tex.write_i32::<LE>(0)?;

        let max_width = if is_raw { 2 } else { 4 };
        if TEX_WITH_4BPP.contains(&format) {
            base_loc += cur_width * cur_height / 2;
        } else if TEX_WITH_16BPP.contains(&format) {
            base_loc += cur_width * cur_height * 2;
        } else if is_raw {
            base_loc += cur_width * cur_height * 4;
        } else {
            base_loc += cur_width * cur_height;
        }

        cur_width /= 2;
        cur_height /= 2;

        cur_width = i32::max(cur_width, max_width);
        cur_height = i32::max(cur_height, max_width);
    }

    out_tex.write_all(&data)?;

    Ok(out_tex)
}

#[cfg(test)]
mod tests {
    use std::{fs::OpenOptions, io::Cursor};

    use super::*;

    const DATA: &[u8] = include_bytes!("../../../test_data/chat_stamp00_ID.dds");

    #[test]
    fn test_convert_to_tex() {
        let mut reader = Cursor::new(DATA);
        let tex_data = convert_to_tex(&mut reader).unwrap();

        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open("../../test_data/chat_stamp00_ID_from_dds.tex")
            .unwrap();
        std::io::copy(&mut Cursor::new(&tex_data), &mut file).unwrap();
    }
}
