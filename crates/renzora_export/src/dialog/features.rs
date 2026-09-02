//! The **Features** tab: the engine-feature strip a lean build compiles from.
//!
//! Grouped into sections (3D rendering, 2D rendering, Systems, …) so the two
//! pipelines can be compared side by side instead of reading as one 60-row wall.
//! Both the section grouping and the parent/child nesting are DERIVED from the
//! capability list rather than relying on it being sorted, so a capability
//! declared anywhere lands in the right place.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_2way, bind_display, bind_text};
use renzora_ember::theme::*;
use renzora_ember::widgets::section;

use crate::overlay::ExportOverlayState;

use super::settings::{finish_tab, tab_panel};
use super::widgets::{ca, cursor, row_fill, switch_control, txt};
use super::SectionToggle;

pub(super) fn build_features_tab(commands: &mut Commands, fonts: &EmberFonts, host: bool, tab_max: f32) -> Entity {
    let panel = tab_panel(commands);
    let (sec, body) = section(commands, fonts, "sliders-horizontal", &renzora::lang::t("export.section.engine_features"), accent());
    if host {
        let note = txt(commands, fonts, &renzora::lang::t("export.features.note_host"), 11.0, text_muted());
        commands.entity(body).add_child(note);
        let list = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() }).id();
        // Within a section: parents first, each followed by its own children, so
        // the nesting still reads as a tree.
        let mut ordered: Vec<(Option<&str>, &crate::capabilities::Capability)> = Vec::new();
        for (sid, heading) in crate::capabilities::SECTIONS {
            let mut first = true;
            for parent in crate::capabilities::CAPABILITIES
                .iter()
                .filter(|c| c.group.is_none() && c.section == *sid)
            {
                // The heading rides on the first row of the section, so a section
                // that ends up empty never leaves a dangling header.
                ordered.push((first.then_some(*heading), parent));
                first = false;
                for child in crate::capabilities::CAPABILITIES
                    .iter()
                    .filter(|c| c.group == Some(parent.id))
                {
                    ordered.push((None, child));
                }
            }
        }
        for (idx, (heading, cap)) in ordered.into_iter().enumerate() {
            if let Some(heading) = heading {
                let sid = cap.section;
                // Header: [checkbox] TITLE ......... [chevron]
                //
                // The row itself is NOT interactive. The checkbox and the
                // fold zone are separate siblings, each owning its own
                // `Interaction` — an earlier version made the whole row the fold
                // control with buttons inside it, and pressing a button folded
                // the section as well as doing its job.
                let hrow = commands.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(8.0),
                        padding: UiRect { left: Val::Px(6.0), right: Val::Px(6.0), top: Val::Px(4.0), bottom: Val::Px(4.0) },
                        margin: UiRect { top: Val::Px(if idx == 0 { 0.0 } else { 8.0 }), bottom: Val::Px(2.0), ..default() },
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(ca(255, 255, 255, 10)),
                )).id();
                // Section checkbox: on when every capability in the section is on,
                // and writing it sets them all. Children included — a child is
                // meaningless without its parent, and the nested entries are where
                // most of the size lives.
                let scb = switch_control(commands, false);
                bind_2way(
                    commands,
                    scb,
                    move |w| {
                        w.get_resource::<ExportOverlayState>().is_some_and(|s| {
                            section_members(sid)
                                .all(|c| s.capabilities.get(c.id).copied().unwrap_or(c.default_on))
                        })
                    },
                    move |w, v: &bool| {
                        if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() {
                            for c in section_members(sid) {
                                s.capabilities.insert(c.id.to_string(), *v);
                            }
                            crate::capabilities::enforce_dependencies(&mut s.capabilities);
                        }
                    },
                );
                // Everything right of the checkbox folds the section.
                let fold = commands.spawn((
                    Node {
                        flex_grow: 1.0,
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        ..default()
                    },
                    Interaction::default(),
                    SectionToggle(sid),
                    cursor(),
                )).id();
                let ht = commands.spawn((
                    Text::new(heading.to_string()),
                    ui_font(&fonts.ui, 11.0),
                    TextColor(rgb(text_primary())),
                    Node { flex_grow: 1.0, ..default() },
                    FocusPolicy::Pass,
                )).id();
                // Chevron direction tracks the fold state, so the row reads as a
                // control rather than decoration.
                let chev = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 11.0);
                commands.entity(chev).insert(FocusPolicy::Pass);
                bind_text(commands, chev, move |w| {
                    let collapsed = w
                        .get_resource::<ExportOverlayState>()
                        .is_some_and(|s| s.collapsed_sections.contains(sid));
                    let name = if collapsed { "caret-right" } else { "caret-down" };
                    renzora_ember::phosphor_map::icon_glyph(name)
                        .unwrap_or('\u{E4C6}')
                        .to_string()
                });
                commands.entity(fold).add_children(&[ht, chev]);
                commands.entity(hrow).add_children(&[scb, fold]);
                commands.entity(list).add_child(hrow);
            }
            let id = cap.id;
            let child = cap.group.is_some();
            // One row per capability: checkbox, label, and an info badge holding
            // what used to be a paragraph of help under every line.
            //
            // One uniform faint fill, plus a hairline rule between rows —
            // see `row_fill` for why the fill cannot simply be dropped.
            // Children indent, take no rule and no fill, so a group still reads
            // as belonging to the row above it.
            let item = commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect {
                            left: Val::Px(if child { 24.0 } else { 6.0 }),
                            right: Val::Px(6.0),
                            top: Val::Px(5.0),
                            bottom: Val::Px(5.0),
                        },
                        border: if child {
                            UiRect::ZERO
                        } else {
                            UiRect::bottom(Val::Px(1.0))
                        },
                        ..default()
                    },
                    BackgroundColor(if child { Color::NONE } else { row_fill() }),
                    BorderColor::all(ca(255, 255, 255, 14)),
                ))
                .id();
            // Fold: hide the row when its section is collapsed. Reactive rather
            // than a rebuild, so the checkboxes and scroll position survive.
            let sid = cap.section;
            bind_display(commands, item, move |w| {
                !w.get_resource::<ExportOverlayState>()
                    .is_some_and(|s| s.collapsed_sections.contains(sid))
            });
            // Inlined `check_state` so the closures can capture the capability id.
            let row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).id();
            let cb = switch_control(commands, false);
            bind_2way(
                commands,
                cb,
                move |w| w.get_resource::<ExportOverlayState>().map(|s| s.capabilities.get(id).copied().unwrap_or(false)).unwrap_or(false),
                move |w, v: &bool| {
                    if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() {
                        s.capabilities.insert(id.to_string(), *v);
                        // Turning 3D rendering off has to visibly take terrain,
                        // the sky set and every post-process effect with it —
                        // the build strips them regardless, and a row left
                        // showing green for something that is about to be
                        // dropped is the dialog lying about what it will ship.
                        // Off-only, so nothing is ever switched back ON here.
                        crate::capabilities::enforce_dependencies(&mut s.capabilities);
                    }
                },
            );
            // Localize the capability label + help (the Features list). Keys are
            // `export.cap.<id>.{label,help}`, falling back to the English const.
            let cap_label = renzora::lang::t_or(&format!("export.cap.{id}.label"), cap.label);
            let cap_help = renzora::lang::t_or(&format!("export.cap.{id}.help"), cap.help);
            let t = txt(commands, fonts, &cap_label, 12.0, text_primary());
            // The help was a full paragraph printed under every capability —
            // forty of them stacked, most of which the reader already knows.
            // Behind a badge it is there when wanted and silent otherwise.
            //
            // Right-aligned via a spacer so the badges line up in a column
            // instead of trailing each label at a different x.
            let spacer = commands
                .spawn((Node { flex_grow: 1.0, ..default() }, FocusPolicy::Pass))
                .id();
            let info = icon_text(commands, &fonts.phosphor, "info", text_muted(), 13.0);
            commands.entity(info).insert((
                // `hover_tooltip_system` reads `Interaction`, so the badge needs
                // one. It stays on the default `Pass`: the badge owns no press,
                // and hover is marked on every node under the cursor regardless.
                Interaction::default(),
                renzora_ember::widgets::HoverTooltip::new(cap_help),
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Help),
            ));
            commands.entity(row).add_children(&[cb, t, spacer, info]);
            commands.entity(item).add_child(row);
            commands.entity(list).add_child(item);
        }
        commands.entity(body).add_child(list);
    } else {
        let note = txt(commands, fonts, &renzora::lang::t("export.features.note_nonhost"), 11.0, text_muted());
        commands.entity(body).add_child(note);
    }
    finish_tab(commands, panel, &[sec], tab_max);
    panel
}

/// Every capability rendered under one section heading, parents and children.
///
/// A child is placed by its PARENT's section — `group` decides nesting and
/// `section` decides placement, and the two agree by construction, but resolving
/// through the parent means a mismatch can't leave a visible row out of the
/// header checkbox's reach.
fn section_members(sid: &'static str) -> impl Iterator<Item = &'static crate::capabilities::Capability> {
    crate::capabilities::CAPABILITIES.iter().filter(move |c| {
        let owning = c
            .group
            .and_then(|p| crate::capabilities::CAPABILITIES.iter().find(|x| x.id == p))
            .map_or(c.section, |p| p.section);
        owning == sid
    })
}
