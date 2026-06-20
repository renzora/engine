//! `vector="..."` — vector-graphics widgets for markup, rendered with ember's
//! own WGSL [`UiMaterial`] widgets (gauge / chart / waveform) + `bevy_text`.
//!
//! These were originally drawn with an external vector-graphics crate; Stage 2
//! of the `renzora_hui` → `renzora_ember` merge re-points them onto ember's
//! existing, proven widget shaders so that heavy dependency can be dropped.
//!
//! ## Primitives (`vector=`)
//! Radial (share `start`/`sweep` in degrees, `inset` px from the edge):
//! - `arc`    — stroked arc track + value fill (`value`/`min`/`max`), drawn with
//!   [`ArcMaterial`]. Optional centred `readout` text.
//! - `speedometer` — a composite dial: arc + numeric labels + needle + centre
//!   readout, assembled from an [`ArcMaterial`] node and `bevy_text` children.
//!
//! Cartesian series (`data="0.2,0.5,..."`, literal or a `{{ path }}` binding to
//! a comma string):
//! - `bars`     — bevy_ui rectangles (one per datum).
//! - `line`     — [`ChartMaterial`].
//! - `waveform` — [`WaveMaterial`].
//!
//! Common attrs: `color`, `track`, `thickness`, `start` (deg, def 135),
//! `sweep` (deg, def 270), `inset` (px, def 2), `count`, `min`/`max`,
//! `readout`, `unit`.
//!
//! ## How it renders
//! The loader stamps a [`VectorSpec`] on the node. Attach systems then add the
//! right ember material / children; sync systems re-resolve the `{{ }}`
//! bindings each frame and update the material params / rebuilt children.

use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui_render::prelude::MaterialNode;

use crate::font::{ui_font, EmberFonts};
use crate::widgets::{
    make_arc_params, ArcMaterial, ChartData, ChartPlugin, GaugePlugin, WaveData, WaveformPlugin,
};

/// 32-sample cap shared by the chart/wave shaders.
const MAX_SAMPLES: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VectorKind {
    Arc,
    Bars,
    Line,
    Wave,
    /// Full dial drawn as ONE node: arc + numeric labels + needle + centre
    /// readout, assembled as `bevy_text`/rect children over an arc material.
    Speedometer,
}

impl VectorKind {
    fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "arc" | "gauge" | "ring" => Self::Arc,
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

/// Stamped from `vector="..."`; the attach/sync systems below render this
/// element with the matching ember material + children.
#[derive(Component, Clone)]
pub struct VectorSpec {
    pub kind: VectorKind,
    /// Data-source entity for `{{ }}` (the binding host).
    pub host: Entity,
    /// Scalar value: a literal number or a `{{ path }}` binding (arc/dial).
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
            .and_then(|v| crate::markup::decor::parse_hex_color(v))
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

// ── Binding / value helpers (unchanged parsing surface) ──────────────────────

const DEG: f32 = std::f32::consts::PI / 180.0;

fn frac(v: f32, spec: &VectorSpec) -> f32 {
    let d = spec.max - spec.min;
    if d.abs() < f32::EPSILON {
        return 0.0;
    }
    ((v - spec.min) / d).clamp(0.0, 1.0)
}

/// Resolve `value` (literal or `{{ path }}`) against the host's live ECS state.
fn resolve_scalar(world: &mut World, host: Entity, value: &str) -> Option<f32> {
    let t = value.trim();
    if let Some(inner) = t.strip_prefix("{{").and_then(|s| s.strip_suffix("}}")) {
        crate::markup::binding::read_path(world, host, inner.trim())?.trim().parse::<f32>().ok()
    } else {
        t.parse::<f32>().ok()
    }
}

/// Resolve `readout` text — a literal (`FUEL`) or a `{{ path }}` binding.
fn resolve_text(world: &mut World, host: Entity, raw: &str) -> String {
    let t = raw.trim();
    if let Some(inner) = t.strip_prefix("{{").and_then(|s| s.strip_suffix("}}")) {
        crate::markup::binding::read_path(world, host, inner.trim())
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
        crate::markup::binding::read_path(world, host, inner.trim()).unwrap_or_default()
    } else {
        s.to_string()
    };
    text.split(',').filter_map(|t| t.trim().parse::<f32>().ok()).collect()
}

/// Format a tick value: integer when whole, else one decimal.
fn fmt_num(v: f32) -> String {
    if (v.round() - v).abs() < 0.05 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.1}")
    }
}

