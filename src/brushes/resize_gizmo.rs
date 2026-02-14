//! Block resize handles for editing brush/mesh dimensions
//!
//! Provides corner-based resize handles that appear when a block-like mesh is selected
//! in Transform mode. Works with BrushData entities and regular Cube/Plane meshes.

use bevy::prelude::*;

use crate::core::{SelectionState, ViewportState};
use crate::gizmo::{EditorTool, GizmoState};
use crate::component_system::{MeshNodeData, MeshPrimitiveType};

use super::{BrushData, BrushSettings};

/// Which corner handle is being interacted with
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockEditHandle {
    /// (+X, +Y, +Z)
    PosXPosYPosZ,
    /// (-X, +Y, +Z)
    NegXPosYPosZ,
    /// (+X, -Y, +Z)
    PosXNegYPosZ,
    /// (-X, -Y, +Z)
    NegXNegYPosZ,
    /// (+X, +Y, -Z)
    PosXPosYNegZ,
    /// (-X, +Y, -Z)
    NegXPosYNegZ,
    /// (+X, -Y, -Z)
    PosXNegYNegZ,
    /// (-X, -Y, -Z)
    NegXNegYNegZ,
}

impl BlockEditHandle {
    /// Get the sign vector for this corner: each component is +1 or -1
    pub fn signs(&self) -> Vec3 {
        match self {
            BlockEditHandle::PosXPosYPosZ => Vec3::new(1.0, 1.0, 1.0),
            BlockEditHandle::NegXPosYPosZ => Vec3::new(-1.0, 1.0, 1.0),
            BlockEditHandle::PosXNegYPosZ => Vec3::new(1.0, -1.0, 1.0),
            BlockEditHandle::NegXNegYPosZ => Vec3::new(-1.0, -1.0, 1.0),
            BlockEditHandle::PosXPosYNegZ => Vec3::new(1.0, 1.0, -1.0),
            BlockEditHandle::NegXPosYNegZ => Vec3::new(-1.0, 1.0, -1.0),
            BlockEditHandle::PosXNegYNegZ => Vec3::new(1.0, -1.0, -1.0),
            BlockEditHandle::NegXNegYNegZ => Vec3::new(-1.0, -1.0, -1.0),
        }
    }

    /// Get the opposite corner (all signs flipped)
    pub fn opposite(&self) -> BlockEditHandle {
        match self {
            BlockEditHandle::PosXPosYPosZ => BlockEditHandle::NegXNegYNegZ,
            BlockEditHandle::NegXPosYPosZ => BlockEditHandle::PosXNegYNegZ,
            BlockEditHandle::PosXNegYPosZ => BlockEditHandle::NegXPosYNegZ,
            BlockEditHandle::NegXNegYPosZ => BlockEditHandle::PosXPosYNegZ,
            BlockEditHandle::PosXPosYNegZ => BlockEditHandle::NegXNegYPosZ,
            BlockEditHandle::NegXPosYNegZ => BlockEditHandle::PosXNegYPosZ,
            BlockEditHandle::PosXNegYNegZ => BlockEditHandle::NegXPosYPosZ,
            BlockEditHandle::NegXNegYNegZ => BlockEditHandle::PosXPosYPosZ,
        }
    }

    /// All 8 corner handles
    pub fn all() -> &'static [BlockEditHandle] {
        &[
            BlockEditHandle::PosXPosYPosZ,
            BlockEditHandle::NegXPosYPosZ,
            BlockEditHandle::PosXNegYPosZ,
            BlockEditHandle::NegXNegYPosZ,
            BlockEditHandle::PosXPosYNegZ,
            BlockEditHandle::NegXPosYNegZ,
            BlockEditHandle::PosXNegYNegZ,
            BlockEditHandle::NegXNegYNegZ,
        ]
    }

    /// Get the world position of this corner
    fn position(&self, center: Vec3, half_dims: Vec3) -> Vec3 {
        center + self.signs() * half_dims
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
    /// Starting box center when drag began
    pub drag_start_position: Vec3,
    /// Starting mouse position when drag began
    pub drag_start_mouse: Vec2,
    /// The fixed opposite corner position at drag start
    pub drag_anchor_corner: Vec3,
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

    /// Start dragging a corner handle
    pub fn start_drag(&mut self, handle: BlockEditHandle, dimensions: Vec3, center: Vec3, mouse: Vec2) {
        self.is_dragging = true;
        self.drag_handle = Some(handle);
        self.drag_start_dimensions = dimensions;
        self.drag_start_position = center;
        self.drag_start_mouse = mouse;
        // The opposite corner stays fixed during single-side resize
        let half = dimensions / 2.0;
        self.drag_anchor_corner = handle.opposite().position(center, half);
    }

    /// End the drag operation
    pub fn end_drag(&mut self) {
        self.is_dragging = false;
        self.drag_handle = None;
    }
}

