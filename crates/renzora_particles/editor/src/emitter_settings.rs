//! The Emitter Settings panel: everything about the open effect that is not a
//! node. The effect's own fields (name, 2D or 3D, duration, looping) and, for
//! the emitter whose tab is showing, its simulation settings (enabled,
//! capacity, backend and what it is running on, space and inherited velocity,
//! seed) and how it is drawn (shading, the mesh shape, model and orientation
//! of mesh particles, the project material any 3D particle may wear, texture,
//! sprite sheet and frame blending, blend, the cutoff, which particles cast
//! shadows, the render method (forward or the deferred G-buffer), scene
//! lighting, facing and
//! stretch, sizes in pixels, erosion, soft distance, draw order, and a
//! ribbon's trail and shape, flat or a tube), the scene light it
//! casts, and its sub-emitters.
//!
//! An inspector-style list of ember rows (`inspector_row` in `section`s),
//! every control bound two-way to the document, so switching the emitter tab
//! re-points every field in place without rebuilding the panel.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora_editor_framework::{AssetDragPayload, SplashState};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::inspector::{color_field_rgba, inspector_row};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::tracked::{bind_2way, bind_display, bind_text, bind_with};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    bind_text_input, checkbox, drag_value, dropdown, scroll_view, section, text_input, DragRange, HoverTooltip,
};
use renzora_ember::UiInteraction;
use renzora_particles::asset::{
    Backend, BlendMode, Dimension, DrawOrder, Emitter, Facing, LightMode, MeshOrient, ParticleMesh, ParticleShadows,
    RenderMethod, RibbonKind, RibbonShape, Shading, SheetMode, SimulationSpace, SubEmitter, SubEmitterTrigger, VfxAsset, DEFAULT_PARTICLE_LIGHTS,
    MAX_PARTICLE_LIGHTS, MAX_TRAIL_POINTS, MAX_TUBE_SEGMENTS, MIN_TUBE_SEGMENTS,
};
use renzora_particles::gpu::layout::MAX_GPU_CAPACITY;
use renzora_particles::gpu::ActiveBackend;

use crate::model;
use crate::pin_editors::{linear_to_srgb, srgb_to_linear};
use crate::preview::VfxPreviewStats;
use crate::state::VfxEditorState;

pub const PANEL: &str = "vfx_emitter";

/// The shared image list, so the texture slot takes SVG like every other slot.
const IMG_EXTS: &[&str] = renzora::IMAGE_EXTENSIONS;

pub struct EmitterSettingsPanel;

impl Plugin for EmitterSettingsPanel {
    fn build(&self, app: &mut App) {
        app.register_panel_content(PANEL, false, build);
        app.add_systems(
            Update,
            (tex_drop, tex_clear, tex_browse, tex_highlight)
                .run_if(in_state(SplashState::Editor))
                .run_if(renzora_ember::dock::panel_active(PANEL)),
        );
    }
}

// ── Document access ──────────────────────────────────────────────────────────

fn doc_get<T: Default>(w: &Rx, f: impl FnOnce(&VfxAsset) -> T) -> T {
    w.get_resource::<VfxEditorState>().map(|s| f(&s.doc)).unwrap_or_default()
}

/// Apply `f` to the document only when it would change something, so a
/// binding settling on the value it already has records no edit.
fn doc_set<T: PartialEq>(w: &mut World, get: impl Fn(&VfxAsset) -> T, value: T, set: impl FnOnce(&mut VfxAsset)) {
    let Some(st) = w.get_resource::<VfxEditorState>() else { return };
    if !st.is_open() || get(&st.doc) == value {
        return;
    }
    set(&mut w.resource_mut::<VfxEditorState>().doc);
}

fn em_get<T: Default>(w: &Rx, f: impl FnOnce(&Emitter) -> T) -> T {
    w.get_resource::<VfxEditorState>()
        .and_then(|s| s.emitter_index().map(|i| &s.doc.emitters[i]))
        .map(f)
        .unwrap_or_default()
}

fn em_set<T: PartialEq>(w: &mut World, get: impl Fn(&Emitter) -> T, value: T, set: impl FnOnce(&mut Emitter)) {
    let Some(st) = w.get_resource::<VfxEditorState>() else { return };
    let Some(i) = st.emitter_index() else { return };
    if get(&st.doc.emitters[i]) == value {
        return;
    }
    set(&mut w.resource_mut::<VfxEditorState>().doc.emitters[i]);
}

// ── Build ────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() },
            BackgroundColor(rgb(panel_bg())),
            Name::new("vfx-emitter-settings"),
        ))
        .id();
    let empty = crate::templates::empty_state(commands, fonts, "Emitter settings appear here once an effect is open.");
    bind_display(commands, empty, |w| !w.get_resource::<VfxEditorState>().is_some_and(VfxEditorState::is_open));

    let body = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), padding: UiRect::all(Val::Px(6.0)), ..default() })
        .id();
    let effect = effect_section(commands, fonts);
    let emitter = emitter_section(commands, fonts);
    let render = render_section(commands, fonts);
    let light = light_section(commands, fonts);
    let subs = sub_section(commands, fonts);
    commands.entity(body).add_children(&[effect, emitter, render, light, subs]);
    let scroll = scroll_view(commands, body);
    bind_display(commands, scroll, |w| w.get_resource::<VfxEditorState>().is_some_and(VfxEditorState::is_open));
    commands.entity(root).add_children(&[empty, scroll]);
    root
}

fn effect_section(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = section(commands, fonts, "magic-wand", "Effect", (255, 140, 70));
    let name = text_input(commands, &fonts.ui, "Effect name", "");
    bind_text_input(commands, name, |w| doc_get(w, |d| d.name.clone()), |w, v| doc_set(w, |d| d.name.clone(), v.clone(), |d| d.name = v));
    let dim = dropdown(commands, fonts, &["3D", "2D"], 0);
    bind_2way(
        commands,
        dim,
        |w: &Rx| doc_get(w, |d| usize::from(d.dimension == Dimension::TwoD)),
        |w: &mut World, v: &usize| {
            let dim = if *v == 1 { Dimension::TwoD } else { Dimension::ThreeD };
            doc_set(w, |d| d.dimension, dim, |d| d.dimension = dim);
        },
    );
    let duration = num(commands, fonts, 0.05, Some((0.0, 600.0)), |w| doc_get(w, |d| d.duration), |w, v| doc_set(w, |d| d.duration, v.max(0.0), |d| d.duration = v.max(0.0)));
    let looping = check(commands, |w| doc_get(w, |d| d.looping), |w, v| doc_set(w, |d| d.looping, v, |d| d.looping = v));
    let rows = [
        inspector_row(commands, &fonts.ui, "Name", name),
        inspector_row(commands, &fonts.ui, "Dimension", dim),
        inspector_row(commands, &fonts.ui, "Duration", duration),
        inspector_row(commands, &fonts.ui, "Looping", looping),
    ];
    commands.entity(body).add_children(&rows);
    root
}

