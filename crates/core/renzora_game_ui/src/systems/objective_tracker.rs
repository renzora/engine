//! Rebuilds objective text when tracker data changes.

use bevy::prelude::*;

use crate::components::{ObjectiveStatus, ObjectiveTrackerData, UiWidgetPart};

pub fn objective_tracker_system(
    trackers: Query<(&ObjectiveTrackerData, &Children), Changed<ObjectiveTrackerData>>,
    mut text_query: Query<(&mut Text, &mut TextColor, &UiWidgetPart)>,
) {
    for (data, children) in &trackers {
        let mut obj_idx = 0usize;
        for child in children.iter() {
            if let Ok((mut text, mut color, part)) = text_query.get_mut(child) {
                if part.role == "title" {
                    **text = data.title.clone();
                    color.0 = data.title_color;
                } else if part.role == "objective" {
                    if let Some(obj) = data.objectives.get(obj_idx) {
                        let prefix = match obj.status {
                            ObjectiveStatus::Active => "\u{25cb} ",
                            ObjectiveStatus::Completed => "\u{25cf} ",
                            ObjectiveStatus::Failed => "\u{2715} ",
                        };
                        let progress_str = match obj.progress {
                            Some((cur, max)) => format!(" ({}/{})", cur, max),
                            None => String::new(),
                        };
                        **text = format!("{}{}{}", prefix, obj.label, progress_str);
                        color.0 = match obj.status {
                            ObjectiveStatus::Active => data.active_color,
                            ObjectiveStatus::Completed => data.completed_color,
                            ObjectiveStatus::Failed => data.failed_color,
                        };
                        obj_idx += 1;
                    }
                }
            }
        }
    }
}
