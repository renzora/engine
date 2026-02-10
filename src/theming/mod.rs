//! Theming system for the Renzora editor
//!
//! Provides a comprehensive color theming system with:
//! - Default dark theme hardcoded in source
//! - Custom themes stored as TOML files in project's themes/ directory
//! - Theme editor UI for live color customization

mod defaults;
mod loader;

#[cfg(test)]
mod tests;

pub use loader::*;

use bevy_egui::egui::Color32;
use serde::{Deserialize, Serialize, Serializer, Deserializer};

/// A color wrapper that serializes to/from hex format (#RRGGBB or #RRGGBBAA)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThemeColor(pub Color32);

impl ThemeColor {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self(Color32::from_rgb(r, g, b))
    }

    pub fn with_alpha(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self(Color32::from_rgba_unmultiplied(r, g, b, a))
    }

    pub fn from_color32(color: Color32) -> Self {
        Self(color)
    }

    pub fn to_color32(self) -> Color32 {
        self.0
    }

    /// Parse from hex string (#RRGGBB or #RRGGBBAA)
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim_start_matches('#');
        match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Self::new(r, g, b))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(Self::with_alpha(r, g, b, a))
            }
            _ => None,
        }
    }

    /// Convert to hex string (#RRGGBB or #RRGGBBAA if alpha != 255)
    pub fn to_hex(self) -> String {
        let [r, g, b, a] = self.0.to_array();
        if a == 255 {
            format!("#{:02X}{:02X}{:02X}", r, g, b)
        } else {
            format!("#{:02X}{:02X}{:02X}{:02X}", r, g, b, a)
        }
    }
}

impl Default for ThemeColor {
    fn default() -> Self {
        Self(Color32::WHITE)
    }
}

impl From<Color32> for ThemeColor {
    fn from(color: Color32) -> Self {
        Self(color)
    }
}

impl From<ThemeColor> for Color32 {
    fn from(color: ThemeColor) -> Self {
        color.0
    }
}

impl Serialize for ThemeColor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for ThemeColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        ThemeColor::from_hex(&s)
            .ok_or_else(|| serde::de::Error::custom(format!("Invalid hex color: {}", s)))
    }
}