// ── Attach: stamp the right ember widget/children for each VectorKind ─────────

/// Marker on a node already wired by [`vector_attach`], so it isn't re-built.
#[derive(Component)]
struct VectorBuilt;

/// Tracks the last-resolved series for bars/line/wave so the sync systems only
/// rebuild/re-pack when the data actually changes.
#[derive(Component, Default)]
struct VectorSeries(Vec<f32>);

/// Marks the centre readout `Text` child of an `arc`/`speedometer` so its value
/// can be updated each frame without rebuilding the dial.
#[derive(Component)]
struct VectorReadout;

/// Marks the needle node of a `speedometer` (so the sync system can rotate it).
#[derive(Component)]
struct VectorNeedle;

/// Convert the spec's `thickness` (px) into the shader's fraction-of-radius.
/// We don't know the node's pixel radius at attach time, so approximate with a
/// reasonable default node radius; the arc shader clamps internally.
fn thick_fraction(spec: &VectorSpec) -> f32 {
    // The arc shader interprets `params.w` as thickness / radius. The markup
    // `thickness` is in logical px; assume a ~64px radius dial as the reference
    // so a 10px ring → ~0.16, matching the original dial look closely enough.
    (spec.thickness / 64.0).clamp(0.02, 0.95)
}

/// Centre `readout` text child (big value, white), reused by arc + speedometer.
fn readout_child(commands: &mut Commands, fonts: &EmberFonts, spec: &VectorSpec, text: &str) -> Entity {
    let size = if spec.readsize > 0.0 { spec.readsize } else { 22.0 };
    let block = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
            Name::new("vector-readout"),
        ))
        .id();
    let value = commands
        .spawn((
            Text::new(text.to_string()),
            ui_font(&fonts.ui, size),
            TextColor(Color::WHITE),
            VectorReadout,
        ))
        .id();
    commands.entity(block).add_child(value);
    if !spec.unit.is_empty() {
        let usize = (size * 0.45).clamp(9.0, 16.0);
        let unit = commands
            .spawn((
                Text::new(spec.unit.clone()),
                ui_font(&fonts.ui, usize),
                TextColor(spec.tickcolor),
            ))
            .id();
        commands.entity(block).add_child(unit);
    }
    block
}

/// Place `count`+1 numeric labels (`min`..`max`) around the dial arc as
/// absolutely-positioned `bevy_text` children (positions computed with cos/sin
/// in node-local %).
fn speedometer_labels(commands: &mut Commands, parent: Entity, fonts: &EmberFonts, spec: &VectorSpec) {
    let size = if spec.size > 0.0 { spec.size } else { 11.0 };
    // Labels sit on a circle of radius `rad` (fraction of half-extent) from the
    // node centre; convert to left/top percentages.
    let rad = 0.40_f32;
    for i in 0..=spec.count {
        let t = i as f32 / spec.count as f32;
        let a = (spec.start + spec.sweep * t) * DEG;
        let v = spec.min + (spec.max - spec.min) * t;
        // Node-local fractional position (0..1), centre at 0.5.
        let fx = 0.5 + rad * a.cos();
        let fy = 0.5 + rad * a.sin();
        let label = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(fx * 100.0),
                    top: Val::Percent(fy * 100.0),
                    // Nudge so the glyph centres on the point.
                    margin: UiRect::new(Val::Px(-8.0), Val::ZERO, Val::Px(-7.0), Val::ZERO),
                    ..default()
                },
                Text::new(fmt_num(v)),
                ui_font(&fonts.ui, size),
                TextColor(spec.tickcolor),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(parent).add_child(label);
    }
}

/// Node-local fractional (left, top) of the value marker on the dial arc.
/// Centre is (0.5, 0.5); the marker rides a circle of radius `NEEDLE_RAD`.
const NEEDLE_RAD: f32 = 0.30;

