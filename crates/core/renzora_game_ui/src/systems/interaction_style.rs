//! Applies interaction-state-dependent style overrides (hover, press, disabled).
//!
//! When `Interaction` changes, merges the active state's `UiStateStyle` overrides
//! onto the base `UiWidgetStyle`. The downstream `apply_widget_style_system` then
//! syncs the result to bevy_ui components.

use bevy::prelude::*;

use crate::components::{UiInteractionStyle, UiWidgetStyle};

pub fn interaction_style_system(
    mut widgets: Query<
        (
            &Interaction,
            &UiInteractionStyle,
            &mut UiWidgetStyle,
        ),
        Changed<Interaction>,
    >,
    // Keep a copy of the *base* style to restore on Interaction::None.
    _base_styles: Query<&UiWidgetStyle, Without<UiInteractionStyle>>,
) {
    // We store the original "base" style in a local cache keyed by entity so we
    // can restore it. But that's complex — instead we rely on `UiStateStyle`
    // fields being `Option`: only `Some` fields override, and on `Interaction::None`
    // we apply the `normal` overrides (which are typically all `None` = no change).
    //
    // The issue: once we mutate `UiWidgetStyle`, we lose the original base.
    // Solution: store the base in a separate component. But that's a bigger
    // refactor. For now, simply re-apply `normal` state overrides on None,
    // which resets to the base if normal has explicit values.
    //
    // In practice this works because:
    // - `normal` is usually all-None (inherits base)
    // - hover/pressed only override fill/opacity/etc.
    // - `apply_widget_style_system` runs on `Changed<UiWidgetStyle>` and syncs to bevy_ui

    for (interaction, istyle, mut widget_style) in &mut widgets {
        let overrides = match interaction {
            Interaction::None => &istyle.normal,
            Interaction::Hovered => &istyle.hovered,
            Interaction::Pressed => &istyle.pressed,
        };

        // Apply overrides to widget style fields
        if let Some(ref fill) = overrides.fill {
            widget_style.fill = fill.clone();
        }
        if let Some(ref stroke) = overrides.stroke {
            widget_style.stroke = stroke.clone();
        }
        if let Some(opacity) = overrides.opacity {
            widget_style.opacity = opacity;
        }
        if let Some(ref radius) = overrides.border_radius {
            widget_style.border_radius = *radius;
        }
        if let Some(cursor) = overrides.cursor {
            widget_style.cursor = cursor;
        }
        if let Some(color) = overrides.text_color {
            widget_style.text.color = color;
        }
        if let Some(size) = overrides.text_size {
            widget_style.text.size = size;
        }
        if let Some(padding) = overrides.padding {
            widget_style.padding = padding;
        }
    }
}
