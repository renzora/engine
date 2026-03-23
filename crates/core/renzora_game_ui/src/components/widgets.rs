//! Data components for each widget type.
//!
//! Each component stores the widget-specific state. Runtime systems read these
//! and drive child entities (fill bars, thumbs, checkmarks, etc.).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

// ── Progress Bar ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum ProgressDirection {
    #[default]
    LeftToRight,
    RightToLeft,
    BottomToTop,
    TopToBottom,
}

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct ProgressBarData {
    pub value: f32,
    pub max: f32,
    pub fill_color: Color,
    pub bg_color: Color,
    pub direction: ProgressDirection,
}

impl Default for ProgressBarData {
    fn default() -> Self {
        Self {
            value: 0.5,
            max: 1.0,
            fill_color: Color::srgba(0.3, 0.7, 0.3, 1.0),
            bg_color: Color::srgba(0.2, 0.2, 0.2, 0.8),
            direction: ProgressDirection::LeftToRight,
        }
    }
}

// ── Health Bar ──────────────────────────────────────────────────────────────

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct HealthBarData {
    pub current: f32,
    pub max: f32,
    pub low_threshold: f32,
    pub fill_color: Color,
    pub low_color: Color,
    pub bg_color: Color,
}

impl Default for HealthBarData {
    fn default() -> Self {
        Self {
            current: 75.0,
            max: 100.0,
            low_threshold: 0.25,
            fill_color: Color::srgba(0.2, 0.8, 0.2, 1.0),
            low_color: Color::srgba(0.9, 0.2, 0.2, 1.0),
            bg_color: Color::srgba(0.15, 0.15, 0.15, 0.9),
        }
    }
}

// ── Slider ──────────────────────────────────────────────────────────────────

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SliderData {
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub track_color: Color,
    pub fill_color: Color,
    pub thumb_color: Color,
}

impl Default for SliderData {
    fn default() -> Self {
        Self {
            value: 0.5,
            min: 0.0,
            max: 1.0,
            step: 0.0,
            track_color: Color::srgba(0.25, 0.25, 0.25, 1.0),
            fill_color: Color::srgba(0.3, 0.5, 0.9, 1.0),
            thumb_color: Color::WHITE,
        }
    }
}

// ── Checkbox ────────────────────────────────────────────────────────────────

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct CheckboxData {
    pub checked: bool,
    pub label: String,
    pub check_color: Color,
    pub box_color: Color,
}

impl Default for CheckboxData {
    fn default() -> Self {
        Self {
            checked: false,
            label: "Checkbox".into(),
            check_color: Color::WHITE,
            box_color: Color::srgba(0.3, 0.3, 0.3, 1.0),
        }
    }
}

// ── Toggle ──────────────────────────────────────────────────────────────────

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct ToggleData {
    pub on: bool,
    pub label: String,
    pub on_color: Color,
    pub off_color: Color,
    pub knob_color: Color,
}

impl Default for ToggleData {
    fn default() -> Self {
        Self {
            on: false,
            label: "Toggle".into(),
            on_color: Color::srgba(0.3, 0.7, 0.3, 1.0),
            off_color: Color::srgba(0.4, 0.4, 0.4, 1.0),
            knob_color: Color::WHITE,
        }
    }
}

// ── Radio Button ────────────────────────────────────────────────────────────

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct RadioButtonData {
    pub group: String,
    pub selected: bool,
    pub label: String,
    pub active_color: Color,
}

impl Default for RadioButtonData {
    fn default() -> Self {
        Self {
            group: "default".into(),
            selected: false,
            label: "Option".into(),
            active_color: Color::srgba(0.3, 0.5, 0.9, 1.0),
        }
    }
}

// ── Dropdown ────────────────────────────────────────────────────────────────

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct DropdownData {
    pub options: Vec<String>,
    pub selected: i32,
    pub open: bool,
    pub placeholder: String,
}

