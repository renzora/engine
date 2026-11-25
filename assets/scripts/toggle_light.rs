//! Toggles a light on and off with a key, by flipping another entity's point
//! light intensity between zero and the configured brightness. The light is
//! found by name, so it need not be related to this entity.
//!
//! Setup: attach to any entity. The light is the entity named "lamp" with a
//! point light; press F to toggle it.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptInput};

const LIGHT_NAME: &str = "lamp";
const TOGGLE_KEY: KeyCode = KeyCode::KeyF;
/// Intensity while lit.
const BRIGHTNESS: f32 = 2000.0;

#[derive(Component)]
struct LampSwitch {
    lit: bool,
}

fn update(ctx: &mut ScriptCtx) {
    if !ctx.has::<LampSwitch>() {
        ctx.insert(LampSwitch { lit: true });
    }
    let pressed = ctx
        .get_resource::<ScriptInput>()
        .is_some_and(|input| input.is_key_just_pressed(TOGGLE_KEY));
    if !pressed {
        return;
    }
    let Some(mut switch) = ctx.get_mut::<LampSwitch>() else { return };
    switch.lit = !switch.lit;
    let lit = switch.lit;
    drop(switch);

    let Some(lamp) = find_named(ctx.world(), LIGHT_NAME) else { return };
    if let Some(mut light) = ctx.get_mut_on::<PointLight>(lamp) {
        light.intensity = if lit { BRIGHTNESS } else { 0.0 };
    }
}

/// The entity with this name (its snake_case id), if there is one.
fn find_named(world: &mut World, name: &str) -> Option<Entity> {
    let mut names = world.query::<(Entity, &Name)>();
    names.iter(world).find(|(_, n)| n.as_str() == name).map(|(e, _)| e)
}

renzora::script!(update);
