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

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use renzora_editor_framework::SplashState;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::inspector::color_field;
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_2way, bind_display, bind_text, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{bind_text_input, checkbox, collapsible, drag_value, menu_item, screen_menu, text_input, DragRange};

use renzora_hanabi::{
    ConformToSphere, EditorMode, EffectVariable, GradientStop, HanabiEffectDefinition, HanabiEmitShape, KillZone,
    OrbitSettings, ParticleAlphaMode, ParticleColorBlendMode, ParticleEditorState, ParticleOrientMode,
    ShapeDimension, SimulationCondition, SimulationSpace, SpawnMode, VelocityMode,
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

    // Simple-mode section stack.
    let sections = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() }).id();
    for section in [
        section_general as fn(&mut Commands, &EmberFonts) -> Entity,
        section_spawning,
        section_lifetime,
        section_shape,
        section_velocity,
        section_forces,
        section_conform,
        section_noise,
        section_orbit,
        section_velocity_limit,
        section_size,
        section_color,
        section_simulation,
        section_rendering,
        section_kill_zones,
        section_variables,
    ] {
        let s = section(commands, fonts);
        commands.entity(sections).add_child(s);
    }
    bind_display(commands, sections, |w| !is_advanced(w));

    // Advanced (graph) mode shows the graph + its node inspector elsewhere.
    let adv_note = commands
        .spawn(Node { width: Val::Percent(100.0), align_items: AlignItems::Center, padding: UiRect::vertical(Val::Px(20.0)), ..default() })
        .id();
    let adv_lbl = commands.spawn((Text::new("Advanced mode — edit nodes in the Particle Graph panel"), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id();
    commands.entity(adv_note).add_child(adv_lbl);
    bind_display(commands, adv_note, is_advanced);

    commands.entity(body).add_children(&[header, sections, adv_note]);
    commands.entity(root).add_children(&[welcome, body]);
    root
}

fn is_advanced(w: &World) -> bool {
    w.get_resource::<ParticleEditorState>().is_some_and(|s| s.editor_mode == EditorMode::Graph)
}

fn toggle_advanced(w: &mut World) {
    if let Some(mut s) = w.get_resource_mut::<ParticleEditorState>() {
        if s.editor_mode == EditorMode::Graph {
            s.editor_mode = EditorMode::Simple;
        } else {
            s.editor_mode = EditorMode::Graph;
            let g = s.current_effect.as_ref().map(renzora_hanabi::node_graph::ParticleNodeGraph::from_effect);
            s.node_graph = g;
        }
    }
}

fn build_header(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), border: UiRect::bottom(Val::Px(1.0)), ..default() })
        .id();
    commands.entity(bar).insert((BorderColor::all(rgb(border())),));
    // Advanced/Simple toggle (reactive label).
    let adv = commands
        .spawn((Node { padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() }, BackgroundColor(rgb(card_bg())), Interaction::default(), ActionBtn(Arc::new(toggle_advanced))))
        .id();
    let adv_t = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())))).id();
    bind_text(commands, adv_t, |w| if is_advanced(w) { "Switch to Simple".into() } else { "Switch to Advanced".into() });
    commands.entity(adv).add_child(adv_t);
    let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let save = action_button(commands, fonts, "Save", text_primary(), Arc::new(|w: &mut World| save_current(w, false)));
    let save_as = action_button(commands, fonts, "Save As", text_muted(), Arc::new(|w: &mut World| save_current(w, true)));
    commands.entity(bar).add_children(&[adv, spacer, save, save_as]);
    bar
}

// ── Part-2 sections: Shape (enum payload), Conform, Orbit ─────────────────────

