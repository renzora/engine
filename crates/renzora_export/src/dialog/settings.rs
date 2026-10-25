//! The middle pane: the platform header, the five category tabs, and the sizing
//! rule every tab's scroll viewport depends on.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::reactive::tracked::{bind_display, bind_text, bind_text_color};
use renzora_ember::theme::*;
use renzora_ember::widgets::scroll_area;
use renzora_ember::UiInteraction;

use crate::overlay::{ExportOverlayState, PackagingMode};
use crate::templates::Platform;

use super::frame::MODAL_VH;
use super::widgets::{ca, cursor, hairline, icon_title, txt};
use super::{ExportDialogUi, ExportRoot, ExportTab, RightPane, TabBtn};

pub(super) fn rebuild_right_pane(world: &mut World) {
    if world.query_filtered::<(), With<ExportRoot>>().iter(world).next().is_none() {
        return;
    }
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
    let platform = world.resource::<ExportOverlayState>().platform;
    let sig = Platform::ALL.iter().position(|p| *p == platform).unwrap_or(0) as u8;

    let mut q = world.query::<(Entity, &RightPane)>();
    let Some((pane, old)) = q.iter(world).map(|(e, r)| (e, r.sig)).next() else { return };
    if old == Some(sig) {
        return;
    }
    let kids: Vec<Entity> = world.get::<Children>(pane).map(|c| c.iter().collect()).unwrap_or_default();
    // Measured here rather than inside the tab builders, which have `Commands`
    // and no way to reach a `Window`. Logical (not physical) pixels, because the
    // `Val::Px` cap it feeds is logical too: using the raw resolution would
    // over-size the cap by the DPI scale factor on a scaled display.
    let window_height = world
        .query_filtered::<&bevy::window::Window, With<bevy::window::PrimaryWindow>>()
        .iter(world)
        .next()
        .map(|w| w.resolution.height() / w.resolution.scale_factor())
        .unwrap_or(0.0);
    let tab_max = tab_content_max(window_height);
    let mut queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        for k in kids {
            commands.entity(k).despawn();
        }
        build_settings(&mut commands, &fonts, pane, platform, tab_max);
    }
    queue.apply(world);
    if let Some(mut r) = world.get_mut::<RightPane>(pane) {
        r.sig = Some(sig);
    }
}

fn build_settings(commands: &mut Commands, fonts: &EmberFonts, pane: Entity, p: Platform, tab_max: f32) {
    let desktop = matches!(p, Platform::WindowsX64 | Platform::LinuxX64 | Platform::MacOSX64 | Platform::MacOSArm64);
    // Not "is this the host?" but "can a lean binary be produced for this
    // platform at all?" Everything downstream (the lean option, engine-feature
    // stripping, linking plugins in) depends on a lean build existing, not on
    // it being local.
    //
    // Two conditions, and the source one is easy to forget. A lean build
    // RECOMPILES the engine, so it needs the engine source, which a canonical
    // editor download does not have. Offering the option there just moves the
    // failure from "not offered" to a runtime error after the asset scan.
    let host = super::summary::lean_possible(p);

    // Platform header: name and the devices it covers, on one line.
    let hdr = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(10.0), flex_wrap: FlexWrap::Wrap, ..default() })
        .id();
    let title = icon_title(commands, fonts, super::widgets::platform_icon(p), p.display_name());
    let sub = txt(commands, fonts, p.supported_devices(), 11.0, text_muted());
    commands.entity(hdr).add_children(&[title, sub]);
    commands.entity(pane).add_child(hdr);

    // Five tabs for three decisions: what am I making (Output), how is it built
    // (Packaging), what goes in (features, plugins, files).
    let panels = [
        (ExportTab::Output, super::output::build_output_tab(commands, fonts, p, desktop, tab_max)),
        (ExportTab::Packaging, super::packaging::build_packaging_tab(commands, fonts, p, desktop, host, tab_max)),
        (ExportTab::Features, super::features::build_features_tab(commands, fonts, host, tab_max)),
        (ExportTab::Plugins, super::plugins::build_plugins_tab(commands, fonts, p, host, tab_max)),
        (ExportTab::Files, super::files::build_files_tab(commands, fonts, tab_max)),
    ];

    let strip = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(2.0),
                border: UiRect::bottom(Val::Px(1.0)),
                margin: UiRect::bottom(Val::Px(8.0)),
                ..default()
            },
            BorderColor::all(hairline()),
        ))
        .id();
    for (tab, _) in &panels {
        let handle = tab_handle(commands, fonts, *tab);
        commands.entity(strip).add_child(handle);
    }
    commands.entity(pane).add_child(strip);

    // The dialog owns which tab is open (`ExportDialogUi::tab`) rather than the
    // ember tab widget, so the summary rail's "Review" buttons can switch to
    // the tab a check is about.
    for (tab, panel) in panels {
        // Hidden until the binding says otherwise. Starting visible would show
        // the Files panel for a frame, and `resolve_included_files` reads the
        // whole project the moment it sees that panel on screen.
        commands.entity(panel).insert(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), display: Display::None, ..default() });
        bind_display(commands, panel, move |w| w.get_resource::<ExportDialogUi>().is_some_and(|u| u.tab == tab));
        commands.entity(pane).add_child(panel);
    }
}

