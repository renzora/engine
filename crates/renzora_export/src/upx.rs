//! Optional UPX compression of the exported game's binaries.
//!
//! UPX packs an executable and prepends a decompressor stub, so the shipped file
//! is roughly half the size and unpacks itself into memory at launch. It is the
//! one size lever that is entirely post-build: it does not change a single line
//! of what was compiled, so it composes with every capability strip, with
//! `panic = "abort"`, and with the profile knobs — and it is the only one that
//! helps the two *copy* packaging modes at all, since those ship an
//! already-built runtime that no cargo flag can reach any more.
//!
//! ## Why the compressed copy is made BEFORE the rpak is appended
//!
//! A single-binary export is `[executable][rpak][16-byte footer]`, and the
//! runtime finds its assets by reading the last 16 bytes of its own file and
//! seeking back (see `renzora_rpak::format::detect_appended_footer`). Handing
//! that file to UPX would mean trusting its overlay handling to preserve trailing
//! bytes it knows nothing about — recoverable if it drops them (the game just
//! finds no assets), but silent. So the order is inverted instead: pack the bare
//! executable, then append the rpak to the *packed* file. The footer still lands
//! at EOF, the runtime still reads it from disk, and UPX never sees our payload.
//!
//! That ordering is also why compression happens on a temporary copy rather than
//! in place: the lean build's output lives in cargo's `target/dist-lean/`, and
//! packing it there would leave a compressed binary where cargo believes it left
//! a fresh one. The next export would find an up-to-date fingerprint, skip the
//! rebuild, and hand UPX its own output — which fails with `AlreadyPackedException`
//! at best, and at worst double-packs.
//!
//! ## `--best --lzma`, not `--brute`
//!
//! Not a compromise for interactivity — measured, they are the same. On the
//! 187 MB `dist` runtime the two produce a **byte-for-byte identical** file
//! (33,363,456 bytes); `--brute` took 1529 s and `--best --lzma` ~100 s.
//! `--lzma` already selects UPX's strongest compressor, and the extra
//! filter/algorithm combinations `--brute` explores have nothing better to offer
//! on an amd64 PE. (Compared on PE only; ELF and Mach-O were not measured.)
//!
//! Measured on `dist/windows-x64/renzora.exe` (the stripped `dist` runtime):
//! **187.3 MB → 31.7 MB in 82 s, an 83% saving.** The packed binary was run
//! headless with `--server` and booted through the full plugin and scripting
//! startup, so packing an engine binary of this size is not merely legal in
//! theory — nothing about Bevy's startup, the dlopen'd plugins or the self-read
//! of the appended rpak minds a decompressor stub. A lean export compresses less
//! (it is already LTO'd and stripped of most of what compresses well) but in the
//! same league.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::templates::Platform;

/// Environment override for the UPX executable, for a UPX that isn't on `PATH`.
const UPX_ENV: &str = "RENZORA_UPX";

/// Whether UPX can pack this platform's binaries.
///
/// PE and ELF are UPX's two best-supported formats and are packed identically
/// well from any host, so a Windows editor cross-packing a Linux export is fine.
/// The rest are excluded for concrete reasons rather than caution:
///
/// * **macOS** — UPX supports Mach-O, but packing invalidates the code signature,
///   and an unsigned/altered binary is refused outright by Gatekeeper on current
///   macOS. A packed .app would not launch on any machine but the one that built
///   it.
/// * **Android / Fire TV** — the shipped artefact is an APK (a zip), already
///   deflated, and the executable code inside is a `.so` loaded by the platform
///   loader.
/// * **iOS** — a static library, not an executable.
/// * **Web** — `.wasm` is not an executable format UPX knows; the server's gzip
///   or brotli is the equivalent lever.
pub fn supports(platform: Platform) -> bool {
    matches!(platform, Platform::WindowsX64 | Platform::LinuxX64)
}

