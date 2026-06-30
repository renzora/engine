//! Bevy-native Camera Debug panel — camera count, a list of scene cameras with
//! per-camera activate (ON/OFF) + select, the selected camera's properties, and
//! the debug-visualization toggles. Interactive bits write `CameraDebugState`
//! (and flip `Camera::is_active`) directly — no DebugBridge.
//!
//! The egui panel's reflection-based component dump + Copy button are omitted
//! (that's effectively a mini-inspector); the CameraInfo property grid is kept.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_2way, bind_bg, bind_display, bind_text, bind_text_color, keyed_list, KeyedSnapshot};
use renzora_ember::theme::{rgb, selection, text_muted, text_primary};
use renzora_ember::widgets::checkbox;
use renzora::SplashState;

use crate::state::{CameraDebugState, CameraInfo, CameraProjectionType};

const SECONDARY: (u8, u8, u8) = (170, 170, 180);
const FAINT_BG: (u8, u8, u8) = (30, 30, 36);

#[derive(Component)]
struct CamSelect(Entity);
#[derive(Component)]
struct CamToggle(Entity);

pub(super) fn register_camera(app: &mut App) {
    app.register_panel_content("camera_debug", true, build);
    app.add_systems(
        Update,
        (camera_toggle_click, camera_select_click).run_if(in_state(SplashState::Editor)),
    );
}

fn cam<R: Default>(w: &World, f: impl FnOnce(&CameraDebugState) -> R) -> R {
    w.get_resource::<CameraDebugState>().map(f).unwrap_or_default()
}

/// Read a field off the selected camera (default if nothing's selected).
fn sel<R: Default>(w: &World, f: impl Fn(&CameraInfo) -> R) -> R {
    w.get_resource::<CameraDebugState>()
        .and_then(|s| s.selected_camera_info().map(f))
        .unwrap_or_default()
}

fn is_active(w: &World, e: Entity) -> bool {
    cam(w, |s| s.cameras.iter().find(|c| c.entity == e).map(|c| c.is_active).unwrap_or(false))
}

fn proj_letter(p: CameraProjectionType) -> &'static str {
    match p {
        CameraProjectionType::Perspective => "P",
        CameraProjectionType::Orthographic => "O",
    }
}

fn vec3(v: Vec3) -> String {
    format!("({:.2}, {:.2}, {:.2})", v.x, v.y, v.z)
}

