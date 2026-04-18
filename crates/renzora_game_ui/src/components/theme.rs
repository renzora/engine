//! Game UI theming — semantic color tokens for consistent widget styling.
//!
//! Widgets that carry the `UiThemed` marker component will have their colors
//! synced from the active `UiTheme` resource whenever it changes.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// The active game-UI color theme. Insert as a resource.
///
/// When this resource changes, the `ui_theme_system` will update all
/// `UiThemed` widget colors to match.
#[derive(Resource, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Resource)]
pub struct UiTheme {
    pub name: String,

    // ── Surface colors ─────────────────────────────────────────────────
    /// Default widget background.
    pub surface: Color,
    /// Slightly elevated surface (panels, cards).
    pub surface_raised: Color,
    /// Overlay/backdrop color (modals, tooltips).
    pub surface_overlay: Color,

    // ── Text colors ────────────────────────────────────────────────────
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_muted: Color,
    pub text_on_accent: Color,

    // ── Accent / interactive ───────────────────────────────────────────
    /// Primary accent (buttons, active tabs, selected radio, slider fill).
    pub accent: Color,
    /// Hovered accent.
    pub accent_hovered: Color,
    /// Pressed accent.
    pub accent_pressed: Color,

    // ── Semantic colors ────────────────────────────────────────────────
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,

    // ── Widget-specific tokens ─────────────────────────────────────────
    /// Border color for input widgets (checkbox box, slider track, etc.).
    pub border: Color,
    /// Track background (slider, toggle off state).
    pub track: Color,
    /// Thumb / knob color (slider, toggle).
    pub thumb: Color,
    /// Progress bar fill.
    pub progress_fill: Color,
    /// Health bar fill (normal).
    pub health_fill: Color,
    /// Health bar fill (low).
    pub health_low: Color,
    /// Toggle "on" color.
    pub toggle_on: Color,
    /// Toggle "off" color.
    pub toggle_off: Color,
    /// Scrollbar color.
    pub scrollbar: Color,
    /// Tooltip background.
    pub tooltip_bg: Color,
    /// Modal backdrop.
    pub modal_backdrop: Color,
    /// Window title bar.
    pub title_bar: Color,

    // ── Typography ─────────────────────────────────────────────────────
    pub font_size_sm: f32,
    pub font_size_md: f32,
    pub font_size_lg: f32,
    pub font_size_xl: f32,

    // ── Spacing / radius ───────────────────────────────────────────────
    pub border_radius: f32,
    pub border_width: f32,
    pub spacing: f32,
}

impl Default for UiTheme {
    fn default() -> Self {
        Self::dark()
    }
}

impl UiTheme {
    /// Dark theme — good for games with dark backgrounds.
    pub fn dark() -> Self {
        Self {
            name: "Dark".into(),
            surface: Color::srgba(0.12, 0.12, 0.15, 0.9),
            surface_raised: Color::srgba(0.18, 0.18, 0.22, 0.95),
            surface_overlay: Color::srgba(0.0, 0.0, 0.0, 0.5),
            text_primary: Color::WHITE,
            text_secondary: Color::srgba(0.7, 0.7, 0.7, 1.0),
            text_muted: Color::srgba(0.5, 0.5, 0.5, 1.0),
            text_on_accent: Color::WHITE,
            accent: Color::srgba(0.3, 0.5, 0.9, 1.0),
            accent_hovered: Color::srgba(0.4, 0.6, 1.0, 1.0),
            accent_pressed: Color::srgba(0.2, 0.4, 0.8, 1.0),
            success: Color::srgba(0.2, 0.8, 0.2, 1.0),
            warning: Color::srgba(0.9, 0.7, 0.1, 1.0),
            error: Color::srgba(0.9, 0.2, 0.2, 1.0),
            info: Color::srgba(0.3, 0.6, 0.9, 1.0),
            border: Color::srgba(0.35, 0.35, 0.4, 1.0),
            track: Color::srgba(0.25, 0.25, 0.25, 1.0),
            thumb: Color::WHITE,
            progress_fill: Color::srgba(0.3, 0.7, 0.3, 1.0),
            health_fill: Color::srgba(0.2, 0.8, 0.2, 1.0),
            health_low: Color::srgba(0.9, 0.2, 0.2, 1.0),
            toggle_on: Color::srgba(0.3, 0.7, 0.3, 1.0),
            toggle_off: Color::srgba(0.4, 0.4, 0.4, 1.0),
            scrollbar: Color::srgba(0.3, 0.3, 0.35, 0.6),
            tooltip_bg: Color::srgba(0.1, 0.1, 0.1, 0.95),
            modal_backdrop: Color::srgba(0.0, 0.0, 0.0, 0.5),
            title_bar: Color::srgba(0.2, 0.2, 0.25, 1.0),
            font_size_sm: 11.0,
            font_size_md: 14.0,
            font_size_lg: 18.0,
            font_size_xl: 24.0,
            border_radius: 4.0,
            border_width: 1.0,
            spacing: 8.0,
        }
    }

