//! Bevy-native (ember) NavMesh panel: global toggles ("Show Agent Paths",
//! "Auto Rebuild") and action buttons ("Rebuild All", "Reset Agents", "Bake to
//! Disk") over a status line and a per-volume list. Each volume card shows its
//! name + entity, a colored build-status label with polygon count, a "Debug
//! Draw" checkbox, and a "Rebuild" button.
//!
//! The native content drives the `NavMeshPanelState` action queue + mirror, so
//! `drain_panel_actions` / `apply_auto_rebuild_setting` / `flush_bake_request`
//! do all the work — this is only the UI surface.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora::SplashState;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{
    bind_2way, bind_display, bind_text, bind_text_color, keyed_list, KeyedSnapshot,
};
use renzora_ember::theme::*;
use renzora_ember::widgets::{button, checkbox};

use vleue_navigator::prelude::NavMeshStatus;

use crate::editor_panel::{NavMeshPanelMirror, NavMeshPanelState};

/// Native plugin that registers the ember navmesh panel body.
pub struct NativeNavmesh;

impl Plugin for NativeNavmesh {
    fn build(&self, app: &mut App) {
        app.register_panel_content("navmesh", true, build);
        app.add_systems(
            Update,
            (button_clicks, volume_actions).run_if(in_state(SplashState::Editor)),
        );
    }
}

// ── Markers ──────────────────────────────────────────────────────────────────

#[derive(Component)]
struct RebuildAllBtn;
#[derive(Component)]
struct ResetAgentsBtn;
#[derive(Component)]
struct BakeBtn;

/// A volume-card "Rebuild" button, tagged with the volume entity.
#[derive(Component)]
struct VolumeRebuildBtn(Entity);

// ── Mirror helpers ─────────────────────────────────────────────────────────────

fn mirror_volume_count(w: &World) -> usize {
    w.get_resource::<NavMeshPanelMirror>().map(|m| m.volumes.len()).unwrap_or(0)
}

fn mirror_agent_count(w: &World) -> usize {
    w.get_resource::<NavMeshPanelMirror>().map(|m| m.agent_count).unwrap_or(0)
}

/// Read one volume's live `debug_draw` flag from the mirror.
fn volume_debug(w: &World, entity: Entity) -> bool {
    w.get_resource::<NavMeshPanelMirror>()
        .and_then(|m| m.volumes.iter().find(|r| r.entity == entity))
        .map(|r| r.debug_draw)
        .unwrap_or(false)
}

/// Read one volume's live status from the mirror.
fn volume_status(w: &World, entity: Entity) -> NavMeshStatus {
    w.get_resource::<NavMeshPanelMirror>()
        .and_then(|m| m.volumes.iter().find(|r| r.entity == entity))
        .map(|r| r.status)
        .unwrap_or(NavMeshStatus::Invalid)
}

/// Read one volume's live polygon count from the mirror.
fn volume_polys(w: &World, entity: Entity) -> Option<usize> {
    w.get_resource::<NavMeshPanelMirror>()
        .and_then(|m| m.volumes.iter().find(|r| r.entity == entity))
        .and_then(|r| r.polygon_count)
}

/// (label, color) for a build status.
fn status_label(status: NavMeshStatus) -> (&'static str, (u8, u8, u8)) {
    match status {
        NavMeshStatus::Built => ("Built", (120, 220, 120)),
        NavMeshStatus::Building => ("Building...", (240, 200, 90)),
        NavMeshStatus::Failed => ("Failed", (230, 90, 90)),
        NavMeshStatus::Cancelled => ("Cancelled", (180, 180, 180)),
        NavMeshStatus::Invalid => ("Invalid", (180, 180, 180)),
    }
}

