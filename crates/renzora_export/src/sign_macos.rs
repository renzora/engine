//! Code-sign a finished macOS export so it will actually run.
//!
//! # Why this is not optional
//!
//! On Apple Silicon the kernel refuses to `exec` a Mach-O that is not at least
//! ad-hoc signed: it SIGKILLs the process before `dyld` runs, which Finder
//! reports as "the application can't be opened" and the shell as `zsh: killed`.
//! An export with no signing step is therefore dead on arrival on every arm64
//! Mac, and that is exactly what shipped before this module existed. The engine's
//! own `dist/` is sealed ad-hoc by `xtask`, and the CI release lane signs the
//! `.app` with a Developer ID (see `.github/workflows/build-engine.yml`); an
//! exported *game* needs the same, done here from inside the editor.
//!
//! # The three tiers
//!
//! What a developer can produce depends only on what they have configured:
//!
//! 1. **Ad-hoc** (the default, no Apple account): `codesign --sign -`. Makes the
//!    binary a *valid* signed image so it runs locally and when copied by hand. A
//!    copy the player *downloads* is quarantined and still meets Gatekeeper, which
//!    they clear once with right-click -> Open. This is as far as free goes, and
//!    it is enough for local testing and for distribution channels that do not
//!    quarantine (itch's app, Steam, a USB stick).
//! 2. **Developer ID** (paid account, no notary key): a real identity plus the
//!    hardened runtime. Signed-but-not-notarized still warns on download, so this
//!    is really just the first half of tier 3.
//! 3. **Developer ID + notarization**: sign -> `notarytool submit` -> `stapler
//!    staple`. Opens with a clean double-click anywhere, online or offline. Only a
//!    `.app` can carry a stapled ticket, so notarization needs the bundle toggle.
//!
//! # Hosting
//!
//! `codesign`, `xcrun notarytool` and `stapler` exist only on macOS, so a game
//! cross-exported from Linux or Windows cannot be signed at all; [`seal`] reports
//! that rather than pretending. Credentials come from the environment for now
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
/// `bundled_app` is the `.app` when the export was wrapped, and `output_dir` is
/// the directory it (and any loose server binary) sits in. Both the bundle and
/// any loose Mach-O beside it are signed, so a `--server` binary next to the game
/// runs too. Best-effort by contract: the caller treats an `Err` as a warning,
/// because a signing failure still leaves a developer a runnable local build.
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
    // Strip extended attributes before signing anything. The runtime template is
    // installed from a downloaded zip, so its binary and dylibs arrive carrying
    // `com.apple.quarantine`, and macOS adds `com.apple.provenance` as files are
    // touched; those ride along when the export moves them into the bundle.
    // Leftover xattrs and resource forks are a documented source of codesign
    // verification failures (Apple's signing guidance is to clean them first),
    // so `xattr -cr` over the whole tree is a prerequisite, not a nicety. The
    // bundle lives inside `output_dir`, so one recursive pass covers it and any
    // loose binary beside it.
    strip_xattrs(output_dir);

    match cfg {
        MacSigning::AdHoc => {
            if let Some(app) = bundled_app {
                progress(format!(
                    "Ad-hoc signing {}...",
                    app.file_name().unwrap_or_default().to_string_lossy()
                ));
                // `--deep` seals the nested dylibs and plugins in one pass; an
                // ad-hoc signature has no team, so there is no library-validation
                // worry to sign them individually for.
                adhoc(app, true)?;
            }
            // Loose Mach-O outside the bundle: the dedicated server, or the whole
            // tree when the export was not wrapped.
            for f in macho_files(output_dir, bundled_app) {
                adhoc(&f, false)?;
            }
            progress(
                "Ad-hoc signed: runs locally, a downloaded copy clears Gatekeeper with right-click -> Open."
                    .to_string(),
            );
        }
        MacSigning::DeveloperId { identity, notary } => {
            let ents = write_entitlements()?;
            // Inside-out: the bundle's seal covers its nested code, so a container
            // signed before its contents is invalidated the moment they are.
            if let Some(app) = bundled_app {
                progress(format!(
                    "Signing {} with {identity} (approve the keychain prompt once, then \"Always Allow\")...",
                    app.file_name().unwrap_or_default().to_string_lossy()
                ));
                // One `--deep` call seals the whole bundle, loose `.rpak` and
                // `plugins/` included, in a single keychain prompt.
                devid_deep(app, identity, &ents)?;
            }
            // A loose server binary beside the bundle, when one was built.
            devid_all(&macho_files(output_dir, bundled_app), identity, &ents)?;
            let _ = std::fs::remove_file(&ents);

            match (notary, bundled_app) {
                (Some(n), Some(app)) => notarize_and_staple(app, n, progress)?,
                (Some(_), None) => progress(
                    "Signed, but notarization needs a .app to staple the ticket to; enable 'Bundle as .app' to notarize."
                        .to_string(),
                ),
                (None, _) => progress(
                    "Signed with your Developer ID (not notarized; a downloaded copy still warns until a notary key is set)."
                        .to_string(),
                ),
            }
        }
    }
    Ok(())
}

