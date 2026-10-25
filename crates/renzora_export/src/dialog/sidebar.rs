//! The preset list.
//!
//! A preset is a named configuration for one platform, so two shipping
//! configurations for the same platform can sit side by side (a demo build and
//! a full one, say) and both survive a restart. The platform is a property OF a
//! preset, chosen when it is added.
//!
//! Presets are listed under their platform family (Desktop, Mobile, TV, Web),
//! and each says in words whether it can export as it stands. That used to be a
//! coloured dot whose three colours were explained nowhere.

use bevy::prelude::*;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::reactive::tracked::{bind_bg, bind_display, bind_text, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::icon_menu_button;
use renzora_ember::UiInteraction;

use crate::overlay::{ExportOverlayState, PackagingMode};
use crate::templates::{Platform, TemplateManager};

use super::widgets::{ca, cursor, hairline, platform_family, platform_icon, small_button, txt};
use super::{PresetBtn, PresetDelBtn, PresetDupBtn, AMBER, GREEN};

pub(super) fn build_sidebar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // `min_height: 0` is what lets the preset list below scroll instead of
    // growing. A flex item's default minimum is its content, so without this the
    // column simply gets taller as presets are added, and since the dialog's
    // height is fixed, the Duplicate/Remove buttons were pushed out through the
    // bottom of the panel.
    let col = commands
        .spawn((
            Node {
                width: Val::Px(220.0),
                flex_shrink: 0.0,
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::right(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(ca(0, 0, 0, 40)),
            BorderColor::all(hairline()),
        ))
        .id();

    // Header: title on the left, add-menu on the right.
    let head_row = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), padding: UiRect { left: Val::Px(4.0), bottom: Val::Px(4.0), ..default() }, ..default() })
        .id();
    let head = txt(commands, fonts, &renzora::lang::t("export.section.presets").to_uppercase(), 10.5, text_muted());
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

    // The list itself: keyed, so adding or removing one preset rebuilds that
    // row rather than the whole sidebar.
    let list = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() })
        .id();
    keyed_list(commands, list, preset_list_snapshot);
    // Scrolled, because the list is unbounded. `scroll_view`'s wrapper already
    // flex-grows with a zero basis, so it takes exactly the height left over
    // after the header above and the actions below.
    let list_scroll = renzora_ember::widgets::scroll_view(commands, list);
    commands.entity(col).add_child(list_scroll);

    // Empty state. A project with no presets shows nothing at all otherwise.
    let empty = txt(commands, fonts, &renzora::lang::t("export.presets.empty"), 11.0, text_muted());
    commands.entity(empty).insert(Node { margin: UiRect::all(Val::Px(6.0)), ..default() });
    bind_display(commands, empty, |w| {
        w.get_resource::<ExportOverlayState>().is_some_and(|s| s.presets.is_empty())
    });
    commands.entity(col).add_child(empty);

    // Duplicate / Remove act on the selection, so they are hidden when there
    // isn't one rather than sitting there doing nothing.
    let actions = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, column_gap: Val::Px(4.0), margin: UiRect::top(Val::Px(6.0)), ..default() })
        .id();
    let dup = small_button(commands, fonts, "copy", &renzora::lang::t("export.presets.duplicate"), PresetDupBtn);
    let del = small_button(commands, fonts, "trash", &renzora::lang::t("export.presets.remove"), PresetDelBtn);
    commands.entity(actions).add_children(&[dup, del]);
    bind_display(commands, actions, |w| {
        w.get_resource::<ExportOverlayState>().is_some_and(|s| s.active_preset.is_some())
    });
    commands.entity(col).add_child(actions);
    col
}

/// A snapshot with no rows: what the preset list shows before the overlay
/// resource exists.
fn empty_snapshot() -> KeyedSnapshot {
    KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) }
}

fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

