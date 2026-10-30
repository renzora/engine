//! The Debug screen: the probes the connected game or editor offers, as cards.
//!
//! Probes (`renzora::probes`) are the editor's old debug panels without their
//! UI: the game builds a report from its `World`, and this screen draws it.
//! Every probe is drawn by the one renderer here, from the report's small
//! vocabulary of figures, sparklines, bars, tables, notes and controls, so a
//! plugin's probe needs no companion code.
//!
//! Only probes shown here are watched, and the game builds reports only for
//! watched probes, so an unopened probe costs the game nothing.

use std::collections::BTreeSet;

use bevy::prelude::*;
use renzora::probes::{ProbeItem, ProbeLevel, ProbeReport};
use renzora_ember::font::EmberFonts;
use renzora_ember::theme::*;

use super::{button, card, mono, text, Page, UiAction};
use crate::app::capture::Capture;
use crate::app::games::{ConnRequest, ConnState, Connection};

/// Which probes are shown. Performance by default, so the screen is never
/// empty the first time it is opened.
#[derive(Resource)]
pub(crate) struct DebugView {
    pub watching: BTreeSet<String>,
}

impl Default for DebugView {
    fn default() -> Self {
        Self { watching: BTreeSet::from(["performance".to_string()]) }
    }
}

#[derive(Component)]
struct ProbeList;
#[derive(Component)]
struct CardGrid;
/// A shown probe's card, with the hash of the report it was last drawn from.
#[derive(Component)]
struct ProbeCard {
    id: String,
    drawn: u64,
}
#[derive(Component)]
struct EmptyHint;

pub(crate) fn build(app: &mut App) {
    app.init_resource::<DebugView>()
        .add_systems(Update, (send_watch, list_probes, sync_cards, draw_cards).chain());
}

pub(crate) fn spawn(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // Both columns scroll through ember's scroll view: a bare
    // `Overflow::scroll_y` only clips, and the cards past the window's
    // bottom could not be reached.
    let list = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(12.0)),
                ..default()
            },
            ProbeList,
        ))
        .id();
    let list_scroll = renzora_ember::widgets::scroll_view(commands, list);
    let list_column = commands
        .spawn((
            Node {
                width: Val::Px(230.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                border: UiRect::right(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(rgb(divider())),
        ))
        .id();
    commands.entity(list_column).add_child(list_scroll);
    let hint = text(
        commands,
        fonts,
        "Pick probes on the left. Connect to the editor or a game on the Games screen first.",
        12.0,
        text_muted(),
    );
    commands.entity(hint).insert(EmptyHint);
    let grid = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_content: AlignContent::FlexStart,
                column_gap: Val::Px(10.0),
                row_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(12.0)),
                ..default()
            },
            CardGrid,
        ))
        .id();
    commands.entity(grid).add_child(hint);
    let grid_scroll = renzora_ember::widgets::scroll_view(commands, grid);
    let grid_column = commands
        .spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Column, ..default() })
        .id();
    commands.entity(grid_column).add_child(grid_scroll);
    let root = commands
        .spawn(Node { width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Row, ..default() })
        .id();
    commands.entity(root).add_children(&[list_column, grid_column]);
    root
}

/// Tell the game what to watch when the selection changes, and again on
/// every new connection (a game that just connected knows nothing of it).
/// Watching stops while this screen is hidden: nothing would draw the
/// reports, and the game would build them for nobody.
fn send_watch(
    view: Res<DebugView>,
    page: Res<Page>,
    conn: Res<Connection>,
    mut requests: MessageWriter<ConnRequest>,
    mut sent: Local<Option<(Vec<String>, bool)>>,
) {
    let connected = conn.state == ConnState::Connected;
    let wanted: Vec<String> = if *page == Page::Debug { view.watching.iter().cloned().collect() } else { Vec::new() };
    let now = (wanted, connected);
    if sent.as_ref() == Some(&now) {
        return;
    }
    if connected {
        requests.write(ConnRequest::Watch(now.0.clone()));
    }
    *sent = Some(now);
}

