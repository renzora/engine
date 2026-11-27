//! Code-sign a finished macOS export so it will actually run.
//!
//! # Why every macOS export is signed here
//!
//! Apple Silicon kills an arm64 Mach-O at `exec` when its signature is missing
//! or invalid; Finder reports that as "the application can't be opened" and the
//! shell as `zsh: killed`. A freshly linked binary carries the linker's ad-hoc
//! signature, so a lean export would run locally without this step. A copy-mode
//! export would not: it ships the runtime template, and CI signs that executable
//! with a Developer ID as the *editor* bundle's main executable (identifier
//! `org.renzora.engine`). That signature is bound to the editor's `Info.plist`,
//! so inside the game's bundle `codesign --verify` calls it "invalid Info.plist
//! (plist or signature have been modified)" and the kernel refuses it. A bundle
//! also needs a seal of its own over everything the export moved into it. So
//! every macOS export is re-signed, as the last step of the export.
//!
//! # The three tiers
//!
//! What a developer gets depends only on what they have configured:
//!
//! 1. **Ad-hoc** (the default, no Apple account): `codesign --sign -`. The game
//!    runs on this Mac and on any Mac it reaches without the download
//!    quarantine (a USB stick, AirDrop, itch's app, Steam). A copy a player
//!    downloads in a browser meets Gatekeeper, which they clear once with
//!    right-click -> Open, or System Settings -> Privacy & Security -> "Open
//!    Anyway". This is as far as free goes.
//! 2. **Developer ID** (paid account, no notary key): a real identity plus the
//!    hardened runtime. Signed but not notarized still warns on download, so this
//!    is really the first half of tier 3.
//! 3. **Developer ID + notarization**: sign, `notarytool submit`, `stapler
//!    staple`. Opens with a clean double-click anywhere, online or offline. Only a
//!    `.app` can carry a stapled ticket, so notarization needs the bundle.
//!
//! # Where the rpak goes, and what that costs
//!
//! `codesign` rejects an executable with data appended to it ("main executable
//! failed strict validation"), which is what the single-file packaging modes
//! produce. Inside a `.app` the exporter therefore never appends: the bundle is
//! already the single unit, so the rpak is written beside the executable in
//! `Contents/MacOS/`, where the runtime's adjacent-rpak lookup finds it.
//!
//! A data file in `Contents/MacOS/` is treated as code, and `codesign` stores its
//! signature in extended attributes on the file (`com.apple.cs.*`). Anything
//! that drops xattrs on the way to a player (a plain `zip`, `tar`, git) breaks
//! the seal and the game reads as "damaged". Compress the `.app` with Finder or
//! `ditto -c -k --keepParent`, which keep them. The robust layout is the rpak in
//! `Contents/Resources/`, sealed by hash with no xattrs; it waits on the runtime
//! looking there, because a copy-mode export runs a downloaded template, and a
//! template older than that lookup would not find its game.
//!
//! An unbundled export signs each Mach-O it holds. An executable carrying an
//! appended rpak is signed with the payload detached and then reattached: the
//! signature covers the code up to its own end, so the kernel accepts the
//! result, but `codesign --verify --strict` and Gatekeeper do not, and a bare
//! binary cannot be notarized either. The `.app` is the shape to distribute.
//!
//! # Hosting
//!
//! `codesign`, `xcrun notarytool` and `stapler` exist only on macOS, so a game
//! cross-exported from Linux or Windows cannot be signed at all; [`seal`] says so
//! rather than pretending. Credentials come from the environment for now
//! (`RENZORA_CODESIGN_IDENTITY`, and `RENZORA_NOTARY_KEY` / `_KEY_ID` / `_ISSUER`
//! for notarization); an in-editor settings surface is the next step.

use std::path::{Path, PathBuf};

