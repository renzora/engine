//! UI-related component data types

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Data component for UI panel nodes
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct UIPanelData {
    /// Panel width in pixels
    pub width: f32,
    /// Panel height in pixels
    pub height: f32,
    /// Background color (RGBA)
    pub background_color: Vec4,
    /// Border radius for rounded corners
    pub border_radius: f32,
    /// Padding inside the panel
    pub padding: f32,
}

impl Default for UIPanelData {
    fn default() -> Self {
        Self {
            width: 200.0,
            height: 100.0,
            background_color: Vec4::new(0.2, 0.2, 0.25, 1.0),
            border_radius: 4.0,
            padding: 8.0,
        }
    }
}

/// Data component for UI label nodes
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct UILabelData {
    /// Text content
    pub text: String,
    /// Font size
    pub font_size: f32,
    /// Text color (RGBA)
    pub color: Vec4,
}

impl Default for UILabelData {
    fn default() -> Self {
        Self {
            text: "Label".to_string(),
            font_size: 16.0,
            color: Vec4::ONE,
        }
    }
}

/// Data component for UI button nodes
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct UIButtonData {
    /// Button text
    pub text: String,
    /// Button width
    pub width: f32,
    /// Button height
    pub height: f32,
    /// Font size
    pub font_size: f32,
    /// Normal background color
    pub normal_color: Vec4,
    /// Hover background color
    pub hover_color: Vec4,
    /// Pressed background color
    pub pressed_color: Vec4,
    /// Text color
    pub text_color: Vec4,
}

impl Default for UIButtonData {
    fn default() -> Self {
        Self {
            text: "Button".to_string(),
            width: 120.0,
            height: 40.0,
            font_size: 16.0,
            normal_color: Vec4::new(0.3, 0.3, 0.35, 1.0),
            hover_color: Vec4::new(0.4, 0.4, 0.45, 1.0),
            pressed_color: Vec4::new(0.2, 0.2, 0.25, 1.0),
            text_color: Vec4::ONE,
        }
    }
}

/// Data component for UI image nodes
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct UIImageData {
    /// Path to the image texture
    pub texture_path: String,
    /// Image width
    pub width: f32,
    /// Image height
    pub height: f32,
    /// Color tint (RGBA)
    pub tint: Vec4,
}

impl Default for UIImageData {
    fn default() -> Self {
        Self {
            texture_path: String::new(),
            width: 100.0,
            height: 100.0,
            tint: Vec4::ONE,
        }
    }
}
