//! The preset list.
//!
//! This used to list every platform, with the settings for whichever was
//! selected held in memory and lost at the next launch. A preset is that
//! configuration given a name, so two shipping configurations for the same
//! platform can sit side by side — a demo build and a full one, say — and both
//! survive a restart. The platform is now a property OF a preset, chosen when it
//! is added, rather than a separate axis.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_bg, bind_display, bind_text, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::icon_menu_button;

use crate::overlay::ExportOverlayState;
use crate::templates::{Platform, TemplateManager};

use super::widgets::{ca, cursor, platform_icon, section_label, small_button, txt};
use super::{PresetBtn, PresetDelBtn, PresetDupBtn, AMBER, GREEN};

pub(super) fn build_sidebar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // `min_height: 0` is what lets the preset list below scroll instead of
    // growing. A flex item's default minimum is its content, so without this the
    // column simply gets taller as presets are added — and since the dialog's
    // height is fixed, the Duplicate/Remove buttons and the release line under
    // the list were pushed out through the bottom of the panel.
    let col = commands.spawn(Node { width: Val::Px(180.0), flex_shrink: 0.0, min_height: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() }).id();

    // Header: title on the left, add-menu on the right.
    let head_row = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
    let head = section_label(commands, fonts, "sliders-horizontal", &renzora::lang::t("export.section.presets"));
    commands.entity(head).insert(Node { flex_grow: 1.0, ..default() });
    // The platform picker lives here, which is the whole reason a bare "+" is
    // enough: adding a preset IS choosing a platform, so there is no separate
    // list to keep in step with the selection.
    let names: Vec<&str> = Platform::ALL.iter().map(|p| p.display_name()).collect();
    let add = icon_menu_button(
        commands,
        fonts,
        "plus",
        "desktop-tower",
        &names,
        |world, i| {
            let Some(&platform) = Platform::ALL.get(i) else { return };
            let Some(mut state) = world.get_resource_mut::<ExportOverlayState>() else { return };
            // Keep the outgoing preset's edits before the new one replaces the
            // working fields.
            state.sync_active_preset();
            let name = crate::presets::unique_name(platform.display_name(), &state.presets);
            state.presets.push(crate::presets::ExportPreset::new(name, platform));
            let last = state.presets.len() - 1;
            // Not `select_preset`: that early-returns when the index is already
            // active and would also re-sync the preset we just pushed.
            state.active_preset = Some(last);
            if let Some(p) = state.presets.get(last).cloned() {
                p.apply(&mut state);
            }
            state.save_presets();
        },
    );
    commands.entity(head_row).add_children(&[head, add]);
    commands.entity(col).add_child(head_row);

    // The list itself — keyed, so adding or removing one preset rebuilds that
    // row rather than the whole sidebar (and never disturbs the rest).
    let list = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() }).id();
    keyed_list(commands, list, preset_list_snapshot);
    // Scrolled, because the list is unbounded — a preset per platform plus
    // duplicates runs past the dialog. `scroll_view`'s wrapper already flex-grows
    // with a zero basis, so it takes exactly the height left over after the
    // header above and the actions below, and scrolls the rest.
    let list_scroll = renzora_ember::widgets::scroll_view(commands, list);
    commands.entity(col).add_child(list_scroll);

    // Empty state. A project with no presets shows nothing at all otherwise,
    // and "the export dialog is blank" is a worse first impression than a line
    // of text saying what to press.
    let empty = txt(commands, fonts, &renzora::lang::t("export.presets.empty"), 11.0, text_muted());
    commands.entity(empty).insert(Node { margin: UiRect::vertical(Val::Px(8.0)), ..default() });
    bind_display(commands, empty, |w| {
        w.get_resource::<ExportOverlayState>().is_some_and(|s| s.presets.is_empty())
    });
    commands.entity(col).add_child(empty);

    // Duplicate / Remove act on the selection, so they are hidden when there
    // isn't one rather than sitting there doing nothing.
    let actions = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, column_gap: Val::Px(4.0), margin: UiRect::top(Val::Px(6.0)), ..default() }).id();
    let dup = small_button(commands, fonts, "copy", &renzora::lang::t("export.presets.duplicate"), PresetDupBtn);
    let del = small_button(commands, fonts, "trash", &renzora::lang::t("export.presets.remove"), PresetDelBtn);
    commands.entity(actions).add_children(&[dup, del]);
    bind_display(commands, actions, |w| {
        w.get_resource::<ExportOverlayState>().is_some_and(|s| s.active_preset.is_some())
    });
    commands.entity(col).add_child(actions);
    // Release info status.
    let rel = txt(commands, fonts, "", 11.0, text_muted());
    commands.entity(rel).insert(Node { margin: UiRect::top(Val::Px(8.0)), ..default() });
    bind_text(commands, rel, |w| {
        let s = w.resource::<ExportOverlayState>();
        if let Some(err) = &s.release_fetch_error {
            format!("⚠ {err}")
        } else if let Some(info) = &s.release_info {
            // Say when this is the nightly fallback rather than a release for
            // this exact version — "where did this runtime come from?" should be
            // answerable without reading the source.
            let key = if info.is_fallback {
                "export.status.nightly"
            } else {
                "export.status.latest"
            };
            format!("{} {}", renzora::lang::t(key), info.tag_name)
        } else if s.release_fetch_started {
            renzora::lang::t("export.status.loading_release")
        } else {
            String::new()
        }
    });
    commands.entity(col).add_child(rel);
    col
}

