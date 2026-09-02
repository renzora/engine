//! The corner progress toast: what the import reports once the window is closed.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::font::{icon_text, EmberFonts};
use renzora_ember::reactive::tracked::{bind_display, bind_text, bind_with};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::{spinner, OverlaySurface};

use crate::overlay::{poll_import_task, ImportOverlayState, ImportProgress};

use super::widgets::{hover_cursor, icon_label, icon_msg, txt};
use super::{ToastDismissBtn, ToastRoot, GREEN, RED};

#[derive(PartialEq)]
pub(super) struct OrderedF32(f32);

fn progress_fraction(w: &Rx) -> OrderedF32 {
    let f = match w.get_resource::<ImportOverlayState>().map(|s| s.progress.clone()) {
        Some(ImportProgress::Working { current, total, .. }) if total > 0 => current as f32 / total as f32,
        _ => 0.0,
    };
    OrderedF32(f)
}

/// Owns the corner progress toast: polls the running import, spawns/despawns the
/// toast entity, and auto-dismisses a few seconds after the import finishes.
pub(super) fn manage_import_toast(world: &mut World) {
    let active = world.resource::<ImportOverlayState>().toast_active;
    if active {
        poll_import_task(world); // keep the bar moving while the modal is closed
    }

    // Once the import reaches a terminal state, arm a short auto-dismiss timer
    // so the success/error toast lingers briefly before clearing itself.
    if active {
        let terminal = matches!(
            world.resource::<ImportOverlayState>().progress,
            ImportProgress::Done(_) | ImportProgress::Error(_)
        );
        if terminal {
            let now = world.resource::<Time>().elapsed_secs_f64();
            let dismiss_at = world.resource::<ImportOverlayState>().toast_dismiss_at;
            match dismiss_at {
                None => world.resource_mut::<ImportOverlayState>().toast_dismiss_at = Some(now + 5.0),
                Some(t) if now >= t => {
                    let mut s = world.resource_mut::<ImportOverlayState>();
                    s.toast_active = false;
                    s.toast_dismiss_at = None;
                    s.progress = ImportProgress::Idle;
                    s.pending_files.clear();
                    s.log_entries.clear();
                }
                _ => {}
            }
        }
    }

    let want = world.resource::<ImportOverlayState>().toast_active;
    let mut q = world.query_filtered::<Entity, With<ToastRoot>>();
    let existing: Vec<Entity> = q.iter(world).collect();
    if want && existing.is_empty() {
        let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            spawn_toast(&mut commands, &fonts);
        }
        queue.apply(world);
    } else if !want && !existing.is_empty() {
        for e in existing {
            world.entity_mut(e).despawn();
        }
    }
}

fn spawn_toast(commands: &mut Commands, fonts: &EmberFonts) {
    // Fixed bottom-right card, above the viewport chrome.
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(16.0),
                bottom: Val::Px(16.0),
                width: Val::Px(320.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            BorderColor::all(rgb(border())),
            GlobalZIndex(9200),
            OverlaySurface,
            ToastRoot,
            Name::new("import-toast"),
        ))
        .id();

    // Header: title + dismiss ×.
    let header = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, ..default() }).id();
    let title = icon_label(commands, fonts, "download-simple", "Importing assets", text_primary(), 12.0);
    let close = commands.spawn((Node { padding: UiRect::all(Val::Px(2.0)), ..default() }, Interaction::default(), ToastDismissBtn, hover_cursor())).id();
    let close_x = icon_text(commands, &fonts.phosphor, "x", text_muted(), 13.0);
    commands.entity(close_x).insert(FocusPolicy::Pass);
    commands.entity(close).add_child(close_x);
    commands.entity(header).add_children(&[title, close]);
    commands.entity(root).add_child(header);

    // Working: label + progress bar.
    let working = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() }).id();
    let toprow = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).id();
    let spin = spinner(commands);
    let plabel = txt(commands, fonts, "", 11.0, text_muted());
    bind_text(commands, plabel, |w| match w.get_resource::<ImportOverlayState>().map(|s| s.progress.clone()) {
        Some(ImportProgress::Working { current, total, label }) => format!("[{current}/{total}] {label}"),
        _ => "Starting…".to_string(),
    });
    commands.entity(toprow).add_children(&[spin, plabel]);
    let track = commands.spawn((Node { width: Val::Percent(100.0), height: Val::Px(6.0), overflow: Overflow::clip(), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() }, BackgroundColor(rgb(section_bg())))).id();
    let fill = commands.spawn((Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(rgb(accent())))).id();
    bind_with(commands, fill, progress_fraction, |w, target, v: &OrderedF32| { if let Some(mut n) = w.get_mut::<Node>(target) { n.width = Val::Percent((v.0 * 100.0).clamp(0.0, 100.0)); } });
    commands.entity(track).add_child(fill);
    commands.entity(working).add_children(&[toprow, track]);
    bind_display(commands, working, |w| matches!(w.get_resource::<ImportOverlayState>().map(|s| &s.progress), Some(ImportProgress::Working { .. }) | Some(ImportProgress::Idle)));
    commands.entity(root).add_child(working);

    // Done / Error result lines.
    let (done, done_msg) = icon_msg(commands, fonts, "check-circle", GREEN);
    bind_text(commands, done_msg, |w| match w.get_resource::<ImportOverlayState>().map(|s| s.progress.clone()) {
        Some(ImportProgress::Done(m)) => m,
        _ => String::new(),
    });
    bind_display(commands, done, |w| matches!(w.get_resource::<ImportOverlayState>().map(|s| &s.progress), Some(ImportProgress::Done(_))));
    commands.entity(root).add_child(done);

    let (err, err_msg) = icon_msg(commands, fonts, "warning", RED);
    bind_text(commands, err_msg, |w| match w.get_resource::<ImportOverlayState>().map(|s| s.progress.clone()) {
        Some(ImportProgress::Error(m)) => m,
        _ => String::new(),
    });
    bind_display(commands, err, |w| matches!(w.get_resource::<ImportOverlayState>().map(|s| &s.progress), Some(ImportProgress::Error(_))));
    commands.entity(root).add_child(err);
}

pub(super) fn toast_dismiss_click(
    q: Query<&Interaction, (With<ToastDismissBtn>, Changed<Interaction>)>,
    mut state: Option<ResMut<ImportOverlayState>>,
) {
    let Some(state) = state.as_mut() else { return };
    if q.iter().any(|i| *i == Interaction::Pressed) {
        // Hide the toast. A still-running import keeps writing in the background;
        // dismissing only removes the notification.
        state.toast_active = false;
        state.toast_dismiss_at = None;
        if matches!(state.progress, ImportProgress::Done(_) | ImportProgress::Error(_)) {
            state.progress = ImportProgress::Idle;
            state.pending_files.clear();
            state.log_entries.clear();
        }
    }
}
