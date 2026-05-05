//! Renzora Grid — 3D editor grid with colored axis indicators.
//!
//! All grid geometry — major lines, minor (subgrid) lines, and the X/Y/Z
//! axes — is packed into a single LineList mesh with per-vertex colors,
//! so the GPU draws the whole grid in one call. The custom unlit
//! `GridMaterial` only carries the camera-distance fade range; line color
//! comes from the vertex COLOR attribute. Being real 3D geometry gives
//! pixel-perfect depth testing against scene meshes for free.

use bevy::camera::visibility::RenderLayers;
use bevy::mesh::{MeshVertexBufferLayoutRef, PrimitiveTopology, VertexAttributeValues};
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use renzora::core::viewport_types::ViewportSettings;

// ── GridMaterial ────────────────────────────────────────────────────────────

/// Unlit line material with horizontal-distance alpha fade. Color is
/// supplied per-vertex via `Mesh::ATTRIBUTE_COLOR` so every line in the
/// merged grid mesh shares this same material handle.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct GridMaterial {
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
                    rebuild_grid_mesh,
                    update_fade_distance,
                )
                    .run_if(in_state(renzora_editor::SplashState::Editor)),
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
    let mesh = build_grid_mesh(&config);
    let material = GridMaterial {
        fade_start: config.fade_start,
        fade_end: config.fade_end,
        _pad0: 0.0,
        _pad1: 0.0,
    };

    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(material)),
        Transform::default(),
        EditorGrid,
        RenderLayers::layer(0),
    ));
}

/// Pack every visible line — major, minor, axes — into a single LineList
/// mesh with vertex colors. One mesh + one material = one draw call for
/// the whole grid.
fn build_grid_mesh(config: &GridConfig) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();

    if config.visible {
        let size = config.size;
        let spacing = config.spacing;
        let extent = size as f32 * spacing;
        let major = linear_array(config.major_color);
        let minor = linear_array(config.minor_color);

        for i in -size..=size {
            if i == 0 {
                continue;
            }
            let is_major = i % config.major_every == 0;
            if !is_major && !config.show_subgrid {
                continue;
            }
            let color = if is_major { major } else { minor };
            let pos = i as f32 * spacing;
            // Lines parallel to Z, stepping along X.
            positions.push([pos, 0.0, -extent]);
            colors.push(color);
            positions.push([pos, 0.0, extent]);
            colors.push(color);
            // Lines parallel to X, stepping along Z.
            positions.push([-extent, 0.0, pos]);
            colors.push(color);
            positions.push([extent, 0.0, pos]);
            colors.push(color);
        }
    }

    if config.show_axes {
        // Lift X/Z axes a hair so they sort cleanly above the grid plane,
        // matching the previous per-entity offset.
        let lift = 0.005;
        let red = [0.92, 0.32, 0.36, 1.0];
        let blue = [0.30, 0.58, 0.95, 1.0];
        let green = [0.40, 0.83, 0.44, 1.0];

        positions.push([-500.0, lift, 0.0]);
        colors.push(red);
        positions.push([500.0, lift, 0.0]);
        colors.push(red);

        positions.push([0.0, lift, -500.0]);
        colors.push(blue);
        positions.push([0.0, lift, 500.0]);
        colors.push(blue);

        positions.push([0.0, -500.0, 0.0]);
        colors.push(green);
        positions.push([0.0, 500.0, 0.0]);
        colors.push(green);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::LineList, default());
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        VertexAttributeValues::Float32x3(positions),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_COLOR,
        VertexAttributeValues::Float32x4(colors),
    );
    mesh
}

fn linear_array(c: Color) -> [f32; 4] {
    let l = c.to_linear();
    [l.red, l.green, l.blue, l.alpha]
}

// ── Systems ─────────────────────────────────────────────────────────────────

fn sync_grid_from_viewport(vp: Res<ViewportSettings>, mut config: ResMut<GridConfig>) {
    if !vp.is_changed() {
        return;
    }
    config.visible = vp.show_grid;
    config.show_axes = vp.show_axis_gizmo;
    config.show_subgrid = vp.show_subgrid;
}

/// Rebuild the merged grid mesh whenever toggles or sizing change. Cheap
/// — the line count is small (~hundreds of vertices) — and runs at most
/// once per change, not per frame.
fn rebuild_grid_mesh(
    config: Res<GridConfig>,
    grid: Query<&Mesh3d, With<EditorGrid>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if !config.is_changed() {
        return;
    }
    let Ok(mesh_handle) = grid.single() else {
        return;
    };
    let new_mesh = build_grid_mesh(&config);
    if let Some(mesh) = meshes.get_mut(&mesh_handle.0) {
        *mesh = new_mesh;
    }
}

/// Scale fade distance with orbit distance so zooming out reveals a bigger
/// patch of the grid, Blender-style.
fn update_fade_distance(
    orbit: Option<Res<renzora::core::viewport_types::CameraOrbitSnapshot>>,
    cam_q: Query<&GlobalTransform, With<renzora_editor::EditorCamera>>,
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
    // Bypass change detection so the rebuild system doesn't fire just
    // because we tweaked fade — fade lives in the material UBO, not the
    // mesh, and rebuilding on every camera nudge would defeat the cache.
    let cfg = config.bypass_change_detection();
    cfg.fade_start = new_start;
    cfg.fade_end = new_end;

    for handle in &grid_entities {
        if let Some(mat) = materials.get_mut(&handle.0) {
            mat.fade_start = new_start;
            mat.fade_end = new_end;
        }
    }
}

renzora::add!(GridPlugin, Editor);
