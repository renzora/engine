//! Tracy profiler bridge — standalone distribution plugin.
//!
//! Streams live engine telemetry to a running [Tracy] profiler over Tracy's
//! native protocol: a frame mark per app frame, plus every Bevy diagnostic —
//! frame time, FPS, entity count, per-render-pass GPU/CPU span times, and
//! system CPU/memory where the platform supports it — as a named Tracy plot.
//!
//! **Gated to developer mode + an explicit opt-in.** Starting the Tracy client
//! opens a network listener and allocates Tracy's profiling ring buffers, and
//! the bridge also adds Bevy's `SystemInformationDiagnosticsPlugin` (per-frame
//! `sysinfo` sampling) — all of which cost real RAM/CPU. So unless BOTH the
//! editor's Dev Mode is on AND this plugin's own "Tracy Profiler" toggle
//! (Settings → Plugins) is enabled, `build` returns early having added *nothing*
//! but the cheap settings toggle: no client, no diagnostic sources, no per-frame
//! systems — a genuinely zero-footprint dormant state. The gate is read once at
//! startup, so toggling either switch takes effect on the next editor launch.
//!
//! Self-contained like any distribution plugin: it depends only on `bevy`,
//! the `renzora` contract (to read `EditorSettings.dev_mode` across the dylib
//! boundary), and `renzora_ember` (to render its settings toggle). Its own
//! opt-in persists to `~/.config/renzora/tracy.json` (APPDATA on Windows).
//!
//! [Tracy]: https://github.com/wolfpld/tracy

use bevy::diagnostic::{
    DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin,
    SystemInformationDiagnosticsPlugin,
};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::inspector::inspector_row;
use renzora_ember::reactive::bind_2way;
use renzora_ember::settings_sections::RegisterSettingsSection;
use renzora_ember::theme::{rgb, text_muted};
use renzora_ember::widgets::toggle_switch;
use tracy_client::{Client, PlotName};

/// The user's opt-in, mirrored live from disk. Edited from the settings toggle;
/// `manage_bridge` reads it (together with dev mode) to decide whether the
/// client should be running.
#[derive(Resource)]
struct TracyEnabled(bool);

/// Present **only while the bridge is active**. Holds the Tracy client alive
/// (it's refcounted — dropping the last handle tears the connection down, so
/// removing this resource is what stops profiling and frees the buffers).
#[derive(Resource)]
struct TracyClient(Client);

/// Tracy `PlotName`s require `'static` storage, but diagnostic paths are
/// dynamic strings — each unique path is leaked once and cached here. The
/// set is small and stable (a few dozen paths), so the leak is bounded.
#[derive(Resource, Default)]
struct PlotNames(HashMap<String, PlotName>);

#[derive(Default)]
pub struct TracyPlugin;

impl Plugin for TracyPlugin {
    fn build(&self, app: &mut App) {
        // ALWAYS cheap: the opt-in resource + the settings toggle, so the user
        // can turn the profiler on. This costs nothing — no client, no diagnostic
        // sources, no per-frame systems.
        let enabled = load_config().enabled;
        app.insert_resource(TracyEnabled(enabled));
        app.register_settings_section("tracy", "Tracy Profiler", "pulse", build_settings);

        // Gate EVERYTHING else on (Dev Mode + opt-in), read once at startup. When
        // the bridge is off we add nothing further — crucially not Bevy's
        // `SystemInformationDiagnosticsPlugin`, whose per-frame `sysinfo` sampling
        // (Tracy is the only thing that adds it) is what was growing RAM even
        // while the profiler was idle. Dormant Tracy now has a truly zero
        // footprint. The gate is read at build, so toggling either switch takes
        // effect on the next editor launch.
        if !(renzora::load_dev_mode() && enabled) {
            info!(
                "[editor] TracyPlugin dormant — enable Dev Mode (Settings → Editor → \
                 Developer) and Settings → Plugins → Tracy Profiler, then restart."
            );
            return;
        }

        warn!(
            "[editor] TracyPlugin active — streaming profiling telemetry. Tracy holds \
             capture buffers in RAM; turn it off (Settings → Plugins → Tracy Profiler) \
             and restart when you're done profiling."
        );

        // Diagnostic sources the bridge streams. Guarded because the editor's
        // debugger already adds some (a duplicate panics); `SystemInformation`
        // is ours alone — and now only added while actively profiling.
        if !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
            app.add_plugins(FrameTimeDiagnosticsPlugin::default());
        }
        if !app.is_plugin_added::<EntityCountDiagnosticsPlugin>() {
            app.add_plugins(EntityCountDiagnosticsPlugin::default());
        }
        if !app.is_plugin_added::<SystemInformationDiagnosticsPlugin>() {
            app.add_plugins(SystemInformationDiagnosticsPlugin);
        }
        // Per-render-pass GPU/CPU span timings (`render/<pass>/elapsed_*`).
        if !app.is_plugin_added::<bevy::render::diagnostic::RenderDiagnosticsPlugin>() {
            app.add_plugins(bevy::render::diagnostic::RenderDiagnosticsPlugin);
        }

