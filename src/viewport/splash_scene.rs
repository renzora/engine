//! Retro 3D splash screen scene
//! Renders wireframe shapes and a grid with a rotating camera

use bevy::prelude::*;
use bevy::camera::RenderTarget;
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use std::f32::consts::{PI, TAU};

/// Resource holding the splash scene render texture
#[derive(Resource)]
pub struct SplashSceneImage(pub Handle<Image>);

/// Marker for the splash scene camera
#[derive(Component)]
pub struct SplashSceneCamera;

/// Marker for splash scene entities (cleaned up when entering editor)
#[derive(Component)]
pub struct SplashSceneEntity;

/// State for the rotating camera and animation
#[derive(Resource)]
pub struct SplashSceneState {
    pub camera_angle: f32,
    pub camera_height_phase: f32,
}

impl Default for SplashSceneState {
    fn default() -> Self {
        Self {
            camera_angle: 0.0,
            camera_height_phase: 0.0,
        }
    }
}

/// Retro shape definition for animation
pub struct RetroShape {
    pub position: Vec3,
    pub rotation_speed: Vec3,
    pub current_rotation: Vec3,
    pub shape_type: RetroShapeType,
    pub color: Color,
    pub size: f32,
}

#[derive(Clone, Copy)]
pub enum RetroShapeType {
    Cube,
    Pyramid,
    Octahedron,
    Diamond,
    Torus,
}

/// Resource holding the animated shapes
#[derive(Resource)]
pub struct RetroShapes(pub Vec<RetroShape>);

impl Default for RetroShapes {
    fn default() -> Self {
        let colors = [
            Color::srgb(0.4, 0.8, 1.0),   // Cyan
            Color::srgb(1.0, 0.4, 0.7),   // Pink
            Color::srgb(0.5, 1.0, 0.6),   // Green
            Color::srgb(1.0, 0.8, 0.3),   // Yellow
            Color::srgb(0.7, 0.5, 1.0),   // Purple
            Color::srgb(1.0, 0.5, 0.3),   // Orange
        ];

        let shapes = vec![
            // Center large cube
            RetroShape {
                position: Vec3::new(0.0, 1.5, 0.0),
                rotation_speed: Vec3::new(0.3, 0.5, 0.2),
                current_rotation: Vec3::ZERO,
                shape_type: RetroShapeType::Cube,
                color: colors[0],
                size: 2.0,
            },
            // Floating pyramid
            RetroShape {
                position: Vec3::new(4.0, 2.0, 3.0),
                rotation_speed: Vec3::new(0.4, 0.3, 0.5),
                current_rotation: Vec3::new(0.5, 0.0, 0.0),
                shape_type: RetroShapeType::Pyramid,
                color: colors[1],
                size: 1.5,
            },
            // Octahedron
            RetroShape {
                position: Vec3::new(-3.5, 1.8, 2.5),
                rotation_speed: Vec3::new(0.2, 0.6, 0.3),
                current_rotation: Vec3::new(0.0, 0.3, 0.0),
                shape_type: RetroShapeType::Octahedron,
                color: colors[2],
                size: 1.3,
            },
            // Diamond
            RetroShape {
                position: Vec3::new(3.0, 2.5, -3.0),
                rotation_speed: Vec3::new(0.5, 0.4, 0.2),
                current_rotation: Vec3::new(0.2, 0.0, 0.5),
                shape_type: RetroShapeType::Diamond,
                color: colors[3],
                size: 1.2,
            },
            // Another cube
            RetroShape {
                position: Vec3::new(-4.0, 1.2, -2.5),
                rotation_speed: Vec3::new(0.35, 0.25, 0.45),
                current_rotation: Vec3::new(0.0, 0.5, 0.2),
                shape_type: RetroShapeType::Cube,
                color: colors[4],
                size: 1.4,
            },
            // Torus-ish (octahedron for now)
            RetroShape {
                position: Vec3::new(0.0, 3.5, -4.0),
                rotation_speed: Vec3::new(0.6, 0.3, 0.4),
                current_rotation: Vec3::new(0.3, 0.2, 0.0),
                shape_type: RetroShapeType::Pyramid,
                color: colors[5],
                size: 1.0,
            },
            // Small floating cubes
            RetroShape {
                position: Vec3::new(2.0, 0.8, 5.0),
                rotation_speed: Vec3::new(0.8, 0.6, 0.4),
                current_rotation: Vec3::ZERO,
                shape_type: RetroShapeType::Cube,
                color: colors[0],
                size: 0.8,
            },
            RetroShape {
                position: Vec3::new(-2.5, 1.0, 4.5),
                rotation_speed: Vec3::new(0.5, 0.8, 0.3),
                current_rotation: Vec3::ZERO,
                shape_type: RetroShapeType::Diamond,
                color: colors[2],
                size: 0.9,
            },
        ];

        Self(shapes)
    }
}