fn section_shape(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = collapsible(commands, fonts, Some("shapes"), "Emission Shape", true);
    let shape = row_combo(commands, fonts, "Shape", |w| getf(w, |e| shape_name(&e.emit_shape), "Point").to_string(), vec![
        ("Point".into(), act(|e| e.emit_shape = HanabiEmitShape::Point)),
        ("Circle".into(), act(|e| e.emit_shape = HanabiEmitShape::Circle { radius: 1.0, dimension: ShapeDimension::Volume })),
        ("Sphere".into(), act(|e| e.emit_shape = HanabiEmitShape::Sphere { radius: 1.0, dimension: ShapeDimension::Volume })),
        ("Cone".into(), act(|e| e.emit_shape = HanabiEmitShape::Cone { base_radius: 0.5, top_radius: 0.0, height: 1.0, dimension: ShapeDimension::Volume })),
        ("Rectangle".into(), act(|e| e.emit_shape = HanabiEmitShape::Rect { half_extents: [1.0, 1.0], dimension: ShapeDimension::Volume })),
        ("Box".into(), act(|e| e.emit_shape = HanabiEmitShape::Box { half_extents: [1.0, 1.0, 1.0] })),
    ]);
    commands.entity(body).add_child(shape);

    // Radius (Circle | Sphere).
    let radius = row_num(commands, fonts, "Radius", 0.1, 0.001, 100.0,
        |w| getf(w, |e| match &e.emit_shape { HanabiEmitShape::Circle { radius, .. } | HanabiEmitShape::Sphere { radius, .. } => *radius, _ => 0.0 }, 0.0),
        |w, v| setf(w, |e| if let HanabiEmitShape::Circle { radius, .. } | HanabiEmitShape::Sphere { radius, .. } = &mut e.emit_shape { *radius = *v; }));
    bind_display(commands, radius, |w| getf(w, |e| matches!(e.emit_shape, HanabiEmitShape::Circle { .. } | HanabiEmitShape::Sphere { .. }), false));
    commands.entity(body).add_child(radius);

    // Cone fields.
    let base_r = row_num(commands, fonts, "Base Radius", 0.1, 0.0, 100.0,
        |w| getf(w, |e| if let HanabiEmitShape::Cone { base_radius, .. } = &e.emit_shape { *base_radius } else { 0.0 }, 0.0),
        |w, v| setf(w, |e| if let HanabiEmitShape::Cone { base_radius, .. } = &mut e.emit_shape { *base_radius = *v; }));
    let top_r = row_num(commands, fonts, "Top Radius", 0.1, 0.0, 100.0,
        |w| getf(w, |e| if let HanabiEmitShape::Cone { top_radius, .. } = &e.emit_shape { *top_radius } else { 0.0 }, 0.0),
        |w, v| setf(w, |e| if let HanabiEmitShape::Cone { top_radius, .. } = &mut e.emit_shape { *top_radius = *v; }));
    let height = row_num(commands, fonts, "Height", 0.1, 0.001, 100.0,
        |w| getf(w, |e| if let HanabiEmitShape::Cone { height, .. } = &e.emit_shape { *height } else { 0.0 }, 0.0),
        |w, v| setf(w, |e| if let HanabiEmitShape::Cone { height, .. } = &mut e.emit_shape { *height = *v; }));
    for r in [base_r, top_r, height] {
        bind_display(commands, r, |w| getf(w, |e| matches!(e.emit_shape, HanabiEmitShape::Cone { .. }), false));
        commands.entity(body).add_child(r);
    }

    // Rect / Box extents.
    let rx = row_num(commands, fonts, "Extents X", 0.1, 0.001, 100.0, |w| shape_ext(w, 0), |w, v| set_shape_ext(w, 0, *v));
    let ry = row_num(commands, fonts, "Extents Y", 0.1, 0.001, 100.0, |w| shape_ext(w, 1), |w, v| set_shape_ext(w, 1, *v));
    for r in [rx, ry] {
        bind_display(commands, r, |w| getf(w, |e| matches!(e.emit_shape, HanabiEmitShape::Rect { .. } | HanabiEmitShape::Box { .. }), false));
        commands.entity(body).add_child(r);
    }
    let rz = row_num(commands, fonts, "Extents Z", 0.1, 0.001, 100.0, |w| shape_ext(w, 2), |w, v| set_shape_ext(w, 2, *v));
    bind_display(commands, rz, |w| getf(w, |e| matches!(e.emit_shape, HanabiEmitShape::Box { .. }), false));
    commands.entity(body).add_child(rz);

    // Emit-from dimension (Circle | Sphere | Cone | Rect).
    let dim = row_combo(commands, fonts, "Emit from", |w| match getf(w, |e| shape_dimension(&e.emit_shape), Some(true)) {
        Some(true) => "Volume".into(),
        Some(false) => "Surface".into(),
        None => "—".into(),
    }, vec![
        ("Volume".into(), act(|e| set_dimension(e, ShapeDimension::Volume))),
        ("Surface".into(), act(|e| set_dimension(e, ShapeDimension::Surface))),
    ]);
    bind_display(commands, dim, |w| getf(w, |e| matches!(e.emit_shape, HanabiEmitShape::Circle { .. } | HanabiEmitShape::Sphere { .. } | HanabiEmitShape::Cone { .. } | HanabiEmitShape::Rect { .. }), false));
    commands.entity(body).add_child(dim);
    root
}

