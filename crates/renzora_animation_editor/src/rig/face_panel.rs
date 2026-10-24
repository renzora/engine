//! The face: rigging a head, and animating it. These are the Animation
//! workspace's face tools, built into its side panels (`stage::panels`).
//!
//! - [`face_setup`] (Rig mode, Face template): what each of the model's
//!   meshes was taken to be (skin, eyes, lashes, mouth interior, rigid), and
//!   four points to place (a mouth corner, both lips' middles, the chin).
//!   Build Face Rig adds jaw, lip, corner, eye, eyelid and brow bones and
//!   weights the head to them (`renzora_rig::face`), keeping the original.
//! - [`expression_sliders`] (Animate, Face tab): the expressions
//!   (`renzora_rig::expressions`); drag them to pose the face, then Key Face
//!   (or K) keys the face bones at the playhead. Moving the playhead drops an
//!   unkeyed face pose, like any other pose, and puts the sliders back to
//!   zero. A head that arrived with blend shapes gets a slider for each.
//! - [`performance`] (Animate, Performance tab): lip sync, keying the jaw and
//!   lips from a voice recording's loudness, and generated blinks.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_2way, bind_display, bind_text, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::{button, section, spin_slider};
use renzora_rig::expressions::{self, EXPRESSIONS};
use renzora_rig::face::{self, names, MeshRole};

use super::markers::{MarkerSet, Markers};
use super::rig_panel::{marker_snapshot, markers_in_model};
use super::{descendants, find_bone, model_file, on_click, rest_pose, set_pose, subject, tell};

/// The sliders' values, by expression id.
#[derive(Resource, Default)]
pub(crate) struct FaceSliders {
    values: HashMap<&'static str, f32>,
    /// The playhead the sliders' pose was made at; moving it zeroes them, as
    /// it drops the pose they made.
    at: Option<u32>,
}

/// How lip sync and blinks are generated: the Performance tab's settings.
#[derive(Resource)]
pub(crate) struct Performance {
    /// The voice recording lip sync reads.
    pub(crate) voice: Option<std::path::PathBuf>,
    /// How far the mouth opens at full volume.
    pub(crate) strength: f32,
    /// How much small changes in loudness are left out: higher keeps fewer
    /// keys and a calmer mouth.
    pub(crate) smoothing: f32,
    /// The shortest and longest gap between blinks, seconds.
    pub(crate) blink_min: f32,
    pub(crate) blink_max: f32,
}

impl Default for Performance {
    fn default() -> Self {
        Self { voice: None, strength: 0.85, smoothing: 0.06, blink_min: 2.5, blink_max: 6.0 }
    }
}

pub(super) fn register(app: &mut App) {
    app.init_resource::<FaceSliders>().init_resource::<Performance>();
    app.add_systems(Update, zero_sliders_on_scrub.run_if(in_state(renzora_editor_framework::SplashState::Editor)));
}

const FACE_BONES: &[&str] = &[
    names::HEAD, names::JAW, names::LIP_UPPER, names::LIP_LOWER, names::CORNER_L, names::CORNER_R, names::EYE_L, names::EYE_R,
    names::LID_UPPER_L, names::LID_UPPER_R, names::LID_LOWER_L, names::LID_LOWER_R, names::BROW_L, names::BROW_R,
];

fn rigged(world: &World) -> bool {
    subject(world).is_some_and(|r| find_bone(world, r, names::JAW).is_some())
}

fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

fn note(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((Text::new(text), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), Node { margin: UiRect::vertical(Val::Px(2.0)), ..default() }))
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

fn column(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() })
        .id()
}

