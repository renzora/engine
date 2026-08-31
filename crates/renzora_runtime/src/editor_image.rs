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
//! Same rule the plugin loaders follow, for the same reason. `Library::new` runs
//! the image's static initializers and the editor registers systems, resources
//! and function pointers into the `App`; unmapping it would leave every one of
//! those dangling, and `FreeLibrary` on a warmed Rust dylib inside the loader
//! lock is the deadlock `renzora_plugin`'s loader hit twice. The image is held
//! for the life of the process.

use std::path::PathBuf;

/// The symbol the editor image exports — see `renzora_editor::renzora_editor_install`.
const INSTALL_SYMBOL: &[u8] = b"renzora_editor_install\0";

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
    use bevy::prelude::*;

    let Some(path) = exe_dir().map(|d| d.join(image_name())) else { return };
    if !path.is_file() {
        return;
    }

    // SAFETY: loading native code, which runs the image's static initializers.
    // Inherent to the mechanism; the protection is that this is the engine's own
    // editor, staged beside the executable by the same build.
    let lib = match unsafe { libloading::Library::new(&path) } {
        Ok(lib) => lib,
        Err(e) => {
            error!("[editor] {} could not be loaded: {e}", path.display());
            return;
        }
    };

    let install: libloading::Symbol<InstallFn> = match unsafe { lib.get(INSTALL_SYMBOL) } {
        Ok(f) => f,
        Err(_) => {
            // A file by the right name that is not the editor. Leaked rather
            // than dropped for the reason in the module docs — the image's
            // initializers have already run.
            std::mem::forget(lib);
            error!(
                "[editor] {} exports no entry point — it is not an editor image",
                path.display()
            );
            return;
        }
    };

    install(app);

    // Held for the life of the process. Everything the editor just registered
    // points into this image.
    std::mem::forget(lib);
    info!("[editor] loaded {}", path.display());
}

/// wasm has no dynamic linking, so the editor is compiled into its own bundle
/// and installed by a direct call instead. See `renzora_editor_app`.
#[cfg(target_arch = "wasm32")]
pub fn install(_app: &mut bevy::app::App) {}