fn shape_name(s: &HanabiEmitShape) -> &'static str {
    match s {
        HanabiEmitShape::Point => "Point",
        HanabiEmitShape::Circle { .. } => "Circle",
        HanabiEmitShape::Sphere { .. } => "Sphere",
        HanabiEmitShape::Cone { .. } => "Cone",
        HanabiEmitShape::Rect { .. } => "Rectangle",
        HanabiEmitShape::Box { .. } => "Box",
    }
}

fn shape_dimension(s: &HanabiEmitShape) -> Option<bool> {
    match s {
        HanabiEmitShape::Circle { dimension, .. }
        | HanabiEmitShape::Sphere { dimension, .. }
        | HanabiEmitShape::Cone { dimension, .. }
        | HanabiEmitShape::Rect { dimension, .. } => Some(*dimension == ShapeDimension::Volume),
        _ => None,
    }
}

fn set_dimension(e: &mut HanabiEffectDefinition, d: ShapeDimension) {
    match &mut e.emit_shape {
        HanabiEmitShape::Circle { dimension, .. }
        | HanabiEmitShape::Sphere { dimension, .. }
        | HanabiEmitShape::Cone { dimension, .. }
        | HanabiEmitShape::Rect { dimension, .. } => *dimension = d,
        _ => {}
    }
}

fn shape_ext(w: &World, i: usize) -> f32 {
    getf(w, |e| match &e.emit_shape {
        HanabiEmitShape::Rect { half_extents, .. } => half_extents.get(i).copied().unwrap_or(0.0),
        HanabiEmitShape::Box { half_extents } => half_extents.get(i).copied().unwrap_or(0.0),
        _ => 0.0,
    }, 0.0)
}

fn set_shape_ext(w: &mut World, i: usize, v: f32) {
    setf(w, |e| match &mut e.emit_shape {
        HanabiEmitShape::Rect { half_extents, .. } => {
            if let Some(c) = half_extents.get_mut(i) { *c = v; }
        }
        HanabiEmitShape::Box { half_extents } => {
            if let Some(c) = half_extents.get_mut(i) { *c = v; }
        }
        _ => {}
    });
}

