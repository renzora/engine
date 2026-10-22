//! Curve and gradient properties: a compact preview on the node, and a
//! popover that edits the keys.
//!
//! ember has a curve widget and a gradient widget, but neither fits: the curve
//! one is a two-handle cubic-bezier easing (a tween's timing, not a table of
//! keys), and the gradient one keeps its stops in a private component with no
//! way to read an edit back out. What *is* reused is ember's live line chart
//! (the plot and the inline preview are both [`line_chart_live`]), bevy_ui's
//! own `BackgroundGradient` for the colour ramps, ember's drag fields,
//! dropdown and RGBA colour field for the selected key, and its overlay
//! surface rules for the popover.
//!
//! The popover is a screen-space panel rather than a child of the node, for
//! the reason the material editor's sample switcher is: inside the graph it
//! would zoom with the canvas and be clipped by the viewport. It is closed by
//! a click outside it (one landing in a dropdown list or colour picker it
//! opened does not count), by Escape, by its X, and by its node going away.
//!
//! Keys keep their file order. A dragged key is held between its neighbours
//! rather than re-sorted, so the key under the pointer stays the one being
//! dragged; the compiler sorts anyway.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::window::SystemCursorIcon;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::inspector::color_field_rgba;
use renzora_ember::interaction::{BLOCK, PASS_THROUGH};
use renzora_ember::reactive::tracked::{bind_2way, bind_text, bind_with, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::{close_button, dropdown, drag_value, line_chart_live, ChartStyle, HoverTooltip, OverlaySurface};
use renzora_ember::UiInteraction;
use renzora_particles::asset::{NodeId, PinValue};
use renzora_particles::curve::{CurvePoint, CurveTable, Ease, GradientStop, GradientTable};

use crate::model;
use crate::pin_editors::{linear_to_srgb, srgb_to_linear, write};
use crate::state::VfxEditorState;

const PLOT_W: f32 = 300.0;
const PLOT_H: f32 = 150.0;
const SAMPLES: usize = 32;
const EASES: [(Ease, &str); 6] = [
    (Ease::Linear, "Linear"),
    (Ease::Step, "Step"),
    (Ease::Smooth, "Smooth"),
    (Ease::EaseIn, "Ease In"),
    (Ease::EaseOut, "Ease Out"),
    (Ease::EaseInOut, "Ease In Out"),
];

pub struct CurveEditorPlugin;

impl Plugin for CurveEditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TableEdit>().add_systems(
            Update,
            (open_on_click, drag_keys, dismiss).chain().run_if(in_state(renzora_editor_framework::SplashState::Editor)),
        );
    }
}

/// Which property a preview or popover edits.
#[derive(Clone, Debug, PartialEq)]
pub struct PropTarget {
    pub emitter: usize,
    pub node: NodeId,
    pub prop: &'static str,
    pub gradient: bool,
}

/// The open popover's selection and plot range. One at a time, so a resource.
#[derive(Resource, Default)]
pub struct TableEdit {
    target: Option<PropTarget>,
    selected: usize,
    /// The curve plot's value range, fixed while the popover is open so the
    /// plot does not rescale under a key being dragged.
    lo: f32,
    hi: f32,
}

/// The clickable preview on the node.
#[derive(Component)]
struct TableButton(PropTarget);

#[derive(Component)]
struct TablePopover;

/// A key handle; `index` into the table.
#[derive(Component)]
struct KeyHandle {
    index: usize,
    plot: Entity,
}

fn points(w: &Rx, t: &PropTarget) -> Vec<CurvePoint> {
    let Some(st) = w.get_resource::<VfxEditorState>() else { return Vec::new() };
    let Some(n) = st.doc.emitters.get(t.emitter).and_then(|e| model::node(&e.graph, t.node)) else {
        return Vec::new();
    };
    match n.input_values.get(t.prop) {
        Some(PinValue::Curve(p)) => p.clone(),
        _ => Vec::new(),
    }
}

fn stops(w: &Rx, t: &PropTarget) -> Vec<GradientStop> {
    let Some(st) = w.get_resource::<VfxEditorState>() else { return Vec::new() };
    let Some(n) = st.doc.emitters.get(t.emitter).and_then(|e| model::node(&e.graph, t.node)) else {
        return Vec::new();
    };
    match n.input_values.get(t.prop) {
        Some(PinValue::Gradient(s)) => s.clone(),
        _ => Vec::new(),
    }
}

