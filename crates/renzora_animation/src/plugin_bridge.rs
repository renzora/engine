//! The animation half of the standalone-plugin boundary.
//!
//! Two halves meet here, and the boundary between them does not know either:
//!
//! * `renzora_plugin::sys` is the frozen mechanism. It carries a service call's
//!   bytes without reading them, and has never heard of animation.
//! * [`renzora_plugin::anim`] is the animation *vocabulary*: [`AnimOp`],
//!   [`AnimCommand`], [`AnimState`]. Bevy-free and dependency-free, so the same
//!   definitions compile into a plugin and into this crate — which is what lets
//!   one definition of each type serve both sides.
//!
//! This module is the engine side. Adding another domain — audio, physics —
//! means a module beside `anim.rs` behind its own feature, plus a module like
//! this one in whichever crate owns that domain. Neither touches `sys`, so
//! neither moves the ABI version.
//!
//! ## Writes: draining a service
//!
//! A plugin's `commands.entity(e).play_animation("run")` encodes an
//! [`AnimCommand`] and hands it to the host tagged with [`renzora_plugin::anim::SERVICE`].
//! [`drain_plugin_anim_commands`] takes the calls bearing that tag — and only
//! those, so a second bridge's calls are left alone — and turns each into a real
//! [`AnimationCommand`], or a [`ProceduralTween`] insert for the tween ops. By
//! the time anything acts on one it is indistinguishable from a script's.
//!
//! ## Reads: a mirrored component
//!
//! [`AnimatorReadState`] is the script-visible mirror and cannot cross: `String`
//! and `HashMap` have no C layout, and a plugin has no type for either.
//! [`PluginAnimState`] is a second, numeric-only mirror that can, and it goes
//! over as an ordinary query cell — so a plugin reading animation state every
//! frame makes **no** calls back into the engine.
//!
//! It is `#[repr(transparent)]` over the very [`AnimState`] the plugin reads, so
//! the two layouts cannot drift. Declaring a struct with matching fields would
//! have worked until someone reordered one of them, and a field-order mismatch
//! here is not a compile error on either side — it is a plugin reading
//! `state_time` out of `time`.

use bevy::prelude::*;

use renzora_plugin::anim::{AnimCommand, AnimOp, AnimState, Easing};
use renzora_plugin::host::PluginServiceCalls;

use crate::read_state::AnimatorReadState;
use crate::systems::{AnimationCommand, AnimationCommandQueue};
use crate::tween::{EasingFunction, ProceduralTween, TweenProperty};

/// Numeric animator state, readable from a standalone plugin.
///
/// Plugins name this type by the string in `renzora_plugin::anim`'s
/// `host_component!` call, so **renaming this type or moving this module breaks
/// every plugin's animation reads.** Nothing on either side fails to compile if
/// the two stop matching, which is why [`install`] asserts they agree.
///
/// Reflected as **opaque**, which is the only option: [`AnimState`] lives in a
/// Bevy-free module and cannot derive `Reflect`. Nothing needs to see inside it
/// reflectively — scripts read [`AnimatorReadState`], which exists for that —
/// but the type must be *registered*, because a plugin's host component is
/// resolved through `AppTypeRegistry::get_with_type_path`.
#[derive(Component, Clone, Copy, Default, Reflect)]
#[reflect(Component, opaque)]
#[repr(transparent)]
pub struct PluginAnimState(pub AnimState);

/// Keeps [`PluginAnimState`] on every entity that has an [`AnimatorReadState`].
///
/// Hangs off the read-state mirror rather than off `AnimatorComponent` so both
/// mirrors describe the same frame: `update_animator_read_state` populates one
/// and this copies from it, in that order.
pub fn sync_plugin_anim_state(
    mut commands: Commands,
    mut q: Query<(Entity, &AnimatorReadState, Option<&mut PluginAnimState>)>,
) {
    for (entity, read, existing) in &mut q {
        let next = AnimState {
            // An empty name hashes to the FNV offset basis, not to 0, so
            // "nothing playing" is spelled out rather than left to collide. A
            // plugin comparing against `is_clip("")` would otherwise match an
            // idle animator.
            clip: if read.current_clip.is_empty() {
                0
            } else {
                renzora_plugin::anim::name_hash(&read.current_clip)
            },
            state: if read.current_state.is_empty() {
                0
            } else {
                renzora_plugin::anim::name_hash(&read.current_state)
            },
            state_time: read.state_time,
            time: read.time,
            playing: read.playing as u32,
            _reserved: 0,
        };
        match existing {
            Some(mut slot) => slot.0 = next,
            None => {
                commands.entity(entity).try_insert(PluginAnimState(next));
            }
        }
    }
}