/// Sets up the splash scene render texture
pub fn setup_splash_scene(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    // Create render texture (full window size will be handled by resize)
    let size = Extent3d {
        width: 1600,
        height: 900,
        depth_or_array_layers: 1,
    };

    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("splash_scene_texture"),
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Bgra8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        },
        ..default()
    };
    image.resize(size);

    let image_handle = images.add(image);
    commands.insert_resource(SplashSceneImage(image_handle.clone()));

    // Initialize state and shapes
    commands.init_resource::<SplashSceneState>();
    commands.init_resource::<RetroShapes>();

    // Spawn the splash scene camera
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.02, 0.02, 0.04)),
            order: -10, // Render before everything
            ..default()
        },
        RenderTarget::Image(image_handle.into()),
        Transform::from_xyz(12.0, 8.0, 12.0).looking_at(Vec3::new(0.0, 1.5, 0.0), Vec3::Y),
        SplashSceneCamera,
        SplashSceneEntity,
    ));
}

/// Update the rotating camera
pub fn update_splash_camera(
    time: Res<Time>,
    mut state: ResMut<SplashSceneState>,
    mut camera: Query<&mut Transform, With<SplashSceneCamera>>,
) {
    let dt = time.delta_secs();

    // Rotate camera around the scene
    state.camera_angle += dt * 0.3; // Slow rotation
    state.camera_height_phase += dt * 0.5;

    if let Some(mut transform) = camera.iter_mut().next() {
        let radius = 14.0;
        let height = 6.0 + (state.camera_height_phase).sin() * 2.0;

        let x = radius * state.camera_angle.cos();
        let z = radius * state.camera_angle.sin();

        transform.translation = Vec3::new(x, height, z);
        transform.look_at(Vec3::new(0.0, 1.5, 0.0), Vec3::Y);
    }
}

/// Update shape rotations
pub fn update_retro_shapes(time: Res<Time>, mut shapes: ResMut<RetroShapes>) {
    let dt = time.delta_secs();

    for shape in &mut shapes.0 {
        shape.current_rotation += shape.rotation_speed * dt;
    }
}

/// Draw the retro grid and shapes using gizmos
pub fn draw_splash_scene(
    mut gizmos: Gizmos,
    shapes: Res<RetroShapes>,
) {
    // Draw the retro grid
    draw_retro_grid(&mut gizmos);

    // Draw each shape
    for shape in &shapes.0 {
        let rotation = Quat::from_euler(
            EulerRot::XYZ,
            shape.current_rotation.x,
            shape.current_rotation.y,
            shape.current_rotation.z,
        );

        match shape.shape_type {
            RetroShapeType::Cube => draw_wireframe_cube(&mut gizmos, shape.position, rotation, shape.size, shape.color),
            RetroShapeType::Pyramid => draw_wireframe_pyramid(&mut gizmos, shape.position, rotation, shape.size, shape.color),
            RetroShapeType::Octahedron => draw_wireframe_octahedron(&mut gizmos, shape.position, rotation, shape.size, shape.color),
            RetroShapeType::Diamond => draw_wireframe_diamond(&mut gizmos, shape.position, rotation, shape.size, shape.color),
            RetroShapeType::Torus => draw_wireframe_torus(&mut gizmos, shape.position, rotation, shape.size, shape.color),
        }
    }
}

