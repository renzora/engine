//! Data components for each widget type.
//!
//! Each component stores the widget-specific state. Single-entity widgets
//! own their state directly; runtime systems read these and update bevy_ui
//! components on the same entity. (No more parent-walks-children-by-role
//! patterns — those caused round-trip serialization bugs.)

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

// ── Bar Fill ────────────────────────────────────────────────────────────────
//
// Single-entity primitive that maps a `value` in `[0, max]` to its `Node`'s
// width or height. Replaces the old ProgressBar / HealthBar / LoadingScreen
// pattern of "widget with role-marker child entities" — that pattern relied
// on a `bar_bg`-tagged child containing a `bar_fill`-tagged grandchild, and
// any serialization wobble would lose the role components.
//
// To build a "progress bar": a Container as the track (sets background,
// padding, border-radius) with one child entity carrying `UiBarFill`. Two
// single-entity widgets composed in the scene hierarchy. No internal child
// roles, nothing to lose on save/load.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum ProgressDirection {
    #[default]
    LeftToRight,
    RightToLeft,
    BottomToTop,
    TopToBottom,
}

/// Drives the entity's `Node` width or height from a 0..=max value.
///
/// Two sizing modes:
///
/// - **Pixel mode** (`max_px > 0`): `Node.width` (or height) is written as
///   `Val::Px(fraction * max_px)`. Works regardless of parent — the bar
///   always grows from 0 to `max_px` pixels. Use this when the bar is
///   positioned absolutely on the canvas alongside a visual track of the
///   same fixed size.
///
/// - **Percent mode** (`max_px == 0`): `Node.width` is written as
///   `Val::Percent(fraction * 100)`. Requires the bar to be a flex child
///   of a parent with a known size. Use this when nested inside a
///   Container with `position_type: Relative`.
///
/// Pixel mode is the default — it's robust against the canvas editor's
/// absolute-positioning model. Switch to Percent only when you've
/// authored a flex layout deliberately.
///
/// Scripting: `set_on("loadbar_fill", "UiBarFill.value", 0.42)` updates
/// the fill in place. The `apply_bar_fill` system rewrites `Node` on the
/// next frame.
#[derive(Component, Clone, Copy, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct UiBarFill {
    /// Current fill amount; clamped to `[0, max]`.
    pub value: f32,
    /// Maximum value `value` can reach. Defaults to `1.0` so callers can
    /// just write a fraction without thinking about scaling.
    pub max: f32,
    /// Which axis the fill grows along, and from which edge.
    pub direction: ProgressDirection,
    /// When > 0, the bar's pixel size grows from 0 to `max_px` driven by
    /// `value/max`. When 0, the bar uses Percent of its parent's size.
    /// Default is 200px so the bar is visible without further setup.
    pub max_px: f32,
}

impl Default for UiBarFill {
    fn default() -> Self {
        Self {
            value: 0.5,
            max: 1.0,
            direction: ProgressDirection::LeftToRight,
            max_px: 200.0,
        }
    }
}

impl UiBarFill {
    /// Fraction in `[0, 1]`. Returns `1.0` when `max <= 0` to avoid NaN.
    pub fn fraction(&self) -> f32 {
        if self.max <= 0.0 {
            return 1.0;
        }
        (self.value / self.max).clamp(0.0, 1.0)
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

// ── Image ──────────────────────────────────────────────────────────────────

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
