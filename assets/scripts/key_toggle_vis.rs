//! Space toggles the entity's visibility. It flips on the just-pressed edge,
//! because reading a held key would toggle every frame the key is down and
//! produce a flicker.
//!
//! Setup: attach to any visible entity.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptInput};

const TOGGLE_KEY: KeyCode = KeyCode::Space;

fn update(ctx: &mut ScriptCtx) {
    let pressed = ctx
        .get_resource::<ScriptInput>()
        .is_some_and(|input| input.is_key_just_pressed(TOGGLE_KEY));
    if !pressed {
        return;
    }
    let Some(mut visibility) = ctx.get_mut::<Visibility>() else { return };
    *visibility = match *visibility {
        Visibility::Hidden => Visibility::Inherited,
        _ => Visibility::Hidden,
    };
}

renzora::script!(update);