/// How to sign a macOS export. Built by [`resolve_from_env`].
//
// `#[allow(dead_code)]`: the fields are read only by the `#[cfg(target_os =
// "macos")]` implementation below. On a Linux or Windows host the editor still
// compiles this (and CI lints it with `-D warnings`), but the only thing it can
// do with a macOS export is decline to sign it, so the data goes unread there.
#[allow(dead_code)]
pub enum MacSigning {
    /// Ad-hoc seal. Free, no account.
    AdHoc,
    /// A Developer ID identity, optionally notarized and stapled.
    DeveloperId {
        /// The common name or SHA-1 of a "Developer ID Application" identity in
        /// the login keychain, as `security find-identity -v -p codesigning`
        /// prints it.
        identity: String,
        /// App Store Connect API credentials, when notarization is wanted.
        notary: Option<Notary>,
    },
}

/// App Store Connect API key for `notarytool`.
#[allow(dead_code)] // read only in the macOS `seal` path; see `MacSigning`.
pub struct Notary {
    /// The `.p8` private-key file.
    pub key_path: PathBuf,
    /// The key's ID (the `AuthKey_<id>.p8` middle, or the console's "Key ID").
    pub key_id: String,
    /// The issuer UUID from App Store Connect -> Users and Access -> Integrations.
    pub issuer: String,
}

/// Read the signing configuration from the environment, falling back to ad-hoc.
///
/// A Developer ID is used only when `RENZORA_CODESIGN_IDENTITY` is set and
/// non-empty; notarization only when all three notary variables are present too.
/// Anything short of that is ad-hoc, which is the right default: it is the one
/// tier that needs nothing and always leaves a runnable local build.
pub fn resolve_from_env() -> MacSigning {
    match std::env::var("RENZORA_CODESIGN_IDENTITY") {
        Ok(id) if !id.trim().is_empty() => {
            let notary = match (
                std::env::var("RENZORA_NOTARY_KEY"),
                std::env::var("RENZORA_NOTARY_KEY_ID"),
                std::env::var("RENZORA_NOTARY_ISSUER"),
            ) {
                (Ok(k), Ok(kid), Ok(iss))
                    if !k.is_empty() && !kid.is_empty() && !iss.is_empty() =>
                {
                    Some(Notary {
                        key_path: PathBuf::from(k),
                        key_id: kid,
                        issuer: iss,
                    })
                }
                _ => None,
            };
            MacSigning::DeveloperId {
                identity: id.trim().to_string(),
                notary,
            }
        }
        _ => MacSigning::AdHoc,
    }
}

/// Sign everything an exported macOS game ships.
///
/// `bundled_app` is the `.app` when the export was wrapped, and then it is the
/// only thing signed: the wrap moved every file the export wrote into it.
/// Unwrapped, `output_dir` is the per-project export folder and each Mach-O in it
/// is signed. Best-effort by contract: the caller reports an `Err` as a warning,
/// because a game that failed to sign is still a game the developer can inspect.
#[cfg(not(target_os = "macos"))]
pub fn seal(
    _output_dir: &Path,
    _bundled_app: Option<&Path>,
    _cfg: &MacSigning,
    _progress: &mut dyn FnMut(String),
) -> Result<(), String> {
    Err("a macOS build can only be code-signed on a macOS host".to_string())
}

