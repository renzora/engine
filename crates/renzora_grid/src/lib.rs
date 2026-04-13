//! Renzora Grid — 3D editor grid with colored axis indicators.
//!
//! The grid is a real 3D line-list mesh rendered by the main editor camera
//! on render layer 0, with a custom unlit `GridMaterial` so sun/point lights
//! don't affect it. Being real 3D geometry gives it pixel-perfect depth
//! testing against scene meshes for free. The material fades line alpha with
//! horizontal distance from the camera for a soft Blender-style falloff.

use bevy::prelude::*;
use bevy::camera::visibility::RenderLayers;
use bevy::mesh::{PrimitiveTopology, VertexAttributeValues, MeshVertexBufferLayoutRef};
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey, MaterialPlugin};
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use renzora::core::viewport_types::ViewportSettings;

// ── GridMaterial ────────────────────────────────────────────────────────────

/// Unlit line material with horizontal-distance alpha fade.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct GridMaterial {
    #[uniform(0)]
    pub base_color: LinearRgba,
    #[uniform(0)]
    pub fade_start: f32,
    #[uniform(0)]
    pub fade_end: f32,
    #[uniform(0)]
    pub _pad0: f32,
    #[uniform(0)]
    pub _pad1: f32,
}

impl Material for GridMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_grid/shaders/grid_material.wgsl".into()
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
        // Leave depth_compare at the pipeline default so the grid is
        // occluded by scene meshes normally. Disable depth writes so
        // overlapping grid lines blend cleanly regardless of draw order.
        if let Some(ref mut depth_stencil) = descriptor.depth_stencil {
            depth_stencil.depth_write_enabled = false;
        }
        Ok(())
    }
}

// ── Components / config ─────────────────────────────────────────────────────

#[derive(Component)]
pub struct EditorGrid;

#[derive(Component)]
pub struct AxisIndicator;

#[derive(Component)]
pub struct SubgridLines;

#[derive(Resource)]
pub struct GridConfig {
    pub size: i32,
    pub spacing: f32,
    pub major_every: i32,
    pub minor_color: Color,
    pub major_color: Color,
    pub visible: bool,
    pub show_axes: bool,
    pub show_subgrid: bool,
    /// Fade-to-transparent start distance (XZ).
    pub fade_start: f32,
    /// Fade-to-transparent end distance (XZ). Beyond this, grid is invisible.
    pub fade_end: f32,
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            size: 200,
            spacing: 1.0,
            major_every: 10,
            minor_color: Color::srgba(0.55, 0.58, 0.63, 0.55),
            major_color: Color::srgba(0.78, 0.82, 0.88, 0.85),
            visible: true,
            show_axes: true,
            show_subgrid: true,
            fade_start: 12.0,
            fade_end: 40.0,
        }
    }
}

#[derive(Default)]
pub struct GridPlugin;

impl Plugin for GridPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] GridPlugin");
        bevy::asset::embedded_asset!(app, "shaders/grid_material.wgsl");
        app.init_resource::<GridConfig>()
            .add_plugins(MaterialPlugin::<GridMaterial>::default())
            .add_systems(PostStartup, spawn_grid)
            .add_systems(
                Update,
                (
                    sync_grid_from_viewport,
                    toggle_grid_visibility,
                    toggle_axis_visibility,
                    toggle_subgrid_visibility,
                    update_fade_distance,
                )
                    .run_if(in_state(renzora::editor::SplashState::Editor)),
            );
    }
}

// ── Spawn ───────────────────────────────────────────────────────────────────

fn spawn_grid(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GridMaterial>>,
    config: Res<GridConfig>,
) {
    let major_mesh = build_grid_mesh(&config, true);
    let minor_mesh = build_grid_mesh(&config, false);

    commands.spawn((
        Mesh3d(meshes.add(major_mesh)),
        MeshMaterial3d(materials.add(grid_material(config.major_color, &config))),
        Transform::default(),
        EditorGrid,
        RenderLayers::layer(0),
    ));

    commands.spawn((
        Mesh3d(meshes.add(minor_mesh)),
        MeshMaterial3d(materials.add(grid_material(config.minor_color, &config))),
        Transform::default(),
        EditorGrid,
        SubgridLines,
        RenderLayers::layer(0),
    ));

    // Axes (R/G/B) share the same fade as major lines but with punchier tints.
    spawn_axis(&mut commands, &mut meshes, &mut materials, &config,
        Vec3::NEG_X * 500.0, Vec3::X * 500.0,
        Color::srgba(0.92, 0.32, 0.36, 1.0),
        Vec3::new(0.0, 0.005, 0.0));
    spawn_axis(&mut commands, &mut meshes, &mut materials, &config,
        Vec3::NEG_Z * 500.0, Vec3::Z * 500.0,
        Color::srgba(0.30, 0.58, 0.95, 1.0),
        Vec3::new(0.0, 0.005, 0.0));
    spawn_axis(&mut commands, &mut meshes, &mut materials, &config,
        Vec3::NEG_Y * 500.0, Vec3::Y * 500.0,
        Color::srgba(0.40, 0.83, 0.44, 1.0),
        Vec3::ZERO);
}

