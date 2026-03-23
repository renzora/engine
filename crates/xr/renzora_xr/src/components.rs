//! VR component data types
//!
//! These are the data-only types used by the VR system. Registration with the
//! component registry happens in the main crate's component_system module.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// VR Controller data component — attached to entities representing VR controllers.
/// Used for visualization and interaction configuration.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct VrControllerData {
    /// Which hand this controller represents ("left" or "right")
    pub hand: String,
    /// Show a laser pointer ray from the controller
    pub show_laser: bool,
    /// Laser color
    pub laser_color: [f32; 4],
    /// Laser length in meters
    pub laser_length: f32,
    /// Show controller mesh model
    pub show_model: bool,
}

impl Default for VrControllerData {
    fn default() -> Self {
        Self {
            hand: "right".to_string(),
            show_laser: true,
            laser_color: [0.2, 0.6, 1.0, 0.8],
            laser_length: 5.0,
            show_model: true,
        }
    }
}

/// Teleport area component — marks a surface as valid for teleport locomotion.
/// Attach to floor/ground entities to allow players to teleport to them.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct TeleportAreaData {
    /// Whether this teleport area is active
    pub enabled: bool,
    /// Visual indicator color when player aims at this surface
    pub indicator_color: [f32; 4],
    /// Restrict teleport to the surface bounds (vs. any point on the collider)
    pub restrict_to_bounds: bool,
}

impl Default for TeleportAreaData {
    fn default() -> Self {
        Self {
            enabled: true,
            indicator_color: [0.0, 0.8, 0.4, 0.6],
            restrict_to_bounds: false,
        }
    }
}

/// Grab type for VR grabbable objects
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum GrabType {
    /// Object snaps to hand position and rotation
    Snap,
    /// Object maintains relative offset from grab point
    Offset,
    /// Object can be grabbed from a distance (force grab)
    Distance,
}

/// VR Grabbable component — marks an entity as grabbable by VR controllers.
/// Requires a physics collider for grab detection.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct VrGrabbableData {
    /// How the object attaches to the hand
    pub grab_type: GrabType,
    /// Whether the object can be thrown
    pub throwable: bool,
    /// Force multiplier for throw velocity
    pub force_multiplier: f32,
    /// Maximum grab distance (for Distance grab type)
    pub max_grab_distance: f32,
    /// Highlight color when in grab range
    pub highlight_color: [f32; 4],
}

impl Default for VrGrabbableData {
    fn default() -> Self {
        Self {
            grab_type: GrabType::Offset,
            throwable: true,
            force_multiplier: 1.5,
            max_grab_distance: 5.0,
            highlight_color: [0.4, 0.8, 1.0, 0.3],
        }
    }
}

// ─── New VR Components ───

/// VR hand/controller model rendering component
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct VrHandModelData {
    /// Which hand ("left" or "right")
    pub hand: String,
    /// Model type: "controller", "hand", or "custom"
    pub model_type: String,
    /// Asset path for custom model (when model_type = "custom")
    pub custom_mesh: String,
    /// Whether the model is visible
    pub visible: bool,
}

impl Default for VrHandModelData {
    fn default() -> Self {
        Self {
            hand: "right".to_string(),
            model_type: "controller".to_string(),
            custom_mesh: String::new(),
            visible: true,
        }
    }
}

/// VR laser pointer / ray component
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct VrPointerData {
    /// Which hand ("left" or "right")
    pub hand: String,
    /// Whether the pointer is active
    pub enabled: bool,
    /// Ray length in meters
    pub ray_length: f32,
    /// Ray color (RGBA)
    pub ray_color: [f32; 4],
    /// Ray width in meters
    pub ray_width: f32,
    /// Show dot cursor at hit point
    pub show_cursor: bool,
    /// Cursor dot size in meters
    pub cursor_size: f32,
    /// Collision layer mask for raycast
    pub interact_layers: u32,
}

