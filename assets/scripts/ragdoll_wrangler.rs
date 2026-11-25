//! A mini "ragdoll": spawn limb primitives around a torso and keep it lively
//! with impulses. Real joints are not involved, so articulated chaos is faked:
//! the limbs are independent cubes, a periodic random impulse kicks the torso,
//! and each limb is eased back toward its rest offset on the torso every frame,
//! an attractor-style soft constraint.
//!
//! Physics is local; replicate it the way `net_position_sync.rs` does if peers must share it.
//!
//! Setup: attach to a physics-enabled body (the torso). The limbs are spawned
//! as `rag_head`, `rag_arm_l`, `rag_arm_r`, `rag_leg_l` and `rag_leg_r`.

use bevy::prelude::*;
use renzora::core::reflection::get_reflected_field;
use renzora::core::{MeshColor, MeshPrimitive};
use renzora::script_bridge::{PropertyValue, ScriptAction};
use renzora::{ScriptCtx, ScriptHook};

/// Each limb's name and its rest offset from the torso.
const LIMBS: [(&str, Vec3); 5] = [
    ("rag_head", Vec3::new(0.0, 1.2, 0.0)),
    ("rag_arm_l", Vec3::new(-0.7, 0.5, 0.0)),
    ("rag_arm_r", Vec3::new(0.7, 0.5, 0.0)),
    ("rag_leg_l", Vec3::new(-0.3, -1.0, 0.0)),
    ("rag_leg_r", Vec3::new(0.3, -1.0, 0.0)),
];
/// Seconds between random kicks.
const TWITCH_EVERY: f32 = 1.5;
/// How fast limbs ease back toward their rest offsets.
const SPRING: f32 = 6.0;

#[derive(Component)]
struct Ragdoll {
    /// Limb entities, in `LIMBS` order.
    limbs: Vec<Entity>,
    twitch: f32,
    seed: u64,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let me = ctx.entity();
    let Some(torso) = ctx.get::<Transform>().map(|t| t.translation) else { return };
    let Some(rag) = ctx.get::<Ragdoll>() else { return };
    let limbs = rag.limbs.clone();

    // Only X is pulled back, as the Lua version did: the limbs sway sideways
    // into place while gravity and kicks keep the other axes loose.
    let k = (dt * SPRING).clamp(0.0, 1.0);
    for (limb, (_, offset)) in limbs.iter().zip(LIMBS) {
        if let Some(mut t) = ctx.get_mut_on::<Transform>(*limb) {
            let target = torso.x + offset.x;
            t.translation.x += (target - t.translation.x) * k;
        }
    }

    let Some(mut rag) = ctx.get_mut::<Ragdoll>() else { return };
    rag.twitch -= dt;
    let kick = if rag.twitch <= 0.0 {
        rag.twitch += TWITCH_EVERY;
        let x = (rand01(&mut rag.seed) - 0.5) * 4.0;
        let y = rand01(&mut rag.seed) * 3.0;
        let z = (rand01(&mut rag.seed) - 0.5) * 4.0;
        Some(Vec3::new(x, y, z))
    } else {
        None
    };
    drop(rag);

    if let Some(kick) = kick {
        ScriptAction::new("apply_impulse", me)
            .arg("x", kick.x)
            .arg("y", kick.y)
            .arg("z", kick.z)
            .send(ctx.world());
        ScriptAction::new("screen_shake", me)
            .arg("intensity", 0.15)
            .arg("duration", 0.2)
            .send(ctx.world());
    }

    // A bounce-ish response while touching the ground.
    let grounded = matches!(
        get_reflected_field(ctx.world(), me, "PhysicsReadState", "grounded"),
        Some(PropertyValue::Bool(true))
    );
    if grounded {
        ScriptAction::new("apply_force", me)
            .arg("x", 0.0)
            .arg("y", 8.0)
            .arg("z", 0.0)
            .send(ctx.world());
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if !matches!(hook, ScriptHook::Ready) {
        return;
    }
    let me = ctx.entity();
    let seed = me.to_bits() ^ ((ctx.elapsed().to_bits() as u64) << 32);
    let torso = ctx.get::<Transform>().map(|t| t.translation).unwrap_or_default();
    let world = ctx.world();
    let limbs = LIMBS
        .iter()
        .map(|(name, offset)| {
            world
                .spawn((
                    Name::new(*name),
                    Transform::from_translation(torso + *offset),
                    Visibility::default(),
                    MeshPrimitive("cube".into()),
                    MeshColor(Color::srgb(0.9, 0.5, 0.4)),
                ))
                .id()
        })
        .collect();
    ctx.insert(Ragdoll { limbs, twitch: TWITCH_EVERY, seed });
}

/// A uniform value in 0..1 from a SplitMix64 step; scripts have no rand crate.
fn rand01(seed: &mut u64) -> f32 {
    *seed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *seed;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1u64 << 24) as f32
}

renzora::script!(update, hooks = hooks);
