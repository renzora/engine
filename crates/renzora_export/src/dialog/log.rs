//! The export's progress view: what is happening, in a line, and Copy log /
//! Cancel.
//!
//! It used to stream the whole build log into a terminal box here. A lean
//! build now shows cargo in a real terminal window (`crate::console`), where it
//! has its colours and progress bar, so repeating it here was a worse copy of
//! that window. What remains is the step the export is on, the result, and on
//! a failure the error. The full log is still collected, so Copy log has it.

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::reactive::tracked::{bind_display, bind_text, bind_text_color};
use renzora_ember::theme::*;
use renzora_ember::widgets::{scroll_area, spinner};
use renzora_ember::UiInteraction;

use crate::overlay::{ExportOverlayState, ExportProgress};

use super::widgets::{cursor, mode_label, pill_button, txt};
use super::{CancelOrBackBtn, CloseBtn, CopyLogBtn, LogScroll, OpenFolderBtn, GREEN, RED};

fn progress(w: &renzora_ember::reactive::Rx) -> Option<ExportProgress> {
    w.get_resource::<ExportOverlayState>().map(|s| s.progress.clone())
}

pub(super) fn build_log_view(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let view = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(10.0), flex_grow: 1.0, min_height: Val::Px(0.0), padding: UiRect::all(Val::Px(20.0)), ..default() })
        .id();

    // The status. The card sizes to it, so the padding is what gives it room.
    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            padding: UiRect::axes(Val::Px(0.0), Val::Px(28.0)),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();

    let heading_row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(10.0), ..default() }).id();
    let sp = spinner(commands);
    bind_display(commands, sp, |w| {
        w.get_resource::<ExportOverlayState>().map(|s| s.active_task.is_some()).unwrap_or(false)
    });
    let heading = txt(commands, fonts, "", 18.0, text_primary());
    bind_text(commands, heading, |w| match progress(w) {
        Some(ExportProgress::Done(_)) => renzora_lang_api::t("export.status.complete"),
        Some(ExportProgress::Error(_)) => renzora_lang_api::t("export.status.failed"),
        _ => renzora_lang_api::t("export.status.exporting_project"),
    });
    bind_text_color(commands, heading, |w| match progress(w) {
        Some(ExportProgress::Done(_)) => rgb(GREEN),
        Some(ExportProgress::Error(_)) => rgb(RED),
        _ => rgb(text_primary()),
    });
    commands.entity(heading_row).add_children(&[sp, heading]);

    // Which build this is, since the settings that said so are hidden while it
    // runs.
    let sub = txt(commands, fonts, "", 12.0, text_muted());
    bind_text(commands, sub, |w| {
        w.get_resource::<ExportOverlayState>()
            .map(|s| format!("{} · {}", s.platform.display_name(), mode_label(s.platform, s.packaging_mode)))
            .unwrap_or_default()
    });

    // One line: the step it is on, where it put the result, or why it failed.
    // An error can run to cargo's last sixty lines when the build ran here
    // rather than in a console, so it scrolls instead of growing the dialog.
    let detail = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
            TextLayout::justify(Justify::Center),
            PASS_THROUGH,
        ))
        .id();
    bind_text(commands, detail, |w| {
        let Some(s) = w.get_resource::<ExportOverlayState>() else { return String::new() };
        match &s.progress {
            ExportProgress::Done(msg) | ExportProgress::Error(msg) => msg.clone(),
            ExportProgress::Working(step) => step.trim().to_string(),
            _ => String::new(),
        }
    });
    bind_text_color(commands, detail, |w| match progress(w) {
        Some(ExportProgress::Error(_)) => rgb(text_primary()),
        _ => rgb(text_muted()),
    });
    let detail_scroll = scroll_area(commands, detail, 260.0);
    commands.entity(detail_scroll).insert(LogScroll);
    let detail_box = commands
        .spawn(Node { width: Val::Percent(100.0), margin: UiRect::top(Val::Px(6.0)), flex_direction: FlexDirection::Column, ..default() })
        .add_child(detail_scroll)
        .id();

    commands.entity(body).add_children(&[heading_row, sub, detail_box]);

    // Footer: Copy log on the left; on the right, Cancel while the build runs,
    // then Open folder (after a success), Back and Close once it has ended.
    // The dialog's header buttons are hidden in this view, so these are the
    // only way out of it, and there is exactly one Cancel.
    let btn_row = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, ..default() }).id();
    let copy_btn = pill_button(commands, fonts, "clipboard", &renzora_lang_api::t("export.btn.copy_log"));
    commands.entity(copy_btn).insert(CopyLogBtn);

    let actions = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).id();
    let open = footer_button(commands, fonts, &renzora_lang_api::t("export.btn.open_folder"));
    commands.entity(open).insert(OpenFolderBtn);
    bind_display(commands, open, |w| matches!(progress(w), Some(ExportProgress::Done(_))));

    let cancel_or_back = footer_button(commands, fonts, "");
    commands.entity(cancel_or_back).insert(CancelOrBackBtn);
    let label = commands.spawn((Text::new(renzora_lang_api::t("common.cancel")), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_primary())), PASS_THROUGH)).id();
    bind_text(commands, label, |w| {
        if running(w) {
            renzora_lang_api::t("common.cancel")
        } else {
            renzora_lang_api::t("export.btn.back")
        }
    });
    commands.entity(cancel_or_back).add_child(label);

    let close = footer_button(commands, fonts, &renzora_lang_api::t("common.close"));
    commands.entity(close).insert(CloseBtn);
    bind_display(commands, close, |w| !running(w));

    commands.entity(actions).add_children(&[open, cancel_or_back, close]);
    commands.entity(btn_row).add_children(&[copy_btn, actions]);

    commands.entity(view).add_children(&[body, btn_row]);
    view
}

fn running(w: &renzora_ember::reactive::Rx) -> bool {
    w.get_resource::<ExportOverlayState>().is_some_and(|s| s.active_task.is_some())
}

/// One of the footer's buttons. An empty `label` leaves the caller to add its
/// own (bound) text.
fn footer_button(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    let btn = commands
        .spawn((
            Node { min_width: Val::Px(90.0), height: Val::Px(32.0), padding: UiRect::axes(Val::Px(14.0), Val::Px(0.0)), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(5.0)), ..default() },
            BackgroundColor(rgb(section_bg())),
            UiInteraction::default(),
            cursor(),
        ))
        .id();
    if !label.is_empty() {
        let t = commands.spawn((Text::new(label), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_primary())), PASS_THROUGH)).id();
        commands.entity(btn).add_child(t);
    }
    btn
}