#[cfg(target_os = "macos")]
pub fn seal(
    output_dir: &Path,
    bundled_app: Option<&Path>,
    cfg: &MacSigning,
    progress: &mut dyn FnMut(String),
) -> Result<(), String> {
    // Removed on drop, so no early return can leave it behind in the temp dir.
    let entitlements = match cfg {
        MacSigning::DeveloperId { .. } => {
            let file = TempFile::new("entitlements", "plist");
            std::fs::write(file.path(), ENTITLEMENTS)
                .map_err(|e| format!("write entitlements: {e}"))?;
            Some(file)
        }
        MacSigning::AdHoc => None,
    };
    let (identity, notary) = match cfg {
        MacSigning::DeveloperId { identity, notary } => (Some(identity.as_str()), notary.as_ref()),
        MacSigning::AdHoc => (None, None),
    };
    let signer = Signer {
        identity,
        entitlements: entitlements.as_ref().map(TempFile::path),
    };

    if let Some(app) = bundled_app {
        let name = app.file_name().unwrap_or_default().to_string_lossy();
        progress(match identity {
            Some(id) => format!(
                "Signing {name} with {id} (approve the keychain prompt, then \"Always Allow\")..."
            ),
            None => format!("Ad-hoc signing {name}..."),
        });
        signer.sign_bundle(app)?;
        match (identity, notary) {
            (Some(_), Some(n)) => notarize_and_staple(app, n, progress)?,
            (Some(_), None) => progress(NOT_NOTARIZED.to_string()),
            (None, _) => progress(AD_HOC_DONE.to_string()),
        }
        return Ok(());
    }

    // Unwrapped. A failure on one file must not leave every file after it
    // unsigned, so collect them and report together.
    let (with_payload, plain): (Vec<PathBuf>, Vec<PathBuf>) =
        macho_files(output_dir).into_iter().partition(|f| payload_len(f).is_some());
    let mut failures = Vec::new();
    if let Err(e) = signer.sign_files(&plain) {
        failures.push(e);
    }
    for exe in &with_payload {
        if let Err(e) = with_payload_detached(exe, || signer.sign_files(std::slice::from_ref(exe))) {
            failures.push(e);
        }
    }
    if !failures.is_empty() {
        return Err(failures.join("; "));
    }
    progress(match (identity, notary) {
        (Some(_), Some(_)) => {
            "Signed with your Developer ID. Notarization needs the .app: turn on \"Wrap as a .app bundle\" to notarize."
                .to_string()
        }
        (Some(_), None) => NOT_NOTARIZED.to_string(),
        (None, _) => AD_HOC_DONE.to_string(),
    });
    Ok(())
}

// ── macOS-only machinery ─────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
const AD_HOC_DONE: &str =
    "Ad-hoc signed: runs on this Mac; a downloaded copy needs right-click -> Open once.";

#[cfg(target_os = "macos")]
const NOT_NOTARIZED: &str = "Signed with your Developer ID, not notarized: a downloaded copy still warns until a notary key is set.";

/// The one entitlement a notarized game cannot live without: the hardened
/// runtime enforces library validation (load only code signed by our own team),
/// and a game `dlopen`s plugins the player's machine compiled under no team at
/// all. Kept in lockstep with `scripts/macos-entitlements.plist`, which does the
/// same for the editor, but embedded here because the editor's install has no
/// copy of the repo's `scripts/` beside it to read.
#[cfg(target_os = "macos")]
const ENTITLEMENTS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.security.cs.disable-library-validation</key>
    <true/>
</dict>
</plist>
"#;

/// The `codesign` arguments for one tier, shared by every invocation.
#[cfg(target_os = "macos")]
struct Signer<'a> {
    /// `None` signs ad-hoc.
    identity: Option<&'a str>,
    entitlements: Option<&'a Path>,
}

#[cfg(target_os = "macos")]
impl Signer<'_> {
    fn command(&self) -> std::process::Command {
        let mut c = std::process::Command::new("codesign");
        c.arg("--force");
        match (self.identity, self.entitlements) {
            (Some(id), Some(ents)) => {
                // `--timestamp` because a signature without one expires with the
                // certificate, and notarization rejects that outright.
                c.args(["--timestamp", "--options", "runtime", "--entitlements"])
                    .arg(ents)
                    .args(["--sign", id]);
            }
            _ => {
                c.args(["--sign", "-"]);
            }
        }
        c
    }

    /// Seal a whole bundle with one `--deep` invocation.
    ///
    /// The game's rpak and `plugins/` sit in `Contents/MacOS/` beside the
    /// executable. An inside-out pass (each Mach-O, then the bundle) leaves the
    /// rpak unsealed, and `codesign --verify` rejects the bundle with "code object
    /// is not signed at all / In subcomponent: <...>.rpak". `--deep` seals it, and
    /// does so in one process, which is one keychain prompt instead of one per
    /// file. It applies the runtime options and entitlements to every nested item;
    /// that is harmless, because library validation is decided by the main
    /// executable's entitlement and a dylib carrying the same one changes nothing.
    /// CI signs the *editor* inside-out (`.github/workflows/build-engine.yml`)
    /// because its `Contents/MacOS/` holds only code.
    fn sign_bundle(&self, app: &Path) -> Result<(), String> {
        let mut c = self.command();
        c.arg("--deep").arg(app);
        run(&mut c, &format!("sign {}", app.display())).map(|_| ())
    }

    /// Sign every path in one `codesign` invocation: each process unlocks the key
    /// once, so a batch is one keychain prompt where a per-file loop asked for the
    /// password once per dylib. Signing nothing is not an error.
    fn sign_files(&self, paths: &[PathBuf]) -> Result<(), String> {
        if paths.is_empty() {
            return Ok(());
        }
        let mut c = self.command();
        c.args(paths);
        run(&mut c, &format!("sign {} file(s)", paths.len())).map(|_| ())
    }
}

