//! Brush creation systems for click-drag level geometry creation
//!
//! Handles the viewport interaction flow for creating brushes:
//! 1. Click to start placement (raycast to ground plane)
//! 2. Drag to set XZ dimensions
//! 3. Shift+drag to adjust height
//! 4. Release to finalize

use bevy::prelude::*;

use crate::core::{EditorEntity, InputFocusState, SceneNode, ViewportState};
use crate::gizmo::{EditorTool, GizmoState};
use crate::shared::MeshNodeData;

use super::{
    BrushCreationPhase, BrushData, BrushSettings, BrushState, BrushType,
    DefaultBrushMaterial, create_brush_material,
};

/// Marker component for brush preview entities
#[derive(Component)]
pub struct BrushPreview;

/// System to handle B key shortcut for brush tool
pub fn brush_tool_shortcut_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut gizmo_state: ResMut<GizmoState>,
    input_focus: Res<InputFocusState>,
) {
    // Don't switch tools if egui has focus
    if input_focus.egui_wants_keyboard {
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyB) {
        gizmo_state.tool = EditorTool::Brush;
    }
}

/// System to start brush creation on click
pub fn brush_creation_start_system(
    mouse: Res<ButtonInput<MouseButton>>,
    mut brush_state: ResMut<BrushState>,
    brush_settings: Res<BrushSettings>,
    gizmo_state: Res<GizmoState>,
    viewport: Res<ViewportState>,
    input_focus: Res<InputFocusState>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<crate::core::ViewportCamera>>,
) {
    // Only in brush mode
    if gizmo_state.tool != EditorTool::Brush {
        return;
    }

    // Don't start if already creating
    if brush_state.is_creating() {
        return;
    }

    // Don't start if egui wants input
    if input_focus.egui_wants_keyboard {
        return;
    }

    // Check for left mouse click
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    // Check if click is in viewport
    let Ok(window) = windows.single() else { return };
    let Some(cursor_pos) = window.cursor_position() else { return };

    if !viewport.contains_point(cursor_pos.x, cursor_pos.y) {
        return;
    }

    // Raycast to ground plane (Y=0)
    let Ok((camera, camera_transform)) = camera_query.single() else { return };

    // Convert cursor position to viewport-local coordinates
    let viewport_pos = Vec2::new(
        cursor_pos.x - viewport.position[0],
        cursor_pos.y - viewport.position[1],
    );

    // Get ray from camera through cursor
    let ray = camera.viewport_to_world(camera_transform, viewport_pos);
    let Ok(ray) = ray else { return };

    // Intersect with ground plane (Y=0)
    let ground_normal = Vec3::Y;
    let ground_point = Vec3::ZERO;

    let denom = ground_normal.dot(*ray.direction);
    if denom.abs() < 0.0001 {
        return; // Ray parallel to ground
    }

    let t = (ground_point - ray.origin).dot(ground_normal) / denom;
    if t < 0.0 {
        return; // Intersection behind camera
    }

    let hit_point = ray.origin + *ray.direction * t;

    // Snap to grid if enabled
    let snapped_point = if brush_settings.snap_enabled {
        brush_settings.snap_vec3(hit_point)
    } else {
        hit_point
    };

    // Start brush creation
    brush_state.start(snapped_point, brush_settings.selected_brush);
}