/// The value range a curve's plot shows: always 0..1, widened to fit keys
/// outside it (a size curve that grows to 3).
pub fn plot_range(points: &[CurvePoint]) -> (f32, f32) {
    let lo = points.iter().map(|p| p.value).fold(0.0f32, f32::min);
    let hi = points.iter().map(|p| p.value).fold(1.0f32, f32::max);
    (lo, if hi - lo < 1e-3 { lo + 1.0 } else { hi })
}

/// `SAMPLES` values of the curve across 0..1, mapped into 0..1 by `range`.
fn sample_normalized(points: &[CurvePoint], (lo, hi): (f32, f32)) -> Vec<f32> {
    let table = CurveTable::new(points);
    let span = (hi - lo).max(1e-4);
    (0..SAMPLES)
        .map(|i| ((table.sample(i as f32 / (SAMPLES - 1) as f32) - lo) / span).clamp(0.0, 1.0))
        .collect()
}

/// The stops as a left-to-right bevy_ui gradient. Sorted, since a CSS-style
/// gradient takes its stops in order and a hand-written file may not.
fn ui_gradient(stops: &[GradientStop]) -> BackgroundGradient {
    let table = GradientTable::new(stops);
    let mut cs: Vec<ColorStop> = table
        .stops
        .iter()
        .map(|s| ColorStop::new(Color::linear_rgba(s.color[0], s.color[1], s.color[2], s.color[3]), Val::Percent(s.t.clamp(0.0, 1.0) * 100.0)))
        .collect();
    if cs.is_empty() {
        cs = vec![ColorStop::auto(Color::WHITE), ColorStop::auto(Color::WHITE)];
    } else if cs.len() == 1 {
        let only = cs[0];
        cs.push(only);
    }
    BackgroundGradient(vec![Gradient::Linear(LinearGradient::new(90f32.to_radians(), cs))])
}

// ── Compact previews on the node ─────────────────────────────────────────────

/// The inline preview of a curve or gradient property. Click it to edit.
pub fn table_preview(commands: &mut Commands, target: PropTarget) -> Entity {
    let wrap = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(90.0),
                height: Val::Px(if target.gradient { 14.0 } else { 24.0 }),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            BorderColor::all(rgb(border())),
            UiInteraction::default(),
            renzora_ember::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            HoverTooltip::new(if target.gradient { "Click to edit the gradient" } else { "Click to edit the curve" }),
            Name::new("vfx-table-preview"),
        ))
        .id();
    if target.gradient {
        let t = target.clone();
        bind_with(commands, wrap, move |w| stops(w, &t), |w, e, s: &Vec<GradientStop>| {
            if let Ok(mut em) = w.get_entity_mut(e) {
                em.insert(ui_gradient(s));
            }
        });
    } else {
        let t = target.clone();
        let chart = line_chart_live(
            commands,
            ChartStyle { color: rgb(accent()), min: Some(0.0), max: Some(1.0), target: None, height: 22.0 },
            move |w| {
                let p = points(w, &t);
                sample_normalized(&p, plot_range(&p))
            },
        );
        commands.entity(chart).insert(PASS_THROUGH).entry::<Node>().and_modify(|mut n| {
            n.border = UiRect::all(Val::Px(0.0));
        });
        commands.entity(wrap).add_child(chart);
    }
    commands.entity(wrap).insert(TableButton(target));
    wrap
}

/// Click a preview: open (or switch) the popover at the cursor.
fn open_on_click(
    q: Query<(&UiInteraction, &TableButton), Changed<UiInteraction>>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    existing: Query<Entity, With<TablePopover>>,
    state: Res<VfxEditorState>,
    mut edit: ResMut<TableEdit>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else { return };
    let Some((_, button)) = q.iter().find(|(i, _)| **i == UiInteraction::Pressed) else { return };
    let Some(win) = windows.iter().find(|w| w.cursor_position().is_some()) else { return };
    let Some(cursor) = win.cursor_position() else { return };
    for e in &existing {
        commands.entity(e).despawn();
    }
    let t = button.0.clone();
    let range = match state
        .doc
        .emitters
        .get(t.emitter)
        .and_then(|e| model::node(&e.graph, t.node))
        .and_then(|n| n.input_values.get(t.prop))
    {
        Some(PinValue::Curve(p)) => plot_range(p),
        _ => (0.0, 1.0),
    };
    edit.target = Some(t.clone());
    edit.selected = 0;
    edit.lo = range.0;
    edit.hi = range.1;
    // Open beside the pointer, kept on-screen by a rough size.
    let (w, h) = (PLOT_W + 24.0, if t.gradient { 222.0 } else { 300.0 });
    let x = (cursor.x + 12.0).min((win.width() - w).max(0.0));
    let y = (cursor.y - 20.0).clamp(0.0, (win.height() - h).max(0.0));
    popover(&mut commands, &fonts, t, x, y);
}