/// Fraction of each edge length used for corner bracket arms
const BRACKET_ARM_FRACTION: f32 = 0.22;

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

    if is_block_like_mesh(mesh_data) {
        let scale = transform.scale;
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

    // Test each corner handle for intersection
    // Hit radius scales with the shortest bracket arm for consistent feel
    let arm_lengths = dimensions * BRACKET_ARM_FRACTION;
    let handle_radius = arm_lengths.min_element().max(0.15);
    let half_dims = dimensions / 2.0;
    let entity_pos = transform.translation;

    let mut closest_handle = None;
    let mut closest_dist = f32::MAX;

    for handle in BlockEditHandle::all() {
        let corner_pos = handle.position(entity_pos, half_dims);

        if let Some(t) = ray_sphere_intersection(ray.origin, *ray.direction, corner_pos, handle_radius) {
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
            entity_pos,
            cursor_pos,
        );
    }
}

/// System to handle block edit corner dragging
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

    // Use a camera-facing plane through the dragged corner's original position
    let half = block_edit.drag_start_dimensions / 2.0;
    let drag_corner_start = handle.position(block_edit.drag_start_position, half);
    let plane_normal = Vec3::from(camera_transform.forward());
    let plane_point = drag_corner_start;

    let denom = plane_normal.dot(*ray.direction);
    if denom.abs() < 0.0001 {
        return;
    }

    let t = (plane_point - ray.origin).dot(plane_normal) / denom;
    if t < 0.0 {
        return;
    }

    let hit_point = ray.origin + *ray.direction * t;

    // New corner position is where the ray hits the plane
    let mut new_corner = hit_point;

    // Snap each axis of the corner if enabled
    if brush_settings.snap_enabled {
        new_corner.x = brush_settings.snap(new_corner.x);
        new_corner.y = brush_settings.snap(new_corner.y);
        new_corner.z = brush_settings.snap(new_corner.z);
    }

    let (new_dims, new_pos);

    if block_edit.symmetric {
        // Symmetric: center stays fixed, corner moves freely
        let center = block_edit.drag_start_position;
        let delta = new_corner - center;
        // Dimensions = 2 * abs(delta from center), minimum 0.1 per axis
        new_dims = Vec3::new(
            (delta.x.abs() * 2.0).max(0.1),
            (delta.y.abs() * 2.0).max(0.1),
            (delta.z.abs() * 2.0).max(0.1),
        );
        new_pos = center;
    } else {
        // Single-side: opposite corner stays fixed
        let anchor = block_edit.drag_anchor_corner;
        new_dims = Vec3::new(
            (new_corner.x - anchor.x).abs().max(0.1),
            (new_corner.y - anchor.y).abs().max(0.1),
            (new_corner.z - anchor.z).abs().max(0.1),
        );
        new_pos = (new_corner + anchor) / 2.0;
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

/// Draw wireframe bounds and corner bracket resize handles using gizmos
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

    let wire_color = Color::srgba(1.0, 0.5, 0.0, 0.4);

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

    // Draw inner corner bracket handles at each corner
    let normal_color = Color::srgb(0.95, 0.95, 0.95);
    let hovered_color = Color::srgb(1.0, 0.5, 0.3);
    let dragging_color = Color::srgb(1.0, 1.0, 0.3);

    for handle in BlockEditHandle::all() {
        let signs = handle.signs();
        let corner = entity_pos + signs * half_dims;

        let color = if block_edit.drag_handle == Some(*handle) {
            dragging_color
        } else if block_edit.hovered_handle == Some(*handle) {
            hovered_color
        } else {
            normal_color
        };

        // Arm lengths proportional to each edge, arms extend inward from corner
        let arm_x = dimensions.x * BRACKET_ARM_FRACTION;
        let arm_y = dimensions.y * BRACKET_ARM_FRACTION;
        let arm_z = dimensions.z * BRACKET_ARM_FRACTION;

        gizmos.line(corner, corner + Vec3::new(-signs.x * arm_x, 0.0, 0.0), color);
        gizmos.line(corner, corner + Vec3::new(0.0, -signs.y * arm_y, 0.0), color);
        gizmos.line(corner, corner + Vec3::new(0.0, 0.0, -signs.z * arm_z), color);
    }
}

/// Ray-sphere intersection test (used for corner handle hit detection)
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