/// System to update brush dimensions during drag
pub fn brush_creation_drag_system(
    mouse: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut brush_state: ResMut<BrushState>,
    brush_settings: Res<BrushSettings>,
    gizmo_state: Res<GizmoState>,
    viewport: Res<ViewportState>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<crate::core::ViewportCamera>>,
) {
    // Only in brush mode and while creating
    if gizmo_state.tool != EditorTool::Brush {
        return;
    }

    if !brush_state.is_creating() {
        return;
    }

    // Check if mouse is still down
    if !mouse.pressed(MouseButton::Left) {
        return;
    }

    // Update phase based on shift key
    if keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight) {
        brush_state.phase = BrushCreationPhase::DraggingHeight;
    } else if brush_state.phase == BrushCreationPhase::Started {
        brush_state.phase = BrushCreationPhase::DraggingXZ;
    }

    // Get cursor position
    let Ok(window) = windows.single() else { return };
    let Some(cursor_pos) = window.cursor_position() else { return };

    // Raycast to get current position
    let Ok((camera, camera_transform)) = camera_query.single() else { return };

    let viewport_pos = Vec2::new(
        cursor_pos.x - viewport.position[0],
        cursor_pos.y - viewport.position[1],
    );

    let ray = camera.viewport_to_world(camera_transform, viewport_pos);
    let Ok(ray) = ray else { return };

    // Intersect with appropriate plane based on dragging mode
    let hit_point = if brush_state.phase == BrushCreationPhase::DraggingHeight {
        // For height dragging, intersect with vertical plane through start point
        // facing the camera
        let camera_forward = camera_transform.forward();
        let plane_normal = Vec3::new(camera_forward.x, 0.0, camera_forward.z).normalize_or_zero();
        if plane_normal.length_squared() < 0.0001 {
            return;
        }

        let denom = plane_normal.dot(*ray.direction);
        if denom.abs() < 0.0001 {
            return;
        }

        let t = (brush_state.start_position - ray.origin).dot(plane_normal) / denom;
        if t < 0.0 {
            return;
        }

        ray.origin + *ray.direction * t
    } else {
        // For XZ dragging, intersect with ground plane
        let ground_normal = Vec3::Y;
        let ground_point = Vec3::new(0.0, brush_state.start_position.y, 0.0);

        let denom = ground_normal.dot(*ray.direction);
        if denom.abs() < 0.0001 {
            return;
        }

        let t = (ground_point - ray.origin).dot(ground_normal) / denom;
        if t < 0.0 {
            return;
        }

        ray.origin + *ray.direction * t
    };

    // Snap to grid if enabled
    let snapped_point = if brush_settings.snap_enabled {
        brush_settings.snap_vec3(hit_point)
    } else {
        hit_point
    };

    brush_state.current_position = snapped_point;
}

/// System to render the brush preview while creating
pub fn brush_preview_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut brush_state: ResMut<BrushState>,
    gizmo_state: Res<GizmoState>,
    preview_query: Query<Entity, With<BrushPreview>>,
) {
    // Clean up preview if not in brush mode or not creating
    if gizmo_state.tool != EditorTool::Brush || !brush_state.is_creating() {
        // Despawn any existing preview
        for entity in preview_query.iter() {
            commands.entity(entity).despawn();
        }
        brush_state.preview_entity = None;
        return;
    }

    // Calculate preview dimensions and position
    let default_height = brush_state.creating_brush_type.default_height();
    let dimensions = brush_state.calculate_dimensions(default_height);
    let center = brush_state.calculate_center(default_height);

    // Create or update preview mesh
    let mesh = create_brush_mesh(brush_state.creating_brush_type, dimensions, &mut meshes);

    // Create semi-transparent preview material
    let preview_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.35, 0.45, 0.73, 0.5),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    // Despawn old preview if dimensions changed
    for entity in preview_query.iter() {
        commands.entity(entity).despawn();
    }

    // Spawn new preview
    let preview = commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(preview_material),
        Transform::from_translation(center),
        Visibility::default(),
        BrushPreview,
    )).id();

    brush_state.preview_entity = Some(preview);
}

