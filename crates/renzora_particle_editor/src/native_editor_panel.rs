//! Bevy-native (ember) port of the egui `ParticleEditorPanel` — the full
//! particle-effect property editor over `HanabiEffectDefinition`.
//!
//! WORK IN PROGRESS: ported section by section. Not yet registered (the egui
//! panel stays active) so the editor never loses sections mid-port; the final
//! commit wires `NativeParticleEditor` in once every section is covered.
//!
//! Each field binds its get/set straight to `ParticleEditorState.current_effect`
//! (marking `is_modified`), reusing ember's drag_value/bind_2way, checkbox,
//! text_input and a small generic combo + action-button system. Sections are
//! self-contained ember collapsibles; rows that only apply to a given mode use
//! `bind_display` to show/hide.

use std::sync::Arc;

use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use renzora_editor::SplashState;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_2way, bind_display, bind_text};
use renzora_ember::theme::*;
use renzora_ember::widgets::{bind_text_input, checkbox, collapsible, drag_value, menu_item, screen_menu, text_input, DragRange};

use renzora_hanabi::{
    HanabiEffectDefinition, ParticleAlphaMode, ParticleEditorState, ParticleOrientMode, SimulationCondition,
    SimulationSpace, SpawnMode, VelocityMode,
};

const LABEL_W: f32 = 96.0;
const AXES3: [(&str, (u8, u8, u8)); 3] = [("X", (230, 90, 90)), ("Y", (90, 200, 90)), ("Z", (90, 130, 230))];

type Action = Arc<dyn Fn(&mut World) + Send + Sync>;

pub struct NativeParticleEditor;

impl Plugin for NativeParticleEditor {
    fn build(&self, app: &mut App) {
        app.register_panel_content("particle_editor", true, build);
        app.add_systems(Update, (action_btn_click, combo_open).run_if(in_state(SplashState::Editor)));
    }
}

// ── Effect get/set ───────────────────────────────────────────────────────────

fn getf<R>(w: &World, f: impl FnOnce(&HanabiEffectDefinition) -> R, default: R) -> R {
    w.get_resource::<ParticleEditorState>().and_then(|s| s.current_effect.as_ref()).map(f).unwrap_or(default)
}

fn setf(w: &mut World, f: impl FnOnce(&mut HanabiEffectDefinition)) {
    if let Some(mut s) = w.get_resource_mut::<ParticleEditorState>() {
        if let Some(e) = s.current_effect.as_mut() {
            f(e);
        }
        s.is_modified = true;
    }
}

/// Wrap an effect mutation as a generic world action (for combo/preset buttons).
fn act(f: impl Fn(&mut HanabiEffectDefinition) + Send + Sync + 'static) -> Action {
    Arc::new(move |w: &mut World| setf(w, &f))
}

fn has_effect(w: &World) -> bool {
    w.get_resource::<ParticleEditorState>().is_some_and(|s| s.current_effect.is_some())
}

