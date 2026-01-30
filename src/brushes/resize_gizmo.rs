//! Block resize handles for editing brush/mesh dimensions
//!
//! Provides face-based resize handles that appear when a block-like mesh is selected
//! in Transform mode. Works with BrushData entities and regular Cube/Plane meshes.

use bevy::prelude::*;
use bevy::camera::visibility::RenderLayers;

use crate::core::{SelectionState, ViewportState};
use crate::gizmo::{EditorTool, GizmoState, GIZMO_RENDER_LAYER};
use crate::shared::{MeshNodeData, MeshPrimitiveType};

use super::{BrushData, BrushSettings};

/// Which face handle is being interacted with
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockEditHandle {
    /// +X face (right)
    PosX,
    /// -X face (left)
    NegX,
    /// +Y face (top)
    PosY,
    /// -Y face (bottom)
    NegY,
    /// +Z face (front)
    PosZ,
    /// -Z face (back)
    NegZ,
}

impl BlockEditHandle {
    /// Get the direction vector for this handle
    pub fn direction(&self) -> Vec3 {
        match self {
            BlockEditHandle::PosX => Vec3::X,
            BlockEditHandle::NegX => Vec3::NEG_X,
            BlockEditHandle::PosY => Vec3::Y,
            BlockEditHandle::NegY => Vec3::NEG_Y,
            BlockEditHandle::PosZ => Vec3::Z,
            BlockEditHandle::NegZ => Vec3::NEG_Z,
        }
    }

    /// Get the opposite handle
    #[allow(dead_code)]
    pub fn opposite(&self) -> BlockEditHandle {
        match self {
            BlockEditHandle::PosX => BlockEditHandle::NegX,
            BlockEditHandle::NegX => BlockEditHandle::PosX,
            BlockEditHandle::PosY => BlockEditHandle::NegY,
            BlockEditHandle::NegY => BlockEditHandle::PosY,
            BlockEditHandle::PosZ => BlockEditHandle::NegZ,
            BlockEditHandle::NegZ => BlockEditHandle::PosZ,
        }
    }

    /// All handles
    pub fn all() -> &'static [BlockEditHandle] {
        &[
            BlockEditHandle::PosX,
            BlockEditHandle::NegX,
            BlockEditHandle::PosY,
            BlockEditHandle::NegY,
            BlockEditHandle::PosZ,
            BlockEditHandle::NegZ,
        ]
    }

    /// Get index for this handle (0-5)
    fn index(&self) -> usize {
        match self {
            BlockEditHandle::PosX => 0,
            BlockEditHandle::NegX => 1,
            BlockEditHandle::PosY => 2,
            BlockEditHandle::NegY => 3,
            BlockEditHandle::PosZ => 4,
            BlockEditHandle::NegZ => 5,
        }
    }
}

/// State for block editing mode
#[derive(Resource, Default)]
pub struct BlockEditState {
    /// Whether block edit mode is active
    pub active: bool,
    /// Entity being edited
    pub entity: Option<Entity>,
    /// Currently hovered handle
    pub hovered_handle: Option<BlockEditHandle>,
    /// Whether currently dragging
    pub is_dragging: bool,
    /// Handle being dragged
    pub drag_handle: Option<BlockEditHandle>,
    /// Starting dimensions when drag began
    pub drag_start_dimensions: Vec3,
    /// Starting position when drag began
    pub drag_start_position: Vec3,
    /// Starting mouse position when drag began
    pub drag_start_mouse: Vec2,
    /// Whether symmetric resize is active (Shift held)
    pub symmetric: bool,
    /// Whether using BrushData or scale-based resize
    pub uses_brush_data: bool,
}

impl BlockEditState {
    /// Enter block edit mode for an entity
    pub fn enter(&mut self, entity: Entity, uses_brush_data: bool) {
        self.active = true;
        self.entity = Some(entity);
        self.hovered_handle = None;
        self.is_dragging = false;
        self.drag_handle = None;
        self.uses_brush_data = uses_brush_data;
    }

    /// Exit block edit mode
    pub fn exit(&mut self) {
        self.active = false;
        self.entity = None;
        self.hovered_handle = None;
        self.is_dragging = false;
        self.drag_handle = None;
    }

