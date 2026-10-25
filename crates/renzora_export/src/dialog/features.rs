//! The **Engine features** tab: the engine-feature strip a lean build compiles
//! from.
//!
//! Grouped into cards (3D rendering, 2D rendering, Systems, ...) so the two
//! pipelines can be compared side by side instead of reading as one 60-row wall.
//! Both the grouping and the parent/child nesting are DERIVED from the
//! capability list rather than relying on it being sorted, so a capability
//! declared anywhere lands in the right place.
//!
//! A search box and three filters sit on top, and a row the user has changed
//! from what the project scan chose says so. The warning in the summary rail
//! about a needed feature being off opens this tab on the "Changed" filter,
//! which is the only way that warning is actionable in a list this long.

use bevy::prelude::*;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_display, bind_text, bind_text_color};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::{bind_text_input, text_input};
use renzora_ember::UiInteraction;

use crate::capabilities::{Capability, CAPABILITIES, SECTIONS};
use crate::overlay::{ExportOverlayState, PackagingMode};

use super::settings::{finish_tab, tab_panel};
use super::widgets::{ca, cursor, hairline, notice, pill_button, primary_button, style_input, switch_control, tag, txt};
use super::{
    CheckAction, CheckActionBtn, ExportDialogUi, FeatureFilter, FeatureFilterBtn, FeaturesResetBtn,
    SectionToggle, AMBER,
};

pub(super) fn build_features_tab(commands: &mut Commands, fonts: &EmberFonts, host: bool, tab_max: f32) -> Entity {
    let panel = tab_panel(commands);
    if !host {
        let note = notice(commands, fonts, "info", &renzora::lang::t("export.features.note_nonhost"), text_muted());
        finish_tab(commands, panel, &[note], tab_max);
        return panel;
    }

    let mut secs = Vec::new();

    // Not lean: nothing here applies. Said, with the fix, rather than leaving a
    // page of live-looking switches that do nothing.
    let inert = notice(commands, fonts, "warning", &renzora::lang::t("export.features.inert"), AMBER);
    let switch = primary_button(commands, fonts, &renzora::lang::t("export.check.action.use_lean"), CheckActionBtn(CheckAction::UseLean));
    commands.entity(inert).add_child(switch);
    bind_display(commands, inert, |w| {
        w.get_resource::<ExportOverlayState>().is_some_and(|s| s.packaging_mode != PackagingMode::LeanSingleBinary)
    });
    secs.push(inert);

    secs.push(build_toolbar(commands, fonts));

    let hint = txt(commands, fonts, &renzora::lang::t("export.features.note_host"), 11.0, text_muted());
    commands.entity(hint).insert(Node { margin: UiRect::vertical(Val::Px(4.0)), ..default() });
    secs.push(hint);

    for &(sid, heading) in SECTIONS {
        let members: Vec<&'static Capability> = section_ordered(sid);
        if members.is_empty() {
            continue;
        }
        secs.push(build_group(commands, fonts, sid, heading, members));
    }

    let none = txt(commands, fonts, &renzora::lang::t("export.features.no_match"), 12.0, text_muted());
    commands.entity(none).insert(Node { align_self: AlignSelf::Center, margin: UiRect::all(Val::Px(20.0)), ..default() });
    bind_display(commands, none, |w| !CAPABILITIES.iter().any(|c| row_matches(w, c)));
    secs.push(none);

    finish_tab(commands, panel, &secs, tab_max);
    panel
}

/// Search box, filter chips, and "Reset to detected".
fn build_toolbar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), margin: UiRect::top(Val::Px(4.0)), ..default() })
        .id();
    let search = text_input(commands, &fonts.ui, &renzora::lang::t("export.features.search"), "");
    style_input(commands, search);
    bind_text_input(
        commands,
        search,
        |w| w.get_resource::<ExportDialogUi>().map(|u| u.feature_query.clone()).unwrap_or_default(),
        |w, v| {
            if let Some(mut u) = w.get_resource_mut::<ExportDialogUi>() {
                u.feature_query = v;
            }
        },
    );

    let chips = commands
        .spawn((
            Node { flex_direction: FlexDirection::Row, padding: UiRect::all(Val::Px(3.0)), column_gap: Val::Px(2.0), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(7.0)), ..default() },
            BackgroundColor(ca(0, 0, 0, 40)),
            BorderColor::all(hairline()),
        ))
        .id();
    for (filter, key) in [
        (FeatureFilter::All, "export.features.filter.all"),
        (FeatureFilter::On, "export.features.filter.on"),
        (FeatureFilter::Changed, "export.features.filter.changed"),
    ] {
        let chip = commands
            .spawn((
                Node { height: Val::Px(24.0), align_items: AlignItems::Center, padding: UiRect::horizontal(Val::Px(10.0)), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() },
                BackgroundColor(Color::NONE),
                UiInteraction::default(),
                FeatureFilterBtn(filter),
                cursor(),
            ))
            .id();
        bind_bg(commands, chip, move |w| {
            if w.get_resource::<ExportDialogUi>().is_some_and(|u| u.feature_filter == filter) { ca(255, 255, 255, 22) } else { Color::NONE }
        });
        let t = commands
            .spawn((Text::new(renzora::lang::t(key)), ui_font(&fonts.ui, 11.5), TextColor(rgb(text_muted())), PASS_THROUGH, bevy::text::TextLayout::no_wrap()))
            .id();
        bind_text_color(commands, t, move |w| {
            if w.get_resource::<ExportDialogUi>().is_some_and(|u| u.feature_filter == filter) { rgb(text_primary()) } else { rgb(text_muted()) }
        });
        commands.entity(chip).add_child(t);
        commands.entity(chips).add_child(chip);
    }

    let reset = pill_button(commands, fonts, "arrow-counter-clockwise", &renzora::lang::t("export.files.reset"));
    commands.entity(reset).insert(FeaturesResetBtn);
    commands.entity(bar).add_children(&[search, chips, reset]);
    bar
}