fn section_conform(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = collapsible(commands, fonts, Some("atom"), "Conform to Sphere", false);
    let en = row_bool(commands, fonts, "Enabled", |w| getf(w, |e| e.conform_to_sphere.is_some(), false), |w, v| setf(w, |e| e.conform_to_sphere = if *v { Some(ConformToSphere::default()) } else { None }));
    commands.entity(body).add_child(en);
    let origin = row_vec3(commands, fonts, "Origin",
        Arc::new(|w, i| getf(w, |e| e.conform_to_sphere.as_ref().map(|c| c.origin[i]).unwrap_or(0.0), 0.0)),
        Arc::new(|w, i, v| setf(w, |e| if let Some(c) = e.conform_to_sphere.as_mut() { c.origin[i] = v; })));
    let radius = cnf_num(commands, fonts, "Radius", 0.1, 0.1, 100.0, |c| c.radius, |c, v| c.radius = v);
    let infl = cnf_num(commands, fonts, "Influence Dist", 0.1, 0.0, 100.0, |c| c.influence_dist, |c, v| c.influence_dist = v);
    let accel = cnf_num(commands, fonts, "Accel", 0.1, 0.0, 100.0, |c| c.attraction_accel, |c, v| c.attraction_accel = v);
    let maxs = cnf_num(commands, fonts, "Max Speed", 0.1, 0.0, 100.0, |c| c.max_attraction_speed, |c, v| c.max_attraction_speed = v);
    let shell = cnf_num(commands, fonts, "Shell Thick.", 0.01, 0.0, 10.0, |c| c.shell_half_thickness, |c, v| c.shell_half_thickness = v);
    let sticky = cnf_num(commands, fonts, "Sticky Factor", 0.01, 0.0, 10.0, |c| c.sticky_factor, |c, v| c.sticky_factor = v);
    for r in [origin, radius, infl, accel, maxs, shell, sticky] {
        bind_display(commands, r, |w| getf(w, |e| e.conform_to_sphere.is_some(), false));
        commands.entity(body).add_child(r);
    }
    root
}

fn cnf_num(commands: &mut Commands, fonts: &EmberFonts, label: &str, step: f32, min: f32, max: f32, get: impl Fn(&ConformToSphere) -> f32 + Send + Sync + Copy + 'static, set: impl Fn(&mut ConformToSphere, f32) + Send + Sync + Copy + 'static) -> Entity {
    row_num(commands, fonts, label, step, min, max,
        move |w| getf(w, |e| e.conform_to_sphere.as_ref().map(get).unwrap_or(0.0), 0.0),
        move |w, v| setf(w, |e| if let Some(c) = e.conform_to_sphere.as_mut() { set(c, *v); }))
}

fn section_orbit(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = collapsible(commands, fonts, Some("planet"), "Orbit", false);
    let en = row_bool(commands, fonts, "Enabled", |w| getf(w, |e| e.orbit.is_some(), false), |w, v| setf(w, |e| e.orbit = if *v { Some(OrbitSettings::default()) } else { None }));
    commands.entity(body).add_child(en);
    let center = row_vec3(commands, fonts, "Center",
        Arc::new(|w, i| getf(w, |e| e.orbit.as_ref().map(|o| o.center[i]).unwrap_or(0.0), 0.0)),
        Arc::new(|w, i, v| setf(w, |e| if let Some(o) = e.orbit.as_mut() { o.center[i] = v; })));
    let axis = row_vec3(commands, fonts, "Axis",
        Arc::new(|w, i| getf(w, |e| e.orbit.as_ref().map(|o| o.axis[i]).unwrap_or(0.0), 0.0)),
        Arc::new(|w, i, v| setf(w, |e| if let Some(o) = e.orbit.as_mut() { o.axis[i] = v; })));
    let speed = orb_num(commands, fonts, "Speed", 0.01, -20.0, 20.0, |o| o.speed, |o, v| o.speed = v);
    let pull = orb_num(commands, fonts, "Radial Pull", 0.01, 0.0, 20.0, |o| o.radial_pull, |o, v| o.radial_pull = v);
    let orad = orb_num(commands, fonts, "Orbit Radius", 0.1, 0.1, 100.0, |o| o.orbit_radius, |o, v| o.orbit_radius = v);
    for r in [center, axis, speed, pull, orad] {
        bind_display(commands, r, |w| getf(w, |e| e.orbit.is_some(), false));
        commands.entity(body).add_child(r);
    }
    root
}