    /// Start dragging a handle
    pub fn start_drag(&mut self, handle: BlockEditHandle, dimensions: Vec3, position: Vec3, mouse: Vec2) {
        self.is_dragging = true;
        self.drag_handle = Some(handle);
        self.drag_start_dimensions = dimensions;
        self.drag_start_position = position;
        self.drag_start_mouse = mouse;
    }

    /// End the drag operation
    pub fn end_drag(&mut self) {
        self.is_dragging = false;
        self.drag_handle = None;
    }
}

/// Marker component for resize handle mesh entities
#[derive(Component)]
pub struct ResizeHandleMesh;

/// Component to identify which handle a mesh represents
#[derive(Component)]
pub struct ResizeHandleId(pub BlockEditHandle);

/// Resource storing the handle mesh entities and materials
#[derive(Resource)]
pub struct ResizeHandleMeshes {
    pub handles: [Entity; 6],
    pub normal_material: Handle<StandardMaterial>,
    pub hovered_material: Handle<StandardMaterial>,
    pub dragging_material: Handle<StandardMaterial>,
}

/// Setup system to create the resize handle meshes
pub fn setup_resize_handle_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let depth_bias = -1.0;

    // Create materials - solid red spheres that render on top
    let normal_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.2, 0.2),
        emissive: LinearRgba::new(0.9, 0.2, 0.2, 1.0),
        unlit: true,
        depth_bias,
        ..default()
    });

    let hovered_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.5, 0.3),
        emissive: LinearRgba::new(1.0, 0.5, 0.3, 1.0),
        unlit: true,
        depth_bias,
        ..default()
    });

    let dragging_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 1.0, 0.3),
        emissive: LinearRgba::new(1.0, 1.0, 0.3, 1.0),
        unlit: true,
        depth_bias,
        ..default()
    });

    // Create sphere mesh for handles
    let sphere_mesh = meshes.add(Sphere::new(0.1).mesh().uv(16, 8));

    let render_layers = RenderLayers::layer(GIZMO_RENDER_LAYER);

    // Spawn 6 handle entities (initially hidden)
    let mut handles = [Entity::PLACEHOLDER; 6];

    for handle in BlockEditHandle::all() {
        let entity = commands.spawn((
            Mesh3d(sphere_mesh.clone()),
            MeshMaterial3d(normal_material.clone()),
            Transform::default(),
            GlobalTransform::default(),
            Visibility::Hidden,
            InheritedVisibility::default(),
            ViewVisibility::default(),
            ResizeHandleMesh,
            ResizeHandleId(*handle),
            render_layers.clone(),
        )).id();

        handles[handle.index()] = entity;
    }

    commands.insert_resource(ResizeHandleMeshes {
        handles,
        normal_material,
        hovered_material,
        dragging_material,
    });
}

/// Check if an entity is a block-like mesh that should show resize handles
fn is_block_like_mesh(mesh_data: Option<&MeshNodeData>) -> bool {
    match mesh_data {
        Some(data) => matches!(data.mesh_type, MeshPrimitiveType::Cube | MeshPrimitiveType::Plane),
        None => false,
    }
}

/// Get dimensions for an entity - either from BrushData or from transform scale
fn get_entity_dimensions(
    brush_data: Option<&BrushData>,
    transform: &Transform,
    mesh_data: Option<&MeshNodeData>,
) -> Option<Vec3> {
    if let Some(brush) = brush_data {
        return Some(brush.dimensions);
    }

    // For regular meshes, use scale as dimensions (default mesh size is 1x1x1 for cubes)
    if is_block_like_mesh(mesh_data) {
        let scale = transform.scale;
        // Plane is flat, so Y dimension is very small
        if let Some(data) = mesh_data {
            if data.mesh_type == MeshPrimitiveType::Plane {
                return Some(Vec3::new(scale.x, 0.1, scale.z));
            }
        }
        return Some(scale);
    }

    None
}

