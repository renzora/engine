//! The Interface page — fonts, language, display scale, and the per-panel
//! presentation preferences.
//!
//! Several of these persist to `~/.renzora/editor.toml` as well as writing
//! `EditorSettings`, so the choice survives a restart rather than only the
//! session.

use bevy::prelude::*;

use renzora_editor_framework::{EditorSettings, InspectorExpandDefault, MonoFont, UiFont};
use renzora_ember::font::EmberFonts;
use renzora_ember::widgets::section;

use crate::lang::{loc_opt, tr};
use crate::rows::{ctl_drag, ctl_dropdown, ctl_toggle, note_row, settings_row};
use crate::state::{A_BLUE, A_GREEN, A_PURPLE};

/// The whole Interface page — one sidebar category, six stacked sections. It
/// takes no `focus`: each of its sections was a single sidebar row before, and
/// three of them held exactly one control.
pub(crate) fn tab_interface(
    commands: &mut Commands,
    fonts: &EmberFonts,
    col: Entity,
    settings: &EditorSettings,
    custom: &[String],
) {
    let (sec, body) = section(commands, fonts, "text-aa", &tr("settings.cat.fonts"), A_BLUE);
    commands.entity(col).add_child(sec);

    // UI font: builtin labels + custom names.
    let ui_opts: Vec<String> = UiFont::BUILTIN
        .iter()
        .map(|f| f.label().to_string())
        .chain(custom.iter().cloned())
        .collect();
    let ui_refs: Vec<&str> = ui_opts.iter().map(|s| s.as_str()).collect();
    let cu = custom.to_vec();
    let cu2 = custom.to_vec();
    let dd = ctl_dropdown(
        commands,
        fonts,
        &ui_refs,
        ui_font_index(&settings.ui_font, custom),
        move |w| ui_font_index(&w.resource::<EditorSettings>().ui_font, &cu),
        move |w, &i| w.resource_mut::<EditorSettings>().ui_font = ui_font_from_index(i, &cu2),
    );
    settings_row(commands, fonts, body, 0, &tr("settings.row.ui_font"), dd);

    let mono_opts: Vec<String> = MonoFont::BUILTIN
        .iter()
        .map(|f| f.label().to_string())
        .chain(custom.iter().cloned())
        .collect();
    let mono_refs: Vec<&str> = mono_opts.iter().map(|s| s.as_str()).collect();
    let cm = custom.to_vec();
    let cm2 = custom.to_vec();
    let dd = ctl_dropdown(
        commands,
        fonts,
        &mono_refs,
        mono_font_index(&settings.mono_font, custom),
        move |w| mono_font_index(&w.resource::<EditorSettings>().mono_font, &cm),
        move |w, &i| w.resource_mut::<EditorSettings>().mono_font = mono_font_from_index(i, &cm2),
    );
    settings_row(commands, fonts, body, 1, &tr("settings.row.code_font"), dd);

    let dv = ctl_drag(
        commands,
        fonts,
        settings.font_size,
        10.0,
        24.0,
        0.5,
        |w| w.resource::<EditorSettings>().font_size,
        |w, &v| w.resource_mut::<EditorSettings>().font_size = v,
    );
    settings_row(commands, fonts, body, 2, &tr("settings.row.font_size"), dv);

    // ── Language ──
    // Picker over every registered language (built-in + external `languages/`
    // packs). Driven straight off the global translation table — its active
    // code is the source of truth — and persisted to `~/.renzora/editor.toml`
    // so the choice survives restarts. The row label itself is localized,
    // demonstrating the end-to-end path.
    let (sec, body) = section(commands, fonts, "globe", &tr("settings.row.language"), A_GREEN);
    commands.entity(col).add_child(sec);

    let langs = renzora::lang::available();
    let lang_labels: Vec<String> = langs
        .iter()
        .map(|m| {
            if m.name.is_empty() {
                m.code.clone()
            } else {
                m.name.clone()
            }
        })
        .collect();
    let lang_refs: Vec<&str> = lang_labels.iter().map(|s| s.as_str()).collect();
    let codes: Vec<String> = langs.iter().map(|m| m.code.clone()).collect();
    let active = renzora::lang::active_code();
    let cur = codes.iter().position(|c| *c == active).unwrap_or(0);
    let codes_get = codes.clone();
    let codes_set = codes;
    let dd = ctl_dropdown(
        commands,
        fonts,
        &lang_refs,
        cur,
        move |_w| {
            let a = renzora::lang::active_code();
            codes_get.iter().position(|c| *c == a).unwrap_or(0)
        },
        move |_w, &i| {
            if let Some(code) = codes_set.get(i) {
                renzora::lang::set_active(code);
                let _ = renzora::save_language(code);
            }
        },
    );
    settings_row(
        commands,
        fonts,
        body,
        0,
        &renzora::lang::t("settings.row.language"),
        dd,
    );

    let (sec, body) = section(commands, fonts, "monitor", &tr("settings.cat.display"), A_PURPLE);
    commands.entity(col).add_child(sec);
    let dd = ctl_dropdown(
        commands,
        fonts,
        UI_SCALE_LABELS,
        ui_scale_index(settings.ui_scale),
        |w| ui_scale_index(w.resource::<EditorSettings>().ui_scale),
        |w, &i| {
            let v = UI_SCALE_STEPS.get(i).copied().unwrap_or(1.0);
            w.resource_mut::<EditorSettings>().ui_scale = v;
            let _ = renzora::save_ui_scale(v);
        },
    );
    settings_row(commands, fonts, body, 0, &tr("settings.row.ui_scale"), dd);
    note_row(commands, fonts, body, &tr("settings.hint.ui_scale"));

    let dv = ctl_drag(
        commands,
        fonts,
        settings.scroll_speed,
        0.25,
        4.0,
        0.05,
        |w| w.resource::<EditorSettings>().scroll_speed,
        |w, &v| {
            let v = v.clamp(0.25, 4.0);
            w.resource_mut::<EditorSettings>().scroll_speed = v;
            let _ = renzora::save_scroll_speed(v);
        },
    );
    settings_row(commands, fonts, body, 1, &tr("settings.row.scroll_speed"), dv);
    note_row(commands, fonts, body, &tr("settings.hint.scroll_speed"));

    let (sec, body) = section(commands, fonts, "list-bullets", &tr("settings.cat.hierarchy"), A_BLUE);
    commands.entity(col).add_child(sec);
    let t = ctl_toggle(
        commands,
        settings.hierarchy_parent_stacking,
        |w| w.resource::<EditorSettings>().hierarchy_parent_stacking,
        |w, &v| w.resource_mut::<EditorSettings>().hierarchy_parent_stacking = v,
    );
    settings_row(commands, fonts, body, 0, &tr("settings.row.parent_stacking"), t);
    let t = ctl_toggle(
        commands,
        settings.hierarchy_toggle_on_click,
        |w| w.resource::<EditorSettings>().hierarchy_toggle_on_click,
        |w, &v| {
            w.resource_mut::<EditorSettings>().hierarchy_toggle_on_click = v;
            let _ = renzora::save_hierarchy_toggle_on_click(v);
        },
    );
    settings_row(commands, fonts, body, 1, &tr("settings.row.toggle_on_click"), t);
    note_row(commands, fonts, body, &tr("settings.hint.toggle_on_click"));

    let (sec, body) = section(commands, fonts, "sliders", &tr("settings.cat.inspector"), A_PURPLE);
    commands.entity(col).add_child(sec);
    let label_strs: Vec<String> =
        InspectorExpandDefault::ALL.iter().map(|m| loc_opt(m.label())).collect();
    let labels: Vec<&str> = label_strs.iter().map(|s| s.as_str()).collect();
    let dd = ctl_dropdown(
        commands,
        fonts,
        &labels,
        inspector_expand_index(settings.inspector_expand_default),
        |w| inspector_expand_index(w.resource::<EditorSettings>().inspector_expand_default),
        |w, &i| {
            w.resource_mut::<EditorSettings>().inspector_expand_default = InspectorExpandDefault::ALL
                .get(i)
                .copied()
                .unwrap_or_default();
        },
    );
    settings_row(commands, fonts, body, 0, &tr("settings.row.default_expand"), dd);
    note_row(commands, fonts, body, &tr("settings.hint.default_expand"));

    let t = ctl_toggle(
        commands,
        settings.drag_value_rail_sweep,
        |w| w.resource::<EditorSettings>().drag_value_rail_sweep,
        |w, &v| w.resource_mut::<EditorSettings>().drag_value_rail_sweep = v,
    );
    settings_row(commands, fonts, body, 1, &tr("settings.row.rail_sweep"), t);
    note_row(commands, fonts, body, &tr("settings.hint.rail_sweep"));

    // UI Workspace — one toggle, so it lives here as a section of Interface
    // rather than as its own sidebar row. It decides whether the game viewport
    // renders behind the UI canvas when the game-UI workspace is entered.
    let (sec, body) = section(commands, fonts, "desktop", &tr("settings.cat.workspace"), A_BLUE);
    commands.entity(col).add_child(sec);
    let t = ctl_toggle(
        commands,
        true,
        |w| w.resource::<EditorSettings>().ui_preview_by_default,
        |w, &v| w.resource_mut::<EditorSettings>().ui_preview_by_default = v,
    );
    settings_row(commands, fonts, body, 0, &tr("common.preview"), t);
    // New scripts and UI templates: a commented starter that shows the hooks and
    // a laid-out panel, or the bare minimum that works. Off is *minimal*, not
    // empty — a `.rs` without `renzora::script!` exports no entry point and a
    // `.html` without a `<template>` root does not parse — so the skeleton is
    // written either way and this decides what is inside it.
    let t = ctl_toggle(
        commands,
        true,
        |w| w.resource::<EditorSettings>().new_file_boilerplate,
        |w, &v| w.resource_mut::<EditorSettings>().new_file_boilerplate = v,
    );
    settings_row(
        commands,
        fonts,
        body,
        1,
        &renzora::lang::t_or("settings.new_file_boilerplate", "Boilerplate in new files"),
        t,
    );
    // Where the open documents are listed: the full-width strip under the top
    // bar, or a dropdown in the top bar beside Play that gives that row back to
    // the dock. Persisted per-user, so the shell builds the right chrome on the
    // first frame of the next session.
    let doc_tab_opts = [
        tr("settings.opt.doc_tabs_strip"),
        tr("settings.opt.doc_tabs_dropdown"),
    ];
    let doc_tab_refs: Vec<&str> = doc_tab_opts.iter().map(|s| s.as_str()).collect();
    let dd = ctl_dropdown(
        commands,
        fonts,
        &doc_tab_refs,
        usize::from(settings.doc_tabs_dropdown),
        |w| usize::from(w.resource::<EditorSettings>().doc_tabs_dropdown),
        |w, &i| {
            let dropdown = i == 1;
            w.resource_mut::<EditorSettings>().doc_tabs_dropdown = dropdown;
            let _ = renzora::save_doc_tabs_dropdown(dropdown);
        },
    );
    settings_row(commands, fonts, body, 1, &tr("settings.row.doc_tabs"), dd);
}