pub(super) fn faint_box(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(8.0)),
                row_gap: Val::Px(3.0),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(FAINT_BG)),
        ))
        .id()
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = super::root(commands);

    // Count header.
    let head = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FlexEnd,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let big = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 28.0), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, big, |w| cam(w, |s| s.scene_camera_count()).to_string());
    let u = commands
        .spawn((Text::new("cameras"), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted())),
            Node { margin: UiRect::bottom(Val::Px(4.0)), ..default() }))
        .id();
    commands.entity(head).add_children(&[big, u]);

    // Camera list.
    let list_label = super::section(commands, fonts, &renzora::lang::t("cam_debug.cameras"));
    let list = faint_box(commands);
    keyed_list(commands, list, camera_snapshot);

    // Selected camera properties.
    let sel_label = super::section(commands, fonts, &renzora::lang::t("cam_debug.selected_camera"));
    bind_display(commands, sel_label, |w| cam(w, |s| s.selected_camera.is_some()));
    let sel_box = faint_box(commands);
    bind_display(commands, sel_box, |w| cam(w, |s| s.selected_camera.is_some()));
    let sel_name = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 14.0), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, sel_name, |w| sel(w, |c| c.name.clone()));
    let mut sel_kids = vec![sel_name];
    sel_kids.push(grid_row(commands, fonts, &renzora::lang::t("cam_debug.projection"), |w| match sel(w, |c| c.projection_type) {
        CameraProjectionType::Perspective => renzora::lang::t("comp.camera.perspective"),
        CameraProjectionType::Orthographic => renzora::lang::t("comp.camera.orthographic"),
    }));
    sel_kids.push(grid_row(commands, fonts, &renzora::lang::t("cam_debug.fov"), |w| {
        sel(w, |c| c.fov_degrees).map(|f| format!("{:.1}\u{b0}", f)).unwrap_or_else(|| "\u{2014}".into())
    }));
    sel_kids.push(grid_row(commands, fonts, &format!("{} / {}", renzora::lang::t("cam_debug.near"), renzora::lang::t("cam_debug.far")), |w| format!("{:.2} / {:.0}", sel(w, |c| c.near), sel(w, |c| c.far))));
    sel_kids.push(grid_row(commands, fonts, &renzora::lang::t("cam_debug.aspect_short"), |w| format!("{:.3}", sel(w, |c| c.aspect_ratio))));
    sel_kids.push(grid_row(commands, fonts, &renzora::lang::t("cam_debug.position"), |w| vec3(sel(w, |c| c.position))));
    sel_kids.push(grid_row(commands, fonts, &renzora::lang::t("cam_debug.rotation"), |w| vec3(sel(w, |c| c.rotation_degrees))));
    sel_kids.push(grid_row(commands, fonts, &renzora::lang::t("cam_debug.forward"), |w| vec3(sel(w, |c| c.forward))));
    commands.entity(sel_box).add_children(&sel_kids);

    // Debug visualization toggles.
    let viz_label = super::section(commands, fonts, &renzora::lang::t("cam_debug.debug_visualization"));
    let viz = faint_box(commands);
    let t1 = checkbox_row(commands, fonts, &renzora::lang::t("cam_debug.show_frustum_selected"), |w| cam(w, |s| s.show_frustum_gizmos), |w, v| set(w, move |s| s.show_frustum_gizmos = v));
    let t2 = checkbox_row(commands, fonts, &renzora::lang::t("cam_debug.show_camera_axes"), |w| cam(w, |s| s.show_camera_axes), |w, v| set(w, move |s| s.show_camera_axes = v));
    let t3 = checkbox_row(commands, fonts, &renzora::lang::t("cam_debug.show_all_frustums"), |w| cam(w, |s| s.show_all_frustums), |w, v| set(w, move |s| s.show_all_frustums = v));
    commands.entity(viz).add_children(&[t1, t2, t3]);

    commands.entity(root).add_children(&[head, list_label, list, sel_label, sel_box, viz_label, viz]);
    root
}

fn set(w: &mut World, f: impl FnOnce(&mut CameraDebugState)) {
    if let Some(mut s) = w.get_resource_mut::<CameraDebugState>() {
        f(&mut s);
    }
}

fn grid_row<V>(commands: &mut Commands, fonts: &EmberFonts, label: &str, value: V) -> Entity
where
    V: Fn(&World) -> String + Send + Sync + 'static,
{
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let l = commands
        .spawn((Text::new(label), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())),
            Node { width: Val::Px(90.0), ..default() }))
        .id();
    let v = commands
        .spawn((Text::new(""), ui_font(&fonts.mono, 11.0), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, v, value);
    commands.entity(row).add_children(&[l, v]);
    row
}

pub(super) fn checkbox_row<G, S>(commands: &mut Commands, fonts: &EmberFonts, label: &str, get: G, set_fn: S) -> Entity
where
    G: Fn(&World) -> bool + Send + Sync + 'static,
    S: Fn(&mut World, bool) + Send + Sync + 'static,
{
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let cb = checkbox(commands, false);
    bind_2way(commands, cb, get, move |w, v| set_fn(w, *v));
    let l = commands
        .spawn((Text::new(label), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary()))))
        .id();
    commands.entity(row).add_children(&[cb, l]);
    row
}

// ── Camera list ──────────────────────────────────────────────────────────────

