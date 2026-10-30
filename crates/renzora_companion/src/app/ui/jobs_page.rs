//! The Jobs screen: what the companion is doing for the editor.

use bevy::prelude::*;
use renzora_ember::font::EmberFonts;
use renzora_ember::theme::*;

use super::{button, card, mono, text, Page, UiAction};
use crate::app::games::Connection;
use crate::app::jobs::{held, JobQueue};
use crate::app::link::EditorState;

#[derive(Component)]
struct JobLists;
#[derive(Component)]
struct HoldBanner;

pub(crate) fn build(app: &mut App) {
    app.add_systems(Update, list_jobs);
}

pub(crate) fn spawn(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let banner = text(commands, fonts, "", 12.0, warn_amber());
    commands.entity(banner).insert(HoldBanner);
    let lists = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(10.0), ..default() },
            JobLists,
        ))
        .id();
    // Ember's scroll view, not a bare `Overflow::scroll_y`: that only clips,
    // and a long Finished list could not be scrolled.
    let lists_scroll = renzora_ember::widgets::scroll_view(commands, lists);
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
    commands.entity(root).add_children(&[banner, lists_scroll]);
    root
}

#[allow(clippy::too_many_arguments)]
fn list_jobs(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    queue: Res<JobQueue>,
    editor: Res<EditorState>,
    conn: Res<Connection>,
    page: Res<Page>,
    lists: Query<(Entity, Option<&Children>), With<JobLists>>,
    mut banner: Query<&mut Text, With<HoldBanner>>,
    time: Res<Time<Real>>,
    mut last: Local<(f32, u64)>,
) {
    if *page != Page::Jobs {
        return;
    }
    let Some(fonts) = fonts else { return };
    let Ok((lists, children)) = lists.single() else { return };

    // Only when a job is actually held: shown for every recording, it claimed
    // a wait nothing was in.
    let hold = queue
        .waiting()
        .find_map(|(entry, job)| held(entry, job, &editor, &conn))
        .map(|why| match why {
            "recording to stop" => {
                "Recording: jobs wait for it to stop, so the profile measures only the game. Run anyway starts one now; frames recorded while it runs are marked.".to_string()
            }
            why => format!("Jobs that render wait for {why}, so they never compete with it."),
        });
    for mut t in &mut banner {
        let line = hold.clone().unwrap_or_default();
        if t.0 != line {
            t.0 = line;
        }
    }

    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (id, (entry, _)) in &queue.running {
        id.hash(&mut h);
        entry.fraction.map(|f| (f * 50.0) as u32).hash(&mut h);
        entry.note.hash(&mut h);
    }
    for (entry, _) in queue.waiting() {
        entry.id.hash(&mut h);
    }
    queue.finished.front().map(|f| f.id).hash(&mut h);
    let signature = h.finish();
    let now = time.elapsed_secs();
    if (signature == last.1 && children.is_some()) || (now - last.0 < 0.25 && !page.is_changed()) {
        return;
    }
    *last = (now, signature);
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }

    let section = |commands: &mut Commands, title: &str| {
        let c = card(commands);
        let t = text(commands, &fonts, title, 13.0, text_primary());
        commands.entity(c).add_child(t);
        commands.entity(lists).add_child(c);
        c
    };

    let running = section(&mut commands, &format!("Running  {}", queue.running.len()));
    if queue.running.is_empty() {
        let none = text(&mut commands, &fonts, "Nothing running.", 12.0, text_muted());
        commands.entity(running).add_child(none);
    }
    for (entry, _) in queue.running.values() {
        let row = row(&mut commands);
        let label = text(&mut commands, &fonts, entry.label, 12.0, text_primary());
        let target = mono(&mut commands, &fonts, entry.target.clone(), 11.0, text_muted());
        let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
        let progress = match entry.fraction {
            Some(f) => renzora_ember::widgets::progress_sized(&mut commands, f, 160.0, 5.0),
            None => renzora_ember::widgets::progress_indeterminate(&mut commands, 160.0, 5.0),
        };
        let note = mono(&mut commands, &fonts, entry.note.clone(), 11.0, text_muted());
        let cancel = button(&mut commands, &fonts, Some("x"), "Cancel", UiAction::CancelJob(entry.id));
        commands.entity(row).add_children(&[label, target, spacer, progress, note, cancel]);
        commands.entity(running).add_child(row);
    }

    let waiting: Vec<_> = queue.waiting().collect();
    let waiting_card = section(&mut commands, &format!("Waiting  {}", waiting.len()));
    for (entry, job) in waiting.iter().take(50) {
        let row = row(&mut commands);
        let label = text(&mut commands, &fonts, entry.label, 12.0, text_primary());
        let target = mono(&mut commands, &fonts, entry.target.clone(), 11.0, text_muted());
        let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
        let reason = held(entry, job, &editor, &conn);
        let why = match reason {
            Some(why) => format!("waits for {why}"),
            None => "waits for a free worker".to_string(),
        };
        let why = text(&mut commands, &fonts, why, 11.0, text_muted());
        let mut kids = vec![label, target, spacer, why];
        if reason == Some("recording to stop") {
            kids.push(button(&mut commands, &fonts, Some("play"), "Run anyway", UiAction::RunJobAnyway(entry.id)));
        }
        kids.push(button(&mut commands, &fonts, Some("x"), "", UiAction::CancelJob(entry.id)));
        commands.entity(row).add_children(&kids);
        commands.entity(waiting_card).add_child(row);
    }

    let finished = section(&mut commands, "Finished");
    for done in queue.finished.iter().take(30) {
        let row = row(&mut commands);
        let label = text(&mut commands, &fonts, done.label, 12.0, text_primary());
        let target = mono(&mut commands, &fonts, done.target.clone(), 11.0, text_muted());
        let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
        let (result, color) = match &done.result {
            Ok(_) => (format!("done in {:.0} ms", done.took_ms), play_green()),
            Err(e) => (format!("failed: {e}"), warn_amber()),
        };
        let result = text(&mut commands, &fonts, result, 11.0, color);
        commands.entity(row).add_children(&[label, target, spacer, result]);
        commands.entity(finished).add_child(row);
    }
}

fn row(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .id()
}
