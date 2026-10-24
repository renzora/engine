//! Building a skeleton into a model, posing it, and animating a face: the
//! tools the Animation workspace (`stage`) arranges into its panels.
//!
//! - [`rig_panel`]: the skeleton as a tree, pose tools (key, rest, mirror,
//!   copy/paste, IK) and, for a model with no skeleton, the humanoid auto-rig.
//! - [`face_panel`]: building a face rig from four points, then generated
//!   blinks and lip sync from a recording.
//! - [`controls`] and [`controls_panel`]: sliders the user makes for any
//!   rig from a pose on the stage, kept in a `.controls` file beside the
//!   model; the face's expressions are a preset of them.
//! - [`markers`]: the points a rig is built from, placed on the stage.
//! - [`respawn`]: once a model file has been rigged or reweighted, reloading
//!   it and rebuilding every instance of it so the change is in the scene.
//!
//! The rigging itself (GLB editing, weights, expressions, IK) is the
//! `renzora_rig` crate; this is its front end.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;

use renzora_ember::UiInteraction;

pub(crate) mod chain_panel;
pub(crate) mod controls;
pub(crate) mod controls_panel;
pub(crate) mod export;
pub(crate) mod face_panel;
pub(crate) mod lipsync;
pub(crate) mod markers;
pub(crate) mod respawn;
pub(crate) mod quadruped_panel;
pub(crate) mod rig_panel;
pub(crate) mod skeleton_panel;

pub struct RigPlugin;

impl Plugin for RigPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, rig_click.run_if(in_state(renzora_editor_framework::SplashState::Editor)));
        markers::register(app);
        respawn::register(app);
        rig_panel::register(app);
        face_panel::register(app);
        controls::register(app);
        chain_panel::register(app);
    }
}

/// What a button in these panels does, run with the whole world.
#[derive(Component, Clone)]
pub(crate) struct RigClick(pub(crate) Arc<dyn Fn(&mut World) + Send + Sync>);

pub(crate) fn on_click(commands: &mut Commands, entity: Entity, f: impl Fn(&mut World) + Send + Sync + 'static) {
    commands.entity(entity).insert(RigClick(Arc::new(f)));
}

fn rig_click(q: Query<(&UiInteraction, &RigClick), Changed<UiInteraction>>, mut commands: Commands) {
    for (i, click) in &q {
        if *i == UiInteraction::Pressed {
            let f = click.0.clone();
            commands.queue(move |w: &mut World| f(w));
        }
    }
}

/// Say something in the Console and as a toast: rigging is slow enough, and
/// its failures specific enough, that both are wanted.
pub(crate) fn tell(world: &mut World, ok: bool, msg: impl Into<String>) {
    let msg = msg.into();
    if ok {
        renzora::core::console_log::console_success("Rig", msg.clone());
    } else {
        renzora::core::console_log::console_error("Rig", msg.clone());
    }
    if let Some(mut q) = world.get_resource_mut::<renzora_ember::widgets::ToastQueue>() {
        let tone = if ok { renzora_ember::widgets::Tone::Success } else { renzora_ember::widgets::Tone::Error };
        q.push(tone, msg);
    }
}

/// The model the panels work on: the animation editor's subject (the model
/// root or animator the selection resolves to).
pub(crate) fn subject(world: &World) -> Option<Entity> {
    world.get_resource::<crate::AnimationEditorState>()?.selected_entity
}

/// The subject's model file, as an absolute path, and as the project-relative
/// path the asset server knows it by.
pub(crate) fn model_file(world: &World, root: Entity) -> Option<(std::path::PathBuf, String)> {
    let rel = world.get::<renzora::core::MeshInstanceData>(root)?.model_path.clone()?;
    let project = world.get_resource::<renzora::core::CurrentProject>()?;
    Some((project.resolve_path(&rel), rel))
}

/// Every descendant of `root`, depth first.
pub(crate) fn descendants(world: &World, root: Entity) -> Vec<Entity> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        out.push(e);
        if let Some(children) = world.get::<Children>(e) {
            stack.extend(children.iter().rev());
        }
    }
    out
}

