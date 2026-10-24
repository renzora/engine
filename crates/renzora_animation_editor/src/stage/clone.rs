//! The copy of the model on the stage, and what poses it.
//!
//! The studio preview spawns the copy (`studio_preview::sync_preview_model`)
//! from the subject's model file. [`StageRig`] then pairs its bones with the
//! real model's by name, which is the whole trick of the stage: a bone clicked
//! on the copy is turned into the real bone, an edit to the real bone is
//! copied back.
//!
//! What the copy shows depends on the mode ([`pose_clone`]):
//!
//! - Animate and Logic: the real model's pose, every frame, so the clip, the
//!   hand-made pose over it, IK and the face sliders all show.
//! - Rig: the bind pose, the shape the rig is built against.
//! - Skin: the bind pose plus the test bend (`skin`), on the copy only:
//!   testing weights must not pose the character in the scene.
//!
//! The copy used to play the clip itself, through a copy of the real player's
//! animation graph. That showed the clip and nothing else: a pose that was not
//! keyed, IK and the face sliders never reached it.

use std::collections::HashMap;

use bevy::prelude::*;

use super::StageMode;
use crate::rig::{bone_name, joints, rest_pose, subject};
use crate::studio_preview::StudioPreviewModel;

/// The copy's bones and the real model's, paired by name.
#[derive(Resource, Default)]
pub(crate) struct StageRig {
    /// The copy's root ([`StudioPreviewModel`]).
    pub(crate) root: Option<Entity>,
    /// The copy's skinned joints.
    pub(crate) joints: Vec<Entity>,
    pub(crate) by_name: HashMap<String, Entity>,
    pub(crate) names: HashMap<Entity, String>,
    /// Each copy joint's parent joint, when its parent is one.
    pub(crate) parent: HashMap<Entity, Entity>,
    /// The copy's bind pose, local.
    pub(crate) rest: HashMap<Entity, Transform>,
    /// The real model these are paired with.
    pub(crate) subject: Option<Entity>,
    pub(crate) real_by_name: HashMap<String, Entity>,
    pub(crate) real_names: HashMap<Entity, String>,
    real_joints: Vec<Entity>,
}

impl StageRig {
    pub(crate) fn clone_of(&self, real: Entity) -> Option<Entity> {
        self.by_name.get(self.real_names.get(&real)?).copied()
    }

    pub(crate) fn real_of(&self, copy: Entity) -> Option<Entity> {
        self.real_by_name.get(self.names.get(&copy)?).copied()
    }

    /// Whether the copy has a skeleton to show.
    pub(crate) fn rigged(&self) -> bool {
        !self.joints.is_empty()
    }
}

pub(super) fn register(app: &mut App) {
    app.init_resource::<StageRig>()
        .add_systems(
            Update,
            track_rig
                .run_if(in_state(renzora_editor_framework::SplashState::Editor))
                .run_if(crate::studio_preview::studio_preview_panel_mounted),
        )
        .add_systems(
            PostUpdate,
            pose_clone
                .after(crate::timeline::pose::apply_pose)
                .before(bevy::transform::TransformSystems::Propagate)
                .run_if(in_state(renzora_editor_framework::SplashState::Editor))
                .run_if(crate::studio_preview::studio_preview_panel_mounted),
        );
}

/// Rebuild the pairing when the copy or the subject changes, or when either
/// gains bones (a model spawns its skeleton a few frames after the model,
/// and rigging one gives it a skeleton it did not have).
fn track_rig(world: &mut World) {
    let root = {
        let mut q = world.query_filtered::<Entity, With<StudioPreviewModel>>();
        q.iter(world).next()
    };
    let subject = subject(world);
    let copy_joints = root.map(|r| joints(world, r)).unwrap_or_default();
    let real_joints = subject.map(|s| joints(world, s)).unwrap_or_default();
    let names: HashMap<Entity, String> = copy_joints.iter().filter_map(|&e| Some((e, bone_name(world, e)?))).collect();
    let real_names: HashMap<Entity, String> = real_joints.iter().filter_map(|&e| Some((e, bone_name(world, e)?))).collect();
    // Compared entity for entity, not by count: a model rebuilt after rigging
    // has as many bones as before, all of them new entities. And name for name:
    // a respawned model's bones can collide with the copy's, get a suffix, and
    // only get their `BindName` back a frame later. Paired on the first frame,
    // those bones stayed unpaired, so the gizmo turned the copy and never the
    // real bone, and keying recorded the pose the bone already had.
    let (same, previous_subject) = {
        let rig = world.resource::<StageRig>();
        (
            rig.root == root
                && rig.subject == subject
                && rig.joints == copy_joints
                && rig.real_joints == real_joints
                && rig.names == names
                && rig.real_names == real_names,
            rig.subject,
        )
    };
    if same {
        return;
    }
    let parent: HashMap<Entity, Entity> = copy_joints
        .iter()
        .filter_map(|&e| {
            let p = world.get::<ChildOf>(e)?.parent();
            copy_joints.contains(&p).then_some((e, p))
        })
        .collect();
    let mut rest = root.map(|r| rest_pose(world, r)).unwrap_or_default();
    for &j in &copy_joints {
        if let (false, Some(t)) = (rest.contains_key(&j), world.get::<Transform>(j)) {
            rest.insert(j, *t);
        }
    }
    let rig = StageRig {
        root,
        by_name: names.iter().map(|(e, n)| (n.clone(), *e)).collect(),
        names,
        joints: copy_joints,
        parent,
        rest,
        subject,
        real_by_name: real_names.iter().map(|(e, n)| (n.clone(), *e)).collect(),
        real_names,
        real_joints: real_joints.clone(),
    };
    // A model with no skeleton opens in Rig, which is the only thing that can
    // be done with it; one with a skeleton stays wherever you were. One with
    // eyes and no skeleton is taken for a head, and gets the Face template.
    if let (Some(s), true) = (subject, subject != previous_subject && real_joints.is_empty()) {
        let head = crate::rig::descendants(world, s).into_iter().any(|e| {
            world.get::<Mesh3d>(e).is_some()
                && world.get::<Name>(e).is_some_and(|n| renzora_rig::face::mesh_role(n.as_str(), "") == renzora_rig::face::MeshRole::Eye)
        });
        if let Some(mut ui) = world.get_resource_mut::<super::StageUi>() {
            if ui.mode == StageMode::Animate {
                ui.mode = StageMode::Rig;
            }
            ui.template = usize::from(head);
        }
    }
    world.insert_resource(rig);
}

/// Pose the copy for the mode. See the module doc.
fn pose_clone(world: &mut World) {
    let mode = super::mode(world);
    let poses: Vec<(Entity, Transform)> = {
        let rig = world.resource::<StageRig>();
        match mode {
            StageMode::Animate | StageMode::Logic => rig
                .by_name
                .iter()
                .filter_map(|(name, &copy)| {
                    let real = rig.real_by_name.get(name)?;
                    Some((copy, *world.get::<Transform>(*real)?))
                })
                .collect(),
            StageMode::Rig => rig.rest.iter().map(|(e, t)| (*e, *t)).collect(),
            StageMode::Skin => super::skin::test_pose(world, rig),
        }
    };
    for (e, t) in poses {
        if let Some(mut tr) = world.get_mut::<Transform>(e) {
            if *tr != t {
                *tr = t;
            }
        }
    }
}
