//! Health bar: like progress bar but with low-health color change.

use bevy::prelude::*;

use crate::components::{HealthBarData, UiWidgetPart};

pub fn health_bar_system(
    bars: Query<(&HealthBarData, &Children), Changed<HealthBarData>>,
    mut parts: Query<(&UiWidgetPart, &mut Node, &mut BackgroundColor)>,
) {
    for (data, children) in &bars {
        let fraction = if data.max > 0.0 {
            (data.current / data.max).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let fill_color = if fraction <= data.low_threshold {
            data.low_color
        } else {
            data.fill_color
        };

        for child in children.iter() {
            let Ok((part, mut node, mut bg)) = parts.get_mut(child) else {
                continue;
            };
            match part.role.as_str() {
                "fill" => {
                    node.width = Val::Percent(fraction * 100.0);
                    node.height = Val::Percent(100.0);
                    bg.0 = fill_color;
                }
                "background" => {
                    bg.0 = data.bg_color;
                }
                _ => {}
            }
        }
    }
}