fn emitter_section(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = section(commands, fonts, "sparkle", "Emitter", accent());
    let name = text_input(commands, &fonts.ui, "Emitter name", "");
    bind_text_input(commands, name, |w| em_get(w, |e| e.name.clone()), |w, v| {
        let mut st = w.resource_mut::<VfxEditorState>();
        if let Some(i) = st.emitter_index() {
            // A taken or blank name is refused here and the field snaps back.
            model::rename_emitter(&mut st.doc, i, &v);
        }
    });
    let enabled = check(commands, |w| em_get(w, |e| e.enabled), |w, v| em_set(w, |e| e.enabled, v, |e| e.enabled = v));
    // Up to the GPU backend's pool limit: past a few thousand particles an
    // emitter is a GPU emitter, and a million is what that backend is for.
    let capacity = num(commands, fonts, 1.0, Some((1.0, MAX_GPU_CAPACITY as f32)), |w| em_get(w, |e| e.capacity as f32), |w, v| {
        let c = v.round().clamp(1.0, MAX_GPU_CAPACITY as f32) as u32;
        em_set(w, |e| e.capacity, c, |e| e.capacity = c);
    });
    let backend = dropdown(commands, fonts, &["Auto", "CPU", "GPU"], 0);
    bind_2way(
        commands,
        backend,
        |w: &Rx| em_get(w, |e| BACKENDS.iter().position(|b| *b == e.backend).unwrap_or(0)),
        |w: &mut World, v: &usize| {
            let b = BACKENDS.get(*v).copied().unwrap_or_default();
            em_set(w, |e| e.backend, b, |e| e.backend = b);
        },
    );
    // What the preview actually runs the emitter on, and why the CPU when
    // the setting left the choice open: the row a user reads when a big
    // emitter is slower than expected.
    let running = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), Node { flex_grow: 1.0, flex_shrink: 1.0, ..default() }))
        .id();
    bind_text(commands, running, |w| {
        let Some(st) = w.get_resource::<VfxEditorState>() else { return String::new() };
        let Some(i) = st.emitter_index() else { return String::new() };
        let stats = w.get_resource::<VfxPreviewStats>();
        match stats.and_then(|s| s.backends.get(i).cloned()).flatten() {
            Some((ActiveBackend::Gpu, _)) => "GPU".to_string(),
            Some((ActiveBackend::Cpu, Some(why))) => format!("CPU, because {why}"),
            Some((ActiveBackend::Cpu, None)) => "CPU".to_string(),
            None => "Not running".to_string(),
        }
    });
    let space = dropdown(commands, fonts, &["Local", "World"], 0);
    bind_2way(
        commands,
        space,
        |w: &Rx| em_get(w, |e| usize::from(e.space == SimulationSpace::World)),
        |w: &mut World, v: &usize| {
            let s = if *v == 1 { SimulationSpace::World } else { SimulationSpace::Local };
            em_set(w, |e| e.space, s, |e| e.space = s);
        },
    );
    let seed = num(commands, fonts, 1.0, Some((0.0, 1.0e9)), |w| em_get(w, |e| e.seed as f32), |w, v| {
        let s = v.round().max(0.0) as u64;
        em_set(w, |e| e.seed, s, |e| e.seed = s);
    });
    // A share of the entity's motion, past 1 for a sling. World space only:
    // a Local emitter already carries its particles along, so the row hides.
    let inherit = num(commands, fonts, 0.01, Some((0.0, 2.0)), |w| em_get(w, |e| e.inherit_velocity), |w, v| {
        let v = v.clamp(0.0, 2.0);
        em_set(w, |e| e.inherit_velocity, v, |e| e.inherit_velocity = v);
    });
    let inherit_row = inspector_row(commands, &fonts.ui, "Inherit Velocity", inherit);
    bind_display(commands, inherit_row, |w| em_get(w, |e| e.space == SimulationSpace::World));
    let rows = [
        inspector_row(commands, &fonts.ui, "Name", name),
        inspector_row(commands, &fonts.ui, "Enabled", enabled),
        inspector_row(commands, &fonts.ui, "Capacity", capacity),
        inspector_row(commands, &fonts.ui, "Backend", backend),
        inspector_row(commands, &fonts.ui, "Running On", running),
        inspector_row(commands, &fonts.ui, "Space", space),
        inherit_row,
        inspector_row(commands, &fonts.ui, "Seed", seed),
    ];
    commands.entity(body).add_children(&rows);
    root
}

const BACKENDS: [Backend; 3] = [Backend::Auto, Backend::Cpu, Backend::Gpu];

const DRAW_ORDERS: [(DrawOrder, &str); 4] = [
    (DrawOrder::Index, "Index"),
    (DrawOrder::OldestFirst, "Oldest First"),
    (DrawOrder::YoungestFirst, "Youngest First"),
    (DrawOrder::ViewDepth, "View Depth"),
];

const BLENDS: [(BlendMode, &str); 7] = [
    (BlendMode::Alpha, "Alpha"),
    (BlendMode::Additive, "Additive"),
    (BlendMode::Premultiplied, "Premultiplied"),
    (BlendMode::Multiply, "Multiply"),
    (BlendMode::AlphaMask, "Alpha Mask"),
    (BlendMode::AlphaToCoverage, "Alpha to Coverage"),
    (BlendMode::Opaque, "Opaque"),
];

const SHADOWS: [(ParticleShadows, &str); 3] =
    [(ParticleShadows::Auto, "Auto (Solid Blends)"), (ParticleShadows::On, "On"), (ParticleShadows::Off, "Off")];

/// The Shape dropdown: a quad, each built-in mesh, then a model. Index 0 is
/// no mesh at all, so a new emitter's quads are the first entry.
fn shape_labels() -> Vec<&'static str> {
    let mut labels = vec!["Quad (Sprite)"];
    labels.extend(ParticleMesh::BUILT_IN.iter().map(|m| match m {
        ParticleMesh::Quad => "Card (Mesh Quad)",
        other => other.label(),
    }));
    labels.push("Model");
    labels
}

fn shape_index(mesh: &Option<ParticleMesh>) -> usize {
    match mesh {
        None => 0,
        Some(ParticleMesh::Asset(_)) => ParticleMesh::BUILT_IN.len() + 1,
        Some(m) => ParticleMesh::BUILT_IN.iter().position(|b| b == m).map_or(0, |i| i + 1),
    }
}

fn shape_at(index: usize, current: &Option<ParticleMesh>) -> Option<ParticleMesh> {
    match index {
        0 => None,
        i if i <= ParticleMesh::BUILT_IN.len() => Some(ParticleMesh::BUILT_IN[i - 1].clone()),
        // Keep a model already chosen when the dropdown settles on Model.
        _ => match current {
            Some(ParticleMesh::Asset(p)) => Some(ParticleMesh::Asset(p.clone())),
            _ => Some(ParticleMesh::Asset(String::new())),
        },
    }
}

fn orient_index(o: MeshOrient) -> usize {
    match o {
        MeshOrient::Tumble => 0,
        MeshOrient::Velocity => 1,
        MeshOrient::Axis(_) => 2,
    }
}

/// Mesh particles are on: a mesh is set and the effect is 3D.
fn em_mesh(w: &Rx) -> bool {
    let dim = doc_get(w, |d| d.dimension);
    em_get(w, |e| e.render.draws_mesh(dim))
}

const SHADINGS: [(Shading, &str); 3] =
    [(Shading::Sprite, "Sprite"), (Shading::ProceduralFlame, "Procedural Flame"), (Shading::ProceduralSmoke, "Procedural Smoke")];

fn facing_index(f: Facing) -> usize {
    match f {
        Facing::CameraBillboard => 0,
        Facing::VelocityAligned => 1,
        Facing::FixedAxis(_) => 2,
        Facing::Ribbon => 3,
    }
}

