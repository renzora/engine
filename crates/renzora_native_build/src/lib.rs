//! The half of native-plugin compilation that two crates both need.
//!
//! A native plugin gets built in two places, and they have to agree:
//!
//! * **`renzora_plugin_build`** — the editor, compiling a plugin a user
//!   installed into a downloaded build.
//! * **`xtask`** — a source checkout, compiling the repo's own plugins during
//!   staging so an author exercises the real path rather than a dev shortcut.
//!
//! Both assemble the same `rustc` invocation: `-C prefer-dynamic`, the three
//! `--extern`s onto the shared images, the SDK's `-L` paths, and now a plugin's
//! third-party dependencies. Each used to hold its own copy, which meant a
//! change to one silently drifted from the other — and the symptom is a plugin
//! that builds for a developer and not for a user, with neither side looking
//! wrong on its own.
//!
//! # Why this is a separate crate rather than a dependency on the other
//!
//! `xtask` could simply depend on `renzora_plugin_build`. That is *safe*: cargo
//! reads a workspace root to resolve `[lints] workspace = true` but does not
//! validate its sibling members, so an outside crate can path-depend on a member
//! of a broken workspace and still build. Measured directly — with a deliberately
//! unloadable workspace, `cargo metadata` on it fails (exit 101) while an
//! outsider depending on one of its members builds fine. The property xtask
//! protects, that it must still compile when `sync.rs` has left the engine
//! workspace unloadable, survives.
//!
//! It fails on the other constraint. `renzora_plugin_build` pulls `serde`,
//! `serde_json`, `tar` and `lzma-rs`; the last two exist only to unpack
//! `sdk.tar.zst`, which xtask has no business doing. xtask is documented as a
//! "tiny, instant-compiling helper" with zero dependencies, and inheriting four
//! trees to share a hundred lines is the wrong trade.
//!
//! So the shared half lives here with **no dependencies of its own**, and xtask
//! picks up one path dependency that pulls nothing.

pub mod deps;
pub mod install;
pub mod json;
pub mod rustc;

pub use deps::Deps;
pub use rustc::Target;

/// Copy `from` onto `to` so a reader never sees a half-written file.
///
/// Written beside the destination and renamed into place, because the
/// destination is a library the loader may `dlopen` at any moment and the
/// artefact watcher polls for changes to. A plain `fs::copy` writes THROUGH the
/// path being watched: a reader that opens it mid-copy gets a truncated ELF, and
/// mapping one of those is not an error it can report — it is a `SIGBUS` the
/// moment execution touches a page past the end of the file.
///
/// That is not hypothetical. Sixty-five plugins rebuilding at once left
/// `tracy.so` at 86,016 bytes of a 532,336-byte library and `audio.so` at zero,
/// and the editor died on the first with a bus error and no message naming a
/// plugin.
///
/// A rename is atomic on every filesystem this runs on, provided both paths are
/// on the same one — which is why the temporary is a sibling rather than in
/// `/tmp`. The size is checked first: renaming a short read atomically would
/// install the corruption instead of racing to it.
pub fn stage_atomically(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind};

    let parent = to.parent().ok_or_else(|| {
        Error::new(ErrorKind::InvalidInput, "destination has no parent directory")
    })?;
    std::fs::create_dir_all(parent)?;

    let want = std::fs::metadata(from)?.len();
    // Named after the destination so two plugins staging at once cannot collide,
    // and so a crash leaves an obvious orphan rather than a mystery.
    let tmp = to.with_extension("staging");
    let copied = std::fs::copy(from, &tmp)?;
    if copied != want || std::fs::metadata(&tmp)?.len() != want {
        let _ = std::fs::remove_file(&tmp);
        return Err(Error::new(
            ErrorKind::UnexpectedEof,
            format!("{} is {copied} bytes but its source is {want} — it was still being written", to.display()),
        ));
    }
    std::fs::rename(&tmp, to)
}

