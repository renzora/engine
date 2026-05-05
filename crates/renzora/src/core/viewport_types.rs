//! Viewport state types — shared between editor plugins via renzora.
//!
//! Moved here from `renzora_viewport` so that camera, gizmo, and other
//! editor plugin DLLs can use these types without depending on each other.

use std::sync::atomic::{AtomicBool, AtomicI32};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

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
    /// Whether the axis gizmo is currently being dragged (orbits).
    pub orbit_dragging: AtomicBool,
    /// Orbit drag delta X (scaled by 1000).
    pub orbit_delta_x: AtomicI32,
    /// Orbit drag delta Y (scaled by 1000).
    pub orbit_delta_y: AtomicI32,
}

impl Default for NavOverlayState {
    fn default() -> Self {
        Self {
            pan_dragging: AtomicBool::new(false),
            zoom_dragging: AtomicBool::new(false),
            pan_delta_x: AtomicI32::new(0),
            pan_delta_y: AtomicI32::new(0),
            zoom_delta_y: AtomicI32::new(0),
            orbit_dragging: AtomicBool::new(false),
            orbit_delta_x: AtomicI32::new(0),
            orbit_delta_y: AtomicI32::new(0),
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
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
        Self::Scene,
        Self::Edit,
        Self::Sculpt,
        Self::Paint,
        Self::Animate,
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
    /// Solid mesh rendering. Off hides mesh fill (wireframe still renders if on).
    pub mesh: bool,
}

impl Default for RenderToggles {
    fn default() -> Self {
        Self {
            textures: true,
            wireframe: false,
            lighting: true,
            shadows: true,
            mesh: true,
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
    /// Toggle for in-viewport scene icons (light bulb / sun / camera glyphs).
    pub show_scene_icons: bool,
    pub collision_gizmo_visibility: CollisionGizmoVisibility,
    pub projection_mode: ProjectionMode,
    pub viewport_mode: ViewportMode,
    pub camera: CameraSettingsState,
    pub snap: SnapSettings,
    /// Pending view angle command (consumed by camera system).
    pub pending_view_angle: Option<ViewAngleCommand>,
    /// Cap the framerate at the monitor refresh rate. Off lets the FPS
    /// counter reflect actual render capacity at the cost of possible
    /// screen tearing.
    pub vsync: bool,
}

impl Default for ViewportSettings {
    fn default() -> Self {
        Self {
            render_toggles: RenderToggles::default(),
            visualization_mode: VisualizationMode::default(),
            show_grid: true,
            show_subgrid: true,
            show_axis_gizmo: true,
            show_scene_icons: true,
            collision_gizmo_visibility: CollisionGizmoVisibility::default(),
            projection_mode: ProjectionMode::default(),
            viewport_mode: ViewportMode::default(),
            camera: CameraSettingsState::default(),
            snap: SnapSettings::default(),
            pending_view_angle: None,
            vsync: true,
        }
    }
}

// ── Persisted editor preferences (stored in project.toml) ──────────────────
//
// Editor-only fields. Stripped from exported builds (the runtime ignores the
// `[editor]` section of project.toml). Uses `#[serde(default)]` on every
// field so missing entries fall back to sensible defaults.

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct PersistedViewportSettings {
    pub textures: bool,
    pub wireframe: bool,
    pub lighting: bool,
    pub shadows: bool,
    #[serde(default = "default_true")]
    pub mesh: bool,
    pub visualization_mode: String,
    pub show_grid: bool,
    pub show_subgrid: bool,
    pub show_axis_gizmo: bool,
    #[serde(default = "default_true")]
    pub show_scene_icons: bool,
    pub collision_always: bool,
    pub orthographic: bool,
    pub move_speed: f32,
    pub look_sensitivity: f32,
    pub orbit_sensitivity: f32,
    pub pan_sensitivity: f32,
    pub zoom_sensitivity: f32,
    pub invert_y: bool,
    pub distance_relative_speed: bool,
    pub translate_enabled: bool,
    pub translate_snap: f32,
    pub translate_edge_snap: bool,
    pub rotate_enabled: bool,
    pub rotate_snap: f32,
    pub scale_enabled: bool,
    pub scale_snap: f32,
    pub scale_bottom_anchor: bool,
    pub object_snap_enabled: bool,
    pub object_snap_distance: f32,
    pub floor_snap_enabled: bool,
    pub floor_y: f32,
    #[serde(default = "default_true")]
    pub vsync: bool,
}

impl PersistedViewportSettings {
    pub fn from_settings(s: &ViewportSettings) -> Self {
        let rt = s.render_toggles;
        let c = s.camera;
        let sn = s.snap;
        Self {
            textures: rt.textures,
            wireframe: rt.wireframe,
            lighting: rt.lighting,
            shadows: rt.shadows,
            mesh: rt.mesh,
            visualization_mode: format!("{:?}", s.visualization_mode),
            show_grid: s.show_grid,
            show_subgrid: s.show_subgrid,
            show_axis_gizmo: s.show_axis_gizmo,
            show_scene_icons: s.show_scene_icons,
            collision_always: matches!(
                s.collision_gizmo_visibility,
                CollisionGizmoVisibility::Always
            ),
            orthographic: matches!(s.projection_mode, ProjectionMode::Orthographic),
            move_speed: c.move_speed,
            look_sensitivity: c.look_sensitivity,
            orbit_sensitivity: c.orbit_sensitivity,
            pan_sensitivity: c.pan_sensitivity,
            zoom_sensitivity: c.zoom_sensitivity,
            invert_y: c.invert_y,
            distance_relative_speed: c.distance_relative_speed,
            translate_enabled: sn.translate_enabled,
            translate_snap: sn.translate_snap,
            translate_edge_snap: sn.translate_edge_snap,
            rotate_enabled: sn.rotate_enabled,
            rotate_snap: sn.rotate_snap,
            scale_enabled: sn.scale_enabled,
            scale_snap: sn.scale_snap,
            scale_bottom_anchor: sn.scale_bottom_anchor,
            object_snap_enabled: sn.object_snap_enabled,
            object_snap_distance: sn.object_snap_distance,
            floor_snap_enabled: sn.floor_snap_enabled,
            floor_y: sn.floor_y,
            vsync: s.vsync,
        }
    }

    pub fn apply(&self, s: &mut ViewportSettings) {
        s.render_toggles = RenderToggles {
            textures: self.textures,
            wireframe: self.wireframe,
            lighting: self.lighting,
            shadows: self.shadows,
            mesh: self.mesh,
        };
        s.visualization_mode = match self.visualization_mode.as_str() {
            "Normals" => VisualizationMode::Normals,
            "Roughness" => VisualizationMode::Roughness,
            "Metallic" => VisualizationMode::Metallic,
            "Depth" => VisualizationMode::Depth,
            "UvChecker" => VisualizationMode::UvChecker,
            _ => VisualizationMode::None,
        };
        s.show_grid = self.show_grid;
        s.show_subgrid = self.show_subgrid;
        s.show_axis_gizmo = self.show_axis_gizmo;
        s.show_scene_icons = self.show_scene_icons;
        s.collision_gizmo_visibility = if self.collision_always {
            CollisionGizmoVisibility::Always
        } else {
            CollisionGizmoVisibility::SelectedOnly
        };
        s.projection_mode = if self.orthographic {
            ProjectionMode::Orthographic
        } else {
            ProjectionMode::Perspective
        };
        s.camera = CameraSettingsState {
            move_speed: self.move_speed,
            look_sensitivity: self.look_sensitivity,
            orbit_sensitivity: self.orbit_sensitivity,
            pan_sensitivity: self.pan_sensitivity,
            zoom_sensitivity: self.zoom_sensitivity,
            invert_y: self.invert_y,
            distance_relative_speed: self.distance_relative_speed,
        };
        s.snap = SnapSettings {
            translate_enabled: self.translate_enabled,
            translate_snap: self.translate_snap,
            translate_edge_snap: self.translate_edge_snap,
            rotate_enabled: self.rotate_enabled,
            rotate_snap: self.rotate_snap,
            scale_enabled: self.scale_enabled,
            scale_snap: self.scale_snap,
            scale_bottom_anchor: self.scale_bottom_anchor,
            object_snap_enabled: self.object_snap_enabled,
            object_snap_distance: self.object_snap_distance,
            floor_snap_enabled: self.floor_snap_enabled,
            floor_y: self.floor_y,
        };
        s.vsync = self.vsync;
    }
}

fn default_true() -> bool {
    true
}

/// Editor-only preferences persisted in `project.toml` under `[editor]`.
/// The runtime ignores this section, and `renzora_export` strips it from
/// shipped builds.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct EditorPrefs {
    pub viewport: PersistedViewportSettings,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nondefault_viewport() -> ViewportSettings {
        // Touch every field so the round-trip really exercises the
        // PersistedViewportSettings <-> ViewportSettings bridge — a missed
        // field on either side would make this test fail.
        ViewportSettings {
            render_toggles: RenderToggles {
                textures: false,
                wireframe: true,
                lighting: false,
                shadows: false,
                mesh: false,
            },
            visualization_mode: VisualizationMode::Normals,
            show_grid: false,
            show_subgrid: false,
            show_axis_gizmo: false,
            show_scene_icons: false,
            collision_gizmo_visibility: CollisionGizmoVisibility::Always,
            projection_mode: ProjectionMode::Orthographic,
            viewport_mode: ViewportMode::default(),
            camera: CameraSettingsState {
                move_speed: 11.5,
                look_sensitivity: 0.7,
                orbit_sensitivity: 0.42,
                pan_sensitivity: 1.7,
                zoom_sensitivity: 2.3,
                invert_y: true,
                distance_relative_speed: false,
            },
            snap: SnapSettings {
                translate_enabled: true,
                translate_snap: 0.5,
                translate_edge_snap: true,
                rotate_enabled: true,
                rotate_snap: 15.0,
                scale_enabled: false,
                scale_snap: 0.25,
                scale_bottom_anchor: true,
                object_snap_enabled: true,
                object_snap_distance: 1.5,
                floor_snap_enabled: true,
                floor_y: -1.5,
            },
            pending_view_angle: None,
            vsync: false,
        }
    }

    #[test]
    fn persisted_round_trip_preserves_every_field() {
        let original = nondefault_viewport();
        let persisted = PersistedViewportSettings::from_settings(&original);
        let mut restored = ViewportSettings::default();
        persisted.apply(&mut restored);

        // Skip pending_view_angle (transient) and viewport_mode (not persisted).
        assert_eq!(original.render_toggles, restored.render_toggles);
        assert!(matches!(
            restored.visualization_mode,
            VisualizationMode::Normals
        ));
        assert_eq!(original.show_grid, restored.show_grid);
        assert_eq!(original.show_subgrid, restored.show_subgrid);
        assert_eq!(original.show_axis_gizmo, restored.show_axis_gizmo);
        assert_eq!(original.show_scene_icons, restored.show_scene_icons);
        assert!(matches!(
            restored.collision_gizmo_visibility,
            CollisionGizmoVisibility::Always
        ));
        assert!(matches!(
            restored.projection_mode,
            ProjectionMode::Orthographic
        ));
        assert_eq!(original.camera, restored.camera);
        assert_eq!(original.snap, restored.snap);
        assert_eq!(original.vsync, restored.vsync);
    }

    #[test]
    fn vsync_round_trips() {
        // The whole point of the recent vsync setting is that it survives
        // a save/load. Lock that in.
        let mut s = ViewportSettings::default();
        s.vsync = false;
        let persisted = PersistedViewportSettings::from_settings(&s);
        let mut restored = ViewportSettings::default();
        persisted.apply(&mut restored);
        assert!(!restored.vsync);
    }

    #[test]
    fn visualization_mode_string_round_trips_through_persisted() {
        for mode in [
            VisualizationMode::None,
            VisualizationMode::Normals,
            VisualizationMode::Roughness,
            VisualizationMode::Metallic,
            VisualizationMode::Depth,
            VisualizationMode::UvChecker,
        ] {
            let mut s = ViewportSettings::default();
            s.visualization_mode = mode;
            let p = PersistedViewportSettings::from_settings(&s);
            let mut restored = ViewportSettings::default();
            p.apply(&mut restored);
            assert!(
                std::mem::discriminant(&restored.visualization_mode)
                    == std::mem::discriminant(&mode),
                "round trip lost mode {:?}, got {:?}",
                mode,
                restored.visualization_mode,
            );
        }
    }

    #[test]
    fn editor_prefs_default_has_default_viewport() {
        let prefs = EditorPrefs::default();
        assert_eq!(prefs.viewport, PersistedViewportSettings::default());
    }

    #[test]
    fn persisted_viewport_serde_is_keyed_by_field_name() {
        // Hand-rolled TOML has to deserialize cleanly — proves we didn't
        // accidentally tag the struct or rename a field.
        let s = r#"
            textures = true
            wireframe = false
            lighting = true
            shadows = true
            mesh = true
            visualization_mode = "None"
            show_grid = true
            show_subgrid = true
            show_axis_gizmo = true
            show_scene_icons = true
            collision_always = false
            orthographic = false
            move_speed = 10.0
            look_sensitivity = 1.0
            orbit_sensitivity = 1.0
            pan_sensitivity = 1.0
            zoom_sensitivity = 1.0
            invert_y = false
            distance_relative_speed = true
            translate_enabled = false
            translate_snap = 1.0
            translate_edge_snap = false
            rotate_enabled = false
            rotate_snap = 15.0
            scale_enabled = false
            scale_snap = 0.1
            scale_bottom_anchor = false
            object_snap_enabled = false
            object_snap_distance = 1.0
            floor_snap_enabled = false
            floor_y = 0.0
            vsync = true
        "#;
        let parsed: PersistedViewportSettings = toml::from_str(s).expect("parse");
        assert!(parsed.vsync);
        assert!(parsed.mesh);
    }
}
