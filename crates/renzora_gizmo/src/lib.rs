#![allow(dead_code, unused_variables)]

//! Renzora Gizmo — 3D transform gizmos for the editor viewport.
//!
//! Spawns real mesh entities (cylinders, cones, cubes) with an always-on-top
//! material. Supports translate (arrows + plane squares), rotate (circles),
//! and scale (lines + cube caps) modes.
//!
//! The transform gizmo itself is split across:
//!
//! - [`types`] — axes, mesh parts, per-slot state, the drag/box-selection resources
//! - [`meshes`] / [`material`] — building the handle entities and their shader
//! - [`transforms`] — following the selection, per viewport slot
//! - [`lines`] — the immediate-mode rotate rings, scale cubes and plane squares
//! - [`ray`] / [`hover`] / [`drag`] — the analytic hit-test and the drag itself
//! - [`picking`] / [`box_select`] — click and marquee selection
//! - [`shortcuts`] — keyboard commands that act on the selection or the document
//!
//! Everything else in `src/` is an independent overlay (colliders, lights,
//! skeletons, the 2D picker and grid, entity labels) that this plugin only
//! schedules.

mod box_select;
mod camera_gizmo;
pub mod collider_gizmo;
pub mod collider_edit_2d;
pub mod collider_handles;
mod drag;
mod entity_labels;
mod grid_2d;
mod hover;
mod light_gizmo;
mod lines;
mod material;
mod meshes;
pub mod modal_transform;
mod picker_2d;
mod picking;
mod pivot;
mod ray;
pub mod selection_visuals;
mod shortcuts;
pub mod skeleton_gizmo;
mod transform_space;
mod transforms;
mod types;

use bevy::camera::visibility::RenderLayers;
use bevy::gizmos::config::{GizmoConfig, GizmoLineConfig};
use bevy::gizmos::AppGizmoBuilder;
use bevy::prelude::*;

use crate::lines::{
    slot_line_config, SlotPlaneGroup0, SlotPlaneGroup1, SlotPlaneGroup2, SlotPlaneGroup3,
    SlotTransformGroup0, SlotTransformGroup1, SlotTransformGroup2, SlotTransformGroup3,
};
use crate::types::PerSlotGizmo;

// The crate's public seam, unchanged by the split — every name here was a
// top-level item of `lib.rs` before, and the sibling overlay modules
// (`modal_transform`, `selection_visuals`, `entity_labels`, …) reach for them
// as `crate::Foo`, so they have to keep resolving at the crate root.
pub use crate::lines::{
    LabelGizmoGroup, OverlayGizmoGroup, PlaneGizmoGroup, TransformGizmoGroup,
};
pub use crate::material::GizmoMaterial;
pub use crate::shortcuts::EditorClipboard;
pub use crate::types::{BoxSelectionState, GizmoAxis, GizmoMode, GizmoSpace, GizmoState};

pub(crate) use crate::drag::snap_rotation;
pub(crate) use crate::lines::{draw_angle_label, draw_rotation_pie};
pub(crate) use crate::pivot::compute_gizmo_pivot;

// ── Constants ───────────────────────────────────────────────────────────────

pub(crate) const GIZMO_SIZE: f32 = 2.0;
pub(crate) const GIZMO_SCALE_REF_DIST: f32 = 10.0;
pub(crate) const GIZMO_PLANE_SIZE: f32 = 0.8;
pub(crate) const GIZMO_PLANE_OFFSET: f32 = 0.6;

// ── Plugin ──────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct GizmoPlugin;

