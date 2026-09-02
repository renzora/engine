//! The Scripting page.

use bevy::prelude::*;

use renzora_editor_framework::EditorSettings;
use renzora_ember::font::EmberFonts;
use renzora_ember::widgets::section;

use crate::lang::tr;
use crate::rows::{ctl_toggle, settings_row};
use crate::state::A_GREEN;

/// Scripting + Code Editor as one page — the two were separate sidebar rows for
/// eight toggles between them.
pub(crate) fn tab_scripting(commands: &mut Commands, fonts: &EmberFonts, col: Entity) {
    let (sec, body) = section(commands, fonts, "code", &tr("settings.category.scripting"), A_GREEN);
    commands.entity(col).add_child(sec);
    let t = ctl_toggle(
        commands, true,
        |w| w.resource::<EditorSettings>().script_rerun_on_ready_on_reload,
        |w, &v| w.resource_mut::<EditorSettings>().script_rerun_on_ready_on_reload = v,
    );
    settings_row(commands, fonts, body, 0, &tr("settings.row.hot_reload"), t);
    let t = ctl_toggle(
        commands, true,
        |w| w.resource::<EditorSettings>().hide_cursor_in_play_mode,
        |w, &v| w.resource_mut::<EditorSettings>().hide_cursor_in_play_mode = v,
    );
    settings_row(commands, fonts, body, 1, &tr("settings.row.cursor"), t);
    let t = ctl_toggle(
        commands, true,
        |w| w.resource::<EditorSettings>().external_play_window,
        // Persisted like the Play dropdown's choice — both edit the same flag.
        |w, &v| {
            w.resource_mut::<EditorSettings>().external_play_window = v;
            let _ = renzora::save_play_runtime_window(v);
        },
    );
    settings_row(commands, fonts, body, 2, &tr("settings.row.external_window"), t);

    let (sec, body) = section(commands, fonts, "code", &tr("settings.cat.code_editor"), A_GREEN);
    commands.entity(col).add_child(sec);
    let t = ctl_toggle(
        commands, true,
        |w| w.resource::<EditorSettings>().code_auto_close_pairs,
        |w, &v| w.resource_mut::<EditorSettings>().code_auto_close_pairs = v,
    );
    settings_row(commands, fonts, body, 0, &tr("settings.row.auto_close_pairs"), t);
    let t = ctl_toggle(
        commands, true,
        |w| w.resource::<EditorSettings>().code_trim_trailing_whitespace_on_save,
        |w, &v| w.resource_mut::<EditorSettings>().code_trim_trailing_whitespace_on_save = v,
    );
    settings_row(commands, fonts, body, 1, &tr("settings.row.trim_on_save"), t);
    let t = ctl_toggle(
        commands, true,
        |w| w.resource::<EditorSettings>().code_show_minimap,
        |w, &v| w.resource_mut::<EditorSettings>().code_show_minimap = v,
    );
    settings_row(commands, fonts, body, 2, &tr("settings.row.minimap"), t);
    let t = ctl_toggle(
        commands, false,
        |w| w.resource::<EditorSettings>().code_show_whitespace,
        |w, &v| w.resource_mut::<EditorSettings>().code_show_whitespace = v,
    );
    settings_row(commands, fonts, body, 3, &tr("settings.row.whitespace_markers"), t);
    let t = ctl_toggle(
        commands, false,
        |w| w.resource::<EditorSettings>().code_word_wrap,
        |w, &v| w.resource_mut::<EditorSettings>().code_word_wrap = v,
    );
    settings_row(commands, fonts, body, 4, &renzora::lang::t_or("settings.row.word_wrap", "Word Wrap"), t);
}
