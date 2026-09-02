//! The **Output** tab: what is being made — binary name, folder, icon, and the
//! window / logging / server flags that describe the artefact.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora::core::WindowMode;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_2way, bind_display, bind_text};
use renzora_ember::theme::*;
use renzora_ember::widgets::{bind_text_input, drag_value, radio_group, section, text_input};

use crate::overlay::ExportOverlayState;
use crate::templates::Platform;

use super::settings::{finish_tab, tab_panel};
use super::widgets::{check_state, cursor, labeled, pill_button, style_input, txt};
use super::{IconBrowseBtn, IconClearBtn, OutputBrowseBtn};

pub(super) fn build_output_tab(commands: &mut Commands, fonts: &EmberFonts, p: Platform, desktop: bool, tab_max: f32) -> Entity {
    let panel = tab_panel(commands);
    let (sec, body) = section(commands, fonts, "folder-open", &renzora::lang::t("export.section.output"), accent());

    // Binary name. (Empty initial value; `bind_text_input` reflects the current
    // state in on the first frame, so no `Init` snapshot is needed here.)
    let name_row = labeled(commands, fonts, &renzora::lang::t("export.field.name"));
    let name = text_input(commands, &fonts.ui, &renzora::lang::t("export.placeholder.binary_name"), "");
    style_input(commands, name);
    bind_text_input(commands, name, |w| w.get_resource::<ExportOverlayState>().map(|s| s.binary_name.clone()).unwrap_or_default(), |w, v| { if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() { s.binary_name = v; } });
    commands.entity(name_row).add_child(name);

    // Export directory.
    let dir_row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
    let dir_lbl = txt(commands, fonts, &renzora::lang::t("export.field.folder"), 12.0, text_muted());
    let dir = text_input(commands, &fonts.ui, &renzora::lang::t("export.placeholder.output_dir"), "");
    style_input(commands, dir);
    bind_text_input(commands, dir, |w| w.get_resource::<ExportOverlayState>().map(|s| s.output_dir.clone()).unwrap_or_default(), |w, v| { if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() { s.output_dir = v; } });
    let dir_browse = pill_button(commands, fonts, "folder", &renzora::lang::t("export.btn.browse"));
    commands.entity(dir_browse).insert(OutputBrowseBtn);
    commands.entity(dir_row).add_children(&[dir_lbl, dir, dir_browse]);

    // Icon.
    let icon_row = commands.spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), margin: UiRect::top(Val::Px(2.0)), ..default() }, Name::new("icon-row"))).id();
    let icon_lbl = txt(commands, fonts, &renzora::lang::t("export.field.icon"), 12.0, text_muted());
    let icon_path = commands.spawn((Text::new(renzora::lang::t("common.none")), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), Node { flex_grow: 1.0, ..default() }, bevy::text::TextLayout::no_wrap())).id();
    bind_text(commands, icon_path, |w| w.get_resource::<ExportOverlayState>().and_then(|s| s.icon_path.clone()).unwrap_or_else(|| renzora::lang::t("common.none")));
    let clear = commands.spawn((Node { padding: UiRect::all(Val::Px(2.0)), ..default() }, Interaction::default(), IconClearBtn, cursor())).id();
    let clx = icon_text(commands, &fonts.phosphor, "x", text_muted(), 12.0);
    commands.entity(clx).insert(FocusPolicy::Pass);
    commands.entity(clear).add_child(clx);
    bind_display(commands, clear, |w| w.get_resource::<ExportOverlayState>().is_some_and(|s| s.icon_path.is_some()));
    let icon_browse = pill_button(commands, fonts, "image", &renzora::lang::t("export.btn.browse"));
    commands.entity(icon_browse).insert(IconBrowseBtn);
    commands.entity(icon_row).add_children(&[icon_lbl, icon_path, clear, icon_browse]);

    commands.entity(body).add_children(&[name_row, dir_row, icon_row]);
    // Window, logging and dedicated-server options follow the output fields:
    // they all describe the artefact being produced, and splitting them across
    // two tabs meant "what am I making?" was answered in two places.
    let mut secs = vec![sec];
    secs.extend(options_sections(commands, fonts, p, desktop));
    finish_tab(commands, panel, &secs, tab_max);
    panel
}

/// Window / logging / server sections.
///
/// Returns sections rather than a tab: these describe the thing being produced —
/// what window it opens, whether it logs — so they belong with Output.
fn options_sections(commands: &mut Commands, fonts: &EmberFonts, p: Platform, desktop: bool) -> Vec<Entity> {
    let mut secs = Vec::new();

    // Window (desktop).
    if desktop {
        let (wsec, wbody) = section(commands, fonts, "monitor", &renzora::lang::t("export.section.window"), accent());
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
        commands.entity(wbody).add_child(radios);
        let size = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
        let szl = txt(commands, fonts, &renzora::lang::t("export.field.size"), 12.0, text_muted());
        let dw = drag_value(commands, &fonts.ui, "", text_primary(), 0.0, 10.0);
        bind_2way(commands, dw, |w| w.resource::<ExportOverlayState>().window_width as f32, |w, v: &f32| w.resource_mut::<ExportOverlayState>().window_width = (v.round() as u32).clamp(320, 7680));
        let xl = txt(commands, fonts, "x", 12.0, text_muted());
        let dh = drag_value(commands, &fonts.ui, "", text_primary(), 0.0, 10.0);
        bind_2way(commands, dh, |w| w.resource::<ExportOverlayState>().window_height as f32, |w, v: &f32| w.resource_mut::<ExportOverlayState>().window_height = (v.round() as u32).clamp(240, 4320));
        commands.entity(size).add_children(&[szl, dw, xl, dh]);
        bind_display(commands, size, |w| matches!(w.resource::<ExportOverlayState>().window_mode, WindowMode::Windowed));
        commands.entity(wbody).add_child(size);
        secs.push(wsec);
    }

    // Flags.
    let (osec, obody) = section(commands, fonts, "gear", &renzora::lang::t("export.section.options"), accent());
    let console = check_state(commands, fonts, &renzora::lang::t("export.options.console_logging"), |s| s.console_logging, |s, v| s.console_logging = v);
    commands.entity(obody).add_child(console);
    if desktop && p.supports_dedicated_server() {
        // Asking for a dedicated server is asking for networking, so it turns
        // the capability back on rather than shipping a server binary with the
        // transport stripped out of it.
        let server = check_state(commands, fonts, &renzora::lang::t("export.options.include_server"), |s| s.include_server, |s, v| {
            s.include_server = v;
            if v {
                s.capabilities.insert("networking".to_string(), true);
            }
        });
        commands.entity(obody).add_child(server);
    }
    secs.push(osec);
    secs
}