/// The probe list down the left, grouped by category, rebuilt when the list
/// or the selection changes.
fn list_probes(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    capture: Res<Capture>,
    view: Res<DebugView>,
    lists: Query<(Entity, Option<&Children>), With<ProbeList>>,
    mut drawn: Local<Option<(Vec<String>, BTreeSet<String>)>>,
) {
    let Some(fonts) = fonts else { return };
    let Ok((list, children)) = lists.single() else { return };
    let ids: Vec<String> = capture.probes.iter().map(|p| p.id.clone()).collect();
    let now = (ids, view.watching.clone());
    if drawn.as_ref() == Some(&now) && children.is_some() {
        return;
    }
    *drawn = Some(now);
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    let title = text(&mut commands, &fonts, "Probes", 13.0, text_primary());
    commands.entity(list).add_child(title);
    if capture.probes.is_empty() {
        let none = text(&mut commands, &fonts, "None yet: connect on the Games screen.", 11.0, text_muted());
        commands.entity(list).add_child(none);
        return;
    }
    let mut probes = capture.probes.clone();
    probes.sort_by(|a, b| (a.category.as_str(), a.title.as_str()).cmp(&(b.category.as_str(), b.title.as_str())));
    let mut category = "";
    for probe in &probes {
        if probe.category != category {
            category = &probe.category;
            let heading = text(&mut commands, &fonts, category.to_uppercase(), 10.0, text_muted());
            commands.entity(heading).insert(Node { margin: UiRect::top(Val::Px(8.0)), ..default() });
            commands.entity(list).add_child(heading);
        }
        let on = view.watching.contains(&probe.id);
        let icon = if on { "check-square" } else { "square" };
        let row = button(&mut commands, &fonts, Some(icon), &probe.title, UiAction::ToggleProbe(probe.id.clone()));
        if on {
            commands.entity(row).insert(BackgroundColor(rgb(hover_bg())));
        }
        commands.entity(list).add_child(row);
    }
}

/// One card per shown probe, in the selection's order.
fn sync_cards(
    mut commands: Commands,
    view: Res<DebugView>,
    grids: Query<Entity, With<CardGrid>>,
    cards: Query<(Entity, &ProbeCard)>,
    mut hints: Query<&mut Node, With<EmptyHint>>,
) {
    if !view.is_changed() && !cards.is_empty() {
        return;
    }
    let Ok(grid) = grids.single() else { return };
    for (entity, card_of) in &cards {
        if !view.watching.contains(&card_of.id) {
            commands.entity(entity).despawn();
        }
    }
    for id in &view.watching {
        if cards.iter().any(|(_, c)| &c.id == id) {
            continue;
        }
        let c = card(&mut commands);
        commands.entity(c).insert((
            ProbeCard { id: id.clone(), drawn: 0 },
            Node {
                width: Val::Px(400.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(6.0),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
        ));
        commands.entity(grid).add_child(c);
    }
    for mut node in &mut hints {
        node.display = if view.watching.is_empty() { Display::Flex } else { Display::None };
    }
}

/// Redraw a card when its report changed. A report that arrives unchanged
/// (most of them: a scene that is not changing) costs nothing.
fn draw_cards(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    capture: Res<Capture>,
    mut cards: Query<(Entity, &mut ProbeCard, Option<&Children>)>,
) {
    let Some(fonts) = fonts else { return };
    if !capture.is_changed() {
        return;
    }
    for (entity, mut card_of, children) in &mut cards {
        let id = card_of.id.clone();
        let title = capture.probes.iter().find(|p| p.id == id).map_or(id.as_str(), |p| p.title.as_str());
        let report = capture.reports.get(&id);
        let hash = report.map_or(1, |(_, h)| *h);
        if hash == card_of.drawn && children.is_some() {
            continue;
        }
        card_of.drawn = hash;
        if let Some(children) = children {
            for child in children.iter() {
                commands.entity(child).despawn();
            }
        }
        let heading = text(&mut commands, &fonts, title, 14.0, text_primary());
        commands.entity(entity).add_child(heading);
        match report {
            Some((report, _)) => draw_report(&mut commands, &fonts, entity, &id, report),
            None => {
                let waiting = text(&mut commands, &fonts, "Waiting for the first report.", 11.0, text_muted());
                commands.entity(entity).add_child(waiting);
            }
        }
    }
}

fn level_color(level: ProbeLevel) -> (u8, u8, u8) {
    match level {
        ProbeLevel::Normal => text_primary(),
        ProbeLevel::Good => play_green(),
        ProbeLevel::Warn => warn_amber(),
        ProbeLevel::Bad => (226, 92, 92),
        ProbeLevel::Muted => text_muted(),
    }
}

fn row(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .id()
}

fn draw_report(commands: &mut Commands, fonts: &EmberFonts, card: Entity, probe: &str, report: &ProbeReport) {
    for section in &report.sections {
        if !section.title.is_empty() {
            let t = text(commands, fonts, section.title.to_uppercase(), 10.0, text_muted());
            commands.entity(t).insert(Node { margin: UiRect::top(Val::Px(6.0)), ..default() });
            commands.entity(card).add_child(t);
        }
        for item in &section.items {
            let entity = draw_item(commands, fonts, probe, item);
            commands.entity(card).add_child(entity);
        }
    }
}

fn draw_item(commands: &mut Commands, fonts: &EmberFonts, probe: &str, item: &ProbeItem) -> Entity {
    match item {
        ProbeItem::Figure { label, value, unit, level } => {
            let col = commands.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).id();
            let l = text(commands, fonts, label.as_str(), 11.0, text_muted());
            let line = commands
                .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Baseline, column_gap: Val::Px(6.0), ..default() })
                .id();
            let v = text(commands, fonts, value.as_str(), 24.0, level_color(*level));
            let u = text(commands, fonts, unit.as_str(), 12.0, text_muted());
            commands.entity(line).add_children(&[v, u]);
            commands.entity(col).add_children(&[l, line]);
            col
        }
        ProbeItem::Stat { label, value, level } => {
            let r = row(commands);
            let l = text(commands, fonts, label.as_str(), 12.0, text_muted());
            let v = mono(commands, fonts, value.as_str(), 12.0, level_color(*level));
            commands.entity(r).add_children(&[l, v]);
            r
        }
        ProbeItem::Series { label, values, unit } => sparkline(commands, fonts, label, values, unit),
        ProbeItem::Bar { label, fraction, value, level } => {
            let col = commands.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() }).id();
            let r = row(commands);
            let l = text(commands, fonts, label.as_str(), 11.0, text_primary());
            let v = mono(commands, fonts, value.as_str(), 11.0, text_muted());
            commands.entity(r).add_children(&[l, v]);
            let track = commands
                .spawn((
                    Node { height: Val::Px(5.0), width: Val::Percent(100.0), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() },
                    BackgroundColor(rgb(faint_bg())),
                ))
                .id();
            let fill_color = if *level == ProbeLevel::Normal { accent() } else { level_color(*level) };
            let fill = commands
                .spawn((
                    Node {
                        height: Val::Percent(100.0),
                        width: Val::Percent(fraction.clamp(0.0, 1.0) * 100.0),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(rgb(fill_color)),
                ))
                .id();
            commands.entity(track).add_child(fill);
            commands.entity(col).add_children(&[r, track]);
            col
        }
        ProbeItem::Table { columns, rows } => {
            let col = commands.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() }).id();
            let share = 100.0 / columns.len().max(1) as f32;
            let line = |commands: &mut Commands, cells: &[String], color: (u8, u8, u8)| {
                let r = commands.spawn(Node { flex_direction: FlexDirection::Row, ..default() }).id();
                for cell in cells {
                    let c = mono(commands, fonts, cell.as_str(), 10.0, color);
                    commands.entity(c).insert(Node { width: Val::Percent(share), overflow: Overflow::clip_x(), ..default() });
                    commands.entity(r).add_child(c);
                }
                r
            };
            let head = line(commands, columns, text_muted());
            commands.entity(col).add_child(head);
            // A card is a summary; a table past this is the probe's to trim.
            for cells in rows.iter().take(40) {
                let r = line(commands, cells, text_primary());
                commands.entity(col).add_child(r);
            }
            if rows.len() > 40 {
                let more = text(commands, fonts, format!("{} more rows", rows.len() - 40), 10.0, text_muted());
                commands.entity(col).add_child(more);
            }
            col
        }
        ProbeItem::Note { text: words, level } => {
            let t = text(commands, fonts, words.as_str(), 11.0, level_color(*level));
            commands.entity(t).insert(Node { max_width: Val::Percent(100.0), ..default() });
            t
        }
        ProbeItem::Toggle { id, label, on } => {
            let r = row(commands);
            let l = text(commands, fonts, label.as_str(), 12.0, text_primary());
            let b = button(
                commands,
                fonts,
                Some(if *on { "toggle-right" } else { "toggle-left" }),
                if *on { "On" } else { "Off" },
                UiAction::ProbeAct { probe: probe.to_string(), item: id.clone(), value: Some(!on) },
            );
            commands.entity(r).add_children(&[l, b]);
            r
        }
        ProbeItem::Button { id, label } => button(
            commands,
            fonts,
            None,
            label,
            UiAction::ProbeAct { probe: probe.to_string(), item: id.clone(), value: None },
        ),
    }
}