fn render_section(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = section(commands, fonts, "image", "Render", (150, 210, 120));
    let texture = texture_slot(commands, fonts);
    let blend_labels: Vec<&str> = BLENDS.iter().map(|(_, l)| *l).collect();
    let blend = dropdown(commands, fonts, &blend_labels, 0);
    bind_2way(
        commands,
        blend,
        |w: &Rx| em_get(w, |e| BLENDS.iter().position(|(b, _)| *b == e.render.blend).unwrap_or(0)),
        |w: &mut World, v: &usize| {
            let b = BLENDS.get(*v).map(|(b, _)| *b).unwrap_or_default();
            em_set(w, |e| e.render.blend, b, |e| e.render.blend = b);
        },
    );
    let facing = dropdown(commands, fonts, &["Camera Billboard", "Velocity Aligned", "Fixed Axis", "Ribbon"], 0);
    bind_2way(
        commands,
        facing,
        |w: &Rx| em_get(w, |e| facing_index(e.render.facing)),
        |w: &mut World, v: &usize| {
            em_set(w, |e| facing_index(e.render.facing), *v, |e| {
                e.render.facing = match v {
                    1 => Facing::VelocityAligned,
                    2 => Facing::FixedAxis([0.0, 1.0, 0.0]),
                    3 => Facing::Ribbon,
                    _ => Facing::CameraBillboard,
                }
            });
        },
    );
    let axis = commands.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(3.0), flex_grow: 1.0, ..default() }).id();
    for (i, (label, color)) in [("X", (230, 90, 90)), ("Y", (90, 200, 90)), ("Z", (90, 130, 230))].into_iter().enumerate() {
        let read = move |e: &Emitter| match e.render.facing {
            Facing::FixedAxis(a) => a[i],
            _ => 0.0,
        };
        let dv = drag_value(commands, &fonts.ui, label, color, 0.0, 0.01);
        commands.entity(dv).entry::<Node>().and_modify(|mut n| {
            n.flex_grow = 1.0;
            n.flex_basis = Val::Px(0.0);
            n.min_width = Val::Px(36.0);
        });
        bind_2way(commands, dv, move |w: &Rx| em_get(w, read), move |w: &mut World, v: &f32| {
            let v = *v;
            em_set(w, read, v, |e| {
                if let Facing::FixedAxis(a) = &mut e.render.facing {
                    a[i] = v;
                }
            });
        });
        commands.entity(axis).add_child(dv);
    }
    let cols = num(commands, fonts, 1.0, Some((1.0, 64.0)), |w| em_get(w, |e| e.render.sprite_sheet.columns as f32), |w, v| {
        let c = v.round().clamp(1.0, 64.0) as u32;
        em_set(w, |e| e.render.sprite_sheet.columns, c, |e| e.render.sprite_sheet.columns = c);
    });
    let rows_f = num(commands, fonts, 1.0, Some((1.0, 64.0)), |w| em_get(w, |e| e.render.sprite_sheet.rows as f32), |w, v| {
        let r = v.round().clamp(1.0, 64.0) as u32;
        em_set(w, |e| e.render.sprite_sheet.rows, r, |e| e.render.sprite_sheet.rows = r);
    });
    let mode = dropdown(commands, fonts, &["Over Lifetime", "Fixed FPS"], 0);
    bind_2way(
        commands,
        mode,
        |w: &Rx| em_get(w, |e| usize::from(matches!(e.render.sprite_sheet.mode, SheetMode::Fps(_)))),
        |w: &mut World, v: &usize| {
            let is_fps = |e: &Emitter| usize::from(matches!(e.render.sprite_sheet.mode, SheetMode::Fps(_)));
            em_set(w, is_fps, *v, |e| {
                e.render.sprite_sheet.mode = if *v == 1 { SheetMode::Fps(12.0) } else { SheetMode::OverLifetime };
            });
        },
    );
    let fps_read = |e: &Emitter| match e.render.sprite_sheet.mode {
        SheetMode::Fps(f) => f,
        SheetMode::OverLifetime => 0.0,
    };
    let fps = num(commands, fonts, 0.5, Some((0.0, 240.0)), move |w| em_get(w, fps_read), move |w, v| {
        em_set(w, fps_read, v, |e| {
            if let SheetMode::Fps(f) = &mut e.render.sprite_sheet.mode {
                *f = v.max(0.0);
            }
        });
    });
    let z = num(commands, fonts, 0.05, None, |w| em_get(w, |e| e.render.z_order), |w, v| em_set(w, |e| e.render.z_order, v, |e| e.render.z_order = v));

    let axis_row = inspector_row(commands, &fonts.ui, "Axis", axis);
    bind_display(commands, axis_row, |w| !em_mesh(w) && em_get(w, |e| matches!(e.render.facing, Facing::FixedAxis(_))));
    let fps_row = inspector_row(commands, &fonts.ui, "Frames / Second", fps);
    bind_display(commands, fps_row, |w| em_get(w, |e| matches!(e.render.sprite_sheet.mode, SheetMode::Fps(_))));
    let z_row = inspector_row(commands, &fonts.ui, "Z Order", z);
    bind_display(commands, z_row, |w| doc_get(w, |d| d.dimension == Dimension::TwoD));
    let shading_labels: Vec<&str> = SHADINGS.iter().map(|(_, l)| *l).collect();
    let shading = dropdown(commands, fonts, &shading_labels, 0);
    bind_2way(
        commands,
        shading,
        |w: &Rx| em_get(w, |e| SHADINGS.iter().position(|(s, _)| *s == e.render.shading).unwrap_or(0)),
        |w: &mut World, v: &usize| {
            let s = SHADINGS.get(*v).map(|(s, _)| *s).unwrap_or_default();
            em_set(w, |e| e.render.shading, s, |e| e.render.shading = s);
        },
    );
    let stretch = num(commands, fonts, 0.005, Some((0.0, 2.0)), |w| em_get(w, |e| e.render.stretch), |w, v| {
        let v = v.max(0.0);
        em_set(w, |e| e.render.stretch, v, |e| e.render.stretch = v);
    });
    let erosion = num(commands, fonts, 0.01, Some((0.0, 1.0)), |w| em_get(w, |e| e.render.erosion), |w, v| {
        let v = v.clamp(0.0, 1.0);
        em_set(w, |e| e.render.erosion, v, |e| e.render.erosion = v);
    });
    let soft = num(commands, fonts, 0.01, Some((0.0, 10.0)), |w| em_get(w, |e| e.render.soft_distance), |w, v| {
        let v = v.max(0.0);
        em_set(w, |e| e.render.soft_distance, v, |e| e.render.soft_distance = v);
    });
    let frame_blend = check(commands, |w| em_get(w, |e| e.render.frame_blend), |w, v| {
        em_set(w, |e| e.render.frame_blend, v, |e| e.render.frame_blend = v)
    });

    // Rows that mean nothing in the current setup hide, like Axis does: a
    // stretch only applies to a velocity-aligned quad, frame blending only to
    // a sheet of more than one frame, soft particles only in 3D.
    // Stretch applies along the velocity: a velocity-aligned quad, or a mesh
    // oriented along it.
    let stretch_row = inspector_row(commands, &fonts.ui, "Stretch (s)", stretch);
    bind_display(commands, stretch_row, |w| {
        let dim = doc_get(w, |d| d.dimension);
        em_get(w, |e| e.render.effective_facing(dim) == Facing::VelocityAligned)
    });
    let blend_frames_row = inspector_row(commands, &fonts.ui, "Blend Frames", frame_blend);
    bind_display(commands, blend_frames_row, |w| em_get(w, |e| e.render.sprite_sheet.frame_count() > 1));
    // Soft fading is for blended particles; opaque and masked ones are solid.
    let soft_row = inspector_row(commands, &fonts.ui, "Soft Distance", soft);
    bind_display(commands, soft_row, |w| {
        doc_get(w, |d| d.dimension == Dimension::ThreeD) && em_get(w, |e| !e.render.blend.writes_depth())
    });
    // Shown wherever draw order changes the picture: blended (not additive,
    // not depth-written) particles that are not a ribbon. Every order but
    // Index costs a sort per frame, which is why it is a choice; View Depth
    // needs a depth to sort by, so a 2D effect draws it in index order.
    let order = dropdown(commands, fonts, &DRAW_ORDERS.map(|(_, l)| l), 0);
    bind_2way(
        commands,
        order,
        |w: &Rx| {
            em_get(w, |e| {
                // An old `sort: true` reads as View Depth, as the renderer
                // reads it.
                let o = if e.render.draw_order == DrawOrder::Index && e.render.sort { DrawOrder::ViewDepth } else { e.render.draw_order };
                DRAW_ORDERS.iter().position(|(d, _)| *d == o).unwrap_or(0)
            })
        },
        |w: &mut World, v: &usize| {
            let o = DRAW_ORDERS.get(*v).map(|(d, _)| *d).unwrap_or_default();
            em_set(w, |e| (e.render.draw_order, e.render.sort), (o, o == DrawOrder::ViewDepth), |e| e.render.set_draw_order(o));
        },
    );
    let sort_row = inspector_row(commands, &fonts.ui, "Draw Order", order);
    bind_display(commands, sort_row, |w| {
        let dim = doc_get(w, |d| d.dimension);
        em_get(w, |e| matches!(e.render.blend, BlendMode::Alpha | BlendMode::Premultiplied) && !e.render.draws_ribbon(dim))
    });
    let cutoff = num(commands, fonts, 0.01, Some((0.0, 1.0)), |w| em_get(w, |e| e.render.alpha_cutoff), |w, v| {
        let v = v.clamp(0.0, 1.0);
        em_set(w, |e| e.render.alpha_cutoff, v, |e| e.render.alpha_cutoff = v);
    });
    let cutoff_row = inspector_row(commands, &fonts.ui, "Alpha Cutoff", cutoff);
    // The cutoff is the mask's edge, the coverage edge, and where a blended
    // particle's shadow is cut when it casts one.
    bind_display(commands, cutoff_row, |w| {
        em_get(w, |e| {
            e.render.blend.uses_cutoff() || (e.render.shadows == ParticleShadows::On && !e.render.blend.writes_depth())
        })
    });
    let shadow_labels: Vec<&str> = SHADOWS.iter().map(|(_, l)| *l).collect();
    let shadows = dropdown(commands, fonts, &shadow_labels, 0);
    bind_2way(
        commands,
        shadows,
        |w: &Rx| em_get(w, |e| SHADOWS.iter().position(|(s, _)| *s == e.render.shadows).unwrap_or(0)),
        |w: &mut World, v: &usize| {
            let s = SHADOWS.get(*v).map(|(s, _)| *s).unwrap_or_default();
            em_set(w, |e| e.render.shadows, s, |e| e.render.shadows = s);
        },
    );
    // Shadow maps are 3D only.
    let shadows_row = inspector_row(commands, &fonts.ui, "Cast Shadows", shadows);
    bind_display(commands, shadows_row, |w| doc_get(w, |d| d.dimension == Dimension::ThreeD));
    let (method_row, method_note_row) = render_method_rows(commands, fonts);
    let screen = check(commands, |w| em_get(w, |e| e.render.screen_space_size), |w, v| {
        em_set(w, |e| e.render.screen_space_size, v, |e| e.render.screen_space_size = v)
    });
    let screen_row = inspector_row(commands, &fonts.ui, "Size in Pixels", screen);
    let facing_row = inspector_row(commands, &fonts.ui, "Facing", facing);
    // A mesh turns by Orient, so Facing is a quad's row only.
    bind_display(commands, facing_row, |w| !em_mesh(w));
    let mesh_rows = mesh_rows(commands, fonts);
    let lit_rows = lit_rows(commands, fonts);
    let ribbon_rows = ribbon_rows(commands, fonts);
    let mut rows = vec![inspector_row(commands, &fonts.ui, "Shading", shading)];
    rows.extend(mesh_rows);
    rows.extend([
        inspector_row(commands, &fonts.ui, "Texture", texture),
        inspector_row(commands, &fonts.ui, "Blend", blend),
        cutoff_row,
        shadows_row,
        method_row,
        method_note_row,
    ]);
    rows.extend(lit_rows);
    rows.extend([facing_row, axis_row, stretch_row, screen_row]);
    rows.extend(ribbon_rows);
    rows.extend([
        inspector_row(commands, &fonts.ui, "Sheet Columns", cols),
        inspector_row(commands, &fonts.ui, "Sheet Rows", rows_f),
        inspector_row(commands, &fonts.ui, "Sheet Mode", mode),
        fps_row,
        blend_frames_row,
        inspector_row(commands, &fonts.ui, "Erosion", erosion),
        soft_row,
        sort_row,
        z_row,
    ]);
    commands.entity(body).add_children(&rows);
    root
}

