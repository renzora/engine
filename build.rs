fn main() {
    println!("cargo:rerun-if-changed=icon.ico");
    println!("cargo:rerun-if-changed={BRANDING_FILE}");

    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    // zstd-sys native lib: Cargo deduplicates its link metadata to the runtime
    // dylib (renzora_rpak uses zstd), but renzora_export/zip also need it in
    // the exe. Re-emit the link-lib directive so the exe linker finds it.
    //
    // NOT on wasm, where there is no libzstd to find: the C zstd crate cannot
    // build for wasm32-unknown-unknown (no libc sysroot), so `renzora_rpak`
    // decodes with `ruzstd` there instead and nothing in the graph provides the
    // native library. Emitting it anyway got past every compile in the wasm lane
    // and then failed the final link with `unable to find library -lzstd` — the
    // one error a `cargo check` can never surface, since check does not link.
    if target_arch != "wasm32" {
        println!("cargo:rustc-link-lib=static=zstd");
    }

    // Emit engine version and build hash for dynamic plugin compatibility checks.
    let pkg_version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let rustc_ver = std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let hash_input = format!("{pkg_version}-{rustc_ver}-bevy0.19");
    let build_hash = simple_hash(&hash_input);
    println!("cargo:rustc-env=RENZORA_ENGINE_VERSION={pkg_version}");
    println!("cargo:rustc-env=RENZORA_BUILD_HASH={build_hash}");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" {
        let branding = Branding::load();
        if std::env::consts::OS != "windows" && target_env == "msvc" {
            compile_windows_resources_with_llvm_rc(&branding);
        } else {
            let mut res = winres::WindowsResource::new();
            res.set("ProductName", &branding.product_name);
            res.set("FileDescription", &branding.file_description);
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

/// Optional overrides for the Win32 version-info strings, read from a file the
/// **exporter** drops next to `Cargo.toml` in its disposable copy of this tree.
///
/// It is a file rather than an environment variable because a cross-platform
/// export compiles inside a toolchain container: the export workspace is bind-
/// mounted, so a file written into it is visible to the build either way, while
/// `Command::env` would only ever reach the `docker` CLI process on the host.
///
/// Absent — which is always the case in the dev tree and in CI — every field
/// keeps the engine's own branding, so an ordinary build is unchanged.
const BRANDING_FILE: &str = "export-branding.txt";

struct Branding {
    product_name: String,
    file_description: String,
}

impl Branding {
    fn load() -> Self {
        let mut b = Branding {
            product_name: "Renzora Engine".into(),
            file_description: "Renzora Engine Editor".into(),
        };
        let Ok(text) = std::fs::read_to_string(BRANDING_FILE) else {
            return b;
        };
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            // The value ends up inside a double-quoted `.rc` string literal, so
            // a stray quote would not merely look wrong — it would make llvm-rc
            // fail to parse a file the author never sees. Drop the characters
            // that could break out, and cap the length: the version-info block
            // is a fixed-size structure and an unbounded string risks tripping
            // the resource compiler rather than being politely truncated.
            let clean: String = value
                .trim()
                .chars()
                .filter(|c| !c.is_control() && *c != '"' && *c != '\\')
                .take(120)
                .collect();
            if clean.is_empty() {
                continue;
            }
            match key.trim() {
                "product_name" => b.product_name = clean,
                "file_description" => b.file_description = clean,
                _ => {}
            }
        }
        b
    }
}

/// Linux→Windows-MSVC cross-compile: winres can't find rc.exe and llvm-rc has
/// a different CLI, so we compile the .rc file ourselves and tell rustc to
/// link the resulting .res object into the binary.
fn compile_windows_resources_with_llvm_rc(branding: &Branding) {
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
    let product = &branding.product_name;
    let description = &branding.file_description;

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
      VALUE \"ProductName\", \"{product}\"\n\
      VALUE \"FileDescription\", \"{description}\"\n\
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
