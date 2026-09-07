//! Immediate-mode line gizmos: rotate rings, scale cubes and the translate
//! plane-drag squares.
//!
//! Bevy binds a gizmo group's render layer at config time, so drawing
//! camera-sized lines into each viewport's private layer needs one group *type*
//! per slot rather than one shared group — hence the macro below and the
//! four-way `match` in [`draw_line_gizmos`]. The mesh handles get the same
//! treatment for the same reason (see `meshes.rs`).

use bevy::camera::visibility::RenderLayers;
use bevy::gizmos::config::{GizmoConfig, GizmoConfigGroup, GizmoLineConfig};
use bevy::prelude::*;

use renzora::core::viewport_types::ViewportSettings;
use renzora_editor_framework::{EditorCamera, EditorSelection};

use crate::drag::angle_label_text;
use crate::modal_transform;
use crate::pivot::compute_gizmo_pivot;
use crate::types::{
    gizmo_basis, GizmoAxis, GizmoMesh, GizmoMode, GizmoRoot, GizmoSpace, GizmoState, PerSlotGizmo,
    PLANES,
};
use crate::{GIZMO_PLANE_SIZE, GIZMO_SIZE};

#[derive(Default, Reflect, GizmoConfigGroup)]
#[reflect(Default)]
pub struct OverlayGizmoGroup;

/// Dedicated group for transform gizmo line elements (rotate circles, scale
/// cubes). Always renders on top of the scene, independent of the
/// selection-bounding-box `on_top` setting.
#[derive(Default, Reflect, GizmoConfigGroup)]
#[reflect(Default)]
pub struct TransformGizmoGroup;

/// Dedicated group for the translate plane-drag squares, drawn with a thicker
/// line than the rest of the gizmo so the handles are easy to see and grab.
#[derive(Default, Reflect, GizmoConfigGroup)]
#[reflect(Default)]
pub struct PlaneGizmoGroup;

/// Dedicated group for entity name labels (stroke-font text gizmos). Kept
/// separate from `OverlayGizmoGroup` because that group's `depth_bias` is
/// toggled at runtime by the `selection_boundary_on_top` setting
/// (`update_selection_gizmo_depth`) — sharing it would make labels disappear
/// behind meshes whenever the user turns the selection box's on-top off.
/// Labels are always-on-top regardless.
#[derive(Default, Reflect, GizmoConfigGroup)]
#[reflect(Default)]
pub struct LabelGizmoGroup;

// Per-slot immediate-mode gizmo groups. Bevy binds a gizmo group's render layer
// at config time, so drawing the *camera-sized* rotate / scale / plane lines
// into each viewport's private layer (sized for its own camera) needs one group
// TYPE per slot rather than one shared group. `VIEWPORT_COUNT` is 4, so four of
// each are declared; `draw_line_gizmos` matches the slot index to the pair.
macro_rules! slot_gizmo_groups {
    ($($t:ident $p:ident),* $(,)?) => {
        $(
            #[derive(Default, Reflect, GizmoConfigGroup)]
            #[reflect(Default)]
            pub struct $t;
            #[derive(Default, Reflect, GizmoConfigGroup)]
            #[reflect(Default)]
            pub struct $p;
        )*
    };
}
slot_gizmo_groups!(
    SlotTransformGroup0 SlotPlaneGroup0,
    SlotTransformGroup1 SlotPlaneGroup1,
    SlotTransformGroup2 SlotPlaneGroup2,
    SlotTransformGroup3 SlotPlaneGroup3,
);

/// `GizmoConfig` for a per-slot transform/plane line group: always-on-top
/// (depth_bias -1), the given line width, rendered onto that slot's private
/// overlay layer so only that slot's camera draws it.
pub(crate) fn slot_line_config(slot: usize, width: f32) -> GizmoConfig {
    GizmoConfig {
        depth_bias: -1.0,
        line: GizmoLineConfig {
            width,
            ..default()
        },
        render_layers: RenderLayers::layer(
            renzora::core::viewport_types::VIEWPORT_3D_GIZMO_LAYER_BASE + slot,
        ),
        ..default()
    }
}