fn inspector_expand_index(v: InspectorExpandDefault) -> usize {
    InspectorExpandDefault::ALL
        .iter()
        .position(|m| *m == v)
        .unwrap_or(0)
}

/// Fixed UI-scale steps. Discrete choices instead of a drag value: the UI
/// relayouts under the cursor as the scale changes, which makes continuous
/// dragging feel like the control is fighting back.
const UI_SCALE_STEPS: &[f32] = &[0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0];
const UI_SCALE_LABELS: &[&str] = &["75%", "100%", "125%", "150%", "175%", "200%", "250%", "300%"];

fn ui_scale_index(v: f32) -> usize {
    UI_SCALE_STEPS
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| (*a - v).abs().total_cmp(&(*b - v).abs()))
        .map(|(i, _)| i)
        .unwrap_or(1)
}

fn ui_font_index(f: &UiFont, custom: &[String]) -> usize {
    match f {
        UiFont::System => 0,
        UiFont::Roboto => 1,
        UiFont::OpenSans => 2,
        UiFont::NotoSans => 3,
        UiFont::Custom(name) => {
            4 + custom.iter().position(|n| n == name).unwrap_or(0)
        }
    }
}

fn ui_font_from_index(i: usize, custom: &[String]) -> UiFont {
    match i {
        0 => UiFont::System,
        1 => UiFont::Roboto,
        2 => UiFont::OpenSans,
        3 => UiFont::NotoSans,
        n => custom
            .get(n - 4)
            .map(|s| UiFont::Custom(s.clone()))
            .unwrap_or(UiFont::NotoSans),
    }
}

fn mono_font_index(f: &MonoFont, custom: &[String]) -> usize {
    match f {
        MonoFont::JetBrainsMono => 0,
        MonoFont::FiraCode => 1,
        MonoFont::SourceCodePro => 2,
        MonoFont::Custom(name) => 3 + custom.iter().position(|n| n == name).unwrap_or(0),
    }
}

fn mono_font_from_index(i: usize, custom: &[String]) -> MonoFont {
    match i {
        0 => MonoFont::JetBrainsMono,
        1 => MonoFont::FiraCode,
        2 => MonoFont::SourceCodePro,
        n => custom
            .get(n - 3)
            .map(|s| MonoFont::Custom(s.clone()))
            .unwrap_or(MonoFont::JetBrainsMono),
    }
}
