//! Debugging panels and profiling support for the Renzora editor.
//!
//! Panels: System Profiler, Memory Profiler, Performance, Render Stats, ECS
//! Stats, Camera Debug, Culling Debug, Material Resolver, Lumen, Scripting.
//! All panels are bevy-native (ember); their content lives in [`native`] and
//! reads the per-frame snapshot resources kept current by the backend-agnostic
//! `update_*` systems in [`state`] (plus the scripting diag updater below).
//! The Lumen panel reads `renzora::LumenDiagState`, produced by the GI plugin.

pub mod native;
pub mod panels;
pub mod state;

use bevy::diagnostic::{
    EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin, SystemInformationDiagnosticsPlugin,
};
use bevy::prelude::*;

use state::*;

// ============================================================================
// Diagnostic snapshot updaters (scripting)
// ============================================================================
//
// The Lumen diagnostics snapshot (`renzora::LumenDiagState`) is produced by the
// GI plugin (`renzora_lumen`) under its `editor` feature, not here — the plugin
// is a cdylib and owns the internal voxel/bake types it reads. The native Lumen
// panel just reads the contract resource.

fn update_scripting_diag_state(
    mut state: ResMut<panels::scripting::ScriptingDiagState>,
    engine: Option<Res<renzora_scripting::ScriptEngine>>,
    perf: Option<Res<renzora_scripting::perf::ScriptPerfStats>>,
    components: Query<&renzora_scripting::ScriptComponent>,
) {
    // Entity-level inventory (cheap, no allocations beyond the count).
    let mut entities = 0usize;
    let mut attachments = 0usize;
    for comp in components.iter() {
        entities += 1;
        attachments += comp.scripts.len();
    }
    state.entities_with_script = entities;
    state.total_script_attachments = attachments;

    if let Some(engine) = engine {
        state.backend_count = engine.backend_count();
        state.scripts_folder = engine
            .scripts_folder()
            .map(|p| p.to_string_lossy().to_string());
    }

    if let Some(perf) = perf {
        state.totals = perf.totals();
        state.per_script = perf.snapshot();
        state.current_frame = perf.frame;
    } else {
        state.totals = Default::default();
        state.per_script.clear();
        state.current_frame = 0;
    }
}

// ============================================================================
// Plugin
// ============================================================================

#[derive(Default)]
pub struct DebuggerPlugin;

impl Plugin for DebuggerPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] DebuggerPlugin");
        // Add Bevy diagnostic plugins
        app.add_plugins((
            FrameTimeDiagnosticsPlugin::default(),
            EntityCountDiagnosticsPlugin::default(),
            SystemInformationDiagnosticsPlugin,
        ));

        // Real per-render-pass CPU/GPU timings (`render/<pass>/elapsed_{cpu,gpu}`).
        // This is the ONLY source of genuine GPU time; without it the render-stats
        // panel has nothing to read. On Vulkan/DX12 Bevy's default
        // `WgpuSettingsPriority::Functionality` already enables `TIMESTAMP_QUERY`,
        // so GPU timestamps populate automatically; on backends without it (GL,
        // some integrated adapters) only CPU spans exist and the panel shows "n/a"
        // for GPU rather than a fabricated number. Guarded because the (currently
        // unused) Tracy bridge can also add it, and a duplicate add panics.
        use bevy::render::diagnostic::RenderDiagnosticsPlugin;
        if !app.is_plugin_added::<RenderDiagnosticsPlugin>() {
            app.add_plugins(RenderDiagnosticsPlugin);
        }

        // Attribute the engine's built-in GPU passes to the components that drive
        // them, so the GPU Pass Breakdown shows *what* is paying for each pass.
        // Plugins that add their own render passes register the same way (via
        // `App::register_gpu_pass_source`) — nothing here is special-cased in the
        // panel. NOTE: the atmosphere environment map becomes a
        // `GeneratedEnvironmentMapLight` on the camera, so counting that catches
        // the realtime atmosphere IBL that drives the `lightprobe_*` passes.
        use bevy::light::{DirectionalLight, GeneratedEnvironmentMapLight, PointLight, SpotLight};
        use renzora::AppEditorExt;
        app.register_gpu_pass_source::<GeneratedEnvironmentMapLight>("lightprobe", "environment map")
            .register_gpu_pass_source::<DirectionalLight>(
                "shadow_directional_light",
                "directional light",
            )
            .register_gpu_pass_source::<PointLight>("shadow_point", "point light")
            .register_gpu_pass_source::<SpotLight>("shadow_spot", "spot light");

        // Init resources
        app.init_resource::<DiagnosticsState>()
            .init_resource::<RenderStats>()
            .init_resource::<SystemTimingState>()
            .init_resource::<MemoryProfilerState>()
            .init_resource::<CameraDebugState>()
            .init_resource::<CullingDebugState>()
            .init_resource::<EcsStatsState>()
            .init_resource::<panels::scripting::ScriptingDiagState>();

        // Update systems
        use renzora::SplashState;
        app.add_systems(
            Update,
            (
                update_diagnostics_state,
                update_memory_profiler,
                update_camera_debug_state,
                update_culling_debug_state,
            )
                .run_if(in_state(SplashState::Editor)),
        );
        // `update_render_stats` walks render-world resources every frame; it only
        // feeds the Render Stats panel, so gate it on that panel being the active
        // tab and throttle at the user-configured interval (Settings → Plugins →
        // Stats Refresh).
        app.add_systems(
            Update,
            update_render_stats
                .run_if(in_state(SplashState::Editor))
                .run_if(renzora_ember::dock::panel_active("render_stats"))
                .run_if(renzora::stat_refresh_throttle(|s| s.render_stats_ms)),
        );
        // Exclusive systems (need `&mut World`): ECS archetype stats, and the GPU
        // pass breakdown (scans archetypes to count the entities driving passes).
        // The archetype scan is heavy and feeds only the ECS Stats panel — gate +
        // throttle it the same way.
        app.add_systems(
            Update,
            update_ecs_stats
                .run_if(in_state(SplashState::Editor))
                .run_if(renzora_ember::dock::panel_active("ecs_stats"))
                .run_if(renzora::stat_refresh_throttle(|s| s.ecs_stats_ms)),
        );
        app.add_systems(
            Update,
            update_system_timing.run_if(in_state(SplashState::Editor)),
        );
        // The entity-inventory pass iterates *every* ScriptComponent in the
        // scene each frame (272k on a stress city, since one is auto-inserted on
        // every named entity). That's a full-scene scan for a readout nobody may
        // be looking at — gate it on the Scripting panel being the active tab and
        // throttle to 4 Hz while it is. Hidden → zero cost.
        app.add_systems(
            Update,
            update_scripting_diag_state
                .run_if(in_state(SplashState::Editor))
                .run_if(renzora_ember::dock::panel_active("scripting_diag"))
                .run_if(bevy::time::common_conditions::on_timer(
                    std::time::Duration::from_millis(250),
                )),
        );

        // User-configurable refresh rates for the live stat readouts. Seed the
        // resource from disk (the throttled stat systems above + the system
        // monitor read it live); the settings section below persists edits.
        app.insert_resource(renzora::load_stats_refresh());
        use renzora_ember::settings_sections::RegisterSettingsSection;
        app.register_settings_section(
            "stats_refresh",
            "Status Bar",
            "gauge",
            build_stats_refresh_section,
        );

        // bevy-native (ember) content for every debug panel.
        native::register_native_debug(app);
    }
}

