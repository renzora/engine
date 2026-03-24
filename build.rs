fn main() {
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