// ── Build ────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() },
            Name::new("native-particle-editor"),
        ))
        .id();

    // ── Welcome (no effect) ──
    let welcome = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(10.0), padding: UiRect::vertical(Val::Px(40.0)), ..default() })
        .id();
    let w1 = commands.spawn((Text::new("Particle Editor"), ui_font(&fonts.ui, 20.0), TextColor(rgb(text_muted())))).id();
    let w2 = commands.spawn((Text::new("Create or load a particle effect to begin"), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted())))).id();
    let wbtns = commands.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(8.0), ..default() }).id();
    let new_btn = action_button(commands, fonts, "New Effect", accent(), Arc::new(|w: &mut World| {
        if let Some(mut s) = w.get_resource_mut::<ParticleEditorState>() {
            s.current_effect = Some(HanabiEffectDefinition::default());
            s.is_modified = true;
            s.current_file_path = None;
        }
    }));
    let open_btn = action_button(commands, fonts, "Open", text_muted(), Arc::new(|w: &mut World| {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = rfd::FileDialog::new().add_filter("Particle files", &["particle"]).pick_file() {
            if let Some(effect) = renzora_hanabi::load_effect_from_file(&path) {
                if let Some(mut s) = w.get_resource_mut::<ParticleEditorState>() {
                    s.current_effect = Some(effect);
                    s.current_file_path = Some(path.to_string_lossy().to_string());
                    s.is_modified = false;
                }
            }
        }
    }));
    commands.entity(wbtns).add_children(&[new_btn, open_btn]);
    commands.entity(welcome).add_children(&[w1, w2, wbtns]);
    bind_display(commands, welcome, |w| !has_effect(w));

    // ── Editor body (effect present) ──
    let body = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() }).id();
    bind_display(commands, body, has_effect);

    let header = build_header(commands, fonts);
    commands.entity(body).add_child(header);

    // Sections.
    for section in [
        section_general as fn(&mut Commands, &EmberFonts) -> Entity,
        section_spawning,
        section_lifetime,
        section_velocity,
        section_forces,
        section_size,
        section_noise,
        section_velocity_limit,
        section_simulation,
        section_rendering,
    ] {
        let s = section(commands, fonts);
        commands.entity(body).add_child(s);
    }

    commands.entity(root).add_children(&[welcome, body]);
    root
}

fn build_header(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), border: UiRect::bottom(Val::Px(1.0)), ..default() })
        .id();
    commands.entity(bar).insert((BorderColor::all(rgb(border())),));
    let save = action_button(commands, fonts, "Save", text_primary(), Arc::new(|w: &mut World| save_current(w, false)));
    let save_as = action_button(commands, fonts, "Save As", text_muted(), Arc::new(|w: &mut World| save_current(w, true)));
    commands.entity(bar).add_children(&[save, save_as]);
    bar
}

fn save_current(w: &mut World, save_as: bool) {
    let (effect, path) = {
        let Some(s) = w.get_resource::<ParticleEditorState>() else { return };
        let Some(effect) = s.current_effect.clone() else { return };
        (effect, s.current_file_path.clone())
    };
    let target = match path {
        Some(p) if !save_as => std::path::PathBuf::from(p),
        _ => {
            let base: String = effect.name.chars().filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == ' ').collect();
            let base = if base.trim().is_empty() { "effect".to_string() } else { base };
            std::path::PathBuf::from(format!("{}.particle", base))
        }
    };
    if crate::editor_panel::save_effect_to_file(&target, &effect) {
        if let Some(mut s) = w.get_resource_mut::<ParticleEditorState>() {
            s.current_file_path = Some(target.to_string_lossy().to_string());
            s.is_modified = false;
        }
    }
}

// ── Sections ─────────────────────────────────────────────────────────────────

fn section_general(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = collapsible(commands, fonts, Some("gear"), "General", true);
    let name_row = base_row(commands, fonts, "Name");
    let ti = text_input(commands, &fonts.ui, "Effect name", "");
    bind_text_input(commands, ti, |w| getf(w, |e| e.name.clone(), String::new()), |w, v| setf(w, |e| e.name = v));
    commands.entity(name_row.1).add_child(ti);
    let cap = row_num(commands, fonts, "Capacity", 10.0, 10.0, 100000.0, |w| getf(w, |e| e.capacity as f32, 0.0), |w, v| setf(w, |e| e.capacity = v.round().max(0.0) as u32));
    commands.entity(body).add_children(&[name_row.0, cap]);
    root
}