// ── Panel ────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(6.0)),
                ..default()
            },
            Name::new("native-navmesh"),
        ))
        .id();

    // Row 1: "Show Agent Paths" toggle.
    let show_row = labeled_checkbox(
        commands,
        fonts,
        "Show Agent Paths",
        |w| {
            w.get_resource::<NavMeshPanelState>()
                .map(|s| s.show_agent_paths())
                .unwrap_or(true)
        },
        |w, v: &bool| {
            if let Some(s) = w.get_resource::<NavMeshPanelState>() {
                s.queue_show_agent_paths(*v);
            }
        },
    );

    // Row 2: "Auto Rebuild" toggle + "Rebuild All" + "Reset Agents".
    let actions_row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            flex_wrap: FlexWrap::Wrap,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    let auto_toggle = labeled_checkbox(
        commands,
        fonts,
        "Auto Rebuild",
        |w| {
            w.get_resource::<NavMeshPanelState>()
                .map(|s| s.auto_rebuild())
                .unwrap_or(true)
        },
        |w, v: &bool| {
            if let Some(s) = w.get_resource::<NavMeshPanelState>() {
                s.queue_auto_rebuild(*v);
            }
        },
    );
    let rebuild_all = button(commands, &fonts.ui, "Rebuild All");
    commands.entity(rebuild_all).insert(RebuildAllBtn);
    let reset_agents = button(commands, &fonts.ui, "Reset Agents");
    commands.entity(reset_agents).insert(ResetAgentsBtn);
    commands
        .entity(actions_row)
        .add_children(&[auto_toggle, rebuild_all, reset_agents]);

    // Row 3: "Bake to Disk".
    let bake_row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            ..default()
        })
        .id();
    let bake = button(commands, &fonts.ui, "Bake to Disk");
    commands.entity(bake).insert(BakeBtn);
    commands.entity(bake_row).add_child(bake);

    // Separator.
    let sep = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(1.0), ..default() },
            BackgroundColor(rgb(border())),
        ))
        .id();

    // Status line: "Volumes: N   Agents: M".
    let status = commands
        .spawn((
            Text::new("Volumes: 0   Agents: 0"),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_text(commands, status, |w| {
        format!("Volumes: {}   Agents: {}", mirror_volume_count(w), mirror_agent_count(w))
    });

    // Empty-state note (shown when there are no volumes).
    let empty = commands
        .spawn((
            Text::new(
                "No NavMesh Volumes in scene. Add the component on any entity to create one.",
            ),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::new_with_justify(bevy::text::Justify::Left),
            Node { width: Val::Percent(100.0), ..default() },
        ))
        .id();
    bind_display(commands, empty, |w| mirror_volume_count(w) == 0);

    // Volume list (keyed <For>).
    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    bind_display(commands, list, |w| mirror_volume_count(w) > 0);
    keyed_list(commands, list, volumes_snapshot);

    commands
        .entity(root)
        .add_children(&[show_row, actions_row, bake_row, sep, status, empty, list]);
    root
}

/// A horizontal `checkbox + label` row, two-way bound to world state.
fn labeled_checkbox<G, S>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    get: G,
    set: S,
) -> Entity
where
    G: Fn(&World) -> bool + Send + Sync + 'static,
    S: Fn(&mut World, &bool) + Send + Sync + 'static,
{
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let cb = checkbox(commands, get_initial(&get));
    bind_2way(commands, cb, get, set);
    let lbl = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::new_with_no_wrap(),
        ))
        .id();
    commands.entity(row).add_children(&[cb, lbl]);
    row
}

/// `bind_2way` seeds the model itself, so the checkbox initial value is just a
/// placeholder; default to `true` (matching the panel defaults) without world
/// access at build time.
fn get_initial<G: Fn(&World) -> bool>(_get: &G) -> bool {
    true
}

// ── Volume list ────────────────────────────────────────────────────────────────

fn volumes_snapshot(world: &World) -> KeyedSnapshot {
    let Some(mirror) = world.get_resource::<NavMeshPanelMirror>() else {
        return KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) };
    };
    // Snapshot the row identity + name (structure). Status / polygons /
    // debug_draw are bound live so they update in place without a rebuild.
    let rows: Vec<(Entity, String)> =
        mirror.volumes.iter().map(|r| (r.entity, r.name.clone())).collect();
    let items: Vec<(u64, u64)> = rows
        .iter()
        .map(|(e, name)| {
            let mut k = hasher();
            e.hash(&mut k);
            let mut h = hasher();
            name.hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (entity, name) = &rows[i];
            volume_card(c, f, *entity, name)
        }),
    }
}