impl Plugin for GizmoPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] GizmoPlugin");
        bevy::asset::embedded_asset!(app, "shaders/gizmo_material.wgsl");
        app.add_plugins(MaterialPlugin::<GizmoMaterial>::default())
            .insert_gizmo_config(
                OverlayGizmoGroup,
                GizmoConfig {
                    depth_bias: -1.0,
                    line: GizmoLineConfig {
                        width: 3.0,
                        ..default()
                    },
                    render_layers: RenderLayers::layer(1),
                    ..default()
                },
            )
            .insert_gizmo_config(
                TransformGizmoGroup,
                GizmoConfig {
                    depth_bias: -1.0,
                    line: GizmoLineConfig {
                        width: 3.0,
                        ..default()
                    },
                    render_layers: RenderLayers::layer(1),
                    ..default()
                },
            )
            .insert_gizmo_config(
                PlaneGizmoGroup,
                GizmoConfig {
                    depth_bias: -1.0,
                    line: GizmoLineConfig {
                        // Thicker than the axes so the plane handles read as
                        // chunky, grabbable brackets.
                        width: 6.0,
                        ..default()
                    },
                    render_layers: RenderLayers::layer(1),
                    ..default()
                },
            )
            .insert_gizmo_config(
                LabelGizmoGroup,
                GizmoConfig {
                    // Always on top (near-plane depth) so labels read over the
                    // geometry they annotate. Thinner lines than the transform
                    // handles keep the stroke text legible rather than chunky.
                    depth_bias: -1.0,
                    line: GizmoLineConfig {
                        width: 1.5,
                        ..default()
                    },
                    render_layers: RenderLayers::layer(1),
                    ..default()
                },
            )
            // Per-slot transform / plane line groups, each bound to its slot's
            // private overlay layer so a viewport draws only its own handle
            // (see `draw_line_gizmos`). Widths mirror the shared groups above.
            .insert_gizmo_config(
                SlotTransformGroup0,
                slot_line_config(0, 3.0),
            )
            .insert_gizmo_config(SlotPlaneGroup0, slot_line_config(0, 6.0))
            .insert_gizmo_config(SlotTransformGroup1, slot_line_config(1, 3.0))
            .insert_gizmo_config(SlotPlaneGroup1, slot_line_config(1, 6.0))
            .insert_gizmo_config(SlotTransformGroup2, slot_line_config(2, 3.0))
            .insert_gizmo_config(SlotPlaneGroup2, slot_line_config(2, 6.0))
            .insert_gizmo_config(SlotTransformGroup3, slot_line_config(3, 3.0))
            .insert_gizmo_config(SlotPlaneGroup3, slot_line_config(3, 6.0))
            .init_resource::<PerSlotGizmo>()
            .init_resource::<renzora::core::viewport_types::ViewportGizmoSpace>()
            .init_resource::<GizmoMode>()
            .init_resource::<GizmoSpace>()
            .init_resource::<GizmoState>()
            .init_resource::<BoxSelectionState>()
            .init_resource::<skeleton_gizmo::BoneSelection>()
            .init_resource::<modal_transform::ModalTransformState>()
            .init_resource::<renzora::core::ModalTransformHud>()
            .add_systems(PostStartup, meshes::setup_gizmo_meshes)
            // Selection shortcuts (Delete / Deselect / CreateNode) aren't
            // 3D-specific — Delete on a 2D entity should also work from
            // any panel. Pull these out of the in_three_view chain so they
            // run in 2D/UI views too.
            .add_systems(
                Update,
                (
                    shortcuts::handle_selection_shortcuts,
                    shortcuts::handle_file_shortcuts,
                )
                    .run_if(in_state(renzora_editor_framework::SplashState::Editor))
                    .run_if(renzora::core::not_in_play_mode),
            )
            // Keep the global `GizmoSpace` (read by the analytic drag/hit-test and
            // any other consumer) in step with the FOCUSED viewport's per-slot
            // space, so interaction always uses the space of the view you're in.
            .add_systems(
                Update,
                transforms::mirror_focused_gizmo_space
                    .before(transforms::update_gizmo_transforms)
                    .run_if(in_state(renzora_editor_framework::SplashState::Editor)),
            )
            .add_systems(
                Update,
                (
                    shortcuts::switch_gizmo_mode,
                    modal_transform::modal_transform_input_system,
                    modal_transform::modal_transform_keyboard_system,
                    modal_transform::modal_transform_apply_system,
                    modal_transform::modal_transform_overlay_system,
                    modal_transform::sync_modal_hud,
                    transforms::update_gizmo_transforms,
                    transforms::update_gizmo_materials,
                    hover::gizmo_hover_detect,
                    drag::gizmo_drag,
                    lines::draw_line_gizmos,
                    selection_visuals::draw_selection_bounding_box,
                    selection_visuals::update_selection_gizmo_depth,
                    camera_gizmo::draw_camera_gizmo,
                    skeleton_gizmo::draw_skeleton_gizmo,
                    // A resize handle overhangs the content it sizes, and the
                    // viewport decides a press is its own purely geometrically
                    // (`RelativeCursorPosition`) — so the global bottom panel's
                    // grip band, which straddles its own top edge, reads as a
                    // press in the scene and armed a selection box that then
                    // stretched across the viewport for the whole drag. A run
                    // condition rather than a `Res` parameter: this system is
                    // already at Bevy's 16-parameter ceiling.
                    picking::entity_pick_system.run_if(renzora::core::resize::not_resizing),
                    box_select::box_selection_system,
                )
                    .chain()
                    .run_if(in_state(renzora_editor_framework::SplashState::Editor))
                    .run_if(renzora::core::not_in_play_mode)
                    .run_if(renzora::core::in_three_view),
            )
            .add_systems(
                Update,
                box_select::render_box_selection
                    .after(box_select::box_selection_system)
                    .run_if(in_state(renzora_editor_framework::SplashState::Editor))
                    .run_if(renzora::core::in_three_view),
            )
            .add_systems(
                Update,
                selection_visuals::terrain_chunk_selection_system
                    .run_if(in_state(renzora_editor_framework::SplashState::Editor))
                    .run_if(renzora::core::in_three_view),
            )
            .init_resource::<collider_handles::ColliderHandleState>()
            .add_systems(
                Update,
                collider_gizmo::draw_collider_gizmos
                    .run_if(in_state(renzora_editor_framework::SplashState::Editor))
                    .run_if(renzora::core::not_in_play_mode)
                    .run_if(renzora::core::in_three_view),
            )
            .add_systems(
                Update,
                light_gizmo::draw_light_gizmos
                    .run_if(in_state(renzora_editor_framework::SplashState::Editor))
                    .run_if(renzora::core::not_in_play_mode)
                    .run_if(renzora::core::in_three_view),
            )
            // Entity name labels (Bevy 0.19 stroke-font text gizmos), gated on
            // the Overlays → "Labels" toggle inside the system itself.
            .add_systems(
                Update,
                entity_labels::draw_entity_labels
                    .run_if(in_state(renzora_editor_framework::SplashState::Editor))
                    .run_if(renzora::core::not_in_play_mode)
                    .run_if(renzora::core::in_three_view),
            )
            .init_resource::<light_gizmo::SceneIconCache>()
            .add_systems(
                Update,
                light_gizmo::update_scene_icon_cache
                    .run_if(in_state(renzora_editor_framework::SplashState::Editor))
                    .run_if(renzora::core::not_in_play_mode)
                    .run_if(renzora::core::in_three_view),
            )
            // Always-on (no view gate): keeps the cached 2D camera entity
            // current so the 2D selection-outline overlay can render
            // without needing &mut World.
            .add_systems(
                Update,
                light_gizmo::update_editor_camera_2d_cache
                    .run_if(in_state(renzora_editor_framework::SplashState::Editor)),
            )
            .add_systems(
                Update,
                (
                    collider_handles::pick_and_drag_handles,
                    collider_handles::spawn_handle_meshes,
                )
                    .chain()
                    .run_if(in_state(renzora_editor_framework::SplashState::Editor))
                    .run_if(renzora::core::not_in_play_mode)
                    .run_if(renzora::core::in_three_view),
            )
            .init_resource::<transforms::LastSelectionCount>()
            .add_systems(
                Update,
                transforms::auto_switch_tool_on_selection
                    .after(picking::entity_pick_system)
                    .after(box_select::box_selection_system)
                    .run_if(in_state(renzora_editor_framework::SplashState::Editor))
                    .run_if(renzora::core::not_in_play_mode)
                    .run_if(renzora::core::in_three_view),
            );

        // 2D picker + drag systems — gated on viewport_view == Two so they
        // don't fight the 3D camera_controller / entity_pick when the user
        // is in 3D mode. `.chain()` enforces pick-before-drag so a fresh
        // click selects an entity *and* captures its drag offset in the
        // same frame.
        app.init_resource::<picker_2d::Drag2dState>();
        app.init_resource::<renzora::core::viewport_types::ViewportCursorRequest>();
        app.init_resource::<renzora::core::viewport_types::ViewportBoxSelect2d>();
        app.add_systems(
            Update,
            (
                picker_2d::pick_2d_system,
                picker_2d::box_select_2d_system,
                picker_2d::drag_move_2d_system,
                picker_2d::keyboard_nudge_2d,
            )
                .chain()
                .run_if(in_state(renzora_editor_framework::SplashState::Editor))
                .run_if(renzora::core::not_in_play_mode)
                .run_if(renzora::core::in_two_view)
                // Stand down while a viewport brush (e.g. tilemap paint) is
                // active, so a paint click doesn't re-select or drag the entity.
                .run_if(|b: Option<Res<renzora::core::ViewportBrushActive>>| {
                    b.map(|b| !b.0).unwrap_or(true)
                }),
        );
        // Viewport hover cursor (Move / resize / rotate). NOT gated on
        // `in_two_view`: it must keep running to clear the cursor request
        // when the pointer leaves the viewport or the view leaves 2D.
        app.add_systems(
            Update,
            picker_2d::update_cursor_2d
                .after(picker_2d::drag_move_2d_system)
                .run_if(in_state(renzora_editor_framework::SplashState::Editor)),
        );

        // 2D collider editing (inspector Edit toggle): moves/resizes the
        // selected `CollisionShapeData` and publishes its own cursor. The
        // picker systems above all stand down while it's active. Like the
        // cursor system it is NOT gated on `in_two_view` — it must keep
        // running to end an in-flight drag (recording its undo step) and
        // clear its cursor request when the view leaves 2D.
        app.init_resource::<collider_edit_2d::ColliderDrag2d>();
        app.add_systems(
            Update,
            collider_edit_2d::collider_edit_2d_system
                .after(picker_2d::update_cursor_2d)
                .run_if(in_state(renzora_editor_framework::SplashState::Editor)),
        );

        // 2D editor grid: a faint line MESH at z=-900 so it renders behind the
        // sprites (2D gizmos always sort on top — see grid_2d.rs), plus the
        // amber camera-boundary gizmo. No `in_two_view` gate: the system must
        // keep running outside the 2D view to hide the grid entity.
        app.add_systems(Update, grid_2d::update_grid_2d);
    }
}

renzora::add!(GizmoPlugin, Editor);