/// The joints of every skin under `root`, in skin order, each once.
pub(crate) fn joints(world: &World, root: Entity) -> Vec<Entity> {
    let mut out: Vec<Entity> = Vec::new();
    for e in descendants(world, root) {
        if let Some(skin) = world.get::<bevy::mesh::skinning::SkinnedMesh>(e) {
            for j in &skin.joints {
                if !out.contains(j) {
                    out.push(*j);
                }
            }
        }
    }
    out
}

/// A bone's name as clips and the pose tools know it: sanitised, and without a
/// suffix the editor added only to keep the name unique.
pub(crate) fn bone_name(world: &World, e: Entity) -> Option<String> {
    let name = world.get::<Name>(e)?;
    Some(renzora::sanitize_id(renzora::BindName::of(name, world.get::<renzora::BindName>(e))))
}

/// A bone under `root` by name, however the entity's name was spelled after
/// import (`mixamorig:LeftArm` arrives as `mixamorig_leftarm`).
pub(crate) fn find_bone(world: &World, root: Entity, name: &str) -> Option<Entity> {
    let want = renzora::sanitize_id(name);
    descendants(world, root).into_iter().find(|&e| {
        world
            .get::<Name>(e)
            .is_some_and(|n| renzora::sanitize_id(renzora::BindName::of(n, world.get::<renzora::BindName>(e))) == want)
    })
}

/// The pose every skinned joint under `root` was bound in, as local
/// transforms.
///
/// Read from the skins' inverse bind matrices: at bind time each joint's world
/// transform times its inverse bind matrix is the skinned mesh's own, so the
/// bind pose is recoverable from any skinned model whatever the clip has done
/// to the bones since. A joint's local rest is then relative to its parent's
/// bind (or to the parent's current transform when the parent is not a
/// joint).
pub(crate) fn rest_pose(world: &World, root: Entity) -> HashMap<Entity, Transform> {
    let bind_world = bind_world(world, root);
    let mut out = HashMap::new();
    for (&j, &m) in &bind_world {
        let parent = world.get::<ChildOf>(j).map(|c| c.parent());
        let parent_world = match parent {
            Some(p) => bind_world.get(&p).copied().or_else(|| world.get::<GlobalTransform>(p).map(|g| g.to_matrix())),
            None => Some(Mat4::IDENTITY),
        };
        let local = parent_world.unwrap_or(Mat4::IDENTITY).inverse() * m;
        out.insert(j, Transform::from_matrix(local));
    }
    out
}

/// Each skinned joint's world transform in the bind pose. See [`rest_pose`].
pub(crate) fn bind_world(world: &World, root: Entity) -> HashMap<Entity, Mat4> {
    let Some(binds_assets) = world.get_resource::<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>() else {
        return HashMap::new();
    };
    let mut out: HashMap<Entity, Mat4> = HashMap::new();
    for e in descendants(world, root) {
        let (Some(skin), Some(gt)) = (world.get::<bevy::mesh::skinning::SkinnedMesh>(e), world.get::<GlobalTransform>(e)) else {
            continue;
        };
        let Some(binds) = binds_assets.get(&skin.inverse_bindposes) else { continue };
        for (j, ibm) in skin.joints.iter().zip(binds.iter()) {
            out.entry(*j).or_insert_with(|| gt.to_matrix() * ibm.inverse());
        }
    }
    out
}

/// The model's left-right axis in world space: its own X, which is the
/// character's left on a Mixamo rig and on the face rig this crate builds.
pub(crate) fn model_side_axis(world: &World, root: Entity) -> Vec3 {
    world.get::<GlobalTransform>(root).map(|g| g.rotation() * Vec3::X).unwrap_or(Vec3::X)
}

/// Hand-made pose changes for these bones: written now, and held over the
/// clip by the pose layer until keyed or the playhead moves, the same as a
/// pose made with the gizmo.
pub(crate) fn set_pose(world: &mut World, poses: &[(Entity, Transform)]) {
    for (e, t) in poses {
        if let Some(mut tr) = world.get_mut::<Transform>(*e) {
            *tr = *t;
        }
    }
    if let Some(mut layer) = world.get_resource_mut::<crate::timeline::pose::PoseLayer>() {
        for (e, t) in poses {
            layer.posed.insert(*e, *t);
        }
    }
}
