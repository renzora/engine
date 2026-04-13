//! Typewriter effect for dialog box text reveal.

use bevy::prelude::*;

use crate::components::{DialogBoxData, UiWidgetPart};

pub fn dialog_box_system(
    time: Res<Time>,
    mut dialogs: Query<(&mut DialogBoxData, &Children)>,
    mut text_query: Query<(&mut Text, &UiWidgetPart)>,
) {
    for (mut data, children) in &mut dialogs {
        if data.chars_per_second <= 0.0 {
            data.chars_revealed = data.text.len();
        } else if data.chars_revealed < data.text.len() {
            data.elapsed += time.delta_secs();
            let new_chars = (data.elapsed * data.chars_per_second) as usize;
            data.chars_revealed = new_chars.min(data.text.len());
        }

        for child in children.iter() {
            if let Ok((mut text, part)) = text_query.get_mut(child) {
                if part.role == "text" {
                    let revealed: String = data.text.chars().take(data.chars_revealed).collect();
                    if **text != revealed {
                        **text = revealed;
                    }
                }
            }
        }
    }
}
