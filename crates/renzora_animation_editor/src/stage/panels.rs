//! The panels either side of the stage: `anim_tools` on the left,
//! `anim_inspector` on the right. Each holds one column per mode and shows
//! the one for the mode the stage is in, so switching mode changes both sides
//! at once without rebuilding either.
//!
//! | Mode | Left | Right |
//! |---|---|---|
//! | Rig | template (humanoid or face), its points, build | skeleton tree, bone |
//! | Skin | auto weights, the brush, the bone to paint | test bend, checks, save |
//! | Animate | controls (IK, pose library), face sliders | pose, clip, performance |
//! | Logic | parameters | the animator's properties |
//!
//! The pieces themselves mostly live with what they drive (`rig::rig_panel`,
//! `rig::face_panel`, the animator panels); this is where they are arranged.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_display, bind_text, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::{button, section, segmented, spin_slider, toggle_switch};
use renzora_ember::UiInteraction;

use super::clone::StageRig;
use super::poses::{apply_pose, delete_pose, save_pose, PoseLibrary};
use super::skin::{self, SkinEdit};
use super::{StageMode, StageUi};
use crate::rig::face_panel::{expression_sliders, face_setup, performance};
use crate::rig::on_click;
use crate::rig::rig_panel::{bone_info, humanoid_builder, ik_list, pose_tools, short_name, skeleton_tree};

pub(super) fn register(app: &mut App) {
    app.register_panel_content("anim_tools", true, build_tools);
    app.register_panel_content("anim_inspector", true, build_inspector);
}

fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

fn mode_is(w: &Rx, m: StageMode) -> bool {
    w.get_resource::<StageUi>().is_some_and(|u| u.mode == m)
}

fn pane(commands: &mut Commands, name: &str) -> Entity {
    commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, padding: UiRect::all(Val::Px(8.0)), row_gap: Val::Px(8.0), ..default() },
            Name::new(name.to_string()),
        ))
        .id()
}

fn column(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() })
        .id()
}

