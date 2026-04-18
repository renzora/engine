fn main() {
    println!("cargo:rerun-if-changed=icon.ico");

    // zstd-sys native lib: Cargo deduplicates its link metadata to the runtime
    // dylib (renzora_rpak uses zstd), but renzora_export/zip also need it in
    // the exe. Re-emit the link-lib directive so the exe linker finds it.
    println!("cargo:rustc-link-lib=static=zstd");

    // Emit engine version and build hash for dynamic plugin compatibility checks.
    let pkg_version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let rustc_ver = std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let hash_input = format!("{pkg_version}-{rustc_ver}-bevy0.18");
    let build_hash = simple_hash(&hash_input);
    println!("cargo:rustc-env=RENZORA_ENGINE_VERSION={pkg_version}");
    println!("cargo:rustc-env=RENZORA_BUILD_HASH={build_hash}");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set("ProductName", "Renzora Engine");
        res.set("FileDescription", "Renzora Engine Editor");
        let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
        res.set("ProductVersion", &version);
        res.set("FileVersion", &version);
        if std::path::Path::new("icon.ico").exists() {
            res.set_icon("icon.ico");
        }

        // Set windres path for cross-compilation if we are not on Windows
        if std::env::consts::OS != "windows" {
            res.set_toolkit_path("/usr/bin");
            res.set_windres_path("/usr/bin/x86_64-w64-mingw32-windres");
            res.set_ar_path("/usr/bin/x86_64-w64-mingw32-ar");
        }

        res.compile().expect("Failed to compile Windows resources");
    }
}

/// Simple deterministic hash (FNV-1a) — no crypto dependency needed in build script.
fn simple_hash(input: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
