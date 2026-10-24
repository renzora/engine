//! The skeleton tools: the bone tree, posing, IK, and the humanoid auto-rig
//! for a model that has no skeleton. These are built into the Animation
//! workspace's side panels (`stage::panels`).
//!
//! - [`skeleton_tree`] lists the model's joints as a tree (from its skins, so
//!   meshes and empty nodes are not bones). Clicking one selects it, the same
//!   as clicking it on the stage.
//! - [`pose_tools`] work on the selected bones, or on every bone when none is
//!   selected: key, back to rest, mirror onto the other side, flip the whole
//!   pose, and copy and paste poses between frames (also mirrored, for the
//!   other half of a walk).
//! - [`ik_list`]: with a limb's IK on, moving its hand or foot bends the
//!   elbow or knee to follow, instead of stretching the wrist away.
//! - [`humanoid_builder`]: seven points placed on the body build a
//!   Mixamo-compatible skeleton and skin, so Mixamo clips play on it as they
//!   are. The original file is kept, and Remove Rig puts it back.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_display, bind_text, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::{button, toggle_switch};
use renzora_ember::UiInteraction;
use renzora_rig::mirror::{mirror_name, mirror_rotation};

use super::markers::{MarkerSet, Markers};
use super::{bind_world, bone_name, joints, model_file, model_side_axis, on_click, rest_pose, set_pose, subject, tell};

/// A copied pose: each bone's local transform, by its name.
#[derive(Resource, Default)]
pub(crate) struct PoseClipboard(HashMap<String, Transform>);

/// Limbs with IK on, by the name of their end bone (hand or foot).
#[derive(Resource, Default)]
pub(crate) struct IkLimbs(pub(crate) HashSet<String>);

pub(super) fn register(app: &mut App) {
    app.init_resource::<PoseClipboard>().init_resource::<IkLimbs>();
    app.add_systems(
        PostUpdate,
        solve_ik
            .after(crate::timeline::pose::capture_pose)
            .before(bevy::app::AnimationSystems)
            .run_if(in_state(renzora_editor_framework::SplashState::Editor))
            .run_if(renzora::not_in_play_mode),
    );
}

fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

fn has_joints(w: &Rx) -> bool {
    subject(w.untracked()).is_some_and(|r| !joints(w.untracked(), r).is_empty())
}

/// A bone's name as the tree shows it: Mixamo's prefix is the same on every
/// bone and says nothing.
pub(crate) fn short_name(name: &str) -> String {
    name.trim_start_matches("mixamorig:").trim_start_matches("mixamorig_").to_string()
}

fn note(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node { margin: UiRect::vertical(Val::Px(2.0)), ..default() },
        ))
        .id()
}

fn row(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id()
}

fn action(commands: &mut Commands, fonts: &EmberFonts, label: &str, f: impl Fn(&mut World) + Send + Sync + 'static) -> Entity {
    let b = button(commands, &fonts.ui, label);
    on_click(commands, b, f);
    b
}

/// A column for one of the builders below.
fn column(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() })
        .id()
}

/// The skeleton as a tree; clicking a bone selects it.
pub(crate) fn skeleton_tree(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = column(commands);
    let count = note(commands, fonts, "");
    bind_text(commands, count, |w| {
        let n = subject(w.untracked()).map(|r| joints(w.untracked(), r).len()).unwrap_or(0);
        if n == 0 { "No skeleton yet.".to_string() } else { format!("{n} bones. Click one to select it.") }
    });
    let tree = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() }).id();
    keyed_list(commands, tree, skeleton_snapshot);
    commands.entity(col).add_children(&[count, tree]);
    col
}

/// The pose tools: key, rest, mirror, flip, copy and paste.
pub(crate) fn pose_tools(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = column(commands);
    let hint = note(commands, fonts, "Acts on the selected bones, or on every bone when none is selected.");
    let r1 = row(commands);
    let key = action(commands, fonts, "Key Pose  (K)", |w| {
        if !crate::timeline::pose::key_pose_bones(w) {
            tell(w, false, "Nothing to key: open a clip in the timeline, and pose or select a bone");
        }
    });
    let rest = action(commands, fonts, "Reset to Rest", reset_to_rest);
    let mirror = action(commands, fonts, "Mirror", |w| mirror_pose(w, false));
    let flip = action(commands, fonts, "Flip Pose", |w| mirror_pose(w, true));
    commands.entity(r1).add_children(&[key, rest, mirror, flip]);
    let r2 = row(commands);
    let copy = action(commands, fonts, "Copy", copy_pose);
    let paste = action(commands, fonts, "Paste", |w| paste_pose(w, false));
    let paste_m = action(commands, fonts, "Paste Mirrored", |w| paste_pose(w, true));
    commands.entity(r2).add_children(&[copy, paste, paste_m]);
    commands.entity(col).add_children(&[hint, r1, r2]);
    col
}