/// A history as thin bars, newest on the right, scaled to its own peak; the
/// latest value and the peak read out beside it.
fn sparkline(commands: &mut Commands, fonts: &EmberFonts, label: &str, values: &[f32], unit: &str) -> Entity {
    const BARS: usize = 90;
    const HEIGHT: f32 = 40.0;
    let col = commands.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() }).id();
    let latest = values.last().copied().unwrap_or(0.0);
    let peak = values.iter().copied().fold(0.0f32, f32::max);
    let r = row(commands);
    let l = text(commands, fonts, label, 11.0, text_muted());
    let v = mono(commands, fonts, format!("{latest:.2} {unit}  (peak {peak:.2})"), 10.0, text_muted());
    commands.entity(r).add_children(&[l, v]);
    let strip = commands
        .spawn((
            Node {
                height: Val::Px(HEIGHT),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexEnd,
                column_gap: Val::Px(1.0),
                ..default()
            },
            BackgroundColor(rgb(faint_bg())),
        ))
        .id();
    let shown = &values[values.len().saturating_sub(BARS)..];
    let scale = if peak > 0.0 { HEIGHT / peak } else { 0.0 };
    for value in shown {
        let bar = commands
            .spawn((
                Node { flex_grow: 1.0, height: Val::Px((value * scale).max(1.0)), ..default() },
                BackgroundColor(rgb(accent())),
            ))
            .id();
        commands.entity(strip).add_child(bar);
    }
    commands.entity(col).add_children(&[r, strip]);
    col
}
