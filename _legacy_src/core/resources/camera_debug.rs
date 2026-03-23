//! Camera debug state resource for camera inspection

use bevy::prelude::*;
use bevy::prelude::ClearColorConfig;

/// Camera projection type classification
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CameraProjectionType {
    #[default]
    Perspective,
    Orthographic,
}

impl std::fmt::Display for CameraProjectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CameraProjectionType::Perspective => write!(f, "Perspective"),
            CameraProjectionType::Orthographic => write!(f, "Orthographic"),
        }
    }
}

/// Information about a single camera in the scene
#[derive(Clone, Debug)]
pub struct CameraInfo {
    /// Camera entity
    pub entity: Entity,
    /// Camera name (from Name component if available)
    pub name: String,
    /// Whether this camera is active
    pub is_active: bool,
    /// Camera render order
    pub order: isize,
    /// Projection type
    pub projection_type: CameraProjectionType,
    /// Field of view in degrees (for perspective)
    pub fov_degrees: Option<f32>,
    /// Near clipping plane
    pub near: f32,
    /// Far clipping plane
    pub far: f32,
    /// Aspect ratio
    pub aspect_ratio: f32,
    /// Orthographic scale (for orthographic)
    pub ortho_scale: Option<f32>,
    /// Camera position
    pub position: Vec3,
    /// Camera rotation (euler angles in degrees)
    pub rotation_degrees: Vec3,
    /// Forward vector
    pub forward: Vec3,
    /// Clear color
    pub clear_color: Option<Color>,
    /// Viewport rect (if custom)
    pub viewport: Option<[f32; 4]>,
    /// Whether this is an editor camera (not part of the scene)
    pub is_editor_camera: bool,
}

impl Default for CameraInfo {
    fn default() -> Self {
        Self {
            entity: Entity::PLACEHOLDER,
            name: "Camera".to_string(),
            is_active: true,
            order: 0,
            projection_type: CameraProjectionType::Perspective,
            fov_degrees: Some(45.0),
            near: 0.1,
            far: 1000.0,
            aspect_ratio: 16.0 / 9.0,
            ortho_scale: None,
            position: Vec3::ZERO,
            rotation_degrees: Vec3::ZERO,
            forward: -Vec3::Z,
            clear_color: None,
            viewport: None,
            is_editor_camera: false,
        }
    }
}

/// Camera debug state for monitoring cameras in the scene
#[derive(Resource)]
pub struct CameraDebugState {
    /// List of all cameras in the scene
    pub cameras: Vec<CameraInfo>,
    /// Currently selected camera for detailed view
    pub selected_camera: Option<Entity>,
    /// Whether to show frustum gizmos for selected camera
    pub show_frustum_gizmos: bool,
    /// Whether to show camera axes gizmos
    pub show_camera_axes: bool,
    /// Whether to show all camera frustums (not just selected)
    pub show_all_frustums: bool,
    /// Frustum gizmo color
    pub frustum_color: Color,
    /// Update interval in seconds
    pub update_interval: f32,
    /// Time since last update
    pub time_since_update: f32,
}

impl Default for CameraDebugState {
    fn default() -> Self {
        Self {
            cameras: Vec::new(),
            selected_camera: None,
            show_frustum_gizmos: false,
            show_camera_axes: false,
            show_all_frustums: false,
            frustum_color: Color::srgba(1.0, 1.0, 0.0, 0.5),
            update_interval: 0.2,
            time_since_update: 0.0,
        }
    }
}

impl CameraDebugState {
    /// Get the selected camera info
    pub fn selected_camera_info(&self) -> Option<&CameraInfo> {
        self.selected_camera.and_then(|entity| {
            self.cameras.iter().find(|c| c.entity == entity)
        })
    }

    /// Get count of scene cameras (non-editor cameras)
    pub fn scene_camera_count(&self) -> usize {
        self.cameras.iter().filter(|c| !c.is_editor_camera).count()
    }
}