/// Settings → Plugins → "Stats Refresh": three sliders setting how often the
/// live readouts poll. Higher ms = fewer updates = cheaper. Edits are bound
/// two-way to [`renzora::StatsRefreshSettings`] and persisted on change.
fn build_stats_refresh_section(
    commands: &mut Commands,
    fonts: &renzora_ember::font::EmberFonts,
) -> Entity {
    let col = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();

    let sm = refresh_row(
        commands,
        fonts,
        "System monitor (status bar)",
        16.0,
        2000.0,
        10.0,
        |w| {
            w.get_resource::<renzora::StatsRefreshSettings>()
                .map(|s| s.system_monitor_ms as f32)
                .unwrap_or(200.0)
        },
        |w, v| commit_refresh(w, *v, 16, 10_000, |s, n| s.system_monitor_ms = n),
    );
    let rs = refresh_row(
        commands,
        fonts,
        "Render Stats panel",
        16.0,
        2000.0,
        10.0,
        |w| {
            w.get_resource::<renzora::StatsRefreshSettings>()
                .map(|s| s.render_stats_ms as f32)
                .unwrap_or(100.0)
        },
        |w, v| commit_refresh(w, *v, 16, 10_000, |s, n| s.render_stats_ms = n),
    );
    let ec = refresh_row(
        commands,
        fonts,
        "ECS Stats panel",
        16.0,
        5000.0,
        10.0,
        |w| {
            w.get_resource::<renzora::StatsRefreshSettings>()
                .map(|s| s.ecs_stats_ms as f32)
                .unwrap_or(250.0)
        },
        |w, v| commit_refresh(w, *v, 16, 10_000, |s, n| s.ecs_stats_ms = n),
    );
    let rates_lbl = group_label(commands, fonts, "REFRESH RATES (MS)");
    let bar_lbl = group_label(commands, fonts, "STATUS BAR ITEMS");
    let t_fps = toggle_row(
        commands,
        fonts,
        "FPS / frame time",
        |w| read_flag(w, |s| s.show_fps),
        |w, v| commit_toggle(w, *v, |s, b| s.show_fps = b),
    );
    let t_ram = toggle_row(
        commands,
        fonts,
        "RAM usage",
        |w| read_flag(w, |s| s.show_ram),
        |w, v| commit_toggle(w, *v, |s, b| s.show_ram = b),
    );
    let t_gpu = toggle_row(
        commands,
        fonts,
        "GPU usage / VRAM",
        |w| read_flag(w, |s| s.show_gpu),
        |w, v| commit_toggle(w, *v, |s, b| s.show_gpu = b),
    );
    let t_mode = toggle_row(
        commands,
        fonts,
        "Rendering mode",
        |w| read_flag(w, |s| s.show_rendering_mode),
        |w, v| commit_toggle(w, *v, |s, b| s.show_rendering_mode = b),
    );
    let t_name = toggle_row(
        commands,
        fonts,
        "GPU name",
        |w| read_flag(w, |s| s.show_gpu_name),
        |w, v| commit_toggle(w, *v, |s, b| s.show_gpu_name = b),
    );

    commands
        .entity(col)
        .add_children(&[rates_lbl, sm, rs, ec, bar_lbl, t_fps, t_ram, t_gpu, t_mode, t_name]);
    col
}

