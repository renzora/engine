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

    // C-ABI plugins from `<exe_dir>/plugins/`. Unaffected by static linking:
    // they link no Bevy at all, so there is no ABI to match — the interface is
    // passed in as a function table.
    // No `statics`: linking plugins in is an export-time choice for a shipped
    // game, and it would cost the editor the thing it needs most from them —
    // hot reload, which needs a file on disk to watch and swap.
    app.add_plugins(renzora_plugin::host::loader::RenzoraPluginHostPlugin {
        is_editor: true,
        statics: Vec::new(),
    });
    // Render passes those plugins registered. Separate plugin because the work
    // happens in `finish`, after every `build` has run and the render sub-app
    // exists.
    app.add_plugins(renzora_postprocess::plugin_bridge::PluginRenderBridgePlugin);
    // Custom shaded materials registered by those plugins — same `finish`
    // reasoning as the render bridge.
    renzora_postprocess::add_plugin_material(&mut app);

    app.run();
}
