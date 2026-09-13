//! The editor executable.
//!
//! Deliberately much smaller than the runtime's `main.rs`. That binary has to
//! decide at startup whether it is a game, a dedicated server, a listen server
//! or a VR session; this one is only ever the editor. The editor launches the
//! *runtime* binary as a child process for Play, so it never needs those modes
//! itself.
//!
//! The editor arrives by a plain function call rather than `dlopen`. With Bevy
//! statically linked there is no shared `bevy_dylib` for a loadable bundle to
//! attach to — a cdylib linking static Bevy would carry a second copy of Bevy,
//! and therefore a second `World` type, so every component crossing the boundary
//! would mismatch. "Editor as a removable file" becomes "editor as a separate
//! executable"; removing editor code from a shipped game is now a property of
//! which binary you ship, not of which files you delete beside it.
//!
//! # No setup window here
//!
//! This package builds for **wasm only** (`required-features = ["wasm"]`), and
//! the first-run setup that unpacks the SDK and compiles the source-only native
//! plugins is desktop-only: a browser can run neither half, there is no SDK
//! beside a wasm bundle to unpack, and no process to relaunch afterwards. It
//! used to carry its own copy of that window behind a
//! `cfg(not(target_arch = "wasm32"))` that could never be true here, so the copy
//! drifted from the live one in the root binary's `src/setup_ui.rs` and was
//! never compiled by anything. That is where setup lives; this binary has none.

fn main() {
    // The editor always keeps a console: its log output is the primary
    // diagnostic channel, and on Windows the runtime binary is built
    // `windows_subsystem = "windows"` precisely so shipped games don't get one.
    renzora_runtime::renzora_engine::crash::install_panic_hook(true);
    renzora_runtime::attach_console();

    let mut app = renzora_runtime::init_app();
    renzora_runtime::add_default_rendering(&mut app, true);
    renzora_runtime::add_engine_plugins(&mut app, true);
    app.add_plugins(renzora_runtime::renzora_engine::crash::CrashReportPlugin);

    // AFTER the engine foundation, so Editor-scope plugins layer on top of the
    // runtime ones — the ordering the old `load_bundle` call site guaranteed.
    renzora_editor::install(&mut app);

    // Plugins from `<exe_dir>/plugins/`, last: an installed plugin layers on top
    // of the editor exactly as an Editor-scope in-workspace one does.
    app.add_plugins(renzora_native_plugin::NativePluginLoader::default());

    app.run();
}