/// Find a usable `upx`, preferring `$RENZORA_UPX` over `PATH`.
///
/// Probing with `--version` rather than just testing for the file means a broken
/// or wrong-architecture binary is reported as "not found" here, where it can be
/// skipped cleanly, instead of failing later mid-export.
pub fn locate() -> Option<PathBuf> {
    let candidates: Vec<PathBuf> = match std::env::var_os(UPX_ENV) {
        Some(p) if !p.is_empty() => vec![PathBuf::from(p)],
        _ => vec![PathBuf::from("upx")],
    };
    candidates.into_iter().find(|c| {
        Command::new(c)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}

/// Human-readable hint shown when the user asked for UPX and we couldn't find it.
pub fn missing_hint() -> String {
    format!(
        "UPX not found — skipping binary compression. Install it (Windows: `scoop install upx`, \
         Linux: `apt install upx-ucl`, macOS: `brew install upx`) or set {UPX_ENV} to its path."
    )
}

/// Pack `file` in place. Returns `(before, after)` byte sizes on success.
///
/// Used for the sibling shared libraries and plugin libraries of a non-lean
/// export, which are ordinary files with no appended payload — unlike the game
/// binary, which goes through [`compress_to_temp`].
pub fn compress_in_place(upx: &Path, file: &Path) -> Result<(u64, u64), String> {
    let before = std::fs::metadata(file)
        .map_err(|e| format!("stat {}: {e}", file.display()))?
        .len();
    let out = Command::new(upx)
        .args(["--best", "--lzma", "-q"])
        .arg(file)
        .output()
        .map_err(|e| format!("run upx: {e}"))?;
    if !out.status.success() {
        // UPX writes its diagnosis to stderr and leaves the input untouched on
        // failure, so the caller can carry on with the uncompressed file.
        let msg = String::from_utf8_lossy(&out.stderr);
        let msg = msg.lines().last().unwrap_or("unknown error").trim();
        return Err(msg.to_string());
    }
    let after = std::fs::metadata(file)
        .map_err(|e| format!("stat {}: {e}", file.display()))?
        .len();
    Ok((before, after))
}

/// Copy `src` to `tmp` and pack the copy, leaving `src` untouched.
///
/// Returns `(packed_path, before, after)`. See the module docs for why the game
/// binary is packed as a copy and before the rpak is appended.
pub fn compress_to_temp(upx: &Path, src: &Path, tmp: &Path) -> Result<(PathBuf, u64, u64), String> {
    // A stale temp from a cancelled export would already be packed, so start from
    // a fresh copy every time rather than trusting whatever is there.
    let _ = std::fs::remove_file(tmp);
    std::fs::copy(src, tmp).map_err(|e| {
        format!("copy {} → {}: {e}", src.display(), tmp.display())
    })?;
    match compress_in_place(upx, tmp) {
        Ok((before, after)) => Ok((tmp.to_path_buf(), before, after)),
        Err(e) => {
            let _ = std::fs::remove_file(tmp);
            Err(e)
        }
    }
}

/// `12.3 MB → 5.1 MB (58% smaller)`, for the export log.
pub fn savings_line(label: &str, before: u64, after: u64) -> String {
    let mb = |b: u64| b as f64 / (1024.0 * 1024.0);
    let pct = if before > 0 {
        100.0 - (after as f64 / before as f64) * 100.0
    } else {
        0.0
    };
    format!(
        "Compressed {label}: {:.1} MB → {:.1} MB ({:.0}% smaller)",
        mb(before),
        mb(after),
        pct
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_pe_and_elf_are_packed() {
        assert!(supports(Platform::WindowsX64));
        assert!(supports(Platform::LinuxX64));
        // Signed Mach-O, APKs, iOS staticlibs and wasm are all excluded — see
        // `supports`.
        assert!(!supports(Platform::MacOSArm64));
        assert!(!supports(Platform::AndroidArm64));
        assert!(!supports(Platform::WebWasm32));
    }

    #[test]
    fn savings_line_reports_the_ratio() {
        let s = savings_line("game.exe", 100 * 1024 * 1024, 40 * 1024 * 1024);
        assert!(s.contains("100.0 MB"), "{s}");
        assert!(s.contains("40.0 MB"), "{s}");
        assert!(s.contains("60% smaller"), "{s}");
    }

    #[test]
    fn savings_line_survives_an_empty_input() {
        // Guards the division: a 0-byte input must not produce NaN in the log.
        let s = savings_line("empty", 0, 0);
        assert!(s.contains("0% smaller"), "{s}");
    }
}
