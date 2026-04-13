//! Viewport state types — shared between editor plugins via renzora_core.
//!
//! Moved here from `renzora_viewport` so that camera, gizmo, and other
//! editor plugin DLLs can use these types without depending on each other.

use std::sync::atomic::{AtomicBool, AtomicI32};

use bevy::prelude::*;

const DEFAULT_WIDTH: u32 = 1280;
const DEFAULT_HEIGHT: u32 = 720;

/// Tracks the render target image and current resolution.
#[derive(Resource)]
pub struct ViewportState {
    pub image_handle: Option<Handle<Image>>,
    pub current_size: UVec2,
    /// Whether the mouse cursor is currently over the viewport.
    pub hovered: bool,
    /// Screen-space position of the viewport panel (top-left corner).
    pub screen_position: Vec2,
    /// Screen-space size of the viewport panel.
    pub screen_size: Vec2,
}

impl Default for ViewportState {
    fn default() -> Self {
        Self {
            image_handle: None,
            current_size: UVec2::new(DEFAULT_WIDTH, DEFAULT_HEIGHT),
            hovered: false,
            screen_position: Vec2::ZERO,
            screen_size: Vec2::new(DEFAULT_WIDTH as f32, DEFAULT_HEIGHT as f32),
        }
    }
}

/// Atomically-writable nav overlay drag state from the panel's `ui()` method.
///
/// The nav overlay buttons write drag deltas here (from `&World`), and the
/// camera controller system reads + consumes them each frame.
#[derive(Resource)]
pub struct NavOverlayState {
    /// Whether the pan button is currently being dragged.
    pub pan_dragging: AtomicBool,
    /// Whether the zoom button is currently being dragged.
    pub zoom_dragging: AtomicBool,
    /// Pan drag delta X (scaled by 1000 to preserve fractional part).
    pub pan_delta_x: AtomicI32,
    /// Pan drag delta Y (scaled by 1000 to preserve fractional part).
    pub pan_delta_y: AtomicI32,
    /// Zoom drag delta Y (scaled by 1000 to preserve fractional part).
    pub zoom_delta_y: AtomicI32,
}

impl Default for NavOverlayState {
    fn default() -> Self {
        Self {
            pan_dragging: AtomicBool::new(false),
            zoom_dragging: AtomicBool::new(false),
            pan_delta_x: AtomicI32::new(0),
            pan_delta_y: AtomicI32::new(0),
            zoom_delta_y: AtomicI32::new(0),
        }
    }
}

/// Camera orbit orientation, written by the camera system and read by the axis gizmo overlay.
#[derive(Resource, Debug, Clone, Default)]
pub struct CameraOrbitSnapshot {
    pub yaw: f32,
    pub pitch: f32,
}

/// Cached clip-from-world matrix of the editor camera, plus camera world position.
/// Updated every frame. Used by CPU-projected viewport overlays (grid, gizmos).
#[derive(Resource, Debug, Clone)]
pub struct EditorCameraMatrix {
    pub clip_from_world: Mat4,
    pub world_from_clip: Mat4,
    pub cam_pos: Vec3,
    pub cam_forward: Vec3,
    pub valid: bool,
}

impl Default for EditorCameraMatrix {
    fn default() -> Self {
        Self {
            clip_from_world: Mat4::IDENTITY,
            world_from_clip: Mat4::IDENTITY,
            cam_pos: Vec3::ZERO,
            cam_forward: Vec3::NEG_Z,
            valid: false,
        }
    }
}

/// Camera projection mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProjectionMode {
    #[default]
    Perspective,
    Orthographic,
}

/// High-level viewport interaction mode (Blender-style mode switcher).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewportMode {
    #[default]
    Scene,
    Edit,
    Sculpt,
    Paint,
    Animate,
}

impl ViewportMode {
    pub const ALL: &'static [ViewportMode] = &[
        Self::Scene, Self::Edit, Self::Sculpt, Self::Paint, Self::Animate,
    ];
    pub fn label(&self) -> &'static str {
        match self {
            Self::Scene => "Scene",
            Self::Edit => "Edit",
            Self::Sculpt => "Sculpt",
            Self::Paint => "Paint",
            Self::Animate => "Animate",
        }
    }
}

/// Visualization mode for debug rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VisualizationMode {
    #[default]
    None,
    Normals,
    Roughness,
    Metallic,
    Depth,
    UvChecker,
}

