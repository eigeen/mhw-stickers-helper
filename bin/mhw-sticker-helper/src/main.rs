use std::{
    fmt::Display,
    fs::OpenOptions,
    io::{Cursor, Write},
    path::Path,
};

use dialoguer::{theme::ColorfulTheme, Input, Select};
use image::DynamicImage;
use workspace::{StickerPackType, Workspace};
use zip::{write::SimpleFileOptions, ZipWriter};

mod asset;
mod util;
mod workspace;

fn main() -> anyhow::Result<()> {
    let mut app = App::new();
    if let Err(e) = app.run() {
        eprintln!("{:#}", e);
    };

    Ok(())
}

enum AppState {
    /// 程序入口
    Enter,
    /// 退出程序
    Exit,
}

struct App {
    state: AppState,
}

impl App {
    pub fn new() -> Self {
        Self {
            state: AppState::Enter,
        }
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        println!("MHW 贴纸助手 v{}", env!("CARGO_PKG_VERSION"));
        loop {
            match self.state {
                AppState::Enter => self.show_main_menu()?,
                AppState::Exit => return Ok(()),
            }
        }
    }

    fn show_main_menu(&mut self) -> anyhow::Result<()> {
        let selection = MainSelection::show_interact()?;
        match selection {
            MainSelection::NewWorkspace => self.show_new_workspace()?,
            MainSelection::OpenWorkspace => self.show_open_workspace()?,
            MainSelection::Exit => self.state = AppState::Exit,
        };

        Ok(())
    }

    fn show_new_workspace(&mut self) -> anyhow::Result<()> {
        let workspace_name: String = Input::with_theme(&ColorfulTheme::default())
            .with_initial_text("example")
            .allow_empty(false)
            .with_prompt("请输入工作区名称： (将会在当前目录下建立工作区目录)")
            .interact_text()?;

        let workspace_mode = WorkspaceModeSelection::show_interact()?;

        let path = Path::new(&workspace_name);
        if let Err(e) = Workspace::create_new(path, workspace_mode.into()) {
            eprintln!("创建工作区失败：{}", e);
            return Ok(());
        };

        println!("工作区创建成功！");
        println!("目录：{}", std::env::current_dir()?.join(path).display());

        Ok(())
    }

    fn show_open_workspace(&mut self) -> anyhow::Result<()> {
        // 读取所有工作区
        let workspaces = Workspace::list_all_workspaces()?;
        if workspaces.is_empty() {
            println!("没有可用的工作区！");
            return Ok(());
        };

        // 选择工作区
        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("请选择工作区： (按↑↓选择，Enter确认)")
            .items(&workspaces)
            .default(0)
            .interact()?;
        let mut workspace = workspaces[selection].clone();

        // 进入工作区操作
        self.show_workspace_menu(&mut workspace)?;

        Ok(())
    }

    fn show_workspace_menu(&mut self, workspace: &mut Workspace) -> anyhow::Result<()> {
        let mut rerun: bool = true;

        while rerun {
            let selection = WorkspaceSelection::show_interact()?;
            match selection {
                WorkspaceSelection::Info => {
                    let modified_stickers = workspace.get_modified_stickers()?;

                    println!("工作区信息：");
                    println!("版本：{}", workspace.info().version());
                    println!("路径：{}", workspace.root_path());
                    println!("贴纸包数量：{}", workspace.info().sticker_packs().len());
                    println!("已更改贴纸包数量：{}", modified_stickers.len());

                    if !modified_stickers.is_empty() {
                        println!("已更改贴纸包：");
                        for sticker in modified_stickers {
                            println!("  - {}/{}", sticker.name, sticker.filename);
                        }
                    }
                }
                WorkspaceSelection::Package => {
                    Self::package_modified_stickers(workspace)?;
                    println!("打包完成！");
                }
                WorkspaceSelection::Back => {
                    rerun = false;
                }
            };
        }

        Ok(())
    }