/// System to detect hovering over block edit handles
pub fn block_edit_hover_system(
    mouse: Res<ButtonInput<MouseButton>>,
    mut block_edit: ResMut<BlockEditState>,
    gizmo_state: Res<GizmoState>,
    selection: Res<SelectionState>,
    viewport: Res<ViewportState>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<crate::core::ViewportCamera>>,
    entity_query: Query<(&Transform, Option<&BrushData>, Option<&MeshNodeData>)>,
) {
    // Active in Transform mode or BlockEdit mode
    let valid_mode = gizmo_state.tool == EditorTool::Transform || gizmo_state.tool == EditorTool::BlockEdit;
    if !valid_mode {
        if block_edit.active {
            block_edit.exit();
        }
        return;
    }

    // Need a selected entity
    let Some(selected) = selection.selected_entity else {
        if block_edit.active {
            block_edit.exit();
        }
        return;
    };

    let Ok((transform, brush_data, mesh_data)) = entity_query.get(selected) else {
        if block_edit.active {
            block_edit.exit();
        }
        return;
    };

    // Check if this entity can be resized
    let has_brush_data = brush_data.is_some();
    let is_block_mesh = is_block_like_mesh(mesh_data);

    if !has_brush_data && !is_block_mesh {
        if block_edit.active {
            block_edit.exit();
        }
        return;
    }

    // Get dimensions
    let Some(dimensions) = get_entity_dimensions(brush_data, transform, mesh_data) else {
        if block_edit.active {
            block_edit.exit();
        }
        return;
    };

    // Enter edit mode if not already, or if entity changed
    if !block_edit.active || block_edit.entity != Some(selected) {
        block_edit.enter(selected, has_brush_data);
    }

    // Don't update hover while dragging
    if block_edit.is_dragging {
        return;
    }

    // Get cursor position
    let Ok(window) = windows.single() else { return };
    let Some(cursor_pos) = window.cursor_position() else {
        block_edit.hovered_handle = None;
        return;
    };

    // Check if in viewport
    if !viewport.contains_point(cursor_pos.x, cursor_pos.y) {
        block_edit.hovered_handle = None;
        return;
    }

    // Get camera ray
    let Ok((camera, camera_transform)) = camera_query.single() else { return };

    let viewport_pos = Vec2::new(
        cursor_pos.x - viewport.position[0],
        cursor_pos.y - viewport.position[1],
    );

    let ray = camera.viewport_to_world(camera_transform, viewport_pos);
    let Ok(ray) = ray else { return };

    // Test each handle for intersection
    let handle_radius = 0.1;
    let half_dims = dimensions / 2.0;
    let entity_pos = transform.translation;

    // Inset distance - handles are inside the shape
    let inset = 0.25;

    let mut closest_handle = None;
    let mut closest_dist = f32::MAX;

    for handle in BlockEditHandle::all() {
        // Calculate handle position inside the face (not at the edge)
        let handle_center = match handle {
            BlockEditHandle::PosX => entity_pos + Vec3::new((half_dims.x - inset).max(0.0), 0.0, 0.0),
            BlockEditHandle::NegX => entity_pos + Vec3::new(-(half_dims.x - inset).max(0.0), 0.0, 0.0),
            BlockEditHandle::PosY => entity_pos + Vec3::new(0.0, (half_dims.y - inset).max(0.0), 0.0),
            BlockEditHandle::NegY => entity_pos + Vec3::new(0.0, -(half_dims.y - inset).max(0.0), 0.0),
            BlockEditHandle::PosZ => entity_pos + Vec3::new(0.0, 0.0, (half_dims.z - inset).max(0.0)),
            BlockEditHandle::NegZ => entity_pos + Vec3::new(0.0, 0.0, -(half_dims.z - inset).max(0.0)),
        };

        // Ray-sphere intersection for handle
        if let Some(t) = ray_sphere_intersection(ray.origin, *ray.direction, handle_center, handle_radius) {
            if t < closest_dist {
                closest_dist = t;
                closest_handle = Some(*handle);
            }
        }
    }

    block_edit.hovered_handle = closest_handle;

    // Start drag on click
    if mouse.just_pressed(MouseButton::Left) && block_edit.hovered_handle.is_some() {
        let handle = block_edit.hovered_handle.unwrap();
        block_edit.start_drag(
            handle,
            dimensions,
            transform.translation,
            cursor_pos,
        );
    }
}