/// Translate parked plugin service calls into engine animation commands.
///
/// Ordered before `process_animation_commands` so a plugin's operation lands in
/// the same frame it was issued, exactly like a script's.
pub fn drain_plugin_anim_commands(
    mut parked: ResMut<PluginServiceCalls>,
    mut queue: ResMut<AnimationCommandQueue>,
    mut commands: Commands,
) {
    for call in parked.take(renzora_plugin::anim::SERVICE) {
        // The payload is bytes the host never looked at, so every check happens
        // here — and it is exact, not a minimum. A `<` check passes a payload that is the right
        // size or larger, which sounds forgiving and is not: a plugin built from
        // a version of `renzora_plugin` that REORDERED this struct sends exactly
        // the right number of bytes and is misread silently, field for field. The
        // domain modules sit outside the ABI version deliberately, so nothing else
        // catches that — not the handshake, not the interface prefix hash, and not
        // cargo, since a plugin's `renzora_plugin = "0.1"` resolves to any 0.1.x.
        // An equality check turns a silent misread into a clean refusal.
        if call.payload.len() != size_of::<AnimCommand>() {
            warn!(
                "[animation] plugin sent {} bytes for an animation command; expected {}",
                call.payload.len(),
                size_of::<AnimCommand>()
            );
            continue;
        }
        // SAFETY: length checked, and `AnimCommand` is `#[repr(C)]` plain-old-data
        // — its inline name is exactly why.
        let cmd = unsafe { call.payload.as_ptr().cast::<AnimCommand>().read_unaligned() };
        let entity = call.entity;
        let name = cmd.name.as_str().to_string();
        let flag = cmd.flag != 0;

        match cmd.op {
            AnimOp::Play => queue.commands.push(AnimationCommand::Play {
                entity,
                name,
                looping: flag,
                speed: cmd.value,
            }),
            AnimOp::Stop => queue.commands.push(AnimationCommand::Stop { entity }),
            AnimOp::Pause => queue.commands.push(AnimationCommand::Pause { entity }),
            AnimOp::Resume => queue.commands.push(AnimationCommand::Resume { entity }),
            AnimOp::SetSpeed => queue.commands.push(AnimationCommand::SetSpeed {
                entity,
                speed: cmd.value,
            }),
            AnimOp::Seek => queue.commands.push(AnimationCommand::Seek {
                entity,
                time: cmd.value,
            }),
            AnimOp::Crossfade => queue.commands.push(AnimationCommand::Crossfade {
                entity,
                name,
                duration: cmd.value,
                looping: flag,
            }),
            AnimOp::SetParam => queue.commands.push(AnimationCommand::SetParam {
                entity,
                name,
                value: cmd.value,
            }),
            AnimOp::SetBool => queue.commands.push(AnimationCommand::SetBoolParam {
                entity,
                name,
                value: flag,
            }),
            AnimOp::Trigger => queue.commands.push(AnimationCommand::Trigger { entity, name }),
            AnimOp::SetLayerWeight => queue.commands.push(AnimationCommand::SetLayerWeight {
                entity,
                layer_name: name,
                weight: cmd.value,
            }),
            AnimOp::TweenPosition => {
                insert_tween(&mut commands, entity, TweenProperty::Position(vec3(cmd.target)), &cmd)
            }
            AnimOp::TweenRotation => {
                insert_tween(&mut commands, entity, TweenProperty::Rotation(vec3(cmd.target)), &cmd)
            }
            AnimOp::TweenScale => {
                insert_tween(&mut commands, entity, TweenProperty::Scale(vec3(cmd.target)), &cmd)
            }
            // From a plugin built against a newer `renzora_plugin`. Nothing
            // upstream validates this — the host carried opaque bytes — so this
            // is the only place it can be caught.
            other => warn!(
                "[animation] plugin used animation op {} ({}), which this build does not have",
                other.0,
                other.name()
            ),
        }
    }
}

fn insert_tween(
    commands: &mut Commands,
    entity: Entity,
    property: TweenProperty,
    cmd: &AnimCommand,
) {
    commands.entity(entity).try_insert(ProceduralTween {
        property,
        start_value: None,
        duration: cmd.value,
        elapsed: 0.0,
        easing: easing(cmd.easing),
    });
}