/// A snapshot with no rows — what the preset list shows before the overlay
/// resource exists.
fn empty_snapshot() -> KeyedSnapshot {
    KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) }
}

fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

/// This frame's preset rows, keyed by index.
///
/// The hash carries everything a row draws — name, platform, and whether it is
/// the selection — so renaming a preset or moving the selection rebuilds only
/// the rows that actually changed.
fn preset_list_snapshot(world: &Rx) -> KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    let Some(state) = world.get_resource::<ExportOverlayState>() else {
        return empty_snapshot();
    };
    let rows: Vec<(String, Platform, bool)> = state
        .presets
        .iter()
        .enumerate()
        .map(|(i, p)| (p.name.clone(), p.platform, state.active_preset == Some(i)))
        .collect();
    let items: Vec<(u64, u64)> = rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            row.hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (name, platform, selected) = &rows[i];
            preset_row(c, f, i, name, *platform, *selected)
        }),
    }
}

/// One preset row: platform icon, name, and the template-status dot that used to
/// sit on the platform button.
///
/// The dot still earns its place — a preset for a platform whose runtime
/// template is not installed cannot export, and saying so here is what makes the
/// Download button in the right pane make sense when you reach it.
fn preset_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    index: usize,
    name: &str,
    p: Platform,
    selected: bool,
) -> Entity {
    let btn = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(40.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), padding: UiRect::horizontal(Val::Px(10.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(if selected { rgb(section_bg()) } else { Color::NONE }),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            PresetBtn(index),
            cursor(),
        ))
        .id();
    // Selection is baked into the row hash, so only hover needs to be live.
    bind_bg(commands, btn, move |w| {
        let hov = matches!(w.get::<Interaction>(btn), Some(Interaction::Hovered));
        if selected { rgb(section_bg()) } else if hov { ca(255, 255, 255, 10) } else { Color::NONE }
    });
    let ic = icon_text(commands, &fonts.phosphor, platform_icon(p), text_primary(), 18.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let nm = commands.spawn((Text::new(name.to_string()), ui_font(&fonts.ui, 12.5), TextColor(rgb(text_primary())), FocusPolicy::Pass, Node { flex_grow: 1.0, ..default() }, bevy::text::TextLayout::no_wrap())).id();
    let dot = commands.spawn((Node { width: Val::Px(8.0), height: Val::Px(8.0), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() }, BackgroundColor(rgb(text_muted())), FocusPolicy::Pass)).id();
    bind_bg(commands, dot, move |w| {
        let installed = w.get_resource::<TemplateManager>().is_some_and(|t| t.is_installed(p));
        let available = w.get_resource::<ExportOverlayState>().and_then(|s| s.release_info.as_ref().map(|r| r.available_platforms.contains(&p))).unwrap_or(false);
        if installed { rgb(GREEN) } else if available { rgb(AMBER) } else { ca(130, 138, 160, 150) }
    });
    commands.entity(btn).add_children(&[ic, nm, dot]);
    btn
}