    fn package_modified_stickers(workspace: &mut Workspace) -> anyhow::Result<()> {
        let modified_stickers = workspace.get_modified_stickers()?;
        if modified_stickers.is_empty() {
            eprintln!("没有发现需要打包的贴纸");
            return Ok(());
        }

        let root_path = Path::new(workspace.root_path());
        let dist_dir = root_path.parent().unwrap().join("dist");
        let workspace_name = Path::new(workspace.root_path())
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let output_dir = dist_dir.join(workspace_name);
        println!("输出目录：{}", output_dir.display());
        if !output_dir.exists() {
            std::fs::create_dir_all(&output_dir)?;
        }

        // 创建zip文件
        let zip_path = dist_dir.join(format!("{}.zip", workspace_name));
        let zip_file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&zip_path)?;
        let mut zip_writer = ZipWriter::new(zip_file);
        println!("导出 MOD 包：{}", zip_path.display());

        for sticker in modified_stickers {
            let input_path = root_path.join(&sticker.filename);
            let tex_data = match Path::new(&sticker.filename)
                .extension()
                .unwrap()
                .to_str()
                .unwrap()
            {
                "dds" => Self::convert_dds_sticker_to_tex(&input_path)?,
                "png" => Self::convert_png_sticker_to_tex(&input_path)?,
                _ => anyhow::bail!("不支持的文件后缀：{}", sticker.filename),
            };

            let file_name = format!("{}.tex", sticker.name);
            // 导出独立文件
            let output_path = output_dir.join(&file_name);
            let mut file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&output_path)?;
            file.write_all(&tex_data)?;
            println!("导出文件：{}", output_path.display());
            // 写入zip文件
            zip_writer.start_file(
                format!("nativePC/ui/chat/tex/stamp/{}", file_name),
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
            )?;
            zip_writer.write_all(&tex_data)?;
        }

        Ok(())
    }

    fn convert_png_sticker_to_tex<P: AsRef<Path>>(path: P) -> anyhow::Result<Vec<u8>> {
        let img = image::open(&path)?;
        if img.width() != 128 && img.height() != 512 {
            anyhow::bail!(
                "贴纸尺寸错误：应为 128x512，实际为 {}x{}",
                img.width(),
                img.height()
            );
        }

        let DynamicImage::ImageRgba8(img) = img else {
            anyhow::bail!("贴纸格式错误：应为 RGBA8 (png)，实际为 {:?}", img.color());
        };
        // Tex文件数据
        let tex_data = tex_convert::convert_image_to_tex(&img)?;

        Ok(tex_data)
    }

    fn convert_dds_sticker_to_tex<P: AsRef<Path>>(path: P) -> anyhow::Result<Vec<u8>> {
        let dds_data = std::fs::read(&path)?;
        let tex_data = tex_convert::dds2tex::convert_to_tex(&mut Cursor::new(dds_data))?;

        Ok(tex_data)
    }
}

#[derive(Debug)]
enum MainSelection {
    NewWorkspace,
    OpenWorkspace,
    Exit,
}

impl Display for MainSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MainSelection::NewWorkspace => write!(f, "新建工作区"),
            MainSelection::OpenWorkspace => write!(f, "打开工作区"),
            MainSelection::Exit => write!(f, "退出"),
        }
    }
}

impl From<usize> for MainSelection {
    fn from(index: usize) -> Self {
        match index {
            0 => MainSelection::NewWorkspace,
            1 => MainSelection::OpenWorkspace,
            2 => MainSelection::Exit,
            _ => unreachable!(),
        }
    }
}

impl MainSelection {
    pub fn show_interact() -> anyhow::Result<Self> {
        let selections = &[
            MainSelection::NewWorkspace,
            MainSelection::OpenWorkspace,
            MainSelection::Exit,
        ];
        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("请选择操作： (按↑↓选择，Enter确认)")
            .items(selections)
            .default(0)
            .interact()?;

        Ok(selection.into())
    }
}

