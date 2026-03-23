use bevy::prelude::*;

use crate::components::{UiWidgetPart, VerticalSliderData};

pub fn vertical_slider_system(
    sliders: Query<(&VerticalSliderData, &Children), Changed<VerticalSliderData>>,
    children_query: Query<&Children>,
    mut node_query: Query<(&mut Node, &UiWidgetPart)>,
) {
    for (data, children) in &sliders {
        let range = data.max - data.min;
        let frac = if range > 0.0 {
            ((data.value - data.min) / range).clamp(0.0, 1.0)
        } else {
            0.0
        };

        for child in children.iter() {
            if let Ok((_, part)) = node_query.get(child) {
                if part.role == "track" {
                    if let Ok(grandchildren) = children_query.get(child) {
                        for gc in grandchildren.iter() {
                            if let Ok((mut node, gp)) = node_query.get_mut(gc) {
                                if gp.role == "fill" {
                                    node.height = Val::Percent(frac * 100.0);
                                }
                            }
                        }
                    }
                }
            }
            if let Ok((mut node, part)) = node_query.get_mut(child) {
                if part.role == "thumb" {
                    node.bottom = Val::Percent(frac * 100.0);
                }
            }
        }
    }
}
