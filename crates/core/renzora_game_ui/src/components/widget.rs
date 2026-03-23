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
    ProgressBar,
    HealthBar,
    Spinner,
    TabBar,

    // ── Overlay ──
    Tooltip,
    Modal,
    DraggableWindow,

    // ── HUD ──
    Crosshair,
    AmmoCounter,
    Compass,
    StatusEffectBar,
    NotificationFeed,
    RadialMenu,
    Minimap,

    // ── Menu ──
    InventoryGrid,
    DialogBox,
    ObjectiveTracker,
    LoadingScreen,
    KeybindRow,
    SettingsRow,

    // ── Extra ──
    Separator,
    NumberInput,
    VerticalSlider,
    Scrollbar,
    List,

    // ── Shapes ──
    Circle,
    Arc,
    RadialProgress,
    Line,
    Triangle,
    Polygon,
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
            Self::ProgressBar => "Progress Bar",
            Self::HealthBar => "Health Bar",
            Self::Spinner => "Spinner",
            Self::TabBar => "Tab Bar",
            Self::Tooltip => "Tooltip",
            Self::Modal => "Modal",
            Self::DraggableWindow => "Draggable Window",
            Self::Crosshair => "Crosshair",
            Self::AmmoCounter => "Ammo Counter",
            Self::Compass => "Compass",
            Self::StatusEffectBar => "Status Effects",
            Self::NotificationFeed => "Notifications",
            Self::RadialMenu => "Radial Menu",
            Self::Minimap => "Minimap",
            Self::InventoryGrid => "Inventory Grid",
            Self::DialogBox => "Dialog Box",
            Self::ObjectiveTracker => "Objective Tracker",
            Self::LoadingScreen => "Loading Screen",
            Self::KeybindRow => "Keybind Row",
            Self::SettingsRow => "Settings Row",
            Self::Separator => "Separator",
            Self::NumberInput => "Number Input",
            Self::VerticalSlider => "Vertical Slider",
            Self::Scrollbar => "Scrollbar",
            Self::List => "List",
            Self::Circle => "Circle",
            Self::Arc => "Arc",
            Self::RadialProgress => "Radial Progress",
            Self::Line => "Line",
            Self::Triangle => "Triangle",
            Self::Polygon => "Polygon",
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
            Self::ProgressBar => BATTERY_MEDIUM,
            Self::HealthBar => HEART,
            Self::Spinner => SPINNER,
            Self::TabBar => TABS,
            Self::Tooltip => CHAT_CIRCLE_TEXT,
            Self::Modal => BROWSERS,
            Self::DraggableWindow => APP_WINDOW,
            Self::Crosshair => CROSSHAIR,
            Self::AmmoCounter => HASH,
            Self::Compass => COMPASS,
            Self::StatusEffectBar => LIGHTNING,
            Self::NotificationFeed => BELL,
            Self::RadialMenu => SELECTION_ALL,
            Self::Minimap => MAP_TRIFOLD,
            Self::InventoryGrid => GRID_FOUR,
            Self::DialogBox => CHAT_DOTS,
            Self::ObjectiveTracker => LIST_CHECKS,
            Self::LoadingScreen => HOURGLASS,
            Self::KeybindRow => KEYBOARD,
            Self::SettingsRow => GEAR,
            Self::Separator => MINUS,
            Self::NumberInput => CALCULATOR,
            Self::VerticalSlider => SLIDERS,
            Self::Scrollbar => ARROWS_DOWN_UP,
            Self::List => LIST,
            Self::Circle => CIRCLE,
            Self::Arc => CIRCLE_DASHED,
            Self::RadialProgress => CIRCLE_NOTCH,
            Self::Line => LINE_SEGMENT,
            Self::Triangle => TRIANGLE,
            Self::Polygon => HEXAGON,
            Self::Wedge => CHART_PIE_SLICE,
        }
    }

    /// Category for grouping in the widget palette.
    pub fn category(&self) -> &'static str {
        match self {
            Self::Container | Self::Panel | Self::ScrollView => "Layout",
            Self::Text | Self::Image | Self::Button => "Basic",
            Self::Slider | Self::Checkbox | Self::Toggle | Self::RadioButton | Self::Dropdown | Self::TextInput => "Input",
            Self::ProgressBar | Self::HealthBar | Self::Spinner | Self::TabBar => "Display",
            Self::Tooltip | Self::Modal | Self::DraggableWindow => "Overlay",
            Self::Crosshair | Self::AmmoCounter | Self::Compass | Self::StatusEffectBar | Self::NotificationFeed | Self::RadialMenu | Self::Minimap => "HUD",
            Self::InventoryGrid | Self::DialogBox | Self::ObjectiveTracker | Self::LoadingScreen | Self::KeybindRow | Self::SettingsRow => "Menu",
            Self::Separator | Self::NumberInput | Self::VerticalSlider | Self::Scrollbar | Self::List => "Extra",
            Self::Circle | Self::Arc | Self::RadialProgress | Self::Line | Self::Triangle | Self::Polygon | Self::Wedge => "Shapes",
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
