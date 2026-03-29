fn main() {
    println!("cargo:rerun-if-changed=icon.ico");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        #[cfg(windows)]
        {
            let mut res = winres::WindowsResource::new();
            res.set("ProductName", "Renzora Engine");
            res.set("FileDescription", "Renzora Engine Editor");
            res.set("ProductVersion", env!("CARGO_PKG_VERSION"));
            res.set("FileVersion", env!("CARGO_PKG_VERSION"));
            if std::path::Path::new("icon.ico").exists() {
                res.set_icon("icon.ico");
            }
            res.compile().expect("Failed to compile Windows resources");
        }
    }
}