fn popover(commands: &mut Commands, fonts: &EmberFonts, t: PropTarget, x: f32, y: f32) -> Entity {
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x),
                top: Val::Px(y),
                width: Val::Px(PLOT_W + 24.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            BoxShadow::new(Color::BLACK.with_alpha(0.4), Val::Px(0.0), Val::Px(6.0), Val::Px(0.0), Val::Px(18.0)),
            GlobalZIndex(900),
            BLOCK,
            OverlaySurface,
            RelativeCursorPosition::default(),
            TablePopover,
            Name::new("vfx-table-popover"),
        ))
        .id();

    let header = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() })
        .id();
    let icon = icon_text(commands, &fonts.phosphor, if t.gradient { "gradient" } else { "chart-line" }, accent(), 14.0);
    let title = commands
        .spawn((Text::new(if t.gradient { "Gradient" } else { "Curve" }), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_primary()))))
        .id();
    let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let close = close_button(commands, fonts);
    commands.entity(close).insert(PopoverClose);
    commands.entity(header).add_children(&[icon, title, spacer, close]);
    commands.entity(root).add_child(header);

    let body = if t.gradient { gradient_body(commands, fonts, t) } else { curve_body(commands, fonts, t) };
    commands.entity(root).add_child(body);
    root
}

#[derive(Component)]
struct PopoverClose;

/// Buttons in the popover's action row.
#[derive(Component, Clone, Copy)]
enum KeyAction {
    Add,
    Delete,
    Reset,
    /// The gradient preset: a fire's blackbody colour over life (see
    /// `renzora_particles::blackbody::fire_gradient`).
    Blackbody,
}

fn small_button(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str, action: KeyAction) -> Entity {
    let b = commands
        .spawn((
            Node {
                height: Val::Px(24.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::horizontal(Val::Px(9.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            renzora_ember::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            action,
            Name::new("vfx-key-action"),
        ))
        .id();
    bind_with(commands, b, move |w| matches!(w.get::<UiInteraction>(b), Some(UiInteraction::Hovered | UiInteraction::Pressed)), |w, e, hot: &bool| {
        if let Some(mut bg) = w.get_mut::<BackgroundColor>(e) {
            bg.0 = if *hot { rgb(hover_bg()) } else { Color::NONE };
        }
    });
    let glyph = icon_text(commands, &fonts.phosphor, icon, accent(), 12.0);
    let text = commands.spawn((Text::new(label), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), PASS_THROUGH)).id();
    commands.entity(glyph).insert(PASS_THROUGH);
    commands.entity(b).add_children(&[glyph, text]);
    b
}

fn field_row(commands: &mut Commands, fonts: &EmberFonts, label: &str, control: Entity) -> Entity {
    let row = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), flex_grow: 1.0, ..default() })
        .id();
    let text = commands.spawn((Text::new(label), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id();
    commands.entity(control).entry::<Node>().and_modify(|mut n| {
        n.flex_grow = 1.0;
        n.min_width = Val::Px(50.0);
    });
    commands.entity(row).add_children(&[text, control]);
    row
}

fn row(commands: &mut Commands, kids: &[Entity]) -> Entity {
    let r = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() })
        .id();
    commands.entity(r).add_children(kids);
    r
}

// ── Curve popover ────────────────────────────────────────────────────────────

