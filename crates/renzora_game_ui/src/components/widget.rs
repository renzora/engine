//! Widget marker component and widget type enum.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// The type of UI widget an entity represents.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub enum UiWidgetType {
    // ── Layout ──
    #[default]
    Container,
    Panel,
    ScrollView,

    // ── Basic ──
    Text,
    Image,
    Button,

    // ── Input ──
    Slider,
    Checkbox,
    Toggle,
    RadioButton,
    Dropdown,
    TextInput,

    // ── Display ──
    BarFill,

    // ── Overlay ──
    Tooltip,
    Modal,
    DraggableWindow,

    // ── Menu ──
    KeybindRow,
    SettingsRow,

    // ── Extra ──
    Separator,
    NumberInput,
    Scrollbar,

    // ── Shapes ──
    Circle,
    Arc,
    RadialProgress,
    Line,
    Triangle,
    Polygon,
    Rectangle,
    Wedge,
}

impl UiWidgetType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Container => "Container",
            Self::Panel => "Panel",
            Self::ScrollView => "Scroll View",
            Self::Text => "Text",
            Self::Image => "Image",
            Self::Button => "Button",
            Self::Slider => "Slider",
            Self::Checkbox => "Checkbox",
            Self::Toggle => "Toggle",
            Self::RadioButton => "Radio Button",
            Self::Dropdown => "Dropdown",
            Self::TextInput => "Text Input",
            Self::BarFill => "Bar Fill",
            Self::Tooltip => "Tooltip",
            Self::Modal => "Modal",
            Self::DraggableWindow => "Draggable Window",
            Self::KeybindRow => "Keybind Row",
            Self::SettingsRow => "Settings Row",
            Self::Separator => "Separator",
            Self::NumberInput => "Number Input",
            Self::Scrollbar => "Scrollbar",
            Self::Circle => "Circle",
            Self::Arc => "Arc",
            Self::RadialProgress => "Radial Progress",
            Self::Line => "Line",
            Self::Triangle => "Triangle",
            Self::Polygon => "Polygon",
            Self::Rectangle => "Rectangle",
            Self::Wedge => "Wedge",
        }
    }

    #[cfg(feature = "editor")]
    pub fn icon(&self) -> &'static str {
        use egui_phosphor::regular::*;
        match self {
            Self::Container => SQUARES_FOUR,
            Self::Panel => RECTANGLE,
            Self::ScrollView => SCROLL,
            Self::Text => TEXT_AA,
            Self::Image => IMAGE,
            Self::Button => CURSOR_CLICK,
            Self::Slider => SLIDERS_HORIZONTAL,
            Self::Checkbox => CHECK_SQUARE,
            Self::Toggle => TOGGLE_RIGHT,
            Self::RadioButton => RADIO_BUTTON,
            Self::Dropdown => CARET_CIRCLE_DOWN,
            Self::TextInput => TEXT_T,
            Self::BarFill => BATTERY_MEDIUM,
            Self::Tooltip => CHAT_CIRCLE_TEXT,
            Self::Modal => BROWSERS,
            Self::DraggableWindow => APP_WINDOW,
            Self::KeybindRow => KEYBOARD,
            Self::SettingsRow => GEAR,
            Self::Separator => MINUS,
            Self::NumberInput => CALCULATOR,
            Self::Scrollbar => ARROWS_DOWN_UP,
            Self::Circle => CIRCLE,
            Self::Arc => CIRCLE_DASHED,
            Self::RadialProgress => CIRCLE_NOTCH,
            Self::Line => LINE_SEGMENT,
            Self::Triangle => TRIANGLE,
            Self::Polygon => HEXAGON,
            Self::Rectangle => RECTANGLE,
            Self::Wedge => CHART_PIE_SLICE,
        }
    }

    /// Category for grouping in the widget palette.
    pub fn category(&self) -> &'static str {
        match self {
            Self::Container | Self::Panel | Self::ScrollView => "Layout",
            Self::Text | Self::Image | Self::Button => "Basic",
            Self::Slider
            | Self::Checkbox
            | Self::Toggle
            | Self::RadioButton
            | Self::Dropdown
            | Self::TextInput => "Input",
            Self::BarFill => "Display",
            Self::Tooltip | Self::Modal | Self::DraggableWindow => "Overlay",
            Self::KeybindRow | Self::SettingsRow => "Menu",
            Self::Separator | Self::NumberInput | Self::Scrollbar => "Extra",
            Self::Circle
            | Self::Arc
            | Self::RadialProgress
            | Self::Line
            | Self::Triangle
            | Self::Polygon
            | Self::Rectangle
            | Self::Wedge => "Shapes",
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

/// Path (project-relative, e.g. `"ui/health_bar.html"`) of a bevy_hui template
/// an entity should display. Serializable source of truth; `renzora_hui`'s
/// observer turns it into a loaded `HtmlNode`. Defined here (not in
/// `renzora_hui`) so the UI canvas editor and viewport — which can't depend on
/// `renzora_hui` without a cycle — can spawn it directly.
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct HtmlTemplatePath(pub String);

/// Identifies a child entity's role within a composite widget.
///
/// E.g. the fill bar inside a ProgressBar, the thumb on a Slider.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct UiWidgetPart {
    pub role: String,
}

impl UiWidgetPart {
    pub fn new(role: impl Into<String>) -> Self {
        Self { role: role.into() }
    }
}
