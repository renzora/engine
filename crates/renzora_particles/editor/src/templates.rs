//! New Effect: a gallery of the built-in effects, as cards grouped by kind, and
//! what picking one does (writes a new `.vfx` into the Assets panel's current
//! folder and opens it).
//!
//! The cards are the built-in presets plus two blank starters (one 3D,
//! one 2D), each a working emitter rather than an empty graph: an editor that
//! opens on nothing moving gives no hint where to start.
//!
//! Card previews are icons on the effect's own colour rather than renders. A
//! live thumbnail would need a preview rig per card or a capture pipeline for
//! a handful of files that never change; a glyph says "fire" just as well.
//!
//! The gallery opens one frame after it is asked for. Every route to it is a
//! click (the Assets menu, the toolbar, the empty panel), and an overlay
//! spawned on the very frame of a press is closed by ember's click-outside
//! rule before its card has a cursor-over state to say the click was inside.

use std::path::{Path, PathBuf};

use bevy::prelude::*;

use renzora::core::CurrentProject;
use renzora_editor_framework::{DocTabKind, SplashState};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::reactive::tracked::bind_bg;
use renzora_ember::theme::*;
use renzora_ember::widgets::{overlay_val_parts, scroll_view, HoverTooltip};
use renzora_ember::UiInteraction;
use renzora_particles::asset::{parse_vfx, write_vfx, Connection, Dimension, Emitter, PinValue, VfxAsset, VfxGraph, VfxNode};
use renzora_particles::presets::PRESETS;

use crate::model;

/// One card in the gallery.
pub struct Template {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub icon: &'static str,
    pub color: (u8, u8, u8),
    /// The preset this starts from, by name; `None` for a blank starter.
    preset: Option<&'static str>,
    dimension: Dimension,
    /// A starter built in code rather than a preset or a blank one.
    make: Option<fn() -> VfxAsset>,
}

pub const TEMPLATES: &[Template] = &[
    Template {
        id: "blank_3d",
        name: "Blank 3D",
        category: "Start",
        description: "One emitter spraying soft dots from a small sphere.",
        icon: "sparkle",
        color: (120, 150, 230),
        preset: None,
        dimension: Dimension::ThreeD,
        make: None,
    },
    Template {
        id: "blank_2d",
        name: "Blank 2D",
        category: "Start",
        description: "One emitter for a 2D scene, in pixels.",
        icon: "square",
        color: (110, 190, 200),
        preset: None,
        dimension: Dimension::TwoD,
        make: None,
    },
    Template {
        id: "campfire",
        name: "Campfire",
        category: "3D",
        description: "Additive flames with embers drifting off them.",
        icon: "fire",
        color: (240, 120, 50),
        preset: Some("Campfire"),
        dimension: Dimension::ThreeD,
        make: None,
    },
    Template {
        id: "realistic_fire",
        name: "Realistic Fire",
        category: "3D",
        description: "A log fire in five layers: procedural flame tongues, smoke, embers and glowing coals, all pushed by the wind.",
        icon: "flame",
        color: (255, 150, 40),
        preset: Some("Realistic Fire"),
        dimension: Dimension::ThreeD,
        make: None,
    },
    Template {
        id: "sparks",
        name: "Sparks Burst",
        category: "3D",
        description: "A one-shot burst of velocity-stretched sparks under gravity.",
        icon: "lightning",
        color: (250, 200, 70),
        preset: Some("Sparks Burst"),
        dimension: Dimension::ThreeD,
        make: None,
    },
    Template {
        id: "smoke",
        name: "Smoke",
        category: "3D",
        description: "Slow, swirling puffs that grow and fade.",
        icon: "cloud",
        color: (150, 160, 190),
        preset: Some("Smoke"),
        dimension: Dimension::ThreeD,
        make: None,
    },
    Template {
        id: "confetti",
        name: "Confetti",
        category: "2D",
        description: "Spinning paper that bursts up and tumbles down.",
        icon: "confetti",
        color: (230, 90, 160),
        preset: Some("Confetti"),
        dimension: Dimension::TwoD,
        make: None,
    },
    Template {
        id: "sparkles",
        name: "Magic Sparkles",
        category: "2D",
        description: "Twinkling stars around a point.",
        icon: "star",
        color: (170, 120, 240),
        preset: Some("Magic Sparkles"),
        dimension: Dimension::TwoD,
        make: None,
    },
    Template {
        id: "gpu_stress",
        name: "GPU Stress",
        category: "GPU",
        description: "200,000 sparks alive at once, fountaining off a floor: a load only the GPU backend carries. Falls back to the CPU, slowly, where there is no compute.",
        icon: "cpu",
        color: (90, 210, 160),
        preset: None,
        dimension: Dimension::ThreeD,
        make: Some(gpu_stress),
    },
];

