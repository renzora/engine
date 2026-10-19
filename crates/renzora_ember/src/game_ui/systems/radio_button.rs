//! Radio button: group exclusivity — selecting one deselects others in the same group.

use bevy::prelude::*;

use crate::game_ui::components::RadioButtonData;
use crate::UiInteraction;

pub fn radio_button_system(
    mut buttons: Query<
        (Entity, &mut RadioButtonData, &UiInteraction, &Children),
        Changed<UiInteraction>,
    >,
    mut all_radios: Query<(Entity, &mut RadioButtonData), Without<UiInteraction>>,
) {
    // Collect clicked entities and their groups
    let mut clicked: Vec<(Entity, String)> = Vec::new();

    for (entity, mut data, interaction, _children) in &mut buttons {
        if *interaction == UiInteraction::Pressed && !data.selected {
            data.selected = true;
            clicked.push((entity, data.group.clone()));
        }
    }

    // Deselect others in the same group
    for (clicked_entity, group) in &clicked {
        // We need a second pass to deselect siblings — query all RadioButtonData
        // Since we can't query the same component mutably twice, we handle this
        // by iterating the full query excluding the UiInteraction filter
        for (entity, mut data) in &mut all_radios {
            if entity != *clicked_entity && data.group == *group && data.selected {
                data.selected = false;
            }
        }
    }
}
