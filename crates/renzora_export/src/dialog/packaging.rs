//! The **Packaging** tab: how the build is produced and packed. The packaging
//! mode, the runtime template's status, whether the game ships moddable, and the
//! compression / mesh-optimisation settings.

use bevy::prelude::*;

use renzora_ember::font::{icon_text, EmberFonts};
use renzora_ember::reactive::tracked::{bind_2way, bind_display, bind_text, bind_text_color};
use renzora_ember::theme::*;
use renzora_ember::widgets::drag_value;

use crate::download::DownloadProgress;
use crate::overlay::{ExportOverlayState, PackagingMode};
use crate::templates::{Platform, TemplateManager};

use super::settings::{finish_tab, lean_source_available, tab_panel};
use super::widgets::{
    card_row, choice_card, group, hairline, notice, pill_button, setting_row, small_button,
    switch_row, txt,
};
use super::{Choice, DownloadBtn, InstallBtn, SourceDownloadBtn, AMBER, GREEN};

pub(super) fn build_packaging_tab(commands: &mut Commands, fonts: &EmberFonts, p: Platform, desktop: bool, host: bool, tab_max: f32) -> Entity {
    let panel = tab_panel(commands);
    let mut secs = Vec::new();

    // The web gets a packaging choice too, but only two of the three modes mean
    // anything there: "Binary + .rpak" and "Single executable" are the same zip
    // either way, since a wasm module has nothing to append an rpak to. So it
    // offers prebuilt-template versus lean-recompile, mapped onto the same enum:
    // `SeparateFiles` is the template path the web has always taken.
    let web = matches!(p, Platform::WebWasm32);

    if desktop || web {
        secs.push(build_mode_group(commands, fonts, web, host));
    }

    // Before the runtime status, because it is a decision about the OUTPUT and
    // belongs beside the packaging mode it composes with.
    if crate::bundle::supported(p) {
        secs.push(build_bundle_group(commands, fonts, p));
    }
    secs.push(build_runtime_status(commands, fonts, p));
    // A browser cannot load a plugin library, so there is nothing to mod with.
    if !web {
        secs.push(build_modding_group(commands, fonts));
    }
    secs.push(compression_group(commands, fonts, p));
    finish_tab(commands, panel, &secs, tab_max);
    panel
}

