//! Editing a skeleton by hand on the stage: the draft, what can be done to
//! it, and drawing and clicking it. The panel is `rig::skeleton_panel`.
//!
//! The draft is a list of bones in the model's space, read from the model
//! file when editing starts (or a single root bone, for a model with no
//! skeleton yet: that is how a fully custom rig starts). Bones are added,
//! deleted, renamed, reparented and moved in the draft only; nothing is
//! written until Apply, which rigs the model's original again with the
//! draft (`renzora_rig::edit::rebuild`). Working on a draft rather than on
//! the model means a half-made skeleton never reaches the scene or the
//! clips, and Cancel costs nothing.
//!
//! The stage copy stands in its bind pose in Rig mode, which is the space
//! the draft is in, so the draft is drawn over it, and a click on the copy
//! places the selected joint: in the middle of the part under the cursor,
//! between where the ray goes into the mesh and where it comes out, since a
//! joint belongs inside a limb rather than on its skin.

use bevy::picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings, RayCastVisibility};
use bevy::prelude::*;

use renzora_rig::chain;
use renzora_rig::edit;
use renzora_rig::skin::Bone;
use renzora_rig::Glb;

use super::view::{StageCursor, StageNav};
use super::{StageMode, StageUi};
use crate::rig::{model_file, subject, tell};
use crate::studio_preview::{StudioPreviewCamera, StudioPreviewGizmoGroup, StudioPreviewModel};

/// The skeleton being edited. Empty (`root` is `None`) when not editing.
#[derive(Resource, Default)]
pub(crate) struct SkeletonDraft {
    /// The model the draft is for.
    pub(crate) root: Option<Entity>,
    pub(crate) bones: Vec<Bone>,
    /// Each draft bone as it was when editing started; `None` for one added
    /// since. Apply turns moved bones from these (`edit::aim`).
    pub(crate) before: Vec<Option<Bone>>,
    pub(crate) selected: Option<usize>,
    /// The next click on a bone in the list or on the stage makes it the
    /// selected bone's parent.
    pub(crate) picking_parent: bool,
    /// The next click on the model places the selected joint.
    pub(crate) placing: bool,
    /// The model's size, for drawing and for a new bone's length.
    pub(crate) size: f32,
    /// Bumped on every change, for the panel's list.
    pub(crate) version: u64,
}

impl SkeletonDraft {
    pub(crate) fn active(&self) -> bool {
        self.root.is_some()
    }

    fn changed(&mut self) {
        self.version = self.version.wrapping_add(1);
    }

    /// Bones in tree order (parents first, then their children), each with
    /// its depth: how the list shows them.
    pub(crate) fn tree(&self) -> Vec<(usize, usize)> {
        let mut out = Vec::with_capacity(self.bones.len());
        let mut stack: Vec<(usize, usize)> = (0..self.bones.len()).rev().filter(|&i| self.bones[i].parent.is_none()).map(|i| (i, 0)).collect();
        while let Some((i, d)) = stack.pop() {
            out.push((i, d));
            for c in (0..self.bones.len()).rev().filter(|&c| self.bones[c].parent == Some(i)) {
                stack.push((c, d + 1));
            }
        }
        out
    }

    fn is_under(&self, bone: usize, maybe_ancestor: usize) -> bool {
        let mut p = Some(bone);
        let mut steps = 0;
        while let Some(x) = p {
            if x == maybe_ancestor {
                return true;
            }
            steps += 1;
            if steps > self.bones.len() {
                return false;
            }
            p = self.bones[x].parent;
        }
        false
    }

    fn unique_name(&self, base: &str) -> String {
        if !self.bones.iter().any(|b| b.name == base) {
            return base.to_string();
        }
        (2..).map(|n| format!("{base}_{n}")).find(|n| !self.bones.iter().any(|b| &b.name == n)).unwrap_or_else(|| base.into())
    }
}