fn note(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands.spawn((Text::new(text), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id()
}

fn row(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node { width: Val::Percent(100.0), flex_wrap: FlexWrap::Wrap, align_items: AlignItems::Center, column_gap: Val::Px(6.0), row_gap: Val::Px(6.0), ..default() })
        .id()
}

fn action(commands: &mut Commands, fonts: &EmberFonts, label: &str, f: impl Fn(&mut World) + Send + Sync + 'static) -> Entity {
    let b = button(commands, &fonts.ui, label);
    on_click(commands, b, f);
    b
}

/// One column per mode, shown while the stage is in that mode.
fn per_mode(commands: &mut Commands, parent: Entity, columns: [Entity; 4]) {
    for (m, c) in StageMode::ALL.into_iter().zip(columns) {
        bind_display(commands, c, move |w| mode_is(w, m));
    }
    commands.entity(parent).add_children(&columns);
}

/// A segmented tab bar over `bodies`, the selected tab read from and written
/// to the workspace's settings.
fn tabs(
    commands: &mut Commands,
    fonts: &EmberFonts,
    labels: &[&str],
    bodies: Vec<Entity>,
    get: fn(&StageUi) -> usize,
    set: fn(&mut StageUi, usize),
) -> Entity {
    let col = column(commands);
    let bar = segmented(commands, &fonts.ui, labels, 0);
    commands.entity(bar).entry::<Node>().and_modify(|mut n| n.width = Val::Percent(100.0));
    bind_2way(
        commands,
        bar,
        move |w: &Rx| w.get_resource::<StageUi>().map(get).unwrap_or(0),
        move |w: &mut World, i: &usize| set(&mut w.resource_mut::<StageUi>(), *i),
    );
    let mut kids = vec![bar];
    for (i, b) in bodies.into_iter().enumerate() {
        bind_display(commands, b, move |w| w.get_resource::<StageUi>().map(get) == Some(i));
        kids.push(b);
    }
    commands.entity(col).add_children(&kids);
    col
}

fn titled(commands: &mut Commands, fonts: &EmberFonts, icon: &str, title: &str, body: Entity) -> Entity {
    let (s, inner) = section(commands, fonts, icon, title, accent());
    commands.entity(inner).add_child(body);
    s
}

// ── Left ─────────────────────────────────────────────────────────────────────

fn build_tools(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = pane(commands, "anim-tools");
    let rig = rig_tools(commands, fonts);
    let skin = skin_tools(commands, fonts);
    let animate = animate_tools(commands, fonts);
    let logic = crate::params::build(commands, fonts);
    per_mode(commands, root, [rig, skin, animate, logic]);
    root
}

fn rig_tools(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = column(commands);
    let empty = note(commands, fonts, "Put a model on the stage to rig it.");
    bind_display(commands, empty, |w| w.get_resource::<crate::AnimationEditorState>().is_none_or(|s| s.selected_entity.is_none()));
    let template = segmented(commands, &fonts.ui, &["Humanoid", "Face"], 0);
    commands.entity(template).entry::<Node>().and_modify(|mut n| n.width = Val::Percent(100.0));
    bind_2way(
        commands,
        template,
        |w: &Rx| w.get_resource::<StageUi>().map(|u| u.template).unwrap_or(0),
        |w: &mut World, i: &usize| w.resource_mut::<StageUi>().template = *i,
    );
    let body = humanoid_builder(commands, fonts);
    bind_display(commands, body, |w| w.get_resource::<StageUi>().is_some_and(|u| u.template == 0));
    let face = face_setup(commands, fonts);
    bind_display(commands, face, |w| w.get_resource::<StageUi>().is_some_and(|u| u.template == 1));
    let build = titled(commands, fonts, "magic-wand", "Build", body);
    let face_s = titled(commands, fonts, "smiley", "Face Rig", face);
    bind_display(commands, build, |w| w.get_resource::<StageUi>().is_some_and(|u| u.template == 0));
    bind_display(commands, face_s, |w| w.get_resource::<StageUi>().is_some_and(|u| u.template == 1));
    commands.entity(col).add_children(&[empty, template, build, face_s]);
    col
}

/// A spin slider bound to a number in [`SkinEdit`].
fn skin_slider(commands: &mut Commands, fonts: &EmberFonts, label: &str, min: f32, max: f32, get: fn(&SkinEdit) -> f32, set: fn(&mut SkinEdit, f32)) -> Entity {
    let s = spin_slider(commands, &fonts.ui, label, 0.0, min, max);
    commands.entity(s).entry::<Node>().and_modify(|mut n| n.width = Val::Percent(100.0));
    bind_2way(
        commands,
        s,
        move |w: &Rx| w.get_resource::<SkinEdit>().map(get).unwrap_or(0.0),
        move |w: &mut World, v: &f32| set(&mut w.resource_mut::<SkinEdit>(), *v),
    );
    s
}

/// A segmented control bound to a choice in [`SkinEdit`].
fn skin_choice(commands: &mut Commands, fonts: &EmberFonts, labels: &[&str], get: fn(&SkinEdit) -> usize, set: fn(&mut SkinEdit, usize)) -> Entity {
    let s = segmented(commands, &fonts.ui, labels, 0);
    commands.entity(s).entry::<Node>().and_modify(|mut n| n.width = Val::Percent(100.0));
    bind_2way(
        commands,
        s,
        move |w: &Rx| w.get_resource::<SkinEdit>().map(get).unwrap_or(0),
        move |w: &mut World, i: &usize| set(&mut w.resource_mut::<SkinEdit>(), *i),
    );
    s
}

fn rigged(w: &Rx) -> bool {
    w.get_resource::<StageRig>().is_some_and(|r| r.rigged())
}

fn skin_tools(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    use renzora_rig::weights::{Brush, Falloff};
    let col = column(commands);
    let none = note(commands, fonts, "This model has no skeleton to weight. Build one in Rig first.");
    bind_display(commands, none, |w| !rigged(w));
    let body = column(commands);
    bind_display(commands, body, rigged);

    let weights = column(commands);
    let w_hint = note(commands, fonts, "Weights every vertex by how close it is to each bone. Smooth blends across the mesh; Sharp keeps hard edges between bones.");
    let method = skin_choice(commands, fonts, &["Smooth", "Sharp"], |s| s.method, |s, i| s.method = i);
    let auto = renzora_ember::widgets::accent_button(commands, fonts, accent(), "Auto Weights");
    commands.entity(auto).entry::<Node>().and_modify(|mut n| n.width = Val::Percent(100.0));
    on_click(commands, auto, skin::auto_weights);
    commands.entity(weights).add_children(&[w_hint, method, auto]);
    let weights = titled(commands, fonts, "magic-wand", "Weights", weights);

    let brush = column(commands);
    let mode = skin_choice(
        commands,
        fonts,
        &["Add", "Remove", "Smooth", "Set"],
        |s| match s.brush {
            Brush::Add => 0,
            Brush::Remove => 1,
            Brush::Smooth => 2,
            Brush::Set => 3,
        },
        |s, i| s.brush = [Brush::Add, Brush::Remove, Brush::Smooth, Brush::Set][i.min(3)],
    );
    let radius = skin_slider(commands, fonts, "Radius", 0.005, 0.4, |s| s.radius, |s, v| s.radius = v);
    let strength = skin_slider(commands, fonts, "Strength", 0.0, 1.0, |s| s.strength, |s, v| s.strength = v);
    let falloff = skin_choice(
        commands,
        fonts,
        &["Smooth", "Sharp", "Flat"],
        |s| match s.falloff {
            Falloff::Smooth => 0,
            Falloff::Sharp => 1,
            Falloff::Flat => 2,
        },
        |s, i| s.falloff = [Falloff::Smooth, Falloff::Sharp, Falloff::Flat][i.min(2)],
    );
    let mirror_row = row(commands);
    let mirror_label = commands.spawn((Text::new("Mirror strokes"), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())), Node { flex_grow: 1.0, ..default() })).id();
    let mirror = toggle_switch(commands, true);
    bind_2way(
        commands,
        mirror,
        |w: &Rx| w.get_resource::<SkinEdit>().is_some_and(|s| s.mirror),
        |w: &mut World, on: &bool| w.resource_mut::<SkinEdit>().mirror = *on,
    );
    commands.entity(mirror_row).add_children(&[mirror_label, mirror]);
    commands.entity(brush).add_children(&[mode, radius, strength, falloff, mirror_row]);
    let brush = titled(commands, fonts, "paint-brush", "Brush", brush);

    let bones = column(commands);
    let b_hint = note(commands, fonts, "The bone being painted. Ctrl+click a bone on the model to pick it there.");
    let list = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() }).id();
    keyed_list(commands, list, paint_bone_snapshot);
    commands.entity(bones).add_children(&[b_hint, list]);
    let bones = titled(commands, fonts, "bone", "Paint Bone", bones);

    commands.entity(body).add_children(&[weights, brush, bones]);
    commands.entity(col).add_children(&[none, body]);
    col
}