fn camera_snapshot(world: &World) -> KeyedSnapshot {
    let cams = cam(world, |s| s.cameras.clone());
    if cams.is_empty() {
        return KeyedSnapshot {
            items: vec![(u64::MAX, 0)],
            build: Box::new(|c, f, _| {
                c.spawn((
                    Text::new(renzora::lang::t("cam_debug.no_cameras")),
                    ui_font(&f.ui, 11.0),
                    TextColor(rgb(text_muted())),
                ))
                .id()
            }),
        };
    }
    // Stable structure keyed by entity; is_active/selection are bindings, so only
    // name/order/projection changes rebuild a row.
    let items: Vec<(u64, u64)> = cams
        .iter()
        .map(|c| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (&c.name, c.order, proj_letter(c.projection_type)).hash(&mut h);
            (c.entity.to_bits(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| camera_row(c, f, &cams[i])),
    }
}

fn camera_row(commands: &mut Commands, fonts: &EmberFonts, info: &CameraInfo) -> Entity {
    let e = info.entity;
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Name::new("camera-row"),
        ))
        .id();
    bind_bg(commands, row, move |w| {
        if cam(w, |s| s.selected_camera) == Some(e) {
            rgb(selection())
        } else {
            Color::NONE
        }
    });

    // Left: dot + name (the select target).
    let select_zone = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                flex_grow: 1.0,
                column_gap: Val::Px(6.0),
                ..default()
            },
            Interaction::default(),
            CamSelect(e),
            Name::new("camera-select"),
        ))
        .id();
    let dot = commands
        .spawn((Text::new("\u{25cf}"), ui_font(&fonts.ui, 8.0), TextColor(rgb((120, 120, 130)))))
        .id();
    bind_text_color(commands, dot, move |w| {
        if is_active(w, e) {
            rgb((100, 200, 100))
        } else {
            rgb((120, 120, 130))
        }
    });
    let name = commands
        .spawn((Text::new(info.name.clone()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary()))))
        .id();
    commands.entity(select_zone).add_children(&[dot, name]);

    // Right: ON/OFF toggle + projection + order.
    let controls = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let toggle = commands
        .spawn((
            Node {
                width: Val::Px(30.0),
                height: Val::Px(15.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(rgb((70, 70, 78))),
            Interaction::default(),
            CamToggle(e),
            Name::new("camera-toggle"),
        ))
        .id();
    bind_bg(commands, toggle, move |w| {
        if is_active(w, e) {
            rgb((120, 210, 120))
        } else {
            rgb((70, 70, 78))
        }
    });
    let toggle_label = commands
        .spawn((Text::new(""), ui_font(&fonts.mono, 9.0), TextColor(rgb((220, 220, 220)))))
        .id();
    bind_text(commands, toggle_label, move |w| if is_active(w, e) { renzora::lang::t("cam_debug.on") } else { renzora::lang::t("cam_debug.off") });
    bind_text_color(commands, toggle_label, move |w| {
        if is_active(w, e) {
            rgb((20, 40, 20))
        } else {
            rgb((220, 220, 220))
        }
    });
    commands.entity(toggle).add_child(toggle_label);
    let proj = commands
        .spawn((Text::new(proj_letter(info.projection_type)), ui_font(&fonts.mono, 9.0), TextColor(rgb(SECONDARY))))
        .id();
    let order = commands
        .spawn((Text::new(format!("#{}", info.order)), ui_font(&fonts.ui, 9.0), TextColor(rgb(text_muted()))))
        .id();
    commands.entity(controls).add_children(&[toggle, proj, order]);

    commands.entity(row).add_children(&[select_zone, controls]);
    row
}

// ── Click systems ────────────────────────────────────────────────────────────

fn camera_toggle_click(
    q: Query<(&Interaction, &CamToggle), Changed<Interaction>>,
    mut cameras: Query<&mut bevy::camera::Camera>,
) {
    for (interaction, toggle) in &q {
        if *interaction == Interaction::Pressed {
            if let Ok(mut c) = cameras.get_mut(toggle.0) {
                c.is_active = !c.is_active;
            }
        }
    }
}

fn camera_select_click(
    q: Query<(&Interaction, &CamSelect), Changed<Interaction>>,
    mut state: ResMut<CameraDebugState>,
) {
    for (interaction, sel) in &q {
        if *interaction == Interaction::Pressed {
            state.selected_camera = Some(sel.0);
        }
    }
}
