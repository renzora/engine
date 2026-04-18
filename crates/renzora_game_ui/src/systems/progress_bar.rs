//! Drives the fill child of a ProgressBar based on `ProgressBarData.value`.

use bevy::prelude::*;

use crate::components::{ProgressBarData, ProgressDirection, UiWidgetPart};

pub fn progress_bar_system(
    bars: Query<(&ProgressBarData, &Children), Changed<ProgressBarData>>,
    mut parts: Query<(&UiWidgetPart, &mut Node, &mut BackgroundColor)>,
) {
    for (data, children) in &bars {
        let fraction = if data.max > 0.0 {
            (data.value / data.max).clamp(0.0, 1.0)
        } else {
            0.0
        };

        for child in children.iter() {
            let Ok((part, mut node, mut bg)) = parts.get_mut(child) else {
                continue;
            };

            match part.role.as_str() {
                "fill" => {
                    match data.direction {
                        ProgressDirection::LeftToRight | ProgressDirection::RightToLeft => {
                            node.width = Val::Percent(fraction * 100.0);
                            node.height = Val::Percent(100.0);
                        }
                        ProgressDirection::BottomToTop | ProgressDirection::TopToBottom => {
                            node.width = Val::Percent(100.0);
                            node.height = Val::Percent(fraction * 100.0);
                        }
                    }
                    bg.0 = data.fill_color;
                }
                "background" => {
                    bg.0 = data.bg_color;
                }
                _ => {}
            }
        }
    }
}
