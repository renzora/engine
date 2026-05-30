//! `vector="..."` — **composable** vector-graphics primitives drawn with
//! [vello](https://docs.rs/bevy_vello) into a [`UiVelloScene`] that bevy_ui lays
//! out and positions like any other element.
//!
//! These are the low-level building blocks; full widgets (speedometer, gauges,
//! charts) are assembled from them in markup component templates under
//! `assets/ui/components/` and reused via `template="..."`. Overlay several
//! absolutely-positioned primitives in one box to compose a dial.
//!
//! ## Primitives (`vector=`)
//! Radial (share `start`/`sweep` in degrees, `inset` px from the edge — set a
//! larger `inset` to nest a layer further in):
//! - `arc`    — stroked arc track + value fill (`value`/`min`/`max`).
//! - `ticks`  — `count`+1 radial tick marks (`len` px long).
//! - `labels` — `count`+1 numeric labels around the arc (`min`..`max`, `size`).
//! - `needle` — a pointer to `value`.
//!
//! Cartesian series (`data="0.2,0.5,..."`, literal or a `{{ path }}` binding to
//! a comma string):
//! - `bars` · `line` · `waveform`.
//!
//! Common attrs: `color`, `track`, `thickness`, `start` (deg, def 135),
//! `sweep` (deg, def 270), `inset` (px, def 2), `len` (px), `count`, `size`,
//! `min`/`max`. Radial widgets centre on the node; `start=135 sweep=270` is a
//! bottom-gap dial, `sweep=360` a full ring.
//!
//! ## How it renders
//! Drawing is in the node's **logical** pixel space (origin top-left, y-down).
//! A managed `Camera2d` + `VelloView` renders the scenes into an offscreen image
//! that a fullscreen `ImageNode` composites into the UI (see
//! [`manage_vello_overlay`]) — works on any target (window or render-to-texture).

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, RenderTarget, Viewport};
use bevy::prelude::*;
use bevy::ui::widget::NodeImageMode;
use bevy::ui::{ComputedNode, FocusPolicy, GlobalZIndex};
use bevy_vello::prelude::*;
use renzora_game_ui::UiCanvas;
use bevy_vello::vello::kurbo::{Affine, Arc, BezPath, Circle, Point, Rect, Stroke};
use bevy_vello::vello::peniko::{Blob, FontData};
use bevy_vello::vello::{peniko, Glyph, Scene};
use skrifa::instance::{LocationRef, Size as FontSize};
use skrifa::{FontRef, MetadataProvider};

/// OpenSans, embedded for in-scene text (dial number labels).
const FONT_BYTES: &[u8] = include_bytes!("../embedded/OpenSans-Regular.ttf");

/// Dedicated render layer for the vello canvas + UI vector scenes, isolating
/// the vello camera so it draws only the vello canvas (not stray sprites).
/// Engine layers in use: 0/1 (world+editor), 31 (viewport blit) — 5 is free.
pub const VECTOR_RENDER_LAYER: usize = 5;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VectorKind {
    Arc,
    Ticks,
    Labels,
    Needle,
    Bars,
    Line,
    Wave,
    /// Full dial drawn as ONE node: arc + ticks + labels + needle + centre
    /// readout. Composing these as separate absolutely-positioned overlay
    /// nodes is unreliable in taffy (childless vello nodes collapse), so the
    /// whole widget is a single sized node instead.
    Speedometer,
}

impl VectorKind {
    fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "arc" | "gauge" | "ring" => Self::Arc,
            "ticks" | "scale" => Self::Ticks,
            "labels" | "arclabels" | "numbers" => Self::Labels,
            "needle" | "pointer" => Self::Needle,
            "bars" | "bar" => Self::Bars,
            "line" | "chart" => Self::Line,
            "wave" | "waveform" => Self::Wave,
            "speedometer" | "dial" => Self::Speedometer,
            _ => return None,
        })
    }
    /// Default (min, max) range for this kind.
    fn default_range(self) -> (f32, f32) {
        match self {
            Self::Wave => (-1.0, 1.0),
            _ => (0.0, 1.0),
        }
    }
}

