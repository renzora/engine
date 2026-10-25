//! A model's controls in the editor: reading and writing its `.controls`
//! file, driving the pose from the sliders, capturing a pose as a control,
//! and keying what the controls move. The panel is [`super::controls_panel`];
//! the data and the mixing are `renzora_rig::controls`.
//!
//! The sliders follow the rule every hand-made pose in the workspace does: a
//! pose that is not keyed lasts until the playhead moves. They write through
//! [`set_pose`] (the bone pose layer) and `timeline::morph::set_morph` (the
//! blend shape layer), so K over the stage keys them like a gizmo pose, and
//! moving the playhead drops the pose and puts the sliders back.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use bevy::mesh::morph::MorphWeights;
use bevy::prelude::*;

use renzora_rig::controls::{self, Control, ControlSet, ControlTarget, LocalPose, Tuner};

use super::{bone_name, descendants, joints, model_file, rest_pose, set_pose, subject, tell};

/// The open model's controls and the sliders' values.
#[derive(Resource, Default)]
pub(crate) struct ControlsState {
    pub(crate) set: ControlSet,
    /// The file `set` was read from (or will be written to): the subject's
    /// model with its extension swapped for `.controls`.
    pub(crate) path: Option<PathBuf>,
    /// Whether that file exists. A face rig with no file yet is offered the
    /// face preset; one whose file was emptied on purpose is not.
    pub(crate) on_disk: bool,
    /// Slider values by control name; a control not in here is untouched
    /// and sits at its default.
    pub(crate) values: HashMap<String, f32>,
    /// The playhead the sliders' pose was made at; moving it zeroes them, as
    /// it drops the pose they made.
    at: Option<u32>,
    /// The pose a bone with no bind pose (a node that is not a skin joint)
    /// had when a slider first moved it, so offsets do not stack on
    /// themselves frame after frame. Dropped with the pose.
    base: HashMap<Entity, Transform>,
    /// The name field above Save as Control.
    pub(crate) name: String,
    /// The control open for editing, by name.
    pub(crate) editing: Option<String>,
    /// The edit section's name field.
    pub(crate) rename: String,
    /// When the set last changed and was not yet written. Writes wait for a
    /// pause, so dragging a range slider does not rewrite the file every
    /// frame.
    unsaved: Option<Instant>,
}

impl ControlsState {
    pub(crate) fn value(&self, c: &Control) -> f32 {
        self.values.get(&c.name).copied().unwrap_or(c.default)
    }

    pub(crate) fn editing_control(&self) -> Option<&Control> {
        self.set.find(self.editing.as_deref()?)
    }
}

const SAVE_AFTER: Duration = Duration::from_millis(400);

pub(super) fn register(app: &mut App) {
    app.init_resource::<ControlsState>();
    app.add_systems(
        Update,
        (follow_subject, zero_on_scrub, flush).run_if(in_state(renzora_editor_framework::SplashState::Editor)),
    );
}

/// The controls file for the subject's model.
fn controls_path(world: &World) -> Option<PathBuf> {
    let root = subject(world)?;
    let (abs, _) = model_file(world, root)?;
    Some(abs.with_extension(controls::EXTENSION))
}

/// Read the subject's controls when the subject changes to another model.
///
/// Only touches the resource mutably when the file to show changed: the
/// panel's lists track it, and a write every frame would rebuild them every
/// frame.
fn follow_subject(world: &mut World) {
    let path = controls_path(world);
    if world.resource::<ControlsState>().path == path {
        return;
    }
    // Anything unsaved belongs to the model being left.
    write_now(world);
    let (set, on_disk) = match &path {
        Some(p) => match std::fs::read_to_string(p) {
            Ok(text) => match ControlSet::from_ron(&text) {
                Ok(set) => (set, true),
                Err(e) => {
                    tell(world, false, format!("Could not read {}: {e}", p.display()));
                    (ControlSet::default(), true)
                }
            },
            Err(_) => (ControlSet::default(), false),
        },
        None => (ControlSet::default(), false),
    };
    let mut s = world.resource_mut::<ControlsState>();
    s.set = set;
    s.on_disk = on_disk;
    s.path = path;
    s.values.clear();
    s.base.clear();
    s.at = None;
    s.editing = None;
}

/// Moving the playhead (or playing) drops the unkeyed pose; the sliders
/// follow it back to their defaults.
fn zero_on_scrub(state: Option<Res<crate::AnimationEditorState>>, mut controls: ResMut<ControlsState>) {
    let Some(state) = state else { return };
    let now = (state.scrub_time * 1000.0).round() as u32;
    let stale = controls.at.is_some_and(|at| at != now) || (state.is_previewing && !controls.values.is_empty());
    if stale {
        controls.values.clear();
        controls.base.clear();
        controls.at = None;
    }
}

