//! `vector="..."` — vector graphics (gauges, rings, charts, waveforms) drawn
//! with [vello](https://docs.rs/bevy_vello) into a [`UiVelloScene`] that
//! **bevy_ui lays out and positions** like any other markup element.
//!
//! ## Markup
//! ```html
//! <node vector="gauge" value="{{ Vehicle.speed }}" min="0" max="240"
//!       color="#4C8BF5" track="#1B2233" thickness="12"
//!       width="160px" height="160px" />
//! ```
//! Kinds:
//! - `gauge`  — 270° sweep arc + progress fill (a speedometer dial).
//! - `ring`   — full radial progress ring (`value`/`min`/`max`).
//! - `bar`    — vertical bar chart from `data="0.2,0.5,..."`.
//! - `line`   — line chart from `data="..."`.
//! - `waveform` — symmetric audio-style waveform from `data="..."`.
//!
//! Attributes: `value` (scalar kinds; literal or `{{ binding }}`), `min`/`max`
//! (range, defaults per kind), `color`/`track` (hex), `thickness` (stroke px),
//! `data` (comma-separated series for chart/waveform kinds).
//!
//! ## How it renders
//! `UiVelloScene` requires `ComputedNode`, so the element sits in the bevy_ui
//! layout and reserves space normally. We never draw a `BackgroundColor`; the
//! vello scene fills the node's box. Drawing is in the node's **local pixel
//! space** (origin top-left, y-down — vello's convention). A managed
//! `Camera2d` + `VelloView` (see [`sync_vello_camera`]) composites the vello
//! pass over whatever camera renders the world/UI.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, RenderTarget, Viewport};
use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy_vello::prelude::*;
use bevy_vello::vello::kurbo::{Affine, Arc, BezPath, Circle, Point, Rect, Stroke};
use bevy_vello::vello::{peniko, Scene};

/// Dedicated render layer for the vello canvas + UI vector scenes, isolating
/// the vello camera so it draws only the vello canvas (not stray sprites).
/// Engine layers in use: 0/1 (world+editor), 31 (viewport blit) — 5 is free.
pub const VECTOR_RENDER_LAYER: usize = 5;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VectorKind {
    Gauge,
    Ring,
    Bar,
    Line,
    Waveform,
}

impl VectorKind {
    fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "gauge" | "dial" | "speedometer" => Self::Gauge,
            "ring" | "radial" | "progress" => Self::Ring,
            "bar" | "bars" | "histogram" => Self::Bar,
            "line" | "chart" => Self::Line,
            "waveform" | "wave" => Self::Waveform,
            _ => return None,
        })
    }
    /// Default (min, max) range for this kind.
    fn default_range(self) -> (f32, f32) {
        match self {
            Self::Waveform => (-1.0, 1.0),
            _ => (0.0, 1.0),
        }
    }
}

/// Stamped from `vector="..."`; the [`update_vectors`] system rebuilds this
/// element's `UiVelloScene` from it each frame.
#[derive(Component, Clone)]
pub struct VectorSpec {
    pub kind: VectorKind,
    /// Data-source entity for `{{ }}` in `value` (the binding host).
    pub host: Entity,
    /// Scalar value: a literal number or a `{{ path }}` binding (gauge/ring).
    pub value: String,
    pub min: f32,
    pub max: f32,
    pub color: Color,
    pub track: Color,
    pub thickness: f32,
    /// Series data for chart/waveform kinds.
    pub data: Vec<f32>,
}

