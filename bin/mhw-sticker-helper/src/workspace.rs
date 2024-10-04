use anyhow::Context;
use image::{DynamicImage, ImageFormat};
use ring::digest::Digest;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
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
    stickers: Vec<StickerInfo>,
}

impl Default for WorkspaceInfo {
    fn default() -> Self {
        Self {
            version: 1,
            stickers: Default::default(),
        }
    }
}

impl WorkspaceInfo {
    pub fn version(&self) -> i32 {
        self.version
    }

    pub fn stickers(&self) -> &[StickerInfo] {
        &self.stickers
    }

    /// 贴纸包数量
    pub fn collection_count(&self) -> usize {
        let stat = self
            .stickers
            .iter()
            .fold(HashSet::new(), |mut stat, sticker| {
                stat.insert(sticker.collection.clone());
                stat
            });

        stat.len()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickerInfo {
    pub collection: String,
    pub id: i32,
    pub name: String,
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
    pub fn create_new<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
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
        this.extract_stickers()?;

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
    pub fn extract_stickers(&mut self) -> anyhow::Result<()> {
        let output_dir = Path::new(&self.root_path);

        for input_name in asset::Asset::iter() {
            if !input_name.ends_with(".tex") {
                continue;
            }
            let file = asset::Asset::get(&input_name).unwrap();
            let mut reader = Cursor::new(file.data);

            let img = tex_convert::load_tex_image(&mut reader)?;
            let mut img = DynamicImage::ImageRgba8(img);

            let width = 120;
            let height = 86;
            let n_tile = 5;

            let input_name_owned = input_name.to_string();
            let input_path = Path::new(&input_name_owned);
            let filestem = input_path
                .file_stem()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default();
            // crop and output
            for row_index in 0..n_tile {
                let tile = img.crop(0, row_index * height, width, height);
                let file_output = output_dir.join(format!("{}_{}.png", filestem, row_index));

                let mut data = vec![];
                let mut writer = Cursor::new(&mut data);
                tile.write_to(&mut writer, ImageFormat::Png)?;

                let info = Self::parse_sticker_info(&mut Cursor::new(&data), &file_output)?;
                self.info.stickers.push(info);

                let mut file = OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .write(true)
                    .open(&file_output)?;
                file.write_all(&data)?;
            }
        }

        Ok(())
    }

    /// 获取工作区中内容变更的贴纸
    pub fn get_modified_stickers(&self) -> anyhow::Result<Vec<StickerInfo>> {
        let mut modified_stickers = vec![];
        for sticker in &self.info.stickers {
            let input_path = Path::new(&self.root_path).join(&sticker.name);

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

    /// 获取工作区中内容变更的贴纸包
    ///
    /// 贴纸包中至少有一个贴纸发生改变时，导出整个贴纸包
    pub fn get_modified_collections(&self) -> anyhow::Result<HashMap<String, Vec<StickerInfo>>> {
        let mut modified_collections: HashMap<String, Vec<StickerInfo>> = HashMap::new();
        for sticker in &self.info.stickers {
            if modified_collections.contains_key(&sticker.collection) {
                continue;
            }
            let input_path = Path::new(&self.root_path).join(&sticker.name);

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
                modified_collections.insert(
                    sticker.collection.clone(),
                    self.get_collection(&sticker.collection),
                );
            }
        }

        Ok(modified_collections)
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

    fn get_collection(&self, collection_name: &str) -> Vec<StickerInfo> {
        self.info
            .stickers
            .iter()
            .filter(|s| s.collection == collection_name)
            .cloned()
            .collect()
    }

    fn parse_sticker_info<R, P>(reader: &mut R, path: P) -> anyhow::Result<StickerInfo>
    where
        R: Read,
        P: AsRef<Path>,
    {
        let digest = util::sha256_digest(reader)?;
        let hash_string = HashString(digest.as_ref().to_vec());

        let name = path.as_ref().file_stem().unwrap().to_str().unwrap();
        let Some((collection, id)) = name.rsplit_once('_') else {
            return Err(anyhow::anyhow!("无法解析贴纸文件名: {}", name));
        };

        Ok(StickerInfo {
            collection: collection.to_string(),
            id: id.parse().context("无法解析贴纸 ID")?,
            name: path
                .as_ref()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            checksum_sha256: hash_string,
        })
    }
}