impl Default for DropdownData {
    fn default() -> Self {
        Self {
            options: vec!["Option A".into(), "Option B".into(), "Option C".into()],
            selected: -1,
            open: false,
            placeholder: "Select...".into(),
        }
    }
}

// ── Text Input ──────────────────────────────────────────────────────────────

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct TextInputData {
    pub text: String,
    pub placeholder: String,
    pub max_length: usize,
    pub focused: bool,
    pub password: bool,
}

impl Default for TextInputData {
    fn default() -> Self {
        Self {
            text: String::new(),
            placeholder: "Enter text...".into(),
            max_length: 256,
            focused: false,
            password: false,
        }
    }
}

// ── Scroll View ─────────────────────────────────────────────────────────────

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct ScrollViewData {
    pub scroll_speed: f32,
    pub show_horizontal: bool,
    pub show_vertical: bool,
}

impl Default for ScrollViewData {
    fn default() -> Self {
        Self {
            scroll_speed: 20.0,
            show_horizontal: false,
            show_vertical: true,
        }
    }
}

// ── Tab Bar ─────────────────────────────────────────────────────────────────

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct TabBarData {
    pub tabs: Vec<String>,
    pub active: usize,
    pub tab_color: Color,
    pub active_color: Color,
}

impl Default for TabBarData {
    fn default() -> Self {
        Self {
            tabs: vec!["Tab 1".into(), "Tab 2".into(), "Tab 3".into()],
            active: 0,
            tab_color: Color::srgba(0.2, 0.2, 0.2, 1.0),
            active_color: Color::srgba(0.3, 0.5, 0.9, 1.0),
        }
    }
}

// ── Spinner ─────────────────────────────────────────────────────────────────

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SpinnerData {
    pub speed: f32,
    pub color: Color,
}

impl Default for SpinnerData {
    fn default() -> Self {
        Self {
            speed: 2.0,
            color: Color::WHITE,
        }
    }
}

// ── Tooltip ─────────────────────────────────────────────────────────────────

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct TooltipData {
    pub text: String,
    pub delay_ms: u32,
    pub bg_color: Color,
    pub text_color: Color,
}

impl Default for TooltipData {
    fn default() -> Self {
        Self {
            text: "Tooltip text".into(),
            delay_ms: 500,
            bg_color: Color::srgba(0.1, 0.1, 0.1, 0.95),
            text_color: Color::WHITE,
        }
    }
}

// ── Modal ───────────────────────────────────────────────────────────────────

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct ModalData {
    pub title: String,
    pub closable: bool,
    pub backdrop_color: Color,
}

impl Default for ModalData {
    fn default() -> Self {
        Self {
            title: "Dialog".into(),
            closable: true,
            backdrop_color: Color::srgba(0.0, 0.0, 0.0, 0.5),
        }
    }
}

// ── Image ────────────────────────────────────────────────────────────

/// Serializable asset path for UI image widgets.
///
/// `ImageNode` contains a `Handle<Image>` which can't be serialized.
/// This component stores the asset-relative path so the image can be
/// rehydrated after scene deserialization.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct UiImagePath {
    pub path: String,
}

// ── Draggable Window ────────────────────────────────────────────────────────

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct DraggableWindowData {
    pub title: String,
    pub closable: bool,
    pub minimizable: bool,
    pub title_bar_color: Color,
}

impl Default for DraggableWindowData {
    fn default() -> Self {
        Self {
            title: "Window".into(),
            closable: true,
            minimizable: true,
            title_bar_color: Color::srgba(0.2, 0.2, 0.25, 1.0),
        }
    }
}

// ── Crosshair ──────────────────────────────────────────────────────────────