pub(super) fn register(app: &mut App) {
    app.init_resource::<SkeletonDraft>().add_systems(
        Update,
        (drop_when_subject_changes, click, draw)
            .chain()
            .run_if(in_state(renzora_editor_framework::SplashState::Editor))
            .run_if(super::stage_mounted),
    );
}

/// A draft belongs to one model: picking another drops it.
fn drop_when_subject_changes(state: Option<Res<crate::AnimationEditorState>>, mut draft: ResMut<SkeletonDraft>) {
    let subject = state.and_then(|s| s.selected_entity);
    if draft.active() && draft.root != subject {
        *draft = SkeletonDraft::default();
    }
}

// ── Operations, from the panel ───────────────────────────────────────────────

/// Start editing the subject's skeleton: its bones as the model file has
/// them, or one root bone at the bottom of a model with none.
pub(crate) fn start(world: &mut World) {
    let Some(root) = subject(world) else { return };
    let Some((abs, rel)) = model_file(world, root) else {
        tell(world, false, "This entity has no model file to give a skeleton");
        return;
    };
    let glb = match std::fs::read(&abs).map_err(|e| e.to_string()).and_then(|b| Glb::parse(&b).map_err(|e| e.to_string())) {
        Ok(g) => g,
        Err(e) => {
            tell(world, false, format!("Could not read {rel}: {e}"));
            return;
        }
    };
    let (base, top) = chain::long_axis(&glb).unwrap_or((Vec3::ZERO, Vec3::Y));
    let size = base.distance(top).max(1e-3);
    let mut bones = edit::read_bones(&glb);
    if bones.is_empty() {
        bones.push(Bone { name: "root".into(), parent: None, head: base, rotation: Quat::from_rotation_arc(Vec3::Y, (top - base).normalize_or(Vec3::Y)) });
    }
    let before = bones.iter().cloned().map(Some).collect();
    let mut draft = world.resource_mut::<SkeletonDraft>();
    *draft = SkeletonDraft { root: Some(root), bones, before, selected: Some(0), size, ..default() };
    draft.changed();
}

pub(crate) fn cancel(world: &mut World) {
    *world.resource_mut::<SkeletonDraft>() = SkeletonDraft::default();
}

pub(crate) fn select(world: &mut World, i: usize) {
    let refused = {
        let mut d = world.resource_mut::<SkeletonDraft>();
        if d.picking_parent {
            d.picking_parent = false;
            match d.selected {
                Some(child) if child == i || d.is_under(i, child) => true,
                Some(child) => {
                    d.bones[child].parent = Some(i);
                    d.changed();
                    false
                }
                None => false,
            }
        } else {
            d.selected = Some(i);
            d.changed();
            false
        }
    };
    if refused {
        tell(world, false, "A bone cannot hang from itself or from a bone below it");
    }
}

/// A new bone hanging from the selected one, carrying on in its direction
/// for the length of the selected bone (or a tenth of the model).
pub(crate) fn add_child(world: &mut World) {
    let mut d = world.resource_mut::<SkeletonDraft>();
    let Some(i) = d.selected else { return };
    let parent = d.bones[i].clone();
    let own_length = d.bones.iter().find(|b| b.parent == Some(i)).map(|c| c.head.distance(parent.head));
    let length = own_length.filter(|l| *l > 1e-4).unwrap_or(d.size * 0.1);
    let name = d.unique_name("bone");
    d.bones.push(Bone { name, parent: Some(i), head: parent.head + parent.rotation * Vec3::Y * length, rotation: parent.rotation });
    d.before.push(None);
    d.selected = Some(d.bones.len() - 1);
    d.changed();
}

