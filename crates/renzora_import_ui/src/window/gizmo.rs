//! The preview viewport's own overlay controls: an axis-orientation gizmo in
//! the top-left corner, and a zoom cluster below it.
//!
//! Deliberately *not* the editor viewport's
//! [`axis_gizmo`](renzora_viewport::axis_gizmo). That one is wired through the
//! viewport slot system — `Viewports.slots[i]`, `CameraOrbitSnapshot`,
//! `NavOverlayState`'s atomics and `ViewportSettings::pending_view_angle` — none
//! of which the import preview has or should acquire. Reusing it would mean
//! `renzora_import_ui` depending on `renzora_viewport` and inventing a fake slot
//! for a camera that is not a viewport.
//!
//! What is shared is the part worth sharing: the projection is the same maths,
//! the axis colours are the same colours, and the cluster behaves the same way,
//! so the two do not need separate muscle memory. Everything here reads and
//! writes [`ImportPreviewOrbit`](crate::preview3d::ImportPreviewOrbit) directly.

use std::f32::consts::{FRAC_PI_2, PI};

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::math::Rot2;
use bevy::prelude::*;
use bevy::ui::{FocusPolicy, RelativeCursorPosition, UiTransform};

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::theme::*;
use renzora_ember::widgets::OverlaySurface;

use crate::matpreview::MaterialPreviewOrbit;
use crate::overlay::ImportOverlayState;
use crate::preview3d::ImportPreviewOrbit;

use super::rows::has_staged;
use super::widgets::hover_cursor;
use super::{ImportNav, ImportTab};

/// Whether the centre is showing the material sphere rather than the model.
///
/// The overlays serve both — a sphere has an orientation and a distance exactly
/// as a model does, and the toolbar's switches now drive both rigs — so the
/// question is not *whether* to draw them but which camera they speak to.
fn on_material(nav: Option<&ImportNav>, state: Option<&ImportOverlayState>) -> bool {
    state.is_some_and(|s| !s.staged.is_empty())
        && nav.is_some_and(|n| n.tab == ImportTab::Materials && n.sel_material.is_some())
}

/// Overall size of the gizmo cluster, in logical pixels. Comfortably larger
/// than the editor viewport's: this one is the *only* way to reorient a preview
/// whose camera has no keyboard shortcuts, so its tips are click targets rather
/// than an orientation readout you glance at.
const SIZE: f32 = 116.0;
/// Half-length of the projected axes.
const AXIS_LEN: f32 = SIZE / 2.0 - 18.0;
/// Container-local centre.
const CENTRE: f32 = SIZE / 2.0;
/// Tip diameters, positive and negative.
const POS_D: f32 = 24.0;
const NEG_D: f32 = 15.0;
/// Inset of the overlays from the viewport's edges. Wider than a token margin:
/// these float over a rendered image rather than sitting in a panel, and pinned
/// tight to the corner they read as something clipped by it.
const MARGIN: f32 = 18.0;

/// One wheel notch's worth of zoom, so a button press and a scroll step move by
/// the same proportion. `preview3d`'s wheel handler uses `0.88^delta`.
const ZOOM_STEP: f32 = 0.88;

#[derive(Component)]
struct GizmoRoot;
#[derive(Component)]
struct GizmoBackplate;

#[derive(Component)]
struct AxisLine {
    dir: Vec3,
    color: (u8, u8, u8),
}

#[derive(Component)]
struct AxisTip {
    dir: Vec3,
    /// Where the camera goes when this tip is clicked.
    yaw: f32,
    pitch: f32,
    positive: bool,
    color: (u8, u8, u8),
}

/// What a zoom button does. `Fit` is the third because "I have zoomed into the
/// wrong place and want out" is at least as common as either direction, and
/// without it the only way back was to switch staged file and switch back.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum ZoomBtn {
    In,
    Out,
    Fit,
}

/// True while the backplate is latched for a drag. A `Local` would not do: the
/// press and the release are seen by the same system, but the drag has to
/// survive the cursor leaving the 74px circle, which it does immediately.
#[derive(Resource, Default)]
struct GizmoDragging(bool);

pub(crate) fn register(app: &mut App) {
    app.init_resource::<GizmoDragging>()
        .add_systems(Update, (gizmo_layout, gizmo_input, zoom_click));
}

fn rgba((r, g, b): (u8, u8, u8), a: f32) -> Color {
    Color::srgba(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a)
}