fn curve_body(commands: &mut Commands, fonts: &EmberFonts, t: PropTarget) -> Entity {
    let body = commands.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() }).id();

    let plot = commands
        .spawn((
            Node {
                width: Val::Px(PLOT_W),
                height: Val::Px(PLOT_H),
                position_type: PositionType::Relative,
                ..default()
            },
            RelativeCursorPosition::default(),
            Name::new("vfx-curve-plot"),
        ))
        .id();
    let tt = t.clone();
    let chart = line_chart_live(
        commands,
        ChartStyle { color: rgb(accent()), min: Some(0.0), max: Some(1.0), target: None, height: PLOT_H },
        move |w| {
            let range = w.get_resource::<TableEdit>().map(|e| (e.lo, e.hi)).unwrap_or((0.0, 1.0));
            sample_normalized(&points(w, &tt), range)
        },
    );
    commands.entity(chart).insert(PASS_THROUGH);
    let handles = commands
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            PASS_THROUGH,
        ))
        .id();
    commands.entity(plot).add_children(&[chart, handles]);
    let tk = t.clone();
    keyed_list(commands, handles, move |w| {
        let n = points(w, &tk).len();
        KeyedSnapshot {
            items: (0..n as u64).map(|i| (i, i)).collect(),
            build: Box::new(move |c, _f, i| key_handle(c, i, plot, false)),
        }
    });

    // Range labels: the plot's top and bottom value.
    let range = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted()))))
        .id();
    bind_text(commands, range, |w| {
        let (lo, hi) = w.get_resource::<TableEdit>().map(|e| (e.lo, e.hi)).unwrap_or((0.0, 1.0));
        format!("Value range {lo:.2} to {hi:.2}. Drag a key; T runs 0 to 1 over the input.")
    });

    // The selected key's fields.
    let tf = t.clone();
    let t_field = key_field(commands, fonts, "T", move |w| sel_point(w, &tf).map(|p| p.t).unwrap_or(0.0), {
        let t = t.clone();
        move |w, v| edit_point(w, &t, |pts, i| {
            let (min, max) = neighbour_bounds_curve(pts, i);
            pts[i].t = v.clamp(min, max);
        })
    });
    let tv = t.clone();
    let v_field = key_field(commands, fonts, "Value", move |w| sel_point(w, &tv).map(|p| p.value).unwrap_or(0.0), {
        let t = t.clone();
        move |w, v| edit_point(w, &t, |pts, i| pts[i].value = v)
    });
    let labels: Vec<&str> = EASES.iter().map(|(_, l)| *l).collect();
    let ease = dropdown(commands, fonts, &labels, 0);
    let te = t.clone();
    bind_2way(
        commands,
        ease,
        move |w: &Rx| sel_point(w, &te).map(|p| EASES.iter().position(|(e, _)| *e == p.ease).unwrap_or(0)).unwrap_or(0),
        {
            let t = t.clone();
            move |w: &mut World, v: &usize| {
                let ease = EASES.get(*v).map(|(e, _)| *e).unwrap_or_default();
                edit_point(w, &t, |pts, i| pts[i].ease = ease);
            }
        },
    );
    let r1 = row(commands, &[t_field, v_field]);
    let ease_row = field_row(commands, fonts, "Ease", ease);

    let add = small_button(commands, fonts, "plus", "Add Key", KeyAction::Add);
    let del = small_button(commands, fonts, "trash", "Delete Key", KeyAction::Delete);
    let reset = small_button(commands, fonts, "arrow-counter-clockwise", "Reset", KeyAction::Reset);
    let actions = row(commands, &[add, del, reset]);

    commands.entity(body).add_children(&[plot, range, r1, ease_row, actions]);
    body
}

fn key_handle(commands: &mut Commands, index: usize, plot: Entity, gradient: bool) -> Entity {
    let size = if gradient { 12.0 } else { 11.0 };
    let h = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(size),
                height: Val::Px(size),
                margin: UiRect::new(Val::Px(-size / 2.0), Val::Px(0.0), Val::Px(if gradient { 0.0 } else { -size / 2.0 }), Val::Px(0.0)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(if gradient { 3.0 } else { size / 2.0 })),
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            BorderColor::all(rgb(accent())),
            UiInteraction::default(),
            renzora_ember::cursor_icon::HoverCursor(SystemCursorIcon::Grab),
            KeyHandle { index, plot },
            Name::new("vfx-key-handle"),
        ))
        .id();
    bind_with(
        commands,
        h,
        move |w| {
            let edit = w.get_resource::<TableEdit>();
            let t = edit.and_then(|e| e.target.clone())?;
            let selected = edit.map(|e| e.selected) == Some(index);
            if gradient {
                let s = stops(w, &t);
                let stop = s.get(index)?;
                Some((stop.t, 0.0, selected, linear_to_srgb(stop.color)))
            } else {
                let p = points(w, &t);
                let pt = p.get(index)?;
                let (lo, hi) = edit.map(|e| (e.lo, e.hi)).unwrap_or((0.0, 1.0));
                let y = ((pt.value - lo) / (hi - lo).max(1e-4)).clamp(0.0, 1.0);
                Some((pt.t, y, selected, [0.0; 4]))
            }
        },
        move |w, e, v: &Option<(f32, f32, bool, [f32; 4])>| {
            let Some((x, y, selected, color)) = *v else { return };
            if let Some(mut n) = w.get_mut::<Node>(e) {
                n.left = Val::Percent(x.clamp(0.0, 1.0) * 100.0);
                if !gradient {
                    n.top = Val::Percent((1.0 - y) * 100.0);
                }
            }
            if let Some(mut b) = w.get_mut::<BorderColor>(e) {
                *b = BorderColor::all(rgb(if selected { text_primary() } else { accent() }));
            }
            if let Some(mut bg) = w.get_mut::<BackgroundColor>(e) {
                bg.0 = if gradient {
                    Color::srgba(color[0], color[1], color[2], 1.0)
                } else if selected {
                    rgb(accent())
                } else {
                    rgb(window_bg())
                };
            }
        },
    );
    h
}

