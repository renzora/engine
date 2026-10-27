//! The Camera and Snap submenus, and [`HeaderClick`]: the one-shot actions
//! every click row in this toolbar fires.

use bevy::prelude::*;

use renzora::core::viewport_types::{
    CameraSettingsState, ProjectionMode, ViewAngleCommand, ViewportSettings,
};
use renzora_editor_framework::EditorCommands;
use renzora_ember::font::EmberFonts;
use renzora_ember::reactive::tracked::bind_2way;
use renzora_ember::reactive::Rx;
use renzora_ember::theme::value_text;
use renzora_ember::widgets::{drag_value, DragRange, Popup};
use renzora_ember::UiInteraction;

use super::rows::{
    click_row, drag_row, proj_row, section_label, separator_row, snap_button, toggle_row,
};
use super::snap::{set_snap, snap_val};
use super::view::{ViewAngleTrigger, VIEW_ANGLE_OPTIONS};
use super::loc_opt;

/// A discrete one-shot click action inside a toolbar dropdown.
#[derive(Component, Clone, Copy)]
pub(super) enum HeaderClick {
    Projection(ProjectionMode),
    ViewAngle { yaw: f32, pitch: f32 },
    /// A per-viewport view-angle pick: snaps THIS slot's camera (the shared
    /// `ViewAngle` above writes the global channel) and relabels its trigger.
    /// `index` is into [`VIEW_ANGLE_OPTIONS`], for the label.
    SlotViewAngle {
        slot: usize,
        index: usize,
        yaw: f32,
        pitch: f32,
    },
    CamReset,
    ToggleObjectSnap,
    ToggleFloorSnap,
}

/// Tags a projection row so it highlights when that projection is current.
#[derive(Component, Clone, Copy)]
pub(super) struct ProjOption(pub(super) ProjectionMode);

/// Object/Floor snap toggle buttons (accent fill when enabled).
#[derive(Component, Clone, Copy)]
pub(super) enum SnapBtnKind {
    Object,
    Floor,
}

/// View-angle presets: (label, shortcut, yaw, pitch). Mirrors egui `ViewAngle`.
const VIEW_ANGLES: &[(&str, &str, f32, f32)] = {
    use std::f32::consts::{FRAC_PI_2, PI};
    &[
        ("Front", "Num1", 0.0, 0.0),
        ("Back", "Ctrl+Num1", PI, 0.0),
        ("Left", "Ctrl+Num3", -FRAC_PI_2, 0.0),
        ("Right", "Num3", FRAC_PI_2, 0.0),
        ("Top", "Num7", 0.0, FRAC_PI_2),
        ("Bottom", "Ctrl+Num7", 0.0, -FRAC_PI_2),
    ]
};

/// The rows of the **Camera** submenu of the viewport options menu (see
/// [`super::options`]).
#[allow(clippy::vec_init_then_push)]
pub(super) fn camera_rows(commands: &mut Commands, fonts: &EmberFonts) -> Vec<Entity> {
    let mut kids: Vec<Entity> = Vec::new();
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.camera.projection")));
    kids.push(proj_row(commands, fonts, ProjectionMode::Perspective, &renzora::lang::t("viewport.camera.perspective")));
    kids.push(proj_row(commands, fonts, ProjectionMode::Orthographic, &renzora::lang::t("viewport.camera.orthographic")));

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.camera.view_angles")));
    for (label, sc, yaw, pitch) in VIEW_ANGLES {
        let lbl = match *label {
            "Front" => renzora::lang::t("viewport.camera.front"),
            "Back" => renzora::lang::t("viewport.camera.back"),
            "Left" => renzora::lang::t("viewport.camera.left"),
            "Right" => renzora::lang::t("viewport.camera.right"),
            "Top" => renzora::lang::t("viewport.camera.top"),
            "Bottom" => renzora::lang::t("viewport.camera.bottom"),
            other => other.to_string(),
        };
        kids.push(click_row(
            commands,
            fonts,
            &format!("{lbl}  ({sc})"),
            HeaderClick::ViewAngle {
                yaw: *yaw,
                pitch: *pitch,
            },
        ));
    }

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.camera.sensitivities")));
    kids.push(drag_row!(commands, fonts, &renzora::lang::t("viewport.camera.look"), 0.05, 2.0, 0.05, camera.look_sensitivity));
    kids.push(drag_row!(commands, fonts, &renzora::lang::t("viewport.camera.orbit"), 0.05, 2.0, 0.05, camera.orbit_sensitivity));
    kids.push(drag_row!(commands, fonts, &renzora::lang::t("viewport.camera.pan"), 0.1, 5.0, 0.1, camera.pan_sensitivity));
    kids.push(drag_row!(commands, fonts, &renzora::lang::t("viewport.camera.zoom"), 0.1, 5.0, 0.1, camera.zoom_sensitivity));

    kids.push(separator_row(commands));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.camera.invert_y"), camera.invert_y));
    kids.push(toggle_row!(
        commands,
        fonts,
        &renzora::lang::t("viewport.camera.distance_relative_speed"),
        camera.distance_relative_speed
    ));
    kids.push(click_row(commands, fonts, &renzora::lang::t("inspector.component.reset"), HeaderClick::CamReset));
    kids
}