fn section_spawning(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = collapsible(commands, fonts, Some("sparkle"), "Spawning", true);
    let mode = row_combo(commands, fonts, "Mode", |w| match getf(w, |e| e.spawn_mode, SpawnMode::Rate) {
        SpawnMode::Rate => "Continuous".into(),
        SpawnMode::Burst => "Single Burst".into(),
        SpawnMode::BurstRate => "Repeated Bursts".into(),
    }, vec![
        ("Continuous".into(), act(|e| e.spawn_mode = SpawnMode::Rate)),
        ("Single Burst".into(), act(|e| e.spawn_mode = SpawnMode::Burst)),
        ("Repeated Bursts".into(), act(|e| e.spawn_mode = SpawnMode::BurstRate)),
    ]);
    let rate = row_num(commands, fonts, "Rate/sec", 1.0, 0.1, 10000.0, |w| getf(w, |e| e.spawn_rate, 0.0), |w, v| setf(w, |e| e.spawn_rate = *v));
    bind_display(commands, rate, |w| getf(w, |e| e.spawn_mode == SpawnMode::Rate, false));
    let count = row_num(commands, fonts, "Count", 1.0, 1.0, 10000.0, |w| getf(w, |e| e.spawn_count as f32, 0.0), |w, v| setf(w, |e| e.spawn_count = v.round().max(1.0) as u32));
    bind_display(commands, count, |w| getf(w, |e| matches!(e.spawn_mode, SpawnMode::Burst | SpawnMode::BurstRate), false));
    let bursts = row_num(commands, fonts, "Bursts/sec", 0.1, 0.1, 100.0, |w| getf(w, |e| e.spawn_rate, 0.0), |w, v| setf(w, |e| e.spawn_rate = *v));
    bind_display(commands, bursts, |w| getf(w, |e| e.spawn_mode == SpawnMode::BurstRate, false));
    let dur = row_num(commands, fonts, "Duration", 0.1, 0.0, 600.0, |w| getf(w, |e| e.spawn_duration, 0.0), |w, v| setf(w, |e| e.spawn_duration = *v));
    let cycles = row_num(commands, fonts, "Cycles", 1.0, 0.0, 1000.0, |w| getf(w, |e| e.spawn_cycle_count as f32, 0.0), |w, v| setf(w, |e| e.spawn_cycle_count = v.round().max(0.0) as u32));
    let active = row_bool(commands, fonts, "Starts Active", |w| getf(w, |e| e.spawn_starts_active, true), |w, v| setf(w, |e| e.spawn_starts_active = *v));
    commands.entity(body).add_children(&[mode, rate, count, bursts, dur, cycles, active]);
    root
}

fn section_lifetime(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = collapsible(commands, fonts, Some("timer"), "Lifetime", true);
    let min = row_num(commands, fonts, "Min", 0.1, 0.01, 60.0, |w| getf(w, |e| e.lifetime_min, 0.0), |w, v| setf(w, |e| {
        e.lifetime_min = *v;
        if e.lifetime_min > e.lifetime_max { e.lifetime_max = e.lifetime_min; }
    }));
    let max = row_num(commands, fonts, "Max", 0.1, 0.01, 60.0, |w| getf(w, |e| e.lifetime_max, 0.0), |w, v| setf(w, |e| {
        e.lifetime_max = *v;
        if e.lifetime_max < e.lifetime_min { e.lifetime_min = e.lifetime_max; }
    }));
    commands.entity(body).add_children(&[min, max]);
    root
}