const RENDER_METHODS: [(RenderMethod, &str); 3] =
    [(RenderMethod::Auto, "Auto (Project)"), (RenderMethod::Forward, "Forward"), (RenderMethod::Deferred, "Deferred")];

const RENDER_METHOD_TIP: &str = "Which renderer solid particles draw through on a 3D camera. Auto follows the project, \
     so on a deferred project they write the G-buffer and are lit, reflected (SSR) and occluded (SSAO) like the scene. \
     Forward keeps them on the forward pass. Deferred asks for the G-buffer even on a forward project, and draws \
     forward while any camera has no deferred prepass.";

const TRANSPARENT_METHOD_TIP: &str = "Transparent blends always draw forward, after the deferred lighting: a G-buffer \
     holds one surface per pixel and a blend needs what is behind it. Pick Opaque, Alpha Mask or Alpha to Coverage \
     to choose a render method.";

/// The Render Method row, 3D and depth-written blends only, and in its place
/// for a transparent blend a fixed "Forward" with the reason, so the setting
/// does not vanish without saying why. `UiInteraction` is what makes a row
/// hoverable for its tooltip.
fn render_method_rows(commands: &mut Commands, fonts: &EmberFonts) -> (Entity, Entity) {
    let labels: Vec<&str> = RENDER_METHODS.iter().map(|(_, l)| *l).collect();
    let method = dropdown(commands, fonts, &labels, 0);
    bind_2way(
        commands,
        method,
        |w: &Rx| em_get(w, |e| RENDER_METHODS.iter().position(|(m, _)| *m == e.render.render_method).unwrap_or(0)),
        |w: &mut World, v: &usize| {
            let m = RENDER_METHODS.get(*v).map(|(m, _)| *m).unwrap_or_default();
            em_set(w, |e| e.render.render_method, m, |e| e.render.render_method = m);
        },
    );
    let row = inspector_row(commands, &fonts.ui, "Render Method", method);
    commands.entity(row).insert((UiInteraction::default(), HoverTooltip::new(RENDER_METHOD_TIP)));
    bind_display(commands, row, |w| doc_get(w, |d| d.dimension == Dimension::ThreeD) && em_get(w, |e| e.render.blend.writes_depth()));

    let note = commands
        .spawn((
            Text::new("Forward (transparent blend)"),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node { flex_grow: 1.0, flex_shrink: 1.0, ..default() },
        ))
        .id();
    let note_row = inspector_row(commands, &fonts.ui, "Render Method", note);
    commands.entity(note_row).insert((UiInteraction::default(), HoverTooltip::new(TRANSPARENT_METHOD_TIP)));
    bind_display(commands, note_row, |w| doc_get(w, |d| d.dimension == Dimension::ThreeD) && em_get(w, |e| !e.render.blend.writes_depth()));
    (row, note_row)
}