// ── macOS-only machinery ─────────────────────────────────────────────────────

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

#[cfg(target_os = "macos")]
fn write_entitlements() -> Result<PathBuf, String> {
    let path = std::env::temp_dir()
        .join(format!("renzora-export-entitlements-{}.plist", std::process::id()));
    std::fs::write(&path, ENTITLEMENTS).map_err(|e| format!("write entitlements: {e}"))?;
    Ok(path)
}

/// Is `path` a Mach-O object (thin or fat), by its leading magic?
///
/// A byte test rather than an extension one, because the files that must be
/// signed are the executable (no extension) and the `*.dylib`s, while the things
/// that must not are the `.rpak`, the icon and `Info.plist`. The four magics
/// appear in both byte orders; the fat `0xCAFEBABE` is shared with Java class
/// files, which an export never contains.
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
            | [0xCF, 0xFA, 0xED, 0xFE]
            | [0xCE, 0xFA, 0xED, 0xFE]
            | [0xFE, 0xED, 0xFA, 0xCF]
            | [0xFE, 0xED, 0xFA, 0xCE]
    )
}

/// Every Mach-O under `root`, deepest path first so nested code signs before
/// whatever contains it. `skip`, when given, prunes a subtree (the `.app`, which
/// the caller signs on its own so it is not signed twice).
#[cfg(target_os = "macos")]
fn macho_files(root: &Path, skip: Option<&Path>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let p = entry.path();
            if let Some(s) = skip {
                if p == s || p.starts_with(s) {
                    continue;
                }
            }
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            if ft.is_dir() {
                stack.push(p);
            } else if ft.is_file() && is_macho(&p) {
                out.push(p);
            }
        }
    }
    out.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    out
}

/// Recursively clear extended attributes (`xattr -cr`). Best-effort: a tree with
/// nothing to strip is a success that prints nothing, and a failure here is not
/// worth failing an export over, so the result is ignored and signing proceeds.
#[cfg(target_os = "macos")]
fn strip_xattrs(path: &Path) {
    let _ = std::process::Command::new("xattr")
        .args(["-cr"])
        .arg(path)
        .output();
}