/// Rig mode's Face template: what each mesh is, the four points, and
/// building.
pub(crate) fn face_setup(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = column(commands);
    let done = note(commands, fonts, "This head has a face rig. Its bones are listed on the right; animate it from Animate's Face tab.");
    bind_display(commands, done, |w| rigged(w.untracked()));
    let setup = column(commands);
    bind_display(commands, setup, |w| subject(w.untracked()).is_some() && !rigged(w.untracked()));
    let hint = note(
        commands,
        fonts,
        "Adds jaw, lip, mouth corner, eye, eyelid and brow bones. Place four points on the face, then build; the original file is kept.",
    );
    let roles_head = note(commands, fonts, "Meshes");
    let roles = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() }).id();
    keyed_list(commands, roles, roles_snapshot);
    let points_head = note(commands, fonts, "Points");
    let points = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() }).id();
    keyed_list(commands, points, |w| marker_snapshot(w, MarkerSet::Face));
    let r = row(commands);
    let place = action(commands, fonts, "Place Points", |w| {
        if let Some(mut m) = w.get_resource_mut::<Markers>() {
            let next = m.points(MarkerSet::Face).iter().position(Option::is_none).unwrap_or(0);
            m.start(MarkerSet::Face, next);
        }
    });
    let clear = action(commands, fonts, "Clear Points", |w| {
        if let Some(mut m) = w.get_resource_mut::<Markers>() {
            m.clear(MarkerSet::Face);
        }
    });
    commands.entity(r).add_children(&[place, clear]);
    let build_btn = renzora_ember::widgets::accent_button(commands, fonts, accent(), "Build Face Rig");
    commands.entity(build_btn).entry::<Node>().and_modify(|mut n| n.width = Val::Percent(100.0));
    on_click(commands, build_btn, build_face_rig);
    commands.entity(setup).add_children(&[hint, roles_head, roles, points_head, points, r, build_btn]);
    let unrig = super::rig_panel::unrig_row(commands, fonts);
    commands.entity(col).add_children(&[done, setup, unrig]);
    col
}

/// The expression sliders, with Key Face and Reset Face.
pub(crate) fn expression_sliders(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = column(commands);
    let none = note(commands, fonts, "No face rig on this model. Build one in Rig, with the Face template.");
    bind_display(commands, none, |w| !rigged(w.untracked()));
    let expr_body = column(commands);
    bind_display(commands, expr_body, |w| rigged(w.untracked()));
    let expr_hint = note(commands, fonts, "Drag to pose the face, then Key Face (or K) to key it at the playhead.");
    let mut kids = vec![expr_hint];
    for e in EXPRESSIONS {
        let (min, max) = if e.signed { (-1.0, 1.0) } else { (0.0, 1.0) };
        let s = spin_slider(commands, &fonts.ui, e.label, 0.0, min, max);
        commands.entity(s).entry::<Node>().and_modify(|mut n| n.width = Val::Percent(100.0));
        let id = e.id;
        bind_2way(
            commands,
            s,
            move |w: &Rx| w.get_resource::<FaceSliders>().and_then(|f| f.values.get(id).copied()).unwrap_or(0.0),
            move |w: &mut World, v: &f32| {
                w.resource_mut::<FaceSliders>().values.insert(id, *v);
                apply_sliders(w);
            },
        );
        kids.push(s);
    }
    let r2 = row(commands);
    let key = action(commands, fonts, "Key Face", key_face);
    let reset = action(commands, fonts, "Reset Face", |w| {
        w.resource_mut::<FaceSliders>().values.clear();
        apply_sliders(w);
    });
    commands.entity(r2).add_children(&[key, reset]);
    kids.push(r2);
    commands.entity(expr_body).add_children(&kids);

    // Blend shapes, for a head that came with them.
    let (morphs, morph_body) = section(commands, fonts, "drop-half", "Blend Shapes", accent());
    bind_display(commands, morphs, |w| !morph_targets(w.untracked()).is_empty());
    let morph_list = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() }).id();
    keyed_list(commands, morph_list, morph_snapshot);
    commands.entity(morph_body).add_child(morph_list);

    commands.entity(col).add_children(&[none, expr_body, morphs]);
    col
}

/// A slider bound to one of [`Performance`]'s numbers.
fn perf_slider(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    min: f32,
    max: f32,
    get: fn(&Performance) -> f32,
    set: fn(&mut Performance, f32),
) -> Entity {
    let s = spin_slider(commands, &fonts.ui, label, 0.0, min, max);
    commands.entity(s).entry::<Node>().and_modify(|mut n| n.width = Val::Percent(100.0));
    bind_2way(
        commands,
        s,
        move |w: &Rx| w.get_resource::<Performance>().map(get).unwrap_or(0.0),
        move |w: &mut World, v: &f32| set(&mut w.resource_mut::<Performance>(), *v),
    );
    s
}

