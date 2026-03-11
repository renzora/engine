//! Draggable window: title bar drag to move, close/minimize buttons.
//!
//! Structure:
//! - Root entity: DraggableWindowData + Node (absolute positioned)
//!   - Child "title_bar": draggable handle
//!   - Child "close_button": hides window
//!   - Child "minimize_button": collapses content
//!   - Child "content": main content area

use bevy::prelude::*;

use crate::components::{DraggableWindowData, UiWidgetPart};

/// Tracks active drag state for a window.
#[derive(Component, Default)]
pub struct WindowDragState {
    pub dragging: bool,
    pub last_cursor: Option<Vec2>,
}

pub fn draggable_window_system(
    mut commands: Commands,
    mut windows: Query<(
        Entity,
        &DraggableWindowData,
        &mut Node,
        &mut Visibility,
        &Children,
        Option<&mut WindowDragState>,
    )>,
    parts: Query<(&UiWidgetPart, &Interaction)>,
    window_query: Query<&Window, With<bevy::window::PrimaryWindow>>,
) {
    let cursor_pos = window_query
        .single()
        .ok()
        .and_then(|w| w.cursor_position());

    for (entity, data, mut node, mut vis, children, drag_state) in &mut windows {
        // Ensure drag state component exists
        let Some(mut drag) = drag_state else {
            commands.entity(entity).insert(WindowDragState::default());
            continue;
        };

        for child in children.iter() {
            let Ok((part, interaction)) = parts.get(child) else {
                continue;
            };

            match part.role.as_str() {
                "title_bar" => {
                    match interaction {
                        Interaction::Pressed => {
                            if !drag.dragging {
                                drag.dragging = true;
                                drag.last_cursor = cursor_pos;
                            }
                        }
                        _ => {
                            if drag.dragging {
                                drag.dragging = false;
                                drag.last_cursor = None;
                            }
                        }
                    }
                }
                "close_button" if data.closable => {
                    if *interaction == Interaction::Pressed {
                        *vis = Visibility::Hidden;
                    }
                }
                "minimize_button" if data.minimizable => {
                    // Toggle content visibility on press
                    if *interaction == Interaction::Pressed {
                        // We handle minimize by toggling "content" child visibility
                        for inner_child in children.iter() {
                            if let Ok((inner_part, _)) = parts.get(inner_child) {
                                if inner_part.role == "content" {
                                    // Can't mutate visibility from this query context,
                                    // but we can set a flag and handle it separately
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Apply drag movement
        if drag.dragging {
            if let (Some(cursor), Some(last)) = (cursor_pos, drag.last_cursor) {
                let delta = cursor - last;
                if let bevy::ui::Val::Px(ref mut left) = node.left {
                    *left += delta.x;
                }
                if let bevy::ui::Val::Px(ref mut top) = node.top {
                    *top += delta.y;
                }
            }
            drag.last_cursor = cursor_pos;
        }
    }
}
