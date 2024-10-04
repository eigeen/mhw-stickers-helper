use crate::error::{Error, Result};

use byteorder::{ReadBytesExt, LE};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use std::io::{Read, Seek, SeekFrom};

use super::DxgiFormat;

pub struct TexInfo {
    pub magic: i32,

    pub mip_map_count: i32,
    pub width: i32,
    pub height: i32,

    pub format: TexFormat,

    pub offset: i64,
}

impl TexInfo {
    const MAGIC: i32 = 0x00584554;

    pub fn from_reader<R>(reader: &mut R) -> Result<Self>
    where
        R: Read + Seek,
    {
        let magic = reader.read_i32::<LE>()?;
        if magic != Self::MAGIC {
            return Err(Error::BadMagic(Self::MAGIC, magic));
        }

        reader.seek(SeekFrom::Start(0x14))?;
        let mip_map_count = reader.read_i32::<LE>()?;
        let width = reader.read_i32::<LE>()?;
        let height = reader.read_i32::<LE>()?;

        reader.seek(SeekFrom::Start(0x24))?;
        let r#type = reader.read_i32::<LE>()?;
        let format = TexFormat::from_i32(r#type).unwrap_or(TexFormat::DxgiFormatUnknown);
        if format == TexFormat::DxgiFormatUnknown {
            return Err(Error::UnknownTexFormat);
        }

        reader.seek(SeekFrom::Start(0xB8))?;
        let offset = reader.read_i64::<LE>()?;

        // read size unused
        // skip

        Ok(TexInfo {
            magic,
            mip_map_count,
            width,
            height,
            format,
            offset,
        })
    }
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromPrimitive)]
pub enum TexFormat {
    DxgiFormatUnknown = 0,
    DxgiFormatR8G8B8A8Unorm = 7,
    DxgiFormatR8G8B8A8UnormSRGB = 9, //LUTs
    DxgiFormatR8G8Unorm = 19,
    DxgiFormatBc1Unorm = 22,
    DxgiFormatBc1UnormSRGB = 23,
    DxgiFormatBc4Unorm = 24,
    DxgiFormatBc5Unorm = 26,
    DxgiFormatBc6hUf16 = 28,
    DxgiFormatBc7Unorm = 30,
    DxgiFormatBc7UnormSRGB = 31,
}

impl TexFormat {
    pub fn tag(&self) -> &'static str {
        match self {
            TexFormat::DxgiFormatUnknown => "UNKN_",
            TexFormat::DxgiFormatR8G8B8A8Unorm => "R8G8B8A8_",
            TexFormat::DxgiFormatR8G8B8A8UnormSRGB => "SR8G8B8A8_",
            TexFormat::DxgiFormatR8G8Unorm => "R8G8_",
            TexFormat::DxgiFormatBc1Unorm => "DXT1L_",
            TexFormat::DxgiFormatBc1UnormSRGB => "BC1S_",
            TexFormat::DxgiFormatBc4Unorm => "BC4_",
            TexFormat::DxgiFormatBc5Unorm => "BC5_",
            TexFormat::DxgiFormatBc6hUf16 => "BC6_",
            TexFormat::DxgiFormatBc7Unorm => "BC7L_",
            TexFormat::DxgiFormatBc7UnormSRGB => "BC7S_",
        }
    }

    pub fn magic(&self) -> &'static [u8; 4] {
        match self {
            TexFormat::DxgiFormatUnknown => b"UNKN",
            TexFormat::DxgiFormatR8G8B8A8Unorm => b"DX10",
            TexFormat::DxgiFormatR8G8B8A8UnormSRGB => b"DX10",
            TexFormat::DxgiFormatR8G8Unorm => b"DX10",
            TexFormat::DxgiFormatBc1Unorm => b"DXT1",
            TexFormat::DxgiFormatBc1UnormSRGB => b"DX10",
            TexFormat::DxgiFormatBc4Unorm => b"BC4U",
            TexFormat::DxgiFormatBc5Unorm => b"BC5U",
            TexFormat::DxgiFormatBc6hUf16 => b"DX10",
            TexFormat::DxgiFormatBc7Unorm => b"DX10",
            TexFormat::DxgiFormatBc7UnormSRGB => b"DX10",
        }
    }

    pub fn from_magic(magic: &[u8; 4]) -> Self {
        match magic {
            b"UNKN" => TexFormat::DxgiFormatUnknown,
            b"SRGB" => TexFormat::DxgiFormatR8G8B8A8UnormSRGB,
            b"DXT1" => TexFormat::DxgiFormatBc1Unorm,
            b"BC1S" => TexFormat::DxgiFormatBc1UnormSRGB,
            b"BC4U" => TexFormat::DxgiFormatBc4Unorm,
            b"BC5U" => TexFormat::DxgiFormatBc5Unorm,
            b"DX10" => {
                // 返回默认值，需要根据文件内容继续判断
                TexFormat::DxgiFormatBc7Unorm
            }
            _ => TexFormat::DxgiFormatUnknown,
        }
    }
}

impl TryFrom<DxgiFormat> for TexFormat {
    type Error = crate::error::Error;

    fn try_from(value: DxgiFormat) -> std::result::Result<Self, Self::Error> {
        match value {
            DxgiFormat::R8G8B8A8Unorm => Ok(TexFormat::DxgiFormatR8G8B8A8Unorm),
            DxgiFormat::R8G8B8A8UnormSrgb => Ok(TexFormat::DxgiFormatR8G8B8A8UnormSRGB),
            DxgiFormat::R8G8Unorm => Ok(TexFormat::DxgiFormatR8G8Unorm),
            DxgiFormat::Bc1Unorm => Ok(TexFormat::DxgiFormatBc1Unorm),
            DxgiFormat::Bc1UnormSrgb => Ok(TexFormat::DxgiFormatBc1UnormSRGB),
            DxgiFormat::Bc4Unorm => Ok(TexFormat::DxgiFormatBc4Unorm),
            DxgiFormat::Bc5Unorm => Ok(TexFormat::DxgiFormatBc5Unorm),
            DxgiFormat::Bc6hUf16 => Ok(TexFormat::DxgiFormatBc6hUf16),
            DxgiFormat::Bc7Unorm => Ok(TexFormat::DxgiFormatBc7Unorm),
            DxgiFormat::Bc7UnormSrgb => Ok(TexFormat::DxgiFormatBc7UnormSRGB),
            _ => Err(crate::error::Error::UnknownTexFormat),
        }
    }
}
