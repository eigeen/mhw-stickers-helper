use std::io::{Read, Seek, SeekFrom, Write};

use byteorder::{WriteBytesExt, LE};

use crate::{
    error::Result,
    spec::{self, TexFormat, TexInfo},
};

const W_MAGIC_NUMBER_DDS: &[u8] = &[
    0x44, 0x44, 0x53, 0x20, 0x7C, 0x00, 0x00, 0x00, 0x07, 0x10, 0x0A, 0x00,
];
const COMPRESS_OPTION: &[u8] = &[0x08, 0x10, 0x40, 0x00];
const DX10_FIXED_FLAGS: &[u8] = &[
    0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
const TEX_WITH_4BPP: &[TexFormat] = &[
    TexFormat::DxgiFormatBc1Unorm,
    TexFormat::DxgiFormatBc1UnormSRGB,
    TexFormat::DxgiFormatBc4Unorm,
];
const TEX_WITH_16BPP: &[TexFormat] = &[TexFormat::DxgiFormatR8G8Unorm];

pub fn convert_to_dds<R>(reader: &mut R) -> Result<Vec<u8>>
where
    R: Read + Seek,
{
    let info = TexInfo::from_reader(reader)?;

    // read data
    reader.seek(SeekFrom::Start(info.offset as u64))?;
    let mut data = vec![];
    reader.read_to_end(&mut data)?;

    let mut out_data = Vec::new();

    // dds header
    out_data.write_all(W_MAGIC_NUMBER_DDS)?;
    out_data.write_i32::<LE>(info.height)?;
    out_data.write_i32::<LE>(info.width)?;

    if TEX_WITH_4BPP.contains(&info.format) {
        out_data.write_i32::<LE>(info.width * info.height / 2)?;
    } else if TEX_WITH_16BPP.contains(&info.format) {
        out_data.write_i32::<LE>(info.width * info.height * 2)?;
    } else {
        // 8bpp
        out_data.write_i32::<LE>(info.width * info.height)?;
    }

    out_data.write_i32::<LE>(1)?; // depth
    out_data.write_i32::<LE>(info.mip_map_count)?;
    out_data.write_all(&[0u8; 11 * 4])?; // reserved 11*4

    // ddspf
    out_data.write_i32::<LE>(32)?;
    out_data.write_i32::<LE>(4)?;
    out_data.write_all(info.format.magic())?;
    out_data.write_all(&[0u8; 5 * 4])?;

    out_data.write_all(COMPRESS_OPTION)?;
    out_data.write_all(&[0u8; 4 * 4])?;

    // ds header dxt10
    if info.format.magic() == b"DX10" {
        let dds_format: spec::DxgiFormat = info.format.try_into().unwrap();
        out_data.write_i32::<LE>(dds_format as i32)?;
        out_data.write_all(DX10_FIXED_FLAGS)?;
    }

    // write data
    out_data.write_all(&data)?;

    Ok(out_data)
}

#[cfg(test)]
mod tests {
    use std::{fs::OpenOptions, io::Cursor};

    use image_dds::ddsfile::Dds;

    use super::*;

    const DATA: &[u8] = include_bytes!("../../../test_data/chat_stamp00_ID.tex");

    #[test]
    fn test_convert_to_dds() {
        let mut reader = Cursor::new(DATA);
        let dds = convert_to_dds(&mut reader).unwrap();

        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open("../../test_data/chat_stamp00_ID.dds")
            .unwrap();
        std::io::copy(&mut Cursor::new(&dds), &mut file).unwrap();

        let dds = Dds::read(&mut Cursor::new(&dds)).unwrap();

        let img = image_dds::image_from_dds(&dds, 0).unwrap();
        img.save("../../test_data/chat_stamp00_ID.png").unwrap();
    }
}