/// System to update camera debug state
pub fn update_camera_debug_state(
    mut state: ResMut<CameraDebugState>,
    time: Res<Time>,
    cameras_3d: Query<(
        Entity,
        &Camera,
        &GlobalTransform,
        Option<&Name>,
        Option<&Projection>,
    ), With<Camera3d>>,
    cameras_2d: Query<(
        Entity,
        &Camera,
        &GlobalTransform,
        Option<&Name>,
    ), (With<Camera2d>, Without<Camera3d>)>,
    editor_cameras: Query<Entity, With<crate::core::EditorEntity>>,
) {
    state.time_since_update += time.delta_secs();

    if state.time_since_update < state.update_interval {
        return;
    }
    state.time_since_update = 0.0;

    state.cameras.clear();

    // Collect 3D cameras
    for (entity, camera, transform, name, projection) in cameras_3d.iter() {
        let is_editor = editor_cameras.contains(entity);

        let (projection_type, fov, near, far, ortho_scale) = if let Some(proj) = projection {
            match proj {
                Projection::Perspective(p) => (
                    CameraProjectionType::Perspective,
                    Some(p.fov.to_degrees()),
                    p.near,
                    p.far,
                    None,
                ),
                Projection::Orthographic(o) => (
                    CameraProjectionType::Orthographic,
                    None,
                    o.near,
                    o.far,
                    Some(o.scale),
                ),
                _ => (CameraProjectionType::Perspective, Some(45.0), 0.1, 1000.0, None),
            }
        } else {
            (CameraProjectionType::Perspective, Some(45.0), 0.1, 1000.0, None)
        };

        let (_, rotation, translation) = transform.to_scale_rotation_translation();
        let euler = rotation.to_euler(EulerRot::YXZ);
        let rotation_degrees = Vec3::new(
            euler.1.to_degrees(),
            euler.0.to_degrees(),
            euler.2.to_degrees(),
        );

        let clear_color = match &camera.clear_color {
            ClearColorConfig::Default => None,
            ClearColorConfig::Custom(color) => Some(color.clone()),
            ClearColorConfig::None => None,
        };

        let viewport = camera.viewport.as_ref().map(|v| {
            [
                v.physical_position.x as f32,
                v.physical_position.y as f32,
                v.physical_size.x as f32,
                v.physical_size.y as f32,
            ]
        });

        state.cameras.push(CameraInfo {
            entity,
            name: name.map(|n| n.as_str().to_string()).unwrap_or_else(|| format!("Camera {:?}", entity)),
            is_active: camera.is_active,
            order: camera.order,
            projection_type,
            fov_degrees: fov,
            near,
            far,
            aspect_ratio: 16.0 / 9.0, // Approximate
            ortho_scale: ortho_scale,
            position: translation,
            rotation_degrees,
            forward: transform.forward().as_vec3(),
            clear_color,
            viewport,
            is_editor_camera: is_editor,
        });
    }

    // Collect 2D cameras
    for (entity, camera, transform, name) in cameras_2d.iter() {
        let is_editor = editor_cameras.contains(entity);

        // 2D cameras use default orthographic projection values
        let (near, far, ortho_scale) = (-1000.0, 1000.0, Some(1.0));

        let (_, rotation, translation) = transform.to_scale_rotation_translation();
        let euler = rotation.to_euler(EulerRot::YXZ);
        let rotation_degrees = Vec3::new(
            euler.1.to_degrees(),
            euler.0.to_degrees(),
            euler.2.to_degrees(),
        );

        let clear_color = match &camera.clear_color {
            ClearColorConfig::Default => None,
            ClearColorConfig::Custom(color) => Some(color.clone()),
            ClearColorConfig::None => None,
        };

        state.cameras.push(CameraInfo {
            entity,
            name: name.map(|n| n.as_str().to_string()).unwrap_or_else(|| format!("Camera2D {:?}", entity)),
            is_active: camera.is_active,
            order: camera.order,
            projection_type: CameraProjectionType::Orthographic,
            fov_degrees: None,
            near,
            far,
            aspect_ratio: 16.0 / 9.0,
            ortho_scale,
            position: translation,
            rotation_degrees,
            forward: transform.forward().as_vec3(),
            clear_color,
            viewport: None,
            is_editor_camera: is_editor,
        });
    }

    // Sort by order
    state.cameras.sort_by(|a, b| a.order.cmp(&b.order));

    // Validate selected camera still exists
    if let Some(selected) = state.selected_camera {
        if !state.cameras.iter().any(|c| c.entity == selected) {
            state.selected_camera = None;
        }
    }
}