/// Project an axis direction to a (screen-space offset, depth) pair. The same
/// yaw-then-pitch rotation the editor's gizmo uses, so a given orbit angle draws
/// the identical cluster in both.
fn project(dir: Vec3, cy: f32, sy: f32, cp: f32, sp: f32) -> (Vec2, f32) {
    let r = Vec3::new(dir.x * cy + dir.z * sy, dir.y, -dir.x * sy + dir.z * cy);
    let v = Vec3::new(r.x, r.y * cp + r.z * sp, -r.y * sp + r.z * cp);
    (Vec2::new(v.x * AXIS_LEN, -v.y * AXIS_LEN), v.z)
}

/// Build the overlay column for the top-right of the preview: the axis cluster
/// with the zoom buttons under it. Returns the column for the centre to host.
///
/// Top-*right*, where the editor viewport keeps its own gizmo and its own
/// navigation buttons — these are the same controls and belong in the same
/// place. The lighting panel gave the corner up and moved to the bottom.
pub(super) fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // A transparent layer over the whole viewport, holding two independently
    // placed clusters. It passes pointer input straight through — only the
    // clusters themselves block — so it costs the camera nothing.
    let layer = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                ..default()
            },
            FocusPolicy::Pass,
            bevy::picking::Pickable::IGNORE,
        ))
        .id();
    // Only while something is staged. It used to hide on the Materials tab as
    // well, on the grounds that a sphere has no meaningful "front" — but it
    // does have a seam and poles, which is the whole reason it is a UV sphere,
    // and telling which way round it is turned is exactly what the gizmo is
    // for.
    renzora_ember::reactive::tracked::bind_display(commands, layer, has_staged);

    let gizmo = build_gizmo(commands, fonts);
    commands.entity(gizmo).insert(Node {
        position_type: PositionType::Absolute,
        right: Val::Px(MARGIN),
        top: Val::Px(MARGIN),
        width: Val::Px(SIZE),
        height: Val::Px(SIZE),
        ..default()
    });

    // The zoom buttons are pinned to the *middle* of the right edge rather than
    // hung under the gizmo. They are not part of it — one says which way the
    // model is facing, the other how close it is — and stacked directly beneath
    // it they read as its tail, a lopsided column growing out of one corner.
    // Centred, they sit where a hand rests.
    //
    // A full-height column with `justify_content: Center` does the centring
    // without a transform; its width is its content's, so it does not span the
    // viewport and cannot swallow a drag.
    let zoom_slot = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(MARGIN),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                ..default()
            },
            FocusPolicy::Pass,
            bevy::picking::Pickable::IGNORE,
        ))
        .id();
    let zoom = build_zoom(commands, fonts);
    commands.entity(zoom_slot).add_child(zoom);

    commands.entity(layer).add_children(&[gizmo, zoom_slot]);
    layer
}

fn build_gizmo(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            // The node is replaced by `build`, which places it; this is only
            // here so the entity has one before its children are attached.
            Node::default(),
            RelativeCursorPosition::default(),
            OverlaySurface,
            Interaction::default(),
            // The preview camera reads its own node's `Interaction` to decide
            // whether to orbit, so without this a drag on the gizmo would move
            // the camera twice.
            FocusPolicy::Block,
            GizmoRoot,
            Name::new("import-preview-axis-gizmo"),
        ))
        .id();

    let backplate = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(SIZE),
                height: Val::Px(SIZE),
                border_radius: BorderRadius::all(Val::Px(SIZE / 2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.22)),
            ZIndex(-100),
            bevy::picking::Pickable::IGNORE,
            GizmoBackplate,
        ))
        .id();
    commands.entity(root).add_child(backplate);

    // (dir, colour, label, target yaw, target pitch, positive)
    let axes: [(Vec3, (u8, u8, u8), &str, f32, f32, bool); 6] = [
        (Vec3::X, (237, 76, 92), "X", FRAC_PI_2, 0.0, true),
        (Vec3::Y, (139, 201, 63), "Y", 0.0, FRAC_PI_2, true),
        (Vec3::Z, (68, 138, 255), "Z", 0.0, 0.0, true),
        (Vec3::NEG_X, (150, 50, 60), "", -FRAC_PI_2, 0.0, false),
        (Vec3::NEG_Y, (80, 120, 40), "", 0.0, -FRAC_PI_2, false),
        (Vec3::NEG_Z, (40, 80, 150), "", PI, 0.0, false),
    ];

    // Lines under the tips, and only for the positive half — the negative axes
    // are drawn as bare balls, which is what keeps the cluster readable when it
    // is edge-on.
    for (dir, color, _, _, _, positive) in axes {
        if !positive {
            continue;
        }
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(0.0),
                height: Val::Px(3.0),
                border_radius: BorderRadius::all(Val::Px(1.5)),
                ..default()
            },
            BackgroundColor(rgb(color)),
            UiTransform::default(),
            ZIndex(0),
            bevy::picking::Pickable::IGNORE,
            AxisLine { dir, color },
            ChildOf(root),
        ));
    }

    for (dir, color, label, yaw, pitch, positive) in axes {
        let d = if positive { POS_D } else { NEG_D };
        let tip = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(d),
                    height: Val::Px(d),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(d / 2.0)),
                    ..default()
                },
                BackgroundColor(rgb(color)),
                ZIndex(100),
                Interaction::default(),
                hover_cursor(),
                AxisTip {
                    dir,
                    yaw,
                    pitch,
                    positive,
                    color,
                },
                ChildOf(root),
            ))
            .id();
        if positive && !label.is_empty() {
            let t = commands
                .spawn((
                    Text::new(label),
                    ui_font(&fonts.ui, 12.0),
                    TextColor(Color::WHITE),
                    bevy::picking::Pickable::IGNORE,
                ))
                .id();
            commands.entity(tip).add_child(t);
        }
    }
    root
}

