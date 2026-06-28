//! Applies the active `UiTheme` to all `UiThemed` widgets when the theme changes.

use bevy::prelude::*;

use crate::game_ui::components::*;

/// When the `UiTheme` resource changes, update all themed widget styles.
pub fn ui_theme_system(
    theme: Res<UiTheme>,
    mut styled_widgets: Query<
        (
            &UiWidget,
            Option<&mut UiFill>,
            Option<&mut UiStroke>,
            Option<&mut UiBorderRadius>,
            Option<&mut UiOpacity>,
            Option<&mut UiClipContent>,
            Option<&mut UiCursor>,
            Option<&mut UiTextStyle>,
            Option<&mut UiPadding>,
            Option<&mut UiInteractionStyle>,
        ),
        With<UiThemed>,
    >,
    mut sliders: Query<(&mut SliderData, &UiThemed)>,
    mut checkboxes: Query<(&mut CheckboxData, &UiThemed)>,
    mut toggles: Query<(&mut ToggleData, &UiThemed)>,
    mut radios: Query<(&mut RadioButtonData, &UiThemed)>,
    mut tooltips: Query<(&mut TooltipData, &UiThemed)>,
    mut modals: Query<(&mut ModalData, &UiThemed)>,
    mut windows: Query<(&mut DraggableWindowData, &UiThemed)>,
) {
    if !theme.is_changed() {
        return;
    }

    // ── Only update style components that already exist on the entity ──
    for (widget, fill, stroke, border_radius, opacity, clip_content, cursor, text, padding, is) in
        &mut styled_widgets
    {
        let style = theme.widget_style(&widget.widget_type);

        if let Some(mut fill) = fill {
            *fill = style.fill.clone();
        }
        if let Some(mut stroke) = stroke {
            *stroke = style.stroke.clone();
        }
        if let Some(mut border_radius) = border_radius {
            *border_radius = style.border_radius;
        }
        if let Some(mut opacity) = opacity {
            opacity.0 = style.opacity;
        }
        if let Some(mut clip_content) = clip_content {
            clip_content.0 = style.clip_content;
        }
        if let Some(mut cursor) = cursor {
            *cursor = style.cursor;
        }
        if let Some(mut text) = text {
            *text = style.text.clone();
        }
        if let Some(mut padding) = padding {
            *padding = style.padding;
        }
        if let Some(mut is) = is {
            *is = theme.interaction_style();
        }
    }

    // ── Widget data component colors (not covered by style components) ──

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
}
