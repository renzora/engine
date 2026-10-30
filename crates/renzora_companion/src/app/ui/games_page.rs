//! The Games screen: what can be profiled, and saved captures to open.

use std::path::PathBuf;

use bevy::prelude::*;
use renzora_ember::font::EmberFonts;
use renzora_ember::theme::*;

use super::{button, card, mono, text, Page, UiAction};
use crate::app::capture::Capture;
use crate::app::games::{ConnState, Connection, Games};
use crate::app::PairingKey;

#[derive(Component)]
struct GameList;
#[derive(Component)]
struct CaptureList;

/// What the lists last showed, so they rebuild only when it changes.
#[derive(Resource, Default)]
struct Listed {
    games: u64,
    captures: Vec<PathBuf>,
}

pub(crate) fn build(app: &mut App) {
    app.init_resource::<Listed>().add_systems(Update, (list_games, list_captures));
}

pub(crate) fn spawn(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let games_card = card(commands);
    commands.entity(games_card).insert(Node {
        flex_grow: 1.0,
        min_width: Val::Px(0.0),
        flex_direction: FlexDirection::Column,
        padding: UiRect::all(Val::Px(14.0)),
        row_gap: Val::Px(10.0),
        border_radius: BorderRadius::all(Val::Px(8.0)),
        ..default()
    });
    let title = text(commands, fonts, "Games you can profile", 14.0, text_primary());
    let sub = text(
        commands,
        fonts,
        "The editor, Play on this PC, and games exported with profiling allowed on your network.",
        12.0,
        text_muted(),
    );
    let list = commands
        .spawn((Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() }, GameList))
        .id();
    let captures_title = text(commands, fonts, "Saved captures", 14.0, text_primary());
    commands.entity(captures_title).insert(Node { margin: UiRect::top(Val::Px(12.0)), ..default() });
    let captures = commands
        .spawn((Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() }, CaptureList))
        .id();
    commands.entity(games_card).add_children(&[title, sub, list, captures_title, captures]);

    let side = card(commands);
    commands.entity(side).insert(Node {
        width: Val::Px(340.0),
        flex_shrink: 0.0,
        flex_direction: FlexDirection::Column,
        padding: UiRect::all(Val::Px(14.0)),
        row_gap: Val::Px(8.0),
        border_radius: BorderRadius::all(Val::Px(8.0)),
        ..default()
    });
    let how = text(commands, fonts, "How games find you", 13.0, text_primary());
    let steps = text(
        commands,
        fonts,
        "1. Export with Allow profiling on. The build carries this editor's pairing key.\n\n\
         2. Run it on any device on the same network. It announces itself and appears here.\n\n\
         3. Connect. Only a companion with the matching key can, so nothing on the device needs setting up.\n\n\
         With Allow profiling off, a game never announces itself or opens a port.",
        12.0,
        text_muted(),
    );
    commands.entity(side).add_children(&[how, steps]);

    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(10.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        })
        .id();
    commands.entity(root).add_children(&[games_card, side]);
    root
}

#[allow(clippy::too_many_arguments)]
fn list_games(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    games: Res<Games>,
    conn: Res<Connection>,
    page: Res<Page>,
    mut listed: ResMut<Listed>,
    list: Query<(Entity, Option<&Children>), With<GameList>>,
    _key: Res<PairingKey>,
) {
    if *page != Page::Games {
        return;
    }
    let Some(fonts) = fonts else { return };
    let Ok((list, children)) = list.single() else { return };
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (addr, (info, _)) in &games.found {
        addr.hash(&mut h);
        info.name.hash(&mut h);
        info.build.hash(&mut h);
    }
    conn.game.as_ref().map(|g| g.address).hash(&mut h);
    format!("{:?}", conn.state).hash(&mut h);
    let signature = h.finish();
    if signature == listed.games && children.is_some() {
        return;
    }
    listed.games = signature;
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    if games.found.is_empty() {
        let none = text(
            &mut commands,
            &fonts,
            "Nothing found yet. The editor appears within a second; press Play to add your game.",
            12.0,
            text_muted(),
        );
        commands.entity(list).add_child(none);
        return;
    }
    for (info, _) in games.found.values() {
        let row = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(12.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(rgb(faint_bg())),
            ))
            .id();
        let icon = renzora_ember::font::icon_text(
            &mut commands,
            &fonts.phosphor,
            if info.local { "desktop" } else { "device-mobile" },
            text_muted(),
            20.0,
        );
        let name = text(&mut commands, &fonts, info.name.clone(), 13.0, text_primary());
        let place = mono(&mut commands, &fonts, format!("{}  ·  {}  ·  pid {}", info.place(), info.build, info.pid), 11.0, text_muted());
        let words = commands.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), flex_grow: 1.0, ..default() }).id();
        commands.entity(words).add_children(&[name, place]);
        let connected = conn.game.as_ref().is_some_and(|g| g.address == info.address);
        let state = match (&conn.state, connected) {
            (ConnState::Connected, true) => Some(("Profiling", play_green())),
            (ConnState::Connecting, true) => Some(("Connecting", text_muted())),
            (ConnState::Failed(_), true) => Some(("Could not connect", warn_amber())),
            _ => None,
        };
        let mut kids = vec![icon, words];
        if let Some((label, color)) = state {
            kids.push(text(&mut commands, &fonts, label, 12.0, color));
        }
        kids.push(if connected {
            button(&mut commands, &fonts, Some("plugs"), "Disconnect", UiAction::Disconnect)
        } else {
            button(&mut commands, &fonts, Some("plugs-connected"), "Connect", UiAction::Connect(info.clone()))
        });
        commands.entity(row).add_children(&kids);
        commands.entity(list).add_child(row);
    }
    if let (ConnState::Failed(why), Some(game)) = (&conn.state, &conn.game) {
        let why = text(&mut commands, &fonts, format!("{}: {why}", game.name), 12.0, warn_amber());
        commands.entity(list).add_child(why);
    }
}

fn list_captures(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    page: Res<Page>,
    mut listed: ResMut<Listed>,
    list: Query<(Entity, Option<&Children>), With<CaptureList>>,
    time: Res<Time<Real>>,
    mut last: Local<f32>,
) {
    if *page != Page::Games {
        return;
    }
    let now = time.elapsed_secs();
    if !page.is_changed() && now - *last < 2.0 {
        return;
    }
    *last = now;
    let Some(fonts) = fonts else { return };
    let Ok((list, children)) = list.single() else { return };
    let mut found: Vec<(PathBuf, std::time::SystemTime)> = Capture::folder()
        .and_then(|dir| std::fs::read_dir(dir).ok())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "rzprof"))
        .filter_map(|e| Some((e.path(), e.metadata().ok()?.modified().ok()?)))
        .collect();
    found.sort_by_key(|(_, modified)| std::cmp::Reverse(*modified));
    found.truncate(12);
    let paths: Vec<PathBuf> = found.into_iter().map(|(p, _)| p).collect();
    if paths == listed.captures && children.is_some() {
        return;
    }
    listed.captures = paths.clone();
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    if paths.is_empty() {
        let none = text(&mut commands, &fonts, "None yet. Save one from the header while profiling.", 12.0, text_muted());
        commands.entity(list).add_child(none);
        return;
    }
    for path in paths {
        let label = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let row = commands
            .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(10.0), ..default() })
            .id();
        let name = mono(&mut commands, &fonts, label, 12.0, text_primary());
        let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
        let open = button(&mut commands, &fonts, Some("folder-open"), "Open", UiAction::OpenCapture(path));
        commands.entity(row).add_children(&[name, spacer, open]);
        commands.entity(list).add_child(row);
    }
}
