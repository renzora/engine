//! The pose library: whole-body poses saved by name and put back with a
//! click.
//!
//! A pose is every bone's local transform, by bone name, so it applies to any
//! model whose bones are named the same (every Mixamo character, for one). The
//! library is the project's `animations/poses/` folder, one `.pose` file per
//! pose, so it is shared by every scene and kept in version control with the
//! clips it is used to make.

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::rig::{bone_name, joints, set_pose, subject, tell};

/// One bone's local `(translation, rotation, scale)`.
type BonePose = ([f32; 3], [f32; 4], [f32; 3]);

/// One saved pose, by bone name.
#[derive(Serialize, Deserialize, Default, Clone)]
pub(crate) struct Pose {
    pub(crate) bones: BTreeMap<String, BonePose>,
}

/// The poses on disk, by name, re-read when the library is changed here.
#[derive(Resource, Default)]
pub(crate) struct PoseLibrary {
    pub(crate) poses: Vec<(String, Pose)>,
    loaded_for: Option<std::path::PathBuf>,
    /// What the next Save Pose is called.
    pub(crate) name: String,
}

pub(super) fn register(app: &mut App) {
    app.init_resource::<PoseLibrary>().add_systems(
        Update,
        load_library.run_if(in_state(renzora_editor_framework::SplashState::Editor)).run_if(super::stage_mounted),
    );
}

fn folder(world: &World) -> Option<std::path::PathBuf> {
    Some(world.get_resource::<renzora::core::CurrentProject>()?.path.join("animations").join("poses"))
}

fn read(dir: &std::path::Path) -> Vec<(String, Pose)> {
    let mut out: Vec<(String, Pose)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "pose"))
        .filter_map(|e| {
            let name = e.path().file_stem()?.to_str()?.to_string();
            let pose = ron::from_str::<Pose>(&std::fs::read_to_string(e.path()).ok()?).ok()?;
            Some((name, pose))
        })
        .collect();
    out.sort_by_key(|p| p.0.to_lowercase());
    out
}

/// Read the library once per project.
fn load_library(world: &mut World) {
    let Some(dir) = folder(world) else { return };
    if world.resource::<PoseLibrary>().loaded_for.as_deref() == Some(dir.as_path()) {
        return;
    }
    let poses = read(&dir);
    let mut lib = world.resource_mut::<PoseLibrary>();
    lib.poses = poses;
    lib.loaded_for = Some(dir);
}

/// A file name for a pose name: anything a file system would refuse is
/// dropped.
fn file_stem(name: &str) -> String {
    let stem: String = name.chars().filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_')).collect();
    let stem = stem.trim().to_string();
    if stem.is_empty() { "Pose".to_string() } else { stem }
}

/// Save the subject's whole pose, as it stands now, under the typed name.
pub(crate) fn save_pose(world: &mut World) {
    let (Some(root), Some(dir)) = (subject(world), folder(world)) else { return };
    let bones: BTreeMap<String, BonePose> = joints(world, root)
        .into_iter()
        .filter_map(|e| {
            let t = world.get::<Transform>(e)?;
            Some((bone_name(world, e)?, (t.translation.to_array(), t.rotation.to_array(), t.scale.to_array())))
        })
        .collect();
    if bones.is_empty() {
        tell(world, false, "This model has no skeleton to save a pose of");
        return;
    }
    let typed = world.resource::<PoseLibrary>().name.trim().to_string();
    let base = file_stem(if typed.is_empty() { "Pose" } else { &typed });
    // Never overwrite: a second "Pose" becomes "Pose 2".
    let mut stem = base.clone();
    let mut n = 2;
    while dir.join(format!("{stem}.pose")).exists() {
        stem = format!("{base} {n}");
        n += 1;
    }
    let text = match ron::ser::to_string_pretty(&Pose { bones }, ron::ser::PrettyConfig::default()) {
        Ok(t) => t,
        Err(e) => {
            tell(world, false, format!("Could not save the pose: {e}"));
            return;
        }
    };
    if let Err(e) = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(dir.join(format!("{stem}.pose")), text)) {
        tell(world, false, format!("Could not save the pose: {e}"));
        return;
    }
    let poses = read(&dir);
    let mut lib = world.resource_mut::<PoseLibrary>();
    lib.poses = poses;
    lib.name.clear();
    tell(world, true, format!("Saved the pose \"{stem}\""));
}

/// Put a saved pose on the subject's bones (or the selected ones only, when
/// some are selected), held over the clip until keyed like any pose.
pub(crate) fn apply_pose(world: &mut World, name: &str) {
    let Some(root) = subject(world) else { return };
    let Some(pose) = world.resource::<PoseLibrary>().poses.iter().find(|(n, _)| n == name).map(|(_, p)| p.clone()) else { return };
    let all = joints(world, root);
    let selected: Vec<Entity> = world
        .get_resource::<renzora_editor_framework::EditorSelection>()
        .map(|s| s.get_all())
        .unwrap_or_default()
        .into_iter()
        .filter(|e| all.contains(e))
        .collect();
    let targets = if selected.is_empty() { all } else { selected };
    let out: Vec<(Entity, Transform)> = targets
        .into_iter()
        .filter_map(|e| {
            let (t, r, s) = pose.bones.get(&bone_name(world, e)?)?;
            Some((e, Transform { translation: Vec3::from_array(*t), rotation: Quat::from_array(*r).normalize(), scale: Vec3::from_array(*s) }))
        })
        .collect();
    if out.is_empty() {
        tell(world, false, format!("\"{name}\" has none of this model's bones"));
        return;
    }
    set_pose(world, &out);
}

pub(crate) fn delete_pose(world: &mut World, name: &str) {
    let Some(dir) = folder(world) else { return };
    let _ = std::fs::remove_file(dir.join(format!("{name}.pose")));
    let poses = read(&dir);
    world.resource_mut::<PoseLibrary>().poses = poses;
}
