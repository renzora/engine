//! The **Output** tab: what is being made. Binary name, folder, icon, and the
//! window / logging / server flags that describe the artefact.

use bevy::prelude::*;

use renzora::core::WindowMode;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::reactive::tracked::{bind_2way, bind_display, bind_text};
use renzora_ember::theme::*;
use renzora_ember::widgets::{bind_text_input, drag_value, radio_group, text_input};
use renzora_ember::UiInteraction;

use crate::overlay::ExportOverlayState;
use crate::templates::Platform;

use super::settings::{finish_tab, tab_panel};
use super::widgets::{cursor, group, pill_button, style_input, switch_row, txt};
use super::{IconBrowseBtn, IconClearBtn, OutputBrowseBtn};

/// A form line: a fixed-width label, then the control filling the rest.
///
/// Fixed width so the inputs start at one x down the group, which is what lets
/// the eye run down the fields rather than zig-zag between labels of different
/// lengths.
fn form_row(commands: &mut Commands, fonts: &EmberFonts, label: &str, controls: &[Entity]) -> Entity {
    let row = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), padding: UiRect::vertical(Val::Px(4.0)), ..default() })
        .id();
    let l = txt(commands, fonts, label, 12.0, text_muted());
    commands.entity(l).insert(Node { width: Val::Px(110.0), flex_shrink: 0.0, ..default() });
    commands.entity(row).add_child(l);
    commands.entity(row).add_children(controls);
    row
}

pub(super) fn build_output_tab(commands: &mut Commands, fonts: &EmberFonts, p: Platform, desktop: bool, tab_max: f32) -> Entity {
    let panel = tab_panel(commands);

    // Binary name. (Empty initial value; `bind_text_input` reflects the current
    // state in on the first frame.)
    let name = text_input(commands, &fonts.ui, &renzora::lang::t("export.placeholder.binary_name"), "");
    style_input(commands, name);
    bind_text_input(commands, name, |w| w.get_resource::<ExportOverlayState>().map(|s| s.binary_name.clone()).unwrap_or_default(), |w, v| { if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() { s.binary_name = v; } });
    let name_row = form_row(commands, fonts, &renzora::lang::t("export.field.name"), &[name]);

    // Export directory.
    let dir = text_input(commands, &fonts.ui, &renzora::lang::t("export.placeholder.output_dir"), "");
    style_input(commands, dir);
    bind_text_input(commands, dir, |w| w.get_resource::<ExportOverlayState>().map(|s| s.output_dir.clone()).unwrap_or_default(), |w, v| { if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() { s.output_dir = v; } });
    let dir_browse = pill_button(commands, fonts, "folder", &renzora::lang::t("export.btn.browse"));
    commands.entity(dir_browse).insert(OutputBrowseBtn);
    let dir_row = form_row(commands, fonts, &renzora::lang::t("export.field.folder"), &[dir, dir_browse]);

    let mut rows = vec![name_row, dir_row];

    // Icon. Not on the web, which has no executable to put one on.
    if !matches!(p, Platform::WebWasm32) {
        let icon_path = commands
            .spawn((Text::new(renzora::lang::t("common.none")), ui_font(&fonts.ui, 11.5), TextColor(rgb(text_muted())), Node { flex_grow: 1.0, min_width: Val::Px(0.0), overflow: Overflow::clip(), ..default() }, bevy::text::TextLayout::no_wrap()))
            .id();
        bind_text(commands, icon_path, |w| w.get_resource::<ExportOverlayState>().and_then(|s| s.icon_path.clone()).unwrap_or_else(|| renzora::lang::t("common.none")));
        let clear = commands.spawn((Node { padding: UiRect::all(Val::Px(2.0)), ..default() }, UiInteraction::default(), IconClearBtn, cursor())).id();
        let clx = icon_text(commands, &fonts.phosphor, "x", text_muted(), 12.0);
        commands.entity(clx).insert(PASS_THROUGH);
        commands.entity(clear).add_child(clx);
        bind_display(commands, clear, |w| w.get_resource::<ExportOverlayState>().is_some_and(|s| s.icon_path.is_some()));
        let icon_browse = pill_button(commands, fonts, "image", &renzora::lang::t("export.btn.choose_image"));
        commands.entity(icon_browse).insert(IconBrowseBtn);
        rows.push(form_row(commands, fonts, &renzora::lang::t("export.field.icon"), &[icon_path, clear, icon_browse]));
    }

    let output = group(commands, fonts, &renzora::lang::t("export.section.output"), &rows);
    // Window, logging and dedicated-server options follow the output fields:
    // they all describe the artefact being produced.
    let mut secs = vec![output];
    secs.extend(options_sections(commands, fonts, p, desktop));
    finish_tab(commands, panel, &secs, tab_max);
    panel
}