fn paint_bone_snapshot(w: &Rx) -> KeyedSnapshot {
    if !mode_is(w, StageMode::Skin) {
        return KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) };
    }
    let names: Vec<String> = {
        let rig = w.get_resource::<StageRig>();
        let mut v: Vec<String> = rig.map(|r| r.joints.iter().filter_map(|j| r.names.get(j).cloned()).collect()).unwrap_or_default();
        v.dedup();
        v
    };
    let painting = w.get_resource::<SkinEdit>().and_then(|s| s.paint_bone.clone());
    let items = names
        .iter()
        .map(|n| {
            let mut k = hasher();
            n.hash(&mut k);
            let mut h = hasher();
            (painting.as_deref() == Some(n.as_str())).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let name = names[i].clone();
            let on = painting.as_deref() == Some(name.as_str());
            let r = c
                .spawn((
                    Node { width: Val::Percent(100.0), height: Val::Px(22.0), align_items: AlignItems::Center, padding: UiRect::horizontal(Val::Px(6.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
                    BackgroundColor(if on { rgb(accent()).with_alpha(0.28) } else { Color::NONE }),
                    UiInteraction::default(),
                    renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                ))
                .id();
            let t = c.spawn((Text::new(short_name(&name)), ui_font(&f.ui, 12.0), TextColor(rgb(text_primary())))).id();
            c.entity(r).add_child(t);
            on_click(c, r, move |w| {
                let mut s = w.resource_mut::<SkinEdit>();
                s.paint_bone = Some(name.clone());
                s.mark_dirty();
            });
            r
        }),
    }
}

fn animate_tools(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let controls = column(commands);
    let none = note(commands, fonts, "This model has no skeleton. Build one in Rig to animate it.");
    bind_display(commands, none, |w| !rigged(w));
    let ik = ik_list(commands, fonts);
    let ik = titled(commands, fonts, "arrows-in-cardinal", "Limbs", ik);
    bind_display(commands, ik, rigged);
    let library = pose_library(commands, fonts);
    bind_display(commands, library, rigged);
    commands.entity(controls).add_children(&[none, ik, library]);
    let face = expression_sliders(commands, fonts);
    tabs(commands, fonts, &["Controls", "Face"], vec![controls, face], |u| u.animate_left, |u, i| u.animate_left = i)
}

fn pose_library(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = column(commands);
    let hint = note(commands, fonts, "Save the pose on the stage, and put it back on any frame with a click. Applies to the selected bones, or all.");
    let r = row(commands);
    let name = renzora_ember::widgets::text_input(commands, &fonts.ui, "Pose name", "");
    commands.entity(name).entry::<Node>().and_modify(|mut n| n.flex_grow = 1.0);
    renzora_ember::widgets::bind_text_input(
        commands,
        name,
        |w: &Rx| w.get_resource::<PoseLibrary>().map(|l| l.name.clone()).unwrap_or_default(),
        |w: &mut World, v: String| w.resource_mut::<PoseLibrary>().name = v,
    );
    let save = action(commands, fonts, "Save Pose", save_pose);
    commands.entity(r).add_children(&[name, save]);
    let grid = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            display: Display::Grid,
            grid_template_columns: RepeatedGridTrack::flex(2, 1.0),
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    keyed_list(commands, grid, pose_snapshot);
    let empty = note(commands, fonts, "No poses saved yet.");
    bind_display(commands, empty, |w| w.get_resource::<PoseLibrary>().is_none_or(|l| l.poses.is_empty()));
    commands.entity(col).add_children(&[hint, r, grid, empty]);
    titled(commands, fonts, "person-simple", "Pose Library", col)
}

fn pose_snapshot(w: &Rx) -> KeyedSnapshot {
    let names: Vec<String> = w.get_resource::<PoseLibrary>().map(|l| l.poses.iter().map(|(n, _)| n.clone()).collect()).unwrap_or_default();
    let items = names
        .iter()
        .map(|n| {
            let mut k = hasher();
            n.hash(&mut k);
            (k.finish(), 0)
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let name = names[i].clone();
            let cell = c.spawn(Node { column_gap: Val::Px(2.0), align_items: AlignItems::Center, ..default() }).id();
            let apply = button(c, &f.ui, &name);
            c.entity(apply).entry::<Node>().and_modify(|mut n| n.flex_grow = 1.0);
            let n1 = name.clone();
            on_click(c, apply, move |w| apply_pose(w, &n1));
            let del = button(c, &f.ui, "x");
            on_click(c, del, move |w| delete_pose(w, &name));
            c.entity(cell).add_children(&[apply, del]);
            cell
        }),
    }
}

