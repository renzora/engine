//! Renzora Gizmo — 3D transform gizmos for the editor viewport.
//!
//! Spawns real mesh entities (cylinders, cones, cubes) with an always-on-top
//! material. Supports translate (arrows + plane squares), rotate (circles),
//! and scale (lines + cube caps) modes.

mod camera_gizmo;
pub mod modal_transform;
pub mod selection_visuals;
pub mod skeleton_gizmo;

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::input::mouse::MouseMotion;
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey};
use bevy::render::render_resource::{
    AsBindGroup, CompareFunction, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::window::PrimaryWindow;
use bevy::picking::mesh_picking::ray_cast::MeshRayCast;

use renzora::core::InputFocusState;
use renzora::core::keybindings::{EditorAction, KeyBindings};
use renzora::core::viewport_types::{NavOverlayState, ViewportState};
use renzora::editor::{EditorSelection, EditorLocked, EditorCamera, HideInHierarchy};

// ── Constants ───────────────────────────────────────────────────────────────

const GIZMO_SIZE: f32 = 2.0;
const GIZMO_SCALE_REF_DIST: f32 = 10.0;
const GIZMO_PLANE_SIZE: f32 = 0.5;
const GIZMO_PLANE_OFFSET: f32 = 0.6;

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
            depth_stencil.depth_compare = CompareFunction::Always;
            depth_stencil.depth_write_enabled = false;
        }
        Ok(())
    }
}

// ── Enums ───────────────────────────────────────────────────────────────────

pub use renzora::editor::GizmoMode;

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
}

const AXES: [GizmoAxis; 3] = [GizmoAxis::X, GizmoAxis::Y, GizmoAxis::Z];
const PLANES: [GizmoAxis; 3] = [GizmoAxis::XY, GizmoAxis::XZ, GizmoAxis::YZ];

// ── Components & Resources ──────────────────────────────────────────────────

#[derive(Component)]
struct GizmoRoot;

#[derive(Component)]
struct GizmoMesh;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum GizmoPart {
    XShaft, XHead,
    YShaft, YHead,
    ZShaft, ZHead,
    XScaleCube, YScaleCube, ZScaleCube,
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

#[derive(Resource, Default)]
pub struct GizmoState {
    pub active_axis: Option<GizmoAxis>,
    pub hovered_axis: Option<GizmoAxis>,
    pub drag_starts: Vec<(Entity, Vec3, Quat, Vec3)>,
    pub drag_angle: f32,
    pub drag_scale_factor: f32,
    pub gizmo_scale: f32,
}

/// State for box/marquee selection (drag to select multiple entities).
#[derive(Resource, Default, Clone, Copy)]
pub struct BoxSelectionState {
    /// Whether box selection is currently active.
    pub active: bool,
    /// Start position in screen coordinates.
    pub start_pos: Vec2,
    /// Current position in screen coordinates.
    pub current_pos: Vec2,
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
        app.add_plugins(bevy_mod_outline::OutlinePlugin)
            .add_plugins(MaterialPlugin::<GizmoMaterial>::default())
            .insert_gizmo_config(
                OverlayGizmoGroup,
                GizmoConfig {
                    depth_bias: -1.0,
                    line: GizmoLineConfig { width: 3.0, ..default() },
                    render_layers: RenderLayers::layer(1),
                    ..default()
                },
            )
            .init_resource::<GizmoMode>()
            .init_resource::<GizmoState>()
            .init_resource::<BoxSelectionState>()
            .init_resource::<skeleton_gizmo::BoneSelection>()
            .init_resource::<modal_transform::ModalTransformState>()
            .init_resource::<renzora::core::ModalTransformHud>()
            .add_systems(PostStartup, setup_gizmo_meshes)
            .add_systems(
                Update,
                (
                    handle_selection_shortcuts,
                    handle_file_shortcuts,
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
                    .run_if(in_state(renzora::editor::SplashState::Editor))
                    .run_if(renzora::core::not_in_play_mode),
            )
            .add_systems(
                Update,
                render_box_selection
                    .after(box_selection_system)
                    .run_if(in_state(renzora::editor::SplashState::Editor)),
            )
            .add_systems(
                Update,
                selection_visuals::terrain_chunk_selection_system
                    .run_if(in_state(renzora::editor::SplashState::Editor)),
            );
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
    let cone_mesh = meshes.add(Cone { radius: 0.15, height: 0.4 });
    let cube_mesh = meshes.add(Cuboid::new(0.25, 0.25, 0.25));

    let gizmo_root = commands.spawn((
        Transform::default(),
        Visibility::Hidden,
        GizmoRoot,
        HideInHierarchy,
        RenderLayers::layer(1),
    )).id();

    let spawn = |commands: &mut Commands, mesh: Handle<Mesh>, mat: Handle<GizmoMaterial>, transform: Transform, part: GizmoPart, root: Entity| {
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
    spawn(&mut commands, shaft_mesh.clone(), gizmo_mats.x_normal.clone(),
        Transform::from_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2))
            .with_translation(Vec3::new(half_shaft, 0.0, 0.0)),
        GizmoPart::XShaft, gizmo_root);
    spawn(&mut commands, cone_mesh.clone(), gizmo_mats.x_normal.clone(),
        Transform::from_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2))
            .with_translation(Vec3::new(GIZMO_SIZE - 0.2, 0.0, 0.0)),
        GizmoPart::XHead, gizmo_root);

    // Y axis (cylinder default is along Y)
    spawn(&mut commands, shaft_mesh.clone(), gizmo_mats.y_normal.clone(),
        Transform::from_translation(Vec3::new(0.0, half_shaft, 0.0)),
        GizmoPart::YShaft, gizmo_root);
    spawn(&mut commands, cone_mesh.clone(), gizmo_mats.y_normal.clone(),
        Transform::from_translation(Vec3::new(0.0, GIZMO_SIZE - 0.2, 0.0)),
        GizmoPart::YHead, gizmo_root);

    // Z axis (rotate cylinder to point along Z)
    spawn(&mut commands, shaft_mesh.clone(), gizmo_mats.z_normal.clone(),
        Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
            .with_translation(Vec3::new(0.0, 0.0, half_shaft)),
        GizmoPart::ZShaft, gizmo_root);
    spawn(&mut commands, cone_mesh.clone(), gizmo_mats.z_normal.clone(),
        Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
            .with_translation(Vec3::new(0.0, 0.0, GIZMO_SIZE - 0.2)),
        GizmoPart::ZHead, gizmo_root);

    // Scale cubes at axis tips (hidden by default, shown in Scale mode)
    let scale_cube_mesh = meshes.add(Cuboid::new(0.15, 0.15, 0.15));
    spawn(&mut commands, scale_cube_mesh.clone(), gizmo_mats.x_normal.clone(),
        Transform::from_translation(Vec3::new(GIZMO_SIZE, 0.0, 0.0)),
        GizmoPart::XScaleCube, gizmo_root);
    spawn(&mut commands, scale_cube_mesh.clone(), gizmo_mats.y_normal.clone(),
        Transform::from_translation(Vec3::new(0.0, GIZMO_SIZE, 0.0)),
        GizmoPart::YScaleCube, gizmo_root);
    spawn(&mut commands, scale_cube_mesh.clone(), gizmo_mats.z_normal.clone(),
        Transform::from_translation(Vec3::new(0.0, 0.0, GIZMO_SIZE)),
        GizmoPart::ZScaleCube, gizmo_root);

    // Center cube
    spawn(&mut commands, cube_mesh, gizmo_mats.center_normal.clone(),
        Transform::default(),
        GizmoPart::Center, gizmo_root);

    commands.insert_resource(gizmo_mats);
}