/// A uniquely named file in the temp dir, removed when dropped.
///
/// Unique per call as well as per process, so two exports in one editor session
/// never share (and delete) each other's entitlements or notarization archive.
#[cfg(target_os = "macos")]
struct TempFile(PathBuf);

#[cfg(target_os = "macos")]
impl TempFile {
    fn new(stem: &str, ext: &str) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        Self(std::env::temp_dir().join(format!(
            "renzora-export-{stem}-{}-{n}.{ext}",
            std::process::id()
        )))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

#[cfg(target_os = "macos")]
impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Is `path` a Mach-O object (thin or fat), by its leading magic?
///
/// A byte test rather than an extension one, because the files that must be
/// signed are the executable (no extension) and the `*.dylib`s, while the things
/// that must not are the `.rpak`, the icon and `Info.plist`. Thin 32/64-bit and
/// fat 32/64-bit magics, in both byte orders; the fat `0xCAFEBABE` is shared with
/// Java class files, which an export never contains.
#[cfg(target_os = "macos")]
fn is_macho(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let mut magic = [0u8; 4];
    if f.read_exact(&mut magic).is_err() {
        return false;
    }
    matches!(
        magic,
        [0xCA, 0xFE, 0xBA, 0xBE]
            | [0xBE, 0xBA, 0xFE, 0xCA]
            | [0xCA, 0xFE, 0xBA, 0xBF]
            | [0xBF, 0xBA, 0xFE, 0xCA]
            | [0xCF, 0xFA, 0xED, 0xFE]
            | [0xCE, 0xFA, 0xED, 0xFE]
            | [0xFE, 0xED, 0xFA, 0xCF]
            | [0xFE, 0xED, 0xFA, 0xCE]
    )
}

/// Every Mach-O under `root`, skipping any `.app` in it.
///
/// `root` is the per-project export folder, which is not emptied between
/// exports, so it can hold the bundle a previous export with wrapping on left
/// behind. Signing that bundle's files one by one would break its seal.
#[cfg(target_os = "macos")]
fn macho_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let p = entry.path();
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            if ft.is_dir() {
                if p.extension().is_some_and(|e| e == "app") {
                    continue;
                }
                stack.push(p);
            } else if ft.is_file() && is_macho(&p) {
                out.push(p);
            }
        }
    }
    out
}

/// Bytes of rpak (archive plus footer) appended to `path`, if it carries one.
#[cfg(target_os = "macos")]
fn payload_len(path: &Path) -> Option<u64> {
    use std::io::{Read, Seek, SeekFrom};
    let footer_len = renzora_rpak::FOOTER_LEN;
    let mut f = std::fs::File::open(path).ok()?;
    let size = f.metadata().ok()?.len();
    if size < footer_len {
        return None;
    }
    f.seek(SeekFrom::End(-(footer_len as i64))).ok()?;
    let mut last = [0u8; 16];
    f.read_exact(&mut last).ok()?;
    let (_, total) = renzora_rpak::detect_appended_footer(size, &last)?;
    Some(total + footer_len)
}