        app.insert_resource(TracyClient(Client::start()))
            .init_resource::<PlotNames>();

        // Stream this frame's plots, then close the frame on Tracy's timeline.
        app.add_systems(Last, (plot_diagnostics, frame_mark).chain());
    }
}

fn frame_mark(client: Res<TracyClient>) {
    client.0.frame_mark();
}

fn plot_diagnostics(
    client: Res<TracyClient>,
    store: Res<DiagnosticsStore>,
    mut names: ResMut<PlotNames>,
) {
    for diag in store.iter() {
        let Some(value) = diag.value() else { continue };
        if !value.is_finite() {
            continue;
        }
        let path = diag.path().as_str();
        let name = names
            .0
            .entry(path.to_string())
            .or_insert_with(|| PlotName::new_leak(path.to_string()));
        client.0.plot(*name, value);
    }
}

// ── Settings UI ────────────────────────────────────────────────────────────

/// Settings → Plugins → "Tracy Profiler": the explicit enable toggle plus the
/// RAM / dev-mode caveat. Bound two-way to [`TracyEnabled`] and persisted on
/// every change; `manage_bridge` reacts to the new value next frame.
fn build_settings(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();

    let toggle = toggle_switch(commands, false);
    bind_2way(
        commands,
        toggle,
        |w| w.get_resource::<TracyEnabled>().map(|t| t.0).unwrap_or(false),
        |w, v| {
            if let Some(mut t) = w.get_resource_mut::<TracyEnabled>() {
                t.0 = *v;
            }
            save_config(&TracyConfig { enabled: *v });
            // The bridge reads the gate at startup, so flipping the toggle only
            // takes effect next launch — surface that as an editor toast rather
            // than leaving the user wondering why nothing happened.
            if let Some(mut toasts) = w.get_resource_mut::<renzora_ui::Toasts>() {
                toasts.info("Tracy Profiler — applies after restarting the editor");
            }
        },
    );
    let row = inspector_row(commands, &fonts.ui, "Enable Tracy", toggle);

    let note = commands
        .spawn((
            Text::new(
                "Streams live profiling telemetry (frame time, FPS, per-pass GPU/CPU \
                 timings, memory) to a running Tracy server. Requires Dev Mode \
                 (Settings → Editor → Developer) — it stays off otherwise. Tracy holds \
                 capture buffers in RAM and samples system stats every frame while \
                 active, so leave this off unless you're actively profiling. Read at \
                 startup: changing this takes effect after restarting the editor.",
            ),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
            Node {
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            },
        ))
        .id();

    commands.entity(col).add_children(&[row, note]);
    col
}

// ── Persistence ──────────────────────────────────────────────────────────────

/// The opt-in, persisted across editor runs at `~/.config/renzora/tracy.json`
/// (APPDATA on Windows) — the same per-user config convention as other plugins.
#[derive(Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
struct TracyConfig {
    enabled: bool,
}

fn config_path() -> Option<std::path::PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(std::path::PathBuf::from)?
    } else {
        std::path::PathBuf::from(std::env::var_os("HOME")?).join(".config")
    };
    Some(base.join("renzora").join("tracy.json"))
}

fn load_config() -> TracyConfig {
    config_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_config(cfg: &TracyConfig) {
    let Some(path) = config_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(cfg) {
        let _ = std::fs::write(path, json);
    }
}

renzora::add!(TracyPlugin, Editor);