// ── Transform update (follow selection, scale by distance) ──────────────────

fn update_gizmo_transforms(
    selection: Res<EditorSelection>,
    mode: Res<GizmoMode>,
    modal: Res<modal_transform::ModalTransformState>,
    mut gizmo_state: ResMut<GizmoState>,
    transforms: Query<&Transform, (Without<GizmoMesh>, Without<GizmoRoot>)>,
    mut gizmo_root: Query<(&mut Transform, &mut Visibility), With<GizmoRoot>>,
    mut gizmo_parts: Query<(&GizmoPart, &mut Visibility), (With<GizmoMesh>, Without<GizmoRoot>)>,
    camera_query: Query<&GlobalTransform, With<EditorCamera>>,
) {
    let Ok((mut root_transform, mut root_vis)) = gizmo_root.single_mut() else { return };

    let selected = selection.get();
    // Hide mesh gizmos during modal transform and when in Scale mode (drawn via immediate gizmos)
    let show_meshes = selected.is_some()
        && !modal.active
        && matches!(*mode, GizmoMode::Translate);
    *root_vis = if show_meshes { Visibility::Visible } else { Visibility::Hidden };

    // Toggle cone heads vs scale cubes based on mode
    for (part, mut vis) in gizmo_parts.iter_mut() {
        if part.is_translate_only() {
            *vis = if *mode == GizmoMode::Translate { Visibility::Inherited } else { Visibility::Hidden };
        } else if part.is_scale_only() {
            *vis = if *mode == GizmoMode::Scale { Visibility::Inherited } else { Visibility::Hidden };
        }
    }

    if let Some(selected) = selected {
        if let Ok(sel_t) = transforms.get(selected) {
            root_transform.translation = sel_t.translation;

            if let Ok(cam_gt) = camera_query.single() {
                let dist = (cam_gt.translation() - sel_t.translation).length().max(0.1);
                let scale = dist / GIZMO_SCALE_REF_DIST;
                root_transform.scale = Vec3::splat(scale);
                gizmo_state.gizmo_scale = scale;
            }
        }
    }
}

// ── Material update (hover/active highlighting) ─────────────────────────────

