#![allow(dead_code, unused_variables)]

//! Renzora Gizmo — 3D transform gizmos for the editor viewport.
//!
//! Spawns real mesh entities (cylinders, cones, cubes) with an always-on-top
//! material. Supports translate (arrows + plane squares), rotate (circles),
//! and scale (lines + cube caps) modes.

mod camera_gizmo;
pub mod collider_gizmo;
pub mod collider_handles;
mod entity_labels;
mod grid_2d;
mod light_gizmo;
pub mod modal_transform;
mod picker_2d;
pub mod selection_visuals;
pub mod skeleton_gizmo;
mod transform_space;

use bevy::camera::visibility::RenderLayers;
use bevy::ecs::system::SystemParam;
use bevy::input::mouse::MouseMotion;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey};
use bevy::picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, CompareFunction, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use renzora::core::keybindings::{EditorAction, KeyBindings};
use renzora::core::viewport_types::{
    NavOverlayState, SnapSettings, ViewportSettings, ViewportState,
};
use renzora::core::InputFocusState;
use renzora::SelectionStop;
use renzora_editor_framework::{
    EditorCamera, EditorLocked, EditorSelection, EditorSettings, HideInHierarchy,
    SelectionGranularity,
};

// ── Constants ───────────────────────────────────────────────────────────────

pub(crate) const GIZMO_SIZE: f32 = 2.0;
const GIZMO_SCALE_REF_DIST: f32 = 10.0;
pub(crate) const GIZMO_PLANE_SIZE: f32 = 0.8;
pub(crate) const GIZMO_PLANE_OFFSET: f32 = 0.6;

// ── Pivot computation ───────────────────────────────────────────────────────

/// Return the world-space pivot to anchor the gizmo on for `entity`.
///
/// Many GLBs (e.g. scenes exported from Blender or assembled in DCCs) author
/// every mesh node with its origin at world (0,0,0) and bake the actual
/// position into the vertex data. Anchoring on `GlobalTransform.translation`
/// would put the gizmo at the world origin instead of on top of the mesh —
/// which is what users hit when dropping large scene GLBs into the editor.
///
/// We instead compute the world-space AABB center over the entity's mesh and
/// every descendant mesh, falling back to the entity's transform if no AABBs
/// are available yet (e.g. just-spawned entities before mesh load).
pub(crate) fn compute_gizmo_pivot(
    entity: Entity,
    aabbs: &Query<(Option<&bevy::camera::primitives::Aabb>, &GlobalTransform), With<Mesh3d>>,
    children: &Query<&Children>,
    fallback_gt: &GlobalTransform,
) -> Vec3 {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    collect_pivot_aabb(entity, aabbs, children, &mut min, &mut max);
    if min.x <= max.x {
        (min + max) * 0.5
    } else {
        fallback_gt.translation()
    }
}

fn collect_pivot_aabb(
    entity: Entity,
    aabbs: &Query<(Option<&bevy::camera::primitives::Aabb>, &GlobalTransform), With<Mesh3d>>,
    children: &Query<&Children>,
    min: &mut Vec3,
    max: &mut Vec3,
) {
    if let Ok((Some(aabb), gt)) = aabbs.get(entity) {
        let c = Vec3::from(aabb.center);
        let h = Vec3::from(aabb.half_extents);
        for sx in [-1.0_f32, 1.0] {
            for sy in [-1.0_f32, 1.0] {
                for sz in [-1.0_f32, 1.0] {
                    let corner = gt.transform_point(c + h * Vec3::new(sx, sy, sz));
                    *min = min.min(corner);
                    *max = max.max(corner);
                }
            }
        }
    }
    if let Ok(kids) = children.get(entity) {
        for child in kids.iter() {
            collect_pivot_aabb(child, aabbs, children, min, max);
        }
    }
}

// ── GizmoMaterial — always renders on top ───────────────────────────────────

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct GizmoMaterial {
    #[uniform(0)]
    pub base_color: LinearRgba,
    #[uniform(0)]
    pub emissive: LinearRgba,
}

impl Material for GizmoMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_gizmo/shaders/gizmo_material.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(ref mut depth_stencil) = descriptor.depth_stencil {
            // wgpu 29: these `DepthStencilState` fields are now `Option`.
            depth_stencil.depth_compare = Some(CompareFunction::Always);
            depth_stencil.depth_write_enabled = Some(false);
        }
        // Gizmo meshes get mirrored via negative root scale when axes flip
        // to face the camera — disable backface culling so cone heads and
        // scale cubes keep rendering correctly regardless of winding.
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

// ── Enums ───────────────────────────────────────────────────────────────────

pub use renzora_editor_framework::{GizmoMode, GizmoSpace};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GizmoAxis {
    X,
    Y,
    Z,
    XY,
    XZ,
    YZ,
}

impl GizmoAxis {
    fn direction(self) -> Vec3 {
        match self {
            Self::X => Vec3::X,
            Self::Y => Vec3::Y,
            Self::Z => Vec3::Z,
            Self::XY => Vec3::Z,
            Self::XZ => Vec3::Y,
            Self::YZ => Vec3::X,
        }
    }

    /// Axis direction with per-axis signs applied so single-axis handles
    /// (X/Y/Z) flip to face the camera. Plane normals are left alone —
    /// the drag plane is the same regardless of viewing side.
    fn signed_direction(self, signs: Vec3) -> Vec3 {
        match self {
            Self::X => Vec3::new(signs.x, 0.0, 0.0),
            Self::Y => Vec3::new(0.0, signs.y, 0.0),
            Self::Z => Vec3::new(0.0, 0.0, signs.z),
            Self::XY | Self::XZ | Self::YZ => self.direction(),
        }
    }

    fn is_plane(self) -> bool {
        matches!(self, Self::XY | Self::XZ | Self::YZ)
    }

    fn plane_axes(self) -> Option<(Vec3, Vec3)> {
        match self {
            Self::XY => Some((Vec3::X, Vec3::Y)),
            Self::XZ => Some((Vec3::X, Vec3::Z)),
            Self::YZ => Some((Vec3::Y, Vec3::Z)),
            _ => None,
        }
    }

    /// Plane axes with `axis_signs` baked in so plane handles flip into the
    /// quadrant facing the camera, matching how single-axis arrows already
    /// flip via `signed_direction`. Used by the picking quads.
    pub(crate) fn signed_plane_axes(self, signs: Vec3) -> Option<(Vec3, Vec3)> {
        match self {
            Self::XY => Some((Vec3::new(signs.x, 0.0, 0.0), Vec3::new(0.0, signs.y, 0.0))),
            Self::XZ => Some((Vec3::new(signs.x, 0.0, 0.0), Vec3::new(0.0, 0.0, signs.z))),
            Self::YZ => Some((Vec3::new(0.0, signs.y, 0.0), Vec3::new(0.0, 0.0, signs.z))),
            _ => None,
        }
    }
}

const AXES: [GizmoAxis; 3] = [GizmoAxis::X, GizmoAxis::Y, GizmoAxis::Z];
const PLANES: [GizmoAxis; 3] = [GizmoAxis::XY, GizmoAxis::XZ, GizmoAxis::YZ];

// ── Components & Resources ──────────────────────────────────────────────────

#[derive(Component)]
pub(crate) struct GizmoRoot;

#[derive(Component)]
pub(crate) struct GizmoMesh;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum GizmoPart {
    XShaft,
    XHead,
    YShaft,
    YHead,
    ZShaft,
    ZHead,
    XScaleCube,
    YScaleCube,
    ZScaleCube,
    Center,
}

impl GizmoPart {
    fn axis(self) -> Option<GizmoAxis> {
        match self {
            Self::XShaft | Self::XHead | Self::XScaleCube => Some(GizmoAxis::X),
            Self::YShaft | Self::YHead | Self::YScaleCube => Some(GizmoAxis::Y),
            Self::ZShaft | Self::ZHead | Self::ZScaleCube => Some(GizmoAxis::Z),
            Self::Center => None,
        }
    }

    fn is_translate_only(self) -> bool {
        matches!(self, Self::XHead | Self::YHead | Self::ZHead)
    }

    fn is_scale_only(self) -> bool {
        matches!(self, Self::XScaleCube | Self::YScaleCube | Self::ZScaleCube)
    }
}

#[derive(Resource)]
struct GizmoMaterials {
    x_normal: Handle<GizmoMaterial>,
    x_highlight: Handle<GizmoMaterial>,
    y_normal: Handle<GizmoMaterial>,
    y_highlight: Handle<GizmoMaterial>,
    z_normal: Handle<GizmoMaterial>,
    z_highlight: Handle<GizmoMaterial>,
    center_normal: Handle<GizmoMaterial>,
    center_highlight: Handle<GizmoMaterial>,
}

#[derive(Resource)]
pub struct GizmoState {
    pub active_axis: Option<GizmoAxis>,
    pub hovered_axis: Option<GizmoAxis>,
    pub drag_starts: Vec<(Entity, Vec3, Quat, Vec3)>,
    pub drag_offset: Vec3,
    pub drag_angle: f32,
    pub drag_scale_factor: f32,
    pub gizmo_scale: f32,
    /// +1 or -1 per axis — flipped so each arrow points toward the camera
    /// rather than away, keeping handles visible and pickable regardless of
    /// the current viewing angle. Locked while a drag is in progress so
    /// the handle direction doesn't flip mid-drag.
    pub axis_signs: Vec3,
    /// World-space orientation of the gizmo handles, captured at drag start so
    /// the axes stay fixed for the whole gesture even in Local space (where the
    /// object's rotation — and thus the live basis — changes as you rotate it).
    pub drag_basis: Quat,
    /// World-space pivot the active drag rotates/scales about (the selection's
    /// AABB center at drag start), so the object pivots in place.
    pub drag_pivot: Vec3,
    /// Each dragged entity's parent world affine, captured at drag start (the
    /// parent doesn't move during the gesture). World-space deltas are converted
    /// into this frame before being written to the entity's local `Transform`,
    /// so transforms are correct under any nesting. Index-aligned with
    /// `drag_starts`.
    pub drag_parents: Vec<bevy::math::Affine3A>,
    /// World point under the cursor at drag start, projected onto the dragged
    /// axis line / plane. Translate keeps this point pinned to the cursor each
    /// frame so the gizmo tracks the pointer exactly instead of drifting.
    pub drag_grab: Vec3,
}

impl Default for GizmoState {
    fn default() -> Self {
        Self {
            active_axis: None,
            hovered_axis: None,
            drag_starts: Vec::new(),
            drag_offset: Vec3::ZERO,
            drag_angle: 0.0,
            drag_scale_factor: 0.0,
            gizmo_scale: 1.0,
            axis_signs: Vec3::ONE,
            drag_basis: Quat::IDENTITY,
            drag_pivot: Vec3::ZERO,
            drag_parents: Vec::new(),
            drag_grab: Vec3::ZERO,
        }
    }
}

/// World-space orientation of the gizmo handles for `mode`, given the
/// selection's world rotation and the active [`GizmoSpace`]. Scale handles are
/// always local-aligned — a non-uniform scale along world axes can't be written
/// back as a `Transform` (it would shear a rotated object) — so the space toggle
/// only changes which way the scale handles point, never the scale math.
fn gizmo_basis(space: GizmoSpace, mode: GizmoMode, sel_world_rot: Quat) -> Quat {
    match mode {
        GizmoMode::Scale => sel_world_rot,
        _ => space.basis(sel_world_rot),
    }
}

/// State for box/marquee selection (drag to select multiple entities).
///
/// A single click is also routed through this state: on press we arm
/// `active` + optionally remember the entity under the cursor in
/// `pending_pick`. On release, if the mouse barely moved, we commit the
/// pending pick (or deselect on empty space); if it moved past the drag
/// threshold, we finalise a box selection. This makes drag-select work
/// whether the drag starts on an entity or on empty space.
#[derive(Resource, Default, Clone, Copy)]
pub struct BoxSelectionState {
    /// Whether a click/drag gesture is in progress.
    pub active: bool,
    /// Start position in screen coordinates.
    pub start_pos: Vec2,
    /// Current position in screen coordinates.
    pub current_pos: Vec2,
    /// Entity under the cursor at press time. Committed as a single-entity
    /// selection on release if the gesture didn't become a drag.
    pub pending_pick: Option<Entity>,
}

impl BoxSelectionState {
    /// Get the selection rectangle as (min, max) screen positions.
    pub fn get_rect(&self) -> (Vec2, Vec2) {
        let min = Vec2::new(
            self.start_pos.x.min(self.current_pos.x),
            self.start_pos.y.min(self.current_pos.y),
        );
        let max = Vec2::new(
            self.start_pos.x.max(self.current_pos.x),
            self.start_pos.y.max(self.current_pos.y),
        );
        (min, max)
    }

    /// Check if the box is large enough to be considered a drag (not just a click).
    pub fn is_drag(&self) -> bool {
        let d = (self.current_pos - self.start_pos).abs();
        d.x > 5.0 || d.y > 5.0
    }
}

// ── Plugin ──────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct GizmoPlugin;