/// Stamped from `vector="..."`; the [`update_vectors`] system rebuilds this
/// element's `UiVelloScene` from it each frame.
#[derive(Component, Clone)]
pub struct VectorSpec {
    pub kind: VectorKind,
    /// Data-source entity for `{{ }}` (the binding host).
    pub host: Entity,
    /// Scalar value: a literal number or a `{{ path }}` binding (arc/needle).
    pub value: String,
    pub min: f32,
    pub max: f32,
    pub color: Color,
    pub track: Color,
    /// Filled disc behind an arc/dial face (transparent = none).
    pub fill: Color,
    pub thickness: f32,
    /// Raw `data` attribute (literal `0.2,0.5,...` or `{{ path }}` → comma
    /// string) for bars/line/waveform; resolved each frame.
    pub data: String,
    /// Number of divisions (`count`+1 ticks/labels).
    pub count: u32,
    /// Arc start angle, degrees (y-down: 270 = up).
    pub start: f32,
    /// Arc sweep, degrees (270 = bottom-gap dial, 360 = full ring).
    pub sweep: f32,
    /// Radius inset from the node edge, px (nest layers by increasing this).
    pub inset: f32,
    /// Tick length, px.
    pub len: f32,
    /// Label font size, px (0 = auto from radius).
    pub size: f32,
    /// Colour for ticks + numeric labels (dial composites).
    pub tickcolor: Color,
    /// Optional centre readout text — a literal or `{{ path }}` binding.
    /// `arc`/`speedometer` draw it centred so a gauge needs no overlay node.
    pub readout: Option<String>,
    /// Small unit/caption line under the readout (e.g. `km/h`). Empty = none.
    pub unit: String,
    /// Readout font size, px (0 = auto from radius).
    pub readsize: f32,
}

/// Build a [`VectorSpec`] from a markup node's attribute map.
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
    let f = |k: &str, d: f32| defs.get(k).and_then(|s| s.trim().parse::<f32>().ok()).unwrap_or(d);
    Some(VectorSpec {
        kind,
        host,
        value: defs.get("value").cloned().unwrap_or_default(),
        min: f("min", dmin),
        max: f("max", dmax),
        color: hex("color", Color::srgb(0.30, 0.55, 0.96)),
        track: hex("track", Color::srgba(1.0, 1.0, 1.0, 0.10)),
        fill: hex("fill", Color::NONE),
        thickness: f("thickness", 10.0).clamp(0.5, 256.0),
        data: defs.get("data").cloned().unwrap_or_default(),
        count: defs
            .get("count")
            .or_else(|| defs.get("ticks"))
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(8)
            .clamp(1, 200),
        start: f("start", 135.0),
        sweep: f("sweep", 270.0),
        inset: f("inset", 2.0).max(0.0),
        len: f("len", 10.0).max(0.0),
        size: f("size", 0.0).max(0.0),
        tickcolor: hex("tickcolor", Color::srgb(0.54, 0.58, 0.65)),
        readout: defs.get("readout").filter(|s| !s.trim().is_empty()).cloned(),
        unit: defs.get("unit").cloned().unwrap_or_default(),
        readsize: f("readsize", 0.0).max(0.0),
    })
}

/// Embedded font, parsed once for in-scene label text.
#[derive(Resource)]
pub struct VectorFont(FontData);