fn update_gizmo_materials(
    gizmo_state: Res<GizmoState>,
    gizmo_mats: Option<Res<GizmoMaterials>>,
    mut query: Query<(&GizmoPart, &mut MeshMaterial3d<GizmoMaterial>), With<GizmoMesh>>,
) {
    let Some(mats) = gizmo_mats else { return };

    let active = gizmo_state.active_axis.or(gizmo_state.hovered_axis);

    for (part, mut mat_handle) in query.iter_mut() {
        let (normal, highlight, highlighted) = match part {
            GizmoPart::XShaft | GizmoPart::XHead | GizmoPart::XScaleCube => (
                mats.x_normal.clone(), mats.x_highlight.clone(),
                matches!(active, Some(GizmoAxis::X) | Some(GizmoAxis::XY) | Some(GizmoAxis::XZ)),
            ),
            GizmoPart::YShaft | GizmoPart::YHead | GizmoPart::YScaleCube => (
                mats.y_normal.clone(), mats.y_highlight.clone(),
                matches!(active, Some(GizmoAxis::Y) | Some(GizmoAxis::XY) | Some(GizmoAxis::YZ)),
            ),
            GizmoPart::ZShaft | GizmoPart::ZHead | GizmoPart::ZScaleCube => (
                mats.z_normal.clone(), mats.z_highlight.clone(),
                matches!(active, Some(GizmoAxis::Z) | Some(GizmoAxis::XZ) | Some(GizmoAxis::YZ)),
            ),
            GizmoPart::Center => (
                mats.center_normal.clone(), mats.center_highlight.clone(),
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

fn draw_line_gizmos(
    mut gizmos: Gizmos<OverlayGizmoGroup>,
    mode: Res<GizmoMode>,
    gizmo_state: Res<GizmoState>,
    selection: Res<EditorSelection>,
    transform_q: Query<&Transform, (Without<EditorCamera>, Without<GizmoRoot>, Without<GizmoMesh>)>,
) {
    let Some(selected) = selection.get() else { return };
    let Ok(sel_t) = transform_q.get(selected) else { return };
    let pos = sel_t.translation;
    let gs = gizmo_state.gizmo_scale;

    if *mode == GizmoMode::Select { return; }

    let active = gizmo_state.active_axis.or(gizmo_state.hovered_axis);
    let highlight = Color::srgb(1.0, 1.0, 0.3);
    let x_base = Color::srgb(1.0, 0.15, 0.15);
    let y_base = Color::srgb(0.15, 1.0, 0.15);
    let z_base = Color::srgb(0.2, 0.3, 1.0);

    match *mode {
        GizmoMode::Select => unreachable!(),
        GizmoMode::Translate => {
            // Plane squares
            let plane_half = GIZMO_PLANE_SIZE * gs * 0.5;
            let po = GIZMO_PLANE_OFFSET * gs;

            let xy_color = if active == Some(GizmoAxis::XY) { highlight } else { Color::srgb(0.9, 0.9, 0.2) };
            let xz_color = if active == Some(GizmoAxis::XZ) { highlight } else { Color::srgb(0.9, 0.2, 0.9) };
            let yz_color = if active == Some(GizmoAxis::YZ) { highlight } else { Color::srgb(0.2, 0.9, 0.9) };

            // XY
            let c = pos + Vec3::new(po, po, 0.0);
            gizmos.line(c + Vec3::new(-plane_half, -plane_half, 0.0), c + Vec3::new(plane_half, -plane_half, 0.0), xy_color);
            gizmos.line(c + Vec3::new(plane_half, -plane_half, 0.0), c + Vec3::new(plane_half, plane_half, 0.0), xy_color);
            gizmos.line(c + Vec3::new(plane_half, plane_half, 0.0), c + Vec3::new(-plane_half, plane_half, 0.0), xy_color);
            gizmos.line(c + Vec3::new(-plane_half, plane_half, 0.0), c + Vec3::new(-plane_half, -plane_half, 0.0), xy_color);

            // XZ
            let c = pos + Vec3::new(po, 0.0, po);
            gizmos.line(c + Vec3::new(-plane_half, 0.0, -plane_half), c + Vec3::new(plane_half, 0.0, -plane_half), xz_color);
            gizmos.line(c + Vec3::new(plane_half, 0.0, -plane_half), c + Vec3::new(plane_half, 0.0, plane_half), xz_color);
            gizmos.line(c + Vec3::new(plane_half, 0.0, plane_half), c + Vec3::new(-plane_half, 0.0, plane_half), xz_color);
            gizmos.line(c + Vec3::new(-plane_half, 0.0, plane_half), c + Vec3::new(-plane_half, 0.0, -plane_half), xz_color);

            // YZ
            let c = pos + Vec3::new(0.0, po, po);
            gizmos.line(c + Vec3::new(0.0, -plane_half, -plane_half), c + Vec3::new(0.0, plane_half, -plane_half), yz_color);
            gizmos.line(c + Vec3::new(0.0, plane_half, -plane_half), c + Vec3::new(0.0, plane_half, plane_half), yz_color);
            gizmos.line(c + Vec3::new(0.0, plane_half, plane_half), c + Vec3::new(0.0, -plane_half, plane_half), yz_color);
            gizmos.line(c + Vec3::new(0.0, -plane_half, plane_half), c + Vec3::new(0.0, -plane_half, -plane_half), yz_color);
        }
        GizmoMode::Rotate => {
            let radius = GIZMO_SIZE * gs * 0.7;
            let x_color = if matches!(active, Some(GizmoAxis::X)) { highlight } else { x_base };
            let y_color = if matches!(active, Some(GizmoAxis::Y)) { highlight } else { y_base };
            let z_color = if matches!(active, Some(GizmoAxis::Z)) { highlight } else { z_base };

            gizmos.circle(Isometry3d::new(pos, Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)), radius, x_color);
            gizmos.circle(Isometry3d::new(pos, Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)), radius, y_color);
            gizmos.circle(Isometry3d::new(pos, Quat::IDENTITY), radius, z_color);
        }
        GizmoMode::Scale => {
            let scale_size = GIZMO_SIZE * gs;
            let x_color = if matches!(active, Some(GizmoAxis::X)) { highlight } else { x_base };
            let y_color = if matches!(active, Some(GizmoAxis::Y)) { highlight } else { y_base };
            let z_color = if matches!(active, Some(GizmoAxis::Z)) { highlight } else { z_base };

            // Lines from center to cube tips
            gizmos.line(pos, pos + Vec3::X * scale_size, x_color);
            gizmos.line(pos, pos + Vec3::Y * scale_size, y_color);
            gizmos.line(pos, pos + Vec3::Z * scale_size, z_color);

            // Cube wireframes at tips
            let cube_half = 0.075 * gs;
            for (axis_dir, color) in [(Vec3::X, x_color), (Vec3::Y, y_color), (Vec3::Z, z_color)] {
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
                    gizmos.line(c + a * h, c + b * h, color);
                }
            }
        }
    }
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
    names: Query<&Name>,
    transforms: Query<&Transform>,
    parents: Query<&ChildOf>,
) {
    if keybindings.rebinding.is_some() { return; }
    if input_focus.egui_wants_keyboard { return; }
    if input_focus.egui_has_pointer && !viewport_state.hovered { return; }
    if mouse_button.pressed(MouseButton::Right) { return; }
    if gizmo_state.active_axis.is_some() { return; }
    if modal.active { return; }

    if keybindings.just_pressed(EditorAction::Delete, &keyboard) {
        let entities = selection.get_all();
        if !entities.is_empty() {
            selection.clear();
            for entity in entities {
                commands.entity(entity).despawn();
            }
        }
    }

    if keybindings.just_pressed(EditorAction::Deselect, &keyboard) {
        selection.clear();
    }

    if keybindings.just_pressed(EditorAction::CreateNode, &keyboard) {
        commands.insert_resource(renzora::core::CreateNodeRequested);
    }

    // Duplicate (Ctrl+D)
    if keybindings.just_pressed(EditorAction::Duplicate, &keyboard) {
        let entities = selection.get_all();
        for entity in entities {
            let name = names
                .get(entity)
                .map(|n| format!("{} (Copy)", n.as_str()))
                .unwrap_or_else(|_| "Entity (Copy)".to_string());
            let transform = transforms.get(entity).copied().unwrap_or_default();
            let parent = parents.get(entity).ok().map(|c| c.parent());
            let new_entity = commands.spawn((Name::new(name), transform)).id();
            if let Some(p) = parent {
                commands.entity(new_entity).set_parent_in_place(p);
            }
            selection.set(Some(new_entity));
        }
    }

    // Duplicate & Move (Alt+D) — duplicate then enter grab mode
    if keybindings.just_pressed(EditorAction::DuplicateAndMove, &keyboard) {
        let entities = selection.get_all();
        let has_entities = !entities.is_empty();
        for entity in entities {
            let name = names
                .get(entity)
                .map(|n| format!("{} (Copy)", n.as_str()))
                .unwrap_or_else(|_| "Entity (Copy)".to_string());
            let transform = transforms.get(entity).copied().unwrap_or_default();
            let parent = parents.get(entity).ok().map(|c| c.parent());
            let new_entity = commands.spawn((Name::new(name), transform)).id();
            if let Some(p) = parent {
                commands.entity(new_entity).set_parent_in_place(p);
            }
            selection.set(Some(new_entity));
        }
        if has_entities {
            commands.insert_resource(PendingModalGrab);
        }
    }
}

/// One-shot resource to signal pending modal grab from duplicate-and-move.
#[derive(Resource)]
struct PendingModalGrab;

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
    if keybindings.rebinding.is_some() { return; }
    if input_focus.egui_wants_keyboard { return; }
    if mouse_button.pressed(MouseButton::Right) { return; }
    if modal.active { return; }

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
    mut active_tool: ResMut<renzora::editor::ActiveTool>,
) {
    if keybindings.rebinding.is_some() { return; }
    if input_focus.egui_wants_keyboard { return; }
    if mouse_button.pressed(MouseButton::Right) { return; }
    if modal.active { return; }
    if keybindings.just_pressed(EditorAction::ToolSelect, &keyboard) {
        *mode = GizmoMode::Select;
        *active_tool = renzora::editor::ActiveTool::Select;
    }
    if keybindings.just_pressed(EditorAction::GizmoTranslate, &keyboard) {
        *mode = GizmoMode::Translate;
        *active_tool = renzora::editor::ActiveTool::Translate;
    }
    if keybindings.just_pressed(EditorAction::GizmoRotate, &keyboard) {
        *mode = GizmoMode::Rotate;
        *active_tool = renzora::editor::ActiveTool::Rotate;
    }
    if keybindings.just_pressed(EditorAction::GizmoScale, &keyboard) {
        *mode = GizmoMode::Scale;
        *active_tool = renzora::editor::ActiveTool::Scale;
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
    if vp_local.x < 0.0 || vp_local.y < 0.0
        || vp_local.x > viewport.screen_size.x || vp_local.y > viewport.screen_size.y
    { return None; }

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
            let world_dir = camera_transform.affine().matrix3.mul_vec3(local_dir).normalize();
            Some(Ray3d { origin: near.into(), direction: Dir3::new(world_dir).ok()? })
        }
        Projection::Orthographic(ortho) => {
            let hw = ortho.area.width() * 0.5;
            let hh = ortho.area.height() * 0.5;
            let offset = camera_transform.affine().matrix3.mul_vec3(Vec3::new(ndc.x * hw, ndc.y * hh, 0.0));
            Some(Ray3d { origin: (near + offset).into(), direction: camera_transform.forward() })
        }
        _ => None,
    }
}

