//! Blending physics with animation: partial weights and the get-up.
//!
//! A full ragdoll is simple: the bones are cut off from the animation player
//! and the physics owns them. Anything in between needs *both* poses every
//! frame, and there is only one `Transform` per bone to hold them. So the
//! frame is split:
//!
//! 1. `First`: [`restore_physics_pose`] puts each simulated bone's own physics
//!    pose back, so the physics step that follows sees exactly the pose it left
//!    (what the screen showed was a blend, and a body handed a blend would read
//!    it as a teleport).
//! 2. The physics step moves the bodies.
//! 3. `PostUpdate`, before the animation player: [`capture_physics_pose`]
//!    records where the physics put each simulated bone.
//! 4. The animation player writes the clip pose over it.
//! 5. After the animation player: [`blend_ragdoll_pose`] mixes the clip pose
//!    with the recorded physics pose by the ragdoll's weight.
//!
//! The get-up uses the same mix with the physics pose frozen at the moment the
//! ragdoll switched off and the weight easing to zero over `get_up_time`, so
//! the character moves smoothly from where it fell into its animation rather
//! than snapping.

use crate::Ragdoll;
use bevy::prelude::*;

/// Per-ragdoll blend state on the `Ragdoll` entity.
#[derive(Component, Default, Clone, Copy, Debug)]
pub struct RagdollBlendState {
    /// The physics' share of the pose right now, 0..1.
    pub weight: f32,
    /// Weight lost per second while getting up.
    pub(crate) fade: f32,
    pub(crate) was_active: bool,
    /// Whether this crate paused the animator, so it resumes only what it
    /// paused.
    pub(crate) paused: bool,
}

/// Per-bone blend state on each generated bone.
#[derive(Component, Clone, Copy, Debug)]
pub struct BoneBlend {
    pub(crate) root: Entity,
    /// Where the physics last put this bone.
    pub(crate) physics: Transform,
    /// The bone is a dynamic body right now.
    pub(crate) simulated: bool,
    /// The bone still has its animation link (anything but a full ragdoll).
    pub(crate) linked: bool,
    /// Part of the current blend: simulating, or getting up from it.
    pub(crate) involved: bool,
}

pub fn update_blend_weights(time: Res<Time>, mut ragdolls: Query<(&Ragdoll, &mut RagdollBlendState)>) {
    let dt = time.delta_secs();
    for (ragdoll, mut state) in &mut ragdolls {
        let next = if ragdoll.active {
            ragdoll.blend.clamp(0.0, 1.0)
        } else {
            (state.weight - state.fade * dt).max(0.0)
        };
        if next != state.weight {
            state.weight = next;
        }
    }
}

pub fn restore_physics_pose(mut bones: Query<(&BoneBlend, &mut Transform)>) {
    for (blend, mut transform) in &mut bones {
        if blend.simulated && blend.linked && *transform != blend.physics {
            *transform = blend.physics;
        }
    }
}

pub fn capture_physics_pose(mut bones: Query<(&mut BoneBlend, &Transform)>) {
    for (mut blend, transform) in &mut bones {
        if blend.simulated {
            blend.physics = *transform;
        }
    }
}

pub fn blend_ragdoll_pose(
    roots: Query<&RagdollBlendState>,
    mut bones: Query<(&BoneBlend, &mut Transform)>,
) {
    for (blend, mut transform) in &mut bones {
        if !blend.involved || (blend.simulated && !blend.linked) {
            // Not blending, or a full ragdoll whose transform already is the
            // physics pose.
            continue;
        }
        let Ok(state) = roots.get(blend.root) else {
            continue;
        };
        let w = state.weight;
        if w <= 0.0 {
            continue;
        }
        let animated = *transform;
        transform.translation = animated.translation.lerp(blend.physics.translation, w);
        transform.rotation = animated.rotation.slerp(blend.physics.rotation, w);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate::RagdollBuilt;
    use crate::{RagdollBone, RagdollPlugin};
    use renzora::physics::{PhysicsBodyData, PhysicsBodyType};

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, RagdollPlugin));
        app
    }

    fn bone(app: &mut App, name: &str, parent: Entity) -> Entity {
        app.world_mut()
            .spawn((
                Name::new(name.to_string()),
                Transform::IDENTITY,
                RagdollBone,
                PhysicsBodyData::kinematic_body(),
                ChildOf(parent),
            ))
            .id()
    }

    fn body(app: &App, e: Entity) -> PhysicsBodyType {
        app.world().get::<PhysicsBodyData>(e).unwrap().body_type
    }

    #[test]
    fn a_partial_ragdoll_simulates_only_the_named_subtree() {
        let mut app = app();
        let root = app.world_mut().spawn((Transform::IDENTITY, RagdollBuilt)).id();
        let spine = bone(&mut app, "Spine", root);
        let arm = bone(&mut app, "LeftArm", spine);
        let hand = bone(&mut app, "LeftHand", arm);
        let other = bone(&mut app, "RightArm", spine);
        app.world_mut()
            .entity_mut(root)
            .insert(Ragdoll { active: true, bone: "LeftArm".into(), blend: 0.5, ..default() });
        app.update();
        assert_eq!(body(&app, spine), PhysicsBodyType::KinematicBody);
        assert_eq!(body(&app, other), PhysicsBodyType::KinematicBody);
        assert_eq!(body(&app, arm), PhysicsBodyType::RigidBody);
        assert_eq!(body(&app, hand), PhysicsBodyType::RigidBody);

        // Off again: everything back to kinematic, and the get-up starts.
        app.world_mut().get_mut::<Ragdoll>(root).unwrap().active = false;
        app.update();
        assert_eq!(body(&app, arm), PhysicsBodyType::KinematicBody);
        let blend = app.world().get::<BoneBlend>(arm).unwrap();
        assert!(blend.involved && !blend.simulated);
    }

    #[test]
    fn the_get_up_blends_from_the_frozen_physics_pose() {
        let mut app = app();
        let root = app.world_mut().spawn((Transform::IDENTITY, RagdollBuilt)).id();
        let arm = bone(&mut app, "Arm", root);
        app.world_mut().entity_mut(arm).insert(BoneBlend {
            root,
            physics: Transform::from_xyz(1.0, 0.0, 0.0),
            simulated: false,
            linked: true,
            involved: true,
        });
        app.world_mut().entity_mut(root).insert((
            Ragdoll { active: false, ..default() },
            RagdollBlendState { weight: 0.5, fade: 0.0, was_active: false, paused: false },
        ));
        app.update();
        let t = app.world().get::<Transform>(arm).unwrap().translation;
        assert!((t.x - 0.5).abs() < 1e-5, "{t:?}");
    }

    #[test]
    fn the_get_up_weight_eases_to_zero() {
        let mut app = app();
        let root = app
            .world_mut()
            .spawn((
                Ragdoll { active: false, ..default() },
                RagdollBlendState { weight: 1.0, fade: 1000.0, was_active: false, paused: false },
                RagdollBuilt,
            ))
            .id();
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(5));
        app.update();
        assert_eq!(app.world().get::<RagdollBlendState>(root).unwrap().weight, 0.0);
    }
}