/// A section's capabilities, parents first, each followed by its own children,
/// so the nesting still reads as a tree.
fn section_ordered(sid: &'static str) -> Vec<&'static Capability> {
    let mut out = Vec::new();
    for parent in CAPABILITIES.iter().filter(|c| c.group.is_none() && c.section == sid) {
        out.push(parent);
        out.extend(CAPABILITIES.iter().filter(|c| c.group == Some(parent.id)));
    }
    out
}

fn is_on(s: &ExportOverlayState, id: &str) -> bool {
    s.capabilities.get(id).copied().unwrap_or(false)
}

/// Whether a row differs from what the project scan chose.
fn is_changed(s: &ExportOverlayState, id: &str) -> bool {
    s.detected_capabilities.get(id).copied().unwrap_or(false) != is_on(s, id)
}

fn filtering(w: &Rx) -> bool {
    w.get_resource::<ExportDialogUi>()
        .is_some_and(|u| !u.feature_query.trim().is_empty() || u.feature_filter != FeatureFilter::All)
}

/// Whether a capability passes the search box and the filter.
fn row_matches(w: &Rx, cap: &Capability) -> bool {
    let (Some(u), Some(s)) = (w.get_resource::<ExportDialogUi>(), w.get_resource::<ExportOverlayState>()) else {
        return true;
    };
    let q = u.feature_query.trim().to_lowercase();
    if !q.is_empty() {
        let label = renzora::lang::t_or(&format!("export.cap.{}.label", cap.id), cap.label).to_lowercase();
        if !label.contains(&q) && !cap.id.contains(&q) {
            return false;
        }
    }
    match u.feature_filter {
        FeatureFilter::All => true,
        FeatureFilter::On => is_on(s, cap.id),
        FeatureFilter::Changed => is_changed(s, cap.id),
    }
}

/// One section as a card: a header that folds it and switches all of it, and
/// a row per capability.
fn build_group(commands: &mut Commands, fonts: &EmberFonts, sid: &'static str, heading: &str, members: Vec<&'static Capability>) -> Entity {
    let card = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, margin: UiRect::top(Val::Px(6.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(8.0)), overflow: Overflow::clip(), ..default() },
            BackgroundColor(rgb(card_bg())),
            BorderColor::all(hairline()),
        ))
        .id();
    let ids: Vec<&'static str> = members.iter().map(|c| c.id).collect();
    {
        let members = members.clone();
        bind_display(commands, card, move |w| members.iter().any(|c| row_matches(w, c)));
    }

    // Header: [fold zone: chevron, title, "n of m on"] ..... [switch]
    //
    // The fold zone and the switch are separate siblings, each owning its own
    // `UiInteraction`: an earlier version made the whole row the fold control
    // with the switch inside it, and pressing the switch folded the section as
    // well.
    let hrow = commands
        .spawn(Node { width: Val::Percent(100.0), height: Val::Px(40.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(10.0), padding: UiRect::horizontal(Val::Px(10.0)), ..default() })
        .id();
    let fold = commands
        .spawn((
            Node { flex_grow: 1.0, height: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() },
            UiInteraction::default(),
            SectionToggle(sid),
            cursor(),
        ))
        .id();
    let chev = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 12.0);
    commands.entity(chev).insert(PASS_THROUGH);
    bind_text(commands, chev, move |w| {
        let open = filtering(w) || !w.get_resource::<ExportOverlayState>().is_some_and(|s| s.collapsed_sections.contains(sid));
        let name = if open { "caret-down" } else { "caret-right" };
        renzora_ember::phosphor_map::icon_glyph(name).unwrap_or('\u{E4C6}').to_string()
    });
    let ht = commands
        .spawn((Text::new(heading.to_string()), ui_font(&fonts.ui, 12.5), TextColor(rgb(text_primary())), PASS_THROUGH))
        .id();
    let summary = txt(commands, fonts, "", 11.0, text_muted());
    commands.entity(summary).insert(PASS_THROUGH);
    {
        let ids = ids.clone();
        bind_text(commands, summary, move |w| {
            let on = w.get_resource::<ExportOverlayState>().map(|s| ids.iter().filter(|id| is_on(s, id)).count()).unwrap_or(0);
            renzora::lang::t("export.features.group_count")
                .replace("{n}", &on.to_string())
                .replace("{total}", &ids.len().to_string())
        });
    }
    commands.entity(fold).add_children(&[chev, ht, summary]);

    // On when every capability in the section is on, and writing it sets them
    // all, children included: a child is meaningless without its parent.
    let master = switch_control(commands, false);
    {
        let get_ids = ids.clone();
        let set_ids = ids.clone();
        bind_2way(
            commands,
            master,
            move |w| w.get_resource::<ExportOverlayState>().is_some_and(|s| get_ids.iter().all(|id| is_on(s, id))),
            move |w, v: &bool| {
                if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() {
                    for id in &set_ids {
                        s.capabilities.insert(id.to_string(), *v);
                    }
                    crate::capabilities::enforce_dependencies(&mut s.capabilities);
                }
            },
        );
    }
    commands.entity(hrow).add_children(&[fold, master]);
    commands.entity(card).add_child(hrow);

    for cap in members {
        let row = feature_row(commands, fonts, cap);
        bind_display(commands, row, move |w| {
            let open = filtering(w) || !w.get_resource::<ExportOverlayState>().is_some_and(|s| s.collapsed_sections.contains(sid));
            open && row_matches(w, cap)
        });
        commands.entity(card).add_child(row);
    }
    card
}