/// The humanoid auto-rig: the seven points, and building from them. For a
/// model that has a skeleton already, says so, and offers to take back a rig
/// this editor built.
pub(crate) fn humanoid_builder(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = column(commands);
    let has = note(commands, fonts, "This model has a skeleton. Its bones are listed on the right; Skin and Animate work on it.");
    bind_display(commands, has, has_joints);
    let body = column(commands);
    bind_display(commands, body, |w| subject(w.untracked()).is_some() && !has_joints(w));
    let hint = note(
        commands,
        fonts,
        "Stand the character in a T or A pose (as imported). Place the points, then generate a Mixamo-compatible skeleton and skin: Mixamo clips play on it as they are. The original file is kept.",
    );
    let marker_list = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() }).id();
    keyed_list(commands, marker_list, |w| marker_snapshot(w, MarkerSet::Body));
    let r = row(commands);
    let place = action(commands, fonts, "Place Points", |w| {
        if let Some(mut m) = w.get_resource_mut::<Markers>() {
            let next = m.points(MarkerSet::Body).iter().position(Option::is_none).unwrap_or(0);
            m.start(MarkerSet::Body, next);
        }
    });
    let clear = action(commands, fonts, "Clear Points", |w| {
        if let Some(mut m) = w.get_resource_mut::<Markers>() {
            m.clear(MarkerSet::Body);
        }
    });
    commands.entity(r).add_children(&[place, clear]);
    let generate = renzora_ember::widgets::accent_button(commands, fonts, accent(), "Generate Skeleton");
    commands.entity(generate).entry::<Node>().and_modify(|mut n| n.width = Val::Percent(100.0));
    on_click(commands, generate, build_body_rig);
    commands.entity(body).add_children(&[hint, marker_list, r, generate]);
    let unrig = unrig_row(commands, fonts);
    commands.entity(col).add_children(&[has, body, unrig]);
    col
}

/// Remove a rig this editor built, putting the original back.
pub(crate) fn unrig_row(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let r = row(commands);
    let unrig = action(commands, fonts, "Remove Rig (restore the original)", remove_rig);
    commands.entity(r).add_child(unrig);
    bind_display(commands, r, |w| {
        subject(w.untracked())
            .and_then(|r| model_file(w.untracked(), r))
            .is_some_and(|(abs, _)| renzora_rig::unrigged_path(&abs).exists())
    });
    r
}

/// The selected bone, read-only: what it is called, what it hangs from, how
/// long it is, and its partner on the other side.
pub(crate) fn bone_info(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = column(commands);
    let text = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())), Node { margin: UiRect::vertical(Val::Px(2.0)), ..default() }))
        .id();
    bind_text(commands, text, |w| {
        let world = w.untracked();
        let Some(bone) = world.get_resource::<renzora_editor_framework::EditorSelection>().and_then(|s| s.get()) else {
            return "Select a bone on the stage or in the tree.".to_string();
        };
        let Some(name) = world.get::<Name>(bone).map(|n| n.as_str().to_string()) else { return String::new() };
        let parent = world.get::<ChildOf>(bone).and_then(|p| world.get::<Name>(p.parent())).map(|n| short_name(n.as_str())).unwrap_or_else(|| "none".into());
        let length = world
            .get::<Children>(bone)
            .and_then(|c| c.iter().filter_map(|c| world.get::<Transform>(c)).map(|t| t.translation.length()).reduce(f32::max))
            .map(|l| format!("{l:.3}"))
            .unwrap_or_else(|| "end bone".into());
        let mirror = bone_name(world, bone).and_then(|n| mirror_name(&n)).map(|n| short_name(&n)).unwrap_or_else(|| "centre bone".into());
        format!("{}\nParent: {parent}\nLength: {length}\nMirror: {mirror}", short_name(&name))
    });
    commands.entity(col).add_child(text);
    col
}

// ── Skeleton tree ────────────────────────────────────────────────────────────

