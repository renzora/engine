//! A file that failed builds are written to, so they can be read after the
//! thing that showed them has gone.
//!
//! A plugin build fails in three places and every one of them reports it
//! somewhere transient: the first-run setup window streams rustc's output and
//! then closes, the plugin loader turns it into a Console line that scrolls, and
//! a Rust script's failure lands in a toast. The diagnostic is the part worth
//! keeping — it is rustc's own, written for the author of the code that failed,
//! and it is exactly what anyone answering "why won't my plugin build" needs.
//!
//! So every failure is also appended to `~/.renzora/logs/build.log`, next to the
//! crash reports and for the same reason: somewhere fixed that a user can be
//! told to open, and that survives the editor closing.
//!
//! **Best effort, everywhere.** A build that failed must not then fail
//! *differently* because the log could not be written — every error here is
//! dropped. Nothing reads this file back; it is for a person.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::{crate_name, Error, Manifest};

/// Start a new file once the old one passes this, keeping one previous.
///
/// One failure is usually a few KB, but rustc's output has no upper bound — a
/// plugin that fails to *link* can emit a line per undefined symbol. Capping by
/// size rather than by number of entries is what bounds the worst case.
const MAX_BYTES: u64 = 1 << 20;

/// Overrides the path, or switches the log off with `0`, `off` or `false`.
///
/// For tests (which should not append to the home directory of whoever runs
/// them) and for anyone who wants the file somewhere else.
const ENV: &str = "RENZORA_BUILD_LOG";

/// Where failed builds are recorded, or `None` if nowhere.
///
/// Public because the useful thing to do with a build failure in the UI is offer
/// to open this, and the caller needs the path to do that.
pub fn path() -> Option<PathBuf> {
    match std::env::var_os(ENV) {
        Some(v) if is_off(&v) => None,
        Some(v) => Some(PathBuf::from(v)),
        // `~/.renzora/logs/`, not the SDK or the plugin directory: an install
        // can be read-only (a macOS bundle is signed, a Linux AppImage is a
        // squashfs), and a log that only appears on the machines where the
        // install happens to be writable is worse than no log at all.
        None => Some(dirs::home_dir()?.join(".renzora").join("logs").join("build.log")),
    }
}

fn is_off(v: &std::ffi::OsStr) -> bool {
    matches!(v.to_string_lossy().trim().to_ascii_lowercase().as_str(), "0" | "off" | "false" | "")
}

/// Append one failure.
pub(crate) fn record(manifest: &Manifest, dir: &Path, out: &Path, err: &Error) {
    let Some(path) = path() else { return };
    let _ = append(&path, &entry(manifest, dir, out, err));
}

fn append(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    rotate(path);
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    f.write_all(text.as_bytes())
}

/// Move the log aside once it is large, keeping exactly one previous file.
///
/// Rotating rather than truncating because the failure someone is trying to
/// explain is often the one *before* the one they just saw.
fn rotate(path: &Path) {
    match std::fs::metadata(path) {
        Ok(meta) if meta.len() >= MAX_BYTES => {}
        _ => return,
    }
    let mut old = path.as_os_str().to_os_string();
    old.push(".old");
    let old = PathBuf::from(old);
    // Windows `rename` refuses to replace an existing file where POSIX replaces
    // it silently, so the old one goes first on both.
    let _ = std::fs::remove_file(&old);
    let _ = std::fs::rename(path, &old);
}

/// The text of one entry.
///
/// Split from the writing so it can be tested without a filesystem, and read
/// without one either — this is the whole format.
fn entry(manifest: &Manifest, dir: &Path, out: &Path, err: &Error) -> String {
    format!(
        "\
================================================================================
{time}  {name}  ({kind})

  source:  {src}
  output:  {out}
  rustc:   {rustc}
  target:  {triple}

{err}
",
        time = timestamp(),
        name = crate_name(dir),
        kind = kind(err),
        src = dir.display(),
        out = out.display(),
        rustc = manifest.rustc,
        triple = manifest.triple,
        err = err.to_string().trim_end(),
    )
}