/// Animate mode's Performance tab: lip sync from a recording, and blinks.
pub(crate) fn performance(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = column(commands);
    let none = note(commands, fonts, "No face rig on this model. Build one in Rig, with the Face template.");
    bind_display(commands, none, |w| !rigged(w.untracked()));

    let (lips, lips_body) = section(commands, fonts, "waveform", "Lip Sync", accent());
    bind_display(commands, lips, |w| rigged(w.untracked()));
    let lips_hint = note(
        commands,
        fonts,
        "Keys the jaw and lips from the recording's loudness, frame by frame, lengthening the clip to fit. It follows how loud the voice is, not which sounds it makes.",
    );
    let voice = note(commands, fonts, "");
    bind_text(commands, voice, |w| match w.get_resource::<Performance>().and_then(|p| p.voice.as_ref()) {
        Some(p) => format!("Voice: {}", p.file_name().and_then(|n| n.to_str()).unwrap_or("recording")),
        None => "Voice: none chosen".to_string(),
    });
    let r = row(commands);
    let choose = action(commands, fonts, "Choose Recording...", choose_voice);
    commands.entity(r).add_child(choose);
    let strength = perf_slider(commands, fonts, "Strength", 0.1, 1.5, |p| p.strength, |p, v| p.strength = v);
    let smoothing = perf_slider(commands, fonts, "Smoothing", 0.0, 0.3, |p| p.smoothing, |p, v| p.smoothing = v);
    let go = renzora_ember::widgets::accent_button(commands, fonts, accent(), "Generate Lip Sync");
    commands.entity(go).entry::<Node>().and_modify(|mut n| n.width = Val::Percent(100.0));
    on_click(commands, go, generate_lip_sync);
    commands.entity(lips_body).add_children(&[lips_hint, voice, r, strength, smoothing, go]);

    let (blinks, blinks_body) = section(commands, fonts, "eye", "Blinks", accent());
    bind_display(commands, blinks, |w| rigged(w.untracked()));
    let blink_hint = note(commands, fonts, "Keys natural blinks across the whole clip: a quick close, a short hold, a slower open.");
    let min = perf_slider(commands, fonts, "Shortest gap (s)", 0.5, 10.0, |p| p.blink_min, |p, v| p.blink_min = v);
    let max = perf_slider(commands, fonts, "Longest gap (s)", 0.5, 15.0, |p| p.blink_max, |p, v| p.blink_max = v);
    let b = action(commands, fonts, "Generate Blinks", generate_blinks);
    commands.entity(blinks_body).add_children(&[blink_hint, min, max, b]);

    let undo = note(commands, fonts, "Both key into the clip open in the timeline. Ctrl+Z takes them back.");
    bind_display(commands, undo, |w| rigged(w.untracked()));
    commands.entity(col).add_children(&[none, lips, blinks, undo]);
    col
}

// ── Setup ────────────────────────────────────────────────────────────────────

/// How the panel shows each kind of mesh: its name, what the rig does with
/// it, and its dot's colour.
fn role_look(role: MeshRole) -> (&'static str, &'static str, (u8, u8, u8)) {
    match role {
        MeshRole::Skin => ("Skin", "weighted to the face bones", (226, 168, 138)),
        MeshRole::Eye => ("Eyes", "turn with the eye bones", (110, 172, 232)),
        MeshRole::Lashes => ("Lashes", "follow the eyelids", (178, 142, 232)),
        MeshRole::MouthInterior => ("Mouth", "teeth and tongue, lower half on the jaw", (232, 112, 132)),
        MeshRole::Rigid => ("Rigid", "stay with the head", (150, 150, 162)),
    }
}

