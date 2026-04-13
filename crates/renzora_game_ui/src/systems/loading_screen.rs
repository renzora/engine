//! Syncs loading screen progress bar fill width and message text.

use bevy::prelude::*;

use crate::components::{LoadingScreenData, UiWidgetPart};

pub fn loading_screen_system(
    screens: Query<(&LoadingScreenData, &Children), Changed<LoadingScreenData>>,
    children_query: Query<&Children>,
    mut node_query: Query<(&mut Node, &UiWidgetPart)>,
    mut text_query: Query<(&mut Text, &UiWidgetPart)>,
) {
    for (data, children) in &screens {
        for child in children.iter() {
            // Update message text
            if let Ok((mut text, part)) = text_query.get_mut(child) {
                if part.role == "message" {
                    **text = data.message.clone();
                }
            }
            // Update bar fill
            if let Ok((_, part)) = node_query.get(child) {
                if part.role == "bar_bg" {
                    if let Ok(grandchildren) = children_query.get(child) {
                        for gc in grandchildren.iter() {
                            if let Ok((mut node, gp)) = node_query.get_mut(gc) {
                                if gp.role == "bar_fill" {
                                    node.width =
                                        Val::Percent(data.progress.clamp(0.0, 1.0) * 100.0);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