fn flush(world: &mut World) {
    let due = world.resource::<ControlsState>().unsaved.is_some_and(|t| t.elapsed() >= SAVE_AFTER);
    if due {
        write_now(world);
    }
}

/// Note that the set changed; it is written shortly.
pub(crate) fn changed(world: &mut World) {
    world.resource_mut::<ControlsState>().unsaved = Some(Instant::now());
}

/// Write the set to its file now.
fn write_now(world: &mut World) {
    let (path, text) = {
        let mut s = world.resource_mut::<ControlsState>();
        if s.unsaved.take().is_none() {
            return;
        }
        let Some(path) = s.path.clone() else { return };
        (path, s.set.to_ron())
    };
    match text.map_err(|e| e.to_string()).and_then(|t| std::fs::write(&path, t).map_err(|e| e.to_string())) {
        Ok(()) => world.resource_mut::<ControlsState>().on_disk = true,
        Err(e) => tell(world, false, format!("Could not save {}: {e}", path.display())),
    }
}

/// Every named node under the subject, by the name the pose tools know it
/// by, so a control's bone names resolve with one walk instead of one per
/// bone.
fn named_nodes(world: &World, root: Entity) -> HashMap<String, Entity> {
    let mut out = HashMap::new();
    for e in descendants(world, root) {
        if let Some(n) = bone_name(world, e) {
            out.entry(n).or_insert(e);
        }
    }
    out
}

/// A blend shape target as the model has it now: the entity carrying the
/// weights and the index, preferring the target's name when it has one.
fn resolve_shape(world: &World, nodes: &HashMap<String, Entity>, mesh: &str, index: usize, name: &str) -> Option<(Entity, usize)> {
    let e = *nodes.get(&renzora::sanitize_id(mesh))?;
    let count = world.get::<MorphWeights>(e)?.weights().len();
    if !name.is_empty() {
        if let Some(i) = (0..count).find(|&i| crate::timeline::morph::target_name(world, e, i) == name) {
            return Some((e, i));
        }
    }
    (index < count).then_some((e, index))
}

/// What the face rig measured, for the blink controls: each lid's angle and
/// push, and the face's scale. A model with no face rig has none, and its
/// controls do exactly what their file says.
fn face_tuning(world: &World) -> (Vec<(String, renzora_rig::expressions::LidTuning)>, f32) {
    match super::face_panel::face_rig(world) {
        Some(rig) => (rig.blink.iter().map(|(n, t)| (renzora::sanitize_id(n), *t)).collect(), rig.scale),
        None => (Vec::new(), 1.0),
    }
}

/// Pose the model from the sliders.
pub(crate) fn apply(world: &mut World) {
    let Some(root) = subject(world) else { return };
    // Every control, not only the touched ones: the controls own the bones
    // they move, so moving one slider puts every controlled bone at its
    // controls' pose (rest, for the untouched ones). Mixing only the touched
    // controls left the others' bones wherever the clip had them, which on a
    // face whose clip was keyed against an earlier rig showed the stale lids
    // under a fresh jaw, and Reset Controls could not clear them. This is
    // what the Face tab's fixed sliders always did.
    let (set, values) = {
        let s = world.resource::<ControlsState>();
        let v: Vec<(String, f32)> = controls::full_values(&s.set, &|n| s.values.get(n).copied()).into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        (s.set.clone(), v)
    };
    let (lids, face_scale) = face_tuning(world);
    let lid = |b: &str| {
        let b = renzora::sanitize_id(b);
        lids.iter().find(|(n, _)| *n == b).map(|(_, t)| *t)
    };
    let tuner = Tuner { lid: &lid, face_scale };
    let refs: Vec<(&str, f32)> = values.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    let mixed = controls::mix(&set, &refs, &tuner);

    let rest = rest_pose(world, root);
    let nodes = named_nodes(world, root);
    let mut poses = Vec::new();
    for b in &mixed.bones {
        let Some(&e) = nodes.get(&renzora::sanitize_id(&b.bone)) else { continue };
        let base = match rest.get(&e) {
            Some(r) => *r,
            None => {
                let now = world.get::<Transform>(e).copied().unwrap_or_default();
                *world.resource_mut::<ControlsState>().base.entry(e).or_insert(now)
            }
        };
        poses.push((
            e,
            Transform {
                translation: base.translation + b.translation,
                rotation: (base.rotation * b.rotation).normalize(),
                scale: base.scale + b.scale,
            },
        ));
    }
    set_pose(world, &poses);
    let shapes: Vec<(Entity, usize, f32)> =
        mixed.shapes.iter().filter_map(|s| resolve_shape(world, &nodes, &s.mesh, s.index, &s.name).map(|(e, i)| (e, i, s.weight))).collect();
    for (e, i, w) in shapes {
        crate::timeline::morph::set_morph(world, e, i, w);
    }
    let at = world.get_resource::<crate::AnimationEditorState>().map(|s| (s.scrub_time * 1000.0).round() as u32);
    world.resource_mut::<ControlsState>().at = at;
}