/// The model's meshes, one row per kind rather than one line per mesh.
///
/// A head arrives as anything from three meshes to thirty (a separate mesh per
/// tooth, per eyeball layer, and the whole head twice in some exports), and a
/// line each, with its role spelled out, buried the four points this mode is
/// actually for under a page of names. What matters at a glance is whether
/// each kind was found and how many; the names are on the row's tooltip.
fn roles_snapshot(w: &Rx) -> KeyedSnapshot {
    let world = w.untracked();
    let mut groups: Vec<(MeshRole, Vec<String>)> =
        [MeshRole::Skin, MeshRole::Eye, MeshRole::Lashes, MeshRole::MouthInterior, MeshRole::Rigid].into_iter().map(|r| (r, Vec::new())).collect();
    if let Some(root) = subject(world) {
        for e in descendants(world, root) {
            if world.get::<Mesh3d>(e).is_none() {
                continue;
            }
            let Some(name) = world.get::<Name>(e).map(|n| n.as_str().to_string()) else { continue };
            let role = face::mesh_role(&name, "");
            if let Some((_, names)) = groups.iter_mut().find(|(r, _)| *r == role) {
                names.push(name);
            }
        }
    }
    groups.retain(|(_, names)| !names.is_empty());
    let items = groups
        .iter()
        .map(|(role, names)| {
            let mut k = hasher();
            format!("{role:?}").hash(&mut k);
            let mut h = hasher();
            names.hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (role, names) = &groups[i];
            let (label, what, dot_colour) = role_look(*role);
            let row = c
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(1.0),
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                        border_radius: BorderRadius::all(Val::Px(5.0)),
                        ..default()
                    },
                    BackgroundColor(rgb(card_bg())),
                    renzora_ember::UiInteraction::default(),
                    renzora_ember::widgets::HoverTooltip::new(names.join("\n")),
                ))
                .id();
            let head = c
                .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() })
                .id();
            let dot = c
                .spawn((
                    Node { width: Val::Px(7.0), height: Val::Px(7.0), flex_shrink: 0.0, border_radius: BorderRadius::MAX, ..default() },
                    BackgroundColor(rgb(dot_colour)),
                ))
                .id();
            let name = c.spawn((Text::new(label), ui_font(&f.ui, 12.0), TextColor(rgb(text_primary())))).id();
            let count = c.spawn((Text::new(names.len().to_string()), ui_font(&f.ui, 11.0), TextColor(rgb(text_muted())))).id();
            let gap = c.spawn(Node { flex_grow: 1.0, ..default() }).id();
            let does = c.spawn((Text::new(what), ui_font(&f.ui, 11.0), TextColor(rgb(text_muted())))).id();
            renzora_ember::widgets::ellipsize(c, does, what, None);
            c.entity(head).add_children(&[dot, name, count, gap, does]);
            let list = names.join(", ");
            let meshes = c.spawn((Text::new(list.clone()), ui_font(&f.ui, 10.0), TextColor(rgb(text_muted())))).id();
            renzora_ember::widgets::ellipsize(c, meshes, list, None);
            c.entity(row).add_children(&[head, meshes]);
            row
        }),
    }
}

fn build_face_rig(world: &mut World) {
    let Some(root) = subject(world) else { return };
    if rigged(world) {
        tell(world, false, "This head already has a face rig");
        return;
    }
    let Some(p) = markers_in_model(world, MarkerSet::Face) else {
        tell(world, false, "Place all four points first");
        return;
    };
    let Some((abs, rel)) = model_file(world, root) else {
        tell(world, false, "This entity has no model file to rig");
        return;
    };
    let markers = face::FaceMarkers { mouth_corner: p[0], upper_lip: p[1], lower_lip: p[2], chin: p[3] };
    match renzora_rig::rig_file(&abs, |glb| face::build(glb, &markers)) {
        Ok((applied, _)) => {
            tell(
                world,
                true,
                format!("Rigged the face of {rel}: {} bones over {} meshes. The original is kept beside it.", applied.bones, applied.meshes),
            );
            if let Some(mut m) = world.get_resource_mut::<Markers>() {
                m.clear(MarkerSet::Face);
            }
            super::respawn::request(world, &rel);
        }
        Err(e) => tell(world, false, format!("Could not rig the face of {rel}: {e}")),
    }
}

// ── Expressions ──────────────────────────────────────────────────────────────