/// The mesh-particle rows: the shape (3D only, quads first), the model file
/// when the shape is Model, how the mesh is turned, and the project material
/// it may wear.
fn mesh_rows(commands: &mut Commands, fonts: &EmberFonts) -> Vec<Entity> {
    let labels = shape_labels();
    let shape = dropdown(commands, fonts, &labels, 0);
    bind_2way(
        commands,
        shape,
        |w: &Rx| em_get(w, |e| shape_index(&e.render.mesh)),
        |w: &mut World, v: &usize| {
            let v = *v;
            em_set(w, |e| shape_index(&e.render.mesh), v, |e| e.render.mesh = shape_at(v, &e.render.mesh));
        },
    );
    let model = path_slot(commands, fonts, Slot::Model);
    let orient = dropdown(commands, fonts, &["Tumble", "Along Velocity", "Fixed Axis"], 0);
    bind_2way(
        commands,
        orient,
        |w: &Rx| em_get(w, |e| orient_index(e.render.orient)),
        |w: &mut World, v: &usize| {
            em_set(w, |e| orient_index(e.render.orient), *v, |e| {
                e.render.orient = match v {
                    1 => MeshOrient::Velocity,
                    2 => MeshOrient::Axis([0.0, 1.0, 0.0]),
                    _ => MeshOrient::Tumble,
                }
            });
        },
    );
    let orient_axis = vec3_fields(
        commands,
        fonts,
        |e| match e.render.orient {
            MeshOrient::Axis(a) => a,
            _ => [0.0; 3],
        },
        |e, i, v| {
            if let MeshOrient::Axis(a) = &mut e.render.orient {
                a[i] = v;
            }
        },
    );
    let material = path_slot(commands, fonts, Slot::Material);

    let shape_row = inspector_row(commands, &fonts.ui, "Shape", shape);
    bind_display(commands, shape_row, |w| doc_get(w, |d| d.dimension == Dimension::ThreeD));
    let model_row = inspector_row(commands, &fonts.ui, "Model", model);
    bind_display(commands, model_row, |w| em_mesh(w) && em_get(w, |e| matches!(e.render.mesh, Some(ParticleMesh::Asset(_)))));
    let orient_row = inspector_row(commands, &fonts.ui, "Orient", orient);
    bind_display(commands, orient_row, em_mesh);
    let orient_axis_row = inspector_row(commands, &fonts.ui, "Orient Axis", orient_axis);
    bind_display(commands, orient_axis_row, |w| em_mesh(w) && em_get(w, |e| matches!(e.render.orient, MeshOrient::Axis(_))));
    // Any 3D particle wears a project material: quads, ribbons and meshes,
    // on either backend. 2D draws through its own pipeline, which a 3D
    // `.material` cannot run in.
    let material_row = inspector_row(commands, &fonts.ui, "Material", material);
    bind_display(commands, material_row, |w| doc_get(w, |d| d.dimension == Dimension::ThreeD));
    vec![shape_row, model_row, orient_row, orient_axis_row, material_row]
}

/// Scene lighting: the Lit switch (3D, not additive, whose colour is light
/// rather than a surface), and the surface it lights while on.
fn lit_rows(commands: &mut Commands, fonts: &EmberFonts) -> Vec<Entity> {
    let lit = check(commands, |w| em_get(w, |e| e.render.lighting.lit), |w, v| {
        em_set(w, |e| e.render.lighting.lit, v, |e| e.render.lighting.lit = v)
    });
    let roughness = num(commands, fonts, 0.01, Some((0.0, 1.0)), |w| em_get(w, |e| e.render.lighting.roughness), |w, v| {
        let v = v.clamp(0.0, 1.0);
        em_set(w, |e| e.render.lighting.roughness, v, |e| e.render.lighting.roughness = v);
    });
    let metallic = num(commands, fonts, 0.01, Some((0.0, 1.0)), |w| em_get(w, |e| e.render.lighting.metallic), |w, v| {
        let v = v.clamp(0.0, 1.0);
        em_set(w, |e| e.render.lighting.metallic, v, |e| e.render.lighting.metallic = v);
    });
    // A project material lights itself, so these rows are the particle
    // shader's. Additive light and a multiply filter are never lit.
    let can_light = |w: &Rx| {
        let dim = doc_get(w, |d| d.dimension);
        dim == Dimension::ThreeD
            && em_get(w, |e| {
                !matches!(e.render.blend, BlendMode::Additive | BlendMode::Multiply) && e.render.material_path(dim).is_none()
            })
    };
    let lit_row = inspector_row(commands, &fonts.ui, "Lit", lit);
    bind_display(commands, lit_row, can_light);
    let roughness_row = inspector_row(commands, &fonts.ui, "Roughness", roughness);
    bind_display(commands, roughness_row, move |w| can_light(w) && em_get(w, |e| e.render.lighting.lit));
    let metallic_row = inspector_row(commands, &fonts.ui, "Metallic", metallic);
    bind_display(commands, metallic_row, move |w| can_light(w) && em_get(w, |e| e.render.lighting.lit));
    vec![lit_row, roughness_row, metallic_row]
}

/// The Ribbon facing's rows, shown only while it is picked: what the strip
/// runs through, the trail's length and spacing (or the strip count), and the
/// taper to the tail.
fn ribbon_rows(commands: &mut Commands, fonts: &EmberFonts) -> Vec<Entity> {
    let kind = dropdown(commands, fonts, &["Trail", "Connect Particles"], 0);
    bind_2way(
        commands,
        kind,
        |w: &Rx| em_get(w, |e| usize::from(e.render.ribbon.kind == RibbonKind::Connect)),
        |w: &mut World, v: &usize| {
            let k = if *v == 1 { RibbonKind::Connect } else { RibbonKind::Trail };
            em_set(w, |e| e.render.ribbon.kind, k, |e| e.render.ribbon.kind = k);
        },
    );
    let points = num(commands, fonts, 1.0, Some((1.0, MAX_TRAIL_POINTS as f32)), |w| em_get(w, |e| e.render.ribbon.points as f32), |w, v| {
        let p = v.round().clamp(1.0, MAX_TRAIL_POINTS as f32) as u32;
        em_set(w, |e| e.render.ribbon.points, p, |e| e.render.ribbon.points = p);
    });
    let spacing = num(commands, fonts, 0.005, Some((0.0, 100.0)), |w| em_get(w, |e| e.render.ribbon.min_distance), |w, v| {
        let v = v.max(0.0);
        em_set(w, |e| e.render.ribbon.min_distance, v, |e| e.render.ribbon.min_distance = v);
    });
    let groups = num(commands, fonts, 1.0, Some((1.0, 64.0)), |w| em_get(w, |e| e.render.ribbon.groups as f32), |w, v| {
        let g = v.round().clamp(1.0, 64.0) as u32;
        em_set(w, |e| e.render.ribbon.groups, g, |e| e.render.ribbon.groups = g);
    });
    let tail_width = num(commands, fonts, 0.01, Some((0.0, 4.0)), |w| em_get(w, |e| e.render.ribbon.tail_width), |w, v| {
        let v = v.max(0.0);
        em_set(w, |e| e.render.ribbon.tail_width, v, |e| e.render.ribbon.tail_width = v);
    });
    let tail_alpha = num(commands, fonts, 0.01, Some((0.0, 1.0)), |w| em_get(w, |e| e.render.ribbon.tail_alpha), |w, v| {
        let v = v.clamp(0.0, 1.0);
        em_set(w, |e| e.render.ribbon.tail_alpha, v, |e| e.render.ribbon.tail_alpha = v);
    });
    let shape = dropdown(commands, fonts, &["Flat Strip", "Tube"], 0);
    bind_2way(
        commands,
        shape,
        |w: &Rx| em_get(w, |e| usize::from(e.render.ribbon.shape == RibbonShape::Tube)),
        |w: &mut World, v: &usize| {
            let s = if *v == 1 { RibbonShape::Tube } else { RibbonShape::Flat };
            em_set(w, |e| e.render.ribbon.shape, s, |e| e.render.ribbon.shape = s);
        },
    );
    let (lo, hi) = (MIN_TUBE_SEGMENTS as f32, MAX_TUBE_SEGMENTS as f32);
    let segments = num(commands, fonts, 1.0, Some((lo, hi)), |w| em_get(w, |e| e.render.ribbon.tube_segments() as f32), move |w, v| {
        let s = v.round().clamp(lo, hi) as u32;
        em_set(w, |e| e.render.ribbon.radial_segments, s, |e| e.render.ribbon.radial_segments = s);
    });
    let is_ribbon = |e: &Emitter| e.render.facing == Facing::Ribbon;
    let rows = [
        (inspector_row(commands, &fonts.ui, "Ribbon", kind), None),
        (inspector_row(commands, &fonts.ui, "Trail Points", points), Some(RibbonKind::Trail)),
        (inspector_row(commands, &fonts.ui, "Point Spacing", spacing), Some(RibbonKind::Trail)),
        (inspector_row(commands, &fonts.ui, "Strips", groups), Some(RibbonKind::Connect)),
        (inspector_row(commands, &fonts.ui, "Tail Width", tail_width), None),
        (inspector_row(commands, &fonts.ui, "Tail Alpha", tail_alpha), None),
    ];
    let mut out: Vec<Entity> = rows
        .into_iter()
        .map(|(row, only)| {
            // A mesh wins over the Ribbon facing, so its rows hide with it.
            bind_display(commands, row, move |w| {
                !em_mesh(w) && em_get(w, |e| is_ribbon(e) && only.is_none_or(|k| e.render.ribbon.kind == k))
            });
            row
        })
        .collect();
    // A tube is 3D geometry; a 2D effect draws every ribbon flat, so the
    // shape rows are 3D rows.
    let shape_row = inspector_row(commands, &fonts.ui, "Ribbon Shape", shape);
    bind_display(commands, shape_row, move |w| {
        !em_mesh(w) && doc_get(w, |d| d.dimension == Dimension::ThreeD) && em_get(w, is_ribbon)
    });
    let segments_row = inspector_row(commands, &fonts.ui, "Tube Segments", segments);
    bind_display(commands, segments_row, move |w| {
        !em_mesh(w)
            && doc_get(w, |d| d.dimension == Dimension::ThreeD)
            && em_get(w, |e| is_ribbon(e) && e.render.ribbon.shape == RibbonShape::Tube)
    });
    out.insert(1, shape_row);
    out.insert(2, segments_row);
    out
}