/// The packaging mode as option cards, release mode first.
///
/// This was a row of radio buttons followed by two paragraphs explaining which
/// to pick. The answer is the same for nearly everyone (the lean mode for a
/// release, a copy mode while iterating), so each card now carries that answer
/// as a badge and a sentence of its own.
fn build_mode_group(commands: &mut Commands, fonts: &EmberFonts, web: bool, host: bool) -> Entity {
    let release = renzora::lang::t("export.packaging.badge.release");
    let test = renzora::lang::t("export.packaging.badge.test");
    let mut cards = Vec::new();
    if host {
        let (title, desc) = if web {
            ("export.packaging.web_lean", "export.packaging.desc.web_lean")
        } else {
            ("export.packaging.lean", "export.packaging.desc.lean")
        };
        cards.push(choice_card(commands, fonts, &renzora::lang::t(title), Some((&release, true)), &renzora::lang::t(desc), Choice::Packaging(PackagingMode::LeanSingleBinary)));
    }
    if web {
        cards.push(choice_card(commands, fonts, &renzora::lang::t("export.packaging.web_template"), Some((&test, false)), &renzora::lang::t("export.packaging.desc.web_template"), Choice::Packaging(PackagingMode::SeparateFiles)));
    } else {
        cards.push(choice_card(commands, fonts, &renzora::lang::t("export.packaging.single_exe"), Some((&test, false)), &renzora::lang::t("export.packaging.desc.single"), Choice::Packaging(PackagingMode::SingleBinary)));
        cards.push(choice_card(commands, fonts, &renzora::lang::t("export.packaging.separate"), Some((&test, false)), &renzora::lang::t("export.packaging.desc.separate"), Choice::Packaging(PackagingMode::SeparateFiles)));
    }
    let row = card_row(commands, &cards);
    let mut rows = vec![row];

    // One line under the cards, saying what the chosen mode needs or what it
    // is for. Bound, because it changes with the selection.
    let foot = txt(commands, fonts, "", 11.0, text_muted());
    commands.entity(foot).insert(Node { margin: UiRect::top(Val::Px(8.0)), ..default() });
    bind_text(commands, foot, move |w| {
        let lean = w.get_resource::<ExportOverlayState>().is_some_and(|s| s.packaging_mode == PackagingMode::LeanSingleBinary);
        renzora::lang::t(match (lean, web) {
            (true, true) => "export.packaging.web_hint",
            (true, false) => "export.packaging.lean_hint",
            (false, _) => "export.packaging.test_note",
        })
    });
    rows.push(foot);

    // No source, no lean build. That is a missing download rather than a
    // permanent limitation, so offer it instead of only leaving the card out.
    if !lean_source_available() {
        let n = notice(commands, fonts, "info", &renzora::lang::t("export.packaging.needs_source"), AMBER);
        let btn = small_button(commands, fonts, "download-simple", &renzora::lang::t("export.packaging.get_source"), SourceDownloadBtn);
        commands.entity(btn).insert(Node { flex_grow: 0.0, height: Val::Px(26.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::Center, column_gap: Val::Px(5.0), padding: UiRect::horizontal(Val::Px(10.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() });
        commands.entity(n).add_child(btn);
        commands.entity(n).insert(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            padding: UiRect::axes(Val::Px(12.0), Val::Px(9.0)),
            margin: UiRect::top(Val::Px(8.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(8.0)),
            ..default()
        });
        rows.push(n);
    }
    group(commands, fonts, &renzora::lang::t("export.section.packaging_mode"), &rows)
}

/// Whether the exported game ships the plugin SDK, and can therefore compile
/// plugins a player adds.
///
/// Off by default: the SDK is about 1.5 GB, and only a game meant to be modded
/// should carry it.
fn build_modding_group(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = switch_row(
        commands,
        fonts,
        &renzora::lang::t("export.modding.enable"),
        Some(&renzora::lang::t("export.modding.hint")),
        |s| s.enable_modding,
        |s, v| s.enable_modding = v,
    );
    // A lean build links Bevy statically and shares no image, so there is
    // nothing for a plugin library to bind to: the SDK would ship and be
    // unusable. Said rather than silently ignored, since someone who turned the
    // switch on and then picked lean would otherwise expect it to apply.
    let note = txt(commands, fonts, &renzora::lang::t("export.modding.lean_note"), 11.0, AMBER);
    bind_display(commands, note, |w| {
        w.get_resource::<ExportOverlayState>()
            .is_some_and(|s| s.packaging_mode == PackagingMode::LeanSingleBinary)
    });
    group(commands, fonts, &renzora::lang::t("export.section.modding"), &[row, note])
}

/// Application-bundle group: wrap the export in an `.AppImage` or a `.app`.
///
/// Shown only where the platform has such a format. Off by default:
/// rearranging the output into a directory the user did not ask for is a worse
/// surprise than an unticked switch.
fn build_bundle_group(commands: &mut Commands, fonts: &EmberFonts, p: Platform) -> Entity {
    let mac = matches!(p, Platform::MacOSX64 | Platform::MacOSArm64);
    let title = if mac { "export.section.app_bundle" } else { "export.section.appimage" };
    let label = renzora::lang::t(if mac { "export.bundle.enable_app" } else { "export.bundle.enable_appimage" });
    let hint = renzora::lang::t(if mac { "export.bundle.hint_app" } else { "export.bundle.hint_appimage" });
    let row = switch_row(commands, fonts, &label, Some(&hint), |s| s.bundle_app, |s, v| s.bundle_app = v);
    group(commands, fonts, &renzora::lang::t(title), &[row])
}

/// The runtime template as one status row: whether it is installed, and the
/// two ways to get it.
///
/// Hidden for a lean build, which recompiles the engine and never opens the
/// template. It used to stay on screen there saying "not installed" above a
/// Download button that would fetch several hundred MB the build then ignored.
fn build_runtime_status(commands: &mut Commands, fonts: &EmberFonts, p: Platform) -> Entity {
    let card = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(12.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            BorderColor::all(hairline()),
        ))
        .id();
    let installed = move |w: &renzora_ember::reactive::Rx| w.get_resource::<TemplateManager>().is_some_and(|t| t.is_installed(p));
    let ic = icon_text(commands, &fonts.phosphor, "download-simple", text_muted(), 16.0);
    bind_text_color(commands, ic, move |w| rgb(if installed(w) { GREEN } else { AMBER }));
    let text = commands
        .spawn(Node { flex_grow: 1.0, flex_basis: Val::Px(0.0), min_width: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() })
        .id();
    let title = txt(commands, fonts, "", 12.5, text_primary());
    bind_text(commands, title, move |w| {
        renzora::lang::t(if installed(w) { "export.runtime.installed_for" } else { "export.runtime.missing_for" }).replace("{platform}", p.display_name())
    });
    let hint = txt(commands, fonts, "", 11.0, text_muted());
    bind_text(commands, hint, move |w| {
        // The download line takes over the hint while one is running, so its
        // progress shows where the problem was described.
        if let Some((dp, prog)) = w.get_resource::<ExportOverlayState>().and_then(|s| s.download_status.clone()) {
            if dp == p {
                match prog {
                    DownloadProgress::Fetching(m) | DownloadProgress::Done(m) | DownloadProgress::Error(m) => return m,
                }
            }
        }
        renzora::lang::t(if installed(w) { "export.runtime.installed_hint" } else { "export.runtime.missing_hint" })
    });
    commands.entity(text).add_children(&[title, hint]);
    let inst = pill_button(commands, fonts, "folder-open", &renzora::lang::t("export.btn.install_from_file"));
    commands.entity(inst).insert(InstallBtn);
    let dl = pill_button(commands, fonts, "download-simple", &renzora::lang::t("export.btn.download_github"));
    commands.entity(dl).insert(DownloadBtn);
    bind_display(commands, inst, move |w| !installed(w));
    bind_display(commands, dl, move |w| !installed(w));
    commands.entity(card).add_children(&[ic, text, inst, dl]);

    let g = group(commands, fonts, &renzora::lang::t("export.section.runtime_template"), &[card]);
    bind_display(commands, g, |w| {
        w.get_resource::<ExportOverlayState>()
            .is_some_and(|s| s.packaging_mode != PackagingMode::LeanSingleBinary)
    });
    g
}

/// A small indented "value" line under a switch, shown while the switch is on.
fn sub_value(commands: &mut Commands, fonts: &EmberFonts, label: &str, control: Entity, shown: fn(&ExportOverlayState) -> bool) -> Entity {
    let row = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), margin: UiRect { left: Val::Px(16.0), bottom: Val::Px(6.0), ..default() }, ..default() })
        .id();
    let l = txt(commands, fonts, label, 11.5, text_muted());
    commands.entity(row).add_children(&[l, control]);
    bind_display(commands, row, move |w| w.get_resource::<ExportOverlayState>().is_some_and(shown));
    row
}