/// The face bones, by name, with their rest pose and the face's scale.
struct FaceRig {
    bones: Vec<(&'static str, Entity, Transform)>,
    scale: f32,
    /// Degrees each lid turns to close, by bone name, from the joints' glTF
    /// extras (`face::BLINK_EXTRA`). Empty for a head rigged before the lids
    /// were measured, whose blinks keep the stock angles.
    blink: Vec<(&'static str, f32)>,
}

/// A lid joint's closing angle, as the rig wrote it into the model.
fn blink_degrees(world: &World, bone: Entity) -> Option<f32> {
    let extras = world.get::<bevy::gltf::GltfExtras>(bone)?;
    let value: renzora::serde_json::Value = renzora::serde_json::from_str(&extras.value).ok()?;
    value.get(face::BLINK_EXTRA)?.as_f64().map(|d| d as f32)
}

fn face_rig(world: &World) -> Option<FaceRig> {
    let root = subject(world)?;
    let rest = rest_pose(world, root);
    let mut bones = Vec::new();
    let mut blink = Vec::new();
    for name in FACE_BONES {
        if let Some(e) = find_bone(world, root, name) {
            let r = rest.get(&e).copied().or_else(|| world.get::<Transform>(e).copied())?;
            bones.push((*name, e, r));
            if let Some(deg) = blink_degrees(world, e) {
                blink.push((*name, deg));
            }
        }
    }
    let at = |n: &str| bones.iter().find(|(b, ..)| *b == n).map(|(_, _, t)| t.translation);
    let scale = match (at(names::EYE_L), at(names::EYE_R)) {
        (Some(l), Some(r)) => l.distance(r).max(1e-4),
        _ => return None,
    };
    Some(FaceRig { bones, scale, blink })
}

/// The face's pose for a set of expression values.
fn face_pose(rig: &FaceRig, values: &[(&str, f32)]) -> Vec<(Entity, Transform)> {
    let blink = |bone: &str| rig.blink.iter().find(|(b, _)| *b == bone).map(|(_, d)| *d);
    let mix = expressions::mix_tuned(values, rig.scale, &blink);
    rig.bones
        .iter()
        .filter(|(name, ..)| *name != names::HEAD)
        .map(|(name, e, rest)| {
            let pose = match mix.iter().find(|(b, ..)| b == name) {
                Some((_, q, t)) => Transform { translation: rest.translation + *t, rotation: (rest.rotation * *q).normalize(), scale: rest.scale },
                None => *rest,
            };
            (*e, pose)
        })
        .collect()
}

fn apply_sliders(world: &mut World) {
    let Some(rig) = face_rig(world) else { return };
    let values: Vec<(&str, f32)> = world.resource::<FaceSliders>().values.iter().map(|(k, v)| (*k, *v)).collect();
    let pose = face_pose(&rig, &values);
    set_pose(world, &pose);
    let at = world.get_resource::<crate::AnimationEditorState>().map(|s| (s.scrub_time * 1000.0).round() as u32);
    world.resource_mut::<FaceSliders>().at = at;
}

/// Moving the playhead drops the unkeyed face pose; the sliders follow.
fn zero_sliders_on_scrub(state: Option<Res<crate::AnimationEditorState>>, mut sliders: ResMut<FaceSliders>) {
    let Some(state) = state else { return };
    let now = (state.scrub_time * 1000.0).round() as u32;
    if sliders.at.is_some_and(|at| at != now) || (state.is_previewing && !sliders.values.is_empty()) {
        sliders.values.clear();
        sliders.at = None;
    }
}

fn key_face(world: &mut World) {
    let Some(rig) = face_rig(world) else { return };
    let bones: Vec<Entity> = rig.bones.iter().filter(|(n, ..)| *n != names::HEAD).map(|(_, e, _)| *e).collect();
    if !crate::timeline::pose::key_bones(world, &bones) {
        tell(world, false, "No clip open to key into: create or pick one in the timeline");
    }
}

/// The open clip's length, if a clip is open.
fn clip_length(world: &World) -> Option<f32> {
    world.get_resource::<crate::timeline::clip::TimelineClip>()?.clip.as_ref().map(|c| c.duration)
}

fn generate_blinks(world: &mut World) {
    let Some(rig) = face_rig(world) else { return };
    let Some(duration) = clip_length(world) else {
        tell(world, false, "No clip open to key into: create or pick one in the timeline");
        return;
    };
    let seed = (duration * 1000.0) as u64 ^ 0x5eed;
    let (min, max) = {
        let p = world.resource::<Performance>();
        (p.blink_min.min(p.blink_max), p.blink_max.max(p.blink_min))
    };
    let starts = expressions::blink_times_between(duration, seed, min, max);
    let lids = [names::LID_UPPER_L, names::LID_UPPER_R, names::LID_LOWER_L, names::LID_LOWER_R];
    let mut keys: Vec<(Entity, f32, Transform)> = Vec::new();
    // The open pose at the start, so the lids are open before the first blink.
    for (name, e, rest) in &rig.bones {
        if lids.contains(name) {
            keys.push((*e, 0.0, *rest));
        }
    }
    for t0 in &starts {
        for o in [0.0, 0.035, 0.07, 0.1, 0.14, 0.19, expressions::BLINK_SECS] {
            let v = expressions::blink_curve(o);
            let pose = face_pose(&rig, &[("blink_l", v), ("blink_r", v)]);
            for (e, t) in pose {
                if rig.bones.iter().any(|(n, b, _)| *b == e && lids.contains(n)) {
                    keys.push((e, t0 + o, t));
                }
            }
        }
    }
    if crate::timeline::pose::key_transforms(world, &keys) {
        tell(world, true, format!("Keyed {} blinks over {duration:.1} s", starts.len()));
    } else {
        tell(world, false, "No clip open to key into: create or pick one in the timeline");
    }
}

fn choose_voice(world: &mut World) {
    let mut dialog = renzora_editor_framework::file_dialog::editor_file_dialog(world)
        .set_title("Lip sync from a voice recording")
        .add_filter("Audio", &["wav", "ogg", "mp3", "flac"]);
    if let Some(project) = world.get_resource::<renzora::core::CurrentProject>() {
        dialog = dialog.set_directory(&project.path);
    }
    if let Some(path) = dialog.pick_file() {
        world.resource_mut::<Performance>().voice = Some(path);
    }
}

fn generate_lip_sync(world: &mut World) {
    let Some(rig) = face_rig(world) else { return };
    if clip_length(world).is_none() {
        tell(world, false, "No clip open to key into: create or pick one in the timeline");
        return;
    }
    if world.resource::<Performance>().voice.is_none() {
        choose_voice(world);
    }
    let (Some(path), strength, smoothing) = ({
        let p = world.resource::<Performance>();
        (p.voice.clone(), p.strength, p.smoothing)
    }) else {
        return;
    };
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_string();
    let decoded = match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| renzora_audio::decode::decode(b, &ext)) {
        Ok(d) => d,
        Err(e) => {
            tell(world, false, format!("Could not read {}: {e}", path.display()));
            return;
        }
    };
    const FPS: f32 = 30.0;
    let env = super::lipsync::envelope(&decoded.samples, decoded.sample_rate, 2, FPS);
    let keep = super::lipsync::thin(&env, smoothing.max(0.005));
    let length = env.len() as f32 / FPS;
    if clip_length(world).is_some_and(|d| d < length) {
        crate::timeline::clip::set_clip_duration(world, length);
    }
    let mut keys = Vec::new();
    for i in keep {
        let open = (env[i] * strength).min(1.2);
        let pose = face_pose(&rig, &[("jaw_open", open), ("lips_part", open * 0.7), ("mouth_wide", open * 0.3)]);
        for (e, t) in pose {
            if rig.bones.iter().any(|(n, b, _)| *b == e && [names::JAW, names::LIP_UPPER, names::LIP_LOWER, names::CORNER_L, names::CORNER_R].contains(n)) {
                keys.push((e, i as f32 / FPS, t));
            }
        }
    }
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("the recording").to_string();
    if crate::timeline::pose::key_transforms(world, &keys) {
        tell(world, true, format!("Lip synced {length:.1} s of {name}. Play the recording beside the clip to hear it."));
    }
}

// ── Blend shapes ─────────────────────────────────────────────────────────────

/// `(entity carrying the weights, target index, name)` for every morph target
/// under the model.
fn morph_targets(world: &World) -> Vec<(Entity, usize, String)> {
    let Some(root) = subject(world) else { return Vec::new() };
    let meshes = world.get_resource::<Assets<Mesh>>();
    let mut out = Vec::new();
    for e in descendants(world, root) {
        let Some(weights) = world.get::<bevy::mesh::morph::MorphWeights>(e) else { continue };
        let names: Vec<String> = weights
            .first_mesh()
            .and_then(|h| meshes?.get(h))
            .and_then(|m| m.morph_target_names().map(|n| n.to_vec()))
            .unwrap_or_default();
        for i in 0..weights.weights().len() {
            out.push((e, i, names.get(i).cloned().unwrap_or_else(|| format!("Shape {}", i + 1))));
        }
    }
    out
}

fn morph_snapshot(w: &Rx) -> KeyedSnapshot {
    let rows = morph_targets(w.untracked());
    let items = rows
        .iter()
        .map(|(e, i, n)| {
            let mut k = hasher();
            (e, i).hash(&mut k);
            let mut h = hasher();
            n.hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, idx| {
            let (e, i, name) = rows[idx].clone();
            let s = spin_slider(c, &f.ui, &name, 0.0, 0.0, 1.0);
            c.entity(s).entry::<Node>().and_modify(|mut n| n.width = Val::Percent(100.0));
            bind_2way(
                c,
                s,
                move |w: &Rx| w.get::<bevy::mesh::morph::MorphWeights>(e).and_then(|m| m.weights().get(i).copied()).unwrap_or(0.0),
                move |w: &mut World, v: &f32| {
                    if let Some(mut m) = w.get_mut::<bevy::mesh::morph::MorphWeights>(e) {
                        if let Some(x) = m.weights_mut().get_mut(i) {
                            *x = *v;
                        }
                    }
                },
            );
            s
        }),
    }
}