/// The stress starter: one emitter of 200k velocity-stretched sparks shot up
/// a cone, falling under gravity and bouncing off a ground plane. Every part
/// of it is GPU-supported (shape, randoms, a gradient, a collider), and it is
/// set to the GPU explicitly so a machine without compute says why it is
/// slow rather than quietly running 200k particles on the CPU.
fn gpu_stress() -> VfxAsset {
    let mut out = VfxNode::new(1, renzora_particles::nodes::OUTPUT_NODE)
        .with("spawn_rate", PinValue::Float(100_000.0))
        .with("size", PinValue::Vec2([0.012, 0.05]))
        .with("gravity", PinValue::Vec3([0.0, -9.81, 0.0]))
        .with("drag", PinValue::Float(0.25))
        .with("color", PinValue::Color([4.0, 1.8, 0.6, 1.0]));
    out.position = model::OUTPUT_POSITION;
    let mut cone = VfxNode::new(2, "shape/cone").with("angle", PinValue::Float(22.0)).with("radius", PinValue::Float(0.05));
    cone.position = [0.0, 0.0];
    let mut speed = VfxNode::new(3, "random/range").with("min", PinValue::Float(4.0)).with("max", PinValue::Float(9.0));
    speed.position = [0.0, 160.0];
    let mut mul = VfxNode::new(4, "math/multiply");
    mul.position = [260.0, 60.0];
    let mut life = VfxNode::new(5, "random/range")
        .with("min", PinValue::Float(1.5))
        .with("max", PinValue::Float(2.5))
        .with("salt", PinValue::Float(1.0));
    life.position = [0.0, 300.0];
    let mut fade = VfxNode::new(6, "curve/gradient").with("stops", PinValue::Gradient(model::default_gradient()));
    fade.position = [260.0, 220.0];
    let mut floor = VfxNode::new(7, "collide/plane").with("bounce", PinValue::Float(0.35));
    floor.position = [260.0, 380.0];
    let graph = VfxGraph {
        nodes: vec![out, cone, speed, mul, life, fade, floor],
        connections: vec![
            Connection::new(2, "position", 1, "position"),
            Connection::new(2, "direction", 4, "a"),
            Connection::new(3, "value", 4, "b"),
            Connection::new(4, "value", 1, "velocity"),
            Connection::new(5, "value", 1, "lifetime"),
            Connection::new(6, "color", 1, "color_over_life"),
        ],
        ..Default::default()
    };
    let mut emitter = Emitter {
        name: "Sparks".to_string(),
        capacity: 200_000,
        seed: 11,
        graph,
        backend: renzora_particles::asset::Backend::Gpu,
        ..Default::default()
    };
    emitter.render.blend = renzora_particles::asset::BlendMode::Additive;
    emitter.render.facing = renzora_particles::asset::Facing::VelocityAligned;
    emitter.render.stretch = 0.015;
    VfxAsset { dimension: Dimension::ThreeD, duration: 4.0, looping: true, emitters: vec![emitter], ..Default::default() }
}

impl Template {
    /// The document this template starts as, named `name`.
    pub fn build(&self, name: &str) -> VfxAsset {
        let mut doc = match (self.make, self.preset.and_then(|p| PRESETS.iter().find(|x| x.name == p))) {
            (Some(make), _) => make(),
            (None, Some(preset)) => parse_vfx(preset.source).unwrap_or_default(),
            (None, None) => blank(self.dimension),
        };
        doc.name = name.to_string();
        model::normalize(&mut doc);
        doc
    }
}