/// The rows of the **Snap** submenu of the viewport options menu.
#[allow(clippy::vec_init_then_push)]
pub(super) fn snap_rows(commands: &mut Commands, fonts: &EmberFonts) -> Vec<Entity> {
    let mut kids: Vec<Entity> = Vec::new();
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.snap.object_snapping")));
    kids.push(snap_dist_row(
        commands,
        fonts,
        &renzora::lang::t("viewport.snap.objects"),
        SnapBtnKind::Object,
        HeaderClick::ToggleObjectSnap,
        0.1,
        10.0,
        0.1,
        |w| snap_val(w, |s| s.object_snap_distance),
        |w, v| set_snap(w, |s| &mut s.object_snap_distance, v),
    ));
    kids.push(snap_dist_row(
        commands,
        fonts,
        &renzora::lang::t("viewport.snap.floor"),
        SnapBtnKind::Floor,
        HeaderClick::ToggleFloorSnap,
        -1000.0,
        1000.0,
        0.1,
        |w| snap_val(w, |s| s.floor_y),
        |w, v| set_snap(w, |s| &mut s.floor_y, v),
    ));

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.snap.transform_aids")));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.snap.edge_snap"), snap.translate_edge_snap));
    kids.push(toggle_row!(
        commands,
        fonts,
        &renzora::lang::t("viewport.snap.scale_from_bottom"),
        snap.scale_bottom_anchor
    ));
    kids
}

/// A snap toggle button + its bound distance/offset drag value, in one row.
#[allow(clippy::too_many_arguments)]
fn snap_dist_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    kind: SnapBtnKind,
    click: HeaderClick,
    min: f32,
    max: f32,
    step: f32,
    get: impl Fn(&Rx) -> f32 + Send + Sync + 'static,
    set: impl Fn(&mut World, f32) + Send + Sync + 'static,
) -> Entity {
    let btn = snap_button(commands, fonts, label, kind, click);
    let dv = drag_value(commands, &fonts.ui, "", value_text(), min, step);
    commands.entity(dv).insert(DragRange { min, max });
    bind_2way(commands, dv, get, move |w, v: &f32| set(w, *v));
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            Name::new("vp-snap-dist-row"),
        ))
        .id();
    commands.entity(row).add_children(&[btn, dv]);
    row
}

pub(super) fn header_click(
    q: Query<(&UiInteraction, &HeaderClick), Changed<UiInteraction>>,
    mut angle_triggers: Query<(&ViewAngleTrigger, &mut Popup)>,
    mut texts: Query<&mut Text>,
    mut nodes: Query<&mut Node>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, click) in &q {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        match *click {
            HeaderClick::SlotViewAngle {
                slot,
                index,
                yaw,
                pitch,
            } => {
                // Per-slot channel, consumed by `renzora_camera::apply_per_slot_view_angle`.
                cmds.push(move |w: &mut World| {
                    if let Some(mut vps) =
                        w.get_resource_mut::<renzora::core::viewport_types::Viewports>()
                    {
                        if let Some(s) = vps.slots.get_mut(slot) {
                            s.pending_view_angle = Some(ViewAngleCommand { yaw, pitch });
                        }
                    }
                });
                // Reflect the pick on this viewport's trigger label, and close
                // the menu (a one-shot action, unlike the switch panels).
                let name = VIEW_ANGLE_OPTIONS.get(index).map(|(l, ..)| loc_opt(l));
                for (tag, mut popup) in &mut angle_triggers {
                    if tag.slot != slot {
                        continue;
                    }
                    if let (Some(name), Ok(mut text)) = (name.as_ref(), texts.get_mut(tag.label)) {
                        if text.0 != *name {
                            text.0 = name.clone();
                        }
                    }
                    popup.open = false;
                    if let Ok(mut n) = nodes.get_mut(popup.panel) {
                        n.display = Display::None;
                    }
                }
            }
            HeaderClick::Projection(mode) => cmds.push(move |w: &mut World| {
                if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                    s.projection_mode = mode;
                }
            }),
            HeaderClick::ViewAngle { yaw, pitch } => cmds.push(move |w: &mut World| {
                if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                    s.pending_view_angle = Some(ViewAngleCommand { yaw, pitch });
                }
            }),
            HeaderClick::CamReset => cmds.push(|w: &mut World| {
                if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                    s.camera = CameraSettingsState::default();
                }
            }),
            HeaderClick::ToggleObjectSnap => cmds.push(|w: &mut World| {
                if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                    s.snap.object_snap_enabled = !s.snap.object_snap_enabled;
                }
            }),
            HeaderClick::ToggleFloorSnap => cmds.push(|w: &mut World| {
                if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                    s.snap.floor_snap_enabled = !s.snap.floor_snap_enabled;
                }
            }),
        }
    }
}
