//! Load the editor, if it is there.
//!
//! One binary, two roles. `renzora_editor.<dll|so|dylib>` beside the executable
//! makes it the editor; without the file the same executable is the game. So
//! "ship without the editor" is deleting one file rather than building and
//! shipping a different binary, and a game and an editor are never two things
//! that can drift apart.
//!
//! # Why this is sound
//!
//! The image links the real Bevy and takes `&mut App` across a `dlopen`
//! boundary, which is only safe while both sides agree on what `App` is.
//! `bevy_dylib` is what makes that true, exactly as it does for a native plugin;
//! `renzora_dylib` and `renzora_ember_dylib` do the same for the contract
//! crate's and the UI toolkit's process-global statics.
//!
//! Crates the executable and the image both depend on are linked into each, and
//! that is fine: a `TypeId` comes from a crate's stable id rather than from
//! which artifact swallowed it, so both sides agree about what a component is
//! even with two copies of the code. The executable IS the runtime — it carries
//! the engine as it always did, and this image adds the editor on top.
//!
//! There is no version handshake, and it would not help: by the time anything
//! could be inspected the damage is done. What makes it safe in practice is that
//! the executable and the image come out of the same `cargo build --workspace`
//! and are staged together — cargo rebuilds both when a shared crate changes.
//!
//! # Why the image is never unloaded
//!
//! Same rule the plugin loader follows, for the same reason. `Library::new` runs
//! the image's static initializers and the editor registers systems, resources
//! and function pointers into the `App`; unmapping it would leave every one of
//! those dangling, and `FreeLibrary` on a warmed Rust dylib inside the loader
//! lock is the deadlock the plugin loader hit twice. The image is held for the
//! life of the process.

use std::path::PathBuf;

/// The symbol the editor image exports — see `renzora_editor::renzora_editor_install`.
const INSTALL_SYMBOL: &[u8] = b"renzora_editor_install\0";

/// The companion's entry point in the same image: see
/// `renzora_editor::renzora_companion_install`. A second symbol rather than a
/// flag to the first, because installing the editor and installing the
/// companion share nothing: the companion is a small app of its own and must
/// not pick up a single editor plugin on the way.
const COMPANION_SYMBOL: &[u8] = b"renzora_companion_install\0";

/// Its signature. A plain Rust `fn` taking `&mut App`, sound only because both
/// sides link one shared Bevy.
type InstallFn = fn(&mut bevy::app::App);

/// `renzora_editor.dll` / `librenzora_editor.so` / `librenzora_editor.dylib`.
fn image_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "renzora_editor.dll"
    } else if cfg!(target_os = "macos") {
        "librenzora_editor.dylib"
    } else {
        "librenzora_editor.so"
    }
}

/// Where the running executable lives.
fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(|p| p.to_path_buf())
}

/// Is an editor image present beside the executable?
///
/// Answered from a single `is_file` so it can be asked before `App` assembly —
/// `add_engine_plugins` needs to know whether this process is the editor, and
/// that decision changes what is added, so it cannot wait for the image to load.
pub fn present() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        false
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        exe_dir().map(|d| d.join(image_name()).is_file()).unwrap_or(false)
    }
}

/// Load the editor image and install it into `app`.
///
/// A no-op when there is no image, which is the shipped-game case and not an
/// error. Call AFTER `add_engine_plugins`, so the editor layers on top of the
/// runtime foundation — the ordering the old static call site guaranteed.
#[cfg(not(target_arch = "wasm32"))]
pub fn install(app: &mut bevy::app::App) {
    install_entry(app, INSTALL_SYMBOL, "editor");
}

/// Load the editor image and install only the companion from it: the
/// `--companion` process the editor starts beside itself.
///
/// Returns whether it was installed. The companion's code lives in the editor
/// image so a shipped game never carries it, which means a missing image has no
/// companion to run either.
#[cfg(not(target_arch = "wasm32"))]
pub fn install_companion(app: &mut bevy::app::App) -> bool {
    install_entry(app, COMPANION_SYMBOL, "companion")
}

/// Load the image and call the entry point named `symbol`.
#[cfg(not(target_arch = "wasm32"))]
fn install_entry(app: &mut bevy::app::App, symbol: &[u8], what: &str) -> bool {
    use bevy::prelude::*;

    let Some(path) = exe_dir().map(|d| d.join(image_name())) else { return false };
    if !path.is_file() {
        return false;
    }

    // SAFETY: loading native code, which runs the image's static initializers.
    // Inherent to the mechanism; the protection is that this is the engine's own
    // editor, staged beside the executable by the same build.
    let lib = match unsafe { libloading::Library::new(&path) } {
        Ok(lib) => lib,
        Err(e) => {
            error!("[{what}] {} could not be loaded: {e}", path.display());
            return false;
        }
    };

    let install: libloading::Symbol<InstallFn> = match unsafe { lib.get(symbol) } {
        Ok(f) => f,
        Err(_) => {
            // A file by the right name that is not the editor, or one built
            // before this entry point existed. Leaked rather than dropped for
            // the reason in the module docs: the image's initializers have
            // already run.
            std::mem::forget(lib);
            error!(
                "[{what}] {} has no {what} entry point; it is not an editor image, or is older than this executable",
                path.display()
            );
            return false;
        }
    };

    install(app);

    // Held for the life of the process. Everything just registered points into
    // this image.
    std::mem::forget(lib);
    info!("[{what}] loaded {}", path.display());
    true
}

/// wasm has no dynamic linking, so the editor is compiled into its own bundle
/// and installed by a direct call instead. See `renzora_editor_app`.
#[cfg(target_arch = "wasm32")]
pub fn install(_app: &mut bevy::app::App) {}

/// No companion on the web: there is no second process to start.
#[cfg(target_arch = "wasm32")]
pub fn install_companion(_app: &mut bevy::app::App) -> bool {
    false
}