impl Plugin for GizmoPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] GizmoPlugin");
        bevy::asset::embedded_asset!(app, "shaders/gizmo_material.wgsl");
        if !app.is_plugin_added::<bevy_mod_outline::OutlinePlugin>() {
            app.add_plugins(bevy_mod_outline::OutlinePlugin);
        }
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
            .init_resource::<GizmoMode>()
            .init_resource::<GizmoSpace>()
            .init_resource::<GizmoState>()
            .init_resource::<BoxSelectionState>()
            .init_resource::<skeleton_gizmo::BoneSelection>()
            .init_resource::<modal_transform::ModalTransformState>()
            .init_resource::<renzora::core::ModalTransformHud>()
            .add_systems(PostStartup, setup_gizmo_meshes)
            // Selection shortcuts (Delete / Deselect / CreateNode) aren't
            // 3D-specific — Delete on a 2D entity should also work from
            // any panel. Pull these out of the in_three_view chain so they
            // run in 2D/UI views too.
            .add_systems(
                Update,
                (handle_selection_shortcuts, handle_file_shortcuts)
                    .run_if(in_state(renzora_editor_framework::SplashState::Editor))
                    .run_if(renzora::core::not_in_play_mode),
            )
            .add_systems(
                Update,
                (
                    switch_gizmo_mode,
                    modal_transform::modal_transform_input_system,
                    modal_transform::modal_transform_keyboard_system,
                    modal_transform::modal_transform_apply_system,
                    modal_transform::modal_transform_overlay_system,
                    modal_transform::sync_modal_hud,
                    update_gizmo_transforms,
                    update_gizmo_materials,
                    gizmo_hover_detect,
                    gizmo_drag,
                    draw_line_gizmos,
                    selection_visuals::update_selection_outlines,
                    selection_visuals::draw_selection_bounding_box,
                    selection_visuals::update_selection_gizmo_depth,
                    camera_gizmo::draw_camera_gizmo,
                    skeleton_gizmo::draw_skeleton_gizmo,
                    entity_pick_system,
                    box_selection_system,
                )
                    .chain()
                    .run_if(in_state(renzora_editor_framework::SplashState::Editor))
                    .run_if(renzora::core::not_in_play_mode)
                    .run_if(renzora::core::in_three_view),
            )
            .add_systems(
                Update,
                render_box_selection
                    .after(box_selection_system)
                    .run_if(in_state(renzora_editor_framework::SplashState::Editor))
                    .run_if(renzora::core::in_three_view),
            )
            .add_systems(
                Update,
                selection_visuals::terrain_chunk_selection_system
                    .run_if(in_state(renzora_editor_framework::SplashState::Editor))
                    .run_if(renzora::core::in_three_view),
            )
            .init_resource::<renzora::core::ClickDebug>()
            .add_systems(
                Update,
                // Not gated on `in_three_view` — runs in UI view too so
                // UI-canvas click bleed is captured.
                click_diag.run_if(in_state(renzora_editor_framework::SplashState::Editor)),
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
            .init_resource::<LastSelectionCount>()
            .add_systems(
                Update,
                auto_switch_tool_on_selection
                    .after(entity_pick_system)
                    .after(box_selection_system)
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
        app.add_systems(
            Update,
            (
                picker_2d::pick_2d_system,
                picker_2d::drag_move_2d_system,
                picker_2d::keyboard_nudge_2d,
            )
                .chain()
                .run_if(in_state(renzora_editor_framework::SplashState::Editor))
                .run_if(renzora::core::not_in_play_mode)
                .run_if(renzora::core::in_two_view),
        );

        // 2D editor grid drawn via Bevy gizmos (not an egui overlay)
        // so it renders into the offscreen image *under* sprites
        // instead of being painted on top of the rendered viewport.
        // Spacing comes from `ViewportSettings.snap.translate_snap`
        // so it matches the toolbar's "Grid Snap" pill.
        app.add_systems(
            Update,
            grid_2d::draw_grid_2d_gizmos.run_if(renzora::core::in_two_view),
        );
    }
}

/// Tracks the previous frame's selection size so the auto-switch system can
/// detect empty → non-empty and non-empty → empty transitions without wiring
/// change detection through the `RwLock`-backed `EditorSelection`.
#[derive(Resource, Default)]
struct LastSelectionCount(usize);

/// When the user selects an entity, switch to the Translate tool so drag
/// handles appear immediately. When the selection becomes empty, switch
/// back to Select. Leaves the tool alone if the user has deliberately
/// chosen Rotate, Scale, a brush, or a plugin tool.
fn auto_switch_tool_on_selection(world: &mut World) {
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

    // Brush tools only make sense while a terrain is selected; revert to
    // Select if the user deselected (or selected a non-terrain entity).
    let is_brush = matches!(
        active,
        ActiveTool::TerrainSculpt | ActiveTool::TerrainPaint | ActiveTool::FoliagePaint
    );
    if is_brush {
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

// ── Mesh setup ──────────────────────────────────────────────────────────────

fn setup_gizmo_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GizmoMaterial>>,
) {
    let mat = |m: &mut Assets<GizmoMaterial>, r: f32, g: f32, b: f32| -> Handle<GizmoMaterial> {
        m.add(GizmoMaterial {
            base_color: LinearRgba::new(r, g, b, 1.0),
            emissive: LinearRgba::new(r, g, b, 1.0),
        })
    };

    let gizmo_mats = GizmoMaterials {
        x_normal: mat(&mut materials, 1.0, 0.15, 0.15),
        x_highlight: mat(&mut materials, 1.0, 1.0, 0.2),
        y_normal: mat(&mut materials, 0.15, 1.0, 0.15),
        y_highlight: mat(&mut materials, 1.0, 1.0, 0.2),
        z_normal: mat(&mut materials, 0.2, 0.3, 1.0),
        z_highlight: mat(&mut materials, 1.0, 1.0, 0.2),
        center_normal: mat(&mut materials, 0.9, 0.9, 0.9),
        center_highlight: mat(&mut materials, 1.0, 1.0, 0.2),
    };

    let shaft_mesh = meshes.add(Cylinder::new(0.05, GIZMO_SIZE - 0.4));
    let cone_mesh = meshes.add(Cone {
        radius: 0.15,
        height: 0.4,
    });
    let cube_mesh = meshes.add(Cuboid::new(0.25, 0.25, 0.25));

    let gizmo_root = commands
        .spawn((
            Transform::default(),
            Visibility::Hidden,
            GizmoRoot,
            HideInHierarchy,
            RenderLayers::layer(1),
        ))
        .id();

    let spawn = |commands: &mut Commands,
                 mesh: Handle<Mesh>,
                 mat: Handle<GizmoMaterial>,
                 transform: Transform,
                 part: GizmoPart,
                 root: Entity| {
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            transform,
            Visibility::Inherited,
            GizmoMesh,
            part,
            HideInHierarchy,
            RenderLayers::layer(1),
            ChildOf(root),
        ));
    };

    let half_shaft = (GIZMO_SIZE - 0.4) / 2.0;

    // X axis (rotate cylinder to point along X)
    spawn(
        &mut commands,
        shaft_mesh.clone(),
        gizmo_mats.x_normal.clone(),
        Transform::from_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2))
            .with_translation(Vec3::new(half_shaft, 0.0, 0.0)),
        GizmoPart::XShaft,
        gizmo_root,
    );
    spawn(
        &mut commands,
        cone_mesh.clone(),
        gizmo_mats.x_normal.clone(),
        Transform::from_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2))
            .with_translation(Vec3::new(GIZMO_SIZE - 0.2, 0.0, 0.0)),
        GizmoPart::XHead,
        gizmo_root,
    );

    // Y axis (cylinder default is along Y)
    spawn(
        &mut commands,
        shaft_mesh.clone(),
        gizmo_mats.y_normal.clone(),
        Transform::from_translation(Vec3::new(0.0, half_shaft, 0.0)),
        GizmoPart::YShaft,
        gizmo_root,
    );
    spawn(
        &mut commands,
        cone_mesh.clone(),
        gizmo_mats.y_normal.clone(),
        Transform::from_translation(Vec3::new(0.0, GIZMO_SIZE - 0.2, 0.0)),
        GizmoPart::YHead,
        gizmo_root,
    );

    // Z axis (rotate cylinder to point along Z)
    spawn(
        &mut commands,
        shaft_mesh.clone(),
        gizmo_mats.z_normal.clone(),
        Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
            .with_translation(Vec3::new(0.0, 0.0, half_shaft)),
        GizmoPart::ZShaft,
        gizmo_root,
    );
    spawn(
        &mut commands,
        cone_mesh.clone(),
        gizmo_mats.z_normal.clone(),
        Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
            .with_translation(Vec3::new(0.0, 0.0, GIZMO_SIZE - 0.2)),
        GizmoPart::ZHead,
        gizmo_root,
    );

    // Scale cubes at axis tips (hidden by default, shown in Scale mode)
    let scale_cube_mesh = meshes.add(Cuboid::new(0.15, 0.15, 0.15));
    spawn(
        &mut commands,
        scale_cube_mesh.clone(),
        gizmo_mats.x_normal.clone(),
        Transform::from_translation(Vec3::new(GIZMO_SIZE, 0.0, 0.0)),
        GizmoPart::XScaleCube,
        gizmo_root,
    );
    spawn(
        &mut commands,
        scale_cube_mesh.clone(),
        gizmo_mats.y_normal.clone(),
        Transform::from_translation(Vec3::new(0.0, GIZMO_SIZE, 0.0)),
        GizmoPart::YScaleCube,
        gizmo_root,
    );
    spawn(
        &mut commands,
        scale_cube_mesh.clone(),
        gizmo_mats.z_normal.clone(),
        Transform::from_translation(Vec3::new(0.0, 0.0, GIZMO_SIZE)),
        GizmoPart::ZScaleCube,
        gizmo_root,
    );

    // Center cube
    spawn(
        &mut commands,
        cube_mesh,
        gizmo_mats.center_normal.clone(),
        Transform::default(),
        GizmoPart::Center,
        gizmo_root,
    );

    commands.insert_resource(gizmo_mats);
}

// ── Transform update (follow selection, scale by distance) ──────────────────

fn update_gizmo_transforms(
    selection: Res<EditorSelection>,
    mode: Res<GizmoMode>,
    space: Res<GizmoSpace>,
    modal: Res<modal_transform::ModalTransformState>,
    collider_edit: Option<Res<renzora_physics::ColliderEditMode>>,
    mut gizmo_state: ResMut<GizmoState>,
    transforms: Query<&GlobalTransform, (Without<GizmoMesh>, Without<GizmoRoot>)>,
    aabbs: Query<(Option<&bevy::camera::primitives::Aabb>, &GlobalTransform), With<Mesh3d>>,
    children_q: Query<&Children>,
    mut gizmo_root: Query<(&mut Transform, &mut Visibility), With<GizmoRoot>>,
    mut gizmo_parts: Query<(&GizmoPart, &mut Visibility), (With<GizmoMesh>, Without<GizmoRoot>)>,
    camera_query: Query<&GlobalTransform, With<EditorCamera>>,
) {
    let Ok((mut root_transform, mut root_vis)) = gizmo_root.single_mut() else {
        return;
    };

    let editing_collider = collider_edit.map(|c| c.active).unwrap_or(false);
    let selected = selection.get();
    // Hide mesh gizmos during modal transform and when in Scale mode (drawn via immediate gizmos)
    let show_meshes = selected.is_some()
        && !modal.active
        && !editing_collider
        && matches!(*mode, GizmoMode::Translate);
    *root_vis = if show_meshes {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };

    // Toggle cone heads vs scale cubes based on mode
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

    if let Some(selected) = selected {
        if let Ok(sel_gt) = transforms.get(selected) {
            // Anchor on the world-space AABB center so the gizmo lands on top
            // of the visible mesh even when the entity's pivot was authored at
            // world (0,0,0) (common for scene-style GLBs). The hover hit-test
            // and immediate-mode line gizmos use the same pivot, so visual,
            // pick, and drag agree.
            let sel_world = compute_gizmo_pivot(selected, &aabbs, &children_q, sel_gt);
            root_transform.translation = sel_world;

            // Orient the handles per the active space (world-aligned, or the
            // object's own rotation in Local mode). Scale handles always follow
            // the object (see `gizmo_basis`).
            let basis = gizmo_basis(*space, *mode, sel_gt.rotation());
            let world_aligned = basis == Quat::IDENTITY;
            root_transform.rotation = basis;

            if let Ok(cam_gt) = camera_query.single() {
                let dist = (cam_gt.translation() - sel_world).length().max(0.1);
                let scale = dist / GIZMO_SCALE_REF_DIST;

                // Per-axis signs: X and Z flip toward the camera so handles
                // stay visible. Y stays +1 always — the up arrow must always
                // point up, never flip when looking from below (otherwise the
                // gizmo can read as upside-down). Locked while dragging so
                // the axis doesn't flip out from under the user. Only applied
                // for world-aligned handles; oriented (Local / scale) handles
                // point along the real axes without flipping.
                if gizmo_state.active_axis.is_none() {
                    gizmo_state.axis_signs = if world_aligned {
                        let cam_dir = cam_gt.translation() - sel_world;
                        Vec3::new(
                            if cam_dir.x >= 0.0 { 1.0 } else { -1.0 },
                            1.0,
                            if cam_dir.z >= 0.0 { 1.0 } else { -1.0 },
                        )
                    } else {
                        Vec3::ONE
                    };
                }
                let s = gizmo_state.axis_signs;
                root_transform.scale = Vec3::new(scale * s.x, scale * s.y, scale * s.z);
                gizmo_state.gizmo_scale = scale;
            }
        }
    }
}

// ── Material update (hover/active highlighting) ─────────────────────────────

