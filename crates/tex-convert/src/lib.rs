use std::io::{Read, Seek};

use image::RgbaImage;
use image_dds::ddsfile::AlphaMode;

pub mod error;
mod intel;
pub mod spec;

#[cfg(feature = "dds2tex")]
pub mod dds2tex;
#[cfg(feature = "tex2dds")]
pub mod tex2dds;

#[cfg(feature = "tex2dds")]
pub fn load_tex_image<R: Read + Seek>(reader: &mut R) -> Result<RgbaImage, error::Error> {
    let dds_data = tex2dds::convert_to_dds(reader)?;

    load_dds_image(&mut &dds_data[..])
}

#[cfg(feature = "dds2tex")]
/// Convert [image::RgbaImage] to tex image
///
/// [image::RgbaImage] -> dds -> tex
pub fn convert_image_to_tex(image: &RgbaImage) -> Result<Vec<u8>, error::Error> {
    use std::io::{Cursor, Write};

    let dds_data = convert_image_to_dds(image)?;

    // debug
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open("debug.dds")
        .unwrap();
    file.write_all(&dds_data).unwrap();

    dds2tex::convert_to_tex(&mut Cursor::new(&dds_data))
}

pub fn convert_image_to_dds(image: &RgbaImage) -> Result<Vec<u8>, error::Error> {
    let mut dds = image_dds::dds_from_image(
        image,
        image_dds::ImageFormat::BC7RgbaUnormSrgb,
        image_dds::Quality::Slow,
        image_dds::Mipmaps::Disabled,
    )?;
    dds.header.depth = Some(1);
    dds.header.mip_map_count = Some(1);
    if let Some(header10) = &mut dds.header10 {
        header10.alpha_mode = AlphaMode::Unknown;
    }

    let mut dds_data = vec![];
    dds.write(&mut dds_data)?;

    Ok(dds_data)
}

/// Read dds image as [image::RgbaImage]
pub fn load_dds_image<R: Read>(reader: &mut R) -> Result<RgbaImage, error::Error> {
    let dds = image_dds::ddsfile::Dds::read(reader)?;
    let image = image_dds::image_from_dds(&dds, 0)?;

    Ok(image)
}

#[cfg(test)]
mod tests {
    use std::{fs::OpenOptions, io::Write};

    use image::DynamicImage;

    use super::*;

    /// png -> <image> -> dds -> tex
    #[test]
    fn test_convert_image_to_tex() {
        let img = image::open("../../test_data/chat_stamp00_ID.png").unwrap();
        if let DynamicImage::ImageRgba8(img) = img {
            let tex_data = convert_image_to_tex(&img).unwrap();
            let mut file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open("../../test_data/chat_stamp00_ID_from_png.tex")
                .unwrap();
            file.write_all(&tex_data).unwrap();
        }
    }

    #[test]
    fn test_convert_image_to_dds() {
        let img = image::open("../../test_data/chat_stamp00_ID.png").unwrap();
        if let DynamicImage::ImageRgba8(img) = img {
            let dds_data = convert_image_to_dds(&img).unwrap();
            let mut file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open("../../test_data/chat_stamp00_ID_from_png.dds")
                .unwrap();
            file.write_all(&dds_data).unwrap();
        }
    }
}
