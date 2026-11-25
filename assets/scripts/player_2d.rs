//! A 2D character controller with a follow camera. WASD or the arrows move
//! the sprite on the XY plane (Z is its draw depth, so it is left alone); a
//! left gamepad stick works too. The camera eases toward the sprite.
//!
//! The amber "camera boundary" in the 2D viewport does not move: it is the
//! fixed project window frame at the world origin. The follow shows when you
//! press Play, since the render camera is driven from the Camera 2D's
//! transform.
//!
//! Centering: a 2D camera's position is the top-left of its view, not the
//! middle, so it is offset by half the visible extent: the project resolution
//! scaled by the camera's zoom.
//!
//! Setup: attach to the sprite. The Camera 2D entity is named "camera".

use bevy::prelude::*;
use renzora::core::project_config::CurrentProject;
use renzora::{ScriptCtx, ScriptInput};

/// Pixels per second.
const SPEED: f32 = 300.0;
const CAMERA: &str = "camera";
/// Camera catch-up rate: higher is snappier, 0 locks it to the sprite.
const FOLLOW_SMOOTH: f32 = 8.0;
/// Must match the Camera 2D's orthographic scale.
const ZOOM: f32 = 1.0;
const USE_GAMEPAD: bool = true;

/// Set once the missing-camera warning has been shown.
#[derive(Component)]
struct Warned;

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let Some(input) = ctx.get_resource::<ScriptInput>() else { return };
    // Already normalized on the diagonals, so straight and diagonal feel the same.
    let mut m = input.get_movement_vector();
    // Stick and keys share axes: take whichever pushed harder rather than
    // adding them, which would let a half-held stick plus a key exceed full
    // speed. Stick Y is up-positive, like W.
    if USE_GAMEPAD {
        if let Some(pad) = input.first_gamepad() {
            let stick = input.get_gamepad_left_stick(pad);
            if stick.x.abs() > m.x.abs() {
                m.x = stick.x;
            }
            if stick.y.abs() > m.y.abs() {
                m.y = stick.y;
            }
        }
    }

    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    transform.translation += Vec3::new(m.x, m.y, 0.0) * SPEED * dt;
    let player = transform.translation;
    drop(transform);

    let (width, height) = ctx
        .get_resource::<CurrentProject>()
        .map_or((1920.0, 1080.0), |p| {
            (p.config.viewport.width.max(1) as f32, p.config.viewport.height.max(1) as f32)
        });

    let Some(camera) = find_named(ctx.world(), CAMERA) else {
        if !ctx.has::<Warned>() {
            renzora::clog_warn!(
                "Script",
                "player_2d: no entity named '{CAMERA}'; name your Camera 2D entity that, or change CAMERA."
            );
            ctx.insert(Warned);
        }
        return;
    };

    // World Y points up, so the top-left corner sits half a view left and up.
    let target = Vec2::new(player.x - width * ZOOM * 0.5, player.y + height * ZOOM * 0.5);
    // Framerate independent because the blend is derived from delta.
    let t = if FOLLOW_SMOOTH > 0.0 { 1.0 - (-FOLLOW_SMOOTH * dt).exp() } else { 1.0 };
    let Some(mut cam) = ctx.get_mut_on::<Transform>(camera) else { return };
    // Only X and Y: the camera's Z is its 2D depth, kept as authored.
    cam.translation.x += (target.x - cam.translation.x) * t;
    cam.translation.y += (target.y - cam.translation.y) * t;
}

/// The entity with this name (its snake_case id), if there is one.
fn find_named(world: &mut World, name: &str) -> Option<Entity> {
    let mut names = world.query::<(Entity, &Name)>();
    names.iter(world).find(|(_, n)| n.as_str() == name).map(|(e, _)| e)
}

renzora::script!(update);