/// Complete theme definition with all editor colors
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Theme {
    /// Theme metadata
    #[serde(default)]
    pub meta: ThemeMeta,

    /// Semantic colors (accent, success, warning, error, etc.)
    #[serde(default)]
    pub semantic: SemanticColors,

    /// Surface/background colors
    #[serde(default)]
    pub surfaces: SurfaceColors,

    /// Text colors
    #[serde(default)]
    pub text: TextColors,

    /// Widget colors (buttons, inputs, etc.)
    #[serde(default)]
    pub widgets: WidgetColors,

    /// Panel-specific colors
    #[serde(default)]
    pub panels: PanelColors,

    /// Component category colors (for inspector)
    #[serde(default)]
    pub categories: CategoryColors,

    /// Blueprint editor colors
    #[serde(default)]
    pub blueprint: BlueprintColors,

    /// Viewport colors
    #[serde(default)]
    pub viewport: ViewportColors,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

/// Theme metadata
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ThemeMeta {
    pub name: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub version: String,
}

/// Semantic colors for common UI states and feedback
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct SemanticColors {
    pub accent: ThemeColor,
    pub success: ThemeColor,
    pub warning: ThemeColor,
    pub error: ThemeColor,
    pub selection: ThemeColor,
    pub selection_stroke: ThemeColor,
}

impl Default for SemanticColors {
    fn default() -> Self {
        Self {
            accent: ThemeColor::new(66, 150, 250),
            success: ThemeColor::new(89, 191, 115),
            warning: ThemeColor::new(242, 166, 64),
            error: ThemeColor::new(230, 89, 89),
            selection: ThemeColor::new(66, 150, 250),
            selection_stroke: ThemeColor::new(100, 180, 255),
        }
    }
}

/// Surface and background colors
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct SurfaceColors {
    pub window: ThemeColor,
    pub window_stroke: ThemeColor,
    pub panel: ThemeColor,
    pub popup: ThemeColor,
    pub overlay: ThemeColor,
    pub faint: ThemeColor,
    pub extreme: ThemeColor,
}

impl Default for SurfaceColors {
    fn default() -> Self {
        Self {
            window: ThemeColor::new(26, 26, 31),
            window_stroke: ThemeColor::new(50, 50, 58),
            panel: ThemeColor::new(26, 26, 31),
            popup: ThemeColor::new(28, 28, 35),
            overlay: ThemeColor::with_alpha(0, 0, 0, 180),
            faint: ThemeColor::new(20, 20, 24),
            extreme: ThemeColor::new(15, 15, 18),
        }
    }
}

/// Text colors
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct TextColors {
    pub primary: ThemeColor,
    pub secondary: ThemeColor,
    pub muted: ThemeColor,
    pub heading: ThemeColor,
    pub disabled: ThemeColor,
    pub hyperlink: ThemeColor,
}

impl Default for TextColors {
    fn default() -> Self {
        Self {
            primary: ThemeColor::new(230, 230, 240),
            secondary: ThemeColor::new(200, 200, 210),
            muted: ThemeColor::new(140, 140, 155),
            heading: ThemeColor::new(180, 180, 195),
            disabled: ThemeColor::new(100, 100, 110),
            hyperlink: ThemeColor::new(100, 180, 255),
        }
    }
}

/// Widget colors (buttons, inputs, etc.)
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct WidgetColors {
    /// Non-interactive widget background
    pub noninteractive_bg: ThemeColor,
    pub noninteractive_fg: ThemeColor,

    /// Inactive (can interact but not hovered)
    pub inactive_bg: ThemeColor,
    pub inactive_fg: ThemeColor,

    /// Hovered state
    pub hovered_bg: ThemeColor,
    pub hovered_fg: ThemeColor,

    /// Active/pressed state
    pub active_bg: ThemeColor,
    pub active_fg: ThemeColor,

    /// Border/stroke colors
    pub border: ThemeColor,
    pub border_light: ThemeColor,
}

impl Default for WidgetColors {
    fn default() -> Self {
        Self {
            noninteractive_bg: ThemeColor::new(36, 36, 42),
            noninteractive_fg: ThemeColor::new(180, 180, 190),
            inactive_bg: ThemeColor::new(46, 46, 56),
            inactive_fg: ThemeColor::new(200, 200, 210),
            hovered_bg: ThemeColor::new(56, 56, 68),
            hovered_fg: ThemeColor::new(220, 220, 230),
            active_bg: ThemeColor::new(66, 150, 250),
            active_fg: ThemeColor::new(255, 255, 255),
            border: ThemeColor::new(50, 50, 60),
            border_light: ThemeColor::new(45, 45, 52),
        }
    }
}

/// Panel-specific colors
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelColors {
    /// Hierarchy panel
    pub tree_line: ThemeColor,
    pub drop_line: ThemeColor,
    pub drop_child_highlight: ThemeColor,
    pub row_odd_bg: ThemeColor,

    /// Inspector panel
    pub inspector_row_even: ThemeColor,
    pub inspector_row_odd: ThemeColor,
    pub category_frame_bg: ThemeColor,

    /// Assets panel
    pub item_bg: ThemeColor,
    pub item_hover: ThemeColor,

    /// Tab bar
    pub tab_active: ThemeColor,
    pub tab_inactive: ThemeColor,
    pub tab_hover: ThemeColor,

    /// Close button hover
    pub close_hover: ThemeColor,
}

impl Default for PanelColors {
    fn default() -> Self {
        Self {
            tree_line: ThemeColor::new(60, 60, 70),
            drop_line: ThemeColor::new(80, 140, 255),
            drop_child_highlight: ThemeColor::with_alpha(80, 140, 255, 50),
            row_odd_bg: ThemeColor::with_alpha(255, 255, 255, 6),
            inspector_row_even: ThemeColor::new(32, 34, 38),
            inspector_row_odd: ThemeColor::new(38, 40, 44),
            category_frame_bg: ThemeColor::new(30, 32, 36),
            item_bg: ThemeColor::new(35, 35, 45),
            item_hover: ThemeColor::new(45, 45, 58),
            tab_active: ThemeColor::new(45, 45, 58),
            tab_inactive: ThemeColor::new(28, 28, 35),
            tab_hover: ThemeColor::new(45, 47, 55),
            close_hover: ThemeColor::new(200, 60, 60),
        }
    }
}

/// Component category colors (for inspector)
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct CategoryColors {
    pub transform: CategoryStyle,
    pub environment: CategoryStyle,
    pub lighting: CategoryStyle,
    pub camera: CategoryStyle,
    pub scripting: CategoryStyle,
    pub physics: CategoryStyle,
    pub rendering: CategoryStyle,
    pub audio: CategoryStyle,
    pub ui: CategoryStyle,
    pub effects: CategoryStyle,
    pub post_process: CategoryStyle,
    pub gameplay: CategoryStyle,
    pub nodes_2d: CategoryStyle,
    pub plugin: CategoryStyle,
}

impl Default for CategoryColors {
    fn default() -> Self {
        Self {
            transform: CategoryStyle {
                accent: ThemeColor::new(99, 178, 238),
                header_bg: ThemeColor::new(35, 45, 55),
            },
            environment: CategoryStyle {
                accent: ThemeColor::new(134, 188, 126),
                header_bg: ThemeColor::new(35, 48, 42),
            },
            lighting: CategoryStyle {
                accent: ThemeColor::new(247, 207, 100),
                header_bg: ThemeColor::new(50, 45, 35),
            },
            camera: CategoryStyle {
                accent: ThemeColor::new(178, 132, 209),
                header_bg: ThemeColor::new(42, 38, 52),
            },
            scripting: CategoryStyle {
                accent: ThemeColor::new(236, 154, 120),
                header_bg: ThemeColor::new(50, 40, 38),
            },
            physics: CategoryStyle {
                accent: ThemeColor::new(120, 200, 200),
                header_bg: ThemeColor::new(35, 48, 50),
            },
            rendering: CategoryStyle {
                accent: ThemeColor::new(99, 178, 238),
                header_bg: ThemeColor::new(35, 45, 55),
            },
            audio: CategoryStyle {
                accent: ThemeColor::new(100, 180, 100),
                header_bg: ThemeColor::new(35, 45, 40),
            },
            ui: CategoryStyle {
                accent: ThemeColor::new(191, 166, 242),
                header_bg: ThemeColor::new(42, 40, 52),
            },
            effects: CategoryStyle {
                accent: ThemeColor::new(255, 180, 220),
                header_bg: ThemeColor::new(50, 38, 45),
            },
            post_process: CategoryStyle {
                accent: ThemeColor::new(130, 200, 160),
                header_bg: ThemeColor::new(35, 48, 45),
            },
            gameplay: CategoryStyle {
                accent: ThemeColor::new(255, 150, 150),
                header_bg: ThemeColor::new(50, 38, 38),
            },
            nodes_2d: CategoryStyle {
                accent: ThemeColor::new(242, 140, 191),
                header_bg: ThemeColor::new(50, 38, 45),
            },
            plugin: CategoryStyle {
                accent: ThemeColor::new(180, 140, 180),
                header_bg: ThemeColor::new(45, 38, 45),
            },
        }
    }
}

/// Style for a component category
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct CategoryStyle {
    pub accent: ThemeColor,
    pub header_bg: ThemeColor,
}

impl Default for CategoryStyle {
    fn default() -> Self {
        Self {
            accent: ThemeColor::new(180, 180, 190),
            header_bg: ThemeColor::new(40, 40, 48),
        }
    }
}

/// Blueprint editor colors
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct BlueprintColors {
    pub canvas_bg: ThemeColor,
    pub grid_dot: ThemeColor,
    pub node_bg: ThemeColor,
    pub node_border: ThemeColor,
    pub node_selected_border: ThemeColor,
    pub connection: ThemeColor,
    pub connection_preview: ThemeColor,
    pub selection_rect_fill: ThemeColor,
    pub selection_rect_stroke: ThemeColor,
}

impl Default for BlueprintColors {
    fn default() -> Self {
        Self {
            canvas_bg: ThemeColor::new(25, 25, 30),
            grid_dot: ThemeColor::new(60, 60, 65),
            node_bg: ThemeColor::new(40, 40, 45),
            node_border: ThemeColor::new(60, 60, 65),
            node_selected_border: ThemeColor::new(100, 150, 255),
            connection: ThemeColor::new(200, 200, 200),
            connection_preview: ThemeColor::new(255, 255, 100),
            selection_rect_fill: ThemeColor::with_alpha(100, 150, 255, 30),
            selection_rect_stroke: ThemeColor::new(100, 150, 255),
        }
    }
}

/// Viewport colors
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewportColors {
    pub grid_line: ThemeColor,
    pub gizmo_x: ThemeColor,
    pub gizmo_y: ThemeColor,
    pub gizmo_z: ThemeColor,
    pub gizmo_selected: ThemeColor,
}

impl Default for ViewportColors {
    fn default() -> Self {
        Self {
            grid_line: ThemeColor::new(77, 77, 77),
            gizmo_x: ThemeColor::new(255, 80, 80),
            gizmo_y: ThemeColor::new(80, 255, 80),
            gizmo_z: ThemeColor::new(80, 80, 255),
            gizmo_selected: ThemeColor::new(255, 255, 0),
        }
    }
}
