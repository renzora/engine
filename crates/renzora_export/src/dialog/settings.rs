//! The right pane: the platform header, the five category tabs, and the sizing
//! rule every tab's scroll viewport depends on.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use renzora_ember::font::EmberFonts;
use renzora_ember::theme::text_muted;
use renzora_ember::widgets::{scroll_area, tabs};

use crate::overlay::ExportOverlayState;
use crate::templates::Platform;

use super::frame::MODAL_VH;
use super::widgets::{icon_title, platform_icon, txt};
use super::{ExportRoot, RightPane};

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
    // `Val::Px` cap it feeds is logical too — using the raw resolution would
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
    // Not "is this the host?" any more, but "can a lean binary be produced for
    // this platform at all?" Everything downstream (the lean radio option,
    // engine-feature stripping, linking plugins in) depends on a lean build
    // existing, not on it being local.
    //
    // Two conditions, and the source one is easy to forget. A lean build
    // RECOMPILES the engine, so it needs the engine source — which a canonical
    // editor download does not have and cannot fetch (releases publish
    // templates, not source). Offering the option there just moves the failure
    // from "greyed out" to a runtime error after the asset scan. The copy-based
    // modes are unaffected: they copy a prebuilt runtime template and are the
    // normal path for anyone without a checkout.
    let host = crate::docker::lean_supported(p) && lean_source_available();

    // Platform header — context above the category tabs.
    let hdr = commands.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), margin: UiRect::bottom(Val::Px(6.0)), ..default() }).id();
    let title = icon_title(commands, fonts, platform_icon(p), p.display_name());
    let sub = txt(commands, fonts, p.supported_devices(), 11.0, text_muted());
    commands.entity(hdr).add_children(&[title, sub]);
    commands.entity(pane).add_child(hdr);

    // The category tabs. Each builder returns a panel container; `tabs()` shows
    // one at a time (the ember tab widget the editor uses elsewhere). Within a
    // panel, each group is an ember collapsible `section` — the same widget the
    // inspector/settings panels use for their categories.
    //
    // Five tabs, not seven. Compression folded into Packaging and Options into
    // Output, because both were a tab holding one idea: seven tabs for what is
    // really three decisions — what am I making, how is it built, what goes in —
    // meant hunting for a setting rather than reading down a page.
    let panels = vec![
        super::output::build_output_tab(commands, fonts, p, desktop, tab_max),
        super::packaging::build_packaging_tab(commands, fonts, p, desktop, host, tab_max),
        super::features::build_features_tab(commands, fonts, host, tab_max),
        super::plugins::build_plugins_tab(commands, fonts, p, host, tab_max),
        super::files::build_files_tab(commands, fonts, tab_max),
    ];
    let tab_labels = [
        renzora::lang::t("export.tab.output"),
        renzora::lang::t("export.tab.packaging"),
        renzora::lang::t("export.tab.features"),
        renzora::lang::t("export.tab.plugins"),
        renzora::lang::t("export.tab.files"),
    ];
    let tab_refs: Vec<&str> = tab_labels.iter().map(|s| s.as_str()).collect();
    let strip = tabs(
        commands,
        &fonts.ui,
        &tab_refs,
        panels.clone(),
    );
    // `tabs()` overwrites each panel's `Node` with `default() + display`, so a
    // column layout has to be re-applied here — preserving the initial
    // visibility it set (only panel 0 shown). `tab_select` later toggles only
    // the `display` field, leaving these other fields intact.
    for (i, &panel) in panels.iter().enumerate() {
        commands.entity(panel).insert(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            display: if i == 0 { Display::Flex } else { Display::None },
            ..default()
        });
    }
    commands.entity(pane).add_child(strip);
}

/// A placeholder container for one tab. Its `Node` is finalized in
/// `build_settings` after `tabs()` (which clobbers whatever we set here).
pub(super) fn tab_panel(commands: &mut Commands) -> Entity {
    commands.spawn(Node::default()).id()
}

/// Is there an engine source checkout for a lean build to recompile?
///
/// A few `is_file`/`is_dir` calls up a short path, and only on a right-pane
/// rebuild (platform change), so it is not worth caching.
pub(super) fn lean_source_available() -> bool {
    crate::build::resolve_engine_source().is_some()
}

/// Max height a single tab's content scrolls within. The platform header + tab
/// bar live above the panels and stay fixed; only this inner content scrolls.
///
/// A `max_height` rather than a flex fill, and that is the load-bearing detail:
/// `scroll_area` gives the viewport a DEFINITE height, which is what lets it
/// clip and what makes the scrollbar appear (the bar is driven by the viewport's
/// own overflow). Filling by flex instead was tried and does not work here —
/// the panel is five flex levels below the dialog and never gets a definite
/// height of its own, so `height: 100%` grew to the content (clipped by the
/// dialog, no bar) and `flex_basis: 0` collapsed to nothing (blank tab).
///
/// Derived from the window rather than the old hardcoded 380px, because the
/// dialog is now a fixed 78vh: a constant cap left a band of dead space between
/// a short tab and the Export button. The subtraction is the dialog's fixed
/// chrome — title, separator, platform header, tab bar, Export row, padding.
fn tab_content_max(window_height: f32) -> f32 {
    // The dialog's fixed chrome: title row, separator, platform header, tab bar,
    // Export row, panel padding.
    const CHROME: f32 = 230.0;
    // No window to measure (headless, or before one exists): the constant this
    // replaced, which is known to be safe on any display rather than merely
    // likely. Also the floor, so a very short window cannot produce a cap so
    // small the tab is unusable — it scrolls instead.
    const FALLBACK: f32 = 380.0;
    if window_height <= 0.0 {
        return FALLBACK;
    }
    (window_height * MODAL_VH - CHROME).max(FALLBACK)
}

/// Finish a tab: stack its `sections` in a column and wrap that in one capped
/// scroll viewport, so the content scrolls under the fixed header/tab bar
/// (sizes to content when short, scrolls past `tab_max`).
pub(super) fn finish_tab(commands: &mut Commands, panel: Entity, sections: &[Entity], tab_max: f32) {
    let content = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() })
        .id();
    commands.entity(content).add_children(sections);
    let scroll = scroll_area(commands, content, tab_max);
    commands.entity(panel).add_child(scroll);
}