fn section_velocity(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = collapsible(commands, fonts, Some("arrows-out"), "Velocity", true);
    let mode = row_combo(commands, fonts, "Mode", |w| match getf(w, |e| e.velocity_mode, VelocityMode::Directional) {
        VelocityMode::Directional => "Directional".into(),
        VelocityMode::Radial => "Radial".into(),
        VelocityMode::Tangent => "Tangent".into(),
        VelocityMode::Random => "Random".into(),
    }, vec![
        ("Directional".into(), act(|e| e.velocity_mode = VelocityMode::Directional)),
        ("Radial".into(), act(|e| e.velocity_mode = VelocityMode::Radial)),
        ("Tangent".into(), act(|e| e.velocity_mode = VelocityMode::Tangent)),
        ("Random".into(), act(|e| e.velocity_mode = VelocityMode::Random)),
    ]);
    let speed = row_num(commands, fonts, "Speed", 0.1, 0.0, 100.0, |w| getf(w, |e| e.velocity_magnitude, 0.0), |w, v| setf(w, |e| e.velocity_magnitude = *v));
    let smin = row_num(commands, fonts, "Speed Min", 0.1, 0.0, 100.0, |w| getf(w, |e| e.velocity_speed_min, 0.0), |w, v| setf(w, |e| e.velocity_speed_min = *v));
    let smax = row_num(commands, fonts, "Speed Max", 0.1, 0.0, 100.0, |w| getf(w, |e| e.velocity_speed_max, 0.0), |w, v| setf(w, |e| e.velocity_speed_max = *v));
    let spread = row_num(commands, fonts, "Spread", 0.05, 0.0, std::f32::consts::PI, |w| getf(w, |e| e.velocity_spread, 0.0), |w, v| setf(w, |e| e.velocity_spread = *v));
    bind_display(commands, spread, |w| getf(w, |e| e.velocity_mode == VelocityMode::Directional, false));
    let dir = row_vec3(commands, fonts, "Direction",
        Arc::new(|w, i| getf(w, |e| e.velocity_direction[i], 0.0)),
        Arc::new(|w, i, v| setf(w, |e| e.velocity_direction[i] = v)));
    bind_display(commands, dir, |w| getf(w, |e| e.velocity_mode == VelocityMode::Directional, false));
    let axis = row_vec3(commands, fonts, "Axis",
        Arc::new(|w, i| getf(w, |e| e.velocity_axis[i], 0.0)),
        Arc::new(|w, i, v| setf(w, |e| e.velocity_axis[i] = v)));
    bind_display(commands, axis, |w| getf(w, |e| e.velocity_mode == VelocityMode::Tangent, false));
    commands.entity(body).add_children(&[mode, speed, smin, smax, spread, dir, axis]);
    root
}

fn section_forces(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = collapsible(commands, fonts, Some("wind"), "Forces", true);
    let accel = row_vec3(commands, fonts, "Accel",
        Arc::new(|w, i| getf(w, |e| e.acceleration[i], 0.0)),
        Arc::new(|w, i, v| setf(w, |e| e.acceleration[i] = v)));
    let presets = row_actions(commands, fonts, "Presets", vec![
        ("None", act(|e| e.acceleration = [0.0, 0.0, 0.0])),
        ("Light", act(|e| e.acceleration = [0.0, -2.0, 0.0])),
        ("Normal", act(|e| e.acceleration = [0.0, -9.8, 0.0])),
    ]);
    let drag = row_num(commands, fonts, "Drag", 0.05, 0.0, 10.0, |w| getf(w, |e| e.linear_drag, 0.0), |w, v| setf(w, |e| e.linear_drag = *v));
    let radial = row_num(commands, fonts, "Radial Accel", 0.1, -100.0, 100.0, |w| getf(w, |e| e.radial_acceleration, 0.0), |w, v| setf(w, |e| e.radial_acceleration = *v));
    let tangent = row_num(commands, fonts, "Tangent Accel", 0.1, -100.0, 100.0, |w| getf(w, |e| e.tangent_acceleration, 0.0), |w, v| setf(w, |e| e.tangent_acceleration = *v));
    let taxis = row_vec3(commands, fonts, "Tangent Axis",
        Arc::new(|w, i| getf(w, |e| e.tangent_accel_axis[i], 0.0)),
        Arc::new(|w, i, v| setf(w, |e| e.tangent_accel_axis[i] = v)));
    bind_display(commands, taxis, |w| getf(w, |e| e.tangent_acceleration.abs() > 0.001, false));
    commands.entity(body).add_children(&[accel, presets, drag, radial, tangent, taxis]);
    root
}

