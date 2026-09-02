//! The modal itself: the backdrop, the panel, the header with Cancel and Export,
//! and the swap between the settings form and the build log.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_bg, bind_display};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::OverlaySurface;

use crate::overlay::{ExportOverlayState, ExportProgress, ExportView, PackagingMode};
use crate::templates::TemplateManager;

use super::settings::lean_source_available;
use super::widgets::{cursor, fullscreen, icon_title, txt};
use super::{
    CloseBtn, ExportBtn, ExportRoot, RightPane, EXPORT_BLUE, EXPORT_BLUE_HOT, RED,
};

/// The dialog's height as a fraction of the window. Kept beside the `Val::Vh`
/// below — the two have to agree or the tab cap is computed against a dialog of a
/// different size.
pub(super) const MODAL_VH: f32 = 0.78;

pub(super) fn spawn_modal(commands: &mut Commands, fonts: &EmberFonts, has_project: bool) {
    let backdrop = commands
        .spawn((
            fullscreen(),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.63)),
            GlobalZIndex(9300),
            FocusPolicy::Block,
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            OverlaySurface,
            ExportRoot,
            Name::new("export-modal"),
        ))
        .id();
    let panel = commands
        .spawn((
            Node {
                // Wider than the 760 it started at: the plugin picker is a grid
                // of thumbnail cards now, and four columns of artwork plus the
                // 180px preset sidebar does not fit in 760.
                width: Val::Px(980.0),
                // Explicit height, not just a cap. The dialog used to be propped
                // open by a sidebar listing twelve platforms; the preset list
                // replacing it starts EMPTY, so the modal collapsed to the height
                // of whichever tab was showing and jumped every time you switched
                // tab or added a preset. A fixed height keeps the tabs, the log
                // view and the Export button in one place regardless of content —
                // `max_height` keeps it on screen on a short display.
                height: Val::Vh(78.0),
                max_height: Val::Vh(86.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(20.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            BorderColor::all(rgb(border())),
            FocusPolicy::Block,
            Name::new("export-panel"),
        ))
        .id();
    commands.entity(backdrop).add_child(panel);

    // Header: title on the left, then Cancel and Export on the right.
    //
    // Export used to sit alone at the bottom of the form, below both columns.
    // Up here it is beside the only other thing that ends the dialog, so the two
    // outcomes are in one place instead of at opposite corners — and the form
    // between them can grow or scroll without the primary action moving.
    let header = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).id();
    let title = icon_title(commands, fonts, "package", &renzora::lang::t("export.title"));
    commands.entity(title).insert(Node { flex_grow: 1.0, ..default() });

    // The bare ✕ becomes a labelled Cancel that keeps the glyph. An icon alone
    // says "close"; next to a primary action the word is what makes it read as
    // the other half of a decision.
    let close = commands
        .spawn((
            Node {
                height: Val::Px(30.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(0.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            Interaction::default(),
            CloseBtn,
            cursor(),
        ))
        .id();
    let cx = icon_text(commands, &fonts.phosphor, "x", text_muted(), 13.0);
    commands.entity(cx).insert(FocusPolicy::Pass);
    let ct = commands
        .spawn((
            Text::new(renzora::lang::t("common.cancel")),
            ui_font(&fonts.ui, 12.5),
            TextColor(rgb(text_primary())),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(close).add_children(&[cx, ct]);

    let export = build_export_btn(commands, fonts);
    commands.entity(header).add_children(&[title, close, export]);
    commands.entity(panel).add_child(header);
    let sep = commands.spawn((Node { width: Val::Percent(100.0), height: Val::Px(1.0), margin: UiRect::vertical(Val::Px(8.0)), ..default() }, BackgroundColor(rgb(divider())))).id();
    commands.entity(panel).add_child(sep);

    if !has_project {
        let w = txt(commands, fonts, &renzora::lang::t("export.no_project"), 12.0, RED);
        commands.entity(panel).add_child(w);
        return;
    }

    // Settings view — the export form. Hidden once an export starts.
    let settings_view = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), flex_grow: 1.0, min_height: Val::Px(0.0), ..default() })
        .id();
    commands.entity(panel).add_child(settings_view);
    bind_display(commands, settings_view, |w| {
        matches!(w.get_resource::<ExportOverlayState>().map(|s| s.view), Some(ExportView::Settings))
    });

    // Two columns.
    let cols = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, column_gap: Val::Px(16.0), flex_grow: 1.0, min_height: Val::Px(0.0), ..default() }).id();
    let sidebar = super::sidebar::build_sidebar(commands, fonts);
    // The right column is NOT scrolled as a whole — the platform header and tab
    // bar inside it stay fixed; each tab caps and scrolls its own content (see
    // `settings::finish_tab`). That keeps the top chrome put while a long list
    // scrolls.
    let right = commands.spawn((Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), min_width: Val::Px(0.0), min_height: Val::Px(0.0), ..default() }, RightPane { sig: None })).id();
    // Every setting in the right pane belongs to the selected preset, so with
    // nothing selected there is nothing to configure. Showing the form anyway
    // invited edits that had nowhere to be saved to — and offered an Export
    // button for a configuration that does not exist.
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

    commands.entity(cols).add_children(&[sidebar, right, right_empty]);
    commands.entity(settings_view).add_child(cols);

    // The Export button used to be built here, in a row below the columns. It
    // lives in the header now, beside Cancel — see above.

    // Log view — the live build terminal + progress bar + cancel. Shown while and
    // after an export runs.
    let log_view = super::log::build_log_view(commands, fonts);
    commands.entity(panel).add_child(log_view);
    bind_display(commands, log_view, |w| {
        matches!(w.get_resource::<ExportOverlayState>().map(|s| s.view), Some(ExportView::Log))
    });
}