fn read_flag(world: &World, pick: fn(&renzora::StatsRefreshSettings) -> bool) -> bool {
    world
        .get_resource::<renzora::StatsRefreshSettings>()
        .map(|s| pick(s))
        .unwrap_or(true)
}

/// Flip one status-bar visibility flag and persist (no-op if unchanged).
fn commit_toggle(
    world: &mut World,
    value: bool,
    apply: impl Fn(&mut renzora::StatsRefreshSettings, bool),
) {
    let snapshot = {
        let Some(mut s) = world.get_resource_mut::<renzora::StatsRefreshSettings>() else {
            return;
        };
        let before = *s;
        apply(&mut s, value);
        if *s == before {
            return;
        }
        *s
    };
    let _ = renzora::save_stats_refresh(&snapshot);
}

/// A small muted group heading between control clusters.
fn group_label(
    commands: &mut Commands,
    fonts: &renzora_ember::font::EmberFonts,
    text: &str,
) -> Entity {
    commands
        .spawn((
            Text::new(text),
            renzora_ember::font::ui_font(&fonts.ui, 10.0),
            TextColor(renzora_ember::theme::rgb(renzora_ember::theme::text_muted())),
            Node {
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            },
        ))
        .id()
}

/// One labelled row with a toggle switch bound two-way to a status-bar flag.
fn toggle_row<G, S>(
    commands: &mut Commands,
    fonts: &renzora_ember::font::EmberFonts,
    label: &str,
    get: G,
    set: S,
) -> Entity
where
    G: Fn(&World) -> bool + Send + Sync + 'static,
    S: Fn(&mut World, &bool) + Send + Sync + 'static,
{
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let lbl = commands
        .spawn((
            Text::new(label),
            renzora_ember::font::ui_font(&fonts.ui, 13.0),
            TextColor(renzora_ember::theme::rgb(renzora_ember::theme::text_primary())),
        ))
        .id();
    let sw = renzora_ember::widgets::toggle_switch(commands, true);
    renzora_ember::reactive::bind_2way(commands, sw, get, set);
    commands.entity(row).add_children(&[lbl, sw]);
    row
}

/// Write one refresh field (clamped) and persist the whole set. Pulled out so
/// each row's setter stays a one-liner.
fn commit_refresh(
    world: &mut World,
    value: f32,
    min: u32,
    max: u32,
    apply: impl Fn(&mut renzora::StatsRefreshSettings, u32),
) {
    let n = (value.round() as i64).clamp(min as i64, max as i64) as u32;
    let snapshot = {
        let Some(mut s) = world.get_resource_mut::<renzora::StatsRefreshSettings>() else {
            return;
        };
        let before = *s;
        apply(&mut s, n);
        // A drag fires many sub-step changes that round to the same ms — only
        // touch the resource tick + persist when the value actually moved, so we
        // don't write the TOML dozens of times per drag.
        if *s == before {
            return;
        }
        *s
    };
    let _ = renzora::save_stats_refresh(&snapshot);
}

/// One labelled row: a title on the left, a bounded `drag_value` on the right
/// bound two-way to the setting.
#[allow(clippy::too_many_arguments)]
fn refresh_row<G, S>(
    commands: &mut Commands,
    fonts: &renzora_ember::font::EmberFonts,
    label: &str,
    min: f32,
    max: f32,
    step: f32,
    get: G,
    set: S,
) -> Entity
where
    G: Fn(&World) -> f32 + Send + Sync + 'static,
    S: Fn(&mut World, &f32) + Send + Sync + 'static,
{
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let lbl = commands
        .spawn((
            Text::new(label),
            renzora_ember::font::ui_font(&fonts.ui, 13.0),
            TextColor(renzora_ember::theme::rgb(renzora_ember::theme::text_primary())),
        ))
        .id();
    // `get` syncs the real value on the first frame, so a placeholder init is fine.
    let dv = renzora_ember::widgets::drag_value(
        commands,
        &fonts.ui,
        "ms",
        renzora_ember::theme::value_text(),
        min,
        step,
    );
    commands
        .entity(dv)
        .insert(renzora_ember::widgets::DragRange { min, max });
    renzora_ember::reactive::bind_2way(commands, dv, get, set);
    commands.entity(row).add_children(&[lbl, dv]);
    row
}

renzora::add!(DebuggerPlugin, Editor);