fn spawn_axis(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<GridMaterial>,
    config: &GridConfig,
    from: Vec3,
    to: Vec3,
    color: Color,
    offset: Vec3,
) {
    let mesh = build_axis_line(from, to);
    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(grid_material(color, config))),
        Transform::from_translation(offset),
        AxisIndicator,
        RenderLayers::layer(0),
    ));
}

fn grid_material(color: Color, config: &GridConfig) -> GridMaterial {
    GridMaterial {
        base_color: color.to_linear(),
        fade_start: config.fade_start,
        fade_end: config.fade_end,
        _pad0: 0.0,
        _pad1: 0.0,
    }
}

fn build_grid_mesh(config: &GridConfig, majors_only: bool) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let size = config.size;
    let spacing = config.spacing;
    let extent = size as f32 * spacing;

    for i in -size..=size {
        if i == 0 {
            continue;
        }
        let is_major = i % config.major_every == 0;
        if is_major != majors_only {
            continue;
        }
        let pos = i as f32 * spacing;
        positions.push([pos, 0.0, -extent]);
        positions.push([pos, 0.0, extent]);
        positions.push([-extent, 0.0, pos]);
        positions.push([extent, 0.0, pos]);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::LineList, default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, VertexAttributeValues::Float32x3(positions));
    mesh
}

fn build_axis_line(from: Vec3, to: Vec3) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::LineList, default());
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        VertexAttributeValues::Float32x3(vec![from.to_array(), to.to_array()]),
    );
    mesh
}

// ── Systems ─────────────────────────────────────────────────────────────────

fn sync_grid_from_viewport(
    vp: Res<ViewportSettings>,
    mut config: ResMut<GridConfig>,
) {
    if !vp.is_changed() {
        return;
    }
    config.visible = vp.show_grid;
    config.show_axes = vp.show_axis_gizmo;
    config.show_subgrid = vp.show_subgrid;
}

fn toggle_grid_visibility(
    config: Res<GridConfig>,
    mut grids: Query<&mut Visibility, (With<EditorGrid>, Without<SubgridLines>)>,
) {
    if !config.is_changed() {
        return;
    }
    let vis = if config.visible { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut grids { *v = vis; }
}

fn toggle_axis_visibility(
    config: Res<GridConfig>,
    mut axes: Query<&mut Visibility, With<AxisIndicator>>,
) {
    if !config.is_changed() {
        return;
    }
    let vis = if config.show_axes { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut axes { *v = vis; }
}

fn toggle_subgrid_visibility(
    config: Res<GridConfig>,
    mut subgrids: Query<&mut Visibility, (With<SubgridLines>, With<EditorGrid>)>,
) {
    if !config.is_changed() {
        return;
    }
    let vis = if config.visible && config.show_subgrid {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut subgrids { *v = vis; }
}

/// Scale fade distance with orbit distance so zooming out reveals a bigger
/// patch of the grid, Blender-style.
fn update_fade_distance(
    orbit: Option<Res<renzora::core::viewport_types::CameraOrbitSnapshot>>,
    cam_q: Query<&GlobalTransform, With<renzora::editor::EditorCamera>>,
    mut config: ResMut<GridConfig>,
    mut materials: ResMut<Assets<GridMaterial>>,
    grid_entities: Query<&MeshMaterial3d<GridMaterial>>,
) {
    // Use camera elevation above the grid plane as a proxy for zoom since
    // we don't have direct access to orbit.distance from here. Fall back to
    // a constant if we can't find the camera.
    let _ = orbit;
    let Ok(cam_tf) = cam_q.single() else { return };
    // Use camera elevation above the XZ plane as a zoom proxy — close to
    // the ground = tight fade; high above = wide fade.
    let elev = cam_tf.translation().y.abs().max(1.5);
    let new_start = elev * 2.5;
    let new_end = elev * 8.0;

    if (config.fade_start - new_start).abs() < 0.01 && (config.fade_end - new_end).abs() < 0.01 {
        return;
    }
    config.fade_start = new_start;
    config.fade_end = new_end;

    for handle in &grid_entities {
        if let Some(mat) = materials.get_mut(&handle.0) {
            mat.fade_start = new_start;
            mat.fade_end = new_end;
        }
    }
}