impl VisualizationMode {
    pub const ALL: &'static [VisualizationMode] = &[
        Self::None,
        Self::Normals,
        Self::Roughness,
        Self::Metallic,
        Self::Depth,
        Self::UvChecker,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Normals => "Normals",
            Self::Roughness => "Roughness",
            Self::Metallic => "Metallic",
            Self::Depth => "Depth",
            Self::UvChecker => "UV Checker",
        }
    }
}

/// Render feature toggles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderToggles {
    pub textures: bool,
    pub wireframe: bool,
    pub lighting: bool,
    pub shadows: bool,
}

impl Default for RenderToggles {
    fn default() -> Self {
        Self {
            textures: true,
            wireframe: false,
            lighting: true,
            shadows: true,
        }
    }
}

/// Collision gizmo visibility mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CollisionGizmoVisibility {
    #[default]
    SelectedOnly,
    Always,
}

/// Snapping settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapSettings {
    pub translate_enabled: bool,
    pub translate_snap: f32,
    /// If true, snap the entity's world-space AABB min corner to the grid
    /// instead of its pivot. Aligns cube edges to gridlines.
    pub translate_edge_snap: bool,
    pub rotate_enabled: bool,
    pub rotate_snap: f32,
    pub scale_enabled: bool,
    pub scale_snap: f32,
    /// If true, Y-axis scaling keeps the entity's world-space AABB bottom
    /// fixed (scales upward from the floor instead of symmetrically).
    pub scale_bottom_anchor: bool,
    pub object_snap_enabled: bool,
    pub object_snap_distance: f32,
    pub floor_snap_enabled: bool,
    pub floor_y: f32,
}

impl Default for SnapSettings {
    fn default() -> Self {
        Self {
            translate_enabled: false,
            translate_snap: 1.0,
            translate_edge_snap: true,
            rotate_enabled: false,
            rotate_snap: 15.0,
            scale_enabled: false,
            scale_snap: 0.25,
            scale_bottom_anchor: true,
            object_snap_enabled: true,
            object_snap_distance: 0.5,
            floor_snap_enabled: true,
            floor_y: 0.0,
        }
    }
}

/// Camera sensitivity settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraSettingsState {
    pub move_speed: f32,
    pub look_sensitivity: f32,
    pub orbit_sensitivity: f32,
    pub pan_sensitivity: f32,
    pub zoom_sensitivity: f32,
    pub invert_y: bool,
    pub distance_relative_speed: bool,
}

impl Default for CameraSettingsState {
    fn default() -> Self {
        Self {
            move_speed: 10.0,
            look_sensitivity: 0.3,
            orbit_sensitivity: 0.5,
            pan_sensitivity: 1.0,
            zoom_sensitivity: 1.0,
            invert_y: false,
            distance_relative_speed: true,
        }
    }
}

/// A pending view angle command.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewAngleCommand {
    pub yaw: f32,
    pub pitch: f32,
}

/// Viewport overlay and rendering settings.
///
/// This resource is the single source of truth for the viewport header UI.
/// Other crates (camera, gizmo) read from this to apply changes.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct ViewportSettings {
    pub render_toggles: RenderToggles,
    pub visualization_mode: VisualizationMode,
    pub show_grid: bool,
    pub show_subgrid: bool,
    pub show_axis_gizmo: bool,
    pub collision_gizmo_visibility: CollisionGizmoVisibility,
    pub projection_mode: ProjectionMode,
    pub viewport_mode: ViewportMode,
    pub camera: CameraSettingsState,
    pub snap: SnapSettings,
    /// Pending view angle command (consumed by camera system).
    pub pending_view_angle: Option<ViewAngleCommand>,
}

impl Default for ViewportSettings {
    fn default() -> Self {
        Self {
            render_toggles: RenderToggles::default(),
            visualization_mode: VisualizationMode::default(),
            show_grid: true,
            show_subgrid: true,
            show_axis_gizmo: true,
            collision_gizmo_visibility: CollisionGizmoVisibility::default(),
            projection_mode: ProjectionMode::default(),
            viewport_mode: ViewportMode::default(),
            camera: CameraSettingsState::default(),
            snap: SnapSettings::default(),
            pending_view_angle: None,
        }
    }
}