/// Window and option groups.
fn options_sections(commands: &mut Commands, fonts: &EmberFonts, p: Platform, desktop: bool) -> Vec<Entity> {
    let mut secs = Vec::new();

    if desktop {
        let windowed = renzora::lang::t("export.window.windowed");
        let fullscreen = renzora::lang::t("export.window.fullscreen");
        let borderless = renzora::lang::t("export.window.borderless");
        let radios = radio_group(commands, &fonts.ui, &[windowed.as_str(), fullscreen.as_str(), borderless.as_str()], 0);
        bind_2way(
            commands,
            radios,
            |w| match w.resource::<ExportOverlayState>().window_mode {
                WindowMode::Fullscreen => 1usize,
                WindowMode::Borderless => 2,
                _ => 0,
            },
            |w, v: &usize| w.resource_mut::<ExportOverlayState>().window_mode = match v { 1 => WindowMode::Fullscreen, 2 => WindowMode::Borderless, _ => WindowMode::Windowed },
        );
        let mode_row = form_row(commands, fonts, &renzora::lang::t("export.field.window_mode"), &[radios]);

        let dw = drag_value(commands, &fonts.ui, "", text_primary(), 0.0, 10.0);
        bind_2way(commands, dw, |w| w.resource::<ExportOverlayState>().window_width as f32, |w, v: &f32| w.resource_mut::<ExportOverlayState>().window_width = (v.round() as u32).clamp(320, 7680));
        let xl = txt(commands, fonts, "×", 12.0, text_muted());
        let dh = drag_value(commands, &fonts.ui, "", text_primary(), 0.0, 10.0);
        bind_2way(commands, dh, |w| w.resource::<ExportOverlayState>().window_height as f32, |w, v: &f32| w.resource_mut::<ExportOverlayState>().window_height = (v.round() as u32).clamp(240, 4320));
        let size_row = form_row(commands, fonts, &renzora::lang::t("export.field.size"), &[dw, xl, dh]);
        // A size means nothing to a fullscreen window, so it goes rather than
        // sitting there looking like it applies.
        bind_display(commands, size_row, |w| matches!(w.resource::<ExportOverlayState>().window_mode, WindowMode::Windowed));
        secs.push(group(commands, fonts, &renzora::lang::t("export.section.window"), &[mode_row, size_row]));
    }

    let mut rows = vec![switch_row(
        commands,
        fonts,
        &renzora::lang::t("export.options.console_logging"),
        Some(&renzora::lang::t("export.options.console_logging_hint")),
        |s| s.console_logging,
        |s, v| s.console_logging = v,
    )];
    // A profiling build carries this editor's pairing key, so the companion
    // can find and profile it on a phone or headset.
    if !matches!(p, Platform::WebWasm32) {
        rows.push(switch_row(
            commands,
            fonts,
            &renzora::lang::t("export.options.allow_profiling"),
            Some(&renzora::lang::t("export.options.allow_profiling_hint")),
            |s| s.allow_profiling,
            |s, v| s.allow_profiling = v,
        ));
    }
    if desktop && p.supports_dedicated_server() {
        // Asking for a dedicated server is asking for networking, so it turns
        // the capability back on rather than shipping a server binary with the
        // transport stripped out of it.
        rows.push(switch_row(
            commands,
            fonts,
            &renzora::lang::t("export.options.include_server"),
            Some(&renzora::lang::t("export.options.include_server_hint")),
            |s| s.include_server,
            |s, v| {
                s.include_server = v;
                if v {
                    s.capabilities.insert("networking".to_string(), true);
                }
            },
        ));
    }
    secs.push(group(commands, fonts, &renzora::lang::t("export.section.options"), &rows));
    secs
}