fn vec3(v: renzora_plugin::sys::Vec3) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}

/// Map `renzora_plugin::anim`'s frozen easing ordinals onto [`EasingFunction`].
///
/// Written out rather than transmuted or indexed, and that is what matters: both
/// sides are named on every arm, so [`EasingFunction`]'s declaration order is
/// irrelevant here. Someone may insert a variant into it freely. An `as`-cast or
/// an index would instead have silently remapped every plugin's easing the first
/// time that happened.
///
/// An earlier version of this comment claimed the match would *stop compiling* if
/// a variant were inserted. It would not, on two counts: the arms construct by
/// name, and there is a `_` arm below. Nothing here is a compile-time guard —
/// the safety comes from naming both sides, which is enough.
///
/// What genuinely is frozen is `Easing`'s own ordinals, because every shipped
/// plugin compiled them in. Renumbering those silently remaps behaviour and no
/// test sees it.
fn easing(e: Easing) -> EasingFunction {
    match e {
        Easing::Linear => EasingFunction::Linear,
        Easing::In => EasingFunction::EaseIn,
        Easing::Out => EasingFunction::EaseOut,
        Easing::InOut => EasingFunction::EaseInOut,
        Easing::InQuad => EasingFunction::EaseInQuad,
        Easing::OutQuad => EasingFunction::EaseOutQuad,
        Easing::InOutQuad => EasingFunction::EaseInOutQuad,
        Easing::InCubic => EasingFunction::EaseInCubic,
        Easing::OutCubic => EasingFunction::EaseOutCubic,
        Easing::InOutCubic => EasingFunction::EaseInOutCubic,
        Easing::InBack => EasingFunction::EaseInBack,
        Easing::OutBack => EasingFunction::EaseOutBack,
        Easing::InOutBack => EasingFunction::EaseInOutBack,
        Easing::InElastic => EasingFunction::EaseInElastic,
        Easing::OutElastic => EasingFunction::EaseOutElastic,
        Easing::InBounce => EasingFunction::EaseInBounce,
        Easing::OutBounce => EasingFunction::EaseOutBounce,
        // From a plugin built against a newer vocabulary. Matches what the script
        // path does with an unrecognised easing name.
        _ => EasingFunction::EaseInOut,
    }
}

/// Wires both directions up. Called from `AnimationPlugin::build`.
pub fn install(app: &mut App) {
    // Both calls matter, and for different reasons. `register_type` is what makes
    // the type path resolvable — a plugin names this component by string and the
    // host looks it up in `AppTypeRegistry`. `register_component` is what gives
    // it a `ComponentId` *now*: bevy assigns one lazily, on first use, and first
    // use would otherwise be `sync_plugin_anim_state`'s query in the first
    // `Update` — long after the C-ABI plugins initialised and asked for the id.
    // They would have been told the component does not exist.
    app.register_type::<PluginAnimState>();
    app.world_mut().register_component::<PluginAnimState>();

    // And say it may be read as *data*, not merely filtered on. Every host
    // component is filterable; handing over raw bytes is the restricted
    // direction, because a mirror is matched by name and its layout is the
    // plugin author's problem. This type is `#[repr(C)]` plain data built for
    // exactly that, which is what makes it safe to expose and why the exposing
    // call lives here rather than in a list the plugin crate maintains.
    renzora_plugin::host::expose_component_data::<PluginAnimState>(app);

    // The two sides agree on this type by *string*, and a mismatch is silent on
    // both: the plugin asks for a path nothing registered, gets `INVALID`, and
    // its animation queries match no entities forever. So assert it, here, where
    // the failure is a startup panic naming both halves instead of a plugin that
    // loads cleanly and reads nothing.
    let ours = <PluginAnimState as bevy::reflect::TypePath>::type_path();
    let theirs = <AnimState as renzora_plugin::ecs::Component>::TYPE_PATH;
    assert_eq!(
        ours, theirs,
        "renzora_plugin::anim names the animation mirror `{theirs}` but it is registered here as \
         `{ours}` — renaming or moving `PluginAnimState` breaks every plugin's animation \
         reads, so update the `host_component!` call in `renzora_plugin::anim` to match"
    );

    app.add_systems(
        Update,
        drain_plugin_anim_commands.before(crate::systems::process_animation_commands),
    );
    app.add_systems(
        Update,
        sync_plugin_anim_state.after(crate::read_state::update_animator_read_state),
    );
}