/// System to handle block edit handle dragging
pub fn block_edit_drag_system(
    mouse: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut block_edit: ResMut<BlockEditState>,
    brush_settings: Res<BrushSettings>,
    gizmo_state: Res<GizmoState>,
    viewport: Res<ViewportState>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<crate::core::ViewportCamera>>,
    mut transform_query: Query<&mut Transform>,
    mut brush_query: Query<&mut BrushData>,
    mut meshes: ResMut<Assets<Mesh>>,
    mesh_handles: Query<&Mesh3d>,
) {
    // Active in Transform mode or BlockEdit mode
    let valid_mode = gizmo_state.tool == EditorTool::Transform || gizmo_state.tool == EditorTool::BlockEdit;
    if !valid_mode {
        return;
    }

    if !block_edit.is_dragging {
        return;
    }

    let Some(entity) = block_edit.entity else { return };
    let Some(handle) = block_edit.drag_handle else { return };

    // Check for mouse release
    if mouse.just_released(MouseButton::Left) {
        block_edit.end_drag();
        return;
    }

    // Update symmetric flag
    block_edit.symmetric = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

    // Get cursor position
    let Ok(window) = windows.single() else { return };
    let Some(cursor_pos) = window.cursor_position() else { return };

    // Get camera for projection
    let Ok((camera, camera_transform)) = camera_query.single() else { return };

    let viewport_pos = Vec2::new(
        cursor_pos.x - viewport.position[0],
        cursor_pos.y - viewport.position[1],
    );

    let ray = camera.viewport_to_world(camera_transform, viewport_pos);
    let Ok(ray) = ray else { return };

    // Get the entity's transform
    let Ok(mut transform) = transform_query.get_mut(entity) else { return };

    // Calculate drag plane based on handle direction
    let drag_dir = handle.direction();
    let plane_normal = if drag_dir.y.abs() > 0.9 {
        // Vertical drag - use camera-facing plane
        let cam_forward = camera_transform.forward();
        Vec3::new(cam_forward.x, 0.0, cam_forward.z).normalize_or_zero()
    } else {
        // Horizontal drag - use vertical plane perpendicular to drag direction
        Vec3::Y.cross(drag_dir).normalize_or_zero()
    };

    if plane_normal.length_squared() < 0.0001 {
        return;
    }

    // Intersect ray with drag plane
    let denom = plane_normal.dot(*ray.direction);
    if denom.abs() < 0.0001 {
        return;
    }

    let plane_point = block_edit.drag_start_position;
    let t = (plane_point - ray.origin).dot(plane_normal) / denom;
    if t < 0.0 {
        return;
    }

    let hit_point = ray.origin + *ray.direction * t;

    // Which axis are we dragging along
    let axis_index = if drag_dir.x.abs() > 0.5 { 0 }
        else if drag_dir.y.abs() > 0.5 { 1 }
        else { 2 };

    // Is this a positive or negative face handle
    let is_positive_face = drag_dir[axis_index] > 0.0;

    // Calculate where the face currently is (at drag start)
    let half_dim = block_edit.drag_start_dimensions[axis_index] / 2.0;
    let start_face_pos = if is_positive_face {
        block_edit.drag_start_position[axis_index] + half_dim
    } else {
        block_edit.drag_start_position[axis_index] - half_dim
    };

    // Where the face should move to based on hit point
    let mut new_face_pos = hit_point[axis_index];

    // Apply snapping to the face position
    if brush_settings.snap_enabled {
        new_face_pos = brush_settings.snap(new_face_pos);
    }

    // Calculate how much the face moved
    let face_delta = new_face_pos - start_face_pos;

    // Calculate new dimensions and position
    let mut new_dims = block_edit.drag_start_dimensions;
    let mut new_pos = block_edit.drag_start_position;

    if block_edit.symmetric {
        // Symmetric resize - both faces move equally
        // For positive face: moving outward = positive delta = increase size
        // For negative face: moving outward = negative delta = increase size
        let size_delta = if is_positive_face { face_delta * 2.0 } else { -face_delta * 2.0 };
        new_dims[axis_index] = (block_edit.drag_start_dimensions[axis_index] + size_delta).max(0.1);
        // Position stays centered
    } else {
        // Single-side resize - one face moves, opposite stays fixed
        // The opposite face position
        let opposite_face_pos = if is_positive_face {
            block_edit.drag_start_position[axis_index] - half_dim
        } else {
            block_edit.drag_start_position[axis_index] + half_dim
        };

        // New dimension is distance between faces
        let new_dim = (new_face_pos - opposite_face_pos).abs();
        new_dims[axis_index] = new_dim.max(0.1);

        // New center is midpoint between faces
        new_pos[axis_index] = (new_face_pos + opposite_face_pos) / 2.0;
    }

    // Apply snapping to final dimensions if enabled
    if brush_settings.snap_enabled {
        new_dims[axis_index] = brush_settings.snap(new_dims[axis_index]).max(brush_settings.snap_size);
    }

    // Update transform position
    transform.translation = new_pos;

    if block_edit.uses_brush_data {
        // Update BrushData dimensions
        if let Ok(mut brush_data) = brush_query.get_mut(entity) {
            brush_data.dimensions = new_dims;
        }

        // Update mesh to match new dimensions
        if let Ok(mesh_handle) = mesh_handles.get(entity) {
            let new_mesh = Cuboid::new(new_dims.x, new_dims.y, new_dims.z);
            let _ = meshes.insert(&mesh_handle.0, new_mesh.into());
        }
    } else {
        // For regular meshes, update scale
        transform.scale = new_dims;
    }
}

