//! Whether this machine can build the engine with NVIDIA DLSS, and staging the
//! DLSS libraries beside a build that has it.
//!
//! DLSS is on whenever it can be. The `dlss` feature compiles `dlss_wgpu`,
//! whose build script needs the DLSS SDK (`DLSS_SDK`), the Vulkan SDK
//! (`VULKAN_SDK`) and clang, and panics without them, so the feature is never
//! in a default set: CI and most machines have none of them. Instead every
//! `cargo renzora` build checks for them here and passes `--features dlss`
//! when they are all present. When one is missing it says which, once, and
//! builds as before; the engine then uses Solari's own denoiser.
//!
//! The version is checked rather than assumed. `dlss_wgpu` is written against
//! one SDK release, and an older SDK under `DLSS_SDK` (the obvious mistake when
//! two are unpacked side by side) compiles bindings that do not match its
//! libraries. The release is read from `nvngx_dlss.dll`'s version resource,
//! since the SDK ships no version file.
//!
//! `RENZORA_DLSS=0` turns it off on a machine that has everything.

use std::path::{Path, PathBuf};

/// The DLSS SDK release `dlss_wgpu` 6 is built against.
const REQUIRED: (u16, u16, u16) = (310, 9, 1);

/// The DLSS SDK to build against, or `None` with the reason printed.
pub fn detect() -> Option<PathBuf> {
    if std::env::var("RENZORA_DLSS").is_ok_and(|v| v == "0") {
        println!("[xtask] DLSS: off (RENZORA_DLSS=0)");
        return None;
    }
    if !cfg!(all(target_arch = "x86_64", any(target_os = "windows", target_os = "linux"))) {
        return None;
    }
    let Some(sdk) = std::env::var_os("DLSS_SDK").map(PathBuf::from) else {
        println!(
            "[xtask] DLSS: off (DLSS_SDK is not set; see docs/r1-alpha8/rendering/dlss.md). \
             Solari uses its built-in denoiser."
        );
        return None;
    };
    let why_not = check(&sdk);
    match why_not {
        None => {
            println!("[xtask] DLSS: on ({})", sdk.display());
            Some(sdk)
        }
        Some(why) => {
            println!(
                "[xtask] DLSS: off ({why}). Solari uses its built-in denoiser."
            );
            None
        }
    }
}

/// Why `sdk` cannot be built against, or `None` if it can.
fn check(sdk: &Path) -> Option<String> {
    if !sdk.is_absolute() || !sdk.join("include").is_dir() {
        return Some(format!("DLSS_SDK={} is not an absolute path to the SDK", sdk.display()));
    }
    let lib = lib_dir(sdk);
    // `dlss_wgpu`'s build script links from `x64/` on Windows and panics
    // without it.
    let link_dir = if cfg!(windows) { lib.join("x64") } else { lib.clone() };
    if !link_dir.is_dir() {
        return Some(format!("{} is missing", link_dir.display()));
    }
    match sdk_version(sdk) {
        Some(v) if v == REQUIRED => {}
        Some((a, b, c)) => {
            let (ra, rb, rc) = REQUIRED;
            return Some(format!(
                "DLSS_SDK is release {a}.{b}.{c}, the engine needs {ra}.{rb}.{rc}"
            ));
        }
        None => return Some(format!("no DLSS library under {}", lib.join("rel").display())),
    }
    match std::env::var_os("VULKAN_SDK") {
        Some(v) if Path::new(&v).is_dir() => None,
        _ => Some("VULKAN_SDK is not set to an installed Vulkan SDK".into()),
    }
}

fn lib_dir(sdk: &Path) -> PathBuf {
    let platform = if cfg!(windows) { "Windows_x86_64" } else { "Linux_x86_64" };
    sdk.join("lib").join(platform)
}

