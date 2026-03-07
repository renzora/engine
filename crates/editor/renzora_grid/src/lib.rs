//! Renzora Grid — 3D editor grid with colored axis indicators.
//!
//! Spawns an infinite-style grid on the XZ plane with:
//! - Thin gray lines at 1-unit spacing
//! - Thicker gray lines every 10 units
//! - Red line along the X axis
//! - Blue line along the Z axis
//! - Green line along the Y axis (vertical)

use bevy::prelude::*;
use bevy::camera::visibility::RenderLayers;
use bevy::mesh::{PrimitiveTopology, VertexAttributeValues};

/// Marker component for the editor grid entity.
#[derive(Component)]
pub struct EditorGrid;

/// Marker component for axis gizmo lines.
#[derive(Component)]
pub struct AxisIndicator;

/// Configuration for the editor grid.
#[derive(Resource)]
pub struct GridConfig {
    /// Half-extent of the grid in units (grid goes from -size to +size).
    pub size: i32,
    /// Spacing between minor grid lines.
    pub spacing: f32,
    /// How many minor lines between major lines.
    pub major_every: i32,
    /// Color for minor grid lines.
    pub minor_color: Color,
    /// Color for major grid lines.
    pub major_color: Color,
    /// Whether the grid is visible.
    pub visible: bool,
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            size: 100,
            spacing: 1.0,
            major_every: 10,
            minor_color: Color::srgba(0.3, 0.3, 0.3, 0.4),
            major_color: Color::srgba(0.4, 0.4, 0.4, 0.6),
            visible: true,
        }
    }
}

pub struct GridPlugin;

impl Plugin for GridPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GridConfig>()
            .add_systems(PostStartup, spawn_grid)
            .add_systems(Update, (
                sync_grid_from_viewport,
                toggle_grid_visibility,
                toggle_axis_visibility,
            ).run_if(in_state(renzora_splash::SplashState::Editor)));
    }
}

fn spawn_grid(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    config: Res<GridConfig>,
) {
    // Build grid mesh
    let grid_mesh = build_grid_mesh(&config);
    commands.spawn((
        Mesh3d(meshes.add(grid_mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::default(),
        EditorGrid,
        RenderLayers::layer(1),
    ));

    // X axis (red)
    let x_axis = build_axis_line(Vec3::NEG_X * 500.0, Vec3::X * 500.0);
    commands.spawn((
        Mesh3d(meshes.add(x_axis)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.9, 0.2, 0.2, 0.8),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.005, 0.0),
        AxisIndicator,
        RenderLayers::layer(1),
    ));

    // Z axis (blue)
    let z_axis = build_axis_line(Vec3::NEG_Z * 500.0, Vec3::Z * 500.0);
    commands.spawn((
        Mesh3d(meshes.add(z_axis)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.2, 0.4, 0.9, 0.8),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.005, 0.0),
        AxisIndicator,
        RenderLayers::layer(1),
    ));

    // Y axis (green, vertical)
    let y_axis = build_axis_line(Vec3::NEG_Y * 500.0, Vec3::Y * 500.0);
    commands.spawn((
        Mesh3d(meshes.add(y_axis)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.3, 0.8, 0.2, 0.8),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::default(),
        AxisIndicator,
        RenderLayers::layer(1),
    ));
}

fn build_grid_mesh(config: &GridConfig) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();

    let size = config.size;
    let spacing = config.spacing;
    let extent = size as f32 * spacing;

    let minor: [f32; 4] = [
        config.minor_color.to_linear().red,
        config.minor_color.to_linear().green,
        config.minor_color.to_linear().blue,
        config.minor_color.alpha(),
    ];
    let major: [f32; 4] = [
        config.major_color.to_linear().red,
        config.major_color.to_linear().green,
        config.major_color.to_linear().blue,
        config.major_color.alpha(),
    ];

    for i in -size..=size {
        // Skip the center lines (axes drawn separately)
        if i == 0 {
            continue;
        }

        let is_major = i % config.major_every == 0;
        let color = if is_major { major } else { minor };
        let pos = i as f32 * spacing;

        // Line parallel to Z axis (constant X)
        positions.push([pos, 0.0, -extent]);
        positions.push([pos, 0.0, extent]);
        colors.push(color);
        colors.push(color);

        // Line parallel to X axis (constant Z)
        positions.push([-extent, 0.0, pos]);
        positions.push([extent, 0.0, pos]);
        colors.push(color);
        colors.push(color);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::LineList, default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, VertexAttributeValues::Float32x3(positions));
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, VertexAttributeValues::Float32x4(colors));
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

fn sync_grid_from_viewport(
    vp: Res<renzora_viewport::ViewportSettings>,
    mut config: ResMut<GridConfig>,
) {
    if !vp.is_changed() {
        return;
    }
    config.visible = vp.show_grid;
}

fn toggle_grid_visibility(
    config: Res<GridConfig>,
    mut grids: Query<&mut Visibility, With<EditorGrid>>,
) {
    if !config.is_changed() {
        return;
    }
    let vis = if config.visible { Visibility::Inherited } else { Visibility::Hidden };
    for mut visibility in &mut grids {
        *visibility = vis;
    }
}

fn toggle_axis_visibility(
    vp: Res<renzora_viewport::ViewportSettings>,
    mut axes: Query<&mut Visibility, With<AxisIndicator>>,
) {
    if !vp.is_changed() {
        return;
    }
    let vis = if vp.show_axis_gizmo { Visibility::Inherited } else { Visibility::Hidden };
    for mut visibility in &mut axes {
        *visibility = vis;
    }
}