/// System to update resize handle mesh positions, visibility, and materials
pub fn update_resize_handle_meshes(
    block_edit: Res<BlockEditState>,
    gizmo_state: Res<GizmoState>,
    handle_meshes: Option<Res<ResizeHandleMeshes>>,
    entity_query: Query<(&Transform, Option<&BrushData>, Option<&MeshNodeData>), Without<ResizeHandleMesh>>,
    mut handle_query: Query<(&ResizeHandleId, &mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>), With<ResizeHandleMesh>>,
) {
    let Some(meshes) = handle_meshes else { return };

    // Active in Transform mode or BlockEdit mode
    let valid_mode = gizmo_state.tool == EditorTool::Transform || gizmo_state.tool == EditorTool::BlockEdit;

    // Check if we should show handles
    let show_handles = valid_mode && block_edit.active && block_edit.entity.is_some();

    if !show_handles {
        // Hide all handles
        for (_, _, mut visibility, _) in handle_query.iter_mut() {
            *visibility = Visibility::Hidden;
        }
        return;
    }

    let Some(entity) = block_edit.entity else { return };
    let Ok((transform, brush_data, mesh_data)) = entity_query.get(entity) else {
        // Hide all handles if entity not found
        for (_, _, mut visibility, _) in handle_query.iter_mut() {
            *visibility = Visibility::Hidden;
        }
        return;
    };

    // Get dimensions
    let Some(dimensions) = get_entity_dimensions(brush_data, transform, mesh_data) else {
        for (_, _, mut visibility, _) in handle_query.iter_mut() {
            *visibility = Visibility::Hidden;
        }
        return;
    };

    let half_dims = dimensions / 2.0;
    let entity_pos = transform.translation;
    let inset = 0.25;

    // Update each handle
    for (handle_id, mut handle_transform, mut visibility, mut material) in handle_query.iter_mut() {
        let handle = handle_id.0;

        // Calculate handle position inside the face
        let handle_center = match handle {
            BlockEditHandle::PosX => entity_pos + Vec3::new((half_dims.x - inset).max(0.0), 0.0, 0.0),
            BlockEditHandle::NegX => entity_pos + Vec3::new(-(half_dims.x - inset).max(0.0), 0.0, 0.0),
            BlockEditHandle::PosY => entity_pos + Vec3::new(0.0, (half_dims.y - inset).max(0.0), 0.0),
            BlockEditHandle::NegY => entity_pos + Vec3::new(0.0, -(half_dims.y - inset).max(0.0), 0.0),
            BlockEditHandle::PosZ => entity_pos + Vec3::new(0.0, 0.0, (half_dims.z - inset).max(0.0)),
            BlockEditHandle::NegZ => entity_pos + Vec3::new(0.0, 0.0, -(half_dims.z - inset).max(0.0)),
        };

        handle_transform.translation = handle_center;
        *visibility = Visibility::Visible;

        // Update material based on state
        let new_material = if block_edit.drag_handle == Some(handle) {
            meshes.dragging_material.clone()
        } else if block_edit.hovered_handle == Some(handle) {
            meshes.hovered_material.clone()
        } else {
            meshes.normal_material.clone()
        };

        material.0 = new_material;
    }
}

