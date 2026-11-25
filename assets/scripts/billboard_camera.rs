//! A Y-axis billboard (sprite, health bar, grass card) that always faces the
//! camera. Copying the camera's yaw is cheaper and steadier than a look-at on
//! the camera position, and constraining to yaw keeps the quad upright
//! instead of tilting.
//!
//! The yaw comes from the active 3D camera. The Lua original read
//! `camera_yaw`, which nothing ever filled in, so it never turned; with the
//! real yaw a +Z-facing quad needs no offset, and `YAW_OFFSET` is 0.
//!
//! Setup: attach to a flat quad or plane facing +Z in its local space.

use bevy::prelude::*;
use renzora::{ScriptCtx, ScriptHook};

/// Degrees added to the camera yaw; 180 if the quad's front faces -Z.
const YAW_OFFSET: f32 = 0.0;
/// Idle bob for pickups and floating labels.
const BOB: bool = false;
const BOB_HEIGHT: f32 = 0.1;

#[derive(Component)]
struct Billboard {
    base_y: f32,
}

fn update(ctx: &mut ScriptCtx) {
    let elapsed = ctx.elapsed();
    let Some(base_y) = ctx.get::<Billboard>().map(|b| b.base_y) else { return };
    let Some(camera_yaw) = active_camera_yaw(ctx.world()) else { return };
    let Some(mut transform) = ctx.get_mut::<Transform>() else { return };
    transform.rotation = Quat::from_rotation_y(camera_yaw + YAW_OFFSET.to_radians());
    if BOB {
        transform.translation.y = base_y + (elapsed * 2.0).sin() * BOB_HEIGHT;
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::Ready = hook {
        let base_y = ctx.get::<Transform>().map_or(0.0, |t| t.translation.y);
        ctx.insert(Billboard { base_y });
    }
}

/// Yaw in radians of the first active 3D camera.
fn active_camera_yaw(world: &mut World) -> Option<f32> {
    let mut cameras = world.query_filtered::<(&Camera, &GlobalTransform), With<Camera3d>>();
    let (_, transform) = cameras.iter(world).find(|(camera, _)| camera.is_active)?;
    let (yaw, _, _) = transform.compute_transform().rotation.to_euler(EulerRot::YXZ);
    Some(yaw)
}

renzora::script!(update, hooks = hooks);
