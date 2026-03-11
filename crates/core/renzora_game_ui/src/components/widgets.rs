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