fn skeleton_snapshot(w: &Rx) -> KeyedSnapshot {
    let world = w.untracked();
    let Some(root) = subject(world) else {
        return KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) };
    };
    let js = joints(world, root);
    // Depth: how many joint ancestors.
    let rows: Vec<(Entity, String, usize)> = tree_order(world, &js)
        .into_iter()
        .map(|(e, depth)| (e, world.get::<Name>(e).map(|n| short_name(n.as_str())).unwrap_or_default(), depth))
        .collect();
    let items = rows
        .iter()
        .map(|(e, name, depth)| {
            let mut k = hasher();
            e.hash(&mut k);
            let mut h = hasher();
            (name, depth).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (e, name, depth) = rows[i].clone();
            bone_row(c, f, e, &name, depth)
        }),
    }
}

/// Joints in tree order (parents before children, siblings in skin order),
/// each with its depth among joints.
fn tree_order(world: &World, js: &[Entity]) -> Vec<(Entity, usize)> {
    let is_joint = |e: Entity| js.contains(&e);
    let roots: Vec<Entity> = js
        .iter()
        .copied()
        .filter(|&j| world.get::<ChildOf>(j).is_none_or(|p| !is_joint(p.parent())))
        .collect();
    let mut out = Vec::with_capacity(js.len());
    let mut stack: Vec<(Entity, usize)> = roots.into_iter().rev().map(|r| (r, 0)).collect();
    while let Some((e, d)) = stack.pop() {
        out.push((e, d));
        if let Some(children) = world.get::<Children>(e) {
            for c in children.iter().rev().filter(|c| is_joint(*c)) {
                stack.push((c, d + 1));
            }
        }
    }
    out
}

fn bone_row(commands: &mut Commands, fonts: &EmberFonts, bone: Entity, name: &str, depth: usize) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(22.0),
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::left(Val::Px(6.0 + depth.min(14) as f32 * 12.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    bind_bg(commands, row, move |w| {
        let selected = w.get_resource::<renzora_editor_framework::EditorSelection>().is_some_and(|s| s.is_selected(bone));
        let hot = matches!(w.get::<UiInteraction>(row), Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed));
        match (selected, hot) {
            (true, _) => rgb(accent()).with_alpha(0.28),
            (false, true) => rgb(hover_bg()),
            _ => Color::NONE,
        }
    });
    let icon = icon_text(commands, &fonts.phosphor, "bone", text_muted(), 12.0);
    let label = commands.spawn((Text::new(name), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())))).id();
    commands.entity(row).add_children(&[icon, label]);
    on_click(commands, row, move |w| {
        if let Some(sel) = w.get_resource::<renzora_editor_framework::EditorSelection>() {
            sel.set(Some(bone));
        }
    });
    row
}

// ── Markers ──────────────────────────────────────────────────────────────────

pub(super) fn marker_snapshot(w: &Rx, set: MarkerSet) -> KeyedSnapshot {
    let m = w.get_resource::<Markers>();
    let rows: Vec<(usize, &'static str, bool, bool)> = set
        .labels()
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let placed = m.is_some_and(|m| m.points(set).get(i).is_some_and(Option::is_some));
            let active = m.is_some_and(|m| m.placing == Some((set, i)));
            (i, *l, placed, active)
        })
        .collect();
    let items = rows
        .iter()
        .map(|r| {
            let mut k = hasher();
            r.0.hash(&mut k);
            let mut h = hasher();
            (r.2, r.3).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (index, label, placed, active) = rows[i];
            marker_row(c, f, set, index, label, placed, active)
        }),
    }
}