/// One volume card: name + entity header, a status row, and a controls row.
fn volume_card(commands: &mut Commands, fonts: &EmberFonts, entity: Entity, name: &str) -> Entity {
    let card = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id();

    // Header: bold name + faint (entity).
    let header = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let name_e = commands
        .spawn((
            Text::new(name.to_string()),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let ent_e = commands
        .spawn((
            Text::new(format!("({entity:?})")),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    commands.entity(header).add_children(&[name_e, ent_e]);

    // Status row: colored status label + faint polygon count.
    let status_row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let status_lbl = commands
        .spawn((
            Text::new("Invalid"),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb((180, 180, 180))),
        ))
        .id();
    bind_text(commands, status_lbl, move |w| status_label(volume_status(w, entity)).0.to_string());
    bind_text_color(commands, status_lbl, move |w| rgb(status_label(volume_status(w, entity)).1));
    let polys_lbl = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_text(commands, polys_lbl, move |w| match volume_polys(w, entity) {
        Some(n) => format!("{n} polygons"),
        None => String::new(),
    });
    bind_display(commands, polys_lbl, move |w| volume_polys(w, entity).is_some());
    commands.entity(status_row).add_children(&[status_lbl, polys_lbl]);

    // Controls row: "Debug Draw" checkbox + "Rebuild" button.
    let controls = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let debug_cb = checkbox(commands, volume_debug_initial());
    bind_2way(
        commands,
        debug_cb,
        move |w| volume_debug(w, entity),
        move |w, v: &bool| {
            // Only queue when the user's edit actually differs from state —
            // the action toggles, so a redundant queue would flip it back.
            if volume_debug(w, entity) != *v {
                if let Some(s) = w.get_resource::<NavMeshPanelState>() {
                    s.queue_toggle_volume_debug(entity);
                }
            }
        },
    );
    let debug_lbl = commands
        .spawn((
            Text::new("Debug Draw"),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::new_with_no_wrap(),
        ))
        .id();
    let rebuild = button(commands, &fonts.ui, "Rebuild");
    commands.entity(rebuild).insert(VolumeRebuildBtn(entity));
    commands.entity(controls).add_children(&[debug_cb, debug_lbl, rebuild]);

    commands.entity(card).add_children(&[header, status_row, controls]);
    card
}

fn volume_debug_initial() -> bool {
    // Seeded for real by `bind_2way` on its first run.
    false
}

fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

// ── Systems ────────────────────────────────────────────────────────────────────

/// Global action buttons → queue the matching `PanelAction`.
fn button_clicks(
    rebuild_all: Query<&Interaction, (With<RebuildAllBtn>, Changed<Interaction>)>,
    reset_agents: Query<&Interaction, (With<ResetAgentsBtn>, Changed<Interaction>)>,
    bake: Query<&Interaction, (With<BakeBtn>, Changed<Interaction>)>,
    state: Option<Res<NavMeshPanelState>>,
    mirror: Option<Res<NavMeshPanelMirror>>,
) {
    let Some(state) = state else { return };
    if rebuild_all.iter().any(|i| *i == Interaction::Pressed) {
        if let Some(mirror) = &mirror {
            for row in &mirror.volumes {
                state.queue_rebuild_volume(row.entity);
            }
        }
    }
    if reset_agents.iter().any(|i| *i == Interaction::Pressed) {
        state.queue_reset_agents();
    }
    if bake.iter().any(|i| *i == Interaction::Pressed) {
        state.queue_bake_to_disk();
    }
}

/// Per-volume "Rebuild" buttons → queue a single-volume rebuild.
fn volume_actions(
    q: Query<(&Interaction, &VolumeRebuildBtn), Changed<Interaction>>,
    state: Option<Res<NavMeshPanelState>>,
) {
    let Some(state) = state else { return };
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed {
            state.queue_rebuild_volume(btn.0);
        }
    }
}
