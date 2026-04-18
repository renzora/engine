//! Tooltip: shows/hides tooltip content after a hover delay.
//!
//! The tooltip widget has a child with `UiWidgetPart { role: "content" }`
//! that is shown when the parent is hovered for `delay_ms` milliseconds.

use bevy::prelude::*;

use crate::components::{TooltipData, UiWidgetPart};

/// Tracks how long the tooltip has been hovered.
#[derive(Component, Default)]
pub struct TooltipHoverTimer {
    pub elapsed_ms: f32,
    pub showing: bool,
}

pub fn tooltip_system(
    mut commands: Commands,
    time: Res<Time>,
    mut tooltips: Query<(Entity, &TooltipData, &Interaction, &Children, Option<&mut TooltipHoverTimer>)>,
    mut parts: Query<(&UiWidgetPart, &mut Visibility)>,
) {
    let dt_ms = time.delta_secs() * 1000.0;

    for (entity, data, interaction, children, timer) in &mut tooltips {
        let is_hovered = *interaction == Interaction::Hovered || *interaction == Interaction::Pressed;

        match timer {
            Some(mut timer) => {
                if is_hovered {
                    timer.elapsed_ms += dt_ms;
                    if timer.elapsed_ms >= data.delay_ms as f32 && !timer.showing {
                        timer.showing = true;
                        set_content_visibility(children, &mut parts, Visibility::Inherited);
                    }
                } else {
                    timer.elapsed_ms = 0.0;
                    if timer.showing {
                        timer.showing = false;
                        set_content_visibility(children, &mut parts, Visibility::Hidden);
                    }
                }
            }
            None => {
                // First time — insert timer component
                commands.entity(entity).try_insert(TooltipHoverTimer::default());
                set_content_visibility(children, &mut parts, Visibility::Hidden);
            }
        }
    }
}

fn set_content_visibility(
    children: &Children,
    parts: &mut Query<(&UiWidgetPart, &mut Visibility)>,
    vis: Visibility,
) {
    for child in children.iter() {
        if let Ok((part, mut v)) = parts.get_mut(child) {
            if part.role == "content" {
                *v = vis;
            }
        }
    }
}