/// Run `sign` with the rpak payload cut off `exe`, then put the payload back.
///
/// `codesign` refuses a Mach-O with trailing data, so the payload comes off
/// first. The footer locates the archive from the end of the file, so after the
/// new signature changes the executable's length, reattaching the same bytes
/// still yields a binary the runtime reads. The payload goes back whether or not
/// signing succeeded: a failure must leave the game exactly as it was.
#[cfg(target_os = "macos")]
fn with_payload_detached(
    exe: &Path,
    sign: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    use std::io::{Read, Seek, SeekFrom, Write};
    let io = |e: std::io::Error| format!("{}: {e}", exe.display());
    let tail_len = payload_len(exe).ok_or_else(|| format!("{}: no rpak payload", exe.display()))?;
    let mut f = std::fs::OpenOptions::new().read(true).write(true).open(exe).map_err(io)?;
    let size = f.metadata().map_err(io)?.len();
    let mut tail = vec![0u8; tail_len as usize];
    f.seek(SeekFrom::Start(size - tail_len)).map_err(io)?;
    f.read_exact(&mut tail).map_err(io)?;
    f.set_len(size - tail_len).map_err(io)?;
    drop(f);

    let signed = sign();

    let reattached = std::fs::OpenOptions::new()
        .append(true)
        .open(exe)
        .and_then(|mut f| f.write_all(&tail))
        .map_err(|e| format!("{}: could not reattach the game data: {e}", exe.display()));
    match (signed, reattached) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(s), Ok(())) => Err(s),
        (Ok(()), Err(r)) => Err(r),
        (Err(s), Err(r)) => Err(format!("{s}; {r}")),
    }
}