fn update_gizmo_materials(
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

// ── Line gizmos for rotate, scale, and plane squares ────────────────────────

/// We still use Bevy's immediate-mode gizmos for circles (rotate), scale cubes,
/// and the plane-drag squares since those change per-mode.
use bevy::gizmos::config::{GizmoConfig, GizmoConfigGroup, GizmoLineConfig};
use bevy::gizmos::AppGizmoBuilder;

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

fn draw_line_gizmos(
    mut gizmos: Gizmos<TransformGizmoGroup>,
    mode: Res<GizmoMode>,
    space: Res<GizmoSpace>,
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
    // Thicker-lined group used only for the plane-drag squares.
    mut plane_gizmos: Gizmos<PlaneGizmoGroup>,
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
    let pos = compute_gizmo_pivot(selected, &aabbs, &children_q, sel_gt);
    let gs = gizmo_state.gizmo_scale;

    if matches!(*mode, GizmoMode::Select | GizmoMode::None) {
        return;
    }

    // Orient the rotate circles / scale lines to match the active space (and the
    // picking basis), so visuals and hit-testing agree.
    let basis = gizmo_basis(*space, *mode, sel_gt.rotation());
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
    let highlight = Color::srgb(1.0, 1.0, 0.3);
    let x_base = Color::srgb(1.0, 0.15, 0.15);
    let y_base = Color::srgb(0.15, 1.0, 0.15);
    let z_base = Color::srgb(0.2, 0.3, 1.0);

    match *mode {
        GizmoMode::Select | GizmoMode::None => unreachable!(),
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
                let (sa, sb) = plane.signed_plane_axes(gizmo_state.axis_signs).unwrap();
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
            // the object.
            if let Some(active_axis) = gizmo_state.active_axis {
                if gizmo_state.drag_angle.abs() > 1e-4 {
                    draw_rotation_pie(
                        &mut gizmos,
                        pos,
                        basis * active_axis.direction(),
                        gizmo_state.drag_angle,
                        radius,
                        highlight,
                    );
                    if let Ok(cam_gt) = camera_q.single() {
                        draw_angle_label(
                            &mut gizmos,
                            pos,
                            cam_gt.translation(),
                            gizmo_state.drag_angle,
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
    let text = format!("{:.1}\u{00B0}", radians.to_degrees());
    let size = (radius * 0.35).max(0.05);
    gizmos.text(
        Isometry3d::new(pivot, rot),
        text.as_str(),
        size,
        Vec2::new(0.0, -0.5),
        color,
    );
}

// ── Mode switching ──────────────────────────────────────────────────────────

// ── Selection shortcuts (Delete, Deselect, CreateNode) ───────────────────────

fn handle_selection_shortcuts(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    input_focus: Res<InputFocusState>,
    viewport_state: Res<ViewportState>,
    selection: Res<EditorSelection>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    gizmo_state: Res<GizmoState>,
    modal: Res<modal_transform::ModalTransformState>,
) {
    if keybindings.rebinding.is_some() {
        return;
    }
    if input_focus.egui_wants_keyboard {
        return;
    }
    if gizmo_state.active_axis.is_some() {
        return;
    }
    if modal.active {
        return;
    }

    // Delete fires from any panel (e.g. selecting in the Hierarchy and
    // pressing Delete without moving the cursor into the viewport). It is
    // suppressed while a panel (e.g. the animation timeline with a keyframe
    // selected) is consuming Delete to remove the keyframe, not the entity.
    if keybindings.just_pressed(EditorAction::Delete, &keyboard) && !input_focus.suppress_entity_delete {
        let entities = selection.get_all();
        if !entities.is_empty() {
            selection.clear();
            commands.queue(move |world: &mut World| {
                let mut items = Vec::new();
                let mut other = Vec::new();
                for entity in &entities {
                    let shape = world.get_entity(*entity).ok().and_then(|e| {
                        let shape_id = e.get::<renzora::core::MeshPrimitive>()?.0.clone();
                        let name = e.get::<Name>()?.as_str().to_string();
                        let transform = *e.get::<Transform>()?;
                        let color = e.get::<renzora::core::MeshColor>()?.0;
                        Some(renzora_undo::DeletedShape {
                            entity: *entity,
                            shape_id,
                            name,
                            transform,
                            color,
                        })
                    });
                    match shape {
                        Some(item) => items.push(item),
                        None => other.push(*entity),
                    }
                }
                for e in other {
                    if let Ok(em) = world.get_entity_mut(e) {
                        em.despawn();
                    }
                }
                if items.is_empty() {
                    return;
                }
                renzora_undo::execute(
                    world,
                    renzora_undo::UndoContext::Scene,
                    Box::new(renzora_undo::DeleteShapesCmd { items }),
                );
            });
        }
    }

    if input_focus.egui_has_pointer && !viewport_state.hovered {
        return;
    }
    if mouse_button.pressed(MouseButton::Right) {
        return;
    }

    if keybindings.just_pressed(EditorAction::Deselect, &keyboard) {
        selection.clear();
    }

    if keybindings.just_pressed(EditorAction::CreateNode, &keyboard) {
        commands.insert_resource(renzora::core::CreateNodeRequested);
    }

    // Copy (Ctrl+C) — snapshot the current selection into the clipboard.
    if keybindings.just_pressed(EditorAction::Copy, &keyboard) {
        let entities = selection.get_all();
        if !entities.is_empty() {
            commands.queue(move |world: &mut World| {
                world.insert_resource(EditorClipboard { entities });
            });
        }
    }

    // Paste (Ctrl+V) — clone every entity on the clipboard (filtering out
    // ones that have since been despawned) and select the copies. If the
    // cursor is over the viewport, pasted entities are re-positioned to
    // the ground-plane hit so paste follows the camera/cursor. Otherwise
    // they land at their original world position.
    if keybindings.just_pressed(EditorAction::Paste, &keyboard) {
        commands.queue(move |world: &mut World| {
            let sources = world
                .get_resource::<EditorClipboard>()
                .map(|c| c.entities.clone())
                .unwrap_or_default();
            if sources.is_empty() {
                return;
            }

            let paste_target = compute_paste_target(world);
            duplicate_entities(world, &sources);

            if let Some(target) = paste_target {
                let new_ids = world
                    .get_resource::<EditorSelection>()
                    .map(|s| s.get_all())
                    .unwrap_or_default();
                if new_ids.is_empty() {
                    return;
                }
                reposition_paste_group(world, &new_ids, target);
            }
        });
    }

    // Move selection to cursor (V): teleports the selected entities so their
    // centroid sits under the viewport cursor, bottom snapped to the hit point.
    // Reuses the paste-placement helpers for consistent behavior.
    if keybindings.just_pressed(EditorAction::MoveSelectionToCursor, &keyboard) {
        commands.queue(move |world: &mut World| {
            let selected = world
                .get_resource::<EditorSelection>()
                .map(|s| s.get_all())
                .unwrap_or_default();
            if selected.is_empty() {
                return;
            }
            let Some(target) = compute_paste_target(world) else {
                return;
            };
            reposition_paste_group(world, &selected, target);
        });
    }

    // Duplicate (Ctrl+D)
    if keybindings.just_pressed(EditorAction::Duplicate, &keyboard) {
        let entities = selection.get_all();
        if !entities.is_empty() {
            commands.queue(move |world: &mut World| {
                duplicate_entities(world, &entities);
            });
        }
    }

    // Duplicate & Move (Alt+D) — duplicate then enter grab mode
    if keybindings.just_pressed(EditorAction::DuplicateAndMove, &keyboard) {
        let entities = selection.get_all();
        if !entities.is_empty() {
            commands.queue(move |world: &mut World| {
                duplicate_entities(world, &entities);
            });
            commands.insert_resource(PendingModalGrab);
        }
    }
}

/// Deep-clone each selected entity (all components, via Bevy's
/// `EntityWorldMut::clone_and_spawn`) and replace the selection with the
/// new copies. The suffix " (Copy)" is appended to the `Name` so
/// duplicates are distinguishable in the hierarchy.
fn duplicate_entities(world: &mut World, sources: &[Entity]) {
    let mut new_ids: Vec<Entity> = Vec::with_capacity(sources.len());
    for src in sources {
        let Ok(mut src_mut) = world.get_entity_mut(*src) else {
            continue;
        };
        let new = src_mut.clone_and_spawn();
        // Append " (Copy)" to the cloned entity's Name.
        if let Some(original) = world.get::<Name>(new).map(|n| n.as_str().to_string()) {
            if let Ok(mut ent) = world.get_entity_mut(new) {
                ent.insert(Name::new(format!("{} (Copy)", original)));
            }
        }
        new_ids.push(new);
    }
    if let Some(sel) = world.get_resource::<EditorSelection>() {
        sel.clear();
        for e in &new_ids {
            sel.toggle(*e);
        }
    }
}

/// One-shot resource to signal pending modal grab from duplicate-and-move.
#[derive(Resource)]
struct PendingModalGrab;

/// Editor-wide clipboard for Copy/Paste of entities. Stores the source
/// entity ids captured at copy time; paste deep-clones each via
/// `EntityWorldMut::clone_and_spawn`, so all components transfer. Sources
/// that have been despawned between copy and paste are silently skipped.
#[derive(Resource, Default, Clone, Debug)]
pub struct EditorClipboard {
    pub entities: Vec<Entity>,
}

/// Project the window cursor onto the ground plane (y=0) through the
/// editor camera. Returns `None` if the cursor isn't over the viewport,
/// the ray misses the ground plane, or any required resource is missing —
/// callers fall back to pasting at the source's original position.
/// Shift `entities` so the group's XZ centroid lands at `target.x/z` and
/// the lowest point of the group's world-space AABB sits at `target.y`
/// (i.e. the floor). Preserves relative layout within the group.
fn reposition_paste_group(world: &mut World, entities: &[Entity], target: Vec3) {
    use bevy::camera::primitives::Aabb;

    // Centroid on XZ (where the cursor is).
    let mut centroid_xz = Vec2::ZERO;
    let mut count = 0u32;
    for e in entities {
        if let Some(t) = world.get::<Transform>(*e) {
            centroid_xz += Vec2::new(t.translation.x, t.translation.z);
            count += 1;
        }
    }
    if count == 0 {
        return;
    }
    centroid_xz /= count as f32;

    // Lowest world-space AABB bottom across the group. Mesh entities
    // carry `Aabb` in local space; transform into world space to get the
    // bottom y. Non-mesh entities fall back to their translation.y.
    let mut min_y = f32::INFINITY;
    for e in entities {
        let t_y = world.get::<Transform>(*e).map(|t| t.translation.y);
        let bottom = if let (Some(aabb), Some(gt)) =
            (world.get::<Aabb>(*e), world.get::<GlobalTransform>(*e))
        {
            world_space_min_y(aabb, gt)
        } else {
            t_y.unwrap_or(f32::INFINITY)
        };
        if bottom < min_y {
            min_y = bottom;
        }
    }
    if !min_y.is_finite() {
        // Nothing with a position — nothing to do.
        return;
    }

    let delta = Vec3::new(
        target.x - centroid_xz.x,
        target.y - min_y,
        target.z - centroid_xz.y,
    );
    for e in entities {
        if let Ok(mut ent) = world.get_entity_mut(*e) {
            if let Some(mut t) = ent.get_mut::<Transform>() {
                t.translation += delta;
            }
        }
    }
}

/// Transform the 8 corners of a local-space AABB by `gt` and return the
/// minimum world-space y — the lowest point of the mesh as it currently
/// sits in the world.
fn world_space_min_y(aabb: &bevy::camera::primitives::Aabb, gt: &GlobalTransform) -> f32 {
    let c = Vec3::from(aabb.center);
    let h = Vec3::from(aabb.half_extents);
    let mut min_y = f32::INFINITY;
    for dx in [-1.0_f32, 1.0] {
        for dy in [-1.0_f32, 1.0] {
            for dz in [-1.0_f32, 1.0] {
                let local = c + Vec3::new(dx * h.x, dy * h.y, dz * h.z);
                let world = gt.transform_point(local);
                if world.y < min_y {
                    min_y = world.y;
                }
            }
        }
    }
    min_y
}

/// Minimum-corner of a local-space AABB transformed by (translation, rotation,
/// scale) into world space. Used by the translate/scale gizmo for edge-snap
/// and bottom-anchor behaviors.
fn world_aabb_min(
    aabb: &bevy::camera::primitives::Aabb,
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,
) -> Vec3 {
    let c = Vec3::from(aabb.center);
    let h = Vec3::from(aabb.half_extents);
    let mut min = Vec3::splat(f32::INFINITY);
    for dx in [-1.0_f32, 1.0] {
        for dy in [-1.0_f32, 1.0] {
            for dz in [-1.0_f32, 1.0] {
                let local = c + Vec3::new(dx * h.x, dy * h.y, dz * h.z);
                let world = translation + rotation * (local * scale);
                min = min.min(world);
            }
        }
    }
    min
}

fn compute_paste_target(world: &mut World) -> Option<Vec3> {
    // Read viewport fields into locals so the immutable borrow is dropped
    // before we use `world.query_filtered` (which needs a mutable borrow).
    let (vp_min, vp_size, current_size, hovered) = {
        let vp = world.get_resource::<ViewportState>()?;
        (
            vp.screen_position,
            vp.screen_size,
            vp.current_size,
            vp.hovered,
        )
    };
    if !hovered {
        return None;
    }

    let cursor = {
        let mut window_q = world.query_filtered::<&Window, With<PrimaryWindow>>();
        let window = window_q.single(world).ok()?;
        window.cursor_position()?
    };

    if cursor.x < vp_min.x
        || cursor.y < vp_min.y
        || cursor.x > vp_min.x + vp_size.x
        || cursor.y > vp_min.y + vp_size.y
    {
        return None;
    }

    let ray = {
        let mut cam_q = world.query_filtered::<(&Camera, &GlobalTransform), With<EditorCamera>>();
        let (camera, cam_xform) = cam_q.single(world).ok()?;
        let viewport_pos = Vec2::new(
            (cursor.x - vp_min.x) / vp_size.x * current_size.x as f32,
            (cursor.y - vp_min.y) / vp_size.y * current_size.y as f32,
        );
        camera.viewport_to_world(cam_xform, viewport_pos).ok()?
    };

    let dir = ray.direction.as_vec3();
    if dir.y.abs() <= 1e-6 {
        return None;
    }
    let t = -ray.origin.y / dir.y;
    if t <= 0.0 || t > 10_000.0 {
        return None;
    }
    let hit = ray.origin + dir * t;
    Some(Vec3::new(hit.x, 0.0, hit.z))
}

/// Handle file & edit keyboard shortcuts (save, open, settings, etc.).
fn handle_file_shortcuts(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    input_focus: Res<InputFocusState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut modal: ResMut<modal_transform::ModalTransformState>,
    pending_grab: Option<Res<PendingModalGrab>>,
) {
    if keybindings.rebinding.is_some() {
        return;
    }
    if input_focus.egui_wants_keyboard {
        return;
    }
    if mouse_button.pressed(MouseButton::Right) {
        return;
    }
    if modal.active {
        return;
    }

    // Consume pending grab from duplicate-and-move
    if pending_grab.is_some() {
        commands.remove_resource::<PendingModalGrab>();
        modal.pending_grab = true;
    }

    // Save (Ctrl+S)
    if keybindings.just_pressed(EditorAction::SaveScene, &keyboard) {
        commands.insert_resource(renzora::core::SaveSceneRequested);
    }

    // Save As (Ctrl+Shift+S)
    if keybindings.just_pressed(EditorAction::SaveSceneAs, &keyboard) {
        commands.insert_resource(renzora::core::SaveAsSceneRequested);
    }

    // Open Scene (Ctrl+O)
    if keybindings.just_pressed(EditorAction::OpenScene, &keyboard) {
        commands.insert_resource(renzora::core::OpenSceneRequested);
    }

    // New Scene (Ctrl+N)
    if keybindings.just_pressed(EditorAction::NewScene, &keyboard) {
        commands.insert_resource(renzora::core::NewSceneRequested);
    }

    // Settings (Ctrl+,)
    if keybindings.just_pressed(EditorAction::OpenSettings, &keyboard) {
        commands.insert_resource(renzora::core::ToggleSettingsRequested);
    }
}

// ── Mode switching ──────────────────────────────────────────────────────────

fn switch_gizmo_mode(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    input_focus: Res<InputFocusState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    modal: Res<modal_transform::ModalTransformState>,
    mut mode: ResMut<GizmoMode>,
    mut active_tool: ResMut<renzora_editor_framework::ActiveTool>,
) {
    if keybindings.rebinding.is_some() {
        return;
    }
    if input_focus.egui_wants_keyboard {
        return;
    }
    if mouse_button.pressed(MouseButton::Right) {
        return;
    }
    if modal.active {
        return;
    }
    if keybindings.just_pressed(EditorAction::ToolSelect, &keyboard) {
        *mode = GizmoMode::Select;
        *active_tool = renzora_editor_framework::ActiveTool::Select;
    }
    if keybindings.just_pressed(EditorAction::GizmoTranslate, &keyboard) {
        *mode = GizmoMode::Translate;
        *active_tool = renzora_editor_framework::ActiveTool::Translate;
    }
    if keybindings.just_pressed(EditorAction::GizmoRotate, &keyboard) {
        *mode = GizmoMode::Rotate;
        *active_tool = renzora_editor_framework::ActiveTool::Rotate;
    }
    if keybindings.just_pressed(EditorAction::GizmoScale, &keyboard) {
        *mode = GizmoMode::Scale;
        *active_tool = renzora_editor_framework::ActiveTool::Scale;
    }
}

// ── Ray helpers ─────────────────────────────────────────────────────────────

fn viewport_cursor_ray(
    window: &Window,
    viewport: &ViewportState,
    camera_transform: &GlobalTransform,
    projection: &Projection,
) -> Option<Ray3d> {
    let cursor = window.cursor_position()?;
    let vp_local = cursor - viewport.screen_position;
    if vp_local.x < 0.0
        || vp_local.y < 0.0
        || vp_local.x > viewport.screen_size.x
        || vp_local.y > viewport.screen_size.y
    {
        return None;
    }

    let ndc = Vec2::new(
        (vp_local.x / viewport.screen_size.x) * 2.0 - 1.0,
        1.0 - (vp_local.y / viewport.screen_size.y) * 2.0,
    );
    let near = camera_transform.translation();

    match projection {
        Projection::Perspective(persp) => {
            let hh = (persp.fov * 0.5).tan();
            let hw = hh * persp.aspect_ratio;
            let local_dir = Vec3::new(ndc.x * hw, ndc.y * hh, -1.0).normalize();
            let world_dir = camera_transform
                .affine()
                .matrix3
                .mul_vec3(local_dir)
                .normalize();
            Some(Ray3d {
                origin: near,
                direction: Dir3::new(world_dir).ok()?,
            })
        }
        Projection::Orthographic(ortho) => {
            let hw = ortho.area.width() * 0.5;
            let hh = ortho.area.height() * 0.5;
            let offset =
                camera_transform
                    .affine()
                    .matrix3
                    .mul_vec3(Vec3::new(ndc.x * hw, ndc.y * hh, 0.0));
            Some(Ray3d {
                origin: (near + offset),
                direction: camera_transform.forward(),
            })
        }
        _ => None,
    }
}

/// Parameter `s` of the point on the infinite line `origin + dir*s` (`dir`
/// unit) closest to `ray`. `None` when the ray and line are near-parallel.
/// Used to keep the dragged point pinned under the cursor along an axis.
fn ray_line_param(ray: &Ray3d, origin: Vec3, dir: Vec3) -> Option<f32> {
    let d = ray.direction.as_vec3();
    let b = d.dot(dir);
    let denom = 1.0 - b * b;
    if denom.abs() < 1e-6 {
        return None;
    }
    let w0 = ray.origin - origin;
    Some((dir.dot(w0) - b * d.dot(w0)) / denom)
}

/// World point where `ray` meets the plane through `origin` with `normal`.
/// `None` when the ray is parallel to (or pointing away from) the plane.
fn ray_plane_point(ray: &Ray3d, origin: Vec3, normal: Vec3) -> Option<Vec3> {
    let d = ray.direction.as_vec3();
    let denom = d.dot(normal);
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = (origin - ray.origin).dot(normal) / denom;
    if t < 0.0 {
        return None;
    }
    Some(ray.origin + d * t)
}

/// The world point under the cursor projected onto the active translate handle:
/// the closest point on the axis line for a single axis, or the ray–plane hit
/// for a plane handle. `None` if the cursor isn't over the viewport or the
/// projection is degenerate.
fn translate_cursor_point(ray: &Ray3d, pivot: Vec3, basis: Quat, axis: GizmoAxis) -> Option<Vec3> {
    if axis.is_plane() {
        ray_plane_point(ray, pivot, basis * axis.direction())
    } else {
        let dir = basis * axis.direction();
        ray_line_param(ray, pivot, dir).map(|s| pivot + dir * s)
    }
}

fn closest_distance_ray_segment(ray: &Ray3d, seg_a: Vec3, seg_b: Vec3) -> Option<f32> {
    let ro: Vec3 = ray.origin;
    let rd: Vec3 = ray.direction.as_vec3();
    let sd = seg_b - seg_a;
    let sl = sd.length();
    if sl < 1e-6 {
        return None;
    }
    let su = sd / sl;
    let w0 = ro - seg_a;
    let a = rd.dot(rd);
    let b = rd.dot(su);
    let c = su.dot(su);
    let d = rd.dot(w0);
    let e = su.dot(w0);
    let denom = a * c - b * b;
    if denom.abs() < 1e-8 {
        return None;
    }
    let t_ray = (b * e - c * d) / denom;
    let t_seg = (a * e - b * d) / denom;
    if t_ray < 0.0 {
        return None;
    }
    let tc = t_seg.clamp(0.0, sl);
    Some((ro + rd * t_ray - (seg_a + su * tc)).length())
}

fn ray_circle_distance(ray: &Ray3d, center: Vec3, normal: Vec3, radius: f32) -> Option<f32> {
    let (p1, p2) = perpendicular_pair(normal);
    let segs = 32;
    let mut best: Option<f32> = None;
    for i in 0..segs {
        let a0 = (i as f32 / segs as f32) * std::f32::consts::TAU;
        let a1 = ((i + 1) as f32 / segs as f32) * std::f32::consts::TAU;
        let s0 = center + (p1 * a0.cos() + p2 * a0.sin()) * radius;
        let s1 = center + (p1 * a1.cos() + p2 * a1.sin()) * radius;
        if let Some(d) = closest_distance_ray_segment(ray, s0, s1) {
            if best.is_none_or(|b| d < b) {
                best = Some(d);
            }
        }
    }
    best
}

fn ray_hits_plane_quad(ray: &Ray3d, corner: Vec3, axis_a: Vec3, axis_b: Vec3, size: f32) -> bool {
    let normal = axis_a.cross(axis_b).normalize();
    let ro: Vec3 = ray.origin;
    let rd: Vec3 = ray.direction.as_vec3();
    let denom = normal.dot(rd);
    if denom.abs() < 1e-6 {
        return false;
    }
    let t = normal.dot(corner - ro) / denom;
    if t < 0.0 {
        return false;
    }
    let hit = ro + rd * t;
    let local = hit - corner;
    let u = local.dot(axis_a);
    let v = local.dot(axis_b);
    u >= 0.0 && u <= size && v >= 0.0 && v <= size
}

fn perpendicular_pair(normal: Vec3) -> (Vec3, Vec3) {
    let p1 = if normal.y.abs() > 0.9 {
        Vec3::X
    } else {
        normal.cross(Vec3::Y).normalize()
    };
    let p2 = normal.cross(p1).normalize();
    (p1, p2)
}

fn pick_threshold(
    cam_gt: &GlobalTransform,
    entity_pos: Vec3,
    projection: &Projection,
    vh: f32,
) -> f32 {
    let dist = (cam_gt.translation() - entity_pos).length();
    let px = 12.0;
    match projection {
        Projection::Perspective(persp) => dist * (persp.fov * 0.5).tan() * 2.0 * px / vh,
        Projection::Orthographic(ortho) => ortho.area.height() * px / vh,
        _ => 0.1,
    }
}

// ── Hover detection ─────────────────────────────────────────────────────────

fn gizmo_hover_detect(
    mut gizmo_state: ResMut<GizmoState>,
    mode: Res<GizmoMode>,
    space: Res<GizmoSpace>,
    selection: Res<EditorSelection>,
    viewport: Option<Res<ViewportState>>,
    camera_q: Query<(&GlobalTransform, &Projection), With<EditorCamera>>,
    transform_q: Query<&GlobalTransform, Without<EditorCamera>>,
    aabbs: Query<(Option<&bevy::camera::primitives::Aabb>, &GlobalTransform), With<Mesh3d>>,
    children_q: Query<&Children>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    modal: Res<modal_transform::ModalTransformState>,
) {
    if modal.active {
        gizmo_state.hovered_axis = None;
        return;
    }
    if matches!(*mode, GizmoMode::Select | GizmoMode::None) {
        gizmo_state.hovered_axis = None;
        return;
    }
    if gizmo_state.active_axis.is_some() {
        return;
    }
    gizmo_state.hovered_axis = None;

    let Some(selected) = selection.get() else {
        return;
    };
    let Some(viewport) = viewport.as_ref() else {
        return;
    };
    if !viewport.hovered {
        return;
    }
    if mouse_button.pressed(MouseButton::Right) || mouse_button.pressed(MouseButton::Middle) {
        return;
    }

    let Ok((cam_gt, projection)) = camera_q.single() else {
        return;
    };
    let Ok(entity_gt) = transform_q.get(selected) else {
        return;
    };
    let Ok(window) = window_q.single() else {
        return;
    };
    let Some(ray) = viewport_cursor_ray(window, viewport, cam_gt, projection) else {
        return;
    };

    let entity_pos = compute_gizmo_pivot(selected, &aabbs, &children_q, entity_gt);
    let gs = gizmo_state.gizmo_scale.max(0.01);
    let gizmo_size = GIZMO_SIZE * gs;
    let threshold = pick_threshold(cam_gt, entity_pos, projection, viewport.screen_size.y);
    // Same orientation the handles are drawn with, so picking matches visuals.
    let basis = gizmo_basis(*space, *mode, entity_gt.rotation());

    let mut best: Option<(GizmoAxis, f32)> = None;

    match *mode {
        GizmoMode::Select | GizmoMode::None => unreachable!(),
        GizmoMode::Translate => {
            // Plane squares first — inner corner at the origin, two edges along
            // the (signed, camera-facing) axes, matching `draw_line_gizmos`.
            let side = GIZMO_PLANE_SIZE * gs;
            for plane in PLANES {
                let (sa, sb) = plane.signed_plane_axes(gizmo_state.axis_signs).unwrap();
                let a = basis * sa;
                let b = basis * sb;
                if ray_hits_plane_quad(&ray, entity_pos, a, b, side) {
                    best = Some((plane, 0.0));
                    break;
                }
            }
            if best.is_none() {
                for axis in AXES {
                    let dir = basis * axis.signed_direction(gizmo_state.axis_signs);
                    if let Some(dist) = closest_distance_ray_segment(
                        &ray,
                        entity_pos,
                        entity_pos + dir * gizmo_size,
                    ) {
                        if dist < threshold && best.is_none_or(|(_, d)| dist < d) {
                            best = Some((axis, dist));
                        }
                    }
                }
            }
        }
        GizmoMode::Scale => {
            for axis in AXES {
                let dir = basis * axis.signed_direction(gizmo_state.axis_signs);
                if let Some(dist) =
                    closest_distance_ray_segment(&ray, entity_pos, entity_pos + dir * gizmo_size)
                {
                    if dist < threshold && best.is_none_or(|(_, d)| dist < d) {
                        best = Some((axis, dist));
                    }
                }
            }
        }
        GizmoMode::Rotate => {
            let radius = gizmo_size * 0.7;
            for axis in AXES {
                if let Some(dist) =
                    ray_circle_distance(&ray, entity_pos, basis * axis.direction(), radius)
                {
                    if dist < threshold && best.is_none_or(|(_, d)| dist < d) {
                        best = Some((axis, dist));
                    }
                }
            }
        }
    }

    gizmo_state.hovered_axis = best.map(|(a, _)| a);
}

// ── Drag handling ───────────────────────────────────────────────────────────

fn acquire_drag_cursor(cursor_q: &mut Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if let Ok(mut cursor) = cursor_q.single_mut() {
        cursor.visible = false;
        cursor.grab_mode = CursorGrabMode::Locked;
    }
}

fn release_drag_cursor(cursor_q: &mut Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if let Ok(mut cursor) = cursor_q.single_mut() {
        cursor.visible = true;
        cursor.grab_mode = CursorGrabMode::None;
    }
}

/// Geometry queries shared by the drag system, bundled so `gizmo_drag` stays
/// under Bevy's 16-parameter system limit.
#[derive(SystemParam)]
struct DragGeom<'w, 's> {
    global: Query<'w, 's, &'static GlobalTransform, Without<EditorCamera>>,
    aabb: Query<'w, 's, &'static bevy::camera::primitives::Aabb>,
    pivot_aabbs: Query<
        'w,
        's,
        (
            Option<&'static bevy::camera::primitives::Aabb>,
            &'static GlobalTransform,
        ),
        With<Mesh3d>,
    >,
    children: Query<'w, 's, &'static Children>,
}

fn gizmo_drag(
    mut gizmo_state: ResMut<GizmoState>,
    mode: Res<GizmoMode>,
    space: Res<GizmoSpace>,
    selection: Res<EditorSelection>,
    collider_edit: Option<Res<renzora_physics::ColliderEditMode>>,
    viewport: Option<Res<ViewportState>>,
    viewport_settings: Option<Res<ViewportSettings>>,
    camera_q: Query<(&GlobalTransform, &Projection), With<EditorCamera>>,
    mut transform_q: Query<
        &mut Transform,
        (
            Without<EditorCamera>,
            Without<EditorLocked>,
            Without<GizmoRoot>,
            Without<GizmoMesh>,
        ),
    >,
    geom: DragGeom,
    window_q: Query<&Window, With<PrimaryWindow>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut cursor_options: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut commands: Commands,
) {
    let snap: SnapSettings = viewport_settings
        .as_deref()
        .map(|s| s.snap)
        .unwrap_or_default();
    if matches!(*mode, GizmoMode::Select | GizmoMode::None) {
        mouse_motion.clear();
        return;
    }
    if collider_edit.map(|c| c.active).unwrap_or(false) {
        if gizmo_state.active_axis.is_some() {
            release_drag_cursor(&mut cursor_options);
        }
        gizmo_state.active_axis = None;
        gizmo_state.drag_starts.clear();
        mouse_motion.clear();
        return;
    }

    let selected_entities = selection.get_all();
    if selected_entities.is_empty() {
        if gizmo_state.active_axis.is_some() {
            release_drag_cursor(&mut cursor_options);
        }
        gizmo_state.active_axis = None;
        gizmo_state.drag_starts.clear();
        mouse_motion.clear();
        return;
    }

    if mouse_button.pressed(MouseButton::Right) || mouse_button.pressed(MouseButton::Middle) {
        mouse_motion.clear();
        return;
    }

    // Start drag
    if mouse_button.just_pressed(MouseButton::Left) && gizmo_state.active_axis.is_none() {
        if let Some(axis) = gizmo_state.hovered_axis {
            let mut starts = Vec::new();
            let mut parents = Vec::new();
            for &entity in &selected_entities {
                if let Ok(t) = transform_q.get(entity) {
                    starts.push((entity, t.translation, t.rotation, t.scale));
                    // Parent frame = world * local⁻¹, captured now (the parent is
                    // stationary for the gesture). Identity when unparented.
                    let parent = geom.global
                        .get(entity)
                        .map(|gt| transform_space::parent_affine(gt, t))
                        .unwrap_or(bevy::math::Affine3A::IDENTITY);
                    parents.push(parent);
                }
            }
            // Capture the handle orientation and the world pivot now, so both
            // stay fixed for the whole gesture (in Local space the live basis
            // would otherwise drift as the object rotates).
            let sel_world_rot = selection
                .get()
                .and_then(|e| geom.global.get(e).ok())
                .map(|gt| gt.rotation())
                .unwrap_or(Quat::IDENTITY);
            let mut pivot_sum = Vec3::ZERO;
            let mut pivot_n = 0u32;
            for &e in &selected_entities {
                if let Ok(gt) = geom.global.get(e) {
                    pivot_sum += compute_gizmo_pivot(e, &geom.pivot_aabbs, &geom.children, gt);
                    pivot_n += 1;
                }
            }
            gizmo_state.drag_basis = gizmo_basis(*space, *mode, sel_world_rot);
            gizmo_state.drag_pivot = if pivot_n > 0 {
                pivot_sum / pivot_n as f32
            } else {
                Vec3::ZERO
            };
            // Reference point under the cursor on the dragged axis/plane, so
            // translate can keep it pinned to the pointer (cursor-locked drag).
            let pivot0 = gizmo_state.drag_pivot;
            let basis = gizmo_state.drag_basis;
            gizmo_state.drag_grab = camera_q
                .single()
                .ok()
                .zip(window_q.single().ok())
                .and_then(|((cam_gt, projection), window)| {
                    let vp = viewport.as_ref()?;
                    let ray = viewport_cursor_ray(window, vp, cam_gt, projection)?;
                    translate_cursor_point(&ray, pivot0, basis, axis)
                })
                .unwrap_or(pivot0);
            gizmo_state.active_axis = Some(axis);
            gizmo_state.drag_starts = starts;
            gizmo_state.drag_parents = parents;
            gizmo_state.drag_offset = Vec3::ZERO;
            gizmo_state.drag_angle = 0.0;
            gizmo_state.drag_scale_factor = 0.0;
            // Leave the cursor visible and free while dragging — the drag tracks
            // raw mouse motion either way, and locking it in place feels frozen.
            mouse_motion.clear();
            return;
        }
    }

    // End drag
    if mouse_button.just_released(MouseButton::Left) && gizmo_state.active_axis.is_some() {
        let mut records: Vec<(Entity, Transform, Transform)> = Vec::new();
        for (entity, old_t, old_r, old_s) in &gizmo_state.drag_starts {
            let Ok(t) = transform_q.get(*entity) else {
                continue;
            };
            let old = Transform {
                translation: *old_t,
                rotation: *old_r,
                scale: *old_s,
            };
            let new = *t;
            if old.translation == new.translation
                && old.rotation == new.rotation
                && old.scale == new.scale
            {
                continue;
            }
            records.push((*entity, old, new));
        }
        if !records.is_empty() {
            commands.queue(move |world: &mut World| {
                for (entity, old, new) in records {
                    renzora_undo::record(
                        world,
                        renzora_undo::UndoContext::Scene,
                        Box::new(renzora_undo::TransformCmd { entity, old, new }),
                    );
                }
            });
        }
        gizmo_state.active_axis = None;
        gizmo_state.drag_starts.clear();
        release_drag_cursor(&mut cursor_options);
        mouse_motion.clear();
        return;
    }

    let Some(axis) = gizmo_state.active_axis else {
        mouse_motion.clear();
        return;
    };

    if !mouse_button.pressed(MouseButton::Left) {
        gizmo_state.active_axis = None;
        gizmo_state.drag_starts.clear();
        release_drag_cursor(&mut cursor_options);
        mouse_motion.clear();
        return;
    }

    let Ok((cam_gt, projection)) = camera_q.single() else {
        mouse_motion.clear();
        return;
    };
    let Some(viewport) = viewport.as_ref() else {
        mouse_motion.clear();
        return;
    };

    let mut total_delta = Vec2::ZERO;
    for ev in mouse_motion.read() {
        total_delta += ev.delta;
    }
    if total_delta.length_squared() < 1e-6 {
        return;
    }

    // Drag-start positions are local-space (so writes go back into local
    // Transform). For camera-distance scaling we need the world-space pivot,
    // so average GlobalTransform translations of the selected entities.
    let center = if gizmo_state.drag_starts.is_empty() {
        Vec3::ZERO
    } else {
        let sum: Vec3 = gizmo_state.drag_starts.iter().map(|(_, t, _, _)| *t).sum();
        sum / gizmo_state.drag_starts.len() as f32
    };
    let world_center = if selected_entities.is_empty() {
        center
    } else {
        let mut sum = Vec3::ZERO;
        let mut n = 0u32;
        for &e in &selected_entities {
            if let Ok(gt) = geom.global.get(e) {
                sum += compute_gizmo_pivot(e, &geom.pivot_aabbs, &geom.children, gt);
                n += 1;
            }
        }
        if n > 0 {
            sum / n as f32
        } else {
            center
        }
    };
    let distance = (cam_gt.translation() - world_center).length();

    match *mode {
        GizmoMode::Select | GizmoMode::None => unreachable!(),
        GizmoMode::Translate => {
            // Cursor-locked: pin the grabbed point under the pointer. Project the
            // cursor ray onto the active axis/plane and move by how far that
            // point has travelled from the grab reference captured at drag start.
            // Absolute (not accumulated), so the handle never drifts off-cursor.
            let Ok(window) = window_q.single() else {
                return;
            };
            let Some(ray) = viewport_cursor_ray(window, viewport, cam_gt, projection) else {
                return;
            };
            let Some(cur) =
                translate_cursor_point(&ray, gizmo_state.drag_pivot, gizmo_state.drag_basis, axis)
            else {
                return;
            };
            let total_offset = cur - gizmo_state.drag_grab;
            for (i, &entity) in selected_entities.iter().enumerate() {
                if let Ok(mut t) = transform_q.get_mut(entity) {
                    let (start_t, start_r, start_s) = gizmo_state
                        .drag_starts
                        .get(i)
                        .map(|(_, p, r, s)| (*p, *r, *s))
                        .unwrap_or((t.translation, t.rotation, t.scale));
                    let parent = gizmo_state
                        .drag_parents
                        .get(i)
                        .copied()
                        .unwrap_or(bevy::math::Affine3A::IDENTITY);
                    // Convert the world-space drag into the entity's parent frame
                    // so it moves along the gizmo's axis, not a parent-rotated one.
                    let mut new_pos =
                        transform_space::world_translation(start_t, total_offset, &parent);
                    if snap.translate_enabled && snap.translate_snap > 0.0 {
                        let step = snap.translate_snap;
                        // For edge-snap, snap the world-space AABB min corner
                        // (computed from the drag-start transform since rot/scale
                        // don't change during translate) to the grid, then derive
                        // the required pivot position.
                        let min_offset = if snap.translate_edge_snap {
                            geom.aabb.get(entity).ok().map(|aabb| {
                                world_aabb_min(aabb, start_t, start_r, start_s) - start_t
                            })
                        } else {
                            None
                        };
                        if let Some(off) = min_offset {
                            let target = new_pos + off;
                            let snapped = Vec3::new(
                                (target.x / step).round() * step,
                                (target.y / step).round() * step,
                                (target.z / step).round() * step,
                            );
                            new_pos = snapped - off;
                        } else {
                            new_pos = Vec3::new(
                                (new_pos.x / step).round() * step,
                                (new_pos.y / step).round() * step,
                                (new_pos.z / step).round() * step,
                            );
                        }
                    }
                    t.translation = new_pos;
                }
            }
        }
        GizmoMode::Rotate => {
            // Rotation axis in world space (world or the object's own axis).
            let world_axis = gizmo_state.drag_basis * axis.direction();
            let delta_angle = screen_delta_to_angle(total_delta, world_axis, cam_gt);
            gizmo_state.drag_angle += delta_angle;

            // If snap is on, snap the accumulated drag_angle to the step and
            // apply the delta needed to reach the snapped value from starts.
            let effective_angle = if snap.rotate_enabled && snap.rotate_snap > 0.0 {
                let step = snap.rotate_snap.to_radians();
                (gizmo_state.drag_angle / step).round() * step
            } else {
                gizmo_state.drag_angle
            };
            let world_rot = Quat::from_axis_angle(world_axis, effective_angle);
            let pivot = gizmo_state.drag_pivot;
            for (i, &entity) in selected_entities.iter().enumerate() {
                if let Ok(mut t) = transform_q.get_mut(entity) {
                    let (start_t, start_r, start_s) = gizmo_state
                        .drag_starts
                        .get(i)
                        .map(|(_, p, r, s)| (*p, *r, *s))
                        .unwrap_or((t.translation, t.rotation, t.scale));
                    let parent = gizmo_state
                        .drag_parents
                        .get(i)
                        .copied()
                        .unwrap_or(bevy::math::Affine3A::IDENTITY);
                    // Rotate about the shared world pivot so single and group
                    // rotations both pivot in place.
                    transform_space::pivot_rotation(
                        &mut t, start_t, start_r, start_s, world_rot, pivot, &parent,
                    );
                }
            }
        }
        GizmoMode::Scale => {
            // Scale is always along the object's own axes; the handle's world
            // direction is what the screen delta projects onto.
            let handle_dir = gizmo_state.drag_basis * axis.direction();
            let delta_scale = screen_delta_to_scale(total_delta, handle_dir, cam_gt);
            gizmo_state.drag_scale_factor += delta_scale;
            let snap_step = if snap.scale_enabled && snap.scale_snap > 0.0 {
                Some(snap.scale_snap)
            } else {
                None
            };
            let apply = |v: f32, step: Option<f32>| -> f32 {
                let v = v.max(0.01);
                match step {
                    Some(s) => ((v / s).round() * s).max(s.min(0.01)),
                    None => v,
                }
            };
            let f = gizmo_state.drag_scale_factor;
            let pivot = gizmo_state.drag_pivot;
            for (i, &entity) in selected_entities.iter().enumerate() {
                if let Ok(mut t) = transform_q.get_mut(entity) {
                    let (start_t, start_r, start_scale) = gizmo_state
                        .drag_starts
                        .get(i)
                        .map(|(_, p, r, s)| (*p, *r, *s))
                        .unwrap_or((t.translation, t.rotation, t.scale));
                    let parent = gizmo_state
                        .drag_parents
                        .get(i)
                        .copied()
                        .unwrap_or(bevy::math::Affine3A::IDENTITY);
                    let mut new_scale = start_scale;
                    match axis {
                        GizmoAxis::X => new_scale.x = apply(start_scale.x + f, snap_step),
                        GizmoAxis::Y => new_scale.y = apply(start_scale.y + f, snap_step),
                        GizmoAxis::Z => new_scale.z = apply(start_scale.z + f, snap_step),
                        _ => {}
                    }
                    // Scale about the world pivot so the object stays in place
                    // (translation is compensated through the parent frame).
                    transform_space::pivot_scale(
                        &mut t, start_t, start_r, start_scale, new_scale, pivot, &parent,
                    );
                }
            }
        }
    }
}

fn screen_delta_to_angle(mouse_delta: Vec2, axis_world: Vec3, cam: &GlobalTransform) -> f32 {
    let cam_fwd = cam.forward().as_vec3();
    let dot = axis_world.dot(cam_fwd).abs();
    let sens = 0.005;
    if dot > 0.7 {
        (mouse_delta.x - mouse_delta.y) * sens
    } else {
        let cr = cam.right().as_vec3();
        let cu = cam.up().as_vec3();
        let sa = Vec2::new(axis_world.dot(cr), -axis_world.dot(cu));
        let sp = Vec2::new(-sa.y, sa.x);
        let len = sp.length();
        if len < 1e-4 {
            0.0
        } else {
            mouse_delta.dot(sp / len) * sens
        }
    }
}

fn screen_delta_to_scale(mouse_delta: Vec2, axis_world: Vec3, cam: &GlobalTransform) -> f32 {
    let cr = cam.right().as_vec3();
    let cu = cam.up().as_vec3();
    let sa = Vec2::new(axis_world.dot(cr), -axis_world.dot(cu));
    let len = sa.length();
    if len < 1e-4 {
        0.0
    } else {
        mouse_delta.dot(sa / len) * 0.005
    }
}

// ── Click diagnostics ───────────────────────────────────────────────────────

/// Console diagnostic for "click bleed" between panels: on each left press it
/// logs the cursor, whether the 3D viewport thinks it's hovered, every UI node
/// that took `Interaction::Pressed`, and the selection; on release it logs the
/// selection again. Runs in every view (not just 3D) so UI-canvas selection is
/// covered too. Gated on [`ClickDebug`] (default on).
fn click_diag(
    mouse: Res<ButtonInput<MouseButton>>,
    debug: Option<Res<renzora::core::ClickDebug>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    viewport: Option<Res<ViewportState>>,
    selection: Res<EditorSelection>,
    interactions: Query<(Entity, &Interaction, Option<&Name>)>,
) {
    if !debug.map(|d| d.0).unwrap_or(false) {
        return;
    }
    let pressed = mouse.just_pressed(MouseButton::Left);
    let released = mouse.just_released(MouseButton::Left);
    if !pressed && !released {
        return;
    }
    let cursor = windows.iter().next().and_then(|w| w.cursor_position());
    if pressed {
        let mut hits: Vec<String> = interactions
            .iter()
            .filter(|(_, i, _)| **i == Interaction::Pressed)
            .map(|(e, _, name)| {
                name.map(|n| n.as_str().to_string())
                    .unwrap_or_else(|| format!("{e:?}"))
            })
            .collect();
        hits.sort();
        let vp = viewport
            .as_ref()
            .map(|v| {
                format!(
                    "hovered={} pos=({:.0},{:.0}) size=({:.0},{:.0})",
                    v.hovered, v.screen_position.x, v.screen_position.y, v.screen_size.x, v.screen_size.y
                )
            })
            .unwrap_or_else(|| "none".into());
        let cur = cursor
            .map(|c| format!("({:.0},{:.0})", c.x, c.y))
            .unwrap_or_else(|| "?".into());
        renzora::core::console_log::console_info(
            "Click",
            format!(
                "press @ {cur} | viewport[{vp}] | pressed=[{}] | sel_before={:?}",
                hits.join(", "),
                selection.get()
            ),
        );
    }
    if released {
        renzora::core::console_log::console_info(
            "Click",
            format!("release | sel_after={:?}", selection.get()),
        );
    }
}

// ── Entity picking (click to select) ────────────────────────────────────────

fn entity_pick_system(
    gizmo_state: Res<GizmoState>,
    mode: Res<GizmoMode>,
    modal: Res<modal_transform::ModalTransformState>,
    collider_edit: Option<Res<renzora_physics::ColliderEditMode>>,
    viewport: Option<Res<ViewportState>>,
    nav_overlay: Option<Res<NavOverlayState>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    settings: Res<EditorSettings>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    mut mesh_ray_cast: MeshRayCast,
    named_entities: Query<(Entity, Has<SelectionStop>), With<Name>>,
    parent_query: Query<&ChildOf>,
    gizmo_meshes: Query<(), Or<(With<GizmoMesh>, With<GizmoRoot>)>>,
    hidden_entities: Query<(), With<HideInHierarchy>>,
    mut box_sel: ResMut<BoxSelectionState>,
) {
    if !mouse_button.just_pressed(MouseButton::Left) {
        return;
    }
    if modal.active {
        return;
    }
    if gizmo_state.active_axis.is_some() || gizmo_state.hovered_axis.is_some() {
        return;
    }
    // Suspend picking while editing a collider — clicks drive handle drags instead.
    if collider_edit.map(|c| c.active).unwrap_or(false) {
        // If a handle is hovered or being dragged, fully consume the click.
        // Otherwise still suppress to avoid deselecting while the user is tweaking.
        return;
    }
    // GizmoMode::None means a plugin tool is driving — skip picking.
    if *mode == GizmoMode::None {
        return;
    }
    // Don't pick while nav overlay buttons (pan/zoom/orbit) are being dragged
    if let Some(ref nav) = nav_overlay {
        if nav.pan_dragging.load(std::sync::atomic::Ordering::Relaxed)
            || nav.zoom_dragging.load(std::sync::atomic::Ordering::Relaxed)
            || nav
                .orbit_dragging
                .load(std::sync::atomic::Ordering::Relaxed)
        {
            return;
        }
    }

    let Some(viewport) = viewport.as_ref() else {
        return;
    };
    if !viewport.hovered {
        return;
    }

    let Ok(window) = window_q.single() else {
        return;
    };
    let Ok((camera, cam_gt)) = camera_q.single() else {
        return;
    };

    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let vp_local = cursor - viewport.screen_position;
    if vp_local.x < 0.0
        || vp_local.y < 0.0
        || vp_local.x > viewport.screen_size.x
        || vp_local.y > viewport.screen_size.y
    {
        return;
    }

    // The render target may be smaller than the on-screen panel (Half / Quarter
    // resolution), so map the panel-local cursor into render-target pixels
    // before building the pick ray — otherwise clicks land off-target.
    if viewport.screen_size.x <= 0.0 || viewport.screen_size.y <= 0.0 {
        return;
    }
    let render_pos = Vec2::new(
        vp_local.x / viewport.screen_size.x * viewport.current_size.x as f32,
        vp_local.y / viewport.screen_size.y * viewport.current_size.y as f32,
    );

    // Modifiers are read at release time in `box_selection_system` — on
    // press we just arm the gesture.
    let Ok(ray) = camera.viewport_to_world(cam_gt, render_pos) else {
        return;
    };

    // Raycast and find the topmost selectable entity (if any). We do NOT
    // commit the selection yet — we arm `box_sel` with this entity as a
    // pending pick and wait for mouse-up to decide click vs drag.
    let hits = mesh_ray_cast.cast_ray(ray, &MeshRayCastSettings { ..default() });
    let mut pending: Option<Entity> = None;
    for (entity, _hit) in hits.iter() {
        if gizmo_meshes.get(*entity).is_ok() {
            continue;
        }
        if hidden_entities.get(*entity).is_ok() {
            continue;
        }

        if let Some(target) = resolve_pick(
            *entity,
            settings.selection_granularity,
            &named_entities,
            &parent_query,
            &hidden_entities,
        ) {
            // `resolve_pick` already skips hidden named ancestors, so `target`
            // is a visible row — this guard is a belt-and-braces no-op.
            if hidden_entities.get(target).is_ok() {
                continue;
            }
            pending = Some(target);
            break;
        }
    }

    // Arm the gesture. `box_selection_system` reads these fields each frame
    // and finalises on release. Only arm from tools where box / click
    // selection is meaningful.
    if matches!(
        *mode,
        GizmoMode::Select | GizmoMode::Translate | GizmoMode::Rotate | GizmoMode::Scale
    ) {
        box_sel.active = true;
        box_sel.start_pos = cursor;
        box_sel.current_pos = cursor;
        box_sel.pending_pick = pending;
    }
}

// ── Box selection system ─────────────────────────────────────────────────────

fn box_selection_system(
    mut box_sel: ResMut<BoxSelectionState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    collider_edit: Option<Res<renzora_physics::ColliderEditMode>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    viewport: Option<Res<ViewportState>>,
    nav_overlay: Option<Res<NavOverlayState>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    selection: Res<EditorSelection>,
    named_entities: Query<(Entity, &GlobalTransform), With<Name>>,
    hidden_entities: Query<(), With<HideInHierarchy>>,
    gizmo_meshes: Query<(), Or<(With<GizmoMesh>, With<GizmoRoot>)>>,
    box_select_excluded: Query<
        (),
        Or<(
            With<renzora_terrain::data::TerrainData>,
            With<renzora_terrain::data::TerrainChunkOf>,
            With<renzora_lighting::Sun>,
        )>,
    >,
) {
    if collider_edit.map(|c| c.active).unwrap_or(false) {
        box_sel.active = false;
        return;
    }
    if !box_sel.active {
        return;
    }
    // Cancel box selection if nav overlay is being used
    if let Some(ref nav) = nav_overlay {
        if nav.pan_dragging.load(std::sync::atomic::Ordering::Relaxed)
            || nav.zoom_dragging.load(std::sync::atomic::Ordering::Relaxed)
            || nav
                .orbit_dragging
                .load(std::sync::atomic::Ordering::Relaxed)
        {
            box_sel.active = false;
            return;
        }
    }

    let Ok(window) = window_q.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };

    // Update current position while dragging
    if mouse_button.pressed(MouseButton::Left) {
        box_sel.current_pos = cursor;
        return;
    }

    // Mouse released — finalize gesture.
    box_sel.active = false;
    let pending_pick = box_sel.pending_pick.take();

    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

    if !box_sel.is_drag() {
        // Click (no drag): commit the pending pick, or deselect on empty space.
        match pending_pick {
            Some(target) => {
                if ctrl {
                    selection.toggle(target);
                } else if shift {
                    if !selection.is_selected(target) {
                        selection.toggle(target);
                    }
                } else {
                    selection.set(Some(target));
                }
            }
            None => {
                if !shift && !ctrl {
                    selection.set(None);
                }
            }
        }
        return;
    }

    let Some(viewport) = viewport.as_ref() else {
        return;
    };
    let Ok((camera, cam_gt)) = camera_q.single() else {
        return;
    };

    let (box_min, box_max) = box_sel.get_rect();

    // Find all named entities whose screen projection falls within the box
    let mut entities_in_box = Vec::new();

    for (entity, global_transform) in named_entities.iter() {
        if hidden_entities.get(entity).is_ok() {
            continue;
        }
        if gizmo_meshes.get(entity).is_ok() {
            continue;
        }
        if box_select_excluded.get(entity).is_ok() {
            continue;
        }

        let world_pos = global_transform.translation();
        let Some(ndc) = camera.world_to_ndc(cam_gt, world_pos) else {
            continue;
        };

        // Must be in front of camera
        if ndc.z < 0.0 || ndc.z > 1.0 {
            continue;
        }

        // Convert NDC to screen coordinates
        let screen_x = viewport.screen_position.x + (ndc.x + 1.0) * 0.5 * viewport.screen_size.x;
        let screen_y = viewport.screen_position.y + (1.0 - ndc.y) * 0.5 * viewport.screen_size.y;

        if screen_x >= box_min.x
            && screen_x <= box_max.x
            && screen_y >= box_min.y
            && screen_y <= box_max.y
        {
            entities_in_box.push(entity);
        }
    }

    if entities_in_box.is_empty() {
        if !shift && !ctrl {
            selection.set(None);
        }
        return;
    }

    if shift {
        // Add to existing selection
        let mut current = selection.get_all();
        for e in entities_in_box {
            if !current.contains(&e) {
                current.push(e);
            }
        }
        selection.set_multiple(current);
    } else if ctrl {
        // Toggle each entity
        for e in entities_in_box {
            selection.toggle(e);
        }
    } else {
        // Replace selection
        selection.set_multiple(entities_in_box);
    }
}

/// Resolve a raycast-hit entity to the entity a click should select, per the
/// configured [`SelectionGranularity`].
///
/// Walking up from the hit mesh toward the scene root, three candidates fall
/// out of a single pass:
///   - `leaf`  — the nearest named ancestor (the clicked mesh itself)
///   - `group` — the topmost named ancestor still *below* a `SelectionStop`
///     boundary (the clicked mesh's own sub-root within an imported model)
///   - `root`  — the `SelectionStop` bearer (the whole imported model), or the
///     topmost named ancestor when the chain has no stop.
///
/// `SelectionStop` marks a compound boundary (an imported model root, terrain
/// root, etc.) whose internals are selected as a unit under `EntireRoot`.
///
/// Entities tagged [`HideInHierarchy`] are transparent here: an imported model
/// often carries a named-but-hidden GLTF wrapper (`RootNode`, `Scene`, …) that
/// `hide_gltf_wrappers` flagged when flatten couldn't collapse it. The hierarchy
/// panel hides those rows and re-parents their children to the nearest visible
/// ancestor, so the click resolution must mirror that — otherwise `MeshRoot`
/// could land on a hidden wrapper and the caller would reject it, selecting
/// nothing.
fn resolve_pick(
    entity: Entity,
    granularity: SelectionGranularity,
    named: &Query<(Entity, Has<SelectionStop>), With<Name>>,
    parents: &Query<&ChildOf>,
    hidden: &Query<(), With<HideInHierarchy>>,
) -> Option<Entity> {
    let mut leaf: Option<Entity> = None;
    let mut group: Option<Entity> = None;
    let mut root: Option<Entity> = None;
    let mut current = entity;
    loop {
        if let Ok((e, stop)) = named.get(current) {
            let visible = hidden.get(e).is_err();
            if visible {
                if leaf.is_none() {
                    leaf = Some(e);
                }
                if !stop {
                    group = Some(e);
                }
            }
            if stop {
                // A `SelectionStop` is a boundary even if the bearer is hidden,
                // but only a *visible* stop is a valid `EntireRoot` target.
                if visible {
                    root = Some(e);
                }
                break;
            }
        }
        match parents.get(current) {
            Ok(child_of) => current = child_of.parent(),
            Err(_) => break,
        }
    }
    // No `SelectionStop` in the chain: the whole-model root is just the topmost
    // visible named ancestor, which is exactly `group`.
    let root = root.or(group);
    match granularity {
        SelectionGranularity::Mesh => leaf,
        SelectionGranularity::MeshRoot => group.or(leaf),
        SelectionGranularity::EntireRoot => root.or(leaf),
    }
}

// ── Box selection overlay ────────────────────────────────────────────────────

/// Marker for the native bevy_ui box-selection rectangle node.
#[derive(Component)]
struct BoxSelectionRect;

/// Native (bevy_ui) box-selection overlay — a translucent blue rectangle node
/// sized to the drag rect. Replaces the egui-painted version. `get_rect`
/// returns window logical pixels, which map directly to an absolute UI node.
///
/// The node is `Pickable::IGNORE` + `FocusPolicy::Pass` so it never occludes the
/// viewport's hover/pick (the drag itself is driven by `box_selection_system`
/// reading the raw cursor, not UI interaction).
fn render_box_selection(
    mut commands: Commands,
    box_sel: Res<BoxSelectionState>,
    mut existing: Query<(Entity, &mut Node), With<BoxSelectionRect>>,
) {
    if !box_sel.active || !box_sel.is_drag() {
        for (e, _) in &existing {
            commands.entity(e).despawn();
        }
        return;
    }

    let (min, max) = box_sel.get_rect();
    let w = (max.x - min.x).max(0.0);
    let h = (max.y - min.y).max(0.0);

    if let Some((_, mut node)) = existing.iter_mut().next() {
        node.left = Val::Px(min.x);
        node.top = Val::Px(min.y);
        node.width = Val::Px(w);
        node.height = Val::Px(h);
    } else {
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(min.x),
                top: Val::Px(min.y),
                width: Val::Px(w),
                height: Val::Px(h),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(66.0 / 255.0, 150.0 / 255.0, 250.0 / 255.0, 0.157)),
            BorderColor::all(Color::srgb_u8(66, 150, 250)),
            GlobalZIndex(8000),
            bevy::ui::FocusPolicy::Pass,
            bevy::picking::Pickable::IGNORE,
            BoxSelectionRect,
            Name::new("box-selection-rect"),
        ));
    }
}

renzora::add!(GizmoPlugin, Editor);

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::primitives::Aabb;
    use bevy::ecs::system::RunSystemOnce;
    use std::f32::consts::{FRAC_PI_2, PI};

    fn ray(origin: Vec3, dir: Vec3) -> Ray3d {
        Ray3d {
            origin,
            direction: Dir3::new(dir).unwrap(),
        }
    }

    // ── closest_distance_ray_segment ────────────────────────────────────────

    #[test]
    fn ray_segment_distance_at_closest_approach() {
        // Ray crossing 1 unit above the midpoint of a segment on the X axis.
        // The ray runs along +Z so it isn't parallel to the segment (a parallel
        // ray collapses the denominator and returns None — see the degenerate
        // test below).
        let r = ray(Vec3::new(0.0, 1.0, -5.0), Vec3::Z);
        let d = closest_distance_ray_segment(&r, Vec3::new(-1.0, 0.0, 0.0), Vec3::X).unwrap();
        assert!((d - 1.0).abs() < 1e-4, "expected 1.0, got {d}");

        // Ray passing straight through the segment midpoint → ~0.
        let r = ray(Vec3::new(0.0, 5.0, 0.0), Vec3::NEG_Y);
        let d = closest_distance_ray_segment(&r, Vec3::new(-1.0, 0.0, 0.0), Vec3::X).unwrap();
        assert!(d < 1e-4, "expected ~0, got {d}");
    }

    #[test]
    fn ray_segment_distance_clamps_to_endpoints() {
        // Closest point on the infinite line is at x=10, but the segment ends
        // at x=1 — distance must be measured to the endpoint instead.
        let r = ray(Vec3::new(10.0, 5.0, 0.0), Vec3::NEG_Y);
        let d = closest_distance_ray_segment(&r, Vec3::ZERO, Vec3::X).unwrap();
        assert!((d - 9.0).abs() < 1e-3, "expected 9.0, got {d}");
    }

    #[test]
    fn ray_segment_distance_degenerate_cases_return_none() {
        // Parallel ray and segment → denominator collapses.
        let r = ray(Vec3::new(0.0, 1.0, 0.0), Vec3::X);
        assert!(closest_distance_ray_segment(&r, Vec3::ZERO, Vec3::X * 5.0).is_none());

        // Zero-length segment.
        let r = ray(Vec3::new(0.0, 5.0, 0.0), Vec3::NEG_Y);
        assert!(closest_distance_ray_segment(&r, Vec3::ONE, Vec3::ONE).is_none());

        // Closest approach behind the ray origin.
        let r = ray(Vec3::new(0.0, 5.0, 0.0), Vec3::Y);
        assert!(closest_distance_ray_segment(&r, Vec3::new(-1.0, 0.0, 0.0), Vec3::X).is_none());
    }

    // ── ray_circle_distance ─────────────────────────────────────────────────

    #[test]
    fn ray_circle_distance_through_center_is_radius() {
        // Ray down the circle's normal through its center: every point on the
        // circle is `radius` away (modulo the 32-segment polyline chords).
        let r = ray(Vec3::new(0.0, 0.0, 10.0), Vec3::NEG_Z);
        let d = ray_circle_distance(&r, Vec3::ZERO, Vec3::Z, 2.0).unwrap();
        assert!(d > 1.95 && d <= 2.001, "expected ~2.0, got {d}");
    }

    #[test]
    fn ray_circle_distance_at_rim_is_near_zero() {
        let r = ray(Vec3::new(2.0, 0.0, 10.0), Vec3::NEG_Z);
        let d = ray_circle_distance(&r, Vec3::ZERO, Vec3::Z, 2.0).unwrap();
        assert!(d < 0.05, "expected ~0, got {d}");
    }

    // ── ray_hits_plane_quad ─────────────────────────────────────────────────

    #[test]
    fn ray_hits_plane_quad_inside_bounds() {
        // Quad spanning (0,0)..(2,2) on the XY plane, ray hits its middle.
        let r = ray(Vec3::new(1.0, 1.0, 5.0), Vec3::NEG_Z);
        assert!(ray_hits_plane_quad(&r, Vec3::ZERO, Vec3::X, Vec3::Y, 2.0));
    }

    #[test]
    fn ray_hits_plane_quad_rejects_misses() {
        // Hits the plane but outside the quad bounds.
        let r = ray(Vec3::new(3.0, 1.0, 5.0), Vec3::NEG_Z);
        assert!(!ray_hits_plane_quad(&r, Vec3::ZERO, Vec3::X, Vec3::Y, 2.0));

        // Ray parallel to the plane.
        let r = ray(Vec3::new(1.0, 1.0, 5.0), Vec3::X);
        assert!(!ray_hits_plane_quad(&r, Vec3::ZERO, Vec3::X, Vec3::Y, 2.0));

        // Plane behind the ray origin.
        let r = ray(Vec3::new(1.0, 1.0, 5.0), Vec3::Z);
        assert!(!ray_hits_plane_quad(&r, Vec3::ZERO, Vec3::X, Vec3::Y, 2.0));
    }

    // ── perpendicular_pair ──────────────────────────────────────────────────

    #[test]
    fn perpendicular_pair_is_orthonormal() {
        for normal in [Vec3::X, Vec3::Y, Vec3::Z, Vec3::new(1.0, 2.0, 3.0).normalize()] {
            let (p1, p2) = perpendicular_pair(normal);
            assert!((p1.length() - 1.0).abs() < 1e-5, "p1 not unit for {normal}");
            assert!((p2.length() - 1.0).abs() < 1e-5, "p2 not unit for {normal}");
            assert!(p1.dot(p2).abs() < 1e-5, "p1/p2 not orthogonal for {normal}");
            assert!(p1.dot(normal).abs() < 1e-5, "p1 not perp to {normal}");
            assert!(p2.dot(normal).abs() < 1e-5, "p2 not perp to {normal}");
        }
    }

    // ── pick_threshold ──────────────────────────────────────────────────────

    #[test]
    fn pick_threshold_perspective_scales_with_distance() {
        let cam = GlobalTransform::IDENTITY;
        let proj = Projection::Perspective(PerspectiveProjection {
            fov: FRAC_PI_2,
            aspect_ratio: 1.0,
            ..Default::default()
        });
        // tan(fov/2) = 1, so threshold = dist * 2 * 12 / vh.
        let near = pick_threshold(&cam, Vec3::new(0.0, 0.0, -10.0), &proj, 600.0);
        let far = pick_threshold(&cam, Vec3::new(0.0, 0.0, -20.0), &proj, 600.0);
        assert!((near - 0.4).abs() < 1e-4, "got {near}");
        assert!((far - 0.8).abs() < 1e-4, "got {far}");
    }

    #[test]
    fn pick_threshold_orthographic_ignores_distance() {
        let cam = GlobalTransform::IDENTITY;
        let mut ortho = OrthographicProjection::default_3d();
        ortho.area = Rect::new(-5.0, -5.0, 5.0, 5.0);
        let proj = Projection::Orthographic(ortho);
        let near = pick_threshold(&cam, Vec3::new(0.0, 0.0, -10.0), &proj, 600.0);
        let far = pick_threshold(&cam, Vec3::new(0.0, 0.0, -1000.0), &proj, 600.0);
        assert!((near - 0.2).abs() < 1e-4, "got {near}");
        assert_eq!(near, far);
    }

    // ── GizmoAxis ───────────────────────────────────────────────────────────

    #[test]
    fn gizmo_axis_directions_and_plane_classification() {
        assert_eq!(GizmoAxis::X.direction(), Vec3::X);
        assert_eq!(GizmoAxis::Y.direction(), Vec3::Y);
        assert_eq!(GizmoAxis::Z.direction(), Vec3::Z);
        // Plane "direction" is the plane normal.
        assert_eq!(GizmoAxis::XY.direction(), Vec3::Z);
        assert_eq!(GizmoAxis::XZ.direction(), Vec3::Y);
        assert_eq!(GizmoAxis::YZ.direction(), Vec3::X);

        for axis in [GizmoAxis::X, GizmoAxis::Y, GizmoAxis::Z] {
            assert!(!axis.is_plane());
            assert!(axis.plane_axes().is_none());
        }
        for plane in [GizmoAxis::XY, GizmoAxis::XZ, GizmoAxis::YZ] {
            assert!(plane.is_plane());
        }
        assert_eq!(GizmoAxis::XY.plane_axes(), Some((Vec3::X, Vec3::Y)));
        assert_eq!(GizmoAxis::XZ.plane_axes(), Some((Vec3::X, Vec3::Z)));
        assert_eq!(GizmoAxis::YZ.plane_axes(), Some((Vec3::Y, Vec3::Z)));
    }

    #[test]
    fn gizmo_axis_signed_direction_flips_single_axes_only() {
        let signs = Vec3::new(-1.0, 1.0, -1.0);
        assert_eq!(GizmoAxis::X.signed_direction(signs), Vec3::new(-1.0, 0.0, 0.0));
        assert_eq!(GizmoAxis::Y.signed_direction(signs), Vec3::Y);
        assert_eq!(GizmoAxis::Z.signed_direction(signs), Vec3::new(0.0, 0.0, -1.0));
        // Plane normals are unaffected by signs.
        assert_eq!(GizmoAxis::XY.signed_direction(signs), Vec3::Z);
    }

    #[test]
    fn gizmo_axis_signed_plane_axes_bake_signs() {
        let signs = Vec3::new(-1.0, 1.0, -1.0);
        assert_eq!(
            GizmoAxis::XY.signed_plane_axes(signs),
            Some((Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)))
        );
        assert_eq!(
            GizmoAxis::YZ.signed_plane_axes(signs),
            Some((Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, -1.0)))
        );
        assert_eq!(GizmoAxis::X.signed_plane_axes(signs), None);
    }

    // ── screen-delta helpers ────────────────────────────────────────────────

    #[test]
    fn screen_delta_to_angle_front_facing_uses_combined_delta() {
        // Identity camera looks down -Z; the Z axis faces the camera
        // (|dot| = 1 > 0.7) → angle = (dx - dy) * 0.005.
        let cam = GlobalTransform::IDENTITY;
        let a = screen_delta_to_angle(Vec2::new(10.0, 4.0), Vec3::Z, &cam);
        assert!((a - 0.03).abs() < 1e-6, "got {a}");
    }

    #[test]
    fn screen_delta_to_angle_edge_on_projects_perpendicular() {
        // X axis is edge-on to the identity camera: screen axis is (1,0),
        // its perpendicular is (0,1) → only the vertical delta contributes.
        let cam = GlobalTransform::IDENTITY;
        let a = screen_delta_to_angle(Vec2::new(3.0, 8.0), Vec3::X, &cam);
        assert!((a - 0.04).abs() < 1e-6, "got {a}");
    }

    #[test]
    fn screen_delta_to_scale_projects_onto_axis() {
        let cam = GlobalTransform::IDENTITY;
        // X axis maps to screen (1, 0): only the horizontal delta counts.
        let s = screen_delta_to_scale(Vec2::new(10.0, 99.0), Vec3::X, &cam);
        assert!((s - 0.05).abs() < 1e-6, "got {s}");
        // Z axis has no screen projection on the identity camera → 0.
        let s = screen_delta_to_scale(Vec2::new(10.0, 10.0), Vec3::Z, &cam);
        assert_eq!(s, 0.0);
    }

    // ── world-space AABB helpers ────────────────────────────────────────────

    #[test]
    fn world_space_min_y_handles_rotation() {
        // Half-extents (2,1,1): rotating 90° about Z swings the ±2 X extent
        // onto the Y axis, so the lowest corner sits at y = -2.
        let aabb = Aabb::from_min_max(Vec3::new(-2.0, -1.0, -1.0), Vec3::new(2.0, 1.0, 1.0));
        let gt = GlobalTransform::from(
            Transform::from_translation(Vec3::new(0.0, 5.0, 0.0))
                .with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
        );
        let min_y = world_space_min_y(&aabb, &gt);
        assert!((min_y - 3.0).abs() < 1e-4, "got {min_y}");
    }

    #[test]
    fn world_aabb_min_applies_translation_and_scale() {
        let aabb = Aabb::from_min_max(Vec3::splat(-1.0), Vec3::splat(1.0));
        let min = world_aabb_min(
            &aabb,
            Vec3::new(10.0, 0.0, 0.0),
            Quat::IDENTITY,
            Vec3::new(2.0, 3.0, 1.0),
        );
        assert!((min - Vec3::new(8.0, -3.0, -1.0)).length() < 1e-4, "got {min}");
    }

    #[test]
    fn world_aabb_min_applies_rotation() {
        // 180° about X flips Y/Z, but a symmetric cube's min is unchanged.
        let aabb = Aabb::from_min_max(Vec3::ZERO, Vec3::ONE);
        let min = world_aabb_min(&aabb, Vec3::ZERO, Quat::from_rotation_x(PI), Vec3::ONE);
        // Local (0..1)³ rotated 180° about X → y/z in (-1..0).
        assert!((min - Vec3::new(0.0, -1.0, -1.0)).length() < 1e-4, "got {min}");
    }

    // ── BoxSelectionState ───────────────────────────────────────────────────

    #[test]
    fn box_selection_get_rect_normalizes_inverted_drag() {
        let state = BoxSelectionState {
            active: true,
            start_pos: Vec2::new(100.0, 20.0),
            current_pos: Vec2::new(40.0, 80.0),
            pending_pick: None,
        };
        let (min, max) = state.get_rect();
        assert_eq!(min, Vec2::new(40.0, 20.0));
        assert_eq!(max, Vec2::new(100.0, 80.0));
    }

    #[test]
    fn box_selection_is_drag_requires_movement_past_threshold() {
        let mut state = BoxSelectionState {
            start_pos: Vec2::new(10.0, 10.0),
            current_pos: Vec2::new(10.0, 10.0),
            ..Default::default()
        };
        assert!(!state.is_drag());
        // Exactly 5px is still a click (threshold is strict >).
        state.current_pos = Vec2::new(15.0, 10.0);
        assert!(!state.is_drag());
        state.current_pos = Vec2::new(15.1, 10.0);
        assert!(state.is_drag());
        // Either axis alone is enough.
        state.current_pos = Vec2::new(10.0, 16.0);
        assert!(state.is_drag());
    }

    // ── resolve_pick ────────────────────────────────────────────────────────

    fn run_resolve_pick(
        world: &mut World,
        start: Entity,
        g: SelectionGranularity,
    ) -> Option<Entity> {
        world
            .run_system_once(
                move |named: Query<(Entity, Has<SelectionStop>), With<Name>>,
                      parents: Query<&ChildOf>,
                      hidden: Query<(), With<HideInHierarchy>>| {
                    resolve_pick(start, g, &named, &parents, &hidden)
                },
            )
            .unwrap()
    }

    #[test]
    fn resolve_pick_no_stop_returns_leaf_or_topmost() {
        let mut world = World::new();
        let root = world.spawn(Name::new("Root")).id();
        let mesh = world.spawn((Name::new("Mesh"), ChildOf(root))).id();
        use SelectionGranularity::*;
        // Without a SelectionStop boundary, Mesh = the clicked mesh; MeshRoot
        // and EntireRoot both bubble to the topmost named ancestor.
        assert_eq!(run_resolve_pick(&mut world, mesh, Mesh), Some(mesh));
        assert_eq!(run_resolve_pick(&mut world, mesh, MeshRoot), Some(root));
        assert_eq!(run_resolve_pick(&mut world, mesh, EntireRoot), Some(root));

        // An unnamed child resolves to its nearest named ancestor.
        let unnamed = world.spawn(ChildOf(mesh)).id();
        assert_eq!(run_resolve_pick(&mut world, unnamed, Mesh), Some(mesh));
    }

    #[test]
    fn resolve_pick_distinguishes_granularity_at_stop_boundary() {
        // model (SelectionStop) → group → mesh
        let mut world = World::new();
        let model = world.spawn((Name::new("Model"), SelectionStop)).id();
        let group = world.spawn((Name::new("Building"), ChildOf(model))).id();
        let mesh = world.spawn((Name::new("Wall"), ChildOf(group))).id();
        use SelectionGranularity::*;
        // Mesh = clicked leaf; MeshRoot = topmost named below the stop (the
        // sub-object); EntireRoot = the whole model at the stop.
        assert_eq!(run_resolve_pick(&mut world, mesh, Mesh), Some(mesh));
        assert_eq!(run_resolve_pick(&mut world, mesh, MeshRoot), Some(group));
        assert_eq!(run_resolve_pick(&mut world, mesh, EntireRoot), Some(model));
    }

    #[test]
    fn resolve_pick_flat_model_meshroot_is_mesh() {
        // model (SelectionStop) → mesh directly. MeshRoot collapses to the mesh
        // since there is no intermediate group below the stop.
        let mut world = World::new();
        let model = world.spawn((Name::new("Car"), SelectionStop)).id();
        let mesh = world.spawn((Name::new("Wheel"), ChildOf(model))).id();
        use SelectionGranularity::*;
        assert_eq!(run_resolve_pick(&mut world, mesh, Mesh), Some(mesh));
        assert_eq!(run_resolve_pick(&mut world, mesh, MeshRoot), Some(mesh));
        assert_eq!(run_resolve_pick(&mut world, mesh, EntireRoot), Some(model));
    }

    #[test]
    fn resolve_pick_skips_hidden_wrapper_between_root_and_mesh() {
        // model (SelectionStop) → RootNode (named + HideInHierarchy) → mesh.
        // The hidden wrapper must be transparent: MeshRoot resolves to the mesh
        // (the topmost VISIBLE named below the stop), not the hidden wrapper —
        // otherwise the caller rejects the hidden target and nothing selects.
        let mut world = World::new();
        let model = world.spawn((Name::new("Model"), SelectionStop)).id();
        let wrapper = world
            .spawn((Name::new("RootNode"), HideInHierarchy, ChildOf(model)))
            .id();
        let mesh = world.spawn((Name::new("Wall"), ChildOf(wrapper))).id();
        use SelectionGranularity::*;
        assert_eq!(run_resolve_pick(&mut world, mesh, Mesh), Some(mesh));
        assert_eq!(run_resolve_pick(&mut world, mesh, MeshRoot), Some(mesh));
        assert_eq!(run_resolve_pick(&mut world, mesh, EntireRoot), Some(model));
    }

    #[test]
    fn resolve_pick_unnamed_chain_returns_none() {
        let mut world = World::new();
        let root = world.spawn_empty().id();
        let child = world.spawn(ChildOf(root)).id();
        assert_eq!(
            run_resolve_pick(&mut world, child, SelectionGranularity::MeshRoot),
            None
        );
    }

    // ── compute_gizmo_pivot ─────────────────────────────────────────────────

    fn run_compute_pivot(world: &mut World, entity: Entity, fallback: GlobalTransform) -> Vec3 {
        world
            .run_system_once(
                move |aabbs: Query<(Option<&Aabb>, &GlobalTransform), With<Mesh3d>>,
                      children: Query<&Children>| {
                    compute_gizmo_pivot(entity, &aabbs, &children, &fallback)
                },
            )
            .unwrap()
    }

    #[test]
    fn compute_gizmo_pivot_uses_world_aabb_center() {
        let mut meshes = Assets::<Mesh>::default();
        let mesh = meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)));

        let mut world = World::new();
        let entity = world
            .spawn((
                Mesh3d(mesh),
                Aabb::from_min_max(Vec3::splat(-1.0), Vec3::splat(1.0)),
                GlobalTransform::from_translation(Vec3::new(10.0, 2.0, 0.0)),
            ))
            .id();
        // Pivot anchors on the mesh AABB, not the (bogus) fallback transform.
        let fallback = GlobalTransform::from_translation(Vec3::splat(99.0));
        let pivot = run_compute_pivot(&mut world, entity, fallback);
        assert!((pivot - Vec3::new(10.0, 2.0, 0.0)).length() < 1e-4, "got {pivot}");
    }

    #[test]
    fn compute_gizmo_pivot_includes_descendant_meshes() {
        let mut meshes = Assets::<Mesh>::default();
        let mesh = meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)));

        let mut world = World::new();
        // Parent has no mesh of its own — pivot must come from the child,
        // matching the scene-GLB case where the root sits at the origin.
        let parent = world.spawn(Name::new("Root")).id();
        world.spawn((
            Mesh3d(mesh),
            Aabb::from_min_max(Vec3::splat(-1.0), Vec3::splat(1.0)),
            GlobalTransform::from_translation(Vec3::new(0.0, 0.0, -6.0)),
            ChildOf(parent),
        ));
        let fallback = GlobalTransform::IDENTITY;
        let pivot = run_compute_pivot(&mut world, parent, fallback);
        assert!((pivot - Vec3::new(0.0, 0.0, -6.0)).length() < 1e-4, "got {pivot}");
    }

    #[test]
    fn compute_gizmo_pivot_falls_back_without_aabbs() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("JustSpawned")).id();
        let fallback = GlobalTransform::from_translation(Vec3::new(1.0, 2.0, 3.0));
        let pivot = run_compute_pivot(&mut world, entity, fallback);
        assert_eq!(pivot, Vec3::new(1.0, 2.0, 3.0));
    }
}