fn orb_num(commands: &mut Commands, fonts: &EmberFonts, label: &str, step: f32, min: f32, max: f32, get: impl Fn(&OrbitSettings) -> f32 + Send + Sync + Copy + 'static, set: impl Fn(&mut OrbitSettings, f32) + Send + Sync + Copy + 'static) -> Entity {
    row_num(commands, fonts, label, step, min, max,
        move |w| getf(w, |e| e.orbit.as_ref().map(get).unwrap_or(0.0), 0.0),
        move |w, v| setf(w, |e| if let Some(o) = e.orbit.as_mut() { set(o, *v); }))
}

// ── Part-3 sections: Color (gradient), Kill Zones, Variables ──────────────────

fn section_color(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = collapsible(commands, fonts, Some("palette"), "Color Over Lifetime", true);
    let flat_check = row_bool(commands, fonts, "Flat Color", |w| getf(w, |e| e.use_flat_color, false), |w, v| setf(w, |e| e.use_flat_color = *v));
    commands.entity(body).add_child(flat_check);

    let (flat_row, flat_cell) = base_row(commands, fonts, "Color");
    let cf = color_field(commands, |w| getf(w, |e| [e.flat_color[0], e.flat_color[1], e.flat_color[2]], [1.0; 3]), |w, c| setf(w, |e| e.flat_color = [c[0], c[1], c[2], e.flat_color[3]]));
    commands.entity(flat_cell).add_child(cf);
    bind_display(commands, flat_row, |w| getf(w, |e| e.use_flat_color, false));
    commands.entity(body).add_child(flat_row);

    let grad = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() }).id();
    let stops = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() }).id();
    keyed_list(commands, stops, gradient_snapshot);
    let actions = row_actions(commands, fonts, "Gradient", vec![
        ("+ Add", Arc::new(|w: &mut World| setf(w, |e| if e.color_gradient.len() < 8 {
            e.color_gradient.push(GradientStop { position: 0.5, color: [1.0, 1.0, 1.0, 1.0] });
            e.color_gradient.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap_or(std::cmp::Ordering::Equal));
        }))),
        ("Fire", act(|e| e.color_gradient = vec![
            GradientStop { position: 0.0, color: [1.0, 1.0, 0.3, 1.0] },
            GradientStop { position: 0.3, color: [1.0, 0.5, 0.0, 1.0] },
            GradientStop { position: 1.0, color: [0.3, 0.1, 0.1, 0.0] },
        ])),
        ("Smoke", act(|e| e.color_gradient = vec![
            GradientStop { position: 0.0, color: [0.3, 0.3, 0.3, 0.8] },
            GradientStop { position: 1.0, color: [0.5, 0.5, 0.5, 0.0] },
        ])),
    ]);
    commands.entity(grad).add_children(&[stops, actions]);
    bind_display(commands, grad, |w| getf(w, |e| !e.use_flat_color, false));
    commands.entity(body).add_child(grad);

    let hdr_check = row_bool(commands, fonts, "HDR Color", |w| getf(w, |e| e.use_hdr_color, false), |w, v| setf(w, |e| e.use_hdr_color = *v));
    let hdr_int = row_num(commands, fonts, "HDR Intensity", 0.1, 1.0, 100.0, |w| getf(w, |e| e.hdr_intensity, 1.0), |w, v| setf(w, |e| e.hdr_intensity = *v));
    bind_display(commands, hdr_int, |w| getf(w, |e| e.use_hdr_color, false));
    let blend = row_combo(commands, fonts, "Blend Mode", |w| match getf(w, |e| e.color_blend_mode, ParticleColorBlendMode::Modulate) {
        ParticleColorBlendMode::Modulate => "Modulate".into(),
        ParticleColorBlendMode::Overwrite => "Overwrite".into(),
        ParticleColorBlendMode::Add => "Add".into(),
    }, vec![
        ("Modulate".into(), act(|e| e.color_blend_mode = ParticleColorBlendMode::Modulate)),
        ("Overwrite".into(), act(|e| e.color_blend_mode = ParticleColorBlendMode::Overwrite)),
        ("Add".into(), act(|e| e.color_blend_mode = ParticleColorBlendMode::Add)),
    ]);
    commands.entity(body).add_children(&[hdr_check, hdr_int, blend]);
    root
}