/// Run a tool and return its stdout. On failure the error carries stderr *and*
/// stdout, because `notarytool` prints the submission id and status on stdout,
/// and without the id there is no `notarytool log` to read the reasons from.
#[cfg(target_os = "macos")]
fn run(cmd: &mut std::process::Command, what: &str) -> Result<String, String> {
    let out = cmd.output().map_err(|e| format!("{what}: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if out.status.success() {
        return Ok(stdout);
    }
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let detail: Vec<&str> = [stderr.as_str(), stdout.as_str()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    if detail.is_empty() {
        Err(format!("{what}: exited with {}", out.status))
    } else {
        Err(format!("{what}: {}", detail.join(" | ")))
    }
}

#[cfg(target_os = "macos")]
fn notarize_and_staple(
    app: &Path,
    n: &Notary,
    progress: &mut dyn FnMut(String),
) -> Result<(), String> {
    let zip = TempFile::new("notarize", "zip");

    // `ditto`, not `zip`: notarytool takes an archive, and only ditto preserves
    // the bundle's structure and the xattr-stored signatures of its data files.
    progress("Zipping the app for notarization...".to_string());
    let mut z = std::process::Command::new("ditto");
    z.args(["-c", "-k", "--keepParent"]).arg(app).arg(zip.path());
    run(&mut z, "ditto for notarization")?;

    progress("Submitting to Apple's notary service (this can take a few minutes)...".to_string());
    let mut s = std::process::Command::new("xcrun");
    s.args(["notarytool", "submit"])
        .arg(zip.path())
        .arg("--key")
        .arg(&n.key_path)
        .args(["--key-id", &n.key_id, "--issuer", &n.issuer, "--wait", "--timeout", "30m"]);
    let out = run(&mut s, "notarytool submit")?;
    // The verdict is the status line, not the exit code: `--wait` returns once
    // processing finishes, and a rejected submission is a finished one.
    if !out.to_ascii_lowercase().contains("status: accepted") {
        return Err(format!(
            "notarization was not accepted; `xcrun notarytool log <id>` gives the reasons: {out}"
        ));
    }

    // Staple the ticket INTO the bundle so a machine with no network clears
    // Gatekeeper on first launch.
    progress("Stapling the notarization ticket...".to_string());
    let mut st = std::process::Command::new("xcrun");
    st.args(["stapler", "staple"]).arg(app);
    run(&mut st, "stapler staple")?;

    progress("Notarized and stapled: opens with a clean double-click anywhere.".to_string());
    Ok(())
}

/// These run the real `codesign`, so they exist only on a macOS host. The
/// signed subject is a copy of this test binary: a genuine arm64 Mach-O that
/// exits cleanly on `--list`, which is how a test proves the kernel accepted the
/// new signature rather than merely that `codesign` printed nothing.
#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("renzora_sign_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn copy_of_this_binary(to: &Path) {
        std::fs::copy(std::env::current_exe().unwrap(), to).unwrap();
    }

    /// An rpak-shaped tail: 64 bytes of archive plus the 16-byte footer.
    fn payload() -> Vec<u8> {
        let mut p = vec![0xABu8; 64];
        p.extend_from_slice(&64u64.to_le_bytes());
        p.extend_from_slice(renzora_rpak::RPAK_MAGIC);
        p.extend_from_slice(&[0, 0, 0, 0]);
        p
    }

    fn runs(exe: &Path) -> bool {
        std::process::Command::new(exe)
            .arg("--list")
            .output()
            .is_ok_and(|o| o.status.success())
    }

    /// The single-file modes outside a bundle: the payload is detached for
    /// signing, comes back byte for byte, and the result still launches.
    #[test]
    fn an_appended_payload_survives_signing_and_the_binary_still_runs() {
        let d = scratch("payload");
        let exe = d.join("game");
        copy_of_this_binary(&exe);
        let tail = payload();
        {
            use std::io::Write;
            std::fs::OpenOptions::new()
                .append(true)
                .open(&exe)
                .unwrap()
                .write_all(&tail)
                .unwrap();
        }
        assert_eq!(payload_len(&exe), Some(tail.len() as u64));

        let mut log = Vec::new();
        seal(&d, None, &MacSigning::AdHoc, &mut |m| log.push(m)).unwrap();

        let bytes = std::fs::read(&exe).unwrap();
        assert!(bytes.ends_with(&tail), "the rpak payload must come back intact");
        assert!(runs(&exe), "a signed binary with its payload reattached must launch");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A bundle holding a data file in `Contents/MacOS/` (where the rpak lives)
    /// seals and passes the strict check Gatekeeper makes.
    #[test]
    fn a_bundle_with_the_rpak_beside_the_executable_verifies_strictly() {
        let d = scratch("bundle");
        let app = d.join("Game.app");
        let macos = app.join("Contents").join("MacOS");
        std::fs::create_dir_all(&macos).unwrap();
        copy_of_this_binary(&macos.join("game"));
        std::fs::write(macos.join("game.rpak"), payload()).unwrap();
        std::fs::write(
            app.join("Contents").join("Info.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>game</string>
<key>CFBundleIdentifier</key><string>org.renzora.signtest</string>
<key>CFBundlePackageType</key><string>APPL</string>
</dict></plist>
"#,
        )
        .unwrap();

        let mut log = Vec::new();
        seal(&d, Some(&app), &MacSigning::AdHoc, &mut |m| log.push(m)).unwrap();

        let verified = std::process::Command::new("codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(&app)
            .status()
            .unwrap();
        assert!(verified.success(), "the sealed bundle must verify strictly");
        assert!(runs(&macos.join("game")));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// An unwrapped export must not re-sign, file by file, a bundle an earlier
    /// export left in the same folder: that would break the bundle's seal.
    #[test]
    fn the_unwrapped_pass_skips_bundles_left_in_the_folder() {
        let d = scratch("skip");
        let old = d.join("Old.app").join("Contents").join("MacOS");
        std::fs::create_dir_all(&old).unwrap();
        copy_of_this_binary(&old.join("game"));
        copy_of_this_binary(&d.join("game"));
        assert_eq!(macho_files(&d), vec![d.join("game")]);
        let _ = std::fs::remove_dir_all(&d);
    }
}
