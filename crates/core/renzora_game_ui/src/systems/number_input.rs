use bevy::prelude::*;

use crate::components::{NumberInputData, UiWidgetPart};

pub fn number_input_system(
    inputs: Query<(&NumberInputData, &Children), Changed<NumberInputData>>,
    mut text_query: Query<(&mut Text, &UiWidgetPart)>,
) {
    for (data, children) in &inputs {
        for child in children.iter() {
            if let Ok((mut text, part)) = text_query.get_mut(child) {
                if part.role == "value" {
                    let formatted = format!("{:.*}", data.precision as usize, data.value);
                    if **text != formatted {
                        **text = formatted;
                    }
                }
            }
        }
    }
}