/// A short tag for which kind of failure this was.
///
/// The full text follows it and says more, but the tag is what makes the file
/// skimmable: "toolchain" three times running is a different problem from three
/// different compile errors, and that should be visible without reading them.
fn kind(err: &Error) -> &'static str {
    match err {
        Error::Missing(_) => "no SDK",
        Error::Packed { .. } => "SDK not unpacked",
        Error::Manifest(_) => "bad SDK manifest",
        Error::Toolchain { .. } => "toolchain",
        Error::NoRustc(_) => "rustc would not run",
        Error::Compile(_) => "compile error",
        Error::Deps(_) => "dependencies",
        Error::Io(_) => "io error",
    }
}

/// `YYYY-MM-DD HH:MM:SS UTC`.
///
/// Hand-rolled because this crate has no date dependency and should not grow one
/// for a log header. `renzora_engine`'s crash reporter carries the same
/// `civil_from_days` for its own reason (it must depend on nothing that can
/// fail); the two are not shared because neither crate depends on the other, and
/// an edge between them for one line of formatting would be the worse trade.
fn timestamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    let rem = secs % 86_400;
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC", rem / 3600, (rem % 3600) / 60, rem % 60)
}

/// Days since the Unix epoch to a Gregorian date — Howard Hinnant's exact
/// algorithm, not an approximation with 365-day years.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Manifest {
        serde_json::from_str(
            r#"{
                "triple": "x86_64-pc-windows-msvc",
                "rustc": "1.95.0",
                "lib_ext": "dll",
                "extern": { "bevy": "b.rlib", "renzora": "r.dll" },
                "link_search": { "dependency": [], "native": [] }
            }"#,
        )
        .unwrap()
    }

    /// The entry has to name the plugin, the source the author edits, and carry
    /// rustc's diagnostic whole — those are the three things it exists for.
    #[test]
    fn an_entry_names_the_plugin_and_keeps_the_diagnostic() {
        let err = Error::Compile("error[E0425]: cannot find value `foo`".into());
        let text = entry(&manifest(), Path::new("/plugins/my-plugin"), Path::new("/o/my.dll"), &err);
        assert!(text.contains("my_plugin"), "{text}");
        assert!(text.contains("/plugins/my-plugin"), "{text}");
        assert!(text.contains("compile error"), "{text}");
        assert!(text.contains("error[E0425]: cannot find value `foo`"), "{text}");
        assert!(text.contains("1.95.0") && text.contains("x86_64-pc-windows-msvc"), "{text}");
    }

    /// Every variant gets a tag. A `_ =>` arm would have let a new one through
    /// as something misleading rather than failing here.
    #[test]
    fn every_failure_has_a_tag() {
        assert_eq!(kind(&Error::Compile(String::new())), "compile error");
        assert_eq!(kind(&Error::Deps(String::new())), "dependencies");
        assert_eq!(kind(&Error::NoRustc(String::new())), "rustc would not run");
        assert_eq!(kind(&Error::Missing(PathBuf::new())), "no SDK");
    }

    #[test]
    fn the_log_can_be_switched_off_and_redirected() {
        // Not a real env round-trip (tests share a process), just the predicate
        // `path` asks — which is the part with a wrong answer available.
        for off in ["0", "off", "OFF", "false", "", "  "] {
            assert!(is_off(std::ffi::OsStr::new(off)), "{off:?} should disable the log");
        }
        for on in ["1", "/tmp/build.log", "C:\\logs\\build.log"] {
            assert!(!is_off(std::ffi::OsStr::new(on)), "{on:?} is a path, not a switch");
        }
    }

    /// The dates the crash reporter's approximate version got wrong — leap days
    /// accumulate, so a naive 365-day year drifts by weeks.
    #[test]
    fn dates_are_exact() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(59), (1970, 3, 1));
        // 2024-02-29 — a leap day, in a leap year that is also a century rule
        // exception away from the simple case.
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
        assert_eq!(civil_from_days(20_000), (2024, 10, 4));
    }

    /// Entries accumulate; the file does not grow without bound.
    #[test]
    fn a_large_log_rotates_instead_of_growing() {
        let dir = std::env::temp_dir().join(format!("renzora-buildlog-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let log = dir.join("build.log");

        append(&log, "first\n").unwrap();
        assert!(!log.with_extension("log.old").exists(), "nothing to rotate yet");

        std::fs::write(&log, vec![b'x'; MAX_BYTES as usize + 1]).unwrap();
        append(&log, "second\n").unwrap();

        let old = dir.join("build.log.old");
        assert!(old.exists(), "the oversized log is kept as .old");
        assert_eq!(std::fs::read_to_string(&log).unwrap(), "second\n", "the new log starts fresh");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