impl FromWorld for VectorFont {
    fn from_world(_: &mut World) -> Self {
        VectorFont(FontData::new(Blob::from(FONT_BYTES.to_vec()), 0))
    }
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

/// Resolve a length that's a fraction of the half-extent when ≤ 1, else px.
/// Lets component templates stay size-independent (`inset="0.25"`).
fn px_or_frac(v: f32, half: f64) -> f64 {
    if v <= 1.0 {
        v as f64 * half
    } else {
        v as f64
    }
}

/// Shared radial geometry: centre + outer radius (after `inset`).
fn geom(w: f64, h: f64, spec: &VectorSpec) -> (f64, f64, f64) {
    let half = w.min(h) / 2.0;
    let r = (half - px_or_frac(spec.inset, half)).max(1.0);
    (w / 2.0, h / 2.0, r)
}

/// Resolve `value` (literal or `{{ path }}`) against the host's live ECS state.
fn resolve_scalar(world: &mut World, host: Entity, value: &str) -> Option<f32> {
    let t = value.trim();
    if let Some(inner) = t.strip_prefix("{{").and_then(|s| s.strip_suffix("}}")) {
        crate::binding::read_path(world, host, inner.trim())?.trim().parse::<f32>().ok()
    } else {
        t.parse::<f32>().ok()
    }
}

/// Resolve `readout` text — a literal (`FUEL`) or a `{{ path }}` binding
/// (`{{ Car.speed }}` → `"87"`) — to the string to draw in the dial centre.
fn resolve_text(world: &mut World, host: Entity, raw: &str) -> String {
    let t = raw.trim();
    if let Some(inner) = t.strip_prefix("{{").and_then(|s| s.strip_suffix("}}")) {
        crate::binding::read_path(world, host, inner.trim())
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    } else {
        t.to_string()
    }
}

/// Resolve `data` (literal `0.2,0.5` or `{{ path }}` → comma string) to a series.
fn resolve_series(world: &mut World, host: Entity, raw: &str) -> Vec<f32> {
    let s = raw.trim();
    let text = if let Some(inner) = s.strip_prefix("{{").and_then(|x| x.strip_suffix("}}")) {
        crate::binding::read_path(world, host, inner.trim()).unwrap_or_default()
    } else {
        s.to_string()
    };
    text.split(',').filter_map(|t| t.trim().parse::<f32>().ok()).collect()
}

/// Horizontal extent of `text` at `size`, in logical px.
fn text_width(font: &FontRef, text: &str, size: f32) -> f32 {
    let metrics = font.glyph_metrics(FontSize::new(size), LocationRef::default());
    let charmap = font.charmap();
    text.chars()
        .map(|c| charmap.map(c).and_then(|g| metrics.advance_width(g)).unwrap_or(0.0))
        .sum()
}

/// Draw `text` with baseline-left at `(x, y)`. Maps chars → glyphs via skrifa.
fn draw_text(scene: &mut Scene, font: &FontData, text: &str, x: f64, y: f64, size: f32, color: Color) {
    let Ok(font_ref) = FontRef::new(FONT_BYTES) else {
        return;
    };
    let metrics = font_ref.glyph_metrics(FontSize::new(size), LocationRef::default());
    let charmap = font_ref.charmap();
    let mut pen_x = x as f32;
    let by = y as f32;
    let glyphs: Vec<Glyph> = text
        .chars()
        .filter_map(|c| {
            let gid = charmap.map(c)?;
            let g = Glyph { id: gid.to_u32(), x: pen_x, y: by };
            pen_x += metrics.advance_width(gid).unwrap_or(0.0);
            Some(g)
        })
        .collect();
    scene
        .draw_glyphs(font)
        .font_size(size)
        .brush(pen(color))
        .transform(Affine::IDENTITY)
        .draw(peniko::Fill::NonZero, glyphs.into_iter());
}

/// Draw `text` horizontally centred on `cx`, vertically centred on `cy`.
fn draw_text_centered(scene: &mut Scene, font: &FontData, text: &str, cx: f64, cy: f64, size: f32, color: Color) {
    let Ok(font_ref) = FontRef::new(FONT_BYTES) else {
        return;
    };
    let w = text_width(&font_ref, text, size) as f64;
    draw_text(scene, font, text, cx - w / 2.0, cy + size as f64 * 0.34, size, color);
}

/// Format a tick value: integer when whole, else one decimal.
fn fmt_num(v: f32) -> String {
    if (v.round() - v).abs() < 0.05 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.1}")
    }
}