pub(crate) fn draw_line_gizmos(
    // Per-slot transform-line groups (rotate rings / scale cubes) and plane-drag
    // groups, each bound to its slot's private overlay layer in the plugin
    // `build`. Grouped into two tuple params to stay under Bevy's 16-param system
    // limit (a tuple of system params counts as one param).
    mut t: (
        Gizmos<SlotTransformGroup0>,
        Gizmos<SlotTransformGroup1>,
        Gizmos<SlotTransformGroup2>,
        Gizmos<SlotTransformGroup3>,
    ),
    mut pl: (
        Gizmos<SlotPlaneGroup0>,
        Gizmos<SlotPlaneGroup1>,
        Gizmos<SlotPlaneGroup2>,
        Gizmos<SlotPlaneGroup3>,
    ),
    per_slot: Res<PerSlotGizmo>,
    mode: Res<GizmoMode>,
    vp_space: Option<Res<renzora::core::viewport_types::ViewportGizmoSpace>>,
    gizmo_state: Res<GizmoState>,
    selection: Res<EditorSelection>,
    modal: Res<modal_transform::ModalTransformState>,
    collider_edit: Option<Res<renzora_physics::ColliderEditMode>>,
    transform_q: Query<
        &GlobalTransform,
        (
            Without<EditorCamera>,
            Without<GizmoRoot>,
            Without<GizmoMesh>,
        ),
    >,
    aabbs: Query<(Option<&bevy::camera::primitives::Aabb>, &GlobalTransform), With<Mesh3d>>,
    children_q: Query<&Children>,
    camera_q: Query<&GlobalTransform, With<EditorCamera>>,
    viewport_settings: Option<Res<ViewportSettings>>,
    viewports: Option<Res<renzora::core::viewport_types::Viewports>>,
) {
    // Modal transforms (G/R/S) take over input — hide the tool-mode handles so
    // they don't sit under the modal HUD while dragging. The modal *scale* HUD
    // (reference circle + line to cursor) is drawn separately by the viewport's
    // `render_modal_scale_hud`, reading `ModalTransformHud`.
    if modal.active {
        return;
    }
    if collider_edit.map(|c| c.active).unwrap_or(false) {
        return;
    }

    let Some(selected) = selection.get() else {
        return;
    };
    let Ok(sel_gt) = transform_q.get(selected) else {
        return;
    };
    let pivot_bottom = viewport_settings
        .as_ref()
        .map(|s| s.gizmo_pivot_bottom)
        .unwrap_or(true);
    let pos = compute_gizmo_pivot(selected, &aabbs, &children_q, sel_gt, pivot_bottom);

    if matches!(*mode, GizmoMode::Select | GizmoMode::None) {
        return;
    }

    // The handle basis is resolved per slot below (each viewport can be in a
    // different Local/World space); keep the object's world rotation here.
    let sel_rot = sel_gt.rotation();
    let slot_space = |i: usize| -> GizmoSpace {
        let local = vp_space
            .as_ref()
            .and_then(|s| s.local.get(i).copied())
            .unwrap_or(false);
        if local {
            GizmoSpace::Local
        } else {
            GizmoSpace::World
        }
    };
    let active = gizmo_state.active_axis.or(gizmo_state.hovered_axis);
    // While actively dragging, fade the line elements (rings, scale lines/cubes,
    // plane squares) so the object underneath stays visible. The rotation pie and
    // angle label are deliberately left at full opacity — they're the drag readout.
    // The fade amount is the user-configurable gizmo drag opacity (Settings →
    // Viewport), matching the mesh handles.
    let drag_fade = if gizmo_state.active_axis.is_some() {
        viewport_settings
            .map(|v| v.gizmo_drag_opacity)
            .unwrap_or(0.25)
            .clamp(0.0, 1.0)
    } else {
        1.0
    };

    // Draw each docked slot's rotate / scale / plane lines into ITS OWN group
    // (bound to that slot's private overlay layer), scaled to ITS OWN camera via
    // `PerSlotGizmo` — so every viewport shows a handle the right size, matching
    // its per-slot mesh handle. The rotation-drag readout (pie + angle label) is
    // drawn only for the focused slot (where the drag is happening) so it isn't
    // duplicated across views.
    let focused = viewports.as_ref().map(|v| v.focused).unwrap_or(0);
    let focused_cam = camera_q.single().ok().map(|gt| gt.translation());

    use renzora::core::viewport_types::VIEWPORT_COUNT;
    for i in 0..VIEWPORT_COUNT {
        if !per_slot.draw[i] {
            continue;
        }
        let gs = per_slot.scale[i];
        let signs = per_slot.signs[i];
        let basis = gizmo_basis(slot_space(i), *mode, sel_rot);
        let readout = if i == focused { focused_cam } else { None };
        match i {
            0 => draw_transform_lines(
                &mut t.0, &mut pl.0, *mode, pos, basis, active, gs, signs, drag_fade,
                &gizmo_state, readout,
            ),
            1 => draw_transform_lines(
                &mut t.1, &mut pl.1, *mode, pos, basis, active, gs, signs, drag_fade,
                &gizmo_state, readout,
            ),
            2 => draw_transform_lines(
                &mut t.2, &mut pl.2, *mode, pos, basis, active, gs, signs, drag_fade,
                &gizmo_state, readout,
            ),
            3 => draw_transform_lines(
                &mut t.3, &mut pl.3, *mode, pos, basis, active, gs, signs, drag_fade,
                &gizmo_state, readout,
            ),
            _ => {}
        }
    }
}