/// Crosshair reticle style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum CrosshairStyle {
    #[default]
    Cross,
    Dot,
    CircleDot,
    CrossDot,
}

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct CrosshairData {
    pub style: CrosshairStyle,
    pub color: Color,
    /// Total size in pixels.
    pub size: f32,
    /// Line length (for Cross/CrossDot styles).
    pub line_length: f32,
    /// Line thickness in pixels.
    pub thickness: f32,
    /// Gap from center in pixels (for Cross/CrossDot).
    pub gap: f32,
    /// Dot radius (for Dot/CircleDot/CrossDot).
    pub dot_size: f32,
}

impl Default for CrosshairData {
    fn default() -> Self {
        Self {
            style: CrosshairStyle::Cross,
            color: Color::WHITE,
            size: 32.0,
            line_length: 8.0,
            thickness: 2.0,
            gap: 4.0,
            dot_size: 2.0,
        }
    }
}

// ── Ammo Counter ───────────────────────────────────────────────────────────

/// How the ammo count is displayed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum AmmoDisplayMode {
    #[default]
    Numeric,
    Icons,
    Both,
}

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct AmmoCounterData {
    pub current: u32,
    pub max: u32,
    pub display_mode: AmmoDisplayMode,
    pub color: Color,
    pub low_color: Color,
    /// Threshold below which `low_color` is used.
    pub low_threshold: u32,
}

impl Default for AmmoCounterData {
    fn default() -> Self {
        Self {
            current: 30,
            max: 30,
            display_mode: AmmoDisplayMode::Numeric,
            color: Color::WHITE,
            low_color: Color::srgba(1.0, 0.3, 0.3, 1.0),
            low_threshold: 5,
        }
    }
}

// ── Compass ────────────────────────────────────────────────────────────────

/// A compass marker at a specific heading angle.
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct CompassMarker {
    /// Label text (e.g. "N", "S", "E", "W", or a custom waypoint).
    pub label: String,
    /// Heading angle in degrees (0 = North, 90 = East).
    pub angle: f32,
    pub color: Color,
}

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct CompassData {
    /// Current player heading in degrees (0 = North, clockwise).
    pub heading: f32,
    /// Field of view shown in the strip (degrees).
    pub fov: f32,
    pub markers: Vec<CompassMarker>,
    pub text_color: Color,
    pub tick_color: Color,
}

impl Default for CompassData {
    fn default() -> Self {
        Self {
            heading: 0.0,
            fov: 180.0,
            markers: vec![
                CompassMarker { label: "N".into(), angle: 0.0, color: Color::srgba(1.0, 0.3, 0.3, 1.0) },
                CompassMarker { label: "E".into(), angle: 90.0, color: Color::WHITE },
                CompassMarker { label: "S".into(), angle: 180.0, color: Color::WHITE },
                CompassMarker { label: "W".into(), angle: 270.0, color: Color::WHITE },
            ],
            text_color: Color::WHITE,
            tick_color: Color::srgba(0.6, 0.6, 0.6, 1.0),
        }
    }
}

// ── Status Effect Bar ──────────────────────────────────────────────────────

/// A single status effect (buff/debuff).
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct StatusEffect {
    pub name: String,
    /// Duration in seconds. 0 = permanent.
    pub duration: f32,
    /// Elapsed time in seconds.
    pub elapsed: f32,
    pub color: Color,
}

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct StatusEffectBarData {
    pub effects: Vec<StatusEffect>,
    /// Size of each effect icon in pixels.
    pub icon_size: f32,
    /// Spacing between icons.
    pub spacing: f32,
}

impl Default for StatusEffectBarData {
    fn default() -> Self {
        Self {
            effects: Vec::new(),
            icon_size: 32.0,
            spacing: 4.0,
        }
    }
}

// ── Notification Feed ──────────────────────────────────────────────────────

/// A single notification message.
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct Notification {
    pub text: String,
    pub color: Color,
    /// Remaining lifetime in seconds.
    pub remaining: f32,
}

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct NotificationFeedData {
    pub notifications: Vec<Notification>,
    /// Maximum number of visible notifications.
    pub max_visible: usize,
    /// Default lifetime for new notifications (seconds).
    pub default_duration: f32,
    /// Fade-out duration (seconds).
    pub fade_duration: f32,
}