fn tab_label(tab: ExportTab) -> String {
    renzora::lang::t(match tab {
        ExportTab::Output => "export.tab.output",
        ExportTab::Packaging => "export.tab.packaging",
        ExportTab::Features => "export.tab.features",
        ExportTab::Plugins => "export.tab.plugins",
        ExportTab::Files => "export.tab.files",
    })
}

/// The count shown beside a tab's name, if it has one: how many features are
/// on, how many plugins and files will ship. These are the three tabs that
/// decide what goes into the build, and their counts make "how much is in it"
/// readable without opening any of them.
fn tab_count(w: &renzora_ember::reactive::Rx, tab: ExportTab) -> Option<String> {
    let s = w.get_resource::<ExportOverlayState>()?;
    match tab {
        ExportTab::Features => Some(if s.packaging_mode == PackagingMode::LeanSingleBinary {
            crate::capabilities::CAPABILITIES
                .iter()
                .filter(|c| s.capabilities.get(c.id).copied().unwrap_or(false))
                .count()
                .to_string()
        } else {
            renzora::lang::t("export.tab.count_all")
        }),
        ExportTab::Plugins => Some(s.selected_plugins.len().to_string()),
        ExportTab::Files => s.included_files.as_ref().map(|f| f.len().to_string()),
        _ => None,
    }
}

fn tab_handle(commands: &mut Commands, fonts: &EmberFonts, tab: ExportTab) -> Entity {
    let handle = commands
        .spawn((
            Node {
                height: Val::Px(34.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(7.0),
                padding: UiRect::horizontal(Val::Px(11.0)),
                border: UiRect::bottom(Val::Px(2.0)),
                margin: UiRect::bottom(Val::Px(-1.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::NONE),
            UiInteraction::default(),
            TabBtn(tab),
            cursor(),
        ))
        .id();
    super::widgets::bind_border(commands, handle, move |w| {
        if w.get_resource::<ExportDialogUi>().is_some_and(|u| u.tab == tab) { rgb(accent()) } else { Color::NONE }
    });
    let label = commands
        .spawn((Text::new(tab_label(tab)), ui_font(&fonts.ui, 12.5), TextColor(rgb(text_muted())), PASS_THROUGH))
        .id();
    bind_text_color(commands, label, move |w| {
        let active = w.get_resource::<ExportDialogUi>().is_some_and(|u| u.tab == tab);
        let hot = matches!(w.get::<UiInteraction>(handle), Some(UiInteraction::Hovered));
        if active || hot { rgb(text_primary()) } else { rgb(text_muted()) }
    });
    let count = commands
        .spawn((
            Node { padding: UiRect::axes(Val::Px(6.0), Val::Px(0.0)), border_radius: BorderRadius::all(Val::Px(8.0)), ..default() },
            BackgroundColor(ca(255, 255, 255, 16)),
            PASS_THROUGH,
        ))
        .id();
    let ct = commands
        .spawn((Text::new(String::new()), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_muted())), PASS_THROUGH))
        .id();
    bind_text(commands, ct, move |w| tab_count(w, tab).unwrap_or_default());
    bind_display(commands, count, move |w| tab_count(w, tab).is_some());
    commands.entity(count).add_child(ct);
    commands.entity(handle).add_children(&[label, count]);
    handle
}

/// A placeholder container for one tab. Its `Node` is finalized in
/// `build_settings`.
pub(super) fn tab_panel(commands: &mut Commands) -> Entity {
    commands.spawn(Node::default()).id()
}

/// Is there an engine source checkout for a lean build to recompile?
///
/// A few `is_file`/`is_dir` calls up a short path; cheap enough not to cache.
pub(super) fn lean_source_available() -> bool {
    crate::build::resolve_engine_source().is_some()
}

/// Max height a single tab's content scrolls within. The platform header and tab
/// bar live above the panels and stay fixed; only this inner content scrolls.
///
/// A `max_height` rather than a flex fill, and that is the load-bearing detail:
/// `scroll_area` gives the viewport a DEFINITE height, which is what lets it
/// clip and what makes the scrollbar appear. Filling by flex instead was tried
/// and does not work here: the panel is several flex levels below the dialog
/// and never gets a definite height of its own, so `height: 100%` grew to the
/// content (clipped by the dialog, no bar) and `flex_basis: 0` collapsed to
/// nothing (blank tab).
///
/// Derived from the window rather than a hardcoded height, because the dialog
/// is a fixed fraction of the window: a constant cap left a band of dead space
/// under a short tab.
fn tab_content_max(window_height: f32) -> f32 {
    // The dialog's fixed chrome: header row, platform header, tab bar, padding.
    const CHROME: f32 = 150.0;
    // No window to measure (headless, or before one exists): a height known to
    // be safe on any display. Also the floor, so a very short window cannot
    // produce a cap so small the tab is unusable: it scrolls instead.
    const FALLBACK: f32 = 380.0;
    if window_height <= 0.0 {
        return FALLBACK;
    }
    (window_height * MODAL_VH - CHROME).max(FALLBACK)
}

/// Finish a tab: stack its `sections` in a column and wrap that in one capped
/// scroll viewport, so the content scrolls under the fixed header and tab bar.
pub(super) fn finish_tab(commands: &mut Commands, panel: Entity, sections: &[Entity], tab_max: f32) {
    let content = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), padding: UiRect { right: Val::Px(10.0), bottom: Val::Px(16.0), ..default() }, ..default() })
        .id();
    commands.entity(content).add_children(sections);
    let scroll = scroll_area(commands, content, tab_max);
    commands.entity(panel).add_child(scroll);
}