fn section_size(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = collapsible(commands, fonts, Some("resize"), "Size Over Lifetime", true);
    let nonu = row_bool(commands, fonts, "Non-Uniform", |w| getf(w, |e| e.size_non_uniform, false), |w, v| setf(w, |e| e.size_non_uniform = *v));
    let sx = row_num(commands, fonts, "Start X", 0.01, 0.001, 10.0, |w| getf(w, |e| e.size_start_x, 0.0), |w, v| setf(w, |e| e.size_start_x = *v));
    let sy = row_num(commands, fonts, "Start Y", 0.01, 0.001, 10.0, |w| getf(w, |e| e.size_start_y, 0.0), |w, v| setf(w, |e| e.size_start_y = *v));
    let ex = row_num(commands, fonts, "End X", 0.01, 0.0, 10.0, |w| getf(w, |e| e.size_end_x, 0.0), |w, v| setf(w, |e| e.size_end_x = *v));
    let ey = row_num(commands, fonts, "End Y", 0.01, 0.0, 10.0, |w| getf(w, |e| e.size_end_y, 0.0), |w, v| setf(w, |e| e.size_end_y = *v));
    for r in [sx, sy, ex, ey] {
        bind_display(commands, r, |w| getf(w, |e| e.size_non_uniform, false));
    }
    let start = row_num(commands, fonts, "Start", 0.01, 0.001, 10.0, |w| getf(w, |e| e.size_start, 0.0), |w, v| setf(w, |e| e.size_start = *v));
    let end = row_num(commands, fonts, "End", 0.01, 0.0, 10.0, |w| getf(w, |e| e.size_end, 0.0), |w, v| setf(w, |e| e.size_end = *v));
    let presets = row_actions(commands, fonts, "Presets", vec![
        ("Constant", act(|e| e.size_end = e.size_start)),
        ("Shrink", act(|e| e.size_end = 0.0)),
        ("Grow", act(|e| e.size_end = e.size_start * 2.0)),
    ]);
    for r in [start, end, presets] {
        bind_display(commands, r, |w| getf(w, |e| !e.size_non_uniform, false));
    }
    let rmin = row_num(commands, fonts, "Random Min", 0.01, 0.0, 10.0, |w| getf(w, |e| e.size_start_min, 0.0), |w, v| setf(w, |e| e.size_start_min = *v));
    let rmax = row_num(commands, fonts, "Random Max", 0.01, 0.0, 10.0, |w| getf(w, |e| e.size_start_max, 0.0), |w, v| setf(w, |e| e.size_start_max = *v));
    let screen = row_bool(commands, fonts, "Screen Space", |w| getf(w, |e| e.screen_space_size, false), |w, v| setf(w, |e| e.screen_space_size = *v));
    let round = row_num(commands, fonts, "Roundness", 0.01, 0.0, 1.0, |w| getf(w, |e| e.roundness, 0.0), |w, v| setf(w, |e| e.roundness = v.clamp(0.0, 1.0)));
    commands.entity(body).add_children(&[nonu, sx, sy, ex, ey, start, end, presets, rmin, rmax, screen, round]);
    root
}

fn section_noise(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = collapsible(commands, fonts, Some("spiral"), "Noise Turbulence", false);
    let freq = row_num(commands, fonts, "Frequency", 0.1, 0.0, 100.0, |w| getf(w, |e| e.noise_frequency, 0.0), |w, v| setf(w, |e| e.noise_frequency = *v));
    let amp = row_num(commands, fonts, "Amplitude", 0.1, 0.0, 100.0, |w| getf(w, |e| e.noise_amplitude, 0.0), |w, v| setf(w, |e| e.noise_amplitude = *v));
    let oct = row_num(commands, fonts, "Octaves", 1.0, 1.0, 8.0, |w| getf(w, |e| e.noise_octaves as f32, 0.0), |w, v| setf(w, |e| e.noise_octaves = v.round().clamp(1.0, 8.0) as u32));
    let lac = row_num(commands, fonts, "Lacunarity", 0.1, 1.0, 4.0, |w| getf(w, |e| e.noise_lacunarity, 0.0), |w, v| setf(w, |e| e.noise_lacunarity = *v));
    commands.entity(body).add_children(&[freq, amp, oct, lac]);
    root
}

fn section_velocity_limit(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = collapsible(commands, fonts, Some("gauge"), "Velocity Limit", false);
    let lim = row_num(commands, fonts, "Max Speed", 0.1, 0.0, 1000.0, |w| getf(w, |e| e.velocity_limit, 0.0), |w, v| setf(w, |e| e.velocity_limit = *v));
    commands.entity(body).add_child(lim);
    root
}

