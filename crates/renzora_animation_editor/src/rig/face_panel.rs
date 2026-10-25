//! The face: rigging a head, and animating it. These are the Animation
//! workspace's face tools, built into its side panels (`stage::panels`).
//!
//! - [`face_setup`] (Rig mode, Face template): what each of the model's
//!   meshes was taken to be (skin, eyes, lashes, mouth interior, rigid), and
//!   four points to place (a mouth corner, both lips' middles, the chin).
//!   Build Face Rig adds jaw, lip, corner, eye, eyelid and brow bones and
//!   weights the head to them (`renzora_rig::face`), keeping the original.
//! - Animating it is the Controls tab (`super::controls_panel`): the face's
//!   expressions are a preset of ordinary controls there, the blinks tuned
//!   from [`face_rig`]'s lid measurements. [`blend_shapes`] is that tab's
//!   raw blend shape list, for a head that arrived with them, keyed into the
//!   clip's blend shape tracks (`timeline::morph`).
//! - [`performance`] (Animate, Performance tab): lip sync, keying the jaw and
//!   lips from a voice recording's loudness, and generated blinks.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_2way, bind_display, bind_text, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::{button, section, spin_slider};
use renzora_rig::expressions;
use renzora_rig::face::{self, names, MeshRole};

use super::markers::{MarkerSet, Markers};
use super::rig_panel::{marker_snapshot, markers_in_model};
use super::{descendants, find_bone, model_file, on_click, rest_pose, subject, tell};

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
    app.init_resource::<Performance>();
}

const FACE_BONES: &[&str] = &[
    names::HEAD, names::JAW, names::LIP_UPPER, names::LIP_LOWER, names::CORNER_L, names::CORNER_R, names::EYE_L, names::EYE_R,
    names::LID_UPPER_L, names::LID_UPPER_R, names::LID_LOWER_L, names::LID_LOWER_R, names::BROW_L, names::BROW_R,
];

/// The points this head needs: the face's four, or five when it has no eye
/// meshes and so needs an eye clicked too.
fn face_set(world: &World) -> MarkerSet {
    let Some(root) = subject(world) else { return MarkerSet::Face };
    let has_eyes = descendants(world, root).into_iter().any(|e| {
        world.get::<Mesh3d>(e).is_some() && world.get::<Name>(e).is_some_and(|n| face::mesh_role(n.as_str(), "") == MeshRole::Eye)
    });
    if has_eyes { MarkerSet::Face } else { MarkerSet::FacePaintedEyes }
}

pub(crate) fn rigged(world: &World) -> bool {
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
    let done = note(commands, fonts, "This head has a face rig. Its bones are listed on the right; animate it from Animate's Controls tab, starting from its face preset.");
    bind_display(commands, done, |w| rigged(w.untracked()));
    let setup = column(commands);
    bind_display(commands, setup, |w| subject(w.untracked()).is_some() && !rigged(w.untracked()));
    let hint = note(
        commands,
        fonts,
        "Adds jaw, lip, mouth corner, eye, eyelid and brow bones. Place the points on the face, then build; the original file is kept.",
    );
    let painted = note(
        commands,
        fonts,
        "No eye meshes: the eyes are part of the skin. Place a point on one eye so the rig knows where they are; with nothing under the lids to cover, blinks and eye turns have no effect.",
    );
    bind_display(commands, painted, |w| face_set(w.untracked()) == MarkerSet::FacePaintedEyes);
    let roles_head = note(commands, fonts, "Meshes");
    let roles = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() }).id();
    keyed_list(commands, roles, roles_snapshot);
    let points_head = note(commands, fonts, "Points");
    let points = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() }).id();
    keyed_list(commands, points, |w| marker_snapshot(w, face_set(w.untracked())));
    let r = row(commands);
    let place = action(commands, fonts, "Place Points", |w| {
        let set = face_set(w);
        if let Some(mut m) = w.get_resource_mut::<Markers>() {
            // Sized to the set first, so the eye row counts as unplaced.
            m.start(set, 0);
            let next = m.points(set).iter().position(Option::is_none).unwrap_or(0);
            m.start(set, next);
        }
    });
    let clear = action(commands, fonts, "Clear Points", |w| {
        let set = face_set(w);
        if let Some(mut m) = w.get_resource_mut::<Markers>() {
            m.clear(set);
        }
    });
    commands.entity(r).add_children(&[place, clear]);
    let build_btn = renzora_ember::widgets::accent_button(commands, fonts, accent(), "Build Face Rig");
    commands.entity(build_btn).entry::<Node>().and_modify(|mut n| n.width = Val::Percent(100.0));
    on_click(commands, build_btn, build_face_rig);
    commands.entity(setup).add_children(&[hint, painted, roles_head, roles, points_head, points, r, build_btn]);
    let unrig = super::rig_panel::unrig_row(commands, fonts);
    commands.entity(col).add_children(&[done, setup, unrig]);
    col
}

