pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
    #[error("dds error: {0}")]
    Dds(#[from] image_dds::ddsfile::Error),
    #[error("Create image from dds error: {0}")]
    CreateImageFromDds(#[from] image_dds::error::CreateImageError),
    #[error("Create dds from image error: {0}")]
    CreateDdsFromImage(#[from] image_dds::CreateDdsError),

    #[error("Invalid magic number: expected {0:#x}, got {1:#x}")]
    BadMagic(i32, i32),
    #[error("Unknown tex format")]
    UnknownTexFormat,
}