fn section_simulation(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = collapsible(commands, fonts, Some("gear"), "Simulation", false);
    let space = row_combo(commands, fonts, "Space", |w| match getf(w, |e| e.simulation_space, SimulationSpace::Local) {
        SimulationSpace::Local => "Local".into(),
        SimulationSpace::World => "World".into(),
    }, vec![
        ("Local".into(), act(|e| e.simulation_space = SimulationSpace::Local)),
        ("World".into(), act(|e| e.simulation_space = SimulationSpace::World)),
    ]);
    let update = row_combo(commands, fonts, "Update", |w| match getf(w, |e| e.simulation_condition, SimulationCondition::Always) {
        SimulationCondition::Always => "Always".into(),
        SimulationCondition::WhenVisible => "Visible".into(),
    }, vec![
        ("Always".into(), act(|e| e.simulation_condition = SimulationCondition::Always)),
        ("Visible".into(), act(|e| e.simulation_condition = SimulationCondition::WhenVisible)),
    ]);
    let integ = row_combo(commands, fonts, "Integration", |w| match getf(w, |e| e.motion_integration, renzora_hanabi::MotionIntegrationMode::PostUpdate) {
        renzora_hanabi::MotionIntegrationMode::PostUpdate => "Post-Update".into(),
        renzora_hanabi::MotionIntegrationMode::PreUpdate => "Pre-Update".into(),
        renzora_hanabi::MotionIntegrationMode::None => "None".into(),
    }, vec![
        ("Post-Update".into(), act(|e| e.motion_integration = renzora_hanabi::MotionIntegrationMode::PostUpdate)),
        ("Pre-Update".into(), act(|e| e.motion_integration = renzora_hanabi::MotionIntegrationMode::PreUpdate)),
        ("None".into(), act(|e| e.motion_integration = renzora_hanabi::MotionIntegrationMode::None)),
    ]);
    commands.entity(body).add_children(&[space, update, integ]);
    root
}

fn section_rendering(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = collapsible(commands, fonts, Some("cube"), "Rendering", false);
    let alpha = row_combo(commands, fonts, "Alpha Mode", |w| match getf(w, |e| e.alpha_mode, ParticleAlphaMode::Blend) {
        ParticleAlphaMode::Blend => "Blend".into(),
        ParticleAlphaMode::Premultiply => "Premultiply".into(),
        ParticleAlphaMode::Add => "Additive".into(),
        ParticleAlphaMode::Multiply => "Multiply".into(),
        ParticleAlphaMode::Mask => "Mask".into(),
        ParticleAlphaMode::Opaque => "Opaque".into(),
    }, vec![
        ("Blend".into(), act(|e| e.alpha_mode = ParticleAlphaMode::Blend)),
        ("Premultiply".into(), act(|e| e.alpha_mode = ParticleAlphaMode::Premultiply)),
        ("Additive".into(), act(|e| e.alpha_mode = ParticleAlphaMode::Add)),
        ("Multiply".into(), act(|e| e.alpha_mode = ParticleAlphaMode::Multiply)),
        ("Mask".into(), act(|e| e.alpha_mode = ParticleAlphaMode::Mask)),
        ("Opaque".into(), act(|e| e.alpha_mode = ParticleAlphaMode::Opaque)),
    ]);
    let thresh = row_num(commands, fonts, "Mask Threshold", 0.01, 0.0, 1.0, |w| getf(w, |e| e.alpha_mask_threshold, 0.0), |w, v| setf(w, |e| e.alpha_mask_threshold = v.clamp(0.0, 1.0)));
    bind_display(commands, thresh, |w| getf(w, |e| e.alpha_mode == ParticleAlphaMode::Mask, false));
    let orient = row_combo(commands, fonts, "Orient Mode", |w| match getf(w, |e| e.orient_mode, ParticleOrientMode::ParallelCameraDepthPlane) {
        ParticleOrientMode::ParallelCameraDepthPlane => "Camera Plane".into(),
        ParticleOrientMode::FaceCameraPosition => "Face Camera".into(),
        ParticleOrientMode::AlongVelocity => "Along Velocity".into(),
    }, vec![
        ("Camera Plane".into(), act(|e| e.orient_mode = ParticleOrientMode::ParallelCameraDepthPlane)),
        ("Face Camera".into(), act(|e| e.orient_mode = ParticleOrientMode::FaceCameraPosition)),
        ("Along Velocity".into(), act(|e| e.orient_mode = ParticleOrientMode::AlongVelocity)),
    ]);
    let rot = row_num(commands, fonts, "Rotation Speed", 0.1, -20.0, 20.0, |w| getf(w, |e| e.rotation_speed, 0.0), |w, v| setf(w, |e| e.rotation_speed = *v));
    let tex_row = base_row(commands, fonts, "Texture");
    let ti = text_input(commands, &fonts.ui, "textures/...", "");
    bind_text_input(commands, ti, |w| getf(w, |e| e.texture_path.clone().unwrap_or_default(), String::new()), |w, v| setf(w, |e| e.texture_path = if v.is_empty() { None } else { Some(v) }));
    commands.entity(tex_row.1).add_child(ti);
    let layer = row_num(commands, fonts, "Layer", 1.0, 0.0, 31.0, |w| getf(w, |e| e.render_layer as f32, 0.0), |w, v| setf(w, |e| e.render_layer = v.round().clamp(0.0, 31.0) as u8));
    commands.entity(body).add_children(&[alpha, thresh, orient, rot, tex_row.0, layer]);
    root
}

