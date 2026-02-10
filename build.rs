fn main() {
    let profile = std::env::var("PROFILE").unwrap_or_default();

    // In release builds, build the updater and stage it for embedding.
    // In debug builds, nothing to do — assets and plugins load from source.
    if profile == "release" {
        build_and_stage_updater();
    }

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

/// Build the updater as a separate crate and stage the binary into OUT_DIR
/// for `include_bytes!`. The updater has its own Cargo.toml so this doesn't
/// deadlock — it's a different workspace from the one cargo is already building.
fn build_and_stage_updater() {
    println!("cargo:rerun-if-changed=updater/src/main.rs");
    println!("cargo:rerun-if-changed=updater/Cargo.toml");

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let updater_manifest = format!("{}/updater/Cargo.toml", manifest_dir);

    // Build the updater in release mode using its own independent workspace
    let status = std::process::Command::new("cargo")
        .args([
            "build",
            "--release",
            "--manifest-path", &updater_manifest,
        ])
        .status()
        .expect("Failed to run cargo build for updater");

    if !status.success() {
        panic!("Failed to build updater binary");
    }

    // The updater builds into its own target directory: updater/target/release/
    let updater_exe = format!("{}/updater/target/release/renzora_updater.exe", manifest_dir);
    let updater_src = std::path::PathBuf::from(&updater_exe);
    let updater_dst = std::path::PathBuf::from(&out_dir).join("renzora_updater.exe");

    std::fs::copy(&updater_src, &updater_dst)
        .unwrap_or_else(|e| panic!(
            "Failed to copy updater from {} to OUT_DIR: {}", updater_src.display(), e
        ));
}
