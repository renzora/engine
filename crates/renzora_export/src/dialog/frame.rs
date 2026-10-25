//! The modal itself: the backdrop, the panel, the header with Cancel and Export,
//! the three columns, and the swap between the settings form and the build log.

use bevy::prelude::*;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::interaction::{BLOCK, PASS_THROUGH};
use renzora_ember::reactive::tracked::{bind_bg, bind_display, bind_text};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::OverlaySurface;
use renzora_ember::UiInteraction;

use crate::overlay::{ExportOverlayState, ExportProgress, ExportView};

use super::summary::blocking_count;
use super::widgets::{ca, cursor, fullscreen, hairline, platform_short, txt};
use super::{CloseBtn, ExportBtn, ExportRoot, RightPane, EXPORT_BLUE, EXPORT_BLUE_HOT, RED};

/// The dialog's height as a fraction of the window. Kept beside the `Val::Vh`
/// below: the two have to agree or the tab cap is computed against a dialog of a
/// different size.
pub(super) const MODAL_VH: f32 = 0.84;

pub(super) fn spawn_modal(commands: &mut Commands, fonts: &EmberFonts, has_project: bool) {
    let backdrop = commands
        .spawn((
            fullscreen(),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.63)),
            GlobalZIndex(9300),
            BLOCK,
            UiInteraction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            OverlaySurface,
            ExportRoot,
            Name::new("export-modal"),
        ))
        .id();
    let panel = commands
        .spawn((
            Node {
                // Three columns now: the preset list, the settings, and the
                // summary rail. The plugin grid still wants four columns of
                // cards in the middle one, which is what sets the width; the cap
                // keeps it on a narrow window.
                width: Val::Px(1240.0),
                max_width: Val::Vw(96.0),
                // Explicit height, not just a cap. The preset list starts EMPTY,
                // so with only a cap the modal collapsed to the height of
                // whichever tab was showing and jumped every time you switched
                // tab or added a preset. A fixed height keeps the tabs, the log
                // view and the Export button in one place regardless of content.
                height: Val::Vh(84.0),
                max_height: Val::Vh(92.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            BorderColor::all(rgb(border())),
            BLOCK,
            Name::new("export-panel"),
        ))
        .id();
    commands.entity(backdrop).add_child(panel);

    let header = build_header(commands, fonts);
    commands.entity(panel).add_child(header);

    if !has_project {
        let w = txt(commands, fonts, &renzora::lang::t("export.no_project"), 12.0, RED);
        commands.entity(w).insert(Node { margin: UiRect::all(Val::Px(20.0)), ..default() });
        commands.entity(panel).add_child(w);
        return;
    }

    // Settings view: the export form. Hidden once an export starts.
    let settings_view = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, flex_grow: 1.0, min_height: Val::Px(0.0), ..default() })
        .id();
    commands.entity(panel).add_child(settings_view);
    bind_display(commands, settings_view, |w| {
        matches!(w.get_resource::<ExportOverlayState>().map(|s| s.view), Some(ExportView::Settings))
    });

    let sidebar = super::sidebar::build_sidebar(commands, fonts);
    // The middle column is NOT scrolled as a whole: the platform header and tab
    // bar inside it stay fixed, and each tab caps and scrolls its own content
    // (see `settings::finish_tab`).
    let right = commands
        .spawn((
            Node { flex_grow: 1.0, flex_basis: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), padding: UiRect { left: Val::Px(22.0), right: Val::Px(22.0), top: Val::Px(14.0), bottom: Val::Px(0.0) }, min_width: Val::Px(0.0), min_height: Val::Px(0.0), ..default() },
            RightPane { sig: None },
        ))
        .id();
    // Every setting in the middle pane belongs to the selected preset, so with
    // nothing selected there is nothing to configure. Showing the form anyway
    // invited edits that had nowhere to be saved to.
    bind_display(commands, right, |w| {
        w.get_resource::<ExportOverlayState>().is_some_and(|s| s.active_preset.is_some())
    });

    // What stands in its place: say what to do, rather than leaving the pane
    // blank next to a sidebar that already says "press +".
    let right_empty = commands
        .spawn(Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, align_items: AlignItems::Center, justify_content: JustifyContent::Center, row_gap: Val::Px(6.0), min_width: Val::Px(0.0), ..default() })
        .id();
    let ei = icon_text(commands, &fonts.phosphor, "package", text_muted(), 30.0);
    let et = txt(commands, fonts, &renzora::lang::t("export.presets.none_selected"), 12.0, text_muted());
    commands.entity(right_empty).add_children(&[ei, et]);
    bind_display(commands, right_empty, |w| {
        w.get_resource::<ExportOverlayState>().is_some_and(|s| s.active_preset.is_none())
    });

    let rail = super::summary::build_summary_rail(commands, fonts);
    bind_display(commands, rail, |w| {
        w.get_resource::<ExportOverlayState>().is_some_and(|s| s.active_preset.is_some())
    });

    commands.entity(settings_view).add_children(&[sidebar, right, right_empty, rail]);

    // Log view: the live build terminal, progress bar and cancel. Shown while
    // and after an export runs.
    let log_view = super::log::build_log_view(commands, fonts);
    commands.entity(panel).add_child(log_view);
    bind_display(commands, log_view, |w| {
        matches!(w.get_resource::<ExportOverlayState>().map(|s| s.view), Some(ExportView::Log))
    });
}