fn marker_row(commands: &mut Commands, fonts: &EmberFonts, set: MarkerSet, index: usize, label: &str, placed: bool, active: bool) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(24.0),
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::horizontal(Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(if active { rgb(accent()).with_alpha(0.22) } else { Color::NONE }),
            UiInteraction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    let dot = commands
        .spawn((
            Node { width: Val::Px(10.0), height: Val::Px(10.0), border_radius: BorderRadius::all(Val::Px(5.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(set.colour(index)),
        ))
        .id();
    let text = commands
        .spawn((Text::new(label), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())), Node { flex_grow: 1.0, ..default() }))
        .id();
    let state = if active { "click the model" } else if placed { "placed" } else { "to place" };
    let colour = if active { accent() } else if placed { (127, 209, 168) } else { text_muted() };
    let status = commands.spawn((Text::new(state), ui_font(&fonts.ui, 11.0), TextColor(rgb(colour)))).id();
    commands.entity(row).add_children(&[dot, text, status]);
    // Clicking a row (re)places that one point.
    on_click(commands, row, move |w| {
        if let Some(mut m) = w.get_resource_mut::<Markers>() {
            m.start(set, index);
        }
    });
    row
}

/// The placed points, in the model's own space (which is where the stage
/// places them; see `markers`).
pub(super) fn markers_in_model(world: &World, set: MarkerSet) -> Option<Vec<Vec3>> {
    world.get_resource::<Markers>()?.complete(set)
}

fn build_body_rig(world: &mut World) {
    let Some(root) = subject(world) else { return };
    if !joints(world, root).is_empty() {
        tell(world, false, "This model already has a skeleton");
        return;
    }
    let Some(p) = markers_in_model(world, MarkerSet::Body) else {
        tell(world, false, "Place all seven points first");
        return;
    };
    let Some((abs, rel)) = model_file(world, root) else {
        tell(world, false, "This entity has no model file to rig");
        return;
    };
    let markers = renzora_rig::humanoid::BodyMarkers { chin: p[0], shoulder: p[1], elbow: p[2], wrist: p[3], groin: p[4], knee: p[5], ankle: p[6] };
    match renzora_rig::rig_file(&abs, |glb| renzora_rig::humanoid::build(glb, &markers)) {
        Ok(applied) => {
            tell(world, true, format!("Rigged {rel}: {} bones over {} meshes. The original is kept beside it.", applied.bones, applied.meshes));
            if let Some(mut m) = world.get_resource_mut::<Markers>() {
                m.clear(MarkerSet::Body);
            }
            super::respawn::request(world, &rel);
        }
        Err(e) => tell(world, false, format!("Could not rig {rel}: {e}")),
    }
}

fn remove_rig(world: &mut World) {
    let Some(root) = subject(world) else { return };
    let Some((abs, rel)) = model_file(world, root) else { return };
    match renzora_rig::unrig_file(&abs) {
        Ok(true) => {
            tell(world, true, format!("Removed the rig from {rel}: the original is back"));
            super::respawn::request(world, &rel);
        }
        Ok(false) => tell(world, false, format!("{rel} has no original kept to restore")),
        Err(e) => tell(world, false, format!("Could not restore {rel}: {e}")),
    }
}

// ── Pose tools ───────────────────────────────────────────────────────────────

/// The bones a pose tool acts on: the selected joints, or all of them.
fn pose_targets(world: &World, root: Entity) -> Vec<Entity> {
    let js = joints(world, root);
    let selected: Vec<Entity> = world
        .get_resource::<renzora_editor_framework::EditorSelection>()
        .map(|s| s.get_all())
        .unwrap_or_default()
        .into_iter()
        .filter(|e| js.contains(e))
        .collect();
    if selected.is_empty() { js } else { selected }
}

fn reset_to_rest(world: &mut World) {
    let Some(root) = subject(world) else { return };
    let rest = rest_pose(world, root);
    let poses: Vec<(Entity, Transform)> = pose_targets(world, root).into_iter().filter_map(|e| Some((e, *rest.get(&e)?))).collect();
    set_pose(world, &poses);
}

/// Bones by their sanitised name, for finding a mirror partner.
fn by_name(world: &World, js: &[Entity]) -> HashMap<String, Entity> {
    js.iter().filter_map(|&e| Some((bone_name(world, e)?, e))).collect()
}

/// Everything mirroring needs about the rig.
struct MirrorFrame {
    rest: HashMap<Entity, Transform>,
    bind: HashMap<Entity, Mat4>,
    axis: Vec3,
}

impl MirrorFrame {
    fn new(world: &World, root: Entity) -> Self {
        Self { rest: rest_pose(world, root), bind: bind_world(world, root), axis: model_side_axis(world, root) }
    }

    /// The bind-pose world rotation of a bone's parent, the frame its local
    /// pose is written in.
    fn parent_rotation(&self, world: &World, bone: Entity) -> Quat {
        match world.get::<ChildOf>(bone).map(|c| c.parent()) {
            Some(p) => self
                .bind
                .get(&p)
                .map(|m| Quat::from_mat4(m).normalize())
                .or_else(|| world.get::<GlobalTransform>(p).map(|g| g.rotation()))
                .unwrap_or(Quat::IDENTITY),
            None => Quat::IDENTITY,
        }
    }

    /// The pose `pose` of bone `from`, mirrored onto bone `to`: the turn and
    /// move away from rest, reflected through the body's middle, then applied
    /// to `to`'s own rest. Right for any rig whose two sides mirror each other,
    /// whatever axes its bones use.
    fn mirror(&self, world: &World, from: Entity, to: Entity, pose: Transform) -> Option<Transform> {
        let (rest_from, rest_to) = (self.rest.get(&from)?, self.rest.get(&to)?);
        let (pf, pt) = (self.parent_rotation(world, from), self.parent_rotation(world, to));
        let turn = pf * (pose.rotation * rest_from.rotation.inverse()) * pf.inverse();
        let turn = mirror_rotation(turn, self.axis);
        let rotation = (pt.inverse() * turn * pt * rest_to.rotation).normalize();
        let shift = pf * (pose.translation - rest_from.translation);
        let shift = shift - self.axis * (2.0 * shift.dot(self.axis));
        Some(Transform { translation: rest_to.translation + pt.inverse() * shift, rotation, scale: pose.scale })
    }
}

/// `flip`: swap the whole pose left for right (centre bones mirrored onto
/// themselves). Otherwise copy each selected bone's pose onto its partner.
fn mirror_pose(world: &mut World, flip: bool) {
    let Some(root) = subject(world) else { return };
    let js = joints(world, root);
    let names = by_name(world, &js);
    let frame = MirrorFrame::new(world, root);
    let sources = if flip { js.clone() } else { pose_targets(world, root) };
    let mut out: Vec<(Entity, Transform)> = Vec::new();
    for src in sources {
        let Some(name) = bone_name(world, src) else { continue };
        let target = match mirror_name(&name).and_then(|m| names.get(&m).copied()) {
            Some(t) => t,
            None if flip => src,
            None => continue,
        };
        let Some(pose) = world.get::<Transform>(src).copied() else { continue };
        if let Some(t) = frame.mirror(world, src, target, pose) {
            out.push((target, t));
        }
    }
    if out.is_empty() {
        tell(world, false, "Nothing to mirror: select a left or right bone");
        return;
    }
    set_pose(world, &out);
}

fn copy_pose(world: &mut World) {
    let Some(root) = subject(world) else { return };
    let pose: HashMap<String, Transform> = pose_targets(world, root)
        .into_iter()
        .filter_map(|e| Some((bone_name(world, e)?, *world.get::<Transform>(e)?)))
        .collect();
    let n = pose.len();
    world.resource_mut::<PoseClipboard>().0 = pose;
    tell(world, true, format!("Copied the pose of {n} bones"));
}

fn paste_pose(world: &mut World, mirrored: bool) {
    let Some(root) = subject(world) else { return };
    let js = joints(world, root);
    let names = by_name(world, &js);
    let clip = world.resource::<PoseClipboard>().0.clone();
    if clip.is_empty() {
        tell(world, false, "No pose copied yet");
        return;
    }
    let frame = MirrorFrame::new(world, root);
    let mut out = Vec::new();
    for (name, pose) in &clip {
        let Some(&src) = names.get(name) else { continue };
        if !mirrored {
            out.push((src, *pose));
            continue;
        }
        let target = mirror_name(name).and_then(|m| names.get(&m).copied()).unwrap_or(src);
        if let Some(t) = frame.mirror(world, src, target, *pose) {
            out.push((target, t));
        }
    }
    set_pose(world, &out);
}

// ── IK ───────────────────────────────────────────────────────────────────────

/// The rig's limbs: each hand or foot joint with an elbow or knee and a
/// shoulder or hip above it.
fn limbs(world: &World) -> Vec<(Entity, String)> {
    let Some(root) = subject(world) else { return Vec::new() };
    let js = joints(world, root);
    js.iter()
        .filter_map(|&e| {
            let name = bone_name(world, e)?;
            let end = ["hand", "foot", "hand_l", "hand_r", "foot_l", "foot_r"].iter().any(|s| name.ends_with(s));
            let b = world.get::<ChildOf>(e)?.parent();
            let a = world.get::<ChildOf>(b)?.parent();
            (end && js.contains(&b) && js.contains(&a)).then_some((e, name))
        })
        .collect()
}

/// Each limb with an IK switch.
pub(crate) fn ik_list(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = column(commands);
    let hint = note(commands, fonts, "With IK on, move the hand or foot (W) and the elbow or knee bends to follow.");
    let list = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() }).id();
    keyed_list(commands, list, ik_snapshot);
    let none = note(commands, fonts, "No arms or legs found on this skeleton.");
    bind_display(commands, none, |w| limbs(w.untracked()).is_empty());
    commands.entity(col).add_children(&[hint, list, none]);
    col
}