// ── Right ────────────────────────────────────────────────────────────────────

fn build_inspector(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = pane(commands, "anim-inspector");
    let rig = {
        let tree = skeleton_tree(commands, fonts);
        let bone = bone_info(commands, fonts);
        tabs(commands, fonts, &["Skeleton", "Bone"], vec![tree, bone], |u| u.rig_right, |u, i| u.rig_right = i)
    };
    let skin = skin_inspector(commands, fonts);
    let animate = {
        let pose = pose_inspector(commands, fonts);
        let clip = crate::panel::build(commands, fonts);
        let perf = performance(commands, fonts);
        tabs(commands, fonts, &["Pose", "Clip", "Performance"], vec![pose, clip, perf], |u| u.animate_right, |u, i| u.animate_right = i)
    };
    let logic = crate::panel::build(commands, fonts);
    per_mode(commands, root, [rig, skin, animate, logic]);
    root
}

/// The selected bone's turn as three sliders, in degrees about its parent's
/// axes, with the pose tools under them.
fn pose_inspector(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = column(commands);
    let none = note(commands, fonts, "This model has no skeleton to pose.");
    bind_display(commands, none, |w| !rigged(w));
    let body = column(commands);
    bind_display(commands, body, rigged);
    let title = commands.spawn((Text::new(""), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_primary())))).id();
    bind_text(commands, title, |w| {
        let world = w.untracked();
        match selected_bone(world).and_then(|e| world.get::<Name>(e)) {
            Some(n) => short_name(n.as_str()),
            None => "No bone selected. Click one on the stage.".to_string(),
        }
    });
    let mut kids = vec![title];
    for (i, axis) in ["Turn X", "Turn Y", "Turn Z"].into_iter().enumerate() {
        let s = spin_slider(commands, &fonts.ui, axis, 0.0, -180.0, 180.0);
        commands.entity(s).entry::<Node>().and_modify(|mut n| n.width = Val::Percent(100.0));
        bind_2way(
            commands,
            s,
            move |w: &Rx| {
                let world = w.untracked();
                selected_bone(world)
                    .and_then(|e| world.get::<Transform>(e))
                    .map(|t| {
                        let (x, y, z) = t.rotation.to_euler(EulerRot::XYZ);
                        [x, y, z][i].to_degrees()
                    })
                    .unwrap_or(0.0)
            },
            move |w: &mut World, v: &f32| {
                let Some(e) = selected_bone(w) else { return };
                let Some(mut t) = w.get_mut::<Transform>(e) else { return };
                let (x, y, z) = t.rotation.to_euler(EulerRot::XYZ);
                let mut a = [x, y, z];
                a[i] = v.to_radians();
                t.rotation = Quat::from_euler(EulerRot::XYZ, a[0], a[1], a[2]);
            },
        );
        kids.push(s);
    }
    let tools = pose_tools(commands, fonts);
    kids.push(tools);
    commands.entity(body).add_children(&kids);
    commands.entity(col).add_children(&[none, body]);
    col
}