impl Default for NotificationFeedData {
    fn default() -> Self {
        Self {
            notifications: Vec::new(),
            max_visible: 5,
            default_duration: 4.0,
            fade_duration: 1.0,
        }
    }
}

// ── Radial Menu ────────────────────────────────────────────────────────────

/// A single item in a radial menu.
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct RadialMenuItem {
    pub label: String,
    pub color: Color,
}

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct RadialMenuData {
    pub items: Vec<RadialMenuItem>,
    /// Whether the menu is currently open.
    pub open: bool,
    /// Currently highlighted segment (-1 = none).
    pub selected: i32,
    /// Inner radius as fraction (0.0–1.0).
    pub inner_radius: f32,
    pub bg_color: Color,
    pub highlight_color: Color,
}

impl Default for RadialMenuData {
    fn default() -> Self {
        Self {
            items: vec![
                RadialMenuItem { label: "Item 1".into(), color: Color::srgba(0.3, 0.5, 0.9, 0.8) },
                RadialMenuItem { label: "Item 2".into(), color: Color::srgba(0.3, 0.5, 0.9, 0.8) },
                RadialMenuItem { label: "Item 3".into(), color: Color::srgba(0.3, 0.5, 0.9, 0.8) },
                RadialMenuItem { label: "Item 4".into(), color: Color::srgba(0.3, 0.5, 0.9, 0.8) },
            ],
            open: false,
            selected: -1,
            inner_radius: 0.3,
            bg_color: Color::srgba(0.15, 0.15, 0.18, 0.9),
            highlight_color: Color::srgba(0.4, 0.6, 1.0, 0.9),
        }
    }
}

// ── Minimap ────────────────────────────────────────────────────────────────

/// Minimap rotation mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum MinimapRotation {
    /// North is always up.
    #[default]
    FixedNorth,
    /// Map rotates with the player.
    PlayerRelative,
}

/// Minimap frame shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum MinimapShape {
    #[default]
    Circle,
    Square,
}

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct MinimapData {
    /// Zoom level (higher = more zoomed in).
    pub zoom: f32,
    pub rotation_mode: MinimapRotation,
    pub shape: MinimapShape,
    pub border_color: Color,
    pub border_width: f32,
    pub bg_color: Color,
}

impl Default for MinimapData {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            rotation_mode: MinimapRotation::FixedNorth,
            shape: MinimapShape::Circle,
            border_color: Color::srgba(0.4, 0.4, 0.45, 1.0),
            border_width: 2.0,
            bg_color: Color::srgba(0.1, 0.12, 0.1, 0.8),
        }
    }
}

// ── Inventory Grid ──────────────────────────────────────────────────────────

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct InventoryGridData {
    /// Number of columns.
    pub columns: u32,
    /// Number of rows.
    pub rows: u32,
    /// Size of each slot in pixels (square).
    pub slot_size: f32,
    /// Gap between slots in pixels.
    pub gap: f32,
    pub slot_bg_color: Color,
    pub slot_border_color: Color,
    pub slot_border_width: f32,
}

impl Default for InventoryGridData {
    fn default() -> Self {
        Self {
            columns: 6,
            rows: 4,
            slot_size: 48.0,
            gap: 4.0,
            slot_bg_color: Color::srgba(0.15, 0.15, 0.18, 0.9),
            slot_border_color: Color::srgba(0.4, 0.4, 0.45, 0.6),
            slot_border_width: 1.0,
        }
    }
}

/// Marker for individual inventory slot entities.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct InventorySlot {
    /// Grid position (column, row).
    pub col: u32,
    pub row: u32,
}