/// Set one slider and pose the model.
pub(crate) fn set_value(world: &mut World, name: &str, value: f32) {
    world.resource_mut::<ControlsState>().values.insert(name.to_string(), value);
    apply(world);
}

/// Put every touched control back to rest, then forget the values.
pub(crate) fn reset(world: &mut World) {
    {
        let mut s = world.resource_mut::<ControlsState>();
        for v in s.values.values_mut() {
            *v = 0.0;
        }
    }
    apply(world);
    let mut s = world.resource_mut::<ControlsState>();
    s.values.clear();
}

/// The model's pose as control targets: every skin joint that differs from
/// its bind pose, and every blend shape that is on.
fn capture(world: &World) -> Option<Vec<ControlTarget>> {
    let root = subject(world)?;
    let rest = rest_pose(world, root);
    let local = |t: &Transform| LocalPose { translation: t.translation, rotation: t.rotation, scale: t.scale };
    let bones: Vec<(String, LocalPose, LocalPose)> = joints(world, root)
        .into_iter()
        .filter_map(|j| {
            let r = rest.get(&j)?;
            let now = world.get::<Transform>(j)?;
            Some((bone_name(world, j)?, local(r), local(now)))
        })
        .collect();
    let shapes: Vec<(String, usize, String, f32)> = super::face_panel::morph_targets(world)
        .into_iter()
        .filter_map(|(e, i, _)| {
            let w = world.get::<MorphWeights>(e)?.weights().get(i).copied()?;
            Some((bone_name(world, e)?, i, crate::timeline::morph::target_name(world, e, i), w))
        })
        .collect();
    let mut out = controls::capture_bones(&bones);
    out.extend(controls::capture_shapes(&shapes));
    Some(out)
}

/// What a capture found, for the toast.
fn describe(targets: &[ControlTarget]) -> String {
    let bones = targets.iter().filter(|t| matches!(t, ControlTarget::Bone { .. })).count();
    let shapes = targets.len() - bones;
    let plural = |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    format!("{} and {}", plural(bones, "bone", "bones"), plural(shapes, "blend shape", "blend shapes"))
}

/// Capture the pose and fail loudly when it is the rest pose, which would
/// make a slider that does nothing.
fn capture_or_tell(world: &mut World) -> Option<Vec<ControlTarget>> {
    if world.resource::<ControlsState>().path.is_none() {
        tell(world, false, "This model has no model file to keep controls beside");
        return None;
    }
    let targets = capture(world).unwrap_or_default();
    if targets.is_empty() {
        tell(world, false, "Nothing is posed: move some bones or blend shapes away from rest first");
        return None;
    }
    Some(targets)
}

/// Save as Control: the pose on the stage, as a new slider.
pub(crate) fn save_new(world: &mut World) {
    let Some(targets) = capture_or_tell(world) else { return };
    let what = describe(&targets);
    let name = {
        let mut s = world.resource_mut::<ControlsState>();
        let name = s.set.unique_name(&s.name.clone());
        s.set.controls.push(Control { name: name.clone(), targets, ..Default::default() });
        // The pose on the stage is this control at full, so its slider says
        // so; it drops with the pose like any other value.
        s.values.insert(name.clone(), 1.0);
        s.name.clear();
        name
    };
    changed(world);
    tell(world, true, format!("Saved control {name}: {what}"));
}

/// Re-capture the control open for editing: its `+1` pose (or its `-1` one)
/// becomes the pose on the stage.
pub(crate) fn recapture(world: &mut World, negative: bool) {
    let Some(name) = world.resource::<ControlsState>().editing.clone() else { return };
    let Some(targets) = capture_or_tell(world) else { return };
    let what = describe(&targets);
    {
        let mut s = world.resource_mut::<ControlsState>();
        let Some(c) = s.set.find_mut(&name) else { return };
        if negative {
            c.negative = targets;
            // A negative end needs a range that reaches it.
            if c.min >= 0.0 {
                c.min = -1.0;
            }
        } else {
            c.targets = targets;
        }
    }
    changed(world);
    let end = if negative { "negative end" } else { "pose" };
    tell(world, true, format!("Re-captured the {end} of {name}: {what}"));
}

/// Drop the edited control's own negative pose, so its negative end mirrors
/// the positive one again.
pub(crate) fn clear_negative(world: &mut World) {
    {
        let mut s = world.resource_mut::<ControlsState>();
        let Some(name) = s.editing.clone() else { return };
        let Some(c) = s.set.find_mut(&name) else { return };
        c.negative.clear();
    }
    changed(world);
}

