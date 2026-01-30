//! Brush-based level design tools for creating level geometry
//!
//! This module provides tools for quickly creating and editing level geometry
//! with click-drag viewport interaction and resize handles.

mod creation;
mod material;
mod resize_gizmo;

pub use creation::*;
pub use material::*;
pub use resize_gizmo::*;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Types of brush geometry that can be created
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Reflect, Serialize, Deserialize)]
pub enum BrushType {
    /// General purpose box geometry
    #[default]
    Block,
    /// Flat horizontal plane
    Floor,
    /// Thin vertical panel
    Wall,
    /// Stepped geometry
    Stairs,
    /// Angled slope
    Ramp,
}

impl BrushType {
    /// Display name for the brush type
    pub fn display_name(&self) -> &'static str {
        match self {
            BrushType::Block => "Block",
            BrushType::Floor => "Floor",
            BrushType::Wall => "Wall",
            BrushType::Stairs => "Stairs",
            BrushType::Ramp => "Ramp",
        }
    }

    /// Get the default dimensions for this brush type
    pub fn default_dimensions(&self) -> Vec3 {
        match self {
            BrushType::Block => Vec3::new(1.0, 1.0, 1.0),
            BrushType::Floor => Vec3::new(4.0, 0.1, 4.0),
            BrushType::Wall => Vec3::new(4.0, 3.0, 0.1),
            BrushType::Stairs => Vec3::new(2.0, 2.0, 4.0),
            BrushType::Ramp => Vec3::new(2.0, 2.0, 4.0),
        }
    }

    /// Get the default height for this brush type
    pub fn default_height(&self) -> f32 {
        match self {
            BrushType::Block => 1.0,
            BrushType::Floor => 0.1,
            BrushType::Wall => 3.0,
            BrushType::Stairs => 2.0,
            BrushType::Ramp => 2.0,
        }
    }

    /// All brush types
    pub fn all() -> &'static [BrushType] {
        &[
            BrushType::Block,
            BrushType::Floor,
            BrushType::Wall,
            BrushType::Stairs,
            BrushType::Ramp,
        ]
    }
}

/// Data component for brush entities - stores brush-specific information
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct BrushData {
    /// Type of brush
    pub brush_type: BrushType,
    /// Dimensions of the brush (width, height, depth)
    pub dimensions: Vec3,
}

impl Default for BrushData {
    fn default() -> Self {
        Self {
            brush_type: BrushType::Block,
            dimensions: Vec3::new(1.0, 1.0, 1.0),
        }
    }
}

/// Current state of brush creation
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BrushCreationPhase {
    /// Not creating a brush
    #[default]
    Idle,
    /// Click started, waiting for drag
    Started,
    /// Dragging to set XZ size
    DraggingXZ,
    /// Dragging to set height (Shift held)
    DraggingHeight,
}

/// Resource tracking the current brush creation state
#[derive(Resource, Default)]
pub struct BrushState {
    /// Current creation phase
    pub phase: BrushCreationPhase,
    /// Position where brush creation started (world coordinates)
    pub start_position: Vec3,
    /// Current end position during drag (world coordinates)
    pub current_position: Vec3,
    /// Preview entity while creating
    pub preview_entity: Option<Entity>,
    /// The brush type being created
    pub creating_brush_type: BrushType,
}

impl BrushState {
    /// Check if we're currently creating a brush
    pub fn is_creating(&self) -> bool {
        self.phase != BrushCreationPhase::Idle
    }

    /// Start brush creation at a position
    pub fn start(&mut self, position: Vec3, brush_type: BrushType) {
        self.phase = BrushCreationPhase::Started;
        self.start_position = position;
        self.current_position = position;
        self.creating_brush_type = brush_type;
    }

    /// Reset to idle state
    pub fn reset(&mut self) {
        self.phase = BrushCreationPhase::Idle;
        self.preview_entity = None;
    }

    /// Calculate brush dimensions from start and current position
    pub fn calculate_dimensions(&self, default_height: f32) -> Vec3 {
        let delta = self.current_position - self.start_position;
        let width = delta.x.abs().max(0.1);
        let depth = delta.z.abs().max(0.1);
        let height = if self.phase == BrushCreationPhase::DraggingHeight {
            delta.y.abs().max(0.1)
        } else {
            default_height
        };
        Vec3::new(width, height, depth)
    }

    /// Calculate center position for the brush
    pub fn calculate_center(&self, default_height: f32) -> Vec3 {
        let dims = self.calculate_dimensions(default_height);
        let center_x = (self.start_position.x + self.current_position.x) / 2.0;
        let center_z = (self.start_position.z + self.current_position.z) / 2.0;
        // Position brush so bottom is at Y=0 or at start height
        let center_y = self.start_position.y + dims.y / 2.0;
        Vec3::new(center_x, center_y, center_z)
    }
}

/// Resource for brush tool settings
#[derive(Resource)]
pub struct BrushSettings {
    /// Currently selected brush type
    pub selected_brush: BrushType,
    /// Quick size presets (width, depth)
    pub quick_sizes: [(f32, f32); 4],
    /// Custom size for current brush
    pub custom_size: Vec3,
    /// Grid snap enabled
    pub snap_enabled: bool,
    /// Grid snap size
    pub snap_size: f32,
    /// Whether to use the checkerboard material
    pub use_checkerboard: bool,
}

impl Default for BrushSettings {
    fn default() -> Self {
        Self {
            selected_brush: BrushType::Block,
            quick_sizes: [(1.0, 1.0), (2.0, 2.0), (4.0, 4.0), (8.0, 8.0)],
            custom_size: Vec3::new(1.0, 1.0, 1.0),
            snap_enabled: true,
            snap_size: 1.0,
            use_checkerboard: true,
        }
    }
}

impl BrushSettings {
    /// Snap a value to the grid if snapping is enabled
    pub fn snap(&self, value: f32) -> f32 {
        if self.snap_enabled && self.snap_size > 0.0 {
            (value / self.snap_size).round() * self.snap_size
        } else {
            value
        }
    }

    /// Snap a Vec3 to the grid if snapping is enabled
    pub fn snap_vec3(&self, value: Vec3) -> Vec3 {
        Vec3::new(
            self.snap(value.x),
            self.snap(value.y),
            self.snap(value.z),
        )
    }
}

/// Plugin for the brush level design system
pub struct BrushPlugin;

impl Plugin for BrushPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<BrushData>()
            .register_type::<BrushType>()
            .init_resource::<BrushState>()
            .init_resource::<BrushSettings>()
            .init_resource::<BlockEditState>()
            .init_resource::<DefaultBrushMaterial>()
            .add_systems(
                Startup,
                (setup_default_brush_material, setup_resize_handle_meshes),
            )
            .add_systems(
                Update,
                (
                    brush_tool_shortcut_system,
                    block_edit_hover_system,
                    brush_preview_system,
                    brush_creation_start_system,
                    brush_creation_drag_system,
                    block_edit_drag_system,
                    brush_creation_end_system,
                    update_resize_handle_meshes,
                    draw_block_edit_bounds,
                )
                    .chain()
                    .run_if(in_state(crate::core::AppState::Editor)),
            );
    }
}