fn gradient_snapshot(world: &World) -> KeyedSnapshot {
    let len = getf(world, |e| e.color_gradient.len(), 0);
    let items: Vec<(u64, u64)> = (0..len)
        .map(|i| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            len.hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot { items, build: Box::new(move |c, f, i| stop_row(c, f, i, len > 2)) }
}

fn stop_row(commands: &mut Commands, fonts: &EmberFonts, i: usize, can_remove: bool) -> Entity {
    let (row, cell) = base_row(commands, fonts, &format!("Stop {}", i + 1));
    let pos = num_field(commands, fonts, "", value_text(), 0.0, 0.01, 0.0, 1.0,
        move |w| getf(w, |e| e.color_gradient.get(i).map(|s| s.position).unwrap_or(0.0), 0.0),
        move |w, v| setf(w, |e| if let Some(s) = e.color_gradient.get_mut(i) { s.position = v.clamp(0.0, 1.0); }));
    let cf = color_field(commands,
        move |w| getf(w, |e| e.color_gradient.get(i).map(|s| [s.color[0], s.color[1], s.color[2]]).unwrap_or([1.0; 3]), [1.0; 3]),
        move |w, c| setf(w, |e| if let Some(s) = e.color_gradient.get_mut(i) { s.color = [c[0], c[1], c[2], s.color[3]]; }));
    commands.entity(cell).add_children(&[pos, cf]);
    if can_remove {
        let rm = small_button(commands, fonts, "\u{2715}", Arc::new(move |w: &mut World| setf(w, |e| if e.color_gradient.len() > 2 && i < e.color_gradient.len() { e.color_gradient.remove(i); })));
        commands.entity(cell).add_child(rm);
    }
    row
}

fn section_kill_zones(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = collapsible(commands, fonts, Some("prohibit"), "Kill Zones", false);
    let list = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() }).id();
    keyed_list(commands, list, kill_snapshot);
    let add = row_actions(commands, fonts, "Add", vec![
        ("+ Sphere", act(|e| e.kill_zones.push(KillZone::Sphere { center: [0.0, 0.0, 0.0], radius: 5.0, kill_inside: false }))),
        ("+ AABB", act(|e| e.kill_zones.push(KillZone::Aabb { center: [0.0, 0.0, 0.0], half_size: [5.0, 5.0, 5.0], kill_inside: false }))),
    ]);
    commands.entity(body).add_children(&[list, add]);
    root
}