/// A working single-emitter effect: a sphere (3D) or circle (2D) emitting
/// outward, sized for the dimension's units.
fn blank(dimension: Dimension) -> VfxAsset {
    let (shape, radius, speed, size) = match dimension {
        Dimension::ThreeD => ("shape/sphere", 0.1, 1.5, 0.08),
        Dimension::TwoD => ("shape/circle", 8.0, 120.0, 8.0),
    };
    let mut out = VfxNode::new(1, renzora_particles::nodes::OUTPUT_NODE)
        .with("spawn_rate", PinValue::Float(30.0))
        .with("lifetime", PinValue::Float(1.5))
        .with("size", PinValue::Vec2([size, size]));
    out.position = model::OUTPUT_POSITION;
    let mut sh = VfxNode::new(2, shape).with("radius", PinValue::Float(radius));
    sh.position = [0.0, 0.0];
    let mut mul = VfxNode::new(3, "math/multiply").with("b", PinValue::Float(speed));
    mul.position = [260.0, 80.0];
    let mut fade = VfxNode::new(4, "curve/gradient").with("stops", PinValue::Gradient(model::default_gradient()));
    fade.position = [260.0, 220.0];
    let graph = VfxGraph {
        nodes: vec![out, sh, mul, fade],
        connections: vec![
            Connection::new(2, "position", 1, "position"),
            Connection::new(2, "direction", 3, "a"),
            Connection::new(3, "value", 1, "velocity"),
            Connection::new(4, "color", 1, "color_over_life"),
        ],
        ..Default::default()
    };
    VfxAsset {
        dimension,
        emitters: vec![Emitter { name: "Emitter".to_string(), graph, ..Default::default() }],
        ..Default::default()
    }
}

/// `dir/stem.vfx`, or `dir/stem_2.vfx` and so on if that is taken.
pub fn unique_path(dir: &Path, stem: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.vfx"));
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|i| dir.join(format!("{stem}_{i}.vfx")))
        .find(|p| !p.exists())
        .unwrap_or(first)
}

/// Write `template` as a new `.vfx` in `dir`. Returns the file written.
pub fn create_from_template(dir: &Path, template: &Template) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let stem = template.id.trim_start_matches("blank_");
    let stem = if template.preset.is_none() { "new_effect" } else { stem };
    let path = unique_path(dir, stem);
    let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or(stem).to_string();
    let text = write_vfx(&template.build(&name)).map_err(std::io::Error::other)?;
    std::fs::write(&path, text)?;
    Ok(path)
}

// ── Gallery ──────────────────────────────────────────────────────────────────

pub struct TemplateGalleryPlugin;

impl Plugin for TemplateGalleryPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (open_gallery, card_click, new_effect_click).run_if(in_state(SplashState::Editor)));
        // The Assets panel's create menu, through the registry every plugin
        // uses. The entry opens the gallery; the starter text is a blank 3D
        // effect for any route that cannot show one.
        use renzora::create_menu::{CreateMenuItem, RegisterCreateMenuItem};
        app.register_create_menu_item(
            CreateMenuItem::new("vfx_effect", "new_effect", "vfx", |_| write_vfx(&TEMPLATES[0].build("new_effect")).unwrap_or_default())
                .label("assets.new.vfx_effect", "Particle Effect")
                .subtitle("assets.new.vfx_effect_sub", "Node-graph effect from a template")
                .icon("magic-wand")
                .folder("effects")
                .opens(request_gallery),
        );
    }
}

/// Asks for the gallery next frame (see the module docs).
#[derive(Resource)]
struct GalleryRequest;

pub fn request_gallery(world: &mut World) {
    world.insert_resource(GalleryRequest);
}

#[derive(Component)]
struct GalleryRoot;

#[derive(Component)]
struct TemplateCard(usize);

fn open_gallery(
    request: Option<Res<GalleryRequest>>,
    mouse: Res<ButtonInput<MouseButton>>,
    fonts: Option<Res<EmberFonts>>,
    open: Query<(), With<GalleryRoot>>,
    mut commands: Commands,
) {
    if request.is_none() || mouse.just_pressed(MouseButton::Left) {
        return;
    }
    commands.remove_resource::<GalleryRequest>();
    let Some(fonts) = fonts else { return };
    if !open.is_empty() {
        return;
    }
    let parts = overlay_val_parts(&mut commands, &fonts, "New Particle Effect", Val::Px(640.0), Val::Px(520.0), true);
    commands.entity(parts.root).insert(GalleryRoot);
    commands.entity(parts.titlebar).insert(BackgroundColor(rgb(faint_bg())));

    let body = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(14.0), padding: UiRect::all(Val::Px(16.0)), ..default() })
        .id();
    let intro = commands
        .spawn((
            Text::new("Pick a starting point. The effect is created in the Assets panel's current folder and opened in the Particles workspace."),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    commands.entity(body).add_child(intro);
    let mut cats: Vec<&str> = Vec::new();
    for t in TEMPLATES {
        if !cats.contains(&t.category) {
            cats.push(t.category);
        }
    }
    for cat in cats {
        let header = commands.spawn((Text::new(cat.to_uppercase()), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())))).id();
        let grid = commands
            .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(10.0), row_gap: Val::Px(10.0), ..default() })
            .id();
        let cards: Vec<Entity> = TEMPLATES
            .iter()
            .enumerate()
            .filter(|(_, t)| t.category == cat)
            .map(|(i, t)| card(&mut commands, &fonts, i, t))
            .collect();
        commands.entity(grid).add_children(&cards);
        commands.entity(body).add_children(&[header, grid]);
    }
    let scroll = scroll_view(&mut commands, body);
    commands.entity(parts.content).add_child(scroll);
}