/// Draw one slot's rotate / scale / plane tool-line gizmos into its own group
/// pair. `gs` is this slot's gizmo world scale and `signs` its camera-facing axis
/// flips (both from `PerSlotGizmo`); `readout` carries the focused camera's world
/// position when this is the focused slot, gating the rotation-drag pie + angle
/// label so they draw once, not once per view.
#[allow(clippy::too_many_arguments)]
fn draw_transform_lines<G: GizmoConfigGroup, P: GizmoConfigGroup>(
    gizmos: &mut Gizmos<G>,
    plane_gizmos: &mut Gizmos<P>,
    mode: GizmoMode,
    pos: Vec3,
    basis: Quat,
    active: Option<GizmoAxis>,
    gs: f32,
    signs: Vec3,
    drag_fade: f32,
    gizmo_state: &GizmoState,
    readout: Option<Vec3>,
) {
    let highlight = Color::srgb(1.0, 1.0, 0.3);
    let x_base = Color::srgb(1.0, 0.15, 0.15);
    let y_base = Color::srgb(0.15, 1.0, 0.15);
    let z_base = Color::srgb(0.2, 0.3, 1.0);

    match mode {
        GizmoMode::Select | GizmoMode::None => {}
        GizmoMode::Translate => {
            // Plane-drag handles: a square bracket in each axis pair's plane
            // whose inner corner sits at the gizmo origin so two of its edges
            // run *along* the axis lines (attached to them). It extends into the
            // camera-facing quadrant (signed axes), matching the arrows. Pick
            // region in `gizmo_hover_detect` mirrors this exactly. Colors blend
            // the two axis colors (XY=yellow, XZ=magenta, YZ=cyan); the
            // active/hovered plane turns white.
            let side = GIZMO_PLANE_SIZE * gs;
            for plane in PLANES {
                let base = match plane {
                    GizmoAxis::XY => Color::srgb(1.0, 0.9, 0.1),
                    GizmoAxis::XZ => Color::srgb(1.0, 0.2, 0.9),
                    GizmoAxis::YZ => Color::srgb(0.1, 0.9, 0.95),
                    _ => continue,
                };
                let color = if active == Some(plane) { Color::WHITE } else { base };
                let color = color.with_alpha(drag_fade);
                let (sa, sb) = plane.signed_plane_axes(signs).unwrap();
                let a = basis * sa;
                let b = basis * sb;
                let c0 = pos;
                let c1 = pos + a * side;
                let c2 = pos + a * side + b * side;
                let c3 = pos + b * side;
                plane_gizmos.line(c0, c1, color);
                plane_gizmos.line(c1, c2, color);
                plane_gizmos.line(c2, c3, color);
                plane_gizmos.line(c3, c0, color);
            }
        }
        GizmoMode::Rotate => {
            let radius = GIZMO_SIZE * gs * 0.7;
            let x_color = if matches!(active, Some(GizmoAxis::X)) {
                highlight
            } else {
                x_base
            };
            let y_color = if matches!(active, Some(GizmoAxis::Y)) {
                highlight
            } else {
                y_base
            };
            let z_color = if matches!(active, Some(GizmoAxis::Z)) {
                highlight
            } else {
                z_base
            };
            let (x_color, y_color, z_color) = (
                x_color.with_alpha(drag_fade),
                y_color.with_alpha(drag_fade),
                z_color.with_alpha(drag_fade),
            );

            gizmos.circle(
                Isometry3d::new(pos, basis * Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
                radius,
                x_color,
            );
            gizmos.circle(
                Isometry3d::new(pos, basis * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
                radius,
                y_color,
            );
            gizmos.circle(Isometry3d::new(pos, basis), radius, z_color);

            // While dragging a ring, fill the swept angle with a pie sector and
            // show the angle in degrees, both always-on-top so they read over
            // the object. Only the focused slot draws this readout.
            if let Some(cam_pos) = readout {
                if let Some(active_axis) = gizmo_state.active_axis {
                    // Test the value that is actually drawn, not the raw
                    // accumulator. With snap on, a drag shorter than half a step
                    // applies no rotation, and guarding on the raw angle drew a
                    // zero-width pie and a `0.0` label for it — a readout the
                    // object was not following.
                    if gizmo_state.drag_angle_snapped.abs() > 1e-4 {
                        draw_rotation_pie(
                            gizmos,
                            pos,
                            basis * active_axis.direction(),
                            gizmo_state.drag_angle_snapped,
                            radius,
                            highlight,
                        );
                        draw_angle_label(
                            gizmos,
                            pos,
                            cam_pos,
                            gizmo_state.drag_angle_snapped,
                            radius,
                            highlight,
                        );
                    }
                }
            }
        }
        GizmoMode::Scale => {
            let scale_size = GIZMO_SIZE * gs;
            let x_color = if matches!(active, Some(GizmoAxis::X)) {
                highlight
            } else {
                x_base
            };
            let y_color = if matches!(active, Some(GizmoAxis::Y)) {
                highlight
            } else {
                y_base
            };
            let z_color = if matches!(active, Some(GizmoAxis::Z)) {
                highlight
            } else {
                z_base
            };
            let (x_color, y_color, z_color) = (
                x_color.with_alpha(drag_fade),
                y_color.with_alpha(drag_fade),
                z_color.with_alpha(drag_fade),
            );

            // Lines from center to cube tips (oriented to the active space).
            let ax = basis * Vec3::X;
            let ay = basis * Vec3::Y;
            let az = basis * Vec3::Z;
            gizmos.line(pos, pos + ax * scale_size, x_color);
            gizmos.line(pos, pos + ay * scale_size, y_color);
            gizmos.line(pos, pos + az * scale_size, z_color);

            // Cube wireframes at tips
            let cube_half = 0.075 * gs;
            for (axis_dir, color) in [(ax, x_color), (ay, y_color), (az, z_color)] {
                let c = pos + axis_dir * scale_size;
                let h = Vec3::splat(cube_half);
                // Draw 12 edges of the cube
                for &(a, b) in &[
                    (Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, -1.0, -1.0)),
                    (Vec3::new(1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, -1.0)),
                    (Vec3::new(1.0, 1.0, -1.0), Vec3::new(-1.0, 1.0, -1.0)),
                    (Vec3::new(-1.0, 1.0, -1.0), Vec3::new(-1.0, -1.0, -1.0)),
                    (Vec3::new(-1.0, -1.0, 1.0), Vec3::new(1.0, -1.0, 1.0)),
                    (Vec3::new(1.0, -1.0, 1.0), Vec3::new(1.0, 1.0, 1.0)),
                    (Vec3::new(1.0, 1.0, 1.0), Vec3::new(-1.0, 1.0, 1.0)),
                    (Vec3::new(-1.0, 1.0, 1.0), Vec3::new(-1.0, -1.0, 1.0)),
                    (Vec3::new(-1.0, -1.0, -1.0), Vec3::new(-1.0, -1.0, 1.0)),
                    (Vec3::new(1.0, -1.0, -1.0), Vec3::new(1.0, -1.0, 1.0)),
                    (Vec3::new(1.0, 1.0, -1.0), Vec3::new(1.0, 1.0, 1.0)),
                    (Vec3::new(-1.0, 1.0, -1.0), Vec3::new(-1.0, 1.0, 1.0)),
                ] {
                    gizmos.line(c + basis * (a * h), c + basis * (b * h), color);
                }
            }
        }
    }
}

/// Draw a "rotation pie": a filled-looking sector on the rotation plane that
/// sweeps from a stable in-plane reference edge by `angle`, conveying how far
/// the object has been rotated. Bevy gizmos can't fill, so the wedge is faked
/// with an arc, two solid edges, and faint radial spokes. Generic over the
/// gizmo group so both the tool gizmo (`TransformGizmoGroup`) and the modal
/// overlay (`OverlayGizmoGroup`) can use it.
pub(crate) fn draw_rotation_pie<C: GizmoConfigGroup>(
    gizmos: &mut Gizmos<C>,
    pivot: Vec3,
    normal: Vec3,
    angle: f32,
    radius: f32,
    color: Color,
) {
    let n = normal.normalize_or_zero();
    if n.length_squared() < 1e-6 || radius <= 0.0 || angle.abs() < 1e-4 {
        return;
    }
    // Stable in-plane reference for the "0°" edge (avoid a near-parallel hint).
    let hint = if n.dot(Vec3::Y).abs() > 0.99 {
        Vec3::X
    } else {
        Vec3::Y
    };
    let u = (hint - n * hint.dot(n)).normalize_or_zero();
    if u.length_squared() < 1e-6 {
        return;
    }
    let v = n.cross(u);

    // ~7.5° per segment, at least one.
    let segs = (angle.abs() / 0.13).ceil().max(1.0) as i32;
    let fill = color.with_alpha(0.18);
    let mut prev = pivot + u * radius;
    gizmos.line(pivot, prev, color); // start edge
    for i in 1..=segs {
        let t = angle * (i as f32 / segs as f32);
        let p = pivot + (u * t.cos() + v * t.sin()) * radius;
        gizmos.line(prev, p, color); // arc
        gizmos.line(pivot, p, fill); // radial fill spoke
        prev = p;
    }
    gizmos.line(pivot, prev, color); // end edge
}

/// Draw the rotation amount in degrees as a camera-facing stroke-text label at
/// `pivot`. Uses the same always-on-top group as the pie so it reads over the
/// object. (Bevy's stroke font is ASCII-only, so the `°` is dropped — the number
/// is the degrees.)
pub(crate) fn draw_angle_label<C: GizmoConfigGroup>(
    gizmos: &mut Gizmos<C>,
    pivot: Vec3,
    cam_pos: Vec3,
    radians: f32,
    radius: f32,
    color: Color,
) {
    let forward = (cam_pos - pivot).normalize_or_zero();
    if forward == Vec3::ZERO {
        return;
    }
    let right = Vec3::Y.cross(forward).normalize_or_zero();
    if right == Vec3::ZERO {
        return;
    }
    let up = forward.cross(right);
    let rot = Quat::from_mat3(&Mat3::from_cols(right, up, forward));
    let text = angle_label_text(radians);
    let size = (radius * 0.35).max(0.05);
    gizmos.text(
        Isometry3d::new(pivot, rot),
        text.as_str(),
        size,
        Vec2::new(0.0, -0.5),
        color,
    );
}
