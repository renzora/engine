//! Spring bones in the world: find the chain, feed the simulation the
//! animated joint positions, turn each bone to follow its simulated child.
//!
//! Only rotations are written. The bones' local translations stay as the
//! animation left them, so a chain can bend and wobble but never stretch,
//! whatever the simulation does.

use crate::components::{SpringBones, SpringCollider};
use crate::names::top_root;
use crate::pose::{track, Pose, PoseCache};
use crate::solve::spring::{Sphere, SpringChain, SpringParams};
use bevy::prelude::*;

/// Longest chain followed, so a mis-placed component on a skeleton's root
/// does not try to simulate a whole spine-to-fingertip path.
const MAX_CHAIN: usize = 64;

#[derive(Component, Default)]
pub struct SpringState {
    chain: Vec<Entity>,
    sim: SpringChain,
    last_anchor: Option<Vec3>,
    /// Start from the pose next frame: set by a script reset and by the chain
    /// being switched off, so switching it back on does not whip.
    pub(crate) reset: bool,
}

pub fn ensure_spring_state(mut commands: Commands, missing: Query<Entity, (With<SpringBones>, Without<SpringState>)>) {
    for entity in &missing {
        commands.entity(entity).try_insert(SpringState { reset: true, ..default() });
    }
}

fn build_chain(start: Entity, children: &Query<&Children>, transforms: &Query<&mut Transform>) -> Vec<Entity> {
    let mut chain = vec![start];
    let mut current = start;
    while chain.len() < MAX_CHAIN {
        let next = children
            .get(current)
            .ok()
            .and_then(|kids| kids.iter().find(|k| transforms.get(*k).is_ok()));
        match next {
            Some(n) => {
                chain.push(n);
                current = n;
            }
            None => break,
        }
    }
    chain
}

#[allow(clippy::too_many_arguments)]
pub fn apply_spring_bones(
    mut commands: Commands,
    time: Res<Time>,
    mut springs: Query<(Entity, &SpringBones, &mut SpringState)>,
    colliders: Query<(Entity, &SpringCollider)>,
    mut transforms: Query<&mut Transform>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    cached: Query<(), With<PoseCache>>,
) {
    let dt = time.delta_secs();
    for (start, spring, mut state) in &mut springs {
        let w = spring.weight.clamp(0.0, 1.0);
        if w <= 0.0 {
            state.reset = true;
            continue;
        }
        if state.chain.first() != Some(&start) || state.chain.iter().any(|e| transforms.get(*e).is_err()) {
            state.chain = build_chain(start, &children, &transforms);
            state.reset = true;
        }
        if state.chain.len() < 2 {
            continue;
        }

        let mut pose = Pose { transforms: &mut transforms, parents: &parents };
        let animated: Vec<Vec3> = state.chain.iter().filter_map(|e| pose.world(*e)).map(|t| t.translation).collect();
        if animated.len() != state.chain.len() {
            continue;
        }
        let anchor = animated[0];
        let teleported = state
            .last_anchor
            .is_some_and(|last| last.distance(anchor) > spring.teleport_distance.max(0.0));
        if state.reset || teleported {
            state.sim.reset(&animated);
            state.reset = false;
        }
        state.last_anchor = Some(anchor);

        let home = top_root(start, &parents);
        let spheres: Vec<Sphere> = colliders
            .iter()
            .filter(|(e, _)| top_root(*e, &parents) == home)
            .filter_map(|(e, c)| {
                let world = pose.world(e)?;
                let scale = world.scale.abs().max_element();
                Some(Sphere { center: world.transform_point(c.offset), radius: c.radius * scale })
            })
            .collect();

        let params = SpringParams {
            stiffness: spring.stiffness,
            damping: spring.damping,
            drag: spring.drag,
            gravity: spring.gravity,
            radius: spring.radius,
        };
        let simulated = state.sim.advance(dt, &animated, &params, &spheres);

        for (i, bone) in state.chain.iter().enumerate().take(state.chain.len() - 1) {
            let child = state.chain[i + 1];
            let (Some(here), Some(next)) = (pose.world(*bone), pose.world(child)) else {
                break;
            };
            let (Some(current), Some(wanted)) = (
                (next.translation - here.translation).try_normalize(),
                (simulated[i + 1] - simulated[i]).try_normalize(),
            ) else {
                continue;
            };
            if let Some(local) = pose.local(*bone) {
                track(&mut commands, &cached, *bone, local);
            }
            let turned = Quat::from_rotation_arc(current, wanted) * here.rotation;
            pose.set_world_rotation(*bone, here.rotation.slerp(turned, w));
        }
    }
}