/// Stroked arc: a `track` band over the full sweep, the `color` band up to `f`.
/// A full sweep (≥360°) draws a clean circle for the track.
fn draw_arc(scene: &mut Scene, w: f64, h: f64, f: f64, spec: &VectorSpec) {
    let (cx, cy, r0) = geom(w, h, spec);
    let r = (r0 - spec.thickness as f64 / 2.0).max(1.0);
    // Filled dial face behind the ring (skipped when transparent).
    if spec.fill.alpha() > 0.0 {
        scene.fill(peniko::Fill::NonZero, Affine::IDENTITY, pen(spec.fill), None,
            &Circle::new((cx, cy), r0));
    }
    let stroke = Stroke::new(spec.thickness as f64);
    let start = spec.start as f64 * DEG;
    let sweep = spec.sweep as f64 * DEG;
    if spec.sweep >= 359.5 {
        scene.stroke(&stroke, Affine::IDENTITY, pen(spec.track), None, &Circle::new((cx, cy), r));
    } else {
        scene.stroke(&stroke, Affine::IDENTITY, pen(spec.track), None,
            &Arc::new((cx, cy), (r, r), start, sweep, 0.0));
    }
    if f > 0.0 {
        scene.stroke(&stroke, Affine::IDENTITY, pen(spec.color), None,
            &Arc::new((cx, cy), (r, r), start, sweep * f, 0.0));
    }
}

/// `count`+1 radial tick marks, `len` px long, just inside the radius.
fn draw_ticks(scene: &mut Scene, w: f64, h: f64, spec: &VectorSpec) {
    let (cx, cy, r) = geom(w, h, spec);
    let inner = (r - px_or_frac(spec.len, w.min(h) / 2.0)).max(0.0);
    let stroke = Stroke::new(spec.thickness.max(1.0) as f64);
    let start = spec.start as f64 * DEG;
    let sweep = spec.sweep as f64 * DEG;
    for i in 0..=spec.count {
        let a = start + sweep * (i as f64 / spec.count as f64);
        let (ca, sa) = (a.cos(), a.sin());
        let mut p = BezPath::new();
        p.move_to((cx + inner * ca, cy + inner * sa));
        p.line_to((cx + r * ca, cy + r * sa));
        scene.stroke(&stroke, Affine::IDENTITY, pen(spec.color), None, &p);
    }
}

/// `count`+1 numeric labels (`min`..`max`) around the arc at the radius.
fn draw_labels(scene: &mut Scene, font: &FontData, w: f64, h: f64, spec: &VectorSpec) {
    let (cx, cy, r) = geom(w, h, spec);
    let size = if spec.size > 0.0 { spec.size } else { (r as f32 * 0.13).clamp(8.0, 22.0) };
    let start = spec.start as f64 * DEG;
    let sweep = spec.sweep as f64 * DEG;
    for i in 0..=spec.count {
        let t = i as f64 / spec.count as f64;
        let a = start + sweep * t;
        let v = spec.min + (spec.max - spec.min) * t as f32;
        draw_text_centered(scene, font, &fmt_num(v), cx + r * a.cos(), cy + r * a.sin(), size, spec.color);
    }
}

/// A tapered needle from the centre to `value`, plus a hub.
fn draw_needle(scene: &mut Scene, w: f64, h: f64, f: f64, spec: &VectorSpec) {
    let (cx, cy, r) = geom(w, h, spec);
    let a = spec.start as f64 * DEG + spec.sweep as f64 * DEG * f;
    let (nc, ns) = (a.cos(), a.sin());
    let half = std::f64::consts::FRAC_PI_2;
    let (pc, ps) = ((a + half).cos(), (a + half).sin());
    let base = spec.thickness as f64 * 0.6 + 3.0;
    let mut needle = BezPath::new();
    needle.move_to((cx + r * nc, cy + r * ns));
    needle.line_to((cx + base * pc, cy + base * ps));
    needle.line_to((cx - base * pc, cy - base * ps));
    needle.close_path();
    scene.fill(peniko::Fill::NonZero, Affine::IDENTITY, pen(spec.color), None, &needle);
    scene.fill(peniko::Fill::NonZero, Affine::IDENTITY,
        pen(Color::srgb(0.92, 0.94, 0.98)), None, &Circle::new((cx, cy), base * 0.85));
}

