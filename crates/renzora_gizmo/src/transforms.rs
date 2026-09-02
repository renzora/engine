//! Keeping the gizmo where the selection is.
//!
//! Every viewport slot gets its own handle set, so this sizes each one against
//! that slot's own camera and resolves the handle basis in that slot's own
//! Local/World space. Only the focused slot's numbers feed back into
//! [`GizmoState`], because interaction always happens in the focused view.

use bevy::prelude::*;

use renzora::core::viewport_types::ViewportSettings;
use renzora_editor_framework::EditorSelection;

use crate::material::{GizmoMaterial, GizmoMaterials};
use crate::modal_transform;
use crate::pivot::compute_gizmo_pivot;
use crate::types::{
    gizmo_basis, GizmoAxis, GizmoMesh, GizmoMode, GizmoPart, GizmoRoot, GizmoSlot, GizmoSpace,
    GizmoState, PerSlotGizmo,
};
use crate::GIZMO_SCALE_REF_DIST;

/// Mirror the focused viewport's per-slot Local/World choice into the global
/// [`GizmoSpace`] resource. The gizmo's drag math and hit-test (and any other
/// `Res<GizmoSpace>` reader) always operate on the focused view, so this keeps
/// them consistent with whichever viewport the cursor is in while each viewport
/// still displays its own space.
pub(crate) fn mirror_focused_gizmo_space(
    viewports: Option<Res<renzora::core::viewport_types::Viewports>>,
    vp_space: Option<Res<renzora::core::viewport_types::ViewportGizmoSpace>>,
    mut space: ResMut<GizmoSpace>,
) {
    let focused = viewports.as_ref().map(|v| v.focused).unwrap_or(0);
    let local = vp_space
        .as_ref()
        .and_then(|s| s.local.get(focused).copied())
        .unwrap_or(false);
    let want = if local {
        GizmoSpace::Local
    } else {
        GizmoSpace::World
    };
    if *space != want {
        *space = want;
    }
}

