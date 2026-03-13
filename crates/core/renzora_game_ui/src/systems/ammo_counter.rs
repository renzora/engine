//! Updates ammo counter text and color based on `AmmoCounterData`.

use bevy::prelude::*;

use crate::components::{AmmoCounterData, UiWidgetPart};

pub fn ammo_counter_system(
    counters: Query<(&AmmoCounterData, &Children), Changed<AmmoCounterData>>,
    mut texts: Query<(&UiWidgetPart, &mut bevy::ui::widget::Text, &mut TextColor)>,
) {
    for (data, children) in &counters {
        let color = if data.current <= data.low_threshold {
            data.low_color
        } else {
            data.color
        };
        for child in children.iter() {
            let Ok((part, mut text, mut tc)) = texts.get_mut(child) else {
                continue;
            };
            if part.role == "count" {
                text.0 = format!("{} / {}", data.current, data.max);
                tc.0 = color;
            }
        }
    }
}