#[derive(Debug)]
enum WorkspaceSelection {
    Info,
    Package,
    Back,
}

impl Display for WorkspaceSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkspaceSelection::Info => write!(f, "查看信息"),
            WorkspaceSelection::Package => write!(f, "打包为 MHW MOD (.zip)"),
            WorkspaceSelection::Back => write!(f, "返回"),
        }
    }
}

impl From<usize> for WorkspaceSelection {
    fn from(index: usize) -> Self {
        match index {
            0 => WorkspaceSelection::Info,
            1 => WorkspaceSelection::Package,
            2 => WorkspaceSelection::Back,
            _ => unreachable!(),
        }
    }
}

impl WorkspaceSelection {
    pub fn show_interact() -> anyhow::Result<Self> {
        let selections = &[
            WorkspaceSelection::Info,
            WorkspaceSelection::Package,
            WorkspaceSelection::Back,
        ];
        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("请选择工作区操作： (按↑↓选择，Enter确认)")
            .items(selections)
            .default(0)
            .interact()?;

        Ok(selection.into())
    }
}

#[derive(Debug)]
enum WorkspaceModeSelection {
    Dds,
    Png,
}

impl Display for WorkspaceModeSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkspaceModeSelection::Dds => write!(f, ".dds"),
            WorkspaceModeSelection::Png => write!(f, ".png"),
        }
    }
}

impl From<usize> for WorkspaceModeSelection {
    fn from(index: usize) -> Self {
        match index {
            0 => WorkspaceModeSelection::Dds,
            1 => WorkspaceModeSelection::Png,
            _ => unreachable!(),
        }
    }
}

impl From<WorkspaceModeSelection> for StickerPackType {
    fn from(val: WorkspaceModeSelection) -> Self {
        match val {
            WorkspaceModeSelection::Dds => StickerPackType::Dds,
            WorkspaceModeSelection::Png => StickerPackType::Png,
        }
    }
}

impl WorkspaceModeSelection {
    pub fn show_interact() -> anyhow::Result<Self> {
        let selections = &[WorkspaceModeSelection::Dds, WorkspaceModeSelection::Png];
        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("请选择导出文件格式： (按↑↓选择，Enter确认)\n如果有PS插件，优先选择.dds格式，否则选择.png")
            .items(selections)
            .default(0)
            .interact()?;

        Ok(selection.into())
    }
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, Rgba, RgbaImage};

    #[test]
    fn read_png() -> anyhow::Result<()> {
        // let orig_img = image::open("../../测试项目/chat_stamp00_ID_0.png")?;
        // let ps_img = image::open("../../测试项目/chat_stamp00_ID_0_ps.png")?;
        let orig_img = image::open("../../debug_orig.png")?;
        let ps_img = image::open("../../debug_ps.png")?;

        let DynamicImage::ImageRgba8(orig_buf) = orig_img else {
            return Err(anyhow::anyhow!("not rgba8"));
        };
        let DynamicImage::ImageRgba8(ps_buf) = ps_img else {
            return Err(anyhow::anyhow!("not rgba8"));
        };

        let (width, height) = orig_buf.dimensions();

        // 创建一个新的图像缓冲区，初始化为原始图像的副本
        let mut output_buf: RgbaImage = ps_buf.clone();

        for y in 0..height {
            for x in 0..width {
                let orig_pixel = orig_buf.get_pixel(x, y);
                let ps_pixel = ps_buf.get_pixel(x, y);

                if orig_pixel != ps_pixel && orig_pixel.0[3] != 0 {
                    println!(
                        "Mismatch at ({}, {}): original = {:?}, compared = {:?}",
                        x, y, orig_pixel, ps_pixel
                    );
                    // 将不匹配的像素标记为品红色
                    output_buf.put_pixel(x, y, Rgba([255, 0, 255, 255])); // 品红色
                }
            }
        }

        // 保存输出图像
        output_buf.save("../../diff.png")?;

        Ok(())
    }
}
