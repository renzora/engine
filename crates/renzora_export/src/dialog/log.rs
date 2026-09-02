//! The build log view: a heading, a progress bar, a scrolling terminal of the
//! live build output, and Copy log / Cancel.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_bg, bind_display, bind_text, bind_text_color};
use renzora_ember::reactive::{react, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::{scroll_view_pinned, spinner};

use crate::overlay::{ExportOverlayState, ExportProgress};

use super::widgets::{cursor, pill_button, txt};
use super::{CancelOrBackBtn, CopyLogBtn, LogScroll, GREEN, RED};

/// Rough crate count for a full lean build, used only to scale the progress bar
/// (cargo gives no total in piped mode). Over/under-estimating just makes the bar
/// move a little fast or slow; it snaps to full on "Finished"/Done.
const LEAN_BUILD_ESTIMATE: f32 = 480.0;

/// Progress-bar fraction (0..1) from compiled-crate count, full once finished.
fn build_fraction(s: &ExportOverlayState) -> f32 {
    if s.build_finished {
        1.0
    } else if s.build_compiled == 0 {
        0.02
    } else {
        (s.build_compiled as f32 / LEAN_BUILD_ESTIMATE).min(0.97)
    }
}

/// Reactively drive a node's width as a percentage (for the progress fill).
fn bind_width_pct(
    commands: &mut Commands,
    target: Entity,
    value: impl Fn(&Rx) -> f32 + Send + Sync + 'static,
) {
    react(commands, move |w: &mut World| {
        if w.get_entity(target).is_err() {
            return false;
        }
        let v = value(&Rx::new(&*w)).clamp(0.0, 1.0);
        if let Some(mut n) = w.get_mut::<Node>(target) {
            n.width = Val::Percent(v * 100.0);
        }
        true
    });
}

pub(super) fn build_log_view(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let view = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), flex_grow: 1.0, min_height: Val::Px(0.0), ..default() })
        .id();

    // Heading — reflects the current phase.
    let heading_row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).id();
    let sp = spinner(commands);
    bind_display(commands, sp, |w| {
        w.get_resource::<ExportOverlayState>().map(|s| s.active_task.is_some()).unwrap_or(false)
    });
    let heading = txt(commands, fonts, "", 14.0, text_primary());
    bind_text(commands, heading, |w| match w.get_resource::<ExportOverlayState>().map(|s| s.progress.clone()) {
        Some(ExportProgress::Done(_)) => renzora::lang::t("export.status.complete"),
        Some(ExportProgress::Error(_)) => renzora::lang::t("export.status.failed"),
        _ => renzora::lang::t("export.status.exporting"),
    });
    bind_text_color(commands, heading, |w| match w.get_resource::<ExportOverlayState>().map(|s| s.progress.clone()) {
        Some(ExportProgress::Done(_)) => rgb(GREEN),
        Some(ExportProgress::Error(_)) => rgb(RED),
        _ => rgb(text_primary()),
    });
    commands.entity(heading_row).add_children(&[sp, heading]);

    // Progress bar.
    let track = commands.spawn((Node { width: Val::Percent(100.0), height: Val::Px(8.0), border_radius: BorderRadius::all(Val::Px(4.0)), overflow: Overflow::clip(), ..default() }, BackgroundColor(rgb(card_bg())))).id();
    let fill = commands.spawn((Node { width: Val::Percent(2.0), height: Val::Percent(100.0), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() }, BackgroundColor(rgb(accent())))).id();
    commands.entity(track).add_child(fill);
    bind_width_pct(commands, fill, |w| w.get_resource::<ExportOverlayState>().map(build_fraction).unwrap_or(0.0));
    bind_bg(commands, fill, |w| match w.get_resource::<ExportOverlayState>().map(|s| s.progress.clone()) {
        Some(ExportProgress::Error(_)) => rgb(RED),
        Some(ExportProgress::Done(_)) => rgb(GREEN),
        _ => rgb(accent()),
    });

    // Terminal — dark monospace box with the FULL build log in a pinned scroll
    // view: it auto-follows the bottom as new output streams in, but releases if
    // the user scrolls up to read back (an error can otherwise be pushed out of
    // view by cargo's huge linker-command dump). `build_log` is tail-capped at 600
    // lines upstream so this stays bounded.
    // Fills whatever height the modal has rather than a fixed 360 px. The modal
    // is tall and the log was showing twenty lines of an eight-hundred-line
    // build with empty space beneath it; `min_height` keeps it usable when the
    // window is short.
    let term = commands.spawn((Node { width: Val::Percent(100.0), flex_grow: 1.0, min_height: Val::Px(220.0), flex_direction: FlexDirection::Column, padding: UiRect::all(Val::Px(8.0)), overflow: Overflow::clip(), ..default() }, BackgroundColor(rgb((14, 16, 20))))).id();
    let log_text = commands.spawn((Text::new(""), ui_font(&fonts.mono, 11.0), TextColor(rgb(text_muted())), FocusPolicy::Pass)).id();
    bind_text(commands, log_text, |w| {
        w.get_resource::<ExportOverlayState>().map(|s| s.build_log.join("\n")).unwrap_or_default()
    });
    let log_scroll = scroll_view_pinned(commands, log_text);
    commands.entity(log_scroll).insert(LogScroll);
    commands.entity(term).add_child(log_scroll);

    // Buttons: Copy log (left), Cancel/Back (right).
    let btn_row = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, ..default() }).id();
    let copy_btn = pill_button(commands, fonts, "clipboard", &renzora::lang::t("export.btn.copy_log"));
    commands.entity(copy_btn).insert(CopyLogBtn);
    let btn = commands.spawn((Node { min_width: Val::Px(100.0), height: Val::Px(32.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(5.0)), ..default() }, BackgroundColor(rgb(section_bg())), Interaction::default(), CancelOrBackBtn, cursor())).id();
    let btn_label = commands.spawn((Text::new(renzora::lang::t("common.cancel")), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_primary())), FocusPolicy::Pass)).id();
    bind_text(commands, btn_label, |w| {
        if w.get_resource::<ExportOverlayState>().map(|s| s.active_task.is_some()).unwrap_or(false) {
            renzora::lang::t("common.cancel")
        } else {
            renzora::lang::t("export.btn.back")
        }
    });
    commands.entity(btn).add_child(btn_label);
    commands.entity(btn_row).add_children(&[copy_btn, btn]);

    commands.entity(view).add_children(&[heading_row, track, term, btn_row]);
    view
}