/// Centre readout: a big value line, optional small unit line below it.
/// `dy` nudges the block down from centre (px) so it sits in a dial's gap.
#[allow(clippy::too_many_arguments)]
fn draw_readout(
    scene: &mut Scene,
    font: &FontData,
    w: f64,
    h: f64,
    value: &str,
    unit: &str,
    readsize: f32,
    dy: f64,
    spec: &VectorSpec,
) {
    let half = w.min(h) / 2.0;
    let size = if readsize > 0.0 { readsize } else { (half as f32 * 0.30).clamp(12.0, 48.0) };
    let cx = w / 2.0;
    let cy = h / 2.0 + dy;
    if !value.is_empty() {
        draw_text_centered(scene, font, value, cx, cy, size, Color::WHITE);
    }
    if !unit.is_empty() {
        let usize = (size * 0.34).clamp(9.0, 16.0);
        draw_text_centered(scene, font, unit, cx, cy + size as f64 * 0.85, usize, spec.tickcolor);
    }
}

/// Full dial composited into ONE scene: arc track + value fill, tick marks,
/// numeric labels, needle, and the centre readout. Drawn by cloning `spec`
/// with a per-layer `inset`/`thickness`/`color`, reusing the primitive draws.
fn draw_speedometer(
    scene: &mut Scene,
    font: &FontData,
    w: f64,
    h: f64,
    value: f32,
    readout: &str,
    spec: &VectorSpec,
) {
    let f = frac(value, spec);
    // Arc track + fill (outermost).
    {
        let mut s = spec.clone();
        s.inset = 0.05;
        draw_arc(scene, w, h, f, &s);
    }
    // Tick marks, just inside the arc.
    {
        let mut s = spec.clone();
        s.inset = 0.27;
        s.len = 0.05;
        s.thickness = 2.0;
        s.color = spec.tickcolor;
        draw_ticks(scene, w, h, &s);
    }
    // Numeric labels, further in.
    {
        let mut s = spec.clone();
        s.inset = 0.42;
        s.color = spec.tickcolor;
        draw_labels(scene, font, w, h, &s);
    }
    // Needle from the centre.
    {
        let mut s = spec.clone();
        s.inset = 0.14;
        draw_needle(scene, w, h, f, &s);
    }
    // Centre readout, nudged into the lower bottom-gap of the 270° dial.
    if !readout.is_empty() {
        let dy = (w.min(h) / 2.0) * 0.34;
        draw_readout(scene, font, w, h, readout, &spec.unit, spec.readsize, dy, spec);
    }
}

/// Vertical bar chart. Each datum normalised by `min`/`max`.
fn draw_bars(scene: &mut Scene, w: f64, h: f64, data: &[f32], spec: &VectorSpec) {
    let n = data.len();
    if n == 0 {
        return;
    }
    let slot = w / n as f64;
    let bw = (slot * 0.66).max(1.0);
    for (i, &v) in data.iter().enumerate() {
        let bh = (frac(v, spec) * (h - 2.0)).max(0.0);
        let x = i as f64 * slot + (slot - bw) / 2.0;
        let rect = Rect::new(x, h - bh, x + bw, h);
        scene.fill(peniko::Fill::NonZero, Affine::IDENTITY, pen(spec.color), None, &rect);
    }
}

/// Line chart through the normalised series.
fn draw_line(scene: &mut Scene, w: f64, h: f64, data: &[f32], spec: &VectorSpec) {
    let n = data.len();
    if n < 2 {
        return;
    }
    let mut path = BezPath::new();
    let pad = spec.thickness as f64;
    let pt = |i: usize, v: f32| -> Point {
        Point::new(
            i as f64 / (n - 1) as f64 * w,
            h - pad - frac(v, spec) * (h - 2.0 * pad),
        )
    };
    path.move_to(pt(0, data[0]));
    for (i, &v) in data.iter().enumerate().skip(1) {
        path.line_to(pt(i, v));
    }
    scene.stroke(&Stroke::new(spec.thickness as f64), Affine::IDENTITY, pen(spec.color), None, &path);
}

/// Symmetric waveform: a vertical line per sample, mirrored about the centre.
fn draw_waveform(scene: &mut Scene, w: f64, h: f64, data: &[f32], spec: &VectorSpec) {
    let n = data.len();
    if n == 0 {
        return;
    }
    let cy = h / 2.0;
    let lw = (w / n as f64 * 0.5).clamp(1.0, spec.thickness as f64);
    let stroke = Stroke::new(lw);
    for (i, &v) in data.iter().enumerate() {
        let half = (frac(v, spec) * (cy - 1.0)).max(0.5);
        let x = (i as f64 + 0.5) / n as f64 * w;
        let mut path = BezPath::new();
        path.move_to(Point::new(x, cy - half));
        path.line_to(Point::new(x, cy + half));
        scene.stroke(&stroke, Affine::IDENTITY, pen(spec.color), None, &path);
    }
}