/// Build a [`VectorSpec`] from a markup node's attribute map (`node.defs`).
pub fn spec_from_defs(
    defs: &bevy::platform::collections::HashMap<String, String>,
    host: Entity,
) -> Option<VectorSpec> {
    let kind = VectorKind::parse(defs.get("vector")?)?;
    let (dmin, dmax) = kind.default_range();
    let hex = |k: &str, fallback: Color| {
        defs.get(k)
            .and_then(|v| crate::decor::parse_hex_color(v))
            .unwrap_or(fallback)
    };
    let data = defs
        .get("data")
        .map(|s| {
            s.split(',')
                .filter_map(|t| t.trim().parse::<f32>().ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Some(VectorSpec {
        kind,
        host,
        value: defs.get("value").cloned().unwrap_or_default(),
        min: defs.get("min").and_then(|s| s.trim().parse().ok()).unwrap_or(dmin),
        max: defs.get("max").and_then(|s| s.trim().parse().ok()).unwrap_or(dmax),
        color: hex("color", Color::srgb(0.30, 0.55, 0.96)),
        track: hex("track", Color::srgba(1.0, 1.0, 1.0, 0.10)),
        thickness: defs
            .get("thickness")
            .and_then(|s| s.trim().parse::<f32>().ok())
            .unwrap_or(10.0)
            .clamp(0.5, 256.0),
        data,
    })
}

// ── Drawing ──────────────────────────────────────────────────────────────

fn pen(c: Color) -> peniko::Color {
    let s = c.to_srgba();
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    peniko::Color::from_rgba8(q(s.red), q(s.green), q(s.blue), q(s.alpha))
}

fn frac(v: f32, spec: &VectorSpec) -> f64 {
    let d = spec.max - spec.min;
    if d.abs() < f32::EPSILON {
        return 0.0;
    }
    (((v - spec.min) / d).clamp(0.0, 1.0)) as f64
}

const DEG: f64 = std::f64::consts::PI / 180.0;

/// Resolve `value` (literal or `{{ path }}`) against the host's live ECS state.
fn resolve_scalar(world: &mut World, host: Entity, value: &str) -> Option<f32> {
    let t = value.trim();
    if let Some(inner) = t.strip_prefix("{{").and_then(|s| s.strip_suffix("}}")) {
        let s = crate::binding::read_path(world, host, inner.trim())?;
        s.trim().parse::<f32>().ok()
    } else {
        t.parse::<f32>().ok()
    }
}

/// Speedometer dial: a 270° track (gap at the bottom) with a progress fill.
/// In y-down space, 270° points up — so [135°, 405°] covers the top, gap at 90°.
fn draw_gauge(scene: &mut Scene, w: f64, h: f64, f: f64, spec: &VectorSpec) {
    let cx = w / 2.0;
    let cy = h * 0.56;
    let r = (w.min(h * 1.7) / 2.0 - spec.thickness as f64 / 2.0 - 2.0).max(1.0);
    let stroke = Stroke::new(spec.thickness as f64);
    let start = 135.0 * DEG;
    let sweep = 270.0 * DEG;
    scene.stroke(&stroke, Affine::IDENTITY, pen(spec.track), None,
        &Arc::new((cx, cy), (r, r), start, sweep, 0.0));
    if f > 0.0 {
        scene.stroke(&stroke, Affine::IDENTITY, pen(spec.color), None,
            &Arc::new((cx, cy), (r, r), start, sweep * f, 0.0));
    }
    scene.fill(peniko::Fill::NonZero, Affine::IDENTITY, pen(spec.color), None,
        &Circle::new((cx, cy), spec.thickness as f64 * 0.45));
}

/// Radial progress ring: full circle track + an arc from the top.
fn draw_ring(scene: &mut Scene, w: f64, h: f64, f: f64, spec: &VectorSpec) {
    let cx = w / 2.0;
    let cy = h / 2.0;
    let r = (w.min(h) / 2.0 - spec.thickness as f64 / 2.0 - 2.0).max(1.0);
    let stroke = Stroke::new(spec.thickness as f64);
    scene.stroke(&stroke, Affine::IDENTITY, pen(spec.track), None, &Circle::new((cx, cy), r));
    if f > 0.0 {
        // 270° in y-down = straight up; sweep clockwise by the fraction.
        scene.stroke(&stroke, Affine::IDENTITY, pen(spec.color), None,
            &Arc::new((cx, cy), (r, r), 270.0 * DEG, 360.0 * DEG * f, 0.0));
    }
}

/// Vertical bar chart. Each datum normalised by `min`/`max`.
fn draw_bars(scene: &mut Scene, w: f64, h: f64, spec: &VectorSpec) {
    let n = spec.data.len();
    if n == 0 {
        return;
    }
    let slot = w / n as f64;
    let bw = (slot * 0.66).max(1.0);
    for (i, &v) in spec.data.iter().enumerate() {
        let f = frac(v, spec);
        let bh = (f * (h - 2.0)).max(0.0);
        let x = i as f64 * slot + (slot - bw) / 2.0;
        let rect = Rect::new(x, h - bh, x + bw, h);
        scene.fill(peniko::Fill::NonZero, Affine::IDENTITY, pen(spec.color), None, &rect);
    }
}

/// Line chart through the normalised series.
fn draw_line(scene: &mut Scene, w: f64, h: f64, spec: &VectorSpec) {
    let n = spec.data.len();
    if n < 2 {
        return;
    }
    let mut path = BezPath::new();
    let pad = spec.thickness as f64;
    let pt = |i: usize, v: f32| -> Point {
        let x = i as f64 / (n - 1) as f64 * w;
        let y = h - pad - frac(v, spec) * (h - 2.0 * pad);
        Point::new(x, y)
    };
    path.move_to(pt(0, spec.data[0]));
    for (i, &v) in spec.data.iter().enumerate().skip(1) {
        path.line_to(pt(i, v));
    }
    scene.stroke(&Stroke::new(spec.thickness as f64), Affine::IDENTITY, pen(spec.color), None, &path);
}

/// Symmetric waveform: a vertical line per sample, mirrored about the centre.
fn draw_waveform(scene: &mut Scene, w: f64, h: f64, spec: &VectorSpec) {
    let n = spec.data.len();
    if n == 0 {
        return;
    }
    let cy = h / 2.0;
    let lw = (w / n as f64 * 0.5).clamp(1.0, spec.thickness as f64);
    let stroke = Stroke::new(lw);
    for (i, &v) in spec.data.iter().enumerate() {
        let f = frac(v, spec); // 0..1 magnitude
        let half = (f * (cy - 1.0)).max(0.5);
        let x = (i as f64 + 0.5) / n as f64 * w;
        let mut path = BezPath::new();
        path.move_to(Point::new(x, cy - half));
        path.line_to(Point::new(x, cy + half));
        scene.stroke(&stroke, Affine::IDENTITY, pen(spec.color), None, &path);
    }
}

fn build_scene(world: &mut World, spec: &VectorSpec, size: Vec2) -> Scene {
    let mut scene = Scene::new();
    let (w, h) = (size.x as f64, size.y as f64);
    if w < 1.0 || h < 1.0 {
        return scene;
    }
    match spec.kind {
        VectorKind::Gauge => {
            let v = resolve_scalar(world, spec.host, &spec.value).unwrap_or(spec.min);
            draw_gauge(&mut scene, w, h, frac(v, spec), spec);
        }
        VectorKind::Ring => {
            let v = resolve_scalar(world, spec.host, &spec.value).unwrap_or(spec.min);
            draw_ring(&mut scene, w, h, frac(v, spec), spec);
        }
        VectorKind::Bar => draw_bars(&mut scene, w, h, spec),
        VectorKind::Line => draw_line(&mut scene, w, h, spec),
        VectorKind::Waveform => draw_waveform(&mut scene, w, h, spec),
    }
    scene
}

/// Rebuild every vector element's `UiVelloScene` from its `VectorSpec` and live
/// bindings. Exclusive so it can reflect bound `{{ }}` values via `read_path`.
pub fn update_vectors(world: &mut World) {
    let mut q = world.query::<(Entity, &VectorSpec, &ComputedNode)>();
    // Draw in *logical* pixels: `ComputedNode::size()` is physical (= logical ×
    // combined scale factor, where combined = DPI × `UiScale`), and vello's UI
    // transform scales the scene back up by that same factor and centers using
    // the physical size. So our scene coords must be logical, else everything
    // renders at `1/scale` and shifts toward the origin.
    let snap: Vec<(Entity, VectorSpec, Vec2)> = q
        .iter(world)
        .map(|(e, s, n)| (e, s.clone(), n.size() * n.inverse_scale_factor()))
        .collect();
    if snap.is_empty() {
        return;
    }
    let mut built: Vec<(Entity, Scene)> = Vec::with_capacity(snap.len());
    for (e, spec, size) in snap {
        built.push((e, build_scene(world, &spec, size)));
    }
    for (e, scene) in built {
        if let Some(mut comp) = world.get_mut::<UiVelloScene>(e) {
            *comp = UiVelloScene::from(scene);
        }
    }
}

// ── Camera ───────────────────────────────────────────────────────────────

/// Marks the single managed `Camera2d` + `VelloView` that composites the vello
/// pass over the world/UI.
#[derive(Component)]
pub struct VelloUiCamera;

/// Keep one vello compositing camera that mirrors the render target of **the
/// camera the UI actually renders to**, drawing just above it
/// (`ClearColorConfig::None`) so the vector scenes land in the same target and
/// coordinate space as the bevy_ui nodes that position them.
///
/// Target selection (the one piece that depends on the host engine's camera
/// layout):
/// 1. If a `UiCanvas` carries a `UiTargetCamera` (editor edit/play mode → a
///    dedicated UI render image), mirror **that** camera.
/// 2. Otherwise (shipped runtime, no explicit target) mirror the active world
///    camera, which is the one bevy_ui auto-selects.
pub fn sync_vello_camera(
    mut commands: Commands,
    specs: Query<(), With<VectorSpec>>,
    ui_targets: Query<&bevy::ui::UiTargetCamera>,
    cameras: Query<(&Camera, &RenderTarget, Option<&Msaa>), Without<VelloUiCamera>>,
    world_cams: Query<
        (&Camera, &RenderTarget, Option<&Msaa>),
        (With<Camera3d>, Without<VelloUiCamera>),
    >,
    mut vello_cam: Query<
        (Entity, &mut Camera, &mut RenderTarget, Option<&Msaa>),
        With<VelloUiCamera>,
    >,
) {
    // No vector elements → don't spawn/maintain the extra pass.
    if specs.is_empty() {
        return;
    }

    // 1. Prefer the camera the UI canvas targets (editor edit/play mode).
    let mut chosen: Option<(isize, RenderTarget, Msaa, Option<UVec2>)> = ui_targets
        .iter()
        .next()
        .and_then(|tc| cameras.get(tc.entity()).ok())
        .map(|(cam, target, msaa)| {
            (cam.order, target.clone(), msaa.copied().unwrap_or_default(), cam.physical_target_size())
        });

    // 2. Fallback: the active world camera (shipped runtime).
    if chosen.is_none() {
        for (cam, target, msaa) in &world_cams {
            if !cam.is_active {
                continue;
            }
            if chosen.as_ref().map_or(true, |(o, _, _, _)| cam.order >= *o) {
                chosen = Some((
                    cam.order,
                    target.clone(),
                    msaa.copied().unwrap_or_default(),
                    cam.physical_target_size(),
                ));
            }
        }
    }
    let Some((src_order, src_target, src_msaa, src_size)) = chosen else {
        return;
    };
    let want_order = src_order + 1;
    // Pin the vello canvas to the target's size, else bevy_vello sizes it to the
    // window and squashes the canvas onto a smaller render-to-texture target.
    let want_viewport = src_size.map(|size| Viewport {
        physical_position: UVec2::ZERO,
        physical_size: size,
        ..default()
    });

    if let Ok((e, mut cam, mut target, cur_msaa)) = vello_cam.single_mut() {
        if cam.order != want_order {
            cam.order = want_order;
        }
        if cam.viewport.as_ref().map(|v| v.physical_size)
            != want_viewport.as_ref().map(|v| v.physical_size)
        {
            cam.viewport = want_viewport;
        }
        // `RenderTarget` has no `PartialEq`; compare via Debug to avoid
        // reassigning (and re-creating the vello rendertarget) every frame.
        if format!("{:?}", *target) != format!("{:?}", src_target) {
            *target = src_target;
        }
        // Sharing a target with the source camera requires matching MSAA, else
        // wgpu rejects the pass (differing sample counts).
        if cur_msaa.copied() != Some(src_msaa) {
            commands.entity(e).insert(src_msaa);
        }
    } else {
        commands.spawn((
            Camera2d,
            VelloView,
            Camera {
                order: want_order,
                clear_color: ClearColorConfig::None,
                viewport: want_viewport,
                ..default()
            },
            src_target,
            src_msaa,
            RenderLayers::layer(VECTOR_RENDER_LAYER),
            VelloUiCamera,
            renzora::HideInHierarchy,
            Name::new("Vello UI Camera"),
        ));
    }
}

/// Hide vello's internal entities (the canvas mesh, and our compositing camera)
/// from the editor hierarchy panel.
fn hide_vello_internals(
    mut commands: Commands,
    q: Query<(Entity, &Name), Without<renzora::HideInHierarchy>>,
) {
    for (e, name) in &q {
        if name.as_str() == "Vello Canvas" {
            commands.entity(e).insert(renzora::HideInHierarchy);
        }
    }
}

pub fn plugin(app: &mut App) {
    app.add_plugins(bevy_vello::VelloPlugin {
        canvas_render_layers: RenderLayers::layer(VECTOR_RENDER_LAYER),
        ..default()
    });
    app.add_systems(
        Update,
        (sync_vello_camera, hide_vello_internals, update_vectors),
    );
}
