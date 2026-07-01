//! `enable_hair()` / `disable_hair()` script calls land here as
//! `renzora::ScriptAction`s (the same indirection `renzora_ragdoll` and
//! `renzora_physics` use) and just flip `Hair.active`. The simulation reads that
//! flag live every frame, so a script toggle and an Inspector checkbox edit
//! behave identically.

use crate::Hair;
use bevy::prelude::*;

pub fn handle_hair_script_actions(
    trigger: On<renzora::ScriptAction>,
    mut hairs: Query<&mut Hair>,
) {
    let action = trigger.event();
    let active = match action.name.as_str() {
        "enable_hair" => true,
        "disable_hair" => false,
        _ => return,
    };
    if let Ok(mut hair) = hairs.get_mut(action.entity) {
        hair.simulate = active;
    }
}