fn build_scene(world: &mut World, font: &FontData, spec: &VectorSpec, size: Vec2) -> Scene {
    let mut scene = Scene::new();
    let (w, h) = (size.x as f64, size.y as f64);
    if w < 1.0 || h < 1.0 {
        return scene;
    }
    let scalar = |world: &mut World| resolve_scalar(world, spec.host, &spec.value).unwrap_or(spec.min);
    match spec.kind {
        VectorKind::Arc => {
            let v = scalar(world);
            draw_arc(&mut scene, w, h, frac(v, spec), spec);
            // Optional centre readout so a gauge/ring needs no overlay node.
            if let Some(raw) = spec.readout.clone() {
                let text = resolve_text(world, spec.host, &raw);
                if !text.is_empty() {
                    draw_readout(&mut scene, font, w, h, &text, &spec.unit, spec.readsize, 0.0, spec);
                }
            }
        }
        VectorKind::Speedometer => {
            let v = scalar(world);
            let text = spec
                .readout
                .clone()
                .map(|raw| resolve_text(world, spec.host, &raw))
                .unwrap_or_default();
            draw_speedometer(&mut scene, font, w, h, v, &text, spec);
        }
        VectorKind::Needle => {
            let v = scalar(world);
            draw_needle(&mut scene, w, h, frac(v, spec), spec);
        }
        VectorKind::Ticks => draw_ticks(&mut scene, w, h, spec),
        VectorKind::Labels => draw_labels(&mut scene, font, w, h, spec),
        VectorKind::Bars => {
            let data = resolve_series(world, spec.host, &spec.data);
            draw_bars(&mut scene, w, h, &data, spec);
        }
        VectorKind::Line => {
            let data = resolve_series(world, spec.host, &spec.data);
            draw_line(&mut scene, w, h, &data, spec);
        }
        VectorKind::Wave => {
            let data = resolve_series(world, spec.host, &spec.data);
            draw_waveform(&mut scene, w, h, &data, spec);
        }
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
    let font = world.resource::<VectorFont>().0.clone();
    let mut built: Vec<(Entity, Scene)> = Vec::with_capacity(snap.len());
    for (e, spec, size) in snap {
        built.push((e, build_scene(world, &font, &spec, size)));
    }
    for (e, scene) in built {
        if let Some(mut comp) = world.get_mut::<UiVelloScene>(e) {
            *comp = UiVelloScene::from(scene);
        }
    }
}

// ── Overlay (render-to-image) ──────────────────────────────────────────────
//
// bevy_vello renders the UI scenes into its canvas, which a `Camera2d` +
// `VelloView` then draws to that camera's target. Compositing that camera *over
// a window* (a 2D overlay on the 3D/UI camera) does not reliably blend onto the
// swapchain — it only worked when the target was a render-to-texture image
// (the editor preview). So the vello camera instead renders to its **own
// transparent image**, and a fullscreen bevy_ui `ImageNode` displays that image.
// That composites through normal UI rendering on any target (window or image)
// and is automatically scale-correct: bevy_ui renders the node at physical
// resolution, 1:1 with the physical-sized overlay image.

/// The managed `Camera2d` + `VelloView` that renders vector scenes into [`VelloOverlay`].
#[derive(Component)]
pub struct VelloUiCamera;

/// The fullscreen UI image (one per `UiCanvas`) that shows the vello overlay.
#[derive(Component)]
pub struct VelloOverlayNode;

/// Shared render-target image the vello camera draws into, sized to the UI
/// target's physical pixels.
#[derive(Resource, Default)]
pub struct VelloOverlay {
    image: Handle<Image>,
    size: UVec2,
}

fn make_overlay_image(size: UVec2) -> Image {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
    let mut img = Image::new_fill(
        Extent3d { width: size.x.max(1), height: size.y.max(1), depth_or_array_layers: 1 },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    img.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    img
}

fn overlay_image_node(image: Handle<Image>) -> ImageNode {
    ImageNode {
        image,
        image_mode: NodeImageMode::Stretch,
        ..default()
    }
}

/// Maintain the vello overlay: keep a transparent render-target image sized to
/// the UI target, point the vello camera at it, and ensure each `UiCanvas`
/// displays it via a fullscreen, click-through `ImageNode`.
#[allow(clippy::too_many_arguments)]
pub fn manage_vello_overlay(
    mut commands: Commands,
    specs: Query<(), With<VectorSpec>>,
    ui_targets: Query<&bevy::ui::UiTargetCamera>,
    cameras: Query<&Camera, Without<VelloUiCamera>>,
    world_cams: Query<&Camera, (With<Camera3d>, Without<VelloUiCamera>)>,
    mut overlay: ResMut<VelloOverlay>,
    mut images: ResMut<Assets<Image>>,
    vello_cam: Query<Entity, With<VelloUiCamera>>,
    canvases: Query<Entity, With<UiCanvas>>,
    overlay_nodes: Query<(Entity, &ChildOf), With<VelloOverlayNode>>,
) {
    if specs.is_empty() {
        return;
    }

    // Physical size of the camera the UI renders to, so scene positions (in the
    // UI's physical px) land 1:1 in the overlay image.
    let size = ui_targets
        .iter()
        .next()
        .and_then(|tc| cameras.get(tc.entity()).ok())
        .and_then(|c| c.physical_target_size())
        .or_else(|| {
            world_cams
                .iter()
                .filter(|c| c.is_active)
                .find_map(|c| c.physical_target_size())
        });
    let Some(size) = size else {
        return;
    };
    if size.x == 0 || size.y == 0 {
        return;
    }

    // (Re)create the image when the target size changes.
    let resized = overlay.size != size || images.get(&overlay.image).is_none();
    if resized {
        overlay.image = images.add(make_overlay_image(size));
        overlay.size = size;
    }

    let viewport = Some(Viewport {
        physical_position: UVec2::ZERO,
        physical_size: size,
        ..default()
    });

    // The vello camera renders the scenes into the overlay image (transparent).
    if let Ok(e) = vello_cam.single() {
        if resized {
            commands.entity(e).insert((
                RenderTarget::Image(overlay.image.clone().into()),
                Camera {
                    order: -100,
                    clear_color: ClearColorConfig::Custom(Color::NONE),
                    viewport: viewport.clone(),
                    ..default()
                },
            ));
        }
    } else {
        commands.spawn((
            Camera2d,
            VelloView,
            Camera {
                order: -100,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                viewport,
                ..default()
            },
            RenderTarget::Image(overlay.image.clone().into()),
            Msaa::Off,
            RenderLayers::layer(VECTOR_RENDER_LAYER),
            VelloUiCamera,
            renzora::HideInHierarchy,
            Name::new("Vello UI Camera"),
        ));
    }

    // Ensure every canvas shows the overlay; refresh the handle on resize.
    let mut covered = bevy::platform::collections::HashSet::new();
    for (node, child_of) in &overlay_nodes {
        covered.insert(child_of.parent());
        if resized {
            commands.entity(node).insert(overlay_image_node(overlay.image.clone()));
        }
    }
    for canvas in &canvases {
        if covered.contains(&canvas) {
            continue;
        }
        let node = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                overlay_image_node(overlay.image.clone()),
                GlobalZIndex(i32::MAX - 100),
                FocusPolicy::Pass,
                RenderLayers::default(),
                VelloOverlayNode,
                renzora::HideInHierarchy,
                Name::new("Vello Overlay"),
            ))
            .id();
        commands.entity(canvas).add_child(node);
    }
}

/// Hide bevy_vello's internal canvas mesh from the editor hierarchy panel.
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
    app.init_resource::<VectorFont>()
        .init_resource::<VelloOverlay>();
    app.add_systems(
        Update,
        (manage_vello_overlay, hide_vello_internals, update_vectors),
    );
}