pub(crate) fn update_gizmo_transforms(
    selection: Res<EditorSelection>,
    mode: Res<GizmoMode>,
    modal: Res<modal_transform::ModalTransformState>,
    collider_edit: Option<Res<renzora_physics::ColliderEditMode>>,
    viewports: Option<Res<renzora::core::viewport_types::Viewports>>,
    viewport_settings: Option<Res<ViewportSettings>>,
    vp_space: Option<Res<renzora::core::viewport_types::ViewportGizmoSpace>>,
    mut gizmo_state: ResMut<GizmoState>,
    mut per_slot: ResMut<PerSlotGizmo>,
    transforms: Query<&GlobalTransform, (Without<GizmoMesh>, Without<GizmoRoot>)>,
    aabbs: Query<(Option<&bevy::camera::primitives::Aabb>, &GlobalTransform), With<Mesh3d>>,
    children_q: Query<&Children>,
    mut gizmo_roots: Query<(&GizmoSlot, &mut Transform, &mut Visibility), With<GizmoRoot>>,
    mut gizmo_parts: Query<(&GizmoPart, &mut Visibility), (With<GizmoMesh>, Without<GizmoRoot>)>,
    cameras: Query<
        (&renzora::core::ViewportCamera, &GlobalTransform),
        Without<GizmoRoot>,
    >,
) {
    use renzora::core::viewport_types::VIEWPORT_COUNT;

    let editing_collider = collider_edit.map(|c| c.active).unwrap_or(false);
    let selected = selection.get();
    // Hide mesh gizmos during modal transform and when NOT in Translate mode
    // (rotate/scale are drawn via immediate line gizmos).
    let show_meshes = selected.is_some()
        && !modal.active
        && !editing_collider
        && matches!(*mode, GizmoMode::Translate);
    // Lines (rotate/scale/plane) draw for any gizmo mode with a live selection.
    let lines_active = selected.is_some()
        && !modal.active
        && !editing_collider
        && matches!(
            *mode,
            GizmoMode::Translate | GizmoMode::Rotate | GizmoMode::Scale
        );

    // Toggle cone heads vs scale cubes based on mode (applies to every slot's set).
    for (part, mut vis) in gizmo_parts.iter_mut() {
        if part.is_translate_only() {
            *vis = if *mode == GizmoMode::Translate {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        } else if part.is_scale_only() {
            *vis = if *mode == GizmoMode::Scale {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    }

    // Reset per-slot draw flags; set below only for slots we actually size.
    per_slot.draw = [false; VIEWPORT_COUNT];

    let pivot_bottom = viewport_settings
        .as_ref()
        .map(|s| s.gizmo_pivot_bottom)
        .unwrap_or(true);

    // Selection pivot + world rotation (shared across slots — same object). The
    // handle *basis* is resolved per slot below, because each viewport can be in a
    // different Local/World space.
    let sel_data = selected.and_then(|s| {
        transforms.get(s).ok().map(|sel_gt| {
            // Anchor on the world-space AABB center so the gizmo lands on top of
            // the visible mesh even when the entity's pivot was authored at world
            // (0,0,0) (common for scene-style GLBs). Hover hit-test + line gizmos
            // use the same pivot so visual, pick, and drag agree.
            let sel_world = compute_gizmo_pivot(s, &aabbs, &children_q, sel_gt, pivot_bottom);
            (sel_world, sel_gt.rotation())
        })
    });

    // Per-slot camera world positions (indexed by slot).
    let mut cam_pos: [Option<Vec3>; VIEWPORT_COUNT] = [None; VIEWPORT_COUNT];
    for (vc, gt) in &cameras {
        if let Some(slot) = cam_pos.get_mut(vc.0) {
            *slot = Some(gt.translation());
        }
    }

    let focused = viewports.as_ref().map(|v| v.focused).unwrap_or(0);
    // Default: the gizmo shows only in the viewport the cursor is in (the focused
    // slot). A setting shows it in every viewport at once.
    let all_viewports = viewport_settings
        .as_ref()
        .map(|s| s.gizmos_all_viewports)
        .unwrap_or(false);
    // Per-slot Local/World space (defaults to World when the resource is absent).
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
    let dragging = gizmo_state.active_axis.is_some();
    // Signs are locked to the focused view's while a drag is in progress, so a
    // handle can't flip out from under the user mid-drag — in any viewport.
    let locked_signs = gizmo_state.axis_signs;

    // Focused-slot scale/signs feed `GizmoState`, which the analytic hover/drag
    // hit-test reads (interaction always happens in the focused view).
    let mut focused_scale: Option<f32> = None;
    let mut focused_signs: Option<Vec3> = None;

    for (slot, mut tf, mut vis) in gizmo_roots.iter_mut() {
        let i = slot.0;
        let docked = viewports
            .as_ref()
            .and_then(|v| v.slots.get(i))
            .map(|s| s.docked)
            .unwrap_or(i == 0);

        let (Some((sel_world, sel_rot)), Some(cam)) = (sel_data, cam_pos.get(i).copied().flatten())
        else {
            *vis = Visibility::Hidden;
            continue;
        };

        // This slot draws the gizmo only if it's the focused viewport, or the
        // "show in all viewports" setting is on.
        let slot_shows = all_viewports || i == focused;

        // Resolve the handle basis in THIS slot's own space (World-aligned or the
        // object's Local rotation).
        let basis = gizmo_basis(slot_space(i), *mode, sel_rot);
        let world_aligned = basis == Quat::IDENTITY;
        let dist = (cam - sel_world).length().max(0.1);
        let scale = dist / GIZMO_SCALE_REF_DIST;

        // Per-axis signs: X and Z flip toward THIS slot's camera so handles stay
        // visible from its angle. Y stays +1 (the up arrow must never flip, or the
        // gizmo reads upside-down). Locked to the focused view's signs while
        // dragging. Only world-aligned handles flip; oriented (Local/scale)
        // handles point along the real axes.
        let signs = if dragging {
            locked_signs
        } else if world_aligned {
            let cam_dir = cam - sel_world;
            Vec3::new(
                if cam_dir.x >= 0.0 { 1.0 } else { -1.0 },
                1.0,
                if cam_dir.z >= 0.0 { 1.0 } else { -1.0 },
            )
        } else {
            Vec3::ONE
        };

        tf.translation = sel_world;
        tf.rotation = basis;
        tf.scale = Vec3::new(scale * signs.x, scale * signs.y, scale * signs.z);

        *vis = if show_meshes && docked && slot_shows {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };

        per_slot.scale[i] = scale;
        per_slot.signs[i] = signs;
        per_slot.draw[i] = lines_active && docked && slot_shows;

        if i == focused {
            focused_scale = Some(scale);
            focused_signs = Some(signs);
        }
    }

    // Feed the focused slot's sizing into GizmoState for the hit-test. Signs are
    // only refreshed when not dragging (they're locked for the drag's duration).
    if let Some(scale) = focused_scale {
        gizmo_state.gizmo_scale = scale;
    }
    if !dragging {
        if let Some(signs) = focused_signs {
            gizmo_state.axis_signs = signs;
        }
    }
}

// ── Material update (hover/active highlighting) ─────────────────────────────

pub(crate) fn update_gizmo_materials(
    gizmo_state: Res<GizmoState>,
    gizmo_mats: Option<Res<GizmoMaterials>>,
    viewport_settings: Option<Res<ViewportSettings>>,
    mut materials: ResMut<Assets<GizmoMaterial>>,
    mut last_alpha: Local<Option<f32>>,
    mut query: Query<(&GizmoPart, &mut MeshMaterial3d<GizmoMaterial>), With<GizmoMesh>>,
) {
    let Some(mats) = gizmo_mats else { return };

    // While a handle is actively dragged, fade every handle to translucent so
    // the object underneath stays visible (the handles render always-on-top,
    // so at full opacity they hide whatever you're moving). The drag opacity is
    // user-configurable (Settings → Viewport). Only re-touch the material assets
    // when the target alpha actually changes, to avoid per-frame churn.
    let drag_alpha = viewport_settings
        .map(|v| v.gizmo_drag_opacity)
        .unwrap_or(0.25)
        .clamp(0.0, 1.0);
    let alpha = if gizmo_state.active_axis.is_some() {
        drag_alpha
    } else {
        1.0
    };
    if *last_alpha != Some(alpha) {
        *last_alpha = Some(alpha);
        for handle in [
            &mats.x_normal,
            &mats.x_highlight,
            &mats.y_normal,
            &mats.y_highlight,
            &mats.z_normal,
            &mats.z_highlight,
            &mats.center_normal,
            &mats.center_highlight,
        ] {
            if let Some(mut m) = materials.get_mut(handle) {
                m.base_color.alpha = alpha;
                m.emissive.alpha = alpha;
            }
        }
    }

    let active = gizmo_state.active_axis.or(gizmo_state.hovered_axis);

    for (part, mut mat_handle) in query.iter_mut() {
        let (normal, highlight, highlighted) = match part {
            GizmoPart::XShaft | GizmoPart::XHead | GizmoPart::XScaleCube => (
                mats.x_normal.clone(),
                mats.x_highlight.clone(),
                matches!(
                    active,
                    Some(GizmoAxis::X) | Some(GizmoAxis::XY) | Some(GizmoAxis::XZ)
                ),
            ),
            GizmoPart::YShaft | GizmoPart::YHead | GizmoPart::YScaleCube => (
                mats.y_normal.clone(),
                mats.y_highlight.clone(),
                matches!(
                    active,
                    Some(GizmoAxis::Y) | Some(GizmoAxis::XY) | Some(GizmoAxis::YZ)
                ),
            ),
            GizmoPart::ZShaft | GizmoPart::ZHead | GizmoPart::ZScaleCube => (
                mats.z_normal.clone(),
                mats.z_highlight.clone(),
                matches!(
                    active,
                    Some(GizmoAxis::Z) | Some(GizmoAxis::XZ) | Some(GizmoAxis::YZ)
                ),
            ),
            GizmoPart::Center => (
                mats.center_normal.clone(),
                mats.center_highlight.clone(),
                false,
            ),
        };

        mat_handle.0 = if highlighted { highlight } else { normal };
    }
}

/// Tracks the previous frame's selection size so the auto-switch system can
/// detect empty → non-empty and non-empty → empty transitions without wiring
/// change detection through the `RwLock`-backed `EditorSelection`.
#[derive(Resource, Default)]
pub(crate) struct LastSelectionCount(pub usize);

/// When the user selects an entity, switch to the Translate tool so drag
/// handles appear immediately. When the selection becomes empty, switch
/// back to Select. Leaves the tool alone if the user has deliberately
/// chosen Rotate, Scale, a brush, or a plugin tool.
pub(crate) fn auto_switch_tool_on_selection(world: &mut World) {
    use renzora_editor_framework::ActiveTool;

    let current = world
        .resource::<renzora_editor_framework::EditorSelection>()
        .get_all()
        .len();
    let prev = world.resource::<LastSelectionCount>().0;
    if current == prev {
        return;
    }
    world.resource_mut::<LastSelectionCount>().0 = current;

    let active = *world.resource::<ActiveTool>();

    // Terrain tools only make sense while a terrain is selected; revert to
    // Select if the user deselected (or selected a non-terrain entity).
    if active.needs_terrain_selection() {
        if !renzora_editor_framework::is_terrain_selected(world) {
            world.insert_resource(ActiveTool::Select);
        }
        return;
    }

    // Only react while a gizmo-style tool is active. `None` drives its own
    // selection semantics (e.g. mesh-draw).
    let is_gizmo_tool = matches!(
        active,
        ActiveTool::Select | ActiveTool::Translate | ActiveTool::Rotate | ActiveTool::Scale
    );
    if !is_gizmo_tool {
        return;
    }

    if prev == 0 && current > 0 {
        // Just selected something. Only promote Select → Translate; don't
        // override a deliberate Rotate / Scale choice.
        if active == ActiveTool::Select {
            world.insert_resource(ActiveTool::Translate);
        }
    } else if prev > 0 && current == 0 {
        // Cleared selection → back to Select.
        world.insert_resource(ActiveTool::Select);
    }
}