fn card(commands: &mut Commands, fonts: &EmberFonts, index: usize, t: &Template) -> Entity {
    let card = commands
        .spawn((
            Node {
                width: Val::Px(186.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            BorderColor::all(rgb(border())),
            UiInteraction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            HoverTooltip::new(t.description),
            TemplateCard(index),
            Name::new("vfx-template-card"),
        ))
        .id();
    bind_bg(commands, card, move |w| {
        let hot = matches!(w.get::<UiInteraction>(card), Some(UiInteraction::Hovered | UiInteraction::Pressed));
        rgb(if hot { hover_bg() } else { card_bg() })
    });
    // The preview: the effect's glyph on a wash of its colour, fading into
    // the card so every tile reads as one family.
    let c = t.color;
    let dark = |v: u8| (v as f32 * 0.22) as u8;
    let preview = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(86.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() },
            BackgroundColor(Color::NONE),
            BackgroundGradient(vec![Gradient::Linear(LinearGradient::new(
                180f32.to_radians(),
                vec![ColorStop::auto(rgb((dark(c.0) + 20, dark(c.1) + 20, dark(c.2) + 26))), ColorStop::auto(rgb(card_bg()))],
            ))]),
            PASS_THROUGH,
        ))
        .id();
    let glyph = icon_text(commands, &fonts.phosphor, t.icon, c, 34.0);
    commands.entity(glyph).insert(PASS_THROUGH);
    commands.entity(preview).add_child(glyph);
    let text = commands
        .spawn((Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), padding: UiRect::all(Val::Px(10.0)), ..default() }, PASS_THROUGH))
        .id();
    let name = commands.spawn((Text::new(t.name), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_primary())), PASS_THROUGH)).id();
    let emitters = t.build(t.name).emitters.len();
    let sub = format!(
        "{} \u{00b7} {} emitter{}",
        match t.dimension {
            Dimension::ThreeD => "3D",
            Dimension::TwoD => "2D",
        },
        emitters,
        if emitters == 1 { "" } else { "s" }
    );
    let sub = commands.spawn((Text::new(sub), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), PASS_THROUGH)).id();
    commands.entity(text).add_children(&[name, sub]);
    commands.entity(card).add_children(&[preview, text]);
    card
}

fn card_click(q: Query<(&UiInteraction, &TemplateCard), Changed<UiInteraction>>, roots: Query<Entity, With<GalleryRoot>>, mut commands: Commands) {
    let Some((_, card)) = q.iter().find(|(i, _)| **i == UiInteraction::Pressed) else { return };
    let index = card.0;
    for r in &roots {
        commands.entity(r).despawn();
    }
    commands.queue(move |w: &mut World| create_and_open(w, index));
}

/// Write the template into the browser's current folder (or `effects/`) and
/// open it as a Particle document, which switches to the Particles workspace.
fn create_and_open(world: &mut World, index: usize) {
    let Some(template) = TEMPLATES.get(index) else { return };
    let Some(root) = world.get_resource::<CurrentProject>().map(|p| p.path.clone()) else {
        renzora::console_log::console_warn("Particles", "Open a project before creating an effect");
        return;
    };
    let rel = world
        .get_resource::<renzora::core::AssetBrowserCwd>()
        .and_then(|c| c.0.clone())
        .unwrap_or_else(|| "effects".to_string());
    let dir = if rel.is_empty() { root } else { root.join(rel) };
    match create_from_template(&dir, template) {
        Ok(path) => {
            info!("[vfx_editor] created {}", path.display());
            renzora_editor_framework::open_asset_tab(world, &path, DocTabKind::Particle);
        }
        Err(e) => renzora::console_log::console_warn("Particles", format!("Could not create the effect: {e}")),
    }
}

