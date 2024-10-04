use std::{fs::File, io::BufReader, path::Path};

use anyhow::Context;
use image::DynamicImage;

fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    if args.len() < 2 {
        println!("Usage: {} <input_dir>", args[0]);
        return Ok(());
    }
    let input_dir = &args[1];

    let mut input_paths = vec![];
    for entry in std::fs::read_dir(input_dir).context("无法打开输入目录")? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().unwrap_or_default() == "tex" {
            input_paths.push(path);
        }
    }

    let output_dir = Path::new(input_dir).parent().unwrap().join("output");
    if !output_dir.exists() {
        std::fs::create_dir_all(&output_dir).context("无法创建输出目录")?;
    }

    for input_path in input_paths {
        let mut reader = BufReader::new(File::open(&input_path)?);
        let img = tex_convert::load_tex_image(&mut reader)?;
        let mut img = DynamicImage::ImageRgba8(img);

        let width = 120;
        let height = 86;
        let n_tile = 5;

        let filestem = input_path
            .file_stem()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();
        // crop and output
        for row_index in 0..n_tile {
            let tile = img.crop(0, row_index * height, width, height);
            let file_output = output_dir.join(format!("{}_{}.png", filestem, row_index));
            println!("Writing {}...", file_output.display());
            tile.save(&file_output).context("无法保存图片")?;
        }
    }

    Ok(())
}