fn ik_snapshot(w: &Rx) -> KeyedSnapshot {
    let rows = limbs(w.untracked());
    let items = rows
        .iter()
        .map(|(_, n)| {
            let mut k = hasher();
            n.hash(&mut k);
            (k.finish(), 0)
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let name = rows[i].1.clone();
            let row = c
                .spawn(Node { width: Val::Percent(100.0), height: Val::Px(24.0), align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() })
                .id();
            let label = c
                .spawn((Text::new(short_name(&name)), ui_font(&f.ui, 12.0), TextColor(rgb(text_primary())), Node { flex_grow: 1.0, ..default() }))
                .id();
            let sw = toggle_switch(c, false);
            let (get_name, set_name) = (name.clone(), name.clone());
            bind_2way(
                c,
                sw,
                move |w: &Rx| w.get_resource::<IkLimbs>().is_some_and(|l| l.0.contains(&get_name)),
                move |w: &mut World, on: &bool| {
                    let mut l = w.resource_mut::<IkLimbs>();
                    if *on {
                        l.0.insert(set_name.clone());
                    } else {
                        l.0.remove(&set_name);
                    }
                },
            );
            c.entity(row).add_children(&[label, sw]);
            row
        }),
    }
}

/// A hand or foot with IK on that was moved this frame: bend its limb to
/// reach where it was moved to, keeping the bone lengths. Runs between the
/// pose capture and the player, on last frame's world transforms.
fn solve_ik(world: &mut World) {
    let changed = match world.get_resource::<crate::timeline::pose::PoseLayer>() {
        Some(l) if !l.changed.is_empty() => l.changed.clone(),
        _ => return,
    };
    let Some(enabled) = world.get_resource::<IkLimbs>().map(|l| l.0.clone()) else { return };
    if enabled.is_empty() {
        return;
    }
    let mut out: Vec<(Entity, Transform)> = Vec::new();
    for c in changed {
        let Some(name) = bone_name(world, c) else { continue };
        if !enabled.contains(&name) {
            continue;
        }
        let Some(b) = world.get::<ChildOf>(c).map(|p| p.parent()) else { continue };
        let Some(a) = world.get::<ChildOf>(b).map(|p| p.parent()) else { continue };
        let (Some(ga), Some(gb), Some(gc)) = (world.get::<GlobalTransform>(a), world.get::<GlobalTransform>(b), world.get::<GlobalTransform>(c)) else {
            continue;
        };
        let (ga, gb, gc) = (*ga, *gb, *gc);
        let Some(moved) = world.get::<Transform>(c).copied() else { continue };
        let before = world.resource::<crate::timeline::pose::PoseLayer>().last_of(c).unwrap_or(moved);
        // Where the hand was moved to, in world space.
        let target = gb.transform_point(moved.translation);
        let (pa, pb, pc) = (ga.translation(), gb.translation(), gc.translation());
        let bend = pb - (pa + pc) * 0.5;
        let (up, lo) = renzora_rig::ik::solve(pa, pb, pc, target, pb + bend);
        let a_parent = world.get::<ChildOf>(a).and_then(|p| world.get::<GlobalTransform>(p.parent())).map(|g| g.rotation()).unwrap_or(Quat::IDENTITY);
        let upper_world = (up * ga.rotation()).normalize();
        let lower_world = (lo * gb.rotation()).normalize();
        let (Some(mut ta), Some(mut tb)) = (world.get::<Transform>(a).copied(), world.get::<Transform>(b).copied()) else { continue };
        ta.rotation = (a_parent.inverse() * upper_world).normalize();
        tb.rotation = (upper_world.inverse() * lower_world).normalize();
        // The hand stays the length of the forearm from the elbow: its move
        // was the target, not a stretch.
        let tc = Transform { translation: before.translation, ..moved };
        out.extend([(a, ta), (b, tb), (c, tc)]);
    }
    if !out.is_empty() {
        set_pose(world, &out);
    }
}