fn build_zoom(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(panel_bg()).with_alpha(0.88)),
            BorderColor::all(rgb(border())),
            FocusPolicy::Block,
            OverlaySurface,
        ))
        .id();
    let mut kids = Vec::new();
    for (which, icon, tip) in [
        (ZoomBtn::In, "plus", "Zoom in"),
        (ZoomBtn::Out, "minus", "Zoom out"),
        (ZoomBtn::Fit, "crosshair-simple", "Fit to view"),
    ] {
        kids.push(zoom_button(commands, fonts, which, icon, tip));
    }
    commands.entity(col).add_children(&kids);
    col
}

fn zoom_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    which: ZoomBtn,
    icon: &str,
    tip: &str,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(26.0),
                height: Val::Px(26.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            FocusPolicy::Block,
            hover_cursor(),
            which,
            renzora_ember::widgets::HoverTooltip::new(tip),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_bg(commands, btn, move |w| {
        if matches!(
            w.get::<Interaction>(btn),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        ) {
            rgb(hover_bg())
        } else {
            Color::NONE
        }
    });
    let ic = icon_text(commands, &fonts.phosphor, icon, text_primary(), 13.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    commands.entity(btn).add_child(ic);
    btn
}

/// Reposition, fade and z-order the tips and lines from the preview's orbit
/// each frame.
fn gizmo_layout(
    orbit: Option<Res<ImportPreviewOrbit>>,
    mat: Option<Res<MaterialPreviewOrbit>>,
    nav: Option<Res<ImportNav>>,
    state: Option<Res<ImportOverlayState>>,
    mut tips: Query<
        (&AxisTip, &mut Node, &mut BackgroundColor, &mut ZIndex),
        Without<AxisLine>,
    >,
    mut lines: Query<
        (
            &AxisLine,
            &mut Node,
            &mut UiTransform,
            &mut BackgroundColor,
            &mut ZIndex,
        ),
        Without<AxisTip>,
    >,
) {
    let Some(orbit) = orbit else { return };
    // The *smoothed* angle for the model, not the target: that camera eases
    // toward its target over several frames, and a gizmo drawn from the target
    // would arrive at a snapped view before the model it is describing does.
    // The material sphere does not ease, so its live angle is its displayed one.
    let (yaw, pitch) = if on_material(nav.as_deref(), state.as_deref()) {
        match mat.as_deref() {
            Some(m) => (m.yaw(), m.pitch()),
            None => return,
        }
    } else {
        (orbit.smooth_yaw(), orbit.smooth_pitch())
    };
    let (cy, sy) = (yaw.cos(), yaw.sin());
    let (cp, sp) = (pitch.cos(), pitch.sin());

    for (tip, mut node, mut bg, mut z) in &mut tips {
        let (off, depth) = project(tip.dir, cy, sy, cp, sp);
        let d = if tip.positive { POS_D } else { NEG_D };
        node.left = Val::Px(CENTRE + off.x - d / 2.0);
        node.top = Val::Px(CENTRE + off.y - d / 2.0);
        bg.0 = rgba(tip.color, if depth < -0.1 { 0.45 } else { 1.0 });
        *z = ZIndex(100 + (depth * 10.0) as i32);
    }

    for (line, mut node, mut tf, mut bg, mut z) in &mut lines {
        let (off, depth) = project(line.dir, cy, sy, cp, sp);
        let len = off.length();
        node.width = Val::Px(len);
        // Centred on the midpoint, then rotated about its own centre so it
        // spans centre → tip.
        node.left = Val::Px(CENTRE + off.x / 2.0 - len / 2.0);
        node.top = Val::Px(CENTRE + off.y / 2.0 - 1.5);
        *tf = UiTransform::from_rotation(Rot2::radians(off.y.atan2(off.x)));
        bg.0 = rgba(line.color, if depth < -0.1 { 0.4 } else { 0.9 });
        *z = ZIndex(10 + (depth * 10.0) as i32);
    }
}

/// Tip click snaps the view; a drag anywhere else on the backplate orbits it.
#[allow(clippy::too_many_arguments)]
fn gizmo_input(
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    mut dragging: ResMut<GizmoDragging>,
    mut orbit: Option<ResMut<ImportPreviewOrbit>>,
    mut mat: Option<ResMut<MaterialPreviewOrbit>>,
    nav: Option<Res<ImportNav>>,
    state: Option<Res<ImportOverlayState>>,
    tips: Query<(&AxisTip, &Interaction)>,
    roots: Query<&Interaction, With<GizmoRoot>>,
    mut plates: Query<&mut BackgroundColor, With<GizmoBackplate>>,
) {
    let Some(orbit) = orbit.as_mut() else { return };
    let material = on_material(nav.as_deref(), state.as_deref());

    if mouse.just_pressed(MouseButton::Left) {
        let snap = tips
            .iter()
            .find(|(_, i)| **i == Interaction::Pressed)
            .map(|(tip, _)| (tip.yaw, tip.pitch));
        if let Some((yaw, pitch)) = snap {
            if material {
                if let Some(m) = mat.as_mut() {
                    m.set_view(yaw, pitch);
                }
            } else {
                orbit.yaw = yaw;
                orbit.pitch = pitch;
                orbit.ease_to_view();
            }
        } else if roots
            .iter()
            .any(|i| matches!(i, Interaction::Hovered | Interaction::Pressed))
        {
            dragging.0 = true;
        }
    }
    if !mouse.pressed(MouseButton::Left) {
        dragging.0 = false;
    }
    for mut bg in &mut plates {
        let a = if dragging.0 { 0.38 } else { 0.22 };
        bg.0 = Color::srgba(0.0, 0.0, 0.0, a);
    }
    if !dragging.0 {
        return;
    }
    let d = motion.delta;
    if d == Vec2::ZERO {
        return;
    }
    if material {
        if let Some(m) = mat.as_mut() {
            m.nudge(d.x, d.y);
        }
        return;
    }
    // Matches the viewport's orbit sensitivity so the two feel the same.
    orbit.yaw -= d.x * 0.005;
    orbit.pitch = (orbit.pitch + d.y * 0.005).clamp(-1.54, 1.54);
    orbit.ease_to_view();
}

/// Zoom in, out, or back to the distance framing chose.
fn zoom_click(
    q: Query<(&Interaction, &ZoomBtn), Changed<Interaction>>,
    mut orbit: Option<ResMut<ImportPreviewOrbit>>,
    mut mat: Option<ResMut<MaterialPreviewOrbit>>,
    nav: Option<Res<ImportNav>>,
    state: Option<Res<ImportOverlayState>>,
) {
    let Some(orbit) = orbit.as_mut() else { return };
    let Some(which) = q
        .iter()
        .find(|(i, _)| **i == Interaction::Pressed)
        .map(|(_, b)| *b)
    else {
        return;
    };
    if on_material(nav.as_deref(), state.as_deref()) {
        if let Some(m) = mat.as_mut() {
            // The sphere's limits are absolute — it is always the same size —
            // so it needs no framed distance to measure against.
            m.zoom(match which {
                ZoomBtn::In => Some(ZOOM_STEP),
                ZoomBtn::Out => Some(1.0 / ZOOM_STEP),
                ZoomBtn::Fit => None,
            });
        }
        return;
    }
    // The same proportional step and the same bounds as the wheel, so a button
    // and a notch are interchangeable and neither can leave the model behind.
    let framed = orbit.framed_distance().max(1e-3);
    match which {
        ZoomBtn::In => orbit.distance = (orbit.distance * ZOOM_STEP).clamp(framed * 0.02, framed * 8.0),
        ZoomBtn::Out => {
            orbit.distance = (orbit.distance / ZOOM_STEP).clamp(framed * 0.02, framed * 8.0)
        }
        ZoomBtn::Fit => {
            orbit.distance = framed;
            orbit.target = Vec3::ZERO;
        }
    }
    orbit.ease_to_view();
}