/// Delete the selected bone; its children hang from its parent instead.
pub(crate) fn delete(world: &mut World) {
    if world.resource::<SkeletonDraft>().bones.len() == 1 {
        tell(world, false, "A skeleton needs at least one bone");
        return;
    }
    let mut d = world.resource_mut::<SkeletonDraft>();
    let Some(i) = d.selected else { return };
    let grand = d.bones[i].parent;
    d.bones.remove(i);
    d.before.remove(i);
    for b in d.bones.iter_mut() {
        b.parent = match b.parent {
            Some(p) if p == i => grand.map(|g| if g > i { g - 1 } else { g }),
            Some(p) if p > i => Some(p - 1),
            other => other,
        };
    }
    d.selected = grand.map(|g| if g > i { g - 1 } else { g }).or(Some(0));
    d.changed();
}

pub(crate) fn rename(world: &mut World, name: String) {
    let name = name.trim().to_string();
    let taken = {
        let mut d = world.resource_mut::<SkeletonDraft>();
        let Some(i) = d.selected else { return };
        if name.is_empty() || d.bones[i].name == name {
            return;
        }
        let taken = d.bones.iter().any(|b| b.name == name);
        if !taken {
            d.bones[i].name = name.clone();
            d.changed();
        }
        taken
    };
    if taken {
        tell(world, false, format!("Another bone is already called {name}"));
    }
}

pub(crate) fn set_head(world: &mut World, axis: usize, v: f32) {
    let mut d = world.resource_mut::<SkeletonDraft>();
    let Some(i) = d.selected else { return };
    if (d.bones[i].head[axis] - v).abs() > 1e-6 {
        d.bones[i].head[axis] = v;
        d.changed();
    }
}

/// Write the draft: rig the model's original again with these bones.
pub(crate) fn apply(world: &mut World) {
    let Some(root) = world.resource::<SkeletonDraft>().root else { return };
    let Some((abs, rel)) = model_file(world, root) else { return };
    let (mut bones, before) = {
        let d = world.resource::<SkeletonDraft>();
        topological(&d.bones, &d.before)
    };
    edit::aim(&before, &mut bones);
    match renzora_rig::rig_file(&abs, |glb| edit::rebuild(glb, &bones)) {
        Ok(applied) => {
            tell(
                world,
                true,
                format!("Rebuilt the skeleton of {rel}: {} bones over {} meshes. Weights were made fresh; paint them in Skin if a joint needs it.", applied.bones, applied.meshes),
            );
            cancel(world);
            crate::rig::respawn::request(world, &rel);
        }
        Err(e) => tell(world, false, format!("Could not rebuild {rel}: {e}")),
    }
}

/// The bones with every parent before its children, as a skin needs them,
/// with `before` kept in step.
fn topological(bones: &[Bone], before: &[Option<Bone>]) -> (Vec<Bone>, Vec<Option<Bone>>) {
    let mut order: Vec<usize> = Vec::with_capacity(bones.len());
    let mut stack: Vec<usize> = (0..bones.len()).rev().filter(|&i| bones[i].parent.is_none()).collect();
    while let Some(i) = stack.pop() {
        order.push(i);
        stack.extend((0..bones.len()).rev().filter(|&c| bones[c].parent == Some(i)));
    }
    let new_index = |old: usize| order.iter().position(|&o| o == old);
    let out = order
        .iter()
        .map(|&o| {
            let mut b = bones[o].clone();
            b.parent = b.parent.and_then(new_index);
            b
        })
        .collect();
    (out, order.iter().map(|&o| before.get(o).cloned().flatten()).collect())
}

// ── On the stage ─────────────────────────────────────────────────────────────

