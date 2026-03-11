//! Slider: interactive track + thumb driven by pointer interaction.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use crate::components::{SliderData, UiWidgetPart};

/// Updates the slider value when the track is dragged, and positions the thumb.
pub fn slider_system(
    mut sliders: Query<(&mut SliderData, &Interaction, &Children, &ComputedNode)>,
    mut parts: Query<(&UiWidgetPart, &mut Node, &mut BackgroundColor)>,
    relative_cursor: Query<&RelativeCursorPosition>,
) {
    for (mut data, interaction, children, _computed) in &mut sliders {
        // Handle drag on slider track
        if *interaction == Interaction::Pressed {
            if let Ok(cursor) = relative_cursor.get(children[0]) {
                if let Some(pos) = cursor.normalized {
                    // pos.x is -0.5..0.5, map to 0..1
                    let t = (pos.x + 0.5).clamp(0.0, 1.0);
                    let mut new_val = data.min + t * (data.max - data.min);
                    if data.step > 0.0 {
                        new_val = (new_val / data.step).round() * data.step;
                    }
                    data.value = new_val.clamp(data.min, data.max);
                }
            }
        }

        // Update visual children
        let fraction = if (data.max - data.min).abs() > f32::EPSILON {
            ((data.value - data.min) / (data.max - data.min)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        for child in children.iter() {
            let Ok((part, mut node, mut bg)) = parts.get_mut(child) else {
                continue;
            };
            match part.role.as_str() {
                "track" => {
                    bg.0 = data.track_color;
                }
                "fill" => {
                    node.width = Val::Percent(fraction * 100.0);
                    bg.0 = data.fill_color;
                }
                "thumb" => {
                    node.left = Val::Percent(fraction * 100.0);
                    bg.0 = data.thumb_color;
                }
                _ => {}
            }
        }
    }
}
