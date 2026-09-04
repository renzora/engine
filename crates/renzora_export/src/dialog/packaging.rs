//! The **Packaging** tab: how the build is produced and packed — the packaging
//! mode, the runtime template's status, whether the game ships moddable, and the
//! compression / mesh-optimisation settings.

use bevy::prelude::*;

use renzora_ember::font::EmberFonts;
use renzora_ember::reactive::tracked::{bind_2way, bind_display, bind_text};
use renzora_ember::theme::*;
use renzora_ember::widgets::{drag_value, radio_group, section};

use crate::download::DownloadProgress;
use crate::overlay::{ExportOverlayState, PackagingMode};
use crate::templates::{Platform, TemplateManager};

use super::settings::{finish_tab, lean_source_available, tab_panel};
use super::widgets::{check_state, icon_msg, labeled, pill_button, small_button, switch_control, txt};
use super::{DownloadBtn, InstallBtn, SourceDownloadBtn, AMBER};

pub(super) fn build_packaging_tab(commands: &mut Commands, fonts: &EmberFonts, p: Platform, desktop: bool, host: bool, tab_max: f32) -> Entity {
    let panel = tab_panel(commands);
    let mut secs = Vec::new();

    // The web gets a packaging choice too, but only two of the three modes mean
    // anything there: "Binary + .rpak" and "Single executable" are the same zip
    // either way, since a wasm module has nothing to append an rpak to. So it
    // offers prebuilt-template versus lean-recompile, mapped onto the same enum —
    // `SeparateFiles` is the template path the web has always taken.
    let web = matches!(p, Platform::WebWasm32);

    // Packaging mode. The lean mode recompiles from source, so it appears only
    // where a lean build can actually be produced (`host` — see its definition,
    // which is about lean-capability, not about being the local machine).
    if desktop || web {
        let (sec, body) = section(commands, fonts, "file-archive", &renzora::lang::t("export.section.packaging_mode"), accent());
        let separate = renzora::lang::t("export.packaging.separate");
        let single = renzora::lang::t("export.packaging.single_exe");
        let lean = renzora::lang::t("export.packaging.lean");
        let web_template = renzora::lang::t("export.packaging.web_template");
        let web_lean = renzora::lang::t("export.packaging.web_lean");
        let labels: Vec<&str> = match (web, host) {
            (true, true) => vec![web_template.as_str(), web_lean.as_str()],
            // No engine source: the template is the only thing left, and the
            // "Download engine source" button below says how to get the other.
            (true, false) => vec![web_template.as_str()],
            (false, true) => vec![separate.as_str(), single.as_str(), lean.as_str()],
            (false, false) => vec![separate.as_str(), single.as_str()],
        };
        let radios = radio_group(commands, &fonts.ui, &labels, 0);
        bind_2way(
            commands,
            radios,
            move |w| match w.resource::<ExportOverlayState>().packaging_mode {
                PackagingMode::SeparateFiles => 0usize,
                // The web's radio has no middle option, so `SingleBinary` — which
                // it can arrive at from a preset or a platform switch — reads back
                // as the template, which is what it does there anyway.
                PackagingMode::SingleBinary => if web { 0 } else { 1 },
                PackagingMode::LeanSingleBinary => if web { 1 } else { 2 },
            },
            move |w, v: &usize| {
                let mode = if web {
                    match *v {
                        1 => PackagingMode::LeanSingleBinary,
                        _ => PackagingMode::SeparateFiles,
                    }
                } else {
                    match *v {
                        2 => PackagingMode::LeanSingleBinary,
                        1 => PackagingMode::SingleBinary,
                        _ => PackagingMode::SeparateFiles,
                    }
                };
                w.resource_mut::<ExportOverlayState>().packaging_mode = mode;
            },
        );
        commands.entity(body).add_child(radios);
        // Which mode to actually ship. Said here rather than left implicit,
        // because the copy-based modes are the fast ones and therefore the ones a
        // person reaches for by habit — while what they produce is the editor's
        // own runtime and its dylibs, not a build made for this game.
        let guidance_key = if web { "export.packaging.web_guidance" } else { "export.packaging.guidance" };
        let guidance = txt(commands, fonts, &renzora::lang::t(guidance_key), 11.0, text_muted());
        commands.entity(body).add_child(guidance);
        if host {
            let hint_key = if web { "export.packaging.web_hint" } else { "export.packaging.lean_hint" };
            let hint = txt(commands, fonts, &renzora::lang::t(hint_key), 11.0, text_muted());
            commands.entity(body).add_child(hint);
        }
        // No source, no lean build — but that is a missing download rather than
        // a permanent limitation, so offer the download instead of leaving the
        // option greyed out with no way forward. A canonical editor ships
        // binaries only; the source rides the release as its own asset.
        if !lean_source_available() {
            let why = txt(commands, fonts, &renzora::lang::t("export.packaging.needs_source"), 11.0, AMBER);
            commands.entity(body).add_child(why);
            let btn = small_button(commands, fonts, "download-simple", &renzora::lang::t("export.packaging.get_source"), SourceDownloadBtn);
            commands.entity(btn).insert(Node { width: Val::Px(190.0), height: Val::Px(26.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::Center, column_gap: Val::Px(5.0), margin: UiRect::top(Val::Px(4.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() });
            commands.entity(body).add_child(btn);
        }
        secs.push(sec);
    }

    // Before the runtime status, because it is a decision about the OUTPUT and
    // belongs beside the packaging mode it composes with — not down among the
    // toolchain plumbing.
    if crate::bundle::supported(p) {
        secs.push(build_bundle_section(commands, fonts, p));
    }
    secs.push(build_runtime_status(commands, fonts, p));
    secs.push(build_modding_section(commands, fonts));
    // Compression and mesh optimisation are packaging decisions — how the build
    // is packed, not what it contains — so they sit here rather than behind a
    // tab of their own.
    secs.extend(compression_sections(commands, fonts));
    finish_tab(commands, panel, &secs, tab_max);
    panel
}

/// Whether the exported game ships the plugin SDK, and can therefore compile
/// plugins a player adds.
///
/// On by default. A moddable game is the norm for this engine — the plugin
/// system is the same one the editor uses — and the cost of a wrong default
/// points one way: shipped without it a game cannot be modded at all, shipped
/// with it a game is merely larger.
fn build_modding_section(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (sec, body) = section(commands, fonts, "puzzle-piece", &renzora::lang::t("export.section.modding"), accent());

    let row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).id();
    let cb = switch_control(commands, true);
    bind_2way(
        commands,
        cb,
        |w| w.get_resource::<ExportOverlayState>().is_some_and(|s| s.enable_modding),
        |w, v: &bool| {
            if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() {
                s.enable_modding = *v;
            }
        },
    );
    let label = txt(commands, fonts, &renzora::lang::t("export.modding.enable"), 12.5, text_primary());
    commands.entity(row).add_children(&[cb, label]);
    commands.entity(body).add_child(row);

    let hint = txt(commands, fonts, &renzora::lang::t("export.modding.hint"), 11.0, text_muted());
    commands.entity(body).add_child(hint);

    // A lean build links Bevy statically and shares no image, so there is
    // nothing for a plugin library to bind to — the SDK would ship and be
    // unusable. Said rather than silently ignored, since the checkbox is on by
    // default and a user picking lean would otherwise expect it to apply.
    let note = txt(commands, fonts, &renzora::lang::t("export.modding.lean_note"), 11.0, AMBER);
    bind_display(commands, note, |w| {
        w.get_resource::<ExportOverlayState>()
            .is_some_and(|s| s.packaging_mode == PackagingMode::LeanSingleBinary)
    });
    commands.entity(body).add_child(note);

    sec
}

/// Application-bundle section: wrap the export in an `.AppImage` or a `.app`.
///
/// Shown only where the platform has such a format — Windows ships a folder and
/// the web ships a directory a server points at, so a switch there would be a
/// control with one meaningful setting.
///
/// Off by default. Rearranging the output into a directory the user did not ask
/// for is a worse surprise than an unticked box, and the unbundled tree is what
/// every export has produced until now.
fn build_bundle_section(commands: &mut Commands, fonts: &EmberFonts, p: Platform) -> Entity {
    let mac = matches!(p, Platform::MacOSX64 | Platform::MacOSArm64);
    let title = if mac { "export.section.app_bundle" } else { "export.section.appimage" };
    let (sec, body) = section(commands, fonts, "package", &renzora::lang::t(title), accent());

    let row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).id();
    let cb = switch_control(commands, false);
    bind_2way(
        commands,
        cb,
        |w| w.get_resource::<ExportOverlayState>().is_some_and(|s| s.bundle_app),
        |w, v: &bool| {
            if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() {
                s.bundle_app = *v;
            }
        },
    );
    let label = renzora::lang::t(if mac { "export.bundle.enable_app" } else { "export.bundle.enable_appimage" });
    let label = txt(commands, fonts, &label, 12.5, text_primary());
    commands.entity(row).add_children(&[cb, label]);
    commands.entity(body).add_child(row);

    let hint = renzora::lang::t(if mac { "export.bundle.hint_app" } else { "export.bundle.hint_appimage" });
    let hint = txt(commands, fonts, &hint, 11.0, text_muted());
    commands.entity(body).add_child(hint);
    sec
}

/// Runtime-template status section (installed line + Download/Install buttons +
/// download progress). Returns the section root for the caller to place.
fn build_runtime_status(commands: &mut Commands, fonts: &EmberFonts, p: Platform) -> Entity {
    let (sec, body) = section(commands, fonts, "download-simple", &renzora::lang::t("export.section.runtime_template"), accent());
    // Installed / not status line.
    let (line, msg) = icon_msg(commands, fonts, "check-circle", text_muted());
    bind_text(commands, msg, move |w| if w.get_resource::<TemplateManager>().is_some_and(|t| t.is_installed(p)) { renzora::lang::t("export.runtime.installed") } else { renzora::lang::t("export.runtime.not_installed") });
    commands.entity(body).add_child(line);
    // A lean export recompiles the engine and never opens the template, so a
    // missing one is not a problem to solve — said here because "not installed"
    // sitting above a Download button reads as exactly that, and the button
    // would fetch several hundred MB the build then ignores.
    let unused = txt(commands, fonts, &renzora::lang::t("export.runtime.lean_unused"), 11.0, text_muted());
    bind_display(commands, unused, |w| {
        w.get_resource::<ExportOverlayState>()
            .is_some_and(|s| s.packaging_mode == PackagingMode::LeanSingleBinary)
    });
    commands.entity(body).add_child(unused);
    // Buttons.
    let btns = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
    let dl = pill_button(commands, fonts, "download-simple", &renzora::lang::t("export.btn.download_github"));
    commands.entity(dl).insert(DownloadBtn);
    let inst = pill_button(commands, fonts, "folder-open", &renzora::lang::t("export.btn.install_from_file"));
    commands.entity(inst).insert(InstallBtn);
    commands.entity(btns).add_children(&[dl, inst]);
    commands.entity(body).add_child(btns);
    // Download progress.
    let (prog, pmsg) = icon_msg(commands, fonts, "spinner", text_muted());
    bind_text(commands, pmsg, move |w| match w.get_resource::<ExportOverlayState>().and_then(|s| s.download_status.clone()) {
        Some((dp, DownloadProgress::Fetching(m))) if dp == p => m,
        Some((dp, DownloadProgress::Done(m))) if dp == p => m,
        Some((dp, DownloadProgress::Error(m))) if dp == p => m,
        _ => String::new(),
    });
    bind_display(commands, prog, move |w| w.get_resource::<ExportOverlayState>().and_then(|s| s.download_status.as_ref().map(|(dp, _)| *dp == p)).unwrap_or(false));
    commands.entity(body).add_child(prog);
    sec
}

/// Compression + mesh-optimisation sections.
///
/// Returns sections rather than a tab: both are decisions about how the build is
/// packed, so they live under Packaging rather than behind a tab of their own.
fn compression_sections(commands: &mut Commands, fonts: &EmberFonts) -> Vec<Entity> {
    // Asset compression level.
    let (csec, cbody) = section(commands, fonts, "file-archive", &renzora::lang::t("export.section.compression"), accent());
    let crow = labeled(commands, fonts, &renzora::lang::t("export.field.compression_level"));
    let dv = drag_value(commands, &fonts.ui, "", text_primary(), 0.0, 1.0);
    bind_2way(commands, dv, |w| w.resource::<ExportOverlayState>().compression_level as f32, |w, v: &f32| w.resource_mut::<ExportOverlayState>().compression_level = (v.round() as i32).clamp(1, 22));
    commands.entity(crow).add_child(dv);
    commands.entity(cbody).add_child(crow);

    // Binary compression (UPX). Sits in the same section as the asset
    // compression level because the two answer one question — how small is the
    // shipped folder — even though one is an rpak setting and the other a
    // post-build pass over the executable.
    let upx = check_state(commands, fonts, &renzora::lang::t("export.compression.upx"), |s| s.upx_compress, |s, v| s.upx_compress = v);
    commands.entity(cbody).add_child(upx);
    let upx_help = txt(commands, fonts, &renzora::lang::t("export.compression.upx_help"), 10.0, text_muted());
    commands.entity(cbody).add_child(upx_help);

    // Mesh optimization.
    let (msec, mbody) = section(commands, fonts, "cube", &renzora::lang::t("export.section.mesh_opt"), accent());
    let simplify = check_state(commands, fonts, &renzora::lang::t("export.mesh.simplify"), |s| s.mesh_simplify, |s, v| s.mesh_simplify = v);
    commands.entity(mbody).add_child(simplify);
    let ratio = labeled(commands, fonts, &renzora::lang::t("export.field.keep_ratio"));
    let dvr = drag_value(commands, &fonts.ui, "", text_primary(), 0.0, 0.01);
    bind_2way(commands, dvr, |w| w.resource::<ExportOverlayState>().mesh_simplify_ratio, |w, v: &f32| w.resource_mut::<ExportOverlayState>().mesh_simplify_ratio = v.clamp(0.1, 1.0));
    commands.entity(ratio).add_child(dvr);
    commands.entity(ratio).insert(Node { margin: UiRect::left(Val::Px(20.0)), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() });
    bind_display(commands, ratio, |w| w.resource::<ExportOverlayState>().mesh_simplify);
    commands.entity(mbody).add_child(ratio);
    let quant = check_state(commands, fonts, &renzora::lang::t("export.mesh.quantize"), |s| s.mesh_quantize, |s, v| s.mesh_quantize = v);
    let lods = check_state(commands, fonts, &renzora::lang::t("export.mesh.generate_lods"), |s| s.mesh_generate_lods, |s, v| s.mesh_generate_lods = v);
    commands.entity(mbody).add_children(&[quant, lods]);
    let levels = labeled(commands, fonts, &renzora::lang::t("export.field.lod_levels"));
    let dvl = drag_value(commands, &fonts.ui, "", text_primary(), 0.0, 1.0);
    bind_2way(commands, dvl, |w| w.resource::<ExportOverlayState>().mesh_lod_levels as f32, |w, v: &f32| w.resource_mut::<ExportOverlayState>().mesh_lod_levels = (v.round() as u32).clamp(1, 5));
    commands.entity(levels).add_child(dvl);
    commands.entity(levels).insert(Node { margin: UiRect::left(Val::Px(20.0)), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() });
    bind_display(commands, levels, |w| w.resource::<ExportOverlayState>().mesh_generate_lods);
    commands.entity(mbody).add_child(levels);
    vec![csec, msec]
}