#[cfg(target_os = "macos")]
fn run(cmd: &mut std::process::Command, what: &str) -> Result<(), String> {
    let out = cmd.output().map_err(|e| format!("{what}: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        let err = err.trim();
        if err.is_empty() {
            Err(format!("{what}: exited with {}", out.status))
        } else {
            Err(format!("{what}: {err}"))
        }
    }
}

#[cfg(target_os = "macos")]
fn adhoc(path: &Path, deep: bool) -> Result<(), String> {
    let mut c = std::process::Command::new("codesign");
    c.arg("--force");
    if deep {
        c.arg("--deep");
    }
    c.args(["--sign", "-"]).arg(path);
    run(&mut c, &format!("ad-hoc sign {}", path.display()))
}

/// Sign a whole bundle in one `--deep` invocation.
///
/// A game ships its `.rpak` and `plugins/` *inside* `Contents/MacOS/`, because
/// the runtime resolves them next to the executable. An inside-out pass that
/// signs each Mach-O and then the bundle leaves those data files unsigned, and
/// `codesign` rejects the result as "unsealed contents present in the bundle
/// root" (the invalid signature that read as "damaged"). `--deep` seals the
/// loose files too, and in a single `codesign` process, so it is also one
/// keychain prompt instead of several. Apple discourages `--deep` for bundles
/// that nest *frameworks or helper tools* needing their own entitlements; a game
/// nests neither, and the one entitlement it needs
/// (`disable-library-validation`) belongs on the main executable, which is where
/// `--deep` puts it.
#[cfg(target_os = "macos")]
fn devid_deep(app: &Path, identity: &str, entitlements: &Path) -> Result<(), String> {
    let mut c = std::process::Command::new("codesign");
    c.args(["--force", "--deep", "--timestamp", "--options", "runtime", "--entitlements"])
        .arg(entitlements)
        .args(["--sign", identity])
        .arg(app);
    run(&mut c, &format!("sign {}", app.display()))
}

/// Sign every `path` in a single `codesign` invocation.
///
/// One call, not one per file, because each `codesign` process unlocks the
/// signing key and so raises a keychain-password prompt: a per-file loop asked
/// the user for their password once per dylib, about six prompts for one export.
/// `codesign` accepts any number of paths and accesses the key once for the whole
/// process, so a batch is a single prompt. An empty slice is a no-op (the loose
/// pass is empty for a bundled export, and signing nothing must not error).
/// `--timestamp` because a signature without one expires with the certificate and
/// notarization rejects that outright.
#[cfg(target_os = "macos")]
fn devid_all(paths: &[PathBuf], identity: &str, entitlements: &Path) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut c = std::process::Command::new("codesign");
    c.args(["--force", "--timestamp", "--options", "runtime", "--entitlements"])
        .arg(entitlements)
        .args(["--sign", identity]);
    for p in paths {
        c.arg(p);
    }
    run(&mut c, &format!("sign {} item(s)", paths.len()))
}

#[cfg(target_os = "macos")]
fn notarize_and_staple(
    app: &Path,
    n: &Notary,
    progress: &mut dyn FnMut(String),
) -> Result<(), String> {
    let zip = std::env::temp_dir().join(format!("renzora-notarize-{}.zip", std::process::id()));

    // `ditto`, not `zip`: notarytool takes an archive, and only ditto preserves
    // the bundle's signature and structure the ticket is computed over.
    progress("Zipping the app for notarization...".to_string());
    let mut z = std::process::Command::new("ditto");
    z.args(["-c", "-k", "--keepParent"]).arg(app).arg(&zip);
    let zres = run(&mut z, "ditto for notarization");
    if zres.is_err() {
        let _ = std::fs::remove_file(&zip);
        return zres;
    }

    progress("Submitting to Apple's notary service (this can take a few minutes)...".to_string());
    let mut s = std::process::Command::new("xcrun");
    s.args(["notarytool", "submit"])
        .arg(&zip)
        .arg("--key")
        .arg(&n.key_path)
        .args(["--key-id", &n.key_id, "--issuer", &n.issuer, "--wait", "--timeout", "30m"]);
    let sres = run(&mut s, "notarytool submit");
    let _ = std::fs::remove_file(&zip);
    sres?;

    // Staple the ticket INTO the bundle so a machine with no network clears
    // Gatekeeper on first launch.
    progress("Stapling the notarization ticket...".to_string());
    let mut st = std::process::Command::new("xcrun");
    st.args(["stapler", "staple"]).arg(app);
    run(&mut st, "stapler staple")?;

    progress("Notarized and stapled: opens with a clean double-click anywhere.".to_string());
    Ok(())
}