    /// Light theme — good for menus and casual games.
    pub fn light() -> Self {
        Self {
            name: "Light".into(),
            surface: Color::srgba(0.95, 0.95, 0.96, 0.95),
            surface_raised: Color::srgba(1.0, 1.0, 1.0, 1.0),
            surface_overlay: Color::srgba(0.0, 0.0, 0.0, 0.3),
            text_primary: Color::srgba(0.1, 0.1, 0.1, 1.0),
            text_secondary: Color::srgba(0.35, 0.35, 0.35, 1.0),
            text_muted: Color::srgba(0.55, 0.55, 0.55, 1.0),
            text_on_accent: Color::WHITE,
            accent: Color::srgba(0.2, 0.45, 0.85, 1.0),
            accent_hovered: Color::srgba(0.3, 0.55, 0.95, 1.0),
            accent_pressed: Color::srgba(0.15, 0.35, 0.75, 1.0),
            success: Color::srgba(0.15, 0.65, 0.15, 1.0),
            warning: Color::srgba(0.85, 0.65, 0.05, 1.0),
            error: Color::srgba(0.85, 0.15, 0.15, 1.0),
            info: Color::srgba(0.2, 0.5, 0.85, 1.0),
            border: Color::srgba(0.78, 0.78, 0.8, 1.0),
            track: Color::srgba(0.82, 0.82, 0.84, 1.0),
            thumb: Color::WHITE,
            progress_fill: Color::srgba(0.2, 0.65, 0.2, 1.0),
            health_fill: Color::srgba(0.15, 0.7, 0.15, 1.0),
            health_low: Color::srgba(0.85, 0.15, 0.15, 1.0),
            toggle_on: Color::srgba(0.2, 0.65, 0.2, 1.0),
            toggle_off: Color::srgba(0.7, 0.7, 0.72, 1.0),
            scrollbar: Color::srgba(0.65, 0.65, 0.68, 0.5),
            tooltip_bg: Color::srgba(0.2, 0.2, 0.22, 0.95),
            modal_backdrop: Color::srgba(0.0, 0.0, 0.0, 0.3),
            title_bar: Color::srgba(0.88, 0.88, 0.9, 1.0),
            font_size_sm: 11.0,
            font_size_md: 14.0,
            font_size_lg: 18.0,
            font_size_xl: 24.0,
            border_radius: 6.0,
            border_width: 1.0,
            spacing: 8.0,
        }
    }