fn closest_distance_ray_segment(ray: &Ray3d, seg_a: Vec3, seg_b: Vec3) -> Option<f32> {
    let ro: Vec3 = ray.origin.into();
    let rd: Vec3 = ray.direction.as_vec3();
    let sd = seg_b - seg_a;
    let sl = sd.length();
    if sl < 1e-6 { return None; }
    let su = sd / sl;
    let w0 = ro - seg_a;
    let a = rd.dot(rd);
    let b = rd.dot(su);
    let c = su.dot(su);
    let d = rd.dot(w0);
    let e = su.dot(w0);
    let denom = a * c - b * b;
    if denom.abs() < 1e-8 { return None; }
    let t_ray = (b * e - c * d) / denom;
    let t_seg = (a * e - b * d) / denom;
    if t_ray < 0.0 { return None; }
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
            if best.map_or(true, |b| d < b) { best = Some(d); }
        }
    }
    best
}

fn ray_hits_plane_quad(ray: &Ray3d, corner: Vec3, axis_a: Vec3, axis_b: Vec3, size: f32) -> bool {
    let normal = axis_a.cross(axis_b).normalize();
    let ro: Vec3 = ray.origin.into();
    let rd: Vec3 = ray.direction.as_vec3();
    let denom = normal.dot(rd);
    if denom.abs() < 1e-6 { return false; }
    let t = normal.dot(corner - ro) / denom;
    if t < 0.0 { return false; }
    let hit = ro + rd * t;
    let local = hit - corner;
    let u = local.dot(axis_a);
    let v = local.dot(axis_b);
    u >= 0.0 && u <= size && v >= 0.0 && v <= size
}

