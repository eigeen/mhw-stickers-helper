extern crate winres;

fn main() {
    if cfg!(target_os = "windows") {
        let mut res = winres::WindowsResource::new();
        res.set("FileVersion", env!("CARGO_PKG_VERSION"));
        res.set("LegalCopyright", "Copyright (C) 2024 Eigeen");
        res.set(
            "OriginalFilename",
            &format!("{}.dll", env!("CARGO_PKG_NAME")),
        );
        res.set("ProductName", env!("CARGO_PKG_NAME"));
        res.set("ProductVersion", env!("CARGO_PKG_VERSION"));

        res.compile().unwrap();
    }
}