/// The selected bone, when it is one of the subject's.
fn selected_bone(world: &World) -> Option<Entity> {
    let e = world.get_resource::<renzora_editor_framework::EditorSelection>()?.get()?;
    world.get_resource::<StageRig>()?.real_names.contains_key(&e).then_some(e)
}

fn skin_inspector(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = column(commands);
    let none = note(commands, fonts, "Nothing to test until the model has a skeleton.");
    bind_display(commands, none, |w| !rigged(w));
    let body = column(commands);
    bind_display(commands, body, rigged);

    let test = column(commands);
    let t_hint = note(commands, fonts, "Bend the painted bone, or run every joint through its range, to see the weights deform. Only the stage moves; nothing is keyed.");
    let mut kids = vec![t_hint];
    for (i, axis) in ["Bend X", "Bend Y", "Bend Z"].into_iter().enumerate() {
        let s = spin_slider(commands, &fonts.ui, axis, 0.0, -150.0, 150.0);
        commands.entity(s).entry::<Node>().and_modify(|mut n| n.width = Val::Percent(100.0));
        bind_2way(
            commands,
            s,
            move |w: &Rx| {
                w.get_resource::<SkinEdit>()
                    .and_then(|s| s.paint_bone.as_ref().and_then(|b| s.test.get(b)))
                    .map(|v| v[i])
                    .unwrap_or(0.0)
            },
            move |w: &mut World, v: &f32| {
                let mut s = w.resource_mut::<SkinEdit>();
                let Some(bone) = s.paint_bone.clone() else { return };
                s.test.entry(bone).or_default()[i] = *v;
            },
        );
        kids.push(s);
    }
    let r = row(commands);
    let rom = button(commands, &fonts.ui, "Range of Motion");
    bind_bg(commands, rom, |w| if w.get_resource::<SkinEdit>().is_some_and(|s| s.range_of_motion) { rgb(accent()).with_alpha(0.35) } else { rgb(card_bg()) });
    on_click(commands, rom, |w| {
        let mut s = w.resource_mut::<SkinEdit>();
        s.range_of_motion = !s.range_of_motion;
    });
    let reset = action(commands, fonts, "Reset Bends", |w| {
        let mut s = w.resource_mut::<SkinEdit>();
        s.test.clear();
        s.range_of_motion = false;
    });
    commands.entity(r).add_children(&[rom, reset]);
    kids.push(r);
    commands.entity(test).add_children(&kids);
    let test = titled(commands, fonts, "person-simple-walk", "Test", test);

    let checks = column(commands);
    let report = commands.spawn((Text::new(""), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())))).id();
    bind_text(commands, report, |w| {
        let Some(s) = w.get_resource::<SkinEdit>() else { return String::new() };
        let h = s.health;
        let a = if h.unweighted == 0 { "Every vertex is weighted.".to_string() } else { format!("{} vertices have no weight.", h.unweighted) };
        let b = if h.not_normalised == 0 { "Weights add up to 1.".to_string() } else { format!("{} vertices' weights do not add up to 1.", h.not_normalised) };
        format!("{a}\n{b}\nFour bones at most per vertex, which is what the GPU skins with.")
    });
    let normalise = action(commands, fonts, "Normalise", skin::normalise);
    commands.entity(checks).add_children(&[report, normalise]);
    let checks = titled(commands, fonts, "check-circle", "Checks", checks);

    let save = column(commands);
    let state = note(commands, fonts, "");
    bind_text(commands, state, |w| {
        if w.get_resource::<SkinEdit>().is_some_and(|s| s.unsaved) {
            "Painted weights are not saved yet. They are saved into the model file when you Save, or when you leave Skin.".to_string()
        } else {
            "The weights match the model file.".to_string()
        }
    });
    let r2 = row(commands);
    let save_b = renzora_ember::widgets::accent_button(commands, fonts, accent(), "Save Weights");
    on_click(commands, save_b, skin::save);
    let revert = action(commands, fonts, "Revert", skin::revert);
    commands.entity(r2).add_children(&[save_b, revert]);
    commands.entity(save).add_children(&[state, r2]);
    let save = titled(commands, fonts, "floppy-disk", "Save", save);

    commands.entity(body).add_children(&[test, checks, save]);
    commands.entity(col).add_children(&[none, body]);
    col
}
