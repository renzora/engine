//! Applies the active `UiTheme` to all `UiThemed` widgets when the theme changes.

use bevy::prelude::*;

use crate::components::*;

/// When the `UiTheme` resource changes, update all themed widget colors.
pub fn ui_theme_system(
    theme: Res<UiTheme>,
    mut bg_query: Query<
        (
            &UiWidget,
            &UiThemed,
            Option<&mut BackgroundColor>,
            Option<&mut BorderColor>,
        ),
    >,
    mut progress_bars: Query<(&mut ProgressBarData, &UiThemed)>,
    mut health_bars: Query<(&mut HealthBarData, &UiThemed)>,
    mut sliders: Query<(&mut SliderData, &UiThemed)>,
    mut checkboxes: Query<(&mut CheckboxData, &UiThemed)>,
    mut toggles: Query<(&mut ToggleData, &UiThemed)>,
    mut radios: Query<(&mut RadioButtonData, &UiThemed)>,
    mut spinners: Query<(&mut SpinnerData, &UiThemed)>,
    mut tooltips: Query<(&mut TooltipData, &UiThemed)>,
    mut modals: Query<(&mut ModalData, &UiThemed)>,
    mut windows: Query<(&mut DraggableWindowData, &UiThemed)>,
    mut tab_bars: Query<(&mut TabBarData, &UiThemed)>,
    mut text_colors: Query<(&mut TextColor, &UiThemed, &UiWidget)>,
    mut interaction_styles: Query<(&mut UiInteractionStyle, &UiThemed)>,
) {
    if !theme.is_changed() {
        return;
    }

    // Background / border colors based on widget type
    for (widget, _themed, bg, border) in &mut bg_query {
        if let Some(mut bg) = bg {
            bg.0 = match widget.widget_type {
                UiWidgetType::Panel | UiWidgetType::Container | UiWidgetType::ScrollView => {
                    theme.surface
                }
                UiWidgetType::Button => theme.accent,
                UiWidgetType::Image => theme.surface_raised,
                _ => theme.surface,
            };
        }
        if let Some(mut bc) = border {
            *bc = BorderColor::all(theme.border);
        }
    }

    // Text colors
    for (mut tc, _themed, widget) in &mut text_colors {
        tc.0 = match widget.widget_type {
            UiWidgetType::Button => theme.text_on_accent,
            _ => theme.text_primary,
        };
    }

    // Interaction styles (buttons, checkboxes, etc.)
    for (mut style, _themed) in &mut interaction_styles {
        style.normal.bg_color = Some(theme.accent);
        style.hovered.bg_color = Some(theme.accent_hovered);
        style.pressed.bg_color = Some(theme.accent_pressed);
    }

    // Progress bars
    for (mut data, _themed) in &mut progress_bars {
        data.fill_color = theme.progress_fill;
        data.bg_color = theme.surface;
    }

    // Health bars
    for (mut data, _themed) in &mut health_bars {
        data.fill_color = theme.health_fill;
        data.low_color = theme.health_low;
        data.bg_color = theme.surface;
    }

    // Sliders
    for (mut data, _themed) in &mut sliders {
        data.track_color = theme.track;
        data.fill_color = theme.accent;
        data.thumb_color = theme.thumb;
    }

    // Checkboxes
    for (mut data, _themed) in &mut checkboxes {
        data.check_color = theme.accent;
        data.box_color = theme.surface;
    }

    // Toggles
    for (mut data, _themed) in &mut toggles {
        data.on_color = theme.toggle_on;
        data.off_color = theme.toggle_off;
        data.knob_color = theme.thumb;
    }

    // Radio buttons
    for (mut data, _themed) in &mut radios {
        data.active_color = theme.accent;
    }

    // Spinners
    for (mut data, _themed) in &mut spinners {
        data.color = theme.accent;
    }

    // Tooltips
    for (mut data, _themed) in &mut tooltips {
        data.bg_color = theme.tooltip_bg;
        data.text_color = theme.text_primary;
    }

    // Modals
    for (mut data, _themed) in &mut modals {
        data.backdrop_color = theme.modal_backdrop;
    }

    // Draggable windows
    for (mut data, _themed) in &mut windows {
        data.title_bar_color = theme.title_bar;
    }

    // Tab bars
    for (mut data, _themed) in &mut tab_bars {
        data.tab_color = theme.surface;
        data.active_color = theme.accent;
    }
}
