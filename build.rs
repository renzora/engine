fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("icon.ico");
        if let Err(e) = res.compile() {
            eprintln!("Warning: Failed to set icon: {}", e);
        }
    }

    #[cfg(feature = "solari")]
    {
        let dlss_sdk =
            std::env::var("DLSS_SDK").expect("DLSS_SDK environment variable not set");
        let out_dir = std::env::var("OUT_DIR").unwrap();
        // OUT_DIR is like .../target/release/build/<pkg>/out
        // Walk up 3 levels to reach the release/ directory
        let mut target_dir = std::path::PathBuf::from(&out_dir);
        for _ in 0..3 {
            target_dir.pop();
        }

        let platform = if cfg!(target_os = "windows") {
            "Windows_x86_64"
        } else {
            "Linux_x86_64"
        };

        let dll_dir = std::path::Path::new(&dlss_sdk)
            .join("lib")
            .join(platform)
            .join("rel");

        let dlls: &[&str] = if cfg!(target_os = "windows") {
            &["nvngx_dlss.dll", "nvngx_dlssd.dll"]
        } else {
            &[
                "libnvidia-ngx-dlss.so.310.4.0",
                "libnvidia-ngx-dlssd.so.310.4.0",
            ]
        };

        for dll in dlls {
            let src = dll_dir.join(dll);
            let dst = target_dir.join(dll);
            if src.exists() {
                std::fs::copy(&src, &dst).ok();
                println!("cargo:warning=Copied {dll} to output directory");
            }
        }

        println!("cargo:rerun-if-env-changed=DLSS_SDK");
    }
}