    /// High-contrast theme — accessibility-focused.
    pub fn high_contrast() -> Self {
        Self {
            name: "High Contrast".into(),
            surface: Color::BLACK,
            surface_raised: Color::srgba(0.1, 0.1, 0.1, 1.0),
            surface_overlay: Color::srgba(0.0, 0.0, 0.0, 0.8),
            text_primary: Color::WHITE,
            text_secondary: Color::srgba(0.9, 0.9, 0.0, 1.0),
            text_muted: Color::srgba(0.7, 0.7, 0.7, 1.0),
            text_on_accent: Color::BLACK,
            accent: Color::srgba(1.0, 1.0, 0.0, 1.0),
            accent_hovered: Color::srgba(1.0, 1.0, 0.5, 1.0),
            accent_pressed: Color::srgba(0.8, 0.8, 0.0, 1.0),
            success: Color::srgba(0.0, 1.0, 0.0, 1.0),
            warning: Color::srgba(1.0, 0.8, 0.0, 1.0),
            error: Color::srgba(1.0, 0.0, 0.0, 1.0),
            info: Color::srgba(0.0, 0.8, 1.0, 1.0),
            border: Color::WHITE,
            track: Color::srgba(0.3, 0.3, 0.3, 1.0),
            thumb: Color::srgba(1.0, 1.0, 0.0, 1.0),
            progress_fill: Color::srgba(0.0, 1.0, 0.0, 1.0),
            health_fill: Color::srgba(0.0, 1.0, 0.0, 1.0),
            health_low: Color::srgba(1.0, 0.0, 0.0, 1.0),
            toggle_on: Color::srgba(0.0, 1.0, 0.0, 1.0),
            toggle_off: Color::srgba(0.5, 0.5, 0.5, 1.0),
            scrollbar: Color::WHITE,
            tooltip_bg: Color::srgba(0.0, 0.0, 0.2, 0.98),
            modal_backdrop: Color::srgba(0.0, 0.0, 0.0, 0.85),
            title_bar: Color::srgba(0.15, 0.15, 0.15, 1.0),
            font_size_sm: 13.0,
            font_size_md: 16.0,
            font_size_lg: 20.0,
            font_size_xl: 28.0,
            border_radius: 2.0,
            border_width: 2.0,
            spacing: 10.0,
        }
    }
}

impl UiTheme {
    /// Generate a `UiWidgetStyle` from theme tokens for the given widget type.
    pub fn widget_style(&self, widget_type: &super::UiWidgetType) -> super::style::UiWidgetStyle {
        use super::style::*;
        use super::UiWidgetType;

        let (fill, text_color) = match widget_type {
            UiWidgetType::Button => (UiFill::Solid(self.accent), self.text_on_accent),
            UiWidgetType::Image => (UiFill::Solid(self.surface_raised), self.text_primary),
            UiWidgetType::Panel | UiWidgetType::Container | UiWidgetType::ScrollView => {
                (UiFill::Solid(self.surface), self.text_primary)
            }
            UiWidgetType::Modal | UiWidgetType::DraggableWindow => {
                (UiFill::Solid(self.surface_raised), self.text_primary)
            }
            _ => (UiFill::Solid(self.surface), self.text_primary),
        };

        let stroke = UiStroke::new(self.border, self.border_width);
        let border_radius = UiBorderRadius::all(self.border_radius);

        UiWidgetStyle {
            fill,
            stroke,
            border_radius,
            shadow: None,
            opacity: 1.0,
            cursor: match widget_type {
                UiWidgetType::Button | UiWidgetType::Checkbox | UiWidgetType::Toggle
                | UiWidgetType::RadioButton | UiWidgetType::Slider | UiWidgetType::Dropdown => {
                    UiCursor::Pointer
                }
                UiWidgetType::TextInput => UiCursor::Text,
                _ => UiCursor::Default,
            },
            clip_content: matches!(
                widget_type,
                UiWidgetType::ProgressBar | UiWidgetType::HealthBar | UiWidgetType::ScrollView
            ),
            text: UiTextStyle {
                color: text_color,
                size: self.font_size_md,
                bold: matches!(widget_type, UiWidgetType::Button),
                italic: false,
                align: UiTextAlign::Center,
            },
            padding: match widget_type {
                UiWidgetType::Button => UiPadding::symmetric(4.0, 16.0),
                UiWidgetType::Panel | UiWidgetType::Container => UiPadding::all(self.spacing),
                _ => UiPadding::default(),
            },
        }
    }

    /// Generate themed `UiInteractionStyle` for interactive widgets.
    pub fn interaction_style(&self) -> super::UiInteractionStyle {
        use super::style::UiFill;
        use super::UiInteractionStyle;

        UiInteractionStyle {
            normal: Default::default(),
            hovered: super::style::UiStateStyle {
                fill: Some(UiFill::Solid(self.accent_hovered)),
                ..Default::default()
            },
            pressed: super::style::UiStateStyle {
                fill: Some(UiFill::Solid(self.accent_pressed)),
                ..Default::default()
            },
            disabled: super::style::UiStateStyle {
                opacity: Some(0.5),
                ..Default::default()
            },
        }
    }
}

/// Marker component: this widget's colors should follow the active `UiTheme`.
///
/// When the theme resource changes, all entities with `UiThemed` will have
/// their data component colors updated to match the theme tokens.
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct UiThemed;