fn kill_snapshot(world: &World) -> KeyedSnapshot {
    let kinds: Vec<u8> = getf(world, |e| e.kill_zones.iter().map(|z| matches!(z, KillZone::Aabb { .. }) as u8).collect(), Vec::new());
    let len = kinds.len();
    let items: Vec<(u64, u64)> = kinds
        .iter()
        .enumerate()
        .map(|(i, k)| {
            let mut kk = hasher();
            i.hash(&mut kk);
            let mut h = hasher();
            (k, len).hash(&mut h);
            (kk.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot { items, build: Box::new(move |c, f, i| kill_zone_group(c, f, i, kinds[i])) }
}

fn kill_zone_group(commands: &mut Commands, fonts: &EmberFonts, i: usize, is_aabb: u8) -> Entity {
    let col = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() }).id();
    let title = commands.spawn((Text::new(format!("{} {}", if is_aabb == 1 { "AABB" } else { "Sphere" }, i + 1)), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), Node { padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)), ..default() })).id();
    let center = row_vec3(commands, fonts, "Center",
        Arc::new(move |w, k| getf(w, |e| match e.kill_zones.get(i) {
            Some(KillZone::Sphere { center, .. }) | Some(KillZone::Aabb { center, .. }) => center.get(k).copied().unwrap_or(0.0),
            _ => 0.0,
        }, 0.0)),
        Arc::new(move |w, k, v| setf(w, |e| match e.kill_zones.get_mut(i) {
            Some(KillZone::Sphere { center, .. }) | Some(KillZone::Aabb { center, .. }) => { if let Some(c) = center.get_mut(k) { *c = v; } }
            _ => {}
        })));
    commands.entity(col).add_children(&[title, center]);
    if is_aabb == 1 {
        let hs = row_vec3(commands, fonts, "Half Size",
            Arc::new(move |w, k| getf(w, |e| if let Some(KillZone::Aabb { half_size, .. }) = e.kill_zones.get(i) { half_size.get(k).copied().unwrap_or(0.0) } else { 0.0 }, 0.0)),
            Arc::new(move |w, k, v| setf(w, |e| if let Some(KillZone::Aabb { half_size, .. }) = e.kill_zones.get_mut(i) { if let Some(c) = half_size.get_mut(k) { *c = v; } })));
        commands.entity(col).add_child(hs);
    } else {
        let radius = row_num(commands, fonts, "Radius", 0.1, 0.1, 100.0,
            move |w| getf(w, |e| if let Some(KillZone::Sphere { radius, .. }) = e.kill_zones.get(i) { *radius } else { 0.0 }, 0.0),
            move |w, v| setf(w, |e| if let Some(KillZone::Sphere { radius, .. }) = e.kill_zones.get_mut(i) { *radius = *v; }));
        commands.entity(col).add_child(radius);
    }
    let inside = row_bool(commands, fonts, "Kill Inside",
        move |w| getf(w, |e| match e.kill_zones.get(i) {
            Some(KillZone::Sphere { kill_inside, .. }) | Some(KillZone::Aabb { kill_inside, .. }) => *kill_inside,
            _ => false,
        }, false),
        move |w, v| setf(w, |e| match e.kill_zones.get_mut(i) {
            Some(KillZone::Sphere { kill_inside, .. }) | Some(KillZone::Aabb { kill_inside, .. }) => *kill_inside = *v,
            _ => {}
        }));
    let remove = row_actions(commands, fonts, "", vec![("\u{2715} Remove", Arc::new(move |w: &mut World| setf(w, |e| if i < e.kill_zones.len() { e.kill_zones.remove(i); })) as Action)]);
    commands.entity(col).add_children(&[inside, remove]);
    col
}

fn section_variables(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = collapsible(commands, fonts, Some("code"), "Variables", false);
    let hint = commands.spawn((Text::new("Exposed to scripts"), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())), Node { padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)), ..default() })).id();
    let list = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() }).id();
    keyed_list(commands, list, variables_snapshot);
    let add = row_actions(commands, fonts, "Add", vec![
        ("+ Float", Arc::new(|w: &mut World| setf(w, |e| { let n = format!("var_{}", e.variables.len()); e.variables.insert(n, EffectVariable::Float { value: 1.0, min: 0.0, max: 10.0 }); }))),
        ("+ Color", Arc::new(|w: &mut World| setf(w, |e| { let n = format!("color_{}", e.variables.len()); e.variables.insert(n, EffectVariable::Color { value: [1.0, 1.0, 1.0, 1.0] }); }))),
    ]);
    commands.entity(body).add_children(&[hint, list, add]);
    root
}