fn key_field<G, S>(commands: &mut Commands, fonts: &EmberFonts, label: &str, get: G, set: S) -> Entity
where
    G: Fn(&Rx) -> f32 + Send + Sync + 'static,
    S: Fn(&mut World, f32) + Send + Sync + 'static,
{
    let dv = drag_value(commands, &fonts.ui, "", value_text(), 0.0, 0.01);
    bind_2way(commands, dv, get, move |w: &mut World, v: &f32| set(w, *v));
    field_row(commands, fonts, label, dv)
}

fn sel_point(w: &Rx, t: &PropTarget) -> Option<CurvePoint> {
    let sel = w.get_resource::<TableEdit>()?.selected;
    points(w, t).get(sel).copied()
}

fn sel_stop(w: &Rx, t: &PropTarget) -> Option<GradientStop> {
    let sel = w.get_resource::<TableEdit>()?.selected;
    stops(w, t).get(sel).copied()
}

/// Where key `i` may move: between its neighbours, inside 0..1.
fn neighbour_bounds_curve(pts: &[CurvePoint], i: usize) -> (f32, f32) {
    let min = if i > 0 { pts[i - 1].t } else { 0.0 };
    let max = pts.get(i + 1).map_or(1.0, |p| p.t);
    (min.min(max), max.max(min))
}

fn neighbour_bounds_gradient(s: &[GradientStop], i: usize) -> (f32, f32) {
    let min = if i > 0 { s[i - 1].t } else { 0.0 };
    let max = s.get(i + 1).map_or(1.0, |p| p.t);
    (min.min(max), max.max(min))
}

fn edit_point(w: &mut World, t: &PropTarget, f: impl FnOnce(&mut Vec<CurvePoint>, usize)) {
    let Some(sel) = w.get_resource::<TableEdit>().map(|e| e.selected) else { return };
    let mut pts = points(&Rx::new(&*w), t);
    if sel >= pts.len() {
        return;
    }
    f(&mut pts, sel);
    write(w, t.emitter, t.node, t.prop, PinValue::Curve(pts));
}

fn edit_stop(w: &mut World, t: &PropTarget, f: impl FnOnce(&mut Vec<GradientStop>, usize)) {
    let Some(sel) = w.get_resource::<TableEdit>().map(|e| e.selected) else { return };
    let mut s = stops(&Rx::new(&*w), t);
    if sel >= s.len() {
        return;
    }
    f(&mut s, sel);
    write(w, t.emitter, t.node, t.prop, PinValue::Gradient(s));
}

// ── Gradient popover ─────────────────────────────────────────────────────────