pub(crate) fn rename(world: &mut World) {
    {
        let mut s = world.resource_mut::<ControlsState>();
        let Some(old) = s.editing.clone() else { return };
        let wanted = s.rename.trim().to_string();
        if wanted.is_empty() || wanted == old {
            return;
        }
        let new = s.set.unique_name(&wanted);
        if let Some(c) = s.set.find_mut(&old) {
            c.name = new.clone();
        }
        if let Some(v) = s.values.remove(&old) {
            s.values.insert(new.clone(), v);
        }
        s.editing = Some(new.clone());
        s.rename = new;
    }
    changed(world);
}

pub(crate) fn delete(world: &mut World) {
    let Some(name) = world.resource::<ControlsState>().editing.clone() else { return };
    // Back to rest first, so the deleted control does not leave its pose
    // behind on the stage.
    if world.resource::<ControlsState>().values.contains_key(&name) {
        set_value(world, &name, 0.0);
    }
    {
        let mut s = world.resource_mut::<ControlsState>();
        s.set.controls.retain(|c| c.name != name);
        s.values.remove(&name);
        s.editing = None;
    }
    changed(world);
    tell(world, true, format!("Deleted control {name}"));
}

/// Set the edited control's range.
pub(crate) fn set_range(world: &mut World, min: Option<f32>, max: Option<f32>) {
    {
        let mut s = world.resource_mut::<ControlsState>();
        let Some(name) = s.editing.clone() else { return };
        let Some(c) = s.set.find_mut(&name) else { return };
        if let Some(v) = min {
            c.min = v.min(0.0);
        }
        if let Some(v) = max {
            c.max = v.max(0.05);
        }
        c.default = c.clamp(c.default);
    }
    changed(world);
}

/// Use Face Preset: the face rig's built-in expressions, written as this
/// model's controls.
pub(crate) fn use_face_preset(world: &mut World) {
    let Some(rig) = super::face_panel::face_rig(world) else {
        tell(world, false, "This model has no face rig to make the preset for");
        return;
    };
    if world.resource::<ControlsState>().path.is_none() {
        tell(world, false, "This model has no model file to keep controls beside");
        return;
    }
    let preset = controls::face_preset(rig.scale);
    let n = preset.controls.len();
    world.resource_mut::<ControlsState>().set = preset;
    changed(world);
    write_now(world);
    tell(world, true, format!("Added the face preset: {n} controls, ready to rename, re-capture or delete"));
}

/// Key Controls: key every bone and blend shape any control moves, at the
/// playhead. All of them, not only the touched controls', because a slider
/// poses every controlled bone ([`apply`]), so that is the pose on the stage
/// and the one K keys too. Like Key Face before it.
pub(crate) fn key_controls(world: &mut World) {
    let Some(root) = subject(world) else { return };
    let (bone_names, shape_refs) = {
        let s = world.resource::<ControlsState>();
        let pick: Vec<&Control> = s.set.controls.iter().collect();
        let mut bones: Vec<String> = Vec::new();
        let mut shapes: Vec<(String, usize, String)> = Vec::new();
        for c in pick {
            for t in c.all_targets() {
                match t {
                    ControlTarget::Bone { bone, .. } => {
                        if !bones.contains(bone) {
                            bones.push(bone.clone());
                        }
                    }
                    ControlTarget::BlendShape { mesh, index, name, .. } => shapes.push((mesh.clone(), *index, name.clone())),
                }
            }
        }
        (bones, shapes)
    };
    if bone_names.is_empty() && shape_refs.is_empty() {
        tell(world, false, "No controls to key: save one first");
        return;
    }
    let nodes = named_nodes(world, root);
    let bones: Vec<Entity> = bone_names.iter().filter_map(|b| nodes.get(&renzora::sanitize_id(b)).copied()).collect();
    let time = world.get_resource::<crate::AnimationEditorState>().map(|s| s.scrub_time).unwrap_or(0.0);
    let mut shapes: Vec<(Entity, usize, f32, f32)> = Vec::new();
    for (mesh, index, name) in &shape_refs {
        let Some((e, i)) = resolve_shape(world, &nodes, mesh, *index, name) else { continue };
        if shapes.iter().any(|(se, si, ..)| *se == e && *si == i) {
            continue;
        }
        let Some(w) = world.get::<MorphWeights>(e).and_then(|m| m.weights().get(i).copied()) else { continue };
        shapes.push((e, i, time, w));
    }
    let keyed_bones = !bones.is_empty() && crate::timeline::pose::key_bones(world, &bones);
    let keyed_shapes = !shapes.is_empty() && crate::timeline::morph::key_morphs(world, &shapes);
    if !keyed_bones && !keyed_shapes {
        tell(world, false, "No clip open to key into: create or pick one in the timeline");
    }
}