// ── Light ────────────────────────────────────────────────────────────────────

fn light_mode_index(m: LightMode) -> usize {
    match m {
        LightMode::Off => 0,
        LightMode::PerEmitter => 1,
        LightMode::BrightestN(_) => 2,
    }
}

/// The emitter's scene light: mode, then everything about the light, shown
/// only while it is on (and the 3D-only note while the effect is 2D).
fn light_section(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = section(commands, fonts, "lightbulb", "Light", (240, 190, 90));
    let mode = dropdown(commands, fonts, &["Off", "Per Emitter", "Brightest Particles"], 0);
    bind_2way(
        commands,
        mode,
        |w: &Rx| em_get(w, |e| light_mode_index(e.light.mode)),
        |w: &mut World, v: &usize| {
            em_set(w, |e| light_mode_index(e.light.mode), *v, |e| {
                e.light.mode = match v {
                    1 => LightMode::PerEmitter,
                    2 => LightMode::BrightestN(DEFAULT_PARTICLE_LIGHTS),
                    _ => LightMode::Off,
                }
            });
        },
    );
    let count_read = |e: &Emitter| match e.light.mode {
        LightMode::BrightestN(n) => n as f32,
        _ => 0.0,
    };
    let count = num(commands, fonts, 1.0, Some((1.0, MAX_PARTICLE_LIGHTS as f32)), move |w| em_get(w, count_read), move |w, v| {
        let n = v.round().clamp(1.0, MAX_PARTICLE_LIGHTS as f32) as u32;
        em_set(w, count_read, n as f32, |e| {
            if let LightMode::BrightestN(c) = &mut e.light.mode {
                *c = n;
            }
        });
    });
    let color = color_field_rgba(
        commands,
        |w| {
            let c = em_get(w, |e| e.light.color);
            linear_to_srgb([c[0], c[1], c[2], 1.0])
        },
        |w, c| {
            let l = srgb_to_linear(c);
            let rgb = [l[0], l[1], l[2]];
            em_set(w, |e| e.light.color, rgb, |e| e.light.color = rgb);
        },
    );
    let intensity = num(commands, fonts, 1.0, Some((0.0, 1.0e7)), |w| em_get(w, |e| e.light.intensity), |w, v| {
        let v = v.max(0.0);
        em_set(w, |e| e.light.intensity, v, |e| e.light.intensity = v);
    });
    let range = num(commands, fonts, 0.05, Some((0.0, 1000.0)), |w| em_get(w, |e| e.light.range), |w, v| {
        let v = v.max(0.0);
        em_set(w, |e| e.light.range, v, |e| e.light.range = v);
    });
    let radius = num(commands, fonts, 0.01, Some((0.0, 10.0)), |w| em_get(w, |e| e.light.radius), |w, v| {
        let v = v.max(0.0);
        em_set(w, |e| e.light.radius, v, |e| e.light.radius = v);
    });
    let shadows = check(commands, |w| em_get(w, |e| e.light.shadows), |w, v| em_set(w, |e| e.light.shadows, v, |e| e.light.shadows = v));
    let smoothing = num(commands, fonts, 0.01, Some((0.0, 5.0)), |w| em_get(w, |e| e.light.smoothing), |w, v| {
        let v = v.max(0.0);
        em_set(w, |e| e.light.smoothing, v, |e| e.light.smoothing = v);
    });
    let flicker = num(commands, fonts, 0.01, Some((0.0, 1.0)), |w| em_get(w, |e| e.light.flicker), |w, v| {
        let v = v.clamp(0.0, 1.0);
        em_set(w, |e| e.light.flicker, v, |e| e.light.flicker = v);
    });
    let rate = num(commands, fonts, 0.1, Some((0.0, 60.0)), |w| em_get(w, |e| e.light.flicker_rate), |w, v| {
        let v = v.max(0.0);
        em_set(w, |e| e.light.flicker_rate, v, |e| e.light.flicker_rate = v);
    });
    let offset = vec3_fields(commands, fonts, |e| e.light.offset, |e, i, v| e.light.offset[i] = v);

    let mode_row = inspector_row(commands, &fonts.ui, "Mode", mode);
    let note = commands
        .spawn((
            Text::new("Lights are 3D only; a 2D effect casts none."),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node { padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)), ..default() },
        ))
        .id();
    bind_display(commands, note, |w| doc_get(w, |d| d.dimension == Dimension::TwoD));
    let on = |e: &Emitter| e.light.mode != LightMode::Off;
    let rows: Vec<(Entity, fn(&Emitter) -> bool)> = vec![
        (inspector_row(commands, &fonts.ui, "Lights", count), |e| matches!(e.light.mode, LightMode::BrightestN(_))),
        (inspector_row(commands, &fonts.ui, "Color", color), on),
        (inspector_row(commands, &fonts.ui, "Lumens / Particle", intensity), on),
        (inspector_row(commands, &fonts.ui, "Range", range), on),
        (inspector_row(commands, &fonts.ui, "Radius", radius), on),
        (inspector_row(commands, &fonts.ui, "Shadows", shadows), on),
        (inspector_row(commands, &fonts.ui, "Smoothing (s)", smoothing), on),
        (inspector_row(commands, &fonts.ui, "Flicker", flicker), on),
        (inspector_row(commands, &fonts.ui, "Flicker Rate (Hz)", rate), |e| e.light.mode != LightMode::Off && e.light.flicker > 0.0),
        (inspector_row(commands, &fonts.ui, "Offset", offset), |e| e.light.mode == LightMode::PerEmitter),
    ];
    let mut kids = vec![mode_row, note];
    for (row, show) in rows {
        bind_display(commands, row, move |w| em_get(w, show));
        kids.push(row);
    }
    commands.entity(body).add_children(&kids);
    root
}