impl Default for VrPointerData {
    fn default() -> Self {
        Self {
            hand: "right".to_string(),
            enabled: true,
            ray_length: 10.0,
            ray_color: [1.0, 1.0, 1.0, 0.5],
            ray_width: 0.002,
            show_cursor: true,
            cursor_size: 0.02,
            interact_layers: u32::MAX,
        }
    }
}

/// Snap zone for VR grabbable objects
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct VrSnapZoneData {
    /// Snap radius in meters
    pub snap_radius: f32,
    /// Highlight when a compatible grabbable is nearby
    pub highlight_when_near: bool,
    /// Highlight color (RGBA)
    pub highlight_color: [f32; 4],
    /// Only accept grabbables with matching tags (empty = accept all)
    pub accepted_tags: Vec<String>,
    /// Whether a grabbable is currently snapped (read-only)
    pub occupied: bool,
}

impl Default for VrSnapZoneData {
    fn default() -> Self {
        Self {
            snap_radius: 0.1,
            highlight_when_near: true,
            highlight_color: [0.2, 1.0, 0.4, 0.4],
            accepted_tags: Vec::new(),
            occupied: false,
        }
    }
}

/// Climbable surface component for VR
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct VrClimbableData {
    /// Whether climbing is enabled on this surface
    pub enabled: bool,
    /// How close hand must be to grab (meters)
    pub grip_distance: f32,
    /// Surface normal direction
    pub surface_normal: [f32; 3],
}

impl Default for VrClimbableData {
    fn default() -> Self {
        Self {
            enabled: true,
            grip_distance: 0.1,
            surface_normal: [0.0, 0.0, 1.0],
        }
    }
}

/// Persistent spatial anchor component
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct VrSpatialAnchorData {
    /// Persistent anchor identifier
    pub anchor_id: String,
    /// Save anchor across sessions
    pub persist_across_sessions: bool,
    /// Current status: "unanchored", "anchoring", "anchored", "lost"
    pub anchor_status: String,
}

impl Default for VrSpatialAnchorData {
    fn default() -> Self {
        Self {
            anchor_id: String::new(),
            persist_across_sessions: false,
            anchor_status: "unanchored".to_string(),
        }
    }
}

/// World-space interactive UI panel for VR
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct VrOverlayPanelData {
    /// World-space width in meters
    pub width: f32,
    /// Pixels per meter (resolution)
    pub pixels_per_meter: f32,
    /// Billboard toward head
    pub follow_head: bool,
    /// Cylindrical curvature
    pub curved: bool,
    /// Curvature radius in meters
    pub curvature_radius: f32,
    /// Responds to VR pointer
    pub interactive: bool,
}

impl Default for VrOverlayPanelData {
    fn default() -> Self {
        Self {
            width: 0.5,
            pixels_per_meter: 1000.0,
            follow_head: false,
            curved: false,
            curvature_radius: 1.5,
            interactive: true,
        }
    }
}

/// Generic tracked device component (Vive Tracker, etc.)
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct VrTrackedObjectData {
    /// Tracker role: "left_foot", "right_foot", "waist", "chest", etc.
    pub tracker_role: String,
    /// Specific device serial number (empty = any matching role)
    pub serial_number: String,
    /// Whether the device is currently tracked (read-only)
    pub tracked: bool,
}

impl Default for VrTrackedObjectData {
    fn default() -> Self {
        Self {
            tracker_role: "waist".to_string(),
            serial_number: String::new(),
            tracked: false,
        }
    }
}

/// Passthrough geometry window for mixed reality
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct VrPassthroughWindowData {
    /// Whether the passthrough window is active
    pub enabled: bool,
    /// Opacity (0.0 - 1.0)
    pub opacity: f32,
    /// Edge color (RGBA, transparent = no edge)
    pub edge_color: [f32; 4],
}

impl Default for VrPassthroughWindowData {
    fn default() -> Self {
        Self {
            enabled: true,
            opacity: 1.0,
            edge_color: [0.0, 0.0, 0.0, 0.0],
        }
    }
}