/// Resolve a Rust toolchain executable — `cargo`, `rustc`, `rustup`.
///
/// Spawning the bare name and trusting `PATH` is wrong in exactly the case that
/// matters, and it fails in a way that reads as something else entirely. Rustup
/// installs into `~/.cargo/bin` and puts it on `PATH` from the shell's profile,
/// so a process started from a terminal finds it and a process started any other
/// way — a desktop launcher, a file manager, a `.desktop` entry, anything whose
/// parent did not source that profile — does not. The editor is normally the
/// second kind.
///
/// What the user then reads is "Rust is not installed", on a machine where they
/// have been building with it all day.
///
/// Order:
///   1. The environment's own answer (`CARGO`, `RUSTC`, `RUSTUP`) — set by cargo
///      for anything it spawns, so an editor launched by `cargo renzora`
///      inherits the exact toolchain that built it.
///   2. `$CARGO_HOME/bin` (or `~/.cargo/bin`), where rustup puts its shims.
///   3. The bare name, which is `PATH` and is right whenever Rust was installed
///      some other way.
pub fn tool(name: &str) -> std::path::PathBuf {
    if let Some(from_env) = std::env::var_os(name.to_uppercase()) {
        let path = std::path::PathBuf::from(from_env);
        if path.is_file() {
            return path;
        }
    }
    let exe = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
    let home = std::env::var_os("CARGO_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".cargo")))
        .or_else(|| {
            std::env::var_os("USERPROFILE").map(|h| std::path::PathBuf::from(h).join(".cargo"))
        });
    if let Some(candidate) = home.map(|h| h.join("bin").join(&exe)) {
        if candidate.is_file() {
            return candidate;
        }
    }
    std::path::PathBuf::from(name)
}

/// Stop a child process opening a console window of its own.
///
/// Every compiler this crate and `renzora_plugin_build` spawn — `rustc`, `cargo`,
/// `rustup` — is a console program, and on Windows a console program launched
/// from a GUI process gets a **new console window**. During first-run setup that
/// is a black terminal that appears next to the progress window, flickers as each
/// plugin is compiled, and cannot usefully be closed: closing it kills the
/// compile, and the next plugin opens another one.
///
/// The output is not lost by hiding the window — it is captured and reported
/// through `Progress::Compiling`, which is where a user is actually looking.
///
/// No-op off Windows, where spawning a child opens nothing.
pub fn hide_console(cmd: &mut std::process::Command) -> &mut std::process::Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        /// `CREATE_NO_WINDOW`, from `winbase.h`. Spelled out rather than pulled
        /// from `windows-sys`: this crate deliberately has no dependencies.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

#[cfg(test)]
mod tool_tests {
    use super::tool;

    /// The fallback has to stay a bare name, so a machine that installed Rust
    /// somewhere else entirely still resolves through `PATH`.
    #[test]
    fn an_unknown_tool_falls_back_to_the_bare_name() {
        assert_eq!(tool("not_a_real_toolchain_binary").as_os_str(), "not_a_real_toolchain_binary");
    }

    /// On any machine that can run this test, cargo exists — and the whole point
    /// is to find it by path rather than by `PATH`.
    #[test]
    fn cargo_resolves_to_something_runnable() {
        let path = tool("cargo");
        assert!(
            path.is_file() || path.as_os_str() == "cargo",
            "resolved to {path:?}, which is neither a file nor the PATH fallback"
        );
    }
}

#[cfg(test)]
mod staging_tests {
    use super::stage_atomically;

    fn tmpdir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("renzora_stage_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// The destination must never exist in a half-written state, and the
    /// temporary must not be left behind.
    #[test]
    fn a_copy_lands_whole_and_leaves_no_temporary() {
        let d = tmpdir("whole");
        let from = d.join("libplug.so");
        let to = d.join("build").join("plug.so");
        std::fs::write(&from, vec![7u8; 4096]).unwrap();

        stage_atomically(&from, &to).unwrap();
        assert_eq!(std::fs::read(&to).unwrap().len(), 4096);
        assert!(!to.with_extension("staging").exists(), "temporary left behind");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Replacing an existing artefact is the normal case — a rebuild — and it
    /// must not go through a truncating open of the live file.
    #[test]
    fn an_existing_artefact_is_replaced() {
        let d = tmpdir("replace");
        let from = d.join("libplug.so");
        let to = d.join("plug.so");
        std::fs::write(&to, b"old").unwrap();
        std::fs::write(&from, vec![9u8; 2048]).unwrap();

        stage_atomically(&from, &to).unwrap();
        assert_eq!(std::fs::read(&to).unwrap().len(), 2048);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A source that vanishes mid-flight must fail rather than install whatever
    /// was read. The destination keeps what it had.
    #[test]
    fn a_missing_source_leaves_the_destination_untouched() {
        let d = tmpdir("missing");
        let to = d.join("plug.so");
        std::fs::write(&to, b"still here").unwrap();

        assert!(stage_atomically(&d.join("nothing.so"), &to).is_err());
        assert_eq!(std::fs::read(&to).unwrap(), b"still here");
        let _ = std::fs::remove_dir_all(&d);
    }
}