// ── Row builders ─────────────────────────────────────────────────────────────

fn base_row(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> (Entity, Entity) {
    let row = commands
        .spawn(Node { width: Val::Percent(100.0), min_height: Val::Px(22.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)), ..default() })
        .id();
    let lbl = commands
        .spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), bevy::text::TextLayout::new_with_no_wrap(), Node { width: Val::Px(LABEL_W), flex_shrink: 0.0, overflow: Overflow::clip(), ..default() }))
        .id();
    let cell = commands
        .spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::FlexEnd, column_gap: Val::Px(3.0), ..default() })
        .id();
    commands.entity(row).add_children(&[lbl, cell]);
    (row, cell)
}

#[allow(clippy::too_many_arguments)]
fn row_num<G, S>(commands: &mut Commands, fonts: &EmberFonts, label: &str, step: f32, min: f32, max: f32, get: G, set: S) -> Entity
where
    G: Fn(&World) -> f32 + Send + Sync + 'static,
    S: Fn(&mut World, &f32) + Send + Sync + 'static,
{
    let (row, cell) = base_row(commands, fonts, label);
    let field = num_field(commands, fonts, "", value_text(), 0.0, step, min, max, get, set);
    commands.entity(cell).add_child(field);
    row
}

fn row_bool<G, S>(commands: &mut Commands, fonts: &EmberFonts, label: &str, get: G, set: S) -> Entity
where
    G: Fn(&World) -> bool + Send + Sync + 'static,
    S: Fn(&mut World, &bool) + Send + Sync + 'static,
{
    let (row, cell) = base_row(commands, fonts, label);
    let cb = checkbox(commands, false);
    bind_2way(commands, cb, get, set);
    commands.entity(cell).add_child(cb);
    row
}

fn row_vec3(commands: &mut Commands, fonts: &EmberFonts, label: &str, get: Arc<dyn Fn(&World, usize) -> f32 + Send + Sync>, set: Arc<dyn Fn(&mut World, usize, f32) + Send + Sync>) -> Entity {
    let (row, cell) = base_row(commands, fonts, label);
    let mut fields = Vec::with_capacity(3);
    for (i, &(axis, col)) in AXES3.iter().enumerate() {
        let g = get.clone();
        let s = set.clone();
        let field = num_field(commands, fonts, axis, col, 0.0, 0.1, 0.0, 0.0, move |w| g(w, i), move |w, v| s(w, i, *v));
        fields.push(field);
    }
    commands.entity(cell).add_children(&fields);
    row
}