// ── Dialog Box ──────────────────────────────────────────────────────────────

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct DialogBoxData {
    /// Speaker name displayed at top.
    pub speaker: String,
    /// Full dialog text (may type-write over time).
    pub text: String,
    /// Characters revealed so far (for typewriter effect).
    pub chars_revealed: usize,
    /// Characters per second for typewriter effect (0 = instant).
    pub chars_per_second: f32,
    /// Accumulated time for typewriter.
    pub elapsed: f32,
    pub speaker_color: Color,
    pub text_color: Color,
    pub bg_color: Color,
}

impl Default for DialogBoxData {
    fn default() -> Self {
        Self {
            speaker: "NPC".into(),
            text: "Hello, adventurer!".into(),
            chars_revealed: 0,
            chars_per_second: 30.0,
            elapsed: 0.0,
            speaker_color: Color::srgba(0.9, 0.8, 0.3, 1.0),
            text_color: Color::WHITE,
            bg_color: Color::srgba(0.08, 0.08, 0.1, 0.95),
        }
    }
}

// ── Objective Tracker ───────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum ObjectiveStatus {
    #[default]
    Active,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct Objective {
    pub label: String,
    pub status: ObjectiveStatus,
    /// Optional progress (e.g. "3/5 items collected").
    pub progress: Option<(u32, u32)>,
}

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct ObjectiveTrackerData {
    pub title: String,
    pub objectives: Vec<Objective>,
    pub title_color: Color,
    pub active_color: Color,
    pub completed_color: Color,
    pub failed_color: Color,
}

impl Default for ObjectiveTrackerData {
    fn default() -> Self {
        Self {
            title: "Objectives".into(),
            objectives: vec![
                Objective {
                    label: "Find the key".into(),
                    status: ObjectiveStatus::Active,
                    progress: None,
                },
                Objective {
                    label: "Collect items".into(),
                    status: ObjectiveStatus::Active,
                    progress: Some((2, 5)),
                },
            ],
            title_color: Color::srgba(0.9, 0.8, 0.3, 1.0),
            active_color: Color::WHITE,
            completed_color: Color::srgba(0.3, 0.8, 0.3, 1.0),
            failed_color: Color::srgba(0.8, 0.3, 0.3, 1.0),
        }
    }
}

// ── Loading Screen ──────────────────────────────────────────────────────────

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct LoadingScreenData {
    /// Loading progress 0.0–1.0.
    pub progress: f32,
    /// Loading message / tip text.
    pub message: String,
    pub bg_color: Color,
    pub bar_color: Color,
    pub bar_bg_color: Color,
    pub text_color: Color,
}

impl Default for LoadingScreenData {
    fn default() -> Self {
        Self {
            progress: 0.0,
            message: "Loading...".into(),
            bg_color: Color::srgba(0.05, 0.05, 0.07, 1.0),
            bar_color: Color::srgba(0.3, 0.6, 1.0, 1.0),
            bar_bg_color: Color::srgba(0.2, 0.2, 0.25, 1.0),
            text_color: Color::WHITE,
        }
    }
}

// ── Keybind Row ─────────────────────────────────────────────────────────────

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct KeybindRowData {
    /// Action label (e.g. "Jump", "Attack").
    pub action: String,
    /// Key binding display text (e.g. "Space", "LMB").
    pub binding: String,
    pub label_color: Color,
    pub key_bg_color: Color,
    pub key_text_color: Color,
}

impl Default for KeybindRowData {
    fn default() -> Self {
        Self {
            action: "Jump".into(),
            binding: "Space".into(),
            label_color: Color::WHITE,
            key_bg_color: Color::srgba(0.25, 0.25, 0.3, 1.0),
            key_text_color: Color::WHITE,
        }
    }
}

// ── Settings Row ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum SettingsControlType {
    #[default]
    Toggle,
    Slider,
    Dropdown,
}

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SettingsRowData {
    pub label: String,
    pub control_type: SettingsControlType,
    /// Current value as a string representation.
    pub value: String,
    pub label_color: Color,
}

