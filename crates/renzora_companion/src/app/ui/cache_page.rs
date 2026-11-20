//! The Cache screen: the open project as the companion sees it, and the
//! thumbnail cache it keeps in step with it (see `app::cache`).

use bevy::prelude::*;
use renzora_ember::font::EmberFonts;
use renzora_ember::theme::*;

use super::{button, card, mono, text, Page, UiAction};
use crate::app::cache::{thumbnailed, CacheRequest, ProjectCache, Thumbnailed};

#[derive(Component)]
struct CacheLine(usize);

pub(crate) fn build(app: &mut App) {
    app.add_systems(Update, refresh);
}

pub(crate) fn spawn(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let project = card(commands);
    let title = text(commands, fonts, "Project", 13.0, text_primary());
    commands.entity(project).add_child(title);
    for i in 0..3 {
        let line = mono(commands, fonts, "", 12.0, text_muted());
        commands.entity(line).insert(CacheLine(i));
        commands.entity(project).add_child(line);
    }

    let thumbs = card(commands);
    let title = text(commands, fonts, "Thumbnails", 13.0, text_primary());
    let note = text(
        commands,
        fonts,
        "Models added or changed get their thumbnail rendered here as soon as they land, so the Assets panel finds them ready. A deleted model's thumbnail goes with it.",
        11.0,
        text_muted(),
    );
    commands.entity(thumbs).add_children(&[title, note]);
    for i in 3..5 {
        let line = mono(commands, fonts, "", 12.0, text_muted());
        commands.entity(line).insert(CacheLine(i));
        commands.entity(thumbs).add_child(line);
    }
    let buttons = commands
        .spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(8.0), margin: UiRect::top(Val::Px(4.0)), ..default() })
        .id();
    let render = button(commands, fonts, Some("image"), "Render missing thumbnails", UiAction::Cache(CacheRequest::RenderMissing));
    let clear = button(commands, fonts, Some("trash"), "Clear thumbnail cache", UiAction::Cache(CacheRequest::ClearThumbnails));
    commands.entity(buttons).add_children(&[render, clear]);
    commands.entity(thumbs).add_child(buttons);

    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        })
        .id();
    commands.entity(root).add_children(&[project, thumbs]);
    root
}

fn bytes(n: u64) -> String {
    match n {
        n if n >= 1 << 30 => format!("{:.2} GB", n as f64 / (1u64 << 30) as f64),
        n if n >= 1 << 20 => format!("{:.1} MB", n as f64 / (1u64 << 20) as f64),
        n if n >= 1 << 10 => format!("{:.0} KB", n as f64 / 1024.0),
        n => format!("{n} B"),
    }
}

fn refresh(page: Res<Page>, cache: Res<ProjectCache>, mut lines: Query<(&CacheLine, &mut Text)>) {
    if *page != Page::Cache || !(cache.is_changed() || page.is_changed()) {
        return;
    }
    let count = |kind| cache.files.keys().filter(|k| thumbnailed(k) == Some(kind)).count();
    let (models, materials) = (count(Thumbnailed::Model), count(Thumbnailed::Material));
    let total: u64 = cache.files.values().map(|f| f.size).sum();
    let content = [
        cache.root.as_ref().map_or("No project open yet".to_string(), |r| r.display().to_string()),
        format!("{} files, {}", cache.files.len(), bytes(total)),
        format!("{models} models, {materials} materials"),
        match (cache.thumbnails, cache.backlog.len()) {
            (_, waiting) if waiting > 0 => format!("checking for missing thumbnails, {waiting} files to go"),
            (Some((count, size)), _) => format!("{count} cached, {}", bytes(size)),
            (None, _) => "measuring...".to_string(),
        },
        format!("This session: {} rendered ahead of time, {} removed with their file", cache.prewarmed, cache.pruned),
    ];
    for (line, mut text) in &mut lines {
        if let Some(s) = content.get(line.0) {
            if text.0 != *s {
                text.0.clone_from(s);
            }
        }
    }
}