/// Size and compression: the asset compression level, UPX, and the mesh passes.
///
/// One group, because they answer one question (how small is the shipped
/// folder), even though one is an rpak setting, one a post-build pass over the
/// executable, and the rest happen at asset-pack time.
fn compression_group(commands: &mut Commands, fonts: &EmberFonts, p: Platform) -> Entity {
    let dv = drag_value(commands, &fonts.ui, "", text_primary(), 0.0, 1.0);
    bind_2way(commands, dv, |w| w.resource::<ExportOverlayState>().compression_level as f32, |w, v: &f32| w.resource_mut::<ExportOverlayState>().compression_level = (v.round() as i32).clamp(1, 22));
    let level = setting_row(
        commands,
        fonts,
        &renzora::lang::t("export.compression.level"),
        Some(&renzora::lang::t("export.compression.level_hint")),
        dv,
    );
    let mut rows = vec![level];

    // UPX packs Windows and Linux executables; the other formats either cannot
    // be packed or are not executables at all.
    if matches!(p, Platform::WindowsX64 | Platform::WindowsArm64 | Platform::LinuxX64 | Platform::LinuxArm64) {
        rows.push(switch_row(
            commands,
            fonts,
            &renzora::lang::t("export.compression.upx"),
            Some(&renzora::lang::t("export.compression.upx_help")),
            |s| s.upx_compress,
            |s, v| s.upx_compress = v,
        ));
    }

    rows.push(switch_row(commands, fonts, &renzora::lang::t("export.mesh.simplify"), Some(&renzora::lang::t("export.mesh.simplify_hint")), |s| s.mesh_simplify, |s, v| s.mesh_simplify = v));
    let dvr = drag_value(commands, &fonts.ui, "", text_primary(), 0.0, 0.01);
    bind_2way(commands, dvr, |w| w.resource::<ExportOverlayState>().mesh_simplify_ratio, |w, v: &f32| w.resource_mut::<ExportOverlayState>().mesh_simplify_ratio = v.clamp(0.1, 1.0));
    rows.push(sub_value(commands, fonts, &renzora::lang::t("export.field.keep_ratio"), dvr, |s| s.mesh_simplify));
    rows.push(switch_row(commands, fonts, &renzora::lang::t("export.mesh.quantize"), Some(&renzora::lang::t("export.mesh.quantize_hint")), |s| s.mesh_quantize, |s, v| s.mesh_quantize = v));
    rows.push(switch_row(commands, fonts, &renzora::lang::t("export.mesh.generate_lods"), Some(&renzora::lang::t("export.mesh.lods_hint")), |s| s.mesh_generate_lods, |s, v| s.mesh_generate_lods = v));
    let dvl = drag_value(commands, &fonts.ui, "", text_primary(), 0.0, 1.0);
    bind_2way(commands, dvl, |w| w.resource::<ExportOverlayState>().mesh_lod_levels as f32, |w, v: &f32| w.resource_mut::<ExportOverlayState>().mesh_lod_levels = (v.round() as u32).clamp(1, 5));
    rows.push(sub_value(commands, fonts, &renzora::lang::t("export.field.lod_levels"), dvl, |s| s.mesh_generate_lods));

    group(commands, fonts, &renzora::lang::t("export.section.size"), &rows)
}