fn gradient_body(commands: &mut Commands, fonts: &EmberFonts, t: PropTarget) -> Entity {
    let body = commands.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() }).id();
    let bar = commands
        .spawn((
            Node {
                width: Val::Px(PLOT_W),
                height: Val::Px(26.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BorderColor::all(rgb(border())),
            Name::new("vfx-gradient-bar"),
        ))
        .id();
    let tb = t.clone();
    bind_with(commands, bar, move |w| stops(w, &tb), |w, e, s: &Vec<GradientStop>| {
        if let Ok(mut em) = w.get_entity_mut(e) {
            em.insert(ui_gradient(s));
        }
    });
    let strip = commands
        .spawn((
            Node { width: Val::Px(PLOT_W), height: Val::Px(14.0), position_type: PositionType::Relative, ..default() },
            RelativeCursorPosition::default(),
            Name::new("vfx-gradient-strip"),
        ))
        .id();
    let tk = t.clone();
    keyed_list(commands, strip, move |w| {
        let n = stops(w, &tk).len();
        KeyedSnapshot {
            items: (0..n as u64).map(|i| (i, i)).collect(),
            build: Box::new(move |c, _f, i| key_handle(c, i, strip, true)),
        }
    });

    let tf = t.clone();
    let t_field = key_field(commands, fonts, "T", move |w| sel_stop(w, &tf).map(|s| s.t).unwrap_or(0.0), {
        let t = t.clone();
        move |w, v| edit_stop(w, &t, |s, i| {
            let (min, max) = neighbour_bounds_gradient(s, i);
            s[i].t = v.clamp(min, max);
        })
    });
    let tc = t.clone();
    let color = color_field_rgba(
        commands,
        move |w| sel_stop(w, &tc).map(|s| linear_to_srgb(s.color)).unwrap_or([1.0; 4]),
        {
            let t = t.clone();
            move |w, c| edit_stop(w, &t, |s, i| s[i].color = srgb_to_linear(c))
        },
    );
    let color_row = field_row(commands, fonts, "Color", color);
    let r1 = row(commands, &[t_field, color_row]);
    let add = small_button(commands, fonts, "plus", "Add Stop", KeyAction::Add);
    let del = small_button(commands, fonts, "trash", "Delete Stop", KeyAction::Delete);
    let reset = small_button(commands, fonts, "arrow-counter-clockwise", "Reset", KeyAction::Reset);
    let fire = small_button(commands, fonts, "fire", "Blackbody", KeyAction::Blackbody);
    commands.entity(fire).insert(HoverTooltip::new(
        "Replace the stops with a fire's colour over life: white-yellow cooling through orange to a dim red, fading out",
    ));
    let actions = row(commands, &[add, del, reset]);
    // Its own row: four buttons overflow the 300 px popover.
    let presets = row(commands, &[fire]);
    let hint = commands
        .spawn((Text::new("Drag a stop along the bar; T runs 0 to 1 over the input."), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted()))))
        .id();
    commands.entity(body).add_children(&[bar, strip, hint, r1, actions, presets]);
    body
}

// ── Systems ──────────────────────────────────────────────────────────────────

