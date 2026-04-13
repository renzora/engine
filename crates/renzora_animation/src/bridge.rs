//! Bridge between script actions and the animation command queue.
//!
//! Observes ScriptAction events for animation commands and converts them
//! into AnimationCommands. This decouples animation from the scripting crate.

use bevy::prelude::*;
use renzora::ScriptAction;

use crate::systems::{AnimationCommand, AnimationCommandQueue};
use crate::tween::{EasingFunction, ProceduralTween, TweenProperty};

/// Observer: handle animation-related ScriptAction events.
pub fn handle_animation_script_actions(
    trigger: On<ScriptAction>,
    mut anim_queue: ResMut<AnimationCommandQueue>,
    mut commands: Commands,
) {
    use renzora::ScriptActionValue as V;
    let action = trigger.event();

    let get_entity = || -> Entity {
        match action.args.get("entity_id") {
            Some(V::Int(id)) => Entity::from_bits(*id as u64),
            _ => action.entity,
        }
    };
    let get_str = |key: &str| -> String {
        match action.args.get(key) {
            Some(V::String(s)) => s.clone(),
            _ => String::new(),
        }
    };
    let get_f32 = |key: &str, default: f32| -> f32 {
        match action.args.get(key) {
            Some(V::Float(v)) => *v,
            Some(V::Int(v)) => *v as f32,
            _ => default,
        }
    };
    let get_bool = |key: &str, default: bool| -> bool {
        match action.args.get(key) {
            Some(V::Bool(v)) => *v,
            _ => default,
        }
    };
    let get_vec3 = |key: &str| -> Vec3 {
        match action.args.get(key) {
            Some(V::Vec3(v)) => Vec3::new(v[0], v[1], v[2]),
            _ => Vec3::ZERO,
        }
    };

    match action.name.as_str() {
        "play_animation" => {
            anim_queue.commands.push(AnimationCommand::Play {
                entity: get_entity(),
                name: get_str("name"),
                looping: get_bool("looping", true),
                speed: get_f32("speed", 1.0),
            });
        }
        "stop_animation" => {
            anim_queue.commands.push(AnimationCommand::Stop {
                entity: get_entity(),
            });
        }
        "pause_animation" => {
            anim_queue.commands.push(AnimationCommand::Pause {
                entity: get_entity(),
            });
        }
        "resume_animation" => {
            anim_queue.commands.push(AnimationCommand::Resume {
                entity: get_entity(),
            });
        }
        "set_animation_speed" => {
            anim_queue.commands.push(AnimationCommand::SetSpeed {
                entity: get_entity(),
                speed: get_f32("speed", 1.0),
            });
        }
        "crossfade_animation" => {
            anim_queue.commands.push(AnimationCommand::Crossfade {
                entity: get_entity(),
                name: get_str("name"),
                duration: get_f32("duration", 0.3),
                looping: get_bool("looping", true),
            });
        }
        "set_anim_param" => {
            anim_queue.commands.push(AnimationCommand::SetParam {
                entity: get_entity(),
                name: get_str("name"),
                value: get_f32("value", 0.0),
            });
        }
        "set_anim_bool" => {
            anim_queue.commands.push(AnimationCommand::SetBoolParam {
                entity: get_entity(),
                name: get_str("name"),
                value: get_bool("value", false),
            });
        }
        "trigger_anim" => {
            anim_queue.commands.push(AnimationCommand::Trigger {
                entity: get_entity(),
                name: get_str("name"),
            });
        }
        "set_layer_weight" => {
            anim_queue.commands.push(AnimationCommand::SetLayerWeight {
                entity: get_entity(),
                layer_name: get_str("layer_name"),
                weight: get_f32("weight", 1.0),
            });
        }
        "tween_position" => {
            let entity = get_entity();
            let target = get_vec3("target");
            let duration = get_f32("duration", 1.0);
            let easing = EasingFunction::from_str(&get_str("easing"));
            commands.entity(entity).insert(ProceduralTween {
                property: TweenProperty::Position(target),
                start_value: None,
                duration,
                elapsed: 0.0,
                easing,
            });
        }
        "tween_rotation" => {
            let entity = get_entity();
            let target = get_vec3("target");
            let duration = get_f32("duration", 1.0);
            let easing = EasingFunction::from_str(&get_str("easing"));
            commands.entity(entity).insert(ProceduralTween {
                property: TweenProperty::Rotation(target),
                start_value: None,
                duration,
                elapsed: 0.0,
                easing,
            });
        }
        "tween_scale" => {
            let entity = get_entity();
            let target = get_vec3("target");
            let duration = get_f32("duration", 1.0);
            let easing = EasingFunction::from_str(&get_str("easing"));
            commands.entity(entity).insert(ProceduralTween {
                property: TweenProperty::Scale(target),
                start_value: None,
                duration,
                elapsed: 0.0,
                easing,
            });
        }
        _ => {} // Not an animation action
    }
}
