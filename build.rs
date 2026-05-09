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
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" {
        if std::env::consts::OS != "windows" && target_env == "msvc" {
            compile_windows_resources_with_llvm_rc();
        } else {
            let mut res = winres::WindowsResource::new();
            res.set("ProductName", "Renzora Engine");
            res.set("FileDescription", "Renzora Engine Editor");
            let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
            res.set("ProductVersion", &version);
            res.set("FileVersion", &version);
            if std::path::Path::new("icon.ico").exists() {
                res.set_icon("icon.ico");
            }
            res.compile().expect("Failed to compile Windows resources");
        }
    }
}

/// Linux→Windows-MSVC cross-compile: winres can't find rc.exe and llvm-rc has
/// a different CLI, so we compile the .rc file ourselves and tell rustc to
/// link the resulting .res object into the binary.
fn compile_windows_resources_with_llvm_rc() {
    use std::io::Write;
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let rc_path = format!("{out_dir}/renzora.rc");
    let res_path = format!("{out_dir}/renzora.res");

    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let icon_line = if std::path::Path::new("icon.ico").exists() {
        let abs = std::fs::canonicalize("icon.ico").unwrap();
        format!(
            "1 ICON \"{}\"\n",
            abs.display().to_string().replace('\\', "/")
        )
    } else {
        String::new()
    };

    let mut version_parts: Vec<&str> = version.split('.').collect();
    while version_parts.len() < 4 {
        version_parts.push("0");
    }
    let v_comma = version_parts[..4].join(",");

    let rc_contents = format!(
        "{icon_line}\
1 VERSIONINFO\n\
FILEVERSION {v_comma}\n\
PRODUCTVERSION {v_comma}\n\
BEGIN\n\
  BLOCK \"StringFileInfo\"\n\
  BEGIN\n\
    BLOCK \"040904b0\"\n\
    BEGIN\n\
      VALUE \"ProductName\", \"Renzora Engine\"\n\
      VALUE \"FileDescription\", \"Renzora Engine Editor\"\n\
      VALUE \"ProductVersion\", \"{version}\"\n\
      VALUE \"FileVersion\", \"{version}\"\n\
    END\n\
  END\n\
  BLOCK \"VarFileInfo\"\n\
  BEGIN\n\
    VALUE \"Translation\", 0x0409, 0x04b0\n\
  END\n\
END\n"
    );

    std::fs::File::create(&rc_path)
        .and_then(|mut f| f.write_all(rc_contents.as_bytes()))
        .expect("write .rc file");

    let llvm_rc = ["llvm-rc", "llvm-rc-19", "llvm-rc-20"]
        .iter()
        .find(|name| {
            std::process::Command::new(name)
                .arg("--help")
                .output()
                .is_ok()
        })
        .copied()
        .expect("llvm-rc not found on PATH (tried llvm-rc, llvm-rc-19, llvm-rc-20)");
    let status = std::process::Command::new(llvm_rc)
        .args(["/fo", &res_path, &rc_path])
        .status()
        .expect("run llvm-rc");
    assert!(status.success(), "llvm-rc failed");

    println!("cargo:rustc-link-arg-bins={res_path}");
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