fn perpendicular_pair(normal: Vec3) -> (Vec3, Vec3) {
    let p1 = if normal.y.abs() > 0.9 { Vec3::X } else { normal.cross(Vec3::Y).normalize() };
    let p2 = normal.cross(p1).normalize();
    (p1, p2)
}

fn pick_threshold(cam_gt: &GlobalTransform, entity_pos: Vec3, projection: &Projection, vh: f32) -> f32 {
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
    selection: Res<EditorSelection>,
    viewport: Option<Res<ViewportState>>,
    camera_q: Query<(&GlobalTransform, &Projection), With<EditorCamera>>,
    transform_q: Query<&GlobalTransform, Without<EditorCamera>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    modal: Res<modal_transform::ModalTransformState>,
) {
    if modal.active { gizmo_state.hovered_axis = None; return; }
    if *mode == GizmoMode::Select { gizmo_state.hovered_axis = None; return; }
    if gizmo_state.active_axis.is_some() { return; }
    gizmo_state.hovered_axis = None;

    let Some(selected) = selection.get() else { return };
    let Some(viewport) = viewport.as_ref() else { return };
    if !viewport.hovered { return; }
    if mouse_button.pressed(MouseButton::Right) || mouse_button.pressed(MouseButton::Middle) { return; }

    let Ok((cam_gt, projection)) = camera_q.single() else { return };
    let Ok(entity_gt) = transform_q.get(selected) else { return };
    let Ok(window) = window_q.single() else { return };
    let Some(ray) = viewport_cursor_ray(window, viewport, cam_gt, projection) else { return };

    let entity_pos = entity_gt.translation();
    let gs = gizmo_state.gizmo_scale.max(0.01);
    let gizmo_size = GIZMO_SIZE * gs;
    let threshold = pick_threshold(cam_gt, entity_pos, projection, viewport.screen_size.y);

    let mut best: Option<(GizmoAxis, f32)> = None;

    match *mode {
        GizmoMode::Select => unreachable!(),
        GizmoMode::Translate => {
            // Plane squares first
            let plane_half = GIZMO_PLANE_SIZE * gs * 0.5;
            let po = GIZMO_PLANE_OFFSET * gs;
            for plane in PLANES {
                let (a, b) = plane.plane_axes().unwrap();
                let center = entity_pos + a * po + b * po;
                let corner = center - a * plane_half - b * plane_half;
                if ray_hits_plane_quad(&ray, corner, a, b, GIZMO_PLANE_SIZE * gs) {
                    best = Some((plane, 0.0));
                    break;
                }
            }
            if best.is_none() {
                for axis in AXES {
                    if let Some(dist) = closest_distance_ray_segment(&ray, entity_pos, entity_pos + axis.direction() * gizmo_size) {
                        if dist < threshold && best.map_or(true, |(_, d)| dist < d) {
                            best = Some((axis, dist));
                        }
                    }
                }
            }
        }
        GizmoMode::Scale => {
            for axis in AXES {
                if let Some(dist) = closest_distance_ray_segment(&ray, entity_pos, entity_pos + axis.direction() * gizmo_size) {
                    if dist < threshold && best.map_or(true, |(_, d)| dist < d) {
                        best = Some((axis, dist));
                    }
                }
            }
        }
        GizmoMode::Rotate => {
            let radius = gizmo_size * 0.7;
            for axis in AXES {
                if let Some(dist) = ray_circle_distance(&ray, entity_pos, axis.direction(), radius) {
                    if dist < threshold && best.map_or(true, |(_, d)| dist < d) {
                        best = Some((axis, dist));
                    }
                }
            }
        }
    }

    gizmo_state.hovered_axis = best.map(|(a, _)| a);
}

