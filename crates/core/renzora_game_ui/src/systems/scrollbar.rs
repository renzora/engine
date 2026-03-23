use bevy::prelude::*;

use crate::components::{ScrollbarData, ScrollbarOrientation, UiWidgetPart};

pub fn scrollbar_system(
    bars: Query<(&ScrollbarData, &Children), Changed<ScrollbarData>>,
    mut node_query: Query<(&mut Node, &UiWidgetPart)>,
) {
    for (data, children) in &bars {
        let thumb_pct = (data.viewport_fraction.clamp(0.05, 1.0)) * 100.0;
        let pos_pct = data.position.clamp(0.0, 1.0) * (100.0 - thumb_pct);

        for child in children.iter() {
            if let Ok((mut node, part)) = node_query.get_mut(child) {
                if part.role == "thumb" {
                    match data.orientation {
                        ScrollbarOrientation::Vertical => {
                            node.height = Val::Percent(thumb_pct);
                            node.top = Val::Percent(pos_pct);
                            node.width = Val::Percent(100.0);
                        }
                        ScrollbarOrientation::Horizontal => {
                            node.width = Val::Percent(thumb_pct);
                            node.left = Val::Percent(pos_pct);
                            node.height = Val::Percent(100.0);
                        }
                    }
                }
            }
        }
    }
}
