//! Rig mode's Quadruped template: thirteen points clicked on the side of a
//! four-legged animal, and building its skeleton (`renzora_rig::quadruped`).

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_display, keyed_list};
use renzora_ember::theme::*;
use renzora_ember::widgets::button;
use renzora_rig::quadruped::{self, QuadrupedMarkers};

use super::markers::{MarkerSet, Markers};
use super::rig_panel::{marker_snapshot, markers_in_model, unrig_row};
use super::{joints, model_file, on_click, subject, tell};

fn note(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((Text::new(text), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), Node { margin: UiRect::vertical(Val::Px(2.0)), ..default() }))
        .id()
}

fn rigged(world: &World) -> bool {
    subject(world).is_some_and(|r| !joints(world, r).is_empty())
}

pub(crate) fn quadruped_builder(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() }).id();
    let has = note(commands, fonts, "This model has a skeleton. Its bones are listed on the right; Skin and Animate work on it.");
    bind_display(commands, has, |w| rigged(w.untracked()));
    let body = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() }).id();
    bind_display(commands, body, |w| subject(w.untracked()).is_some() && !rigged(w.untracked()));
    let hint = note(
        commands,
        fonts,
        "Stand the animal on all fours and look at it from the side (the Side button). Place the points on the near legs; the far legs are mirrored. For an animal with no tail, put the tail tip at the end of the spine. The original file is kept.",
    );
    let points = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() }).id();
    keyed_list(commands, points, |w| marker_snapshot(w, MarkerSet::Quadruped));
    let r = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(4.0), row_gap: Val::Px(4.0), ..default() })
        .id();
    let place = button(commands, &fonts.ui, "Place Points");
    on_click(commands, place, |w| {
        if let Some(mut m) = w.get_resource_mut::<Markers>() {
            m.start(MarkerSet::Quadruped, 0);
            let next = m.points(MarkerSet::Quadruped).iter().position(Option::is_none).unwrap_or(0);
            m.start(MarkerSet::Quadruped, next);
        }
    });
    let clear = button(commands, &fonts.ui, "Clear Points");
    on_click(commands, clear, |w| {
        if let Some(mut m) = w.get_resource_mut::<Markers>() {
            m.clear(MarkerSet::Quadruped);
        }
    });
    commands.entity(r).add_children(&[place, clear]);
    let build = renzora_ember::widgets::accent_button(commands, fonts, accent(), "Generate Skeleton");
    commands.entity(build).entry::<Node>().and_modify(|mut n| n.width = Val::Percent(100.0));
    on_click(commands, build, build_quadruped);
    commands.entity(body).add_children(&[hint, points, r, build]);
    let unrig = unrig_row(commands, fonts);
    commands.entity(col).add_children(&[has, body, unrig]);
    col
}

fn build_quadruped(world: &mut World) {
    let Some(root) = subject(world) else { return };
    if rigged(world) {
        tell(world, false, "This model already has a skeleton");
        return;
    }
    let Some(p) = markers_in_model(world, MarkerSet::Quadruped) else {
        tell(world, false, "Place all thirteen points first");
        return;
    };
    let Some((abs, rel)) = model_file(world, root) else {
        tell(world, false, "This entity has no model file to rig");
        return;
    };
    let markers = QuadrupedMarkers {
        nose: p[0],
        skull: p[1],
        withers: p[2],
        front_shoulder: p[3],
        front_elbow: p[4],
        front_wrist: p[5],
        front_toes: p[6],
        pelvis: p[7],
        hip: p[8],
        knee: p[9],
        hock: p[10],
        hind_toes: p[11],
        tail_tip: p[12],
    };
    match renzora_rig::rig_file(&abs, |glb| quadruped::build(glb, &markers)) {
        Ok(applied) => {
            tell(world, true, format!("Rigged {rel} as a quadruped: {} bones over {} meshes. The original is kept beside it.", applied.bones, applied.meshes));
            if let Some(mut m) = world.get_resource_mut::<Markers>() {
                m.clear(MarkerSet::Quadruped);
            }
            super::respawn::request(world, &rel);
        }
        Err(e) => tell(world, false, format!("Could not rig {rel}: {e}")),
    }
}