/// The libraries a DLSS build loads at run time: Super Resolution and Ray
/// Reconstruction. Missing either is not fatal (the engine falls back), but
/// Solari's Ray Reconstruction needs the second.
fn runtime_libraries(sdk: &Path) -> Vec<PathBuf> {
    let rel = lib_dir(sdk).join("rel");
    if cfg!(windows) {
        vec![rel.join("nvngx_dlss.dll"), rel.join("nvngx_dlssd.dll")]
    } else {
        let (a, b, c) = REQUIRED;
        vec![
            rel.join(format!("libnvidia-ngx-dlss.so.{a}.{b}.{c}")),
            rel.join(format!("libnvidia-ngx-dlssd.so.{a}.{b}.{c}")),
        ]
    }
}

/// The SDK release, read from its Super Resolution library.
fn sdk_version(sdk: &Path) -> Option<(u16, u16, u16)> {
    let library = runtime_libraries(sdk).into_iter().next()?;
    if !cfg!(windows) {
        // On Linux the release is in the file name, and that file existing is
        // the check.
        return library.is_file().then_some(REQUIRED);
    }
    let bytes = std::fs::read(library).ok()?;
    pe_file_version(&bytes)
}

/// The file version in a Windows binary's `VS_FIXEDFILEINFO`, found by its
/// signature rather than by walking the resource tree, which would be a
/// hundred lines for the same answer.
///
/// The search starts at the UTF-16 `VS_VERSION_INFO` key the block follows.
/// The signature's four bytes also occur by chance in the code of a library
/// this size, and taking the first one anywhere in the file read DLSS 310.9.1
/// as version 46863.0.46863.
fn pe_file_version(bytes: &[u8]) -> Option<(u16, u16, u16)> {
    const SIGNATURE: [u8; 4] = 0xFEEF_04BDu32.to_le_bytes();
    let key: Vec<u8> = "VS_VERSION_INFO".encode_utf16().flat_map(u16::to_le_bytes).collect();
    let from = bytes.windows(key.len()).position(|w| w == key.as_slice())?;
    let at = from + bytes[from..].windows(4).position(|w| w == SIGNATURE)?;
    let word = |offset: usize| -> Option<u32> {
        let b = bytes.get(at + offset..at + offset + 4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    // After the signature: the structure version, then the file version as
    // two words, major.minor and build.revision.
    let (high, low) = (word(8)?, word(12)?);
    Some(((high >> 16) as u16, high as u16, (low >> 16) as u16))
}

/// Copy the DLSS libraries and NVIDIA's licence beside a staged build. The
/// licence travels with the libraries because the SDK's terms require it of
/// anything that redistributes them.
pub fn stage(sdk: &Path, out: &Path) -> std::io::Result<()> {
    for library in runtime_libraries(sdk) {
        let Some(name) = library.file_name() else { continue };
        if library.is_file() {
            std::fs::copy(&library, out.join(name))?;
        } else {
            eprintln!("[xtask] WARN: {} missing; DLSS will not start", library.display());
        }
    }
    let license = sdk.join("LICENSE.txt");
    if license.is_file() {
        std::fs::copy(license, out.join("DLSS_LICENSE.txt"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_version_after_the_signature() {
        // A stray copy of the signature ahead of the resource, as in a real
        // library's code, must not be the one read.
        let mut image = 0xFEEF_04BDu32.to_le_bytes().to_vec();
        image.extend_from_slice(&[0xAA; 12]);
        image.extend("VS_VERSION_INFO".encode_utf16().flat_map(u16::to_le_bytes));
        image.extend_from_slice(&[0u8; 2]);
        image.extend_from_slice(&0xFEEF_04BDu32.to_le_bytes());
        image.extend_from_slice(&0x0001_0000u32.to_le_bytes());
        image.extend_from_slice(&((310u32 << 16) | 9).to_le_bytes());
        image.extend_from_slice(&(1u32 << 16).to_le_bytes());
        assert_eq!(pe_file_version(&image), Some((310, 9, 1)));
    }

    #[test]
    fn no_signature_no_version() {
        assert_eq!(pe_file_version(&[0u8; 64]), None);
    }
}