fn variables_snapshot(world: &World) -> KeyedSnapshot {
    let mut vars: Vec<(String, u8)> = getf(world, |e| e.variables.iter().map(|(k, v)| (k.clone(), var_disc(v))).collect(), Vec::new());
    vars.sort_by(|a, b| a.0.cmp(&b.0));
    let items: Vec<(u64, u64)> = vars
        .iter()
        .map(|(name, disc)| {
            let mut k = hasher();
            name.hash(&mut k);
            let mut h = hasher();
            (name, disc).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot { items, build: Box::new(move |c, f, i| var_row(c, f, &vars[i].0, vars[i].1)) }
}

fn var_disc(v: &EffectVariable) -> u8 {
    match v {
        EffectVariable::Float { .. } => 0,
        EffectVariable::Color { .. } => 1,
        EffectVariable::Vec3 { .. } => 2,
    }
}

fn var_row(commands: &mut Commands, fonts: &EmberFonts, name: &str, disc: u8) -> Entity {
    let (row, cell) = base_row(commands, fonts, name);
    let n = name.to_string();
    match disc {
        1 => {
            let na = n.clone();
            let nb = n.clone();
            let cf = color_field(commands,
                move |w| getf(w, |e| if let Some(EffectVariable::Color { value }) = e.variables.get(&na) { [value[0], value[1], value[2]] } else { [1.0; 3] }, [1.0; 3]),
                move |w, c| setf(w, |e| if let Some(EffectVariable::Color { value }) = e.variables.get_mut(&nb) { *value = [c[0], c[1], c[2], value[3]]; }));
            commands.entity(cell).add_child(cf);
        }
        2 => {
            let get = Arc::new({
                let n = n.clone();
                move |w: &World, k: usize| getf(w, |e| if let Some(EffectVariable::Vec3 { value }) = e.variables.get(&n) { value.get(k).copied().unwrap_or(0.0) } else { 0.0 }, 0.0)
            });
            let set = Arc::new({
                let n = n.clone();
                move |w: &mut World, k: usize, v: f32| setf(w, |e| if let Some(EffectVariable::Vec3 { value }) = e.variables.get_mut(&n) { if let Some(c) = value.get_mut(k) { *c = v; } })
            });
            // Reuse the vec3 field trio inline (no label, into the cell).
            for (k, &(axis, col)) in AXES3.iter().enumerate() {
                let g = get.clone();
                let s = set.clone();
                let field = num_field(commands, fonts, axis, col, 0.0, 0.1, 0.0, 0.0, move |w| g(w, k), move |w, v| s(w, k, *v));
                commands.entity(cell).add_child(field);
            }
        }
        _ => {
            let na = n.clone();
            let nb = n.clone();
            let field = num_field(commands, fonts, "", value_text(), 0.0, 0.05, 0.0, 0.0,
                move |w| getf(w, |e| if let Some(EffectVariable::Float { value, .. }) = e.variables.get(&na) { *value } else { 0.0 }, 0.0),
                move |w, v| setf(w, |e| if let Some(EffectVariable::Float { value, .. }) = e.variables.get_mut(&nb) { *value = *v; }));
            commands.entity(cell).add_child(field);
        }
    }
    let rm = small_button(commands, fonts, "\u{2715}", Arc::new(move |w: &mut World| setf(w, |e| { e.variables.remove(&n); })));
    commands.entity(cell).add_child(rm);
    row
}

fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
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
    if save_effect_to_file(&target, &effect) {
        if let Some(mut s) = w.get_resource_mut::<ParticleEditorState>() {
            s.current_file_path = Some(target.to_string_lossy().to_string());
            s.is_modified = false;
        }
    }
}

/// Serialize a particle effect definition to a `.particle` (RON) file.
fn save_effect_to_file(path: &std::path::Path, effect: &HanabiEffectDefinition) -> bool {
    let pretty = ron::ser::PrettyConfig::new()
        .depth_limit(4)
        .separate_tuple_members(true);

    match ron::ser::to_string_pretty(effect, pretty) {
        Ok(contents) => match std::fs::write(path, contents) {
            Ok(_) => {
                info!("Saved particle effect to {:?}", path);
                true
            }
            Err(e) => {
                error!("Failed to write particle effect {:?}: {}", path, e);
                false
            }
        },
        Err(e) => {
            error!("Failed to serialize particle effect: {}", e);
            false
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