/// Drag a key handle: it follows the pointer inside its plot or strip, held
/// between its neighbours. Also handles the popover's action buttons.
#[allow(clippy::too_many_arguments)]
fn drag_keys(
    handles: Query<(&UiInteraction, &KeyHandle)>,
    pressed: Query<(&UiInteraction, &KeyHandle), Changed<UiInteraction>>,
    actions: Query<(&UiInteraction, &KeyAction), Changed<UiInteraction>>,
    plots: Query<&RelativeCursorPosition>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut edit: ResMut<TableEdit>,
    mut state: ResMut<VfxEditorState>,
    mut dragging: Local<Option<usize>>,
) {
    let Some(t) = edit.target.clone() else {
        *dragging = None;
        return;
    };
    for (i, h) in &pressed {
        if *i == UiInteraction::Pressed {
            edit.selected = h.index;
            *dragging = Some(h.index);
        }
    }
    if !mouse.pressed(MouseButton::Left) {
        *dragging = None;
    }
    if let Some(index) = *dragging {
        let plot = handles.iter().find(|(_, h)| h.index == index).map(|(_, h)| h.plot);
        if let Some(n) = plot.and_then(|p| plots.get(p).ok()).and_then(|r| r.normalized) {
            let x = (n.x + 0.5).clamp(0.0, 1.0);
            let y = (0.5 - n.y).clamp(0.0, 1.0);
            let (lo, hi) = (edit.lo, edit.hi);
            if let Some(node) = state.doc.emitters.get_mut(t.emitter).and_then(|e| model::node_mut(&mut e.graph, t.node)) {
                match node.input_values.get_mut(t.prop) {
                    Some(PinValue::Curve(pts)) if index < pts.len() => {
                        let (min, max) = neighbour_bounds_curve(pts, index);
                        let p = CurvePoint { t: x.clamp(min, max), value: lo + y * (hi - lo), ease: pts[index].ease };
                        if pts[index] != p {
                            pts[index] = p;
                        }
                    }
                    Some(PinValue::Gradient(s)) if index < s.len() => {
                        let (min, max) = neighbour_bounds_gradient(s, index);
                        let nt = x.clamp(min, max);
                        if s[index].t != nt {
                            s[index].t = nt;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    for (i, action) in &actions {
        if *i != UiInteraction::Pressed {
            continue;
        }
        let Some(node) = state.doc.emitters.get_mut(t.emitter).and_then(|e| model::node_mut(&mut e.graph, t.node)) else {
            continue;
        };
        let sel = edit.selected;
        match node.input_values.get_mut(t.prop) {
            Some(PinValue::Curve(pts)) => {
                edit.selected = apply_curve_action(pts, sel, *action);
                if matches!(action, KeyAction::Reset) {
                    let (lo, hi) = plot_range(pts);
                    edit.lo = lo;
                    edit.hi = hi;
                }
            }
            Some(PinValue::Gradient(s)) => edit.selected = apply_gradient_action(s, sel, *action),
            _ => {}
        }
    }
}

/// Add a key halfway along the selected key's segment (or before the end),
/// delete the selected key (never the last one), or reset to the default.
/// Returns the new selection.
fn apply_curve_action(pts: &mut Vec<CurvePoint>, sel: usize, action: KeyAction) -> usize {
    match action {
        KeyAction::Add => {
            let table = CurveTable::new(pts);
            let (a, b) = match (pts.get(sel), pts.get(sel + 1)) {
                (Some(a), Some(b)) => (a.t, b.t),
                (Some(a), None) if sel > 0 => (pts[sel - 1].t, a.t),
                _ => (0.0, 1.0),
            };
            let t = (a + b) * 0.5;
            let at = if pts.get(sel + 1).is_some() { sel + 1 } else { sel.min(pts.len()) };
            let ease = pts.get(sel).map(|p| p.ease).unwrap_or_default();
            pts.insert(at, CurvePoint { t, value: table.sample(t), ease });
            at
        }
        KeyAction::Delete => {
            if pts.len() > 1 && sel < pts.len() {
                pts.remove(sel);
            }
            sel.min(pts.len().saturating_sub(1))
        }
        KeyAction::Reset => {
            *pts = model::default_curve();
            0
        }
        // A gradient preset; the curve popover has no such button.
        KeyAction::Blackbody => sel,
    }
}

fn apply_gradient_action(s: &mut Vec<GradientStop>, sel: usize, action: KeyAction) -> usize {
    match action {
        KeyAction::Add => {
            let table = GradientTable::new(s);
            let (a, b) = match (s.get(sel), s.get(sel + 1)) {
                (Some(a), Some(b)) => (a.t, b.t),
                (Some(a), None) if sel > 0 => (s[sel - 1].t, a.t),
                _ => (0.0, 1.0),
            };
            let t = (a + b) * 0.5;
            let at = if s.get(sel + 1).is_some() { sel + 1 } else { sel.min(s.len()) };
            s.insert(at, GradientStop { t, color: table.sample(t).to_array() });
            at
        }
        KeyAction::Delete => {
            if s.len() > 1 && sel < s.len() {
                s.remove(sel);
            }
            sel.min(s.len().saturating_sub(1))
        }
        KeyAction::Reset => {
            *s = model::default_gradient();
            0
        }
        KeyAction::Blackbody => {
            *s = renzora_particles::blackbody::fire_gradient();
            0
        }
    }
}

/// Close the popover on a click outside it, Escape, its X, or its node going
/// away (deleted, undone, another effect opened).
#[allow(clippy::too_many_arguments)]
fn dismiss(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    popovers: Query<(Entity, &RelativeCursorPosition, Ref<TablePopover>)>,
    surfaces: Query<(&RelativeCursorPosition, &Node), (With<OverlaySurface>, Without<TablePopover>)>,
    closes: Query<&UiInteraction, (With<PopoverClose>, Changed<UiInteraction>)>,
    state: Res<VfxEditorState>,
    mut edit: ResMut<TableEdit>,
    mut commands: Commands,
) {
    if popovers.is_empty() {
        if edit.target.is_some() {
            edit.target = None;
        }
        return;
    }
    let gone = edit.target.as_ref().is_none_or(|t| {
        state.doc.emitters.get(t.emitter).and_then(|e| model::node(&e.graph, t.node)).is_none()
    });
    let x = closes.iter().any(|i| *i == UiInteraction::Pressed);
    let pressed = mouse.just_pressed(MouseButton::Left) || mouse.just_pressed(MouseButton::Right);
    let over_child_surface = surfaces.iter().any(|(r, n)| r.cursor_over && n.display != Display::None);
    let mut close = gone || x || keys.just_pressed(KeyCode::Escape);
    for (_, rcp, p) in &popovers {
        if pressed && !p.is_added() && !rcp.cursor_over && !over_child_surface {
            close = true;
        }
    }
    if close {
        for (e, _, _) in &popovers {
            commands.entity(e).despawn();
        }
        edit.target = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(v: &[(f32, f32)]) -> Vec<CurvePoint> {
        v.iter().map(|&(t, value)| CurvePoint { t, value, ease: Ease::Linear }).collect()
    }

    #[test]
    fn the_plot_always_shows_zero_to_one_and_widens_for_keys_outside() {
        assert_eq!(plot_range(&pts(&[(0.0, 0.2), (1.0, 0.8)])), (0.0, 1.0));
        assert_eq!(plot_range(&pts(&[(0.0, -1.0), (1.0, 3.0)])), (-1.0, 3.0));
    }

    #[test]
    fn samples_are_normalised_into_the_plot() {
        let s = sample_normalized(&pts(&[(0.0, 0.0), (1.0, 2.0)]), (0.0, 2.0));
        assert_eq!(s.len(), SAMPLES);
        assert!(s[0].abs() < 1e-5 && (s[SAMPLES - 1] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn adding_a_key_splits_the_selected_segment_on_the_curve() {
        let mut p = pts(&[(0.0, 0.0), (1.0, 1.0)]);
        let sel = apply_curve_action(&mut p, 0, KeyAction::Add);
        assert_eq!(sel, 1);
        assert_eq!(p.len(), 3);
        assert!((p[1].t - 0.5).abs() < 1e-5 && (p[1].value - 0.5).abs() < 1e-5);
    }

    #[test]
    fn the_last_key_cannot_be_deleted() {
        let mut p = pts(&[(0.0, 1.0)]);
        apply_curve_action(&mut p, 0, KeyAction::Delete);
        assert_eq!(p.len(), 1);
        let mut s = model::default_gradient();
        let sel = apply_gradient_action(&mut s, 1, KeyAction::Delete);
        assert_eq!((s.len(), sel), (1, 0));
        apply_gradient_action(&mut s, 0, KeyAction::Delete);
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn a_new_stop_takes_the_colour_already_there() {
        let mut s = vec![
            GradientStop { t: 0.0, color: [0.0, 0.0, 0.0, 1.0] },
            GradientStop { t: 1.0, color: [1.0, 1.0, 1.0, 1.0] },
        ];
        apply_gradient_action(&mut s, 0, KeyAction::Add);
        assert!((s[1].color[0] - 0.5).abs() < 1e-5);
    }

    #[test]
    fn the_blackbody_preset_runs_hot_to_cold_and_fades_out() {
        let mut s = model::default_gradient();
        assert_eq!(apply_gradient_action(&mut s, 1, KeyAction::Blackbody), 0);
        let (first, last) = (s[0], s[s.len() - 1]);
        assert_eq!((first.t, last.t), (0.0, 1.0));
        // Hot is bright and yellow-white (green near red), cold dim and red.
        assert!(first.color[1] > 0.5 * first.color[0] && first.color[0] > 0.9, "{first:?}");
        assert!(last.color[1] < 0.2 * last.color[0].max(1e-6) || last.color[0] < 0.05, "{last:?}");
        assert_eq!(first.color[3], 1.0);
        assert_eq!(last.color[3], 0.0);
        assert!(s.iter().all(|g| g.color.iter().all(|c| (0.0..=1.0).contains(c))));
        // A curve has no such preset; the button is not on its popover.
        let mut p = pts(&[(0.0, 1.0), (1.0, 0.0)]);
        assert_eq!(apply_curve_action(&mut p, 1, KeyAction::Blackbody), 1);
        assert_eq!(p.len(), 2);
    }

    #[test]
    fn keys_are_held_between_their_neighbours() {
        let p = pts(&[(0.0, 0.0), (0.4, 0.0), (0.8, 0.0)]);
        assert_eq!(neighbour_bounds_curve(&p, 1), (0.0, 0.8));
        assert_eq!(neighbour_bounds_curve(&p, 0), (0.0, 0.4));
        assert_eq!(neighbour_bounds_curve(&p, 2), (0.4, 1.0));
    }
}