fn needle_pos(spec: &VectorSpec, f: f32) -> (f32, f32) {
    let a = (spec.start + spec.sweep * f) * DEG;
    (0.5 + NEEDLE_RAD * a.cos(), 0.5 + NEEDLE_RAD * a.sin())
}

/// A small value marker (a coloured dot) placed on the arc at `value`. Bevy
/// `UiTransform` only rotates about a node's own centre, so a rotated needle
/// can't pivot from the dial hub cleanly — a positioned marker is the
/// lowest-risk, layout-safe indicator. Updated each frame via its `left`/`top`.
fn speedometer_needle(commands: &mut Commands, parent: Entity, spec: &VectorSpec, f: f32) -> Entity {
    let (fx, fy) = needle_pos(spec, f);
    let needle = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(fx * 100.0),
                top: Val::Percent(fy * 100.0),
                width: Val::Px(8.0),
                height: Val::Px(8.0),
                margin: UiRect::new(Val::Px(-4.0), Val::ZERO, Val::Px(-4.0), Val::ZERO),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(spec.color),
            VectorNeedle,
            Pickable::IGNORE,
            Name::new("vector-needle"),
        ))
        .id();
    commands.entity(parent).add_child(needle);
    needle
}

/// Rebuild `bars` rect children sized to the node, one per normalized datum.
fn build_bars(commands: &mut Commands, node: Entity, spec: &VectorSpec, data: &[f32]) {
    commands.entity(node).despawn_children();
    if data.is_empty() {
        return;
    }
    let bars: Vec<Entity> = data
        .iter()
        .map(|&v| {
            let h = (frac(v, spec) * 100.0).max(0.0);
            commands
                .spawn((
                    Node {
                        flex_grow: 1.0,
                        min_width: Val::Px(2.0),
                        height: Val::Percent(h),
                        ..default()
                    },
                    BackgroundColor(spec.color),
                    Pickable::IGNORE,
                    Name::new("vector-bar"),
                ))
                .id()
        })
        .collect();
    commands.entity(node).insert(Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::FlexEnd,
        column_gap: Val::Px(2.0),
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        ..default()
    });
    commands.entity(node).add_children(&bars);
}

/// Map a spec's data range onto 0..1 waveform amplitudes (the wave shader
/// expects 0..1). The default wave range is (-1, 1) so |v| maps to the envelope.
fn wave_amps(spec: &VectorSpec, data: &[f32]) -> Vec<f32> {
    let mut amps: Vec<f32> = data.iter().map(|&v| frac(v, spec)).collect();
    if amps.len() > MAX_SAMPLES {
        debug!("[vector] waveform: {} samples truncated to {MAX_SAMPLES}", amps.len());
        amps.truncate(MAX_SAMPLES);
    }
    amps
}