/// A click on the stage while editing: place the selected joint inside the
/// part under the cursor, or pick the joint nearest the cursor.
#[allow(clippy::too_many_arguments)]
fn click(
    mut draft: ResMut<SkeletonDraft>,
    ui: Res<StageUi>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Res<StageCursor>,
    nav: Res<StageNav>,
    mut ray_cast: MeshRayCast,
    roots: Query<(), With<StudioPreviewModel>>,
    parents: Query<&ChildOf>,
) {
    if !draft.active() || ui.mode != StageMode::Rig || !mouse.just_pressed(MouseButton::Left) || !cursor.over || nav.dragging.is_some() {
        return;
    }
    let Some(ray) = cursor.ray else { return };
    if draft.placing {
        let Some(i) = draft.selected else { return };
        let on_copy = |mut e: Entity| loop {
            if roots.contains(e) {
                return true;
            }
            match parents.get(e) {
                Ok(p) => e = p.parent(),
                Err(_) => return false,
            }
        };
        let settings = MeshRayCastSettings::default().with_visibility(RayCastVisibility::Any).with_filter(&on_copy).never_early_exit();
        let hits: Vec<Vec3> = ray_cast.cast_ray(ray, &settings).iter().map(|(_, h)| h.point).collect();
        let Some(first) = hits.first().copied() else { return };
        // In and out of the part under the cursor: the next hit along the ray.
        let inside = hits.get(1).map_or(first, |out| (first + *out) * 0.5);
        draft.bones[i].head = inside;
        draft.placing = false;
        draft.changed();
        return;
    }
    // The joint nearest the ray, within a little of it.
    let origin = ray.origin;
    let dir = *ray.direction;
    let near = draft
        .bones
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let t = (b.head - origin).dot(dir).max(0.0);
            (i, (origin + dir * t).distance(b.head) / t.max(1e-3))
        })
        .filter(|(_, angle)| *angle < 0.02)
        .min_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((i, _)) = near {
        if draft.picking_parent {
            let child = draft.selected;
            draft.picking_parent = false;
            if let Some(child) = child {
                if child != i && !draft.is_under(i, child) {
                    draft.bones[child].parent = Some(i);
                }
            }
        } else {
            draft.selected = Some(i);
        }
        draft.changed();
    }
}

/// The draft over the copy: a ball at each joint, a line to its parent, the
/// selected bone bright.
fn draw(mut gizmos: Gizmos<StudioPreviewGizmoGroup>, draft: Res<SkeletonDraft>, ui: Res<StageUi>, cams: Query<&GlobalTransform, With<StudioPreviewCamera>>) {
    if !draft.active() || ui.mode != StageMode::Rig {
        return;
    }
    let Ok(cam) = cams.single() else { return };
    let cam_pos = cam.translation();
    let normal = Color::srgb(0.95, 0.76, 0.29);
    let lit = Color::srgb(0.45, 0.75, 1.0);
    for (i, b) in draft.bones.iter().enumerate() {
        let selected = draft.selected == Some(i);
        let c = if selected { lit } else { normal };
        let r = (cam_pos.distance(b.head) * 0.009).max(1e-5);
        gizmos.sphere(Isometry3d::from_translation(b.head), if selected { r * 1.5 } else { r }, c);
        if let Some(p) = b.parent.and_then(|p| draft.bones.get(p)) {
            let child_selected = selected || draft.selected == b.parent;
            gizmos.line(p.head, b.head, if child_selected { lit } else { normal.with_alpha(0.8) });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bone(name: &str, parent: Option<usize>, y: f32) -> Bone {
        Bone { name: name.into(), parent, head: Vec3::Y * y, rotation: Quat::IDENTITY }
    }

    /// A bone reparented below one listed after it still comes out after
    /// its parent.
    #[test]
    fn topological_puts_parents_first() {
        let bones = vec![bone("a", None, 0.0), bone("b", Some(2), 1.0), bone("c", Some(0), 2.0)];
        let before = vec![None, None, None];
        let (out, _) = topological(&bones, &before);
        let names: Vec<&str> = out.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, ["a", "c", "b"]);
        assert_eq!(out[2].parent, Some(1));
    }

    #[test]
    fn a_bone_cannot_hang_below_itself() {
        let d = SkeletonDraft { root: Some(Entity::PLACEHOLDER), bones: vec![bone("a", None, 0.0), bone("b", Some(0), 1.0), bone("c", Some(1), 2.0)], ..default() };
        assert!(d.is_under(2, 0));
        assert!(!d.is_under(0, 2));
        assert_eq!(d.tree(), [(0, 0), (1, 1), (2, 2)]);
    }
}