fn row_combo(commands: &mut Commands, fonts: &EmberFonts, label: &str, value: impl Fn(&World) -> String + Send + Sync + 'static, options: Vec<(String, Action)>) -> Entity {
    let (row, cell) = base_row(commands, fonts, label);
    let combo = commands
        .spawn((
            Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            RelativeCursorPosition::default(),
            Combo { options },
        ))
        .id();
    let vtext = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), Node { min_width: Val::Px(80.0), ..default() })).id();
    bind_text(commands, vtext, value);
    let caret = renzora_ember::font::icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 9.0);
    commands.entity(combo).add_children(&[vtext, caret]);
    commands.entity(cell).add_child(combo);
    row
}

fn row_actions(commands: &mut Commands, fonts: &EmberFonts, label: &str, buttons: Vec<(&str, Action)>) -> Entity {
    let (row, cell) = base_row(commands, fonts, label);
    let kids: Vec<Entity> = buttons.into_iter().map(|(txt, action)| small_button(commands, fonts, txt, action)).collect();
    commands.entity(cell).add_children(&kids);
    row
}

fn action_button(commands: &mut Commands, fonts: &EmberFonts, label: &str, color: (u8, u8, u8), action: Action) -> Entity {
    let btn = commands
        .spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() }, BackgroundColor(rgb(card_bg())), Interaction::default(), ActionBtn(action))).id();
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(color)))).id();
    commands.entity(btn).add_child(t);
    btn
}

fn small_button(commands: &mut Commands, fonts: &EmberFonts, label: &str, action: Action) -> Entity {
    let btn = commands
        .spawn((Node { align_items: AlignItems::Center, justify_content: JustifyContent::Center, padding: UiRect::axes(Val::Px(5.0), Val::Px(2.0)), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() }, BackgroundColor(rgb(card_bg())), Interaction::default(), ActionBtn(action))).id();
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_primary())))).id();
    commands.entity(btn).add_child(t);
    btn
}

#[allow(clippy::too_many_arguments)]
fn num_field<G, S>(commands: &mut Commands, fonts: &EmberFonts, axis: &str, axis_color: (u8, u8, u8), init: f32, step: f32, min: f32, max: f32, get: G, set: S) -> Entity
where
    G: Fn(&World) -> f32 + Send + Sync + 'static,
    S: Fn(&mut World, &f32) + Send + Sync + 'static,
{
    let dv = drag_value(commands, &fonts.ui, axis, axis_color, init, step);
    if max > min {
        commands.entity(dv).insert(DragRange { min, max });
    }
    bind_2way(commands, dv, get, set);
    dv
}

// ── Generic combo + action button ────────────────────────────────────────────

#[derive(Component)]
struct Combo {
    options: Vec<(String, Action)>,
}
#[derive(Component)]
struct ActionBtn(Action);

fn action_btn_click(q: Query<(&Interaction, &ActionBtn), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed {
            let action = btn.0.clone();
            commands.queue(move |w: &mut World| action(w));
        }
    }
}

fn combo_open(
    q: Query<(&Interaction, &RelativeCursorPosition, &ComputedNode, &Combo), Changed<Interaction>>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else { return };
    let Some((_, rcp, cn, combo)) = q.iter().find(|(i, _, _, _)| **i == Interaction::Pressed) else { return };
    let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) else { return };
    let size = cn.size() * cn.inverse_scale_factor();
    let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
    let menu = screen_menu(&mut commands, top_left.x, top_left.y + size.y + 2.0);
    let kids: Vec<Entity> = combo
        .options
        .iter()
        .map(|(label, action)| {
            let action = action.clone();
            menu_item(&mut commands, &fonts, "circle", label, move |w| action(w))
        })
        .collect();
    commands.entity(menu).add_children(&kids);
}