/// Attach the matching ember widget to every freshly-stamped `VectorSpec` node.
/// Exclusive so the initial value/data bindings can be read via `read_path`.
fn vector_attach(world: &mut World) {
    let pending: Vec<Entity> = {
        let mut q = world.query_filtered::<Entity, (With<VectorSpec>, Without<VectorBuilt>)>();
        q.iter(world).collect()
    };
    if pending.is_empty() {
        return;
    }
    let fonts = world.get_resource::<EmberFonts>().cloned();
    for e in pending {
        let Some(spec) = world.get::<VectorSpec>(e).cloned() else {
            continue;
        };
        // Kinds that draw text need `EmberFonts`. Under `WidgetsPlugin`/the
        // editor it loads a frame or two in; wait for it so labels/readout
        // aren't permanently dropped. (Plain `arc` w/o readout, bars/line/wave
        // need no font — build them immediately.)
        let needs_font = matches!(spec.kind, VectorKind::Speedometer) || spec.readout.is_some();
        if needs_font && fonts.is_none() {
            continue;
        }
        match spec.kind {
            VectorKind::Arc => {
                let v = resolve_scalar(world, spec.host, &spec.value).unwrap_or(spec.min);
                let mat = make_arc_params(
                    frac(v, &spec),
                    spec.start * DEG,
                    spec.sweep * DEG,
                    thick_fraction(&spec),
                    spec.track,
                    spec.fill_or_track_for_arc(),
                );
                let handle = world.resource_mut::<Assets<ArcMaterial>>().add(mat);
                world.entity_mut(e).insert(MaterialNode(handle));
                // Optional centre readout child.
                if let (Some(raw), Some(fonts)) = (spec.readout.clone(), fonts.as_ref()) {
                    let text = resolve_text(world, spec.host, &raw);
                    let child = {
                        let mut commands = world.commands();
                        readout_child(&mut commands, fonts, &spec, &text)
                    };
                    world.flush();
                    world.entity_mut(e).add_child(child);
                }
            }
            VectorKind::Speedometer => {
                let v = resolve_scalar(world, spec.host, &spec.value).unwrap_or(spec.min);
                let f = frac(v, &spec);
                // Arc material drives the dial face + value sweep.
                let mat = make_arc_params(
                    f,
                    spec.start * DEG,
                    spec.sweep * DEG,
                    thick_fraction(&spec),
                    spec.track,
                    spec.color,
                );
                let handle = world.resource_mut::<Assets<ArcMaterial>>().add(mat);
                world.entity_mut(e).insert(MaterialNode(handle));
                if let Some(fonts) = fonts.as_ref() {
                    let text = spec
                        .readout
                        .clone()
                        .map(|raw| resolve_text(world, spec.host, &raw))
                        .unwrap_or_default();
                    // NOTE: not GPU-verified. The dial composite (numeric
                    // labels + needle + centre readout) replaces the original
                    // dial; visual is approximate, not pixel-identical.
                    let child = {
                        let mut commands = world.commands();
                        speedometer_labels(&mut commands, e, fonts, &spec);
                        speedometer_needle(&mut commands, e, &spec, f);
                        readout_child(&mut commands, fonts, &spec, &text)
                    };
                    world.flush();
                    world.entity_mut(e).add_child(child);
                }
            }
            VectorKind::Bars => {
                let data = resolve_series(world, spec.host, &spec.data);
                {
                    let mut commands = world.commands();
                    build_bars(&mut commands, e, &spec, &data);
                }
                world.flush();
                world.entity_mut(e).insert(VectorSeries(data));
            }
            VectorKind::Line => {
                let mut data = resolve_series(world, spec.host, &spec.data);
                if data.len() > MAX_SAMPLES {
                    debug!("[vector] line: {} samples truncated to {MAX_SAMPLES}", data.len());
                    data.truncate(MAX_SAMPLES);
                }
                world
                    .entity_mut(e)
                    .insert(ChartData::ranged(data.clone(), spec.min, spec.max, spec.color));
                world.entity_mut(e).insert(VectorSeries(data));
            }
            VectorKind::Wave => {
                let data = resolve_series(world, spec.host, &spec.data);
                let amps = wave_amps(&spec, &data);
                world.entity_mut(e).insert(WaveData::new(amps));
                world.entity_mut(e).insert(VectorSeries(data));
            }
        }
        world.entity_mut(e).insert(VectorBuilt);
    }
}

impl VectorSpec {
    /// An `arc` fills with its `color`; the shader's `fill` slot is the value
    /// band. (The separate `fill` field is the disc behind the ring, which the
    /// ember arc shader doesn't support — see the parity note in `plugin`.)
    fn fill_or_track_for_arc(&self) -> Color {
        self.color
    }
}

// ── Sync: re-resolve bindings each frame & update materials / children ────────

/// Update arc + speedometer dials from their live `{{ value }}` binding: re-pack
/// the [`ArcMaterial`] value param, the centre readout text, and (dial) needle.
fn vector_dial_sync(world: &mut World) {
    let dials: Vec<(Entity, VectorSpec)> = {
        let mut q = world.query_filtered::<(Entity, &VectorSpec), With<VectorBuilt>>();
        q.iter(world)
            .filter(|(_, s)| matches!(s.kind, VectorKind::Arc | VectorKind::Speedometer))
            .map(|(e, s)| (e, s.clone()))
            .collect()
    };
    for (e, spec) in dials {
        let v = resolve_scalar(world, spec.host, &spec.value).unwrap_or(spec.min);
        let f = frac(v, &spec);
        // Material value param.
        if let Some(node) = world.get::<MaterialNode<ArcMaterial>>(e).map(|n| n.0.clone()) {
            if let Some(mut m) = world.resource_mut::<Assets<ArcMaterial>>().get_mut(&node) {
                m.params.x = f;
            }
        }
        // Centre readout text (search this dial's descendants for the marker).
        if let Some(raw) = spec.readout.clone() {
            let text = resolve_text(world, spec.host, &raw);
            if let Some(child) = find_readout_child(world, e) {
                if let Some(mut t) = world.get_mut::<Text>(child) {
                    if t.0 != text {
                        t.0 = text;
                    }
                }
            }
        }
        // Needle marker position (speedometer only).
        if matches!(spec.kind, VectorKind::Speedometer) {
            if let Some(needle) = find_needle_child(world, e) {
                let (fx, fy) = needle_pos(&spec, f);
                if let Some(mut node) = world.get_mut::<Node>(needle) {
                    node.left = Val::Percent(fx * 100.0);
                    node.top = Val::Percent(fy * 100.0);
                }
            }
        }
    }
}

