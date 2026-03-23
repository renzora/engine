//! Tab bar: switches the active tab on click.
//!
//! Each tab is a child entity with `UiWidgetPart { role: "tab_N" }`.
//! The system highlights the active tab and fires a change.

use bevy::prelude::*;

use crate::components::{TabBarData, UiWidgetPart};

pub fn tab_bar_system(
    tab_bars: Query<(&TabBarData, &Children), Changed<TabBarData>>,
    mut parts: Query<(&UiWidgetPart, &mut BackgroundColor)>,
) {
    for (data, children) in &tab_bars {
        // Update child tab colors
        for child in children.iter() {
            let Ok((part, mut bg)) = parts.get_mut(child) else {
                continue;
            };
            if let Some(idx_str) = part.role.strip_prefix("tab_") {
                if let Ok(idx) = idx_str.parse::<usize>() {
                    bg.0 = if idx == data.active {
                        data.active_color
                    } else {
                        data.tab_color
                    };
                }
            }
        }
    }
}
