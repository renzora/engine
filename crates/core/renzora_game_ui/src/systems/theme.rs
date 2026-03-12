//! Applies the active `UiTheme` to all `UiThemed` widgets when the theme changes.

use bevy::prelude::*;

use crate::components::*;

/// When the `UiTheme` resource changes, update all themed widget styles.
pub fn ui_theme_system(
    theme: Res<UiTheme>,
    mut styled_widgets: Query<
        (
            &UiWidget,
            &UiThemed,
            Option<&mut UiWidgetStyle>,
            Option<&mut UiInteractionStyle>,
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
) {
    if !theme.is_changed() {
        return;
    }

    // ── UiWidgetStyle + UiInteractionStyle from theme tokens ──
    for (widget, _themed, ws, is) in &mut styled_widgets {
        if let Some(mut ws) = ws {
            *ws = theme.widget_style(&widget.widget_type);
        }
        if let Some(mut is) = is {
            *is = theme.interaction_style();
        }
    }

    // ── Widget data component colors (not covered by UiWidgetStyle) ──

    for (mut data, _themed) in &mut progress_bars {
        data.fill_color = theme.progress_fill;
        data.bg_color = theme.surface;
    }

    for (mut data, _themed) in &mut health_bars {
        data.fill_color = theme.health_fill;
        data.low_color = theme.health_low;
        data.bg_color = theme.surface;
    }

    for (mut data, _themed) in &mut sliders {
        data.track_color = theme.track;
        data.fill_color = theme.accent;
        data.thumb_color = theme.thumb;
    }

    for (mut data, _themed) in &mut checkboxes {
        data.check_color = theme.accent;
        data.box_color = theme.surface;
    }

    for (mut data, _themed) in &mut toggles {
        data.on_color = theme.toggle_on;
        data.off_color = theme.toggle_off;
        data.knob_color = theme.thumb;
    }

    for (mut data, _themed) in &mut radios {
        data.active_color = theme.accent;
    }

    for (mut data, _themed) in &mut spinners {
        data.color = theme.accent;
    }

    for (mut data, _themed) in &mut tooltips {
        data.bg_color = theme.tooltip_bg;
        data.text_color = theme.text_primary;
    }

    for (mut data, _themed) in &mut modals {
        data.backdrop_color = theme.modal_backdrop;
    }

    for (mut data, _themed) in &mut windows {
        data.title_bar_color = theme.title_bar;
    }

    for (mut data, _themed) in &mut tab_bars {
        data.tab_color = theme.surface;
        data.active_color = theme.accent;
    }
}