// ── Drag handling ───────────────────────────────────────────────────────────

fn gizmo_drag(
    mut gizmo_state: ResMut<GizmoState>,
    mode: Res<GizmoMode>,
    selection: Res<EditorSelection>,
    viewport: Option<Res<ViewportState>>,
    camera_q: Query<(&GlobalTransform, &Projection), With<EditorCamera>>,
    mut transform_q: Query<&mut Transform, (Without<EditorCamera>, Without<EditorLocked>, Without<GizmoRoot>, Without<GizmoMesh>)>,
    global_q: Query<&GlobalTransform, Without<EditorCamera>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<MouseMotion>,
) {
    if *mode == GizmoMode::Select {
        mouse_motion.clear();
        return;
    }

    let selected_entities = selection.get_all();
    if selected_entities.is_empty() {
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
            for &entity in &selected_entities {
                if let Ok(t) = transform_q.get(entity) {
                    starts.push((entity, t.translation, t.rotation, t.scale));
                }
            }
            gizmo_state.active_axis = Some(axis);
            gizmo_state.drag_starts = starts;
            gizmo_state.drag_angle = 0.0;
            gizmo_state.drag_scale_factor = 0.0;
            mouse_motion.clear();
            return;
        }
    }

    // End drag
    if mouse_button.just_released(MouseButton::Left) && gizmo_state.active_axis.is_some() {
        gizmo_state.active_axis = None;
        gizmo_state.drag_starts.clear();
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
        mouse_motion.clear();
        return;
    }

    let Ok((cam_gt, projection)) = camera_q.single() else { mouse_motion.clear(); return; };
    let Some(viewport) = viewport.as_ref() else { mouse_motion.clear(); return; };

    let mut total_delta = Vec2::ZERO;
    for ev in mouse_motion.read() { total_delta += ev.delta; }
    if total_delta.length_squared() < 1e-6 { return; }

    let center = if gizmo_state.drag_starts.is_empty() { Vec3::ZERO } else {
        let sum: Vec3 = gizmo_state.drag_starts.iter().map(|(_, t, _, _)| *t).sum();
        sum / gizmo_state.drag_starts.len() as f32
    };
    let distance = (cam_gt.translation() - center).length();

    match *mode {
        GizmoMode::Select => unreachable!(),
        GizmoMode::Translate => {
            let scale = match projection {
                Projection::Perspective(persp) => distance * (persp.fov * 0.5).tan() * 2.0 / viewport.screen_size.y,
                Projection::Orthographic(ortho) => ortho.area.height() / viewport.screen_size.y,
                _ => return,
            };

            let offset = if axis.is_plane() {
                let (a, b) = axis.plane_axes().unwrap();
                let cam_right = cam_gt.right().as_vec3();
                let cam_up = cam_gt.up().as_vec3();
                let sa = Vec2::new(a.dot(cam_right), -a.dot(cam_up));
                let sb = Vec2::new(b.dot(cam_right), -b.dot(cam_up));
                let la = sa.length(); let lb = sb.length();
                let da = if la > 1e-4 { total_delta.dot(sa / la) } else { 0.0 };
                let db = if lb > 1e-4 { total_delta.dot(sb / lb) } else { 0.0 };
                a * da * scale + b * db * scale
            } else {
                let cam_right = cam_gt.right().as_vec3();
                let cam_up = cam_gt.up().as_vec3();
                let sa = Vec2::new(axis.direction().dot(cam_right), -axis.direction().dot(cam_up));
                let len = sa.length();
                if len < 1e-4 { return; }
                axis.direction() * total_delta.dot(sa / len) * scale
            };

            for &entity in &selected_entities {
                if let Ok(mut t) = transform_q.get_mut(entity) { t.translation += offset; }
            }
        }
        GizmoMode::Rotate => {
            let delta_angle = screen_delta_to_angle(total_delta, axis.direction(), cam_gt);
            gizmo_state.drag_angle += delta_angle;
            let rotation = Quat::from_axis_angle(axis.direction(), delta_angle);
            for &entity in &selected_entities {
                if let Ok(mut t) = transform_q.get_mut(entity) {
                    if selected_entities.len() == 1 {
                        t.rotation = rotation * t.rotation;
                    } else {
                        t.translation = center + rotation * (t.translation - center);
                        t.rotation = rotation * t.rotation;
                    }
                }
            }
        }
        GizmoMode::Scale => {
            let delta_scale = screen_delta_to_scale(total_delta, axis.direction(), cam_gt);
            for &entity in &selected_entities {
                if let Ok(mut t) = transform_q.get_mut(entity) {
                    match axis {
                        GizmoAxis::X => t.scale.x = (t.scale.x + delta_scale).max(0.01),
                        GizmoAxis::Y => t.scale.y = (t.scale.y + delta_scale).max(0.01),
                        GizmoAxis::Z => t.scale.z = (t.scale.z + delta_scale).max(0.01),
                        _ => {}
                    }
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
        if len < 1e-4 { 0.0 } else { mouse_delta.dot(sp / len) * sens }
    }
}

fn screen_delta_to_scale(mouse_delta: Vec2, axis_world: Vec3, cam: &GlobalTransform) -> f32 {
    let cr = cam.right().as_vec3();
    let cu = cam.up().as_vec3();
    let sa = Vec2::new(axis_world.dot(cr), -axis_world.dot(cu));
    let len = sa.length();
    if len < 1e-4 { 0.0 } else { mouse_delta.dot(sa / len) * 0.005 }
}

// ── Entity picking (click to select) ────────────────────────────────────────

fn entity_pick_system(
    gizmo_state: Res<GizmoState>,
    mode: Res<GizmoMode>,
    modal: Res<modal_transform::ModalTransformState>,
    selection: Res<EditorSelection>,
    viewport: Option<Res<ViewportState>>,
    nav_overlay: Option<Res<NavOverlayState>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    mut mesh_ray_cast: MeshRayCast,
    named_entities: Query<Entity, With<Name>>,
    parent_query: Query<&ChildOf>,
    gizmo_meshes: Query<(), Or<(With<GizmoMesh>, With<GizmoRoot>)>>,
    hidden_entities: Query<(), With<HideInHierarchy>>,
    mut box_sel: ResMut<BoxSelectionState>,
) {
    if !mouse_button.just_pressed(MouseButton::Left) { return; }
    if modal.active { return; }
    if gizmo_state.active_axis.is_some() || gizmo_state.hovered_axis.is_some() { return; }
    // Don't pick while nav overlay pan/zoom buttons are being dragged
    if let Some(ref nav) = nav_overlay {
        if nav.pan_dragging.load(std::sync::atomic::Ordering::Relaxed)
            || nav.zoom_dragging.load(std::sync::atomic::Ordering::Relaxed)
        {
            return;
        }
    }

    let Some(viewport) = viewport.as_ref() else { return };
    if !viewport.hovered { return; }

    let Ok(window) = window_q.single() else { return };
    let Ok((camera, cam_gt)) = camera_q.single() else { return };

    let Some(cursor) = window.cursor_position() else { return };
    let vp_local = cursor - viewport.screen_position;
    if vp_local.x < 0.0 || vp_local.y < 0.0
        || vp_local.x > viewport.screen_size.x || vp_local.y > viewport.screen_size.y
    { return; }

    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

    let Ok(ray) = camera.viewport_to_world(cam_gt, vp_local) else { return };

    let hits = mesh_ray_cast.cast_ray(ray, &default());

    for (entity, _hit) in hits.iter() {
        if gizmo_meshes.get(*entity).is_ok() { continue; }
        if hidden_entities.get(*entity).is_ok() { continue; }

        let selectable = find_named_ancestor(*entity, &named_entities, &parent_query);
        if let Some(target) = selectable {
            if hidden_entities.get(target).is_ok() { continue; }
            info!("[pick] Selected {:?} (hit {:?})", target, entity);
            if ctrl {
                selection.toggle(target);
            } else if shift {
                // Add to selection (only if not already selected)
                if !selection.is_selected(target) {
                    selection.toggle(target);
                }
            } else {
                selection.set(Some(target));
            }
            return;
        }
    }

    // Clicked empty space — start box selection in Select mode, else just deselect
    if *mode == GizmoMode::Select {
        box_sel.active = true;
        box_sel.start_pos = cursor;
        box_sel.current_pos = cursor;
    } else if !shift && !ctrl {
        selection.set(None);
    }
}

// ── Box selection system ─────────────────────────────────────────────────────

fn box_selection_system(
    mut box_sel: ResMut<BoxSelectionState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    viewport: Option<Res<ViewportState>>,
    nav_overlay: Option<Res<NavOverlayState>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    selection: Res<EditorSelection>,
    named_entities: Query<(Entity, &GlobalTransform), With<Name>>,
    hidden_entities: Query<(), With<HideInHierarchy>>,
    gizmo_meshes: Query<(), Or<(With<GizmoMesh>, With<GizmoRoot>)>>,
) {
    if !box_sel.active { return; }
    // Cancel box selection if nav overlay is being used
    if let Some(ref nav) = nav_overlay {
        if nav.pan_dragging.load(std::sync::atomic::Ordering::Relaxed)
            || nav.zoom_dragging.load(std::sync::atomic::Ordering::Relaxed)
        {
            box_sel.active = false;
            return;
        }
    }

    let Ok(window) = window_q.single() else { return; };
    let Some(cursor) = window.cursor_position() else { return; };

    // Update current position while dragging
    if mouse_button.pressed(MouseButton::Left) {
        box_sel.current_pos = cursor;
        return;
    }

    // Mouse released — finalize box selection
    box_sel.active = false;

    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

    if !box_sel.is_drag() {
        // Was just a click on empty space, not a drag
        if !shift && !ctrl {
            selection.set(None);
        }
        return;
    }

    let Some(viewport) = viewport.as_ref() else { return; };
    let Ok((camera, cam_gt)) = camera_q.single() else { return; };

    let (box_min, box_max) = box_sel.get_rect();

    // Find all named entities whose screen projection falls within the box
    let mut entities_in_box = Vec::new();

    for (entity, global_transform) in named_entities.iter() {
        if hidden_entities.get(entity).is_ok() { continue; }
        if gizmo_meshes.get(entity).is_ok() { continue; }

        let world_pos = global_transform.translation();
        let Some(ndc) = camera.world_to_ndc(cam_gt, world_pos) else { continue; };

        // Must be in front of camera
        if ndc.z < 0.0 || ndc.z > 1.0 { continue; }

        // Convert NDC to screen coordinates
        let screen_x = viewport.screen_position.x + (ndc.x + 1.0) * 0.5 * viewport.screen_size.x;
        let screen_y = viewport.screen_position.y + (1.0 - ndc.y) * 0.5 * viewport.screen_size.y;

        if screen_x >= box_min.x && screen_x <= box_max.x
            && screen_y >= box_min.y && screen_y <= box_max.y
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

fn find_named_ancestor(
    entity: Entity,
    named: &Query<Entity, With<Name>>,
    parents: &Query<&ChildOf>,
) -> Option<Entity> {
    if named.get(entity).is_ok() {
        return Some(entity);
    }
    let mut current = entity;
    while let Ok(child_of) = parents.get(current) {
        let parent = child_of.parent();
        if named.get(parent).is_ok() {
            return Some(parent);
        }
        current = parent;
    }
    None
}

// ── Box selection overlay ────────────────────────────────────────────────────

fn render_box_selection(
    box_sel: Res<BoxSelectionState>,
    mut ctx: renzora::bevy_egui::EguiContexts,
) {
    if !box_sel.active || !box_sel.is_drag() { return; }

    let Some(ctx) = ctx.ctx_mut().ok() else { return; };
    let (min, max) = box_sel.get_rect();

    let rect = renzora::bevy_egui::egui::Rect::from_min_max(
        renzora::bevy_egui::egui::Pos2::new(min.x, min.y),
        renzora::bevy_egui::egui::Pos2::new(max.x, max.y),
    );

    renzora::bevy_egui::egui::Area::new(renzora::bevy_egui::egui::Id::new("box_selection"))
        .fixed_pos(rect.min)
        .order(renzora::bevy_egui::egui::Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            let painter = ui.painter();
            painter.rect_filled(
                rect,
                renzora::bevy_egui::egui::CornerRadius::ZERO,
                renzora::bevy_egui::egui::Color32::from_rgba_unmultiplied(66, 150, 250, 40),
            );
            painter.rect_stroke(
                rect,
                renzora::bevy_egui::egui::CornerRadius::ZERO,
                renzora::bevy_egui::egui::Stroke::new(1.0, renzora::bevy_egui::egui::Color32::from_rgb(66, 150, 250)),
                renzora::bevy_egui::egui::StrokeKind::Outside,
            );
        });
}

renzora::add!(GizmoPlugin);
