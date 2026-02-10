fn main() {
    // Copy plugins/ directory to the target output directory
    copy_plugins_dir();

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

/// Copy the source `plugins/` directory next to the output binary
fn copy_plugins_dir() {
    // Always re-run when plugins/ changes
    println!("cargo:rerun-if-changed=plugins");

    let src_dir = std::path::Path::new("plugins");
    if !src_dir.exists() {
        return;
    }

    // Find the target profile directory (where the binary ends up)
    // Use CARGO_TARGET_DIR or default to "target", then append the profile
    let target_base = std::env::var("CARGO_TARGET_DIR")
        .unwrap_or_else(|_| "target".to_string());
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let target_dir = std::path::PathBuf::from(&target_base).join(&profile);

    let dst_dir = target_dir.join("plugins");

    // Create destination directory
    if let Err(e) = std::fs::create_dir_all(&dst_dir) {
        println!("cargo:warning=Failed to create plugins dir: {}", e);
        return;
    }

    // Copy all plugin files
    if let Ok(entries) = std::fs::read_dir(src_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let file_name = path.file_name().unwrap();
                let dst_path = dst_dir.join(file_name);
                match std::fs::copy(&path, &dst_path) {
                    Ok(_) => println!("cargo:warning=Copied system plugin: {}", file_name.to_string_lossy()),
                    Err(e) => println!("cargo:warning=Failed to copy {}: {}", file_name.to_string_lossy(), e),
                }
            }
        }
    }
}
