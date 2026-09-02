//! The Editor page — developer switches, auto-save, the graphics backend, and
//! import behaviour, plus the plugin grid it ends with.
//!
//! Three sidebar categories share this page (`general`, `autosave`, `plugins`)
//! and its sections are keyed to them via `focus_hide`.

use bevy::prelude::*;

use renzora_editor_framework::EditorSettings;
use renzora_ember::font::EmberFonts;
use renzora_ember::widgets::section;

use crate::lang::{loc_opt, tr};
use crate::rows::{ctl_drag, ctl_dropdown, ctl_toggle, focus_hide, note_row, settings_row};
use crate::state::{A_BLUE, A_GREEN, A_ORANGE};
use crate::tabs::plugins::plugins_section;

pub(crate) fn tab_editor(
    commands: &mut Commands,
    fonts: &EmberFonts,
    col: Entity,
    focus: Option<&str>,
) {
    let (sec, body) = section(commands, fonts, "wrench", &tr("settings.category.developer"), A_ORANGE);
    commands.entity(col).add_child(sec);
    focus_hide(commands, sec, focus, "general");
    let t = ctl_toggle(
        commands,
        false, // corrected by bind_2way on first frame
        |w| w.resource::<EditorSettings>().dev_mode,
        |w, &v| {
            w.resource_mut::<EditorSettings>().dev_mode = v;
            // Persist so dev mode (and plugins gated on it, e.g. plugins/tracy)
            // survive a restart.
            let _ = renzora::save_dev_mode(v);
        },
    );
    settings_row(commands, fonts, body, 0, &tr("settings.row.dev_mode"), t);

    let dv = ctl_drag(
        commands,
        fonts,
        renzora::core::console_log::DEFAULT_MAX_LOG_ENTRIES as f32,
        10.0,
        10000.0,
        10.0,
        |w| w.resource::<EditorSettings>().console_log_limit as f32,
        |w, &v| {
            let limit = (v.round() as usize).clamp(10, 10000);
            w.resource_mut::<EditorSettings>().console_log_limit = limit;
            // Apply immediately to the live buffer cap, then persist.
            renzora::core::console_log::set_max_log_entries(limit);
            let _ = renzora::save_console_log_limit(limit);
        },
    );
    settings_row(commands, fonts, body, 1, &tr("settings.row.console_log_limit"), dv);
    note_row(commands, fonts, body, &tr("settings.hint.console_log_limit"));

    let (sec, body) = section(commands, fonts, "floppy-disk", &tr("settings.cat.autosave"), A_GREEN);
    commands.entity(col).add_child(sec);
    focus_hide(commands, sec, focus, "autosave");
    let t = ctl_toggle(
        commands,
        true,
        |w| w.resource::<renzora::AutoSaveSettings>().enabled,
        |w, &v| {
            w.resource_mut::<renzora::AutoSaveSettings>().enabled = v;
            let snap = *w.resource::<renzora::AutoSaveSettings>();
            let _ = renzora::save_autosave(&snap);
        },
    );
    settings_row(commands, fonts, body, 0, &tr("settings.row.enable_autosave"), t);
    let dv = ctl_drag(
        commands,
        fonts,
        300.0,
        10.0,
        3600.0,
        10.0,
        |w| w.resource::<renzora::AutoSaveSettings>().interval_secs as f32,
        |w, &v| {
            let secs = v.round().clamp(10.0, 3600.0) as u32;
            w.resource_mut::<renzora::AutoSaveSettings>().interval_secs = secs;
            let snap = *w.resource::<renzora::AutoSaveSettings>();
            let _ = renzora::save_autosave(&snap);
        },
    );
    settings_row(commands, fonts, body, 1, &tr("settings.row.interval_secs"), dv);
    note_row(commands, fonts, body, &tr("settings.hint.autosave"));

    let (sec, body) = section(commands, fonts, "monitor", &tr("settings.cat.renderer"), A_BLUE);
    commands.entity(col).add_child(sec);
    focus_hide(commands, sec, focus, "general");
    let avail: Vec<renzora::RendererBackend> = renzora::RendererBackend::available().to_vec();
    let label_strs: Vec<String> = avail.iter().map(|b| loc_opt(b.label())).collect();
    let labels: Vec<&str> = label_strs.iter().map(|s| s.as_str()).collect();
    let av1 = avail.clone();
    let av2 = avail.clone();
    let dd = ctl_dropdown(
        commands,
        fonts,
        &labels,
        0, // reseeded from state by bind_2way on the first frame

        move |w| {
            let b = w.resource::<EditorSettings>().renderer_backend;
            av1.iter().position(|x| *x == b).unwrap_or(0)
        },
        move |w, &i| {
            if let Some(b) = av2.get(i).copied() {
                w.resource_mut::<EditorSettings>().renderer_backend = b;
                let _ = renzora::save_renderer_backend(b);
            }
        },
    );
    settings_row(commands, fonts, body, 0, &tr("settings.row.graphics_backend"), dd);
    note_row(commands, fonts, body, &tr("settings.hint.restart_editor"));

    // Import — the former Assets tab, a single toggle. Folded in here so it
    // stops being a whole sidebar category holding one checkbox.
    let (sec, body) = section(commands, fonts, "folder-open", &tr("common.import"), A_BLUE);
    commands.entity(col).add_child(sec);
    focus_hide(commands, sec, focus, "general");
    let t = ctl_toggle(
        commands, true,
        |w| w.resource::<EditorSettings>().auto_import_on_drop,
        |w, &v| w.resource_mut::<EditorSettings>().auto_import_on_drop = v,
    );
    settings_row(commands, fonts, body, 0, &tr("settings.row.drop_import"), t);

    plugins_section(commands, fonts, col, focus);
}