/// Three scrub fields for one Vec3 of the emitter, X Y Z coloured like the
/// Fixed Axis row.
fn vec3_fields(
    commands: &mut Commands,
    fonts: &EmberFonts,
    read: fn(&Emitter) -> [f32; 3],
    write: fn(&mut Emitter, usize, f32),
) -> Entity {
    let row = commands.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(3.0), flex_grow: 1.0, ..default() }).id();
    for (i, (label, color)) in [("X", (230, 90, 90)), ("Y", (90, 200, 90)), ("Z", (90, 130, 230))].into_iter().enumerate() {
        let dv = drag_value(commands, &fonts.ui, label, color, 0.0, 0.01);
        commands.entity(dv).entry::<Node>().and_modify(|mut n| {
            n.flex_grow = 1.0;
            n.flex_basis = Val::Px(0.0);
            n.min_width = Val::Px(36.0);
        });
        bind_2way(commands, dv, move |w: &Rx| em_get(w, |e| read(e)[i]), move |w: &mut World, v: &f32| {
            let v = *v;
            em_set(w, |e| read(e)[i], v, |e| write(e, i, v));
        });
        commands.entity(row).add_child(dv);
    }
    row
}

// ── Sub-emitters ─────────────────────────────────────────────────────────────

/// How many sub-emitter slots the panel shows. The file holds any number;
/// two covers the cases the effects people build use (a burst on death, a
/// trail of smoke), and each slot only appears once the one before is set.
const SUB_SLOTS: usize = 2;

fn trigger_index(t: Option<SubEmitterTrigger>) -> usize {
    match t {
        None => 0,
        Some(SubEmitterTrigger::OnBirth) => 1,
        Some(SubEmitterTrigger::OnDeath) => 2,
        Some(SubEmitterTrigger::Interval(_)) => 3,
    }
}

fn sub_get<T: Default>(w: &Rx, slot: usize, f: impl FnOnce(&SubEmitter) -> T) -> T {
    w.get_resource::<VfxEditorState>()
        .and_then(|s| s.emitter_index().and_then(|i| s.doc.emitters[i].sub_emitters.get(slot)))
        .map(f)
        .unwrap_or_default()
}

fn sub_set<T: PartialEq>(w: &mut World, slot: usize, get: impl Fn(&SubEmitter) -> T, value: T, set: impl FnOnce(&mut SubEmitter)) {
    let Some(st) = w.get_resource::<VfxEditorState>() else { return };
    let Some(i) = st.emitter_index() else { return };
    match st.doc.emitters[i].sub_emitters.get(slot) {
        Some(s) if get(s) != value => {}
        _ => return,
    }
    set(&mut w.resource_mut::<VfxEditorState>().doc.emitters[i].sub_emitters[slot]);
}

/// Set (or with `None`, remove) the trigger of sub-emitter `slot`. A new one
/// targets the first other emitter, so it does something the moment it is
/// switched on.
fn set_trigger(w: &mut World, slot: usize, trigger: Option<SubEmitterTrigger>) {
    let Some(st) = w.get_resource::<VfxEditorState>() else { return };
    let Some(i) = st.emitter_index() else { return };
    let subs = &st.doc.emitters[i].sub_emitters;
    let current = subs.get(slot).map(|s| s.trigger);
    if trigger_index(current) == trigger_index(trigger) {
        return;
    }
    let other = (0..st.doc.emitters.len()).find(|&k| k != i).unwrap_or(0);
    let len = subs.len();
    let mut st = w.resource_mut::<VfxEditorState>();
    let subs = &mut st.doc.emitters[i].sub_emitters;
    match trigger {
        None if slot < len => {
            subs.remove(slot);
        }
        None => {}
        Some(t) if slot < len => subs[slot].trigger = t,
        Some(t) => subs.push(SubEmitter { trigger: t, target: other, ..Default::default() }),
    }
}

fn sub_section(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = section(commands, fonts, "tree-structure", "Sub-Emitters", (120, 170, 220));
    let mut kids = Vec::new();
    for slot in 0..SUB_SLOTS {
        let trigger = dropdown(commands, fonts, &["None", "On Birth", "On Death", "Every Interval"], 0);
        bind_2way(
            commands,
            trigger,
            move |w: &Rx| trigger_index(sub_get(w, slot, |s| Some(s.trigger))),
            move |w: &mut World, v: &usize| {
                let t = match v {
                    1 => Some(SubEmitterTrigger::OnBirth),
                    2 => Some(SubEmitterTrigger::OnDeath),
                    3 => Some(SubEmitterTrigger::Interval(0.25)),
                    _ => None,
                };
                set_trigger(w, slot, t);
            },
        );
        let every_read = |s: &SubEmitter| match s.trigger {
            SubEmitterTrigger::Interval(t) => t,
            _ => 0.0,
        };
        let every = num(commands, fonts, 0.01, Some((0.01, 60.0)), move |w| sub_get(w, slot, every_read), move |w, v| {
            let v = v.max(0.01);
            sub_set(w, slot, every_read, v, |s| s.trigger = SubEmitterTrigger::Interval(v));
        });
        // The target by its tab number, with the tab's name beside it, since
        // emitters have names but the file (like a track) holds an index.
        let target = num(commands, fonts, 1.0, Some((1.0, 64.0)), move |w| sub_get(w, slot, |s| s.target as f32 + 1.0), move |w, v| {
            let n = w.get_resource::<VfxEditorState>().map_or(1, |s| s.doc.emitters.len());
            let t = (v.round().max(1.0) as usize - 1).min(n.saturating_sub(1));
            sub_set(w, slot, |s| s.target, t, |s| s.target = t);
        });
        let target_name = commands
            .spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), bevy::text::TextLayout::no_wrap()))
            .id();
        bind_with(
            commands,
            target_name,
            move |w| {
                let target: Option<usize> = sub_get(w, slot, |s| Some(s.target));
                w.get_resource::<VfxEditorState>()
                    .and_then(|s| target.and_then(|t| s.doc.emitters.get(t)).map(|e| e.name.clone()))
                    .unwrap_or_else(|| "(missing)".to_string())
            },
            |w, e, name: &String| {
                if let Some(mut t) = w.get_mut::<Text>(e) {
                    if t.0 != *name {
                        t.0 = name.clone();
                    }
                }
            },
        );
        let target_cell = commands
            .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), flex_grow: 1.0, ..default() })
            .id();
        commands.entity(target_cell).add_children(&[target, target_name]);
        let count = num(commands, fonts, 1.0, Some((1.0, 10_000.0)), move |w| sub_get(w, slot, |s| s.count as f32), move |w, v| {
            let c = v.round().max(1.0) as u32;
            sub_set(w, slot, |s| s.count, c, |s| s.count = c);
        });
        let inherit_v = num(commands, fonts, 0.01, Some((0.0, 1.0)), move |w| sub_get(w, slot, |s| s.inherit_velocity), move |w, v| {
            let v = v.clamp(0.0, 1.0);
            sub_set(w, slot, |s| s.inherit_velocity, v, |s| s.inherit_velocity = v);
        });
        let inherit_c = check(commands, move |w| sub_get(w, slot, |s| s.inherit_color), move |w, v| {
            sub_set(w, slot, |s| s.inherit_color, v, |s| s.inherit_color = v)
        });

        let label = if slot == 0 { "Trigger" } else { "Then Trigger" };
        let trigger_row = inspector_row(commands, &fonts.ui, label, trigger);
        // A slot appears once every slot before it is in use.
        bind_display(commands, trigger_row, move |w| em_get(w, |e| e.sub_emitters.len() >= slot));
        kids.push(trigger_row);
        let set_rows = [
            (inspector_row(commands, &fonts.ui, "Interval (s)", every), true),
            (inspector_row(commands, &fonts.ui, "Spawn Emitter", target_cell), false),
            (inspector_row(commands, &fonts.ui, "Count", count), false),
            (inspector_row(commands, &fonts.ui, "Inherit Velocity", inherit_v), false),
            (inspector_row(commands, &fonts.ui, "Inherit Color", inherit_c), false),
        ];
        for (row, interval_only) in set_rows {
            bind_display(commands, row, move |w| {
                sub_get(w, slot, |s| !interval_only || matches!(s.trigger, SubEmitterTrigger::Interval(_)))
            });
            kids.push(row);
        }
    }
    commands.entity(body).add_children(&kids);
    root
}

