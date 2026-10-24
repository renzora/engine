//! Script actions. The functions declared in `script_extension` (compiled
//! only with `scripting`) arrive here as `renzora::ScriptAction`s, the same
//! indirection `renzora_ragdoll` and `renzora_physics` use. The observer
//! itself lives outside the `scripting` gate because `ScriptAction` is a
//! contract type: anything can fire one, a blueprint as much as a script.
//!
//! Every function takes a **bone name** first, looked up under the script's
//! own entity, so a script on the character root reaches any bone of its own
//! skeleton and never another character's bone of the same name. An empty name
//! means every constraint of that kind on the entity and below it, which is
//! what "switch all IK off" wants.

use crate::apply::spring::SpringState;
use crate::components::{CopyPosition, CopyRotation, FootIk, LookAt, RotationLimit, SpringBones, TwoBoneIk};
use crate::names::NameLookup;
use bevy::prelude::*;
use renzora::{ScriptAction, ScriptActionValue};

fn arg_str<'a>(action: &'a ScriptAction, key: &str) -> &'a str {
    match action.args.get(key) {
        Some(ScriptActionValue::String(s)) => s.as_str(),
        _ => "",
    }
}

fn arg_f32(action: &ScriptAction, key: &str) -> Option<f32> {
    match action.args.get(key)? {
        ScriptActionValue::Float(f) => Some(*f),
        ScriptActionValue::Int(i) => Some(*i as f32),
        _ => None,
    }
}

fn arg_vec3(action: &ScriptAction, key: &str) -> Option<Vec3> {
    match action.args.get(key)? {
        ScriptActionValue::Vec3(v) => Some(Vec3::from_array(*v)),
        _ => None,
    }
}

/// The entities an action addresses: the named bone under the script's
/// entity, or with no name the entity and all its descendants.
fn addressed(
    action: &ScriptAction,
    lookup: &NameLookup,
    names: &Query<&Name>,
    parents: &Query<&ChildOf>,
    children: &Query<&Children>,
) -> Vec<Entity> {
    let bone = arg_str(action, "bone");
    if !bone.is_empty() {
        return lookup.find_under(bone, action.entity, names, parents).into_iter().collect();
    }
    let mut out = Vec::new();
    let mut stack = vec![action.entity];
    while let Some(e) = stack.pop() {
        out.push(e);
        if let Ok(kids) = children.get(e) {
            stack.extend(kids.iter());
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
pub fn handle_ik_script_actions(
    trigger: On<ScriptAction>,
    lookup: Res<NameLookup>,
    names: Query<&Name>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    mut two_bone: Query<&mut TwoBoneIk>,
    mut look: Query<&mut LookAt>,
    mut feet: Query<&mut FootIk>,
    mut copy_rotation: Query<&mut CopyRotation>,
    mut copy_position: Query<&mut CopyPosition>,
    mut limits: Query<&mut RotationLimit>,
    mut springs: Query<&mut SpringBones>,
    mut spring_state: Query<&mut SpringState>,
) {
    let action = trigger.event();
    let known = matches!(
        action.name.as_str(),
        "ik_set_target"
            | "ik_set_target_entity"
            | "ik_set_pole"
            | "ik_set_weight"
            | "look_at_set_target"
            | "look_at_set_target_entity"
            | "look_at_set_weight"
            | "foot_ik_set_weight"
            | "copy_constraint_set_weight"
            | "rotation_limit_set_weight"
            | "spring_bones_set_weight"
            | "spring_bones_reset"
    );
    if !known {
        return;
    }
    let weight = arg_f32(action, "weight").map(|w| w.clamp(0.0, 1.0));
    let point = arg_vec3(action, "target");
    let target_name = arg_str(action, "target").to_string();

    for e in addressed(action, &lookup, &names, &parents, &children) {
        match action.name.as_str() {
            "ik_set_target" => {
                if let (Ok(mut ik), Some(p)) = (two_bone.get_mut(e), point) {
                    ik.target.clear();
                    ik.target_position = p;
                }
            }
            "ik_set_target_entity" => {
                if let Ok(mut ik) = two_bone.get_mut(e) {
                    ik.target.clone_from(&target_name);
                }
            }
            "ik_set_pole" => {
                if let (Ok(mut ik), Some(d)) = (two_bone.get_mut(e), arg_vec3(action, "direction")) {
                    ik.pole_target.clear();
                    ik.pole_direction = d;
                }
            }
            "ik_set_weight" => {
                if let (Ok(mut ik), Some(w)) = (two_bone.get_mut(e), weight) {
                    ik.weight = w;
                }
            }
            "look_at_set_target" => {
                if let (Ok(mut l), Some(p)) = (look.get_mut(e), point) {
                    l.target.clear();
                    l.target_position = p;
                }
            }
            "look_at_set_target_entity" => {
                if let Ok(mut l) = look.get_mut(e) {
                    l.target.clone_from(&target_name);
                }
            }
            "look_at_set_weight" => {
                if let (Ok(mut l), Some(w)) = (look.get_mut(e), weight) {
                    l.weight = w;
                }
            }
            "foot_ik_set_weight" => {
                if let (Ok(mut f), Some(w)) = (feet.get_mut(e), weight) {
                    f.weight = w;
                }
            }
            "copy_constraint_set_weight" => {
                if let Some(w) = weight {
                    if let Ok(mut c) = copy_rotation.get_mut(e) {
                        c.weight = w;
                    }
                    if let Ok(mut c) = copy_position.get_mut(e) {
                        c.weight = w;
                    }
                }
            }
            "rotation_limit_set_weight" => {
                if let (Ok(mut l), Some(w)) = (limits.get_mut(e), weight) {
                    l.weight = w;
                }
            }
            "spring_bones_set_weight" => {
                if let (Ok(mut s), Some(w)) = (springs.get_mut(e), weight) {
                    s.weight = w;
                }
            }
            "spring_bones_reset" => {
                if let Ok(mut s) = spring_state.get_mut(e) {
                    s.reset = true;
                }
            }
            _ => {}
        }
    }
}
