//! Serializable UI components that mark bevy_ui entities for the editor.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// The type of UI widget an entity represents.
#[derive(Clone, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum UiWidgetType {
    /// Empty Node — layout container (flexbox grouping).
    #[default]
    Container,
    /// Text label.
    Text,
    /// Image / sprite.
    Image,
    /// Clickable button (container + interaction).
    Button,
    /// Styled panel (background, border, rounded corners).
    Panel,
}

impl UiWidgetType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Container => "Container",
            Self::Text => "Text",
            Self::Image => "Image",
            Self::Button => "Button",
            Self::Panel => "Panel",
        }
    }

    #[cfg(feature = "editor")]
    pub fn icon(&self) -> &'static str {
        use egui_phosphor::regular::*;
        match self {
            Self::Container => SQUARES_FOUR,
            Self::Text => TEXT_AA,
            Self::Image => IMAGE,
            Self::Button => CURSOR_CLICK,
            Self::Panel => RECTANGLE,
        }
    }
}

/// Marker component for a UI canvas root entity.
///
/// A canvas is a top-level container that groups UI widgets.
/// Multiple canvases can exist per scene (e.g. HUD, pause menu, inventory).
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct UiCanvas {
    /// Render order — higher values draw on top.
    pub sort_order: i32,
    /// When to show: "always", "play_only", "editor_only".
    pub visibility_mode: String,
}

impl Default for UiCanvas {
    fn default() -> Self {
        Self {
            sort_order: 0,
            visibility_mode: "always".into(),
        }
    }
}

/// Marker component for any UI widget entity (child of a UiCanvas).
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct UiWidget {
    pub widget_type: UiWidgetType,
    /// Locked widgets cannot be moved/resized in the canvas editor.
    pub locked: bool,
}

impl Default for UiWidget {
    fn default() -> Self {
        Self {
            widget_type: UiWidgetType::Container,
            locked: false,
        }
    }
}