fn num<G, S>(commands: &mut Commands, fonts: &EmberFonts, step: f32, range: Option<(f32, f32)>, get: G, set: S) -> Entity
where
    G: Fn(&Rx) -> f32 + Send + Sync + 'static,
    S: Fn(&mut World, f32) + Send + Sync + 'static,
{
    let dv = drag_value(commands, &fonts.ui, "", value_text(), 0.0, step);
    if let Some((min, max)) = range {
        commands.entity(dv).insert(DragRange { min, max });
    }
    bind_2way(commands, dv, get, move |w: &mut World, v: &f32| set(w, *v));
    dv
}

fn check<G, S>(commands: &mut Commands, get: G, set: S) -> Entity
where
    G: Fn(&Rx) -> bool + Send + Sync + 'static,
    S: Fn(&mut World, bool) + Send + Sync + 'static,
{
    let cb = checkbox(commands, false);
    bind_2way(commands, cb, get, move |w: &mut World, v: &bool| set(w, *v));
    cb
}

// ── Asset slots ──────────────────────────────────────────────────────────────

/// Which file a path slot holds. One widget and one set of systems for all
/// three, since each is the same field: drop a file from the Assets panel,
/// click to browse, X to clear.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    /// The sprite texture.
    Texture,
    /// A mesh particle's model (`ParticleMesh::Asset`).
    Model,
    /// The project material the particles wear.
    Material,
}

const MODEL_EXTS: [&str; 3] = ["glb", "gltf", "rvg"];
const MATERIAL_EXTS: [&str; 1] = ["material"];

impl Slot {
    fn exts(self) -> &'static [&'static str] {
        match self {
            Slot::Texture => IMG_EXTS,
            Slot::Model => &MODEL_EXTS,
            Slot::Material => &MATERIAL_EXTS,
        }
    }

    fn filter_name(self) -> &'static str {
        match self {
            Slot::Texture => "Image",
            Slot::Model => "Model",
            Slot::Material => "Material",
        }
    }

    /// What the slot says while empty.
    fn empty_label(self) -> &'static str {
        match self {
            Slot::Texture => "Soft dot (drop an image or click)",
            Slot::Model => "Drop a .glb or .gltf, or click",
            Slot::Material => "Particle shader (drop a .material or click)",
        }
    }

    fn get(self, e: &Emitter) -> Option<String> {
        let path = match self {
            Slot::Texture => e.render.texture.clone(),
            Slot::Model => match &e.render.mesh {
                Some(ParticleMesh::Asset(p)) => Some(p.clone()),
                _ => None,
            },
            Slot::Material => e.render.material.clone(),
        };
        path.filter(|p| !p.trim().is_empty())
    }

    fn set(self, e: &mut Emitter, path: Option<String>) {
        match self {
            Slot::Texture => e.render.texture = path,
            // Clearing the model keeps the Model shape, waiting for a file.
            Slot::Model => e.render.mesh = Some(ParticleMesh::Asset(path.unwrap_or_default())),
            Slot::Material => e.render.material = path,
        }
    }
}

#[derive(Component)]
struct TexDropZone(Slot);

#[derive(Component)]
struct TexClearBtn(Slot);

/// The material editor's texture field, for one of the emitter's files: drop
/// it from the Assets panel, click to browse, X to clear it.
fn texture_slot(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    path_slot(commands, fonts, Slot::Texture)
}

fn path_slot(commands: &mut Commands, fonts: &EmberFonts, slot: Slot) -> Entity {
    let text = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), bevy::text::TextLayout::no_wrap(), PASS_THROUGH))
        .id();
    bind_with(
        commands,
        text,
        move |w| em_get(w, |e| slot.get(e)),
        move |w, e, tex: &Option<String>| {
            let (label, has) = match tex {
                Some(p) => (std::path::Path::new(p).file_name().and_then(|n| n.to_str()).unwrap_or(p).to_string(), true),
                None => (slot.empty_label().to_string(), false),
            };
            if let Some(mut t) = w.get_mut::<Text>(e) {
                if t.0 != label {
                    t.0 = label;
                }
            }
            if let Some(mut c) = w.get_mut::<TextColor>(e) {
                c.0 = rgb(if has { text_primary() } else { text_muted() });
            }
        },
    );
    let zone = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            UiInteraction::default(),
            RelativeCursorPosition::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            TexDropZone(slot),
            Name::new("vfx-path-drop"),
        ))
        .id();
    commands.entity(zone).add_child(text);
    let clear = commands
        .spawn((
            Text::new("\u{2715}"),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node { padding: UiRect::horizontal(Val::Px(3.0)), ..default() },
            UiInteraction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            TexClearBtn(slot),
        ))
        .id();
    let row = commands
        .spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), ..default() })
        .id();
    commands.entity(row).add_children(&[zone, clear]);
    row
}

fn set_path(state: &mut VfxEditorState, slot: Slot, path: Option<String>) {
    let Some(i) = state.emitter_index() else { return };
    let path = path.filter(|p| !p.trim().is_empty());
    if slot.get(&state.doc.emitters[i]) != path {
        slot.set(&mut state.doc.emitters[i], path);
    }
}

fn tex_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    payload: Option<Res<AssetDragPayload>>,
    project: Option<Res<renzora::core::CurrentProject>>,
    zones: Query<(&RelativeCursorPosition, &TexDropZone)>,
    state: Option<ResMut<VfxEditorState>>,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let (Some(payload), Some(mut state)) = (payload, state) else { return };
    if !payload.is_detached {
        return;
    }
    let Some(slot) = zones.iter().find(|(r, z)| r.cursor_over && payload.matches_extensions(z.0.exts())).map(|(_, z)| z.0) else {
        return;
    };
    let rel = project.as_ref().map(|p| p.make_asset_relative(&payload.path)).unwrap_or_else(|| payload.path.to_string_lossy().to_string());
    set_path(&mut state, slot, Some(rel));
}

fn tex_clear(q: Query<(&UiInteraction, &TexClearBtn), Changed<UiInteraction>>, state: Option<ResMut<VfxEditorState>>) {
    let Some(mut state) = state else { return };
    for (i, btn) in &q {
        if *i == UiInteraction::Pressed {
            set_path(&mut state, btn.0, None);
        }
    }
}

#[cfg_attr(target_arch = "wasm32", allow(unused_variables, unused_mut))]
fn tex_browse(
    q: Query<(&UiInteraction, &TexDropZone), Changed<UiInteraction>>,
    payload: Option<Res<AssetDragPayload>>,
    project: Option<Res<renzora::core::CurrentProject>>,
    state: Option<ResMut<VfxEditorState>>,
) {
    if payload.as_ref().is_some_and(|p| p.is_detached) {
        return;
    }
    let Some(slot) = q.iter().find(|(i, _)| **i == UiInteraction::Pressed).map(|(_, z)| z.0) else { return };
    let Some(mut state) = state else { return };
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(file) = rfd::FileDialog::new().add_filter(slot.filter_name(), slot.exts()).pick_file() {
        let rel = project.as_ref().map(|p| p.make_asset_relative(&file)).unwrap_or_else(|| file.to_string_lossy().to_string());
        set_path(&mut state, slot, Some(rel));
    }
}

/// Accent a slot's border while a file it takes hovers it.
fn tex_highlight(payload: Option<Res<AssetDragPayload>>, mut zones: Query<(&RelativeCursorPosition, &mut BorderColor, &TexDropZone)>) {
    for (rcp, mut bc, zone) in &mut zones {
        let active = payload.as_ref().is_some_and(|p| p.is_detached && rcp.cursor_over && p.matches_extensions(zone.0.exts()));
        let want = BorderColor::all(rgb(if active { accent() } else { border() }));
        if *bc != want {
            *bc = want;
        }
    }
}
