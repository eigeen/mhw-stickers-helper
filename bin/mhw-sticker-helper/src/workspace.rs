use image::ImageFormat;
use ring::digest::Digest;
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{BufReader, Cursor, Read, Write},
    path::Path,
};

use crate::{asset, util};

/// 工作区信息
///
/// 统计工作区包含的 Stickers 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    version: i32,
    sticker_packs: Vec<StickerPack>,
}

impl Default for WorkspaceInfo {
    fn default() -> Self {
        Self {
            version: 1,
            sticker_packs: Default::default(),
        }
    }
}

impl WorkspaceInfo {
    pub fn version(&self) -> i32 {
        self.version
    }

    pub fn sticker_packs(&self) -> &[StickerPack] {
        &self.sticker_packs
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickerPack {
    pub name: String,
    pub filename: String,
    pub checksum_sha256: HashString,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HashString(Vec<u8>);

impl HashString {
    pub fn to_hex(&self) -> String {
        use std::fmt::Write;

        let hex_string = String::with_capacity(self.0.len() * 2);
        self.0.iter().fold(hex_string, |mut hex_string, byte| {
            write!(hex_string, "{:02x}", byte).unwrap();
            hex_string
        })
    }

    pub fn from_hex(hex_str: &str) -> Result<HashString, hex::FromHexError> {
        let bytes = hex::decode(hex_str)?;
        Ok(HashString(bytes))
    }
}

impl Serialize for HashString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for HashString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        HashString::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

impl PartialEq<Digest> for HashString {
    fn eq(&self, other: &Digest) -> bool {
        self.0 == other.as_ref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StickerPackType {
    Dds,
    Png,
}

/// 工作区
#[derive(Debug, Clone)]
pub struct Workspace {
    info: WorkspaceInfo,
    root_path: String,
}

impl std::fmt::Display for Workspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.root_path)
    }
}

impl Workspace {
    pub fn create_new<P: AsRef<Path>>(
        path: P,
        sticker_type: StickerPackType,
    ) -> anyhow::Result<Self> {
        if path.as_ref().exists() {
            return Err(anyhow::anyhow!(
                "目录已存在: {}\n请删除该目录或指定其他目录作为工作区目录",
                path.as_ref().display()
            ));
        }

        let info = WorkspaceInfo::default();
        let mut this = Workspace {
            info,
            root_path: path.as_ref().to_string_lossy().to_string(),
        };

        // 创建文件
        std::fs::create_dir_all(&path)?;
        this.extract_stickers(sticker_type)?;

        // 写入工作区信息
        this.write_info()?;

        Ok(this)
    }

    pub fn info(&self) -> &WorkspaceInfo {
        &self.info
    }

    pub fn root_path(&self) -> &str {
        &self.root_path
    }

    /// 同步工作区信息到工作区文件
    pub fn write_info(&self) -> anyhow::Result<()> {
        let info_path = Path::new(&self.root_path).join("workspace.json");
        let info_str = serde_json::to_string_pretty(&self.info)?;
        std::fs::write(info_path, info_str)?;

        Ok(())
    }

    /// 解压贴纸到工作区目录，转换为png格式，并更新工作区信息
    pub fn extract_stickers(&mut self, sticker_type: StickerPackType) -> anyhow::Result<()> {
        let output_dir = Path::new(&self.root_path);

        for input_name in asset::Asset::iter() {
            if !input_name.ends_with(".tex") {
                continue;
            }
            let file = asset::Asset::get(&input_name).unwrap();
            let mut reader = Cursor::new(file.data);

            let input_name_owned = input_name.to_string();
            let input_path = Path::new(&input_name_owned);
            let filestem = input_path
                .file_stem()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default();
            let file_output_path = match sticker_type {
                StickerPackType::Dds => output_dir.join(format!("{}.dds", filestem)),
                StickerPackType::Png => output_dir.join(format!("{}.png", filestem)),
            };

            let mut data = vec![];
            let mut writer = Cursor::new(&mut data);
            match sticker_type {
                StickerPackType::Dds => {
                    let dds_data = tex_convert::tex2dds::convert_to_dds(&mut reader)?;
                    data = dds_data;
                }
                StickerPackType::Png => {
                    let img = tex_convert::load_tex_image(&mut reader)?;
                    img.write_to(&mut writer, ImageFormat::Png)?;
                }
            }

            // 解析信息
            let info = Self::parse_sticker_info(&mut Cursor::new(&data), &file_output_path)?;
            self.info.sticker_packs.push(info);
            // 写入文件
            let mut file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&file_output_path)?;
            file.write_all(&data)?;
        }

        Ok(())
    }

    /// 获取工作区中内容变更的贴纸
    pub fn get_modified_stickers(&self) -> anyhow::Result<Vec<StickerPack>> {
        let mut modified_stickers = vec![];
        for sticker in &self.info.sticker_packs {
            let input_path = Path::new(&self.root_path).join(&sticker.filename);

            if !input_path.exists() {
                continue;
            }
            let Ok(file) = File::open(&input_path) else {
                eprintln!("无法打开文件: {}, 跳过", input_path.display());
                continue;
            };
            let mut reader = BufReader::new(file);
            let digest = util::sha256_digest(&mut reader)?;
            if sticker.checksum_sha256 != digest {
                modified_stickers.push(sticker.clone());
            }
        }

        Ok(modified_stickers)
    }

    /// 列出当前目录下所有的工作区
    pub fn list_all_workspaces() -> anyhow::Result<Vec<Workspace>> {
        // 遍历当前目录
        let current_dir = std::env::current_dir()?;
        let mut workspaces = vec![];
        for entry in current_dir.read_dir()? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let info_path = path.join("workspace.json");
                if info_path.exists() {
                    let info_str = std::fs::read_to_string(&info_path)?;
                    let info: WorkspaceInfo = serde_json::from_str(&info_str)?;
                    workspaces.push(Workspace {
                        info,
                        root_path: path.to_string_lossy().to_string(),
                    });
                }
            }
        }

        Ok(workspaces)
    }

    fn parse_sticker_info<R, P>(reader: &mut R, path: P) -> anyhow::Result<StickerPack>
    where
        R: Read,
        P: AsRef<Path>,
    {
        let digest = util::sha256_digest(reader)?;
        let hash_string = HashString(digest.as_ref().to_vec());

        Ok(StickerPack {
            name: path
                .as_ref()
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            filename: path
                .as_ref()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            checksum_sha256: hash_string,
        })
    }
}