impl Default for SettingsRowData {
    fn default() -> Self {
        Self {
            label: "Setting".into(),
            control_type: SettingsControlType::Toggle,
            value: "On".into(),
            label_color: Color::WHITE,
        }
    }
}

// ── Separator ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum SeparatorDirection {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SeparatorData {
    pub direction: SeparatorDirection,
    pub thickness: f32,
    pub color: Color,
    /// Margin on each side in pixels.
    pub margin: f32,
}

impl Default for SeparatorData {
    fn default() -> Self {
        Self {
            direction: SeparatorDirection::Horizontal,
            thickness: 1.0,
            color: Color::srgba(0.4, 0.4, 0.45, 0.6),
            margin: 4.0,
        }
    }
}

// ── Number Input ───────────────────────────────────────────────────────────

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct NumberInputData {
    pub value: f64,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    /// Decimal places to display.
    pub precision: u32,
    pub label: String,
    pub text_color: Color,
    pub bg_color: Color,
    pub button_color: Color,
}

impl Default for NumberInputData {
    fn default() -> Self {
        Self {
            value: 0.0,
            min: 0.0,
            max: 100.0,
            step: 1.0,
            precision: 0,
            label: String::new(),
            text_color: Color::WHITE,
            bg_color: Color::srgba(0.15, 0.15, 0.18, 1.0),
            button_color: Color::srgba(0.25, 0.25, 0.3, 1.0),
        }
    }
}

// ── Vertical Slider ────────────────────────────────────────────────────────

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct VerticalSliderData {
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub track_color: Color,
    pub fill_color: Color,
    pub thumb_color: Color,
}

impl Default for VerticalSliderData {
    fn default() -> Self {
        Self {
            value: 0.5,
            min: 0.0,
            max: 1.0,
            track_color: Color::srgba(0.2, 0.2, 0.25, 1.0),
            fill_color: Color::srgba(0.3, 0.6, 1.0, 1.0),
            thumb_color: Color::WHITE,
        }
    }
}

// ── Scrollbar ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum ScrollbarOrientation {
    #[default]
    Vertical,
    Horizontal,
}

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct ScrollbarData {
    pub orientation: ScrollbarOrientation,
    /// Visible portion as fraction 0.0–1.0 (determines thumb size).
    pub viewport_fraction: f32,
    /// Scroll position 0.0–1.0.
    pub position: f32,
    pub track_color: Color,
    pub thumb_color: Color,
    pub thumb_hover_color: Color,
}

impl Default for ScrollbarData {
    fn default() -> Self {
        Self {
            orientation: ScrollbarOrientation::Vertical,
            viewport_fraction: 0.3,
            position: 0.0,
            track_color: Color::srgba(0.12, 0.12, 0.15, 0.8),
            thumb_color: Color::srgba(0.4, 0.4, 0.45, 0.8),
            thumb_hover_color: Color::srgba(0.5, 0.5, 0.55, 1.0),
        }
    }
}

// ── List ───────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct ListItem {
    pub label: String,
    pub selected: bool,
}

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct ListData {
    pub items: Vec<ListItem>,
    /// Allow multiple selection.
    pub multi_select: bool,
    pub item_height: f32,
    pub text_color: Color,
    pub selected_bg_color: Color,
    pub hover_bg_color: Color,
    pub bg_color: Color,
}

impl Default for ListData {
    fn default() -> Self {
        Self {
            items: vec![
                ListItem { label: "Item 1".into(), selected: false },
                ListItem { label: "Item 2".into(), selected: false },
                ListItem { label: "Item 3".into(), selected: false },
            ],
            multi_select: false,
            item_height: 28.0,
            text_color: Color::WHITE,
            selected_bg_color: Color::srgba(0.2, 0.4, 0.8, 0.5),
            hover_bg_color: Color::srgba(0.3, 0.3, 0.35, 0.5),
            bg_color: Color::srgba(0.12, 0.12, 0.15, 0.9),
        }
    }
}