fn draw_retro_grid(gizmos: &mut Gizmos) {
    let grid_color = Color::srgb(0.15, 0.3, 0.4);
    let grid_size = 20;
    let grid_spacing = 1.0;
    let half_size = grid_size as f32 * grid_spacing / 2.0;

    // Horizontal lines (along X)
    for i in 0..=grid_size {
        let z = -half_size + i as f32 * grid_spacing;
        gizmos.line(
            Vec3::new(-half_size, 0.0, z),
            Vec3::new(half_size, 0.0, z),
            grid_color,
        );
    }

    // Vertical lines (along Z)
    for i in 0..=grid_size {
        let x = -half_size + i as f32 * grid_spacing;
        gizmos.line(
            Vec3::new(x, 0.0, -half_size),
            Vec3::new(x, 0.0, half_size),
            grid_color,
        );
    }

    // Add some "horizon" lines for depth effect
    let horizon_color = Color::srgb(0.1, 0.2, 0.3);
    for i in 1..=5 {
        let y = i as f32 * 0.5;
        let scale = 1.0 + i as f32 * 0.3;
        gizmos.line(
            Vec3::new(-half_size * scale, y, -half_size * scale),
            Vec3::new(half_size * scale, y, -half_size * scale),
            horizon_color,
        );
    }
}

fn draw_wireframe_cube(gizmos: &mut Gizmos, center: Vec3, rotation: Quat, size: f32, color: Color) {
    let half = size / 2.0;

    // 8 vertices of a cube
    let vertices = [
        Vec3::new(-half, -half, -half),
        Vec3::new(half, -half, -half),
        Vec3::new(half, half, -half),
        Vec3::new(-half, half, -half),
        Vec3::new(-half, -half, half),
        Vec3::new(half, -half, half),
        Vec3::new(half, half, half),
        Vec3::new(-half, half, half),
    ];

    // Transform vertices
    let transformed: Vec<Vec3> = vertices.iter()
        .map(|v| center + rotation * *v)
        .collect();

    // 12 edges of a cube
    let edges = [
        (0, 1), (1, 2), (2, 3), (3, 0), // Front face
        (4, 5), (5, 6), (6, 7), (7, 4), // Back face
        (0, 4), (1, 5), (2, 6), (3, 7), // Connecting edges
    ];

    for (a, b) in edges {
        gizmos.line(transformed[a], transformed[b], color);
    }
}

fn draw_wireframe_pyramid(gizmos: &mut Gizmos, center: Vec3, rotation: Quat, size: f32, color: Color) {
    let half = size / 2.0;
    let height = size;

    // 5 vertices: 4 base + 1 apex
    let vertices = [
        Vec3::new(-half, 0.0, -half),
        Vec3::new(half, 0.0, -half),
        Vec3::new(half, 0.0, half),
        Vec3::new(-half, 0.0, half),
        Vec3::new(0.0, height, 0.0),
    ];

    let transformed: Vec<Vec3> = vertices.iter()
        .map(|v| center + rotation * *v)
        .collect();

    // Base edges
    for i in 0..4 {
        gizmos.line(transformed[i], transformed[(i + 1) % 4], color);
    }

    // Edges to apex
    for i in 0..4 {
        gizmos.line(transformed[i], transformed[4], color);
    }
}