/// The placeholder a panel shows with no effect open: what to do, and a
/// button that does it.
pub fn empty_state(commands: &mut Commands, fonts: &EmberFonts, hint: &str) -> Entity {
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(10.0),
            padding: UiRect::all(Val::Px(24.0)),
            ..default()
        })
        .id();
    let icon = icon_text(commands, &fonts.phosphor, "magic-wand", (255, 140, 70), 30.0);
    let title = commands.spawn((Text::new("No effect open"), ui_font(&fonts.ui, 15.0), TextColor(rgb(text_primary())))).id();
    let text = commands
        .spawn((Text::new(hint), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted())), bevy::text::TextLayout::justify(bevy::text::Justify::Center)))
        .id();
    let button = commands
        .spawn((
            Node {
                height: Val::Px(30.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::horizontal(Val::Px(14.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(accent())),
            UiInteraction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            NewEffectButton,
        ))
        .id();
    bind_bg(commands, button, move |w| {
        let hot = matches!(w.get::<UiInteraction>(button), Some(UiInteraction::Hovered | UiInteraction::Pressed));
        if hot { renzora_ember::theme::mix(accent(), text_primary(), 0.15) } else { rgb(accent()) }
    });
    let plus = icon_text(commands, &fonts.phosphor, "file-plus", on_accent(), 14.0);
    let label = commands.spawn((Text::new("New Effect"), ui_font(&fonts.ui, 12.0), TextColor(rgb(on_accent())), PASS_THROUGH)).id();
    commands.entity(plus).insert(PASS_THROUGH);
    commands.entity(button).add_children(&[plus, label]);
    commands.entity(root).add_children(&[icon, title, text, button]);
    root
}

/// The empty state's button; clicked through [`new_effect_click`].
#[derive(Component)]
pub struct NewEffectButton;

pub fn new_effect_click(q: Query<&UiInteraction, (With<NewEffectButton>, Changed<UiInteraction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == UiInteraction::Pressed) {
        commands.queue(request_gallery);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_particles::compile::compile_graph;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("renzora_vfx_templates_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn every_preset_has_a_card() {
        for preset in PRESETS {
            assert!(TEMPLATES.iter().any(|t| t.preset == Some(preset.name)), "{} has no card", preset.name);
        }
    }

    #[test]
    fn every_template_builds_a_document_that_compiles() {
        for t in TEMPLATES {
            let doc = t.build("x");
            assert_eq!(doc.dimension, t.dimension, "{}", t.name);
            assert!(!doc.emitters.is_empty());
            for e in &doc.emitters {
                compile_graph(&e.graph).unwrap_or_else(|errs| panic!("{}: {errs:?}", t.name));
            }
        }
    }

    #[test]
    fn the_stress_template_runs_on_the_gpu_where_there_is_one() {
        use renzora_particles::gpu::select::{choose, gpu_blockers, ActiveBackend, GpuCaps};
        let t = TEMPLATES.iter().find(|t| t.id == "gpu_stress").expect("stress card");
        let doc = t.build("stress");
        let e = &doc.emitters[0];
        assert!(e.capacity >= 200_000);
        let compiled = compile_graph(&e.graph).unwrap();
        assert!(gpu_blockers(e, &compiled, false, false, &GpuCaps::all()).is_empty(), "every feature it uses is on the GPU");
        assert_eq!(choose(e, &compiled, false, &GpuCaps::all()).active, ActiveBackend::Gpu);
        let program = renzora_particles::gpu::codegen::generate(&compiled, e.space).unwrap();
        renzora::wgsl::check(&program.source).expect("its kernels validate");
        // Without the GPU it says so, rather than silently crawling.
        assert!(choose(e, &compiled, false, &GpuCaps::none()).fell_back);
    }

    #[test]
    fn creating_from_a_template_writes_a_valid_vfx() {
        let dir = temp_dir("valid");
        let path = create_from_template(&dir, &TEMPLATES[2]).unwrap();
        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("vfx"));
        let doc = parse_vfx(&std::fs::read_to_string(&path).unwrap()).expect("the written file parses");
        assert_eq!(doc.name, "campfire");
        assert_eq!(doc, TEMPLATES[2].build("campfire"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_second_effect_from_one_template_gets_a_new_name() {
        let dir = temp_dir("unique");
        let a = create_from_template(&dir, &TEMPLATES[0]).unwrap();
        let b = create_from_template(&dir, &TEMPLATES[0]).unwrap();
        assert_ne!(a, b);
        assert_eq!(b.file_name().and_then(|n| n.to_str()), Some("new_effect_2.vfx"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