/// Title and project on the left; Cancel and Export on the right, beside each
/// other because they are the two ways this dialog ends.
fn build_header(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let header = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::axes(Val::Px(18.0), Val::Px(12.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(hairline()),
        ))
        .id();

    let badge = commands
        .spawn((
            Node { width: Val::Px(30.0), height: Val::Px(30.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(7.0)), ..default() },
            BackgroundColor({
                let (r, g, b) = accent();
                ca(r, g, b, 40)
            }),
        ))
        .id();
    let bi = icon_text(commands, &fonts.phosphor, "package", accent(), 16.0);
    commands.entity(badge).add_child(bi);
    let titles = commands
        .spawn(Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(1.0), ..default() })
        .id();
    let title = txt(commands, fonts, &renzora::lang::t("export.title"), 14.5, text_primary());
    let sub = txt(commands, fonts, "", 11.0, text_muted());
    // Which project, which engine: the two facts that decide what an export
    // will contain, and neither was anywhere on the old dialog.
    bind_text(commands, sub, |w| {
        let project = w
            .get_resource::<renzora::core::CurrentProject>()
            .map(|p| p.config.name.clone())
            .unwrap_or_default();
        format!("{project} · {}", renzora::version::ENGINE_VERSION)
    });
    commands.entity(titles).add_children(&[title, sub]);

    // The bare ✕ became a labelled Cancel. An icon alone says "close"; next to a
    // primary action the word is what makes it read as the other half of a
    // decision.
    let close = commands
        .spawn((
            Node {
                height: Val::Px(32.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(14.0), Val::Px(0.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(rgb(border())),
            UiInteraction::default(),
            CloseBtn,
            cursor(),
        ))
        .id();
    bind_bg(commands, close, move |w| {
        if matches!(w.get::<UiInteraction>(close), Some(UiInteraction::Hovered)) { ca(255, 255, 255, 12) } else { Color::NONE }
    });
    let ct = commands
        .spawn((Text::new(renzora::lang::t("common.cancel")), ui_font(&fonts.ui, 12.5), TextColor(rgb(text_primary())), PASS_THROUGH))
        .id();
    commands.entity(close).add_child(ct);

    let export = build_export_btn(commands, fonts);
    commands.entity(header).add_children(&[badge, titles, close, export]);
    header
}

/// The primary action.
///
/// Blue rather than the theme accent. Every other accent-coloured control in the
/// dialog is a selected tab or a checked switch, so the one button that actually
/// starts a build looked like more of the same.
///
/// It names its target ("Export Web") because the platform is chosen in a
/// sidebar that can scroll out of view, and it carries the count of blocking
/// checks when it is greyed out, so the reason is one glance to the right
/// rather than a hunt through five tabs.
fn build_export_btn(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let btn = commands
        .spawn((
            Node {
                min_width: Val::Px(110.0),
                height: Val::Px(32.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(7.0),
                padding: UiRect::axes(Val::Px(14.0), Val::Px(0.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(EXPORT_BLUE)),
            UiInteraction::default(),
            ExportBtn,
            cursor(),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        if can_export(w) {
            let hot = matches!(
                w.get::<UiInteraction>(btn),
                Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed)
            );
            rgb(if hot { EXPORT_BLUE_HOT } else { EXPORT_BLUE })
        } else {
            rgb(section_bg())
        }
    });
    let ic = icon_text(commands, &fonts.phosphor, "rocket-launch", (255, 255, 255), 14.0);
    commands.entity(ic).insert(PASS_THROUGH);
    let t = commands
        .spawn((Text::new(renzora::lang::t("common.export")), ui_font(&fonts.ui, 13.0), TextColor(Color::WHITE), PASS_THROUGH))
        .id();
    bind_text(commands, t, |w| {
        let base = renzora::lang::t("common.export");
        match w.get_resource::<ExportOverlayState>() {
            Some(s) if s.active_preset.is_some() => format!("{base} {}", platform_short(s.platform)),
            _ => base,
        }
    });
    let count = commands
        .spawn((
            Node { padding: UiRect::axes(Val::Px(7.0), Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(9.0)), ..default() },
            BackgroundColor(ca(RED.0, RED.1, RED.2, 70)),
            PASS_THROUGH,
        ))
        .id();
    let ctext = commands
        .spawn((Text::new(String::new()), ui_font(&fonts.ui, 10.5), TextColor(Color::WHITE), PASS_THROUGH))
        .id();
    commands.entity(count).add_child(ctext);
    bind_text(commands, ctext, |w| blocking_count(w).to_string());
    bind_display(commands, count, |w| {
        w.get_resource::<ExportOverlayState>().is_some_and(|s| s.active_preset.is_some() && s.view == ExportView::Settings)
            && blocking_count(w) > 0
    });
    commands.entity(btn).add_children(&[ic, t, count]);
    btn
}

/// Whether the Export button does anything, and, through `bind_bg`, whether it
/// looks like it will.
///
/// The conditions themselves live in [`preflight`](super::summary::preflight),
/// which the summary rail draws: a lean export needs a platform it can build for
/// and the engine source (it never opens a template), a copy-based one needs the
/// installed template, and every export needs a folder. Asking the same function
/// the rail lists is what keeps a grey button from ever disagreeing with the
/// reasons shown beside it.
pub(super) fn can_export(w: &Rx) -> bool {
    let Some(s) = w.get_resource::<ExportOverlayState>() else { return false };
    s.active_preset.is_some()
        && s.active_task.is_none()
        && matches!(s.progress, ExportProgress::Idle | ExportProgress::Done(_) | ExportProgress::Error(_))
        && blocking_count(w) == 0
}