fn draw_wireframe_octahedron(gizmos: &mut Gizmos, center: Vec3, rotation: Quat, size: f32, color: Color) {
    let half = size / 2.0;

    // 6 vertices
    let vertices = [
        Vec3::new(0.0, half, 0.0),    // Top
        Vec3::new(0.0, -half, 0.0),   // Bottom
        Vec3::new(half, 0.0, 0.0),    // Right
        Vec3::new(-half, 0.0, 0.0),   // Left
        Vec3::new(0.0, 0.0, half),    // Front
        Vec3::new(0.0, 0.0, -half),   // Back
    ];

    let transformed: Vec<Vec3> = vertices.iter()
        .map(|v| center + rotation * *v)
        .collect();

    // Top edges
    gizmos.line(transformed[0], transformed[2], color);
    gizmos.line(transformed[0], transformed[3], color);
    gizmos.line(transformed[0], transformed[4], color);
    gizmos.line(transformed[0], transformed[5], color);

    // Bottom edges
    gizmos.line(transformed[1], transformed[2], color);
    gizmos.line(transformed[1], transformed[3], color);
    gizmos.line(transformed[1], transformed[4], color);
    gizmos.line(transformed[1], transformed[5], color);

    // Middle ring
    gizmos.line(transformed[2], transformed[4], color);
    gizmos.line(transformed[4], transformed[3], color);
    gizmos.line(transformed[3], transformed[5], color);
    gizmos.line(transformed[5], transformed[2], color);
}

fn draw_wireframe_diamond(gizmos: &mut Gizmos, center: Vec3, rotation: Quat, size: f32, color: Color) {
    // Elongated octahedron
    let half_w = size / 2.0;
    let half_h = size;

    let vertices = [
        Vec3::new(0.0, half_h, 0.0),     // Top
        Vec3::new(0.0, -half_h * 0.3, 0.0),  // Bottom (shorter)
        Vec3::new(half_w, 0.0, 0.0),
        Vec3::new(-half_w, 0.0, 0.0),
        Vec3::new(0.0, 0.0, half_w),
        Vec3::new(0.0, 0.0, -half_w),
    ];

    let transformed: Vec<Vec3> = vertices.iter()
        .map(|v| center + rotation * *v)
        .collect();

    // Top edges
    for i in 2..6 {
        gizmos.line(transformed[0], transformed[i], color);
    }

    // Bottom edges
    for i in 2..6 {
        gizmos.line(transformed[1], transformed[i], color);
    }

    // Middle ring
    gizmos.line(transformed[2], transformed[4], color);
    gizmos.line(transformed[4], transformed[3], color);
    gizmos.line(transformed[3], transformed[5], color);
    gizmos.line(transformed[5], transformed[2], color);
}

fn draw_wireframe_torus(gizmos: &mut Gizmos, center: Vec3, rotation: Quat, size: f32, color: Color) {
    let major_radius = size;
    let segments = 12;

    // Draw circles at different angles around the torus
    for i in 0..segments {
        let angle = (i as f32 / segments as f32) * TAU;
        let next_angle = ((i + 1) as f32 / segments as f32) * TAU;

        let p1 = Vec3::new(major_radius * angle.cos(), 0.0, major_radius * angle.sin());
        let p2 = Vec3::new(major_radius * next_angle.cos(), 0.0, major_radius * next_angle.sin());

        gizmos.line(center + rotation * p1, center + rotation * p2, color);

        // Add vertical lines for 3D effect
        if i % 3 == 0 {
            let up = Vec3::new(
                major_radius * 0.7 * angle.cos(),
                size * 0.4,
                major_radius * 0.7 * angle.sin(),
            );
            let down = Vec3::new(
                major_radius * 0.7 * angle.cos(),
                -size * 0.4,
                major_radius * 0.7 * angle.sin(),
            );
            gizmos.line(center + rotation * p1, center + rotation * up, color);
            gizmos.line(center + rotation * p1, center + rotation * down, color);
        }
    }
}

/// Clean up splash scene when entering editor
pub fn cleanup_splash_scene(
    mut commands: Commands,
    entities: Query<Entity, With<SplashSceneEntity>>,
) {
    for entity in entities.iter() {
        commands.entity(entity).despawn_recursive();
    }
    commands.remove_resource::<SplashSceneState>();
    commands.remove_resource::<RetroShapes>();
    commands.remove_resource::<SplashSceneImage>();
}