/// The model's blend shapes, a slider each, for a model that came with them.
/// Part of the Controls tab: what is set here is captured by Save as Control.
pub(crate) fn blend_shapes(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (morphs, morph_body) = section(commands, fonts, "drop-half", "Blend Shapes", accent());
    bind_display(commands, morphs, |w| !morph_targets(w.untracked()).is_empty());
    let morph_hint = note(
        commands,
        fonts,
        "Drag to shape the model, then Key Blend Shapes (or K) to key the ones you moved at the playhead, or Save as Control above to make a slider of them.",
    );
    let morph_list = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() }).id();
    keyed_list(commands, morph_list, morph_snapshot);
    let morph_row = row(commands);
    let key_shapes = action(commands, fonts, "Key Blend Shapes", key_blend_shapes);
    commands.entity(morph_row).add_child(key_shapes);
    commands.entity(morph_body).add_children(&[morph_hint, morph_list, morph_row]);
    morphs
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
    let set = face_set(world);
    let Some(p) = markers_in_model(world, set) else {
        tell(world, false, "Place all the points first");
        return;
    };
    let Some((abs, rel)) = model_file(world, root) else {
        tell(world, false, "This entity has no model file to rig");
        return;
    };
    let markers = face::FaceMarkers { mouth_corner: p[0], upper_lip: p[1], lower_lip: p[2], chin: p[3], eye: p.get(4).copied() };
    match renzora_rig::rig_file(&abs, |glb| face::build(glb, &markers)) {
        Ok((applied, _)) => {
            tell(
                world,
                true,
                format!("Rigged the face of {rel}: {} bones over {} meshes. The original is kept beside it.", applied.bones, applied.meshes),
            );
            if let Some(mut m) = world.get_resource_mut::<Markers>() {
                m.clear(set);
            }
            super::respawn::request(world, &rel);
        }
        Err(e) => tell(world, false, format!("Could not rig the face of {rel}: {e}")),
    }
}

// ── Expressions ──────────────────────────────────────────────────────────────

/// The face bones, by name, with their rest pose and the face's scale.
/// Generated blinks and lip sync key these bones directly; the Controls tab
/// reads the lid measurements and the scale for its tuned blink controls.
pub(crate) struct FaceRig {
    bones: Vec<(&'static str, Entity, Transform)>,
    pub(crate) scale: f32,
    /// How each lid closes, by bone name, from the joints' glTF extras
    /// (`face::BLINK_EXTRA`, `face::BLINK_PUSH_EXTRA`). Empty for a head
    /// rigged before the lids were measured, whose blinks keep the stock
    /// angles.
    pub(crate) blink: Vec<(&'static str, expressions::LidTuning)>,
}

/// A lid joint's closing angle and push, as the rig wrote them into the
/// model. A head rigged before the push was measured reads as no push.
fn blink_tuning(world: &World, bone: Entity) -> Option<expressions::LidTuning> {
    let extras = world.get::<bevy::gltf::GltfExtras>(bone)?;
    let value: renzora::serde_json::Value = renzora::serde_json::from_str(&extras.value).ok()?;
    let degrees = value.get(face::BLINK_EXTRA)?.as_f64()? as f32;
    let push = value.get(face::BLINK_PUSH_EXTRA).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
    Some(expressions::LidTuning { degrees, push })
}

pub(crate) fn face_rig(world: &World) -> Option<FaceRig> {
    let root = subject(world)?;
    let rest = rest_pose(world, root);
    let mut bones = Vec::new();
    let mut blink = Vec::new();
    for name in FACE_BONES {
        if let Some(e) = find_bone(world, root, name) {
            let r = rest.get(&e).copied().or_else(|| world.get::<Transform>(e).copied())?;
            bones.push((*name, e, r));
            if let Some(tuning) = blink_tuning(world, e) {
                blink.push((*name, tuning));
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

/// The web editor has no native file dialog, so it says why nothing opened.
#[cfg(target_arch = "wasm32")]
fn choose_voice(world: &mut World) {
    tell(world, false, "Picking a voice recording needs the desktop editor");
}

#[cfg(not(target_arch = "wasm32"))]
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
pub(crate) fn morph_targets(world: &World) -> Vec<(Entity, usize, String)> {
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
                // Held over the clip until the playhead moves, and keyed by
                // Key Blend Shapes, Key Controls or K (see `timeline::morph`).
                move |w: &mut World, v: &f32| crate::timeline::morph::set_morph(w, e, i, *v),
            );
            s
        }),
    }
}

/// Key the blend shapes at the playhead: the ones moved by hand, or, when
/// none were, every shape at its current weight (keying the face as it is).
fn key_blend_shapes(world: &mut World) {
    let posed = world.get_resource::<crate::timeline::morph::MorphLayer>().is_some_and(|l| !l.posed.is_empty());
    let keyed = if posed {
        crate::timeline::morph::key_posed_morphs(world)
    } else {
        let Some(time) = world.get_resource::<crate::AnimationEditorState>().map(|s| s.scrub_time) else { return };
        let keys: Vec<(Entity, usize, f32, f32)> = morph_targets(world)
            .into_iter()
            .filter_map(|(e, i, _)| {
                let w = world.get::<bevy::mesh::morph::MorphWeights>(e)?.weights().get(i).copied()?;
                Some((e, i, time, w))
            })
            .collect();
        crate::timeline::morph::key_morphs(world, &keys)
    };
    if !keyed {
        tell(world, false, "No clip open to key into: create or pick one in the timeline");
    }
}