/// System to finalize brush creation on mouse release
pub fn brush_creation_end_system(
    mouse: Res<ButtonInput<MouseButton>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut brush_state: ResMut<BrushState>,
    brush_settings: Res<BrushSettings>,
    default_material: Res<DefaultBrushMaterial>,
    gizmo_state: Res<GizmoState>,
    preview_query: Query<Entity, With<BrushPreview>>,
) {
    // Only in brush mode and while creating
    if gizmo_state.tool != EditorTool::Brush {
        return;
    }

    if !brush_state.is_creating() {
        return;
    }

    // Check if mouse was released
    if !mouse.just_released(MouseButton::Left) {
        return;
    }

    // Despawn preview
    for entity in preview_query.iter() {
        commands.entity(entity).despawn();
    }

    // Calculate final dimensions and position
    let default_height = brush_state.creating_brush_type.default_height();
    let dimensions = brush_state.calculate_dimensions(default_height);
    let center = brush_state.calculate_center(default_height);

    // Don't create if dimensions are too small (just a click, not a drag)
    let min_size = 0.2;
    if dimensions.x < min_size && dimensions.z < min_size {
        brush_state.reset();
        return;
    }

    // Create the actual brush entity
    let mesh = create_brush_mesh(brush_state.creating_brush_type, dimensions, &mut meshes);
    let material = if brush_settings.use_checkerboard {
        create_brush_material(&default_material, &mut materials)
    } else {
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.7, 0.7, 0.7),
            perceptual_roughness: 0.9,
            ..default()
        })
    };

    let brush_name = format!("{}", brush_state.creating_brush_type.display_name());

    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(center),
        Visibility::default(),
        EditorEntity {
            name: brush_name,
            tag: String::new(),
            visible: true,
            locked: false,
        },
        SceneNode,
        MeshNodeData {
            mesh_type: crate::shared::MeshPrimitiveType::Cube,
        },
        BrushData {
            brush_type: brush_state.creating_brush_type,
            dimensions,
        },
    ));

    // Reset brush state
    brush_state.reset();
}

/// Create a mesh for the given brush type and dimensions
pub fn create_brush_mesh(
    brush_type: BrushType,
    dimensions: Vec3,
    meshes: &mut Assets<Mesh>,
) -> Handle<Mesh> {
    match brush_type {
        BrushType::Block | BrushType::Floor | BrushType::Wall => {
            meshes.add(Cuboid::new(dimensions.x, dimensions.y, dimensions.z))
        }
        BrushType::Stairs => {
            // Create stepped geometry
            create_stairs_mesh(dimensions, 4, meshes)
        }
        BrushType::Ramp => {
            // Create angled slope
            create_ramp_mesh(dimensions, meshes)
        }
    }
}

/// Create a stairs mesh with the given dimensions and step count
fn create_stairs_mesh(dimensions: Vec3, step_count: usize, meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
    // For now, use a simple box as placeholder
    // TODO: Implement proper stairs mesh generation
    meshes.add(Cuboid::new(dimensions.x, dimensions.y, dimensions.z))
}

/// Create a ramp mesh with the given dimensions
fn create_ramp_mesh(dimensions: Vec3, meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
    // For now, use a simple box as placeholder
    // TODO: Implement proper ramp mesh generation (wedge shape)
    meshes.add(Cuboid::new(dimensions.x, dimensions.y, dimensions.z))
}

/// Spawn a brush entity with the given parameters
pub fn spawn_brush(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    default_material: &DefaultBrushMaterial,
    brush_type: BrushType,
    dimensions: Vec3,
    position: Vec3,
    use_checkerboard: bool,
) -> Entity {
    let mesh = create_brush_mesh(brush_type, dimensions, meshes);
    let material = if use_checkerboard {
        create_brush_material(default_material, materials)
    } else {
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.7, 0.7, 0.7),
            perceptual_roughness: 0.9,
            ..default()
        })
    };

    let brush_name = format!("{}", brush_type.display_name());

    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(position),
        Visibility::default(),
        EditorEntity {
            name: brush_name,
            tag: String::new(),
            visible: true,
            locked: false,
        },
        SceneNode,
        MeshNodeData {
            mesh_type: crate::shared::MeshPrimitiveType::Cube,
        },
        BrushData {
            brush_type,
            dimensions,
        },
    )).id()
}