/// One capability: label, a "changed" tag when it differs from the scan, the
/// help behind an info badge, and the switch.
fn feature_row(commands: &mut Commands, fonts: &EmberFonts, cap: &'static Capability) -> Entity {
    let id = cap.id;
    let child = cap.group.is_some();
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(34.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect { left: Val::Px(if child { 40.0 } else { 30.0 }), right: Val::Px(10.0), ..default() },
                border: UiRect::top(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(hairline()),
        ))
        .id();
    // Localize the capability label + help. Keys are
    // `export.cap.<id>.{label,help}`, falling back to the English const.
    let cap_label = renzora::lang::t_or(&format!("export.cap.{id}.label"), cap.label);
    let cap_help = renzora::lang::t_or(&format!("export.cap.{id}.help"), cap.help);
    let t = txt(commands, fonts, &cap_label, 12.0, text_primary());
    bind_text_color(commands, t, move |w| {
        if w.get_resource::<ExportOverlayState>().is_some_and(|s| is_on(s, id)) { rgb(text_primary()) } else { rgb(text_muted()) }
    });
    let changed = tag(commands, fonts, &renzora::lang::t("export.features.changed"), AMBER, ca(AMBER.0, AMBER.1, AMBER.2, 34));
    commands.entity(changed).insert((
        UiInteraction::default(),
        renzora_ember::widgets::HoverTooltip::new(renzora::lang::t("export.features.changed_tooltip")),
    ));
    bind_display(commands, changed, move |w| w.get_resource::<ExportOverlayState>().is_some_and(|s| is_changed(s, id)));
    let spacer = commands.spawn((Node { flex_grow: 1.0, ..default() }, PASS_THROUGH)).id();
    // The help is a tooltip: a full paragraph printed under every capability
    // was forty of them stacked, most of which the reader already knows.
    let info = icon_text(commands, &fonts.phosphor, "info", text_muted(), 13.0);
    commands.entity(info).insert((
        UiInteraction::default(),
        renzora_ember::widgets::HoverTooltip::new(cap_help),
        renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Help),
    ));
    let sw = switch_control(commands, false);
    bind_2way(
        commands,
        sw,
        move |w| w.get_resource::<ExportOverlayState>().is_some_and(|s| is_on(s, id)),
        move |w, v: &bool| {
            if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() {
                s.capabilities.insert(id.to_string(), *v);
                // Turning 3D rendering off has to visibly take terrain, the sky
                // set and every post-process effect with it: the build strips
                // them regardless, and a row left showing on for something that
                // is about to be dropped is the dialog lying about what it will
                // ship. Off-only, so nothing is ever switched back ON here.
                crate::capabilities::enforce_dependencies(&mut s.capabilities);
            }
        },
    );
    commands.entity(row).add_children(&[t, changed, spacer, info, sw]);
    row
}
