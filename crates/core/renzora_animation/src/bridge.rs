//! Bridge between renzora_scripting's ScriptCommandQueue and AnimationCommandQueue.
//!
//! Reads animation-related ScriptCommands from the scripting queue and converts them
//! into AnimationCommands. This lets the animation system process commands without
//! the scripting crate depending on animation.

use bevy::prelude::*;
use renzora_scripting::ScriptCommand;
use renzora_scripting::systems::execution::ScriptCommandQueue;

use crate::systems::{AnimationCommand, AnimationCommandQueue};
use crate::tween::{EasingFunction, ProceduralTween, TweenProperty};

/// Drain animation commands from the script queue and push them into AnimationCommandQueue.
///
/// Runs before `process_animation_commands`.
pub fn route_script_animation_commands(
    mut commands: Commands,
    mut script_queue: ResMut<ScriptCommandQueue>,
    mut anim_queue: ResMut<AnimationCommandQueue>,
) {
    let mut i = 0;
    while i < script_queue.commands.len() {
        let is_animation = matches!(
            &script_queue.commands[i].1,
            ScriptCommand::PlayAnimation { .. }
                | ScriptCommand::StopAnimation { .. }
                | ScriptCommand::PauseAnimation { .. }
                | ScriptCommand::ResumeAnimation { .. }
                | ScriptCommand::SetAnimationSpeed { .. }
                | ScriptCommand::CrossfadeAnimation { .. }
                | ScriptCommand::SetAnimationParam { .. }
                | ScriptCommand::SetAnimationBoolParam { .. }
                | ScriptCommand::TriggerAnimation { .. }
                | ScriptCommand::SetAnimationLayerWeight { .. }
                | ScriptCommand::TweenPosition { .. }
                | ScriptCommand::TweenRotation { .. }
                | ScriptCommand::TweenScale { .. }
        );

        if is_animation {
            let (source_entity, cmd) = script_queue.commands.remove(i);
            match cmd {
                ScriptCommand::PlayAnimation {
                    entity_id,
                    name,
                    looping,
                    speed,
                } => {
                    let entity = entity_id
                        .map(|id| Entity::from_bits(id))
                        .unwrap_or(source_entity);
                    anim_queue.commands.push(AnimationCommand::Play {
                        entity,
                        name,
                        looping,
                        speed,
                    });
                }
                ScriptCommand::StopAnimation { entity_id } => {
                    let entity = entity_id
                        .map(|id| Entity::from_bits(id))
                        .unwrap_or(source_entity);
                    anim_queue.commands.push(AnimationCommand::Stop { entity });
                }
                ScriptCommand::PauseAnimation { entity_id } => {
                    let entity = entity_id
                        .map(|id| Entity::from_bits(id))
                        .unwrap_or(source_entity);
                    anim_queue.commands.push(AnimationCommand::Pause { entity });
                }
                ScriptCommand::ResumeAnimation { entity_id } => {
                    let entity = entity_id
                        .map(|id| Entity::from_bits(id))
                        .unwrap_or(source_entity);
                    anim_queue.commands.push(AnimationCommand::Resume { entity });
                }
                ScriptCommand::SetAnimationSpeed { entity_id, speed } => {
                    let entity = entity_id
                        .map(|id| Entity::from_bits(id))
                        .unwrap_or(source_entity);
                    anim_queue
                        .commands
                        .push(AnimationCommand::SetSpeed { entity, speed });
                }
                ScriptCommand::CrossfadeAnimation {
                    entity_id,
                    name,
                    duration,
                    looping,
                } => {
                    let entity = entity_id
                        .map(|id| Entity::from_bits(id))
                        .unwrap_or(source_entity);
                    anim_queue.commands.push(AnimationCommand::Crossfade {
                        entity,
                        name,
                        duration,
                        looping,
                    });
                }
                ScriptCommand::SetAnimationParam {
                    entity_id,
                    name,
                    value,
                } => {
                    let entity = entity_id
                        .map(|id| Entity::from_bits(id))
                        .unwrap_or(source_entity);
                    anim_queue.commands.push(AnimationCommand::SetParam {
                        entity,
                        name,
                        value,
                    });
                }
                ScriptCommand::SetAnimationBoolParam {
                    entity_id,
                    name,
                    value,
                } => {
                    let entity = entity_id
                        .map(|id| Entity::from_bits(id))
                        .unwrap_or(source_entity);
                    anim_queue.commands.push(AnimationCommand::SetBoolParam {
                        entity,
                        name,
                        value,
                    });
                }
                ScriptCommand::TriggerAnimation { entity_id, name } => {
                    let entity = entity_id
                        .map(|id| Entity::from_bits(id))
                        .unwrap_or(source_entity);
                    anim_queue
                        .commands
                        .push(AnimationCommand::Trigger { entity, name });
                }
                ScriptCommand::SetAnimationLayerWeight {
                    entity_id,
                    layer_name,
                    weight,
                } => {
                    let entity = entity_id
                        .map(|id| Entity::from_bits(id))
                        .unwrap_or(source_entity);
                    anim_queue.commands.push(AnimationCommand::SetLayerWeight {
                        entity,
                        layer_name,
                        weight,
                    });
                }
                ScriptCommand::TweenPosition {
                    entity_id,
                    target,
                    duration,
                    easing,
                } => {
                    let entity = entity_id
                        .map(|id| Entity::from_bits(id))
                        .unwrap_or(source_entity);
                    commands.entity(entity).insert(ProceduralTween {
                        property: TweenProperty::Position(target),
                        start_value: None,
                        easing: EasingFunction::from_str(&easing),
                        duration,
                        elapsed: 0.0,
                    });
                }
                ScriptCommand::TweenRotation {
                    entity_id,
                    target,
                    duration,
                    easing,
                } => {
                    let entity = entity_id
                        .map(|id| Entity::from_bits(id))
                        .unwrap_or(source_entity);
                    commands.entity(entity).insert(ProceduralTween {
                        property: TweenProperty::Rotation(target),
                        start_value: None,
                        easing: EasingFunction::from_str(&easing),
                        duration,
                        elapsed: 0.0,
                    });
                }
                ScriptCommand::TweenScale {
                    entity_id,
                    target,
                    duration,
                    easing,
                } => {
                    let entity = entity_id
                        .map(|id| Entity::from_bits(id))
                        .unwrap_or(source_entity);
                    commands.entity(entity).insert(ProceduralTween {
                        property: TweenProperty::Scale(target),
                        start_value: None,
                        easing: EasingFunction::from_str(&easing),
                        duration,
                        elapsed: 0.0,
                    });
                }
                _ => unreachable!(),
            }
        } else {
            i += 1;
        }
    }
}