/// The primary action, for the header.
///
/// Blue rather than the theme accent. Every other accent-coloured control in the
/// dialog is a selected tab or a checked switch, so the one button that actually
/// starts a build looked like more of the same. A fixed blue makes it the only
/// thing of its colour in the panel, and keeps it recognisable under a theme
/// whose accent happens to be near the background.
///
/// It still greys out when the form is incomplete — [`can_export`] decides, and
/// the button is the only thing that reports it, so the colour has to be bound
/// rather than set once.
fn build_export_btn(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let btn = commands
        .spawn((
            Node {
                min_width: Val::Px(100.0),
                height: Val::Px(30.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(0.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(rgb(EXPORT_BLUE)),
            Interaction::default(),
            ExportBtn,
            cursor(),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        if can_export(w) {
            let hot = matches!(
                w.get::<Interaction>(btn),
                Some(Interaction::Hovered) | Some(Interaction::Pressed)
            );
            rgb(if hot { EXPORT_BLUE_HOT } else { EXPORT_BLUE })
        } else {
            rgb(section_bg())
        }
    });
    let ic = icon_text(commands, &fonts.phosphor, "rocket-launch", (255, 255, 255), 14.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let t = commands.spawn((Text::new(renzora::lang::t("common.export")), ui_font(&fonts.ui, 13.0), TextColor(Color::WHITE), FocusPolicy::Pass)).id();
    commands.entity(btn).add_children(&[ic, t]);
    btn
}

/// Whether the Export button does anything — and, through `bind_bg`, whether it
/// looks like it will.
///
/// The interesting half is what counts as "there is a runtime to ship". For the
/// two copy-based modes it is the installed template, because those literally
/// copy it. **A lean export never opens the template** — it recompiles the engine
/// from source — so requiring one there greys out the button for a reason that
/// does not apply, with the Packaging tab's "Runtime template not installed" as
/// the only clue and a Download button that fixes nothing.
///
/// That was only ever theoretical on desktop, where the template for your own
/// platform is the editor's own `dist/` dir and is therefore always installed.
/// The web made it real: a checkout that has never run `cargo renzora wasm` has
/// no web template, and a lean web export has no reason to want one. What a lean
/// build needs instead is a platform it can build for and the engine source,
/// which is what this checks.
pub(super) fn can_export(w: &Rx) -> bool {
    let Some(s) = w.get_resource::<ExportOverlayState>() else { return false };
    let have_runtime = if s.packaging_mode == PackagingMode::LeanSingleBinary {
        crate::docker::lean_supported(s.platform) && lean_source_available()
    } else {
        w.get_resource::<TemplateManager>().is_some_and(|t| t.is_installed(s.platform))
    };
    have_runtime && !s.output_dir.is_empty() && s.active_task.is_none() && matches!(s.progress, ExportProgress::Idle | ExportProgress::Done(_) | ExportProgress::Error(_))
}