/// Find the `VectorReadout`-marked `Text` descendant of a dial node.
fn find_readout_child(world: &mut World, root: Entity) -> Option<Entity> {
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if world.get::<VectorReadout>(e).is_some() {
            return Some(e);
        }
        if let Some(children) = world.get::<Children>(e) {
            stack.extend(children.iter());
        }
    }
    None
}

/// Find the `VectorNeedle`-marked descendant of a dial node.
fn find_needle_child(world: &mut World, root: Entity) -> Option<Entity> {
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if world.get::<VectorNeedle>(e).is_some() {
            return Some(e);
        }
        if let Some(children) = world.get::<Children>(e) {
            stack.extend(children.iter());
        }
    }
    None
}

/// Re-resolve series-backed widgets (`bars`/`line`/`waveform`) each frame and
/// rebuild / re-pack only when the resolved data changes.
fn vector_series_sync(world: &mut World) {
    let series: Vec<(Entity, VectorSpec)> = {
        let mut q = world.query_filtered::<(Entity, &VectorSpec), With<VectorBuilt>>();
        q.iter(world)
            .filter(|(_, s)| matches!(s.kind, VectorKind::Bars | VectorKind::Line | VectorKind::Wave))
            .map(|(e, s)| (e, s.clone()))
            .collect()
    };
    for (e, spec) in series {
        let data = resolve_series(world, spec.host, &spec.data);
        let changed = world.get::<VectorSeries>(e).map(|s| s.0 != data).unwrap_or(true);
        if !changed {
            continue;
        }
        match spec.kind {
            VectorKind::Bars => {
                {
                    let mut commands = world.commands();
                    build_bars(&mut commands, e, &spec, &data);
                }
                world.flush();
            }
            VectorKind::Line => {
                let mut clipped = data.clone();
                if clipped.len() > MAX_SAMPLES {
                    clipped.truncate(MAX_SAMPLES);
                }
                if let Some(mut cd) = world.get_mut::<ChartData>(e) {
                    cd.set_values(clipped);
                }
            }
            VectorKind::Wave => {
                let amps = wave_amps(&spec, &data);
                if let Some(mut wd) = world.get_mut::<WaveData>(e) {
                    wd.set_amps(amps);
                }
            }
            _ => {}
        }
        if let Some(mut s) = world.get_mut::<VectorSeries>(e) {
            s.0 = data;
        } else {
            world.entity_mut(e).insert(VectorSeries(data));
        }
    }
}

pub fn plugin(app: &mut App) {
    // The markup runtime can run WITHOUT `WidgetsPlugin` (a shipped game with
    // only `MarkupPlugin`), so register the material widgets we reuse here.
    // `WidgetsPlugin` (editor) adds the same three; bevy panics on a duplicate
    // plugin TYPE at the `add_plugins` call (before `build()` runs), so guard
    // each at the call site — whichever path runs first installs it.
    if !app.is_plugin_added::<GaugePlugin>() {
        app.add_plugins(GaugePlugin);
    }
    if !app.is_plugin_added::<ChartPlugin>() {
        app.add_plugins(ChartPlugin);
    }
    if !app.is_plugin_added::<WaveformPlugin>() {
        app.add_plugins(WaveformPlugin);
    }
    app.add_systems(
        Update,
        (vector_attach, vector_dial_sync, vector_series_sync).chain(),
    );
}