#[derive(Clone, Hash)]
enum Entry {
    /// A family heading.
    Family(&'static str),
    /// A preset: its index, name, platform and whether it is selected.
    Preset(usize, String, Platform, bool),
}

/// This frame's sidebar entries: each family that has a preset, followed by its
/// presets in the order they were added.
///
/// Keyed by preset index (and heading name), so renaming a preset or moving the
/// selection rebuilds only the rows that actually changed.
fn preset_list_snapshot(world: &Rx) -> KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    let Some(state) = world.get_resource::<ExportOverlayState>() else {
        return empty_snapshot();
    };
    let mut presets: Vec<(usize, &crate::presets::ExportPreset)> = state.presets.iter().enumerate().collect();
    presets.sort_by_key(|(i, p)| (platform_family(p.platform).0, *i));
    let mut entries: Vec<Entry> = Vec::new();
    let mut last_family = None;
    for (i, p) in presets {
        let family = platform_family(p.platform);
        if last_family != Some(family.0) {
            entries.push(Entry::Family(family.1));
            last_family = Some(family.0);
        }
        entries.push(Entry::Preset(i, p.name.clone(), p.platform, state.active_preset == Some(i)));
    }
    let items: Vec<(u64, u64)> = entries
        .iter()
        .map(|e| {
            let mut k = hasher();
            match e {
                Entry::Family(key) => key.hash(&mut k),
                Entry::Preset(i, ..) => i.hash(&mut k),
            }
            let mut h = hasher();
            e.hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| match &entries[i] {
            Entry::Family(key) => {
                let t = txt(c, f, &renzora::lang::t(key).to_uppercase(), 10.0, text_muted());
                c.entity(t).insert(Node { margin: UiRect { left: Val::Px(6.0), top: Val::Px(8.0), bottom: Val::Px(2.0), ..default() }, ..default() });
                t
            }
            Entry::Preset(index, name, platform, selected) => preset_row(c, f, *index, name, *platform, *selected),
        }),
    }
}

/// What a preset's status line says, and its colour.
///
/// Read live (template installs and downloads change it while the dialog is
/// open), and from the working state for the selected preset, whose packaging
/// mode may have been changed since it was last saved.
fn preset_status(w: &Rx, index: usize, p: Platform) -> (String, (u8, u8, u8)) {
    let Some(s) = w.get_resource::<ExportOverlayState>() else { return (String::new(), text_muted()) };
    let mode = if s.active_preset == Some(index) {
        s.packaging_mode
    } else {
        s.presets.get(index).map(|pr| pr.packaging_mode).unwrap_or(s.packaging_mode)
    };
    if mode == PackagingMode::LeanSingleBinary {
        return (renzora::lang::t("export.preset.lean"), GREEN);
    }
    let installed = w.get_resource::<TemplateManager>().is_some_and(|t| t.is_installed(p));
    let available = s.release_info.as_ref().is_some_and(|r| r.available_platforms.contains(&p));
    if installed {
        (renzora::lang::t("export.preset.template_installed"), GREEN)
    } else if available {
        (renzora::lang::t("export.preset.template_available"), AMBER)
    } else {
        (renzora::lang::t("export.preset.no_template"), text_muted())
    }
}

/// One preset row: platform icon in a tile, the name, and a status line under it.
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
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(9.0),
                padding: UiRect::axes(Val::Px(7.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(if selected { rgb(accent()) } else { Color::NONE }),
            UiInteraction::default(),
            PresetBtn(index),
            cursor(),
        ))
        .id();
    // Selection is baked into the row hash, so only hover needs to be live.
    bind_bg(commands, btn, move |w| {
        let hov = matches!(w.get::<UiInteraction>(btn), Some(UiInteraction::Hovered));
        let (r, g, b) = accent();
        if selected { ca(r, g, b, 34) } else if hov { ca(255, 255, 255, 10) } else { Color::NONE }
    });
    let tile = commands
        .spawn((
            Node { width: Val::Px(28.0), height: Val::Px(28.0), flex_shrink: 0.0, align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(6.0)), ..default() },
            BackgroundColor(ca(255, 255, 255, 12)),
            PASS_THROUGH,
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, platform_icon(p), if selected { accent() } else { text_primary() }, 16.0);
    commands.entity(ic).insert(PASS_THROUGH);
    commands.entity(tile).add_child(ic);
    let text = commands
        .spawn((Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(1.0), overflow: Overflow::clip(), ..default() }, PASS_THROUGH))
        .id();
    let nm = commands
        .spawn((Text::new(name.to_string()), ui_font(&fonts.ui, 12.5), TextColor(rgb(text_primary())), PASS_THROUGH, bevy::text::TextLayout::no_wrap()))
        .id();
    let status_row = commands
        .spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(5.0), ..default() }, PASS_THROUGH))
        .id();
    let dot = commands
        .spawn((Node { width: Val::Px(6.0), height: Val::Px(6.0), flex_shrink: 0.0, border_radius: BorderRadius::all(Val::Px(3.0)), ..default() }, BackgroundColor(rgb(text_muted())), PASS_THROUGH))
        .id();
    bind_bg(commands, dot, move |w| rgb(preset_status(w, index, p).1));
    let st = commands
        .spawn((Text::new(String::new()), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_muted())), PASS_THROUGH, bevy::text::TextLayout::no_wrap()))
        .id();
    bind_text(commands, st, move |w| preset_status(w, index, p).0);
    commands.entity(status_row).add_children(&[dot, st]);
    commands.entity(text).add_children(&[nm, status_row]);
    commands.entity(btn).add_children(&[tile, text]);
    btn
}