/// Draw wireframe bounds using gizmos (these will also render on top via SelectionGizmoGroup)
pub fn draw_block_edit_bounds(
    mut gizmos: Gizmos,
    block_edit: Res<BlockEditState>,
    gizmo_state: Res<GizmoState>,
    entity_query: Query<(&Transform, Option<&BrushData>, Option<&MeshNodeData>)>,
) {
    // Active in Transform mode or BlockEdit mode
    let valid_mode = gizmo_state.tool == EditorTool::Transform || gizmo_state.tool == EditorTool::BlockEdit;
    if !valid_mode || !block_edit.active {
        return;
    }

    let Some(entity) = block_edit.entity else { return };
    let Ok((transform, brush_data, mesh_data)) = entity_query.get(entity) else { return };

    let Some(dimensions) = get_entity_dimensions(brush_data, transform, mesh_data) else { return };

    let half_dims = dimensions / 2.0;
    let entity_pos = transform.translation;

    // Draw wireframe bounds
    let corners = [
        entity_pos + Vec3::new(-half_dims.x, -half_dims.y, -half_dims.z),
        entity_pos + Vec3::new(half_dims.x, -half_dims.y, -half_dims.z),
        entity_pos + Vec3::new(half_dims.x, -half_dims.y, half_dims.z),
        entity_pos + Vec3::new(-half_dims.x, -half_dims.y, half_dims.z),
        entity_pos + Vec3::new(-half_dims.x, half_dims.y, -half_dims.z),
        entity_pos + Vec3::new(half_dims.x, half_dims.y, -half_dims.z),
        entity_pos + Vec3::new(half_dims.x, half_dims.y, half_dims.z),
        entity_pos + Vec3::new(-half_dims.x, half_dims.y, half_dims.z),
    ];

    let wire_color = Color::srgba(0.9, 0.3, 0.3, 0.6);

    // Bottom edges
    gizmos.line(corners[0], corners[1], wire_color);
    gizmos.line(corners[1], corners[2], wire_color);
    gizmos.line(corners[2], corners[3], wire_color);
    gizmos.line(corners[3], corners[0], wire_color);

    // Top edges
    gizmos.line(corners[4], corners[5], wire_color);
    gizmos.line(corners[5], corners[6], wire_color);
    gizmos.line(corners[6], corners[7], wire_color);
    gizmos.line(corners[7], corners[4], wire_color);

    // Vertical edges
    gizmos.line(corners[0], corners[4], wire_color);
    gizmos.line(corners[1], corners[5], wire_color);
    gizmos.line(corners[2], corners[6], wire_color);
    gizmos.line(corners[3], corners[7], wire_color);
}

/// Ray-sphere intersection test
fn ray_sphere_intersection(ray_origin: Vec3, ray_dir: Vec3, sphere_center: Vec3, sphere_radius: f32) -> Option<f32> {
    let oc = ray_origin - sphere_center;
    let a = ray_dir.dot(ray_dir);
    let b = 2.0 * oc.dot(ray_dir);
    let c = oc.dot(oc) - sphere_radius * sphere_radius;
    let discriminant = b * b - 4.0 * a * c;

    if discriminant < 0.0 {
        return None;
    }

    let sqrt_discriminant = discriminant.sqrt();
    let t1 = (-b - sqrt_discriminant) / (2.0 * a);
    let t2 = (-b + sqrt_discriminant) / (2.0 * a);

    if t1 > 0.0 {
        Some(t1)
    } else if t2 > 0.0 {
        Some(t2)
    } else {
        None
    }
}
