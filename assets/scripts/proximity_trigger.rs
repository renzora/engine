//! A one-shot reaction the moment something starts touching this entity.
//! Contact is a level (true every frame while overlapping), so the previous
//! state is kept and the effect fires only on the rising edge.
//!
//! The Lua version read `is_colliding`, which nothing filled in, so it never
//! fired. This reads the body's physics contacts, which every backend fills.
//!
//! Setup: attach to an entity with a physics body and a sensor or collider.

use bevy::prelude::*;
use renzora::physics::PhysicsContacts;
use renzora::script_bridge::ScriptAction;
use renzora::ScriptCtx;

const ENTER_SOUND: &str = "audio/trigger.ogg";
const ACTIVE_COLOR: [f32; 3] = [1.0, 0.85, 0.2];
const IDLE_COLOR: [f32; 3] = [0.6, 0.6, 0.6];

#[derive(Component)]
struct Trigger {
    was_touching: bool,
}

fn update(ctx: &mut ScriptCtx) {
    if !ctx.has::<Trigger>() {
        ctx.insert(Trigger { was_touching: false });
    }
    let touching = ctx.get::<PhysicsContacts>().is_some_and(|c| !c.0.is_empty());
    let Some(mut trigger) = ctx.get_mut::<Trigger>() else { return };
    let was = trigger.was_touching;
    trigger.was_touching = touching;
    drop(trigger);

    let me = ctx.entity();
    if touching && !was {
        ScriptAction::new("play_sound", me)
            .arg("path", ENTER_SOUND)
            .arg("volume", 0.8)
            .arg("looping", false)
            .arg("bus", "Sfx")
            .send(ctx.world());
        set_color(ctx, ACTIVE_COLOR);
        let name = ctx.name().unwrap_or("unnamed").to_string();
        renzora::clog_info!("Script", "Proximity trigger entered: {name}");
    } else if !touching && was {
        set_color(ctx, IDLE_COLOR);
    }
}

fn set_color(ctx: &mut ScriptCtx, [r, g, b]: [f32; 3]) {
    let me = ctx.entity();
    ScriptAction::new("set_material_color", me)
        .arg("r", r)
        .arg("g", g)
        .arg("b", b)
        .arg("a", 1.0)
        .send(ctx.world());
}

renzora::script!(update);
