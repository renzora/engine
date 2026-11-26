//! The newest glibc a built Linux file asks for.
//!
//! A Linux binary does not need "glibc", it needs particular versions of
//! particular glibc functions, and the linker records the newest version the
//! BUILD machine had of each one it used. So the oldest distro a game runs on is
//! decided by where it was compiled, not by its code. The editor's own releases
//! are built on AlmaLinux 8 to keep that at glibc 2.28; a lean export is compiled
//! on the user's machine and inherits that machine's glibc instead.
//!
//! Read from the file rather than from the host's glibc, because the two differ:
//! a build on a glibc 2.39 machine that happens to use no function newer than
//! 2.34 runs fine on 2.34, and warning about 2.39 would be wrong. The version
//! names sit in the dynamic string table as plain text (`GLIBC_2.34`), so a byte
//! scan finds exactly the set `objdump -T` would print, with no ELF parser. A
//! false hit would need the same text embedded as data, which no game ships.
//! `GLIBCXX_` (libstdc++) never matches, because the scan requires the digit
//! straight after `GLIBC_`.

use std::path::Path;

/// The glibc the editor's own Linux releases need: the AlmaLinux 8 build
/// container's (`.github/scripts/el8-toolchain.sh`). The export compares against
/// it, because "runs where the editor runs" is the promise worth keeping.
pub const RELEASE_FLOOR: (u32, u32) = (2, 28);

/// Format a version as `2.28`.
pub fn display(v: (u32, u32)) -> String {
    format!("{}.{}", v.0, v.1)
}

/// The newest `GLIBC_x.y` named in `bytes`, if any.
pub fn newest_in(bytes: &[u8]) -> Option<(u32, u32)> {
    const TAG: &[u8] = b"GLIBC_";
    let mut best: Option<(u32, u32)> = None;
    let mut i = 0;
    while let Some(off) = find(&bytes[i..], TAG) {
        let start = i + off + TAG.len();
        i = start;
        let (major, after_major) = digits(bytes, start);
        let Some(major) = major else { continue };
        if bytes.get(after_major) != Some(&b'.') {
            continue;
        }
        let (minor, _) = digits(bytes, after_major + 1);
        let Some(minor) = minor else { continue };
        if best.is_none_or(|b| (major, minor) > b) {
            best = Some((major, minor));
        }
    }
    best
}

/// The newest glibc any ELF file under `dir` asks for, and the file that asks
/// for it. A whole directory because a game is more than its executable: the
/// shared libraries and native plugins beside it are compiled on the same
/// machine and raise the floor just as much.
pub fn newest_under(dir: &Path) -> Option<((u32, u32), std::path::PathBuf)> {
    let mut best: Option<((u32, u32), std::path::PathBuf)> = None;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_dir() {
                stack.push(path);
                continue;
            }
            if !kind.is_file() || !is_elf(&path) {
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else { continue };
            if let Some(v) = newest_in(&bytes) {
                if best.as_ref().is_none_or(|(b, _)| v > *b) {
                    best = Some((v, path));
                }
            }
        }
    }
    best
}

/// The warning for a game that needs a newer glibc than the editor's releases,
/// or `None` when it runs everywhere they do.
pub fn warning(needed: (u32, u32), file: &Path) -> Option<String> {
    (needed > RELEASE_FLOOR).then(|| {
        format!(
            "WARN: this game needs glibc {} or newer, because it was compiled on this \
             machine ({} asks for it). Players on older Linux systems cannot start it; \
             the editor's own builds need only glibc {}. To reach them, package as \
             Binary + .rpak, which uses the editor's prebuilt runtime, or run the lean \
             export on an older system (AlmaLinux 8 or Ubuntu 20.04, for example in \
             Distrobox).",
            display(needed),
            file.file_name().map(|n| n.to_string_lossy()).unwrap_or_default(),
            display(RELEASE_FLOOR),
        )
    })
}

fn is_elf(path: &Path) -> bool {
    use std::io::Read;
    let mut magic = [0u8; 4];
    std::fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut magic))
        .is_ok_and(|()| magic == *b"\x7fELF")
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Parse the run of ASCII digits at `at`: its value, and the index after it.
fn digits(bytes: &[u8], at: usize) -> (Option<u32>, usize) {
    let mut end = at;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    let value = std::str::from_utf8(&bytes[at..end]).ok().and_then(|s| s.parse().ok());
    (value, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_newest_version_wins() {
        let table = b"\0GLIBC_2.2.5\0GLIBC_2.34\0GLIBC_2.17\0";
        assert_eq!(newest_in(table), Some((2, 34)));
    }

    #[test]
    fn libstdcxx_and_private_names_are_not_glibc() {
        let table = b"\0GLIBCXX_3.4.30\0GLIBC_PRIVATE\0CXXABI_1.3\0GLIBC_2.28\0";
        assert_eq!(newest_in(table), Some((2, 28)));
    }

    #[test]
    fn a_two_digit_minor_compares_as_a_number() {
        // 2.9 is older than 2.28; a string comparison says otherwise.
        assert_eq!(newest_in(b"GLIBC_2.9\0GLIBC_2.28\0"), Some((2, 28)));
    }

    #[test]
    fn no_versions_means_no_requirement() {
        assert_eq!(newest_in(b"\x7fELF static binary"), None);
        assert_eq!(newest_in(b"GLIBC_"), None);
    }

    #[test]
    fn a_directory_is_judged_by_its_elf_files_only() {
        let d = std::env::temp_dir().join(format!("renzora_glibc_{}", std::process::id()));
        std::fs::create_dir_all(d.join("plugins")).unwrap();
        std::fs::write(d.join("mygame"), b"\x7fELF\0GLIBC_2.17\0GLIBC_2.28\0").unwrap();
        // A plugin compiled on a newer machine raises the whole game's floor.
        std::fs::write(d.join("plugins/librain.so"), b"\x7fELF\0GLIBC_2.34\0").unwrap();
        // Text that names a version is not a requirement.
        std::fs::write(d.join("README.txt"), b"needs GLIBC_2.99").unwrap();
        let (v, file) = newest_under(&d).expect("found");
        assert_eq!(v, (2, 34));
        assert!(file.ends_with("librain.so"), "{}", file.display());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn only_newer_than_the_release_floor_warns() {
        let f = Path::new("mygame");
        assert!(warning((2, 28), f).is_none());
        assert!(warning((2, 17), f).is_none());
        let w = warning((2, 39), f).expect("2.39 warns");
        assert!(w.starts_with("WARN:") && w.contains("2.39") && w.contains("mygame"), "{w}");
    }
}
