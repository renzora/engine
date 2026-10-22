//! The page strip under the Inspector's header, and the two pages that are
//! not component categories (Info and Relations).
//!
//! The Inspector shows one page of component cards at a time. The strip lists
//! the pages the inspected entity has something on, in the fixed order of
//! [`InspectorPage::ALL`], with Info and Relations apart at its right end.
//! Picking a page rebuilds the card list with only that page's sections: a
//! hidden page's cards are never built, which matters because bevy_ui lays out
//! a hidden subtree every frame (see `section::build_section`).
//!
//! It was a vertical rail down the left edge first. The rail cost every card
//! 40px of width, and in a narrow Inspector that was the difference between a
//! field and a squashed one; across the top it costs one short row instead.
//! The "rail" names in this module (`RailSelection`, `RailButton`) are from
//! then: they still mean "which page is picked".
//!
//! The strip is rebuilt with the rest of the panel by `rebuild_inspector`, into
//! a stable host ([`RailHost`]), for the same reason as the entity header: its
//! buttons depend on which components the entity has, and that only changes
//! with a rebuild.

use bevy::prelude::*;

use renzora::inspector_pages::InspectorPage;
use renzora_editor_framework::InspectorRegistry;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::reactive::tracked::{bind_bg, bind_display};
use renzora_ember::theme::{accent, mix, panel_bg, rgb, text_muted, text_primary};
use renzora_ember::UiInteraction;

use super::{c, inspected_entity, phosphor_glyph, InspectorState};

/// Height of the strip, the size of its buttons, and their corner radius.
const STRIP_H: f32 = 34.0;
const BTN_W: f32 = 28.0;
const BTN_H: f32 = 26.0;
const BTN_RADIUS: f32 = 6.0;

/// What the rail has selected: every card at once, a page of component cards,
/// or one of the two pages about the entity itself.
/// All is the default: a fresh Inspector shows every card, not just Object's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum RailSelection {
    #[default]
    All,
    Page(InspectorPage),
    Info,
    Relations,
}

/// One page as the rail shows it for the inspected entity.
pub(crate) struct PageEntry {
    pub(crate) page: InspectorPage,
    /// Sections on this page for the inspected entity.
    pub(crate) count: usize,
    /// Sections on this page matching the filter (equal to `count` with no
    /// filter).
    pub(crate) matches: usize,
}

/// The pages `entity` has sections on, in rail order. Object is always there:
/// it is where the rail starts, and an entity with nothing on it still has a
/// page that says so.
///
/// Reads only each registration's `has_fn`, never a field, so it is cheap
/// enough to run on every rebuild for every page at once.
pub(crate) fn entity_pages(world: &World, entity: Entity, filter: &str) -> Vec<PageEntry> {
    let mut counts = [(0usize, 0usize); InspectorPage::ALL.len()];
    if let Some(reg) = world.get_resource::<InspectorRegistry>() {
        for entry in reg.iter() {
            if !(entry.has_fn)(world, entity) {
                continue;
            }
            let page = InspectorPage::for_entry(entry.category, entry.type_id);
            let slot = &mut counts[page_index(page)];
            slot.0 += 1;
            if filter.is_empty() || entry.display_name.to_lowercase().contains(filter) {
                slot.1 += 1;
            }
        }
    }
    InspectorPage::ALL
        .into_iter()
        .zip(counts)
        .filter(|(page, (count, _))| *count > 0 || *page == InspectorPage::Object)
        .map(|(page, (count, matches))| PageEntry { page, count, matches })
        .collect()
}

fn page_index(page: InspectorPage) -> usize {
    InspectorPage::ALL
        .iter()
        .position(|p| *p == page)
        .unwrap_or(0)
}

/// The selection actually shown: the user's choice when the entity has that
/// page, otherwise All.
///
/// The choice itself is left alone, so tuning colliders on several crates
/// stays on Physics while a light selected in between shows everything.
pub(crate) fn effective_selection(choice: RailSelection, pages: &[PageEntry]) -> RailSelection {
    match choice {
        RailSelection::Page(p) if pages.iter().any(|e| e.page == p) => choice,
        RailSelection::Page(_) => RailSelection::All,
        other => other,
    }
}

// ── The rail ─────────────────────────────────────────────────────────────────

/// The stable host the strip's buttons are rebuilt into.
#[derive(Component)]
pub(crate) struct RailHost;

/// A page button. Pressing it picks its page.
#[derive(Component)]
pub(crate) struct RailButton(RailSelection);

/// The strip's host node, built once with the panel.
///
/// It scrolls sideways rather than wrapping: a second row of page buttons
/// would push the cards down for the rare entity with a dozen pages. The
/// wheel pans it ([`page_strip_wheel`]), as it does a dock's tab strip.
pub(crate) fn build_rail_host(commands: &mut Commands) -> Entity {
    let host = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(STRIP_H),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(2.0),
                padding: UiRect::horizontal(Val::Px(10.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                overflow: Overflow::scroll_x(),
                ..default()
            },
            BorderColor::all(c(renzora_ember::theme::divider())),
            bevy::ui::ScrollPosition::default(),
            bevy::ui::RelativeCursorPosition::default(),
            RailHost,
            Name::new("inspector-page-strip"),
        ))
        .id();
    bind_display(commands, host, |w| inspected_entity(w).is_some());
    host
}

/// Wheel over the page strip pans it sideways. It clips and draws no
/// scrollbar, so without this the pages past its right edge on a narrow panel
/// would be unreachable. Vertical wheel maps to horizontal, since a strip one
/// button tall has no vertical range. Bevy clamps `ScrollPosition` to the
/// scrollable range, so only the near edge needs guarding.
pub(crate) fn page_strip_wheel(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut strips: Query<(&bevy::ui::RelativeCursorPosition, &mut bevy::ui::ScrollPosition), With<RailHost>>,
) {
    use bevy::input::mouse::MouseScrollUnit;
    let mut delta = 0.0;
    for ev in wheel.read() {
        let unit = if matches!(ev.unit, MouseScrollUnit::Line) { 24.0 } else { 1.0 };
        delta += (-ev.y + ev.x) * unit;
    }
    if delta == 0.0 {
        return;
    }
    for (rcp, mut scroll) in &mut strips {
        if rcp.cursor_over {
            scroll.x = (scroll.x + delta).max(0.0);
        }
    }
}

/// The strip's buttons for one entity: its pages, a spacer, then Info and
/// Relations. `shown` is the selection on screen; `filtering` dims pages with
/// no card matching the filter.
///
/// Every page is an icon, the one on screen in the accent pill. It used to
/// spell out its name and card count ("Physics 3") as well, which made the
/// strip jump in width as you switched page; the tooltip carries both.
pub(crate) fn build_rail(
    commands: &mut Commands,
    fonts: &EmberFonts,
    pages: &[PageEntry],
    shown: RailSelection,
    filtering: bool,
) -> Vec<Entity> {
    let mut kids = Vec::new();
    // "All" leads the strip: every card on one page, for when you would
    // rather scroll than hunt through categories.
    let total: usize = pages.iter().map(|e| e.count).sum();
    let all_tip = format!("{} ({})", renzora::lang::t_or("inspector.page.all", "All"), components_label(total));
    kids.push(rail_button(commands, fonts, RailSelection::All, "squares-four", text_muted(), &all_tip, shown == RailSelection::All, false));
    for entry in pages {
        let sel = RailSelection::Page(entry.page);
        let name = entry.page.display_name();
        // Spelled out: a bare "Physics · 3" read as a keyboard shortcut.
        let tip = if entry.count > 0 {
            format!("{name} ({})", components_label(entry.count))
        } else {
            name.clone()
        };
        let dim = filtering && entry.matches == 0;
        let active = shown == sel;
        kids.push(rail_button(commands, fonts, sel, entry.page.icon(), entry.page.hue(), &tip, active, dim));
    }
    kids.push(
        commands
            .spawn((Node { flex_grow: 1.0, min_width: Val::Px(8.0), ..default() }, PASS_THROUGH))
            .id(),
    );
    for (sel, icon, key, fallback) in [
        (RailSelection::Info, "info", "inspector.page.info", "Info"),
        (RailSelection::Relations, "link", "inspector.page.relations", "Relations"),
    ] {
        let tip = renzora::lang::t_or(key, fallback);
        let active = shown == sel;
        kids.push(rail_button(commands, fonts, sel, icon, text_muted(), &tip, active, false));
    }
    kids
}

fn components_label(n: usize) -> String {
    if n == 1 {
        renzora::lang::t_or("inspector.page.one_component", "1 component")
    } else {
        renzora::lang::t_or("inspector.page.n_components", "{n} components").replace("{n}", &n.to_string())
    }
}

/// One page button: its icon, with the name and card count in the tooltip.
#[allow(clippy::too_many_arguments)]
fn rail_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    sel: RailSelection,
    icon: &str,
    hue: (u8, u8, u8),
    tooltip: &str,
    active: bool,
    dim: bool,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(BTN_W),
                height: Val::Px(BTN_H),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(BTN_RADIUS)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            renzora_ember::widgets::HoverTooltip::new(tooltip),
            RailButton(sel),
            Name::new("inspector-page-button"),
        ))
        .id();
    // Active is fixed at build (picking a page rebuilds the strip); only the
    // hover wash is live.
    bind_bg(commands, btn, move |w| {
        if active {
            return rgb(accent()).with_alpha(0.26);
        }
        match w.get::<UiInteraction>(btn) {
            Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(renzora_ember::theme::hover_bg()),
            _ => Color::NONE,
        }
    });
    // The active page's glyph is at full strength and the rest a little
    // under it, so the pill is not the only thing saying which one you are on.
    // A page with nothing matching the filter fades well back.
    let alpha = if dim {
        0.3
    } else if active {
        1.0
    } else {
        0.78
    };
    let glyph = phosphor_glyph(commands, fonts, icon, hue, 16.0);
    commands.entity(glyph).insert((TextColor(rgb(hue).with_alpha(alpha)), PASS_THROUGH));
    commands.entity(btn).add_child(glyph);
    btn
}

/// Press a page button: pick its page. The rebuild signature hashes the
/// choice, so the panel rebuilds onto it.
pub(crate) fn rail_click(
    q: Query<(&UiInteraction, &RailButton), Changed<UiInteraction>>,
    mut state: ResMut<InspectorState>,
) {
    for (interaction, btn) in &q {
        if *interaction == UiInteraction::Pressed && state.rail != btn.0 {
            state.rail = btn.0;
        }
    }
}

// ── Info and Relations ───────────────────────────────────────────────────────

/// What the Info and Relations pages show, read under the exclusive borrow
/// before the rebuild starts building.
pub(crate) struct EntityFacts {
    id: String,
    components: Vec<String>,
    parent: Option<String>,
    children: Vec<String>,
}

/// How many child names Relations lists before it just counts the rest.
const CHILDREN_SHOWN: usize = 12;

pub(crate) fn entity_facts(world: &World, entity: Entity) -> EntityFacts {
    let name_of = |e: Entity| {
        world
            .get::<Name>(e)
            .map(|n| n.as_str().to_string())
            .unwrap_or_else(|| format!("{}v{}", e.index(), e.generation()))
    };
    let mut components: Vec<String> = world
        .inspect_entity(entity)
        .map(|it| it.map(|(_, info)| info.name().shortname().to_string()).collect())
        .unwrap_or_default();
    components.sort();
    EntityFacts {
        id: format!("{}v{}", entity.index(), entity.generation()),
        components,
        parent: world.get::<ChildOf>(entity).map(|c| name_of(c.parent())),
        children: world
            .get::<Children>(entity)
            .map(|ch| ch.iter().map(name_of).collect())
            .unwrap_or_default(),
    }
}

/// The Info page: the entity's id and every component it carries, including
/// the ones no card is drawn for.
pub(crate) fn build_info_page(commands: &mut Commands, fonts: &EmberFonts, facts: &EntityFacts) -> Vec<Entity> {
    let mut rows = vec![
        fact_row(commands, fonts, &renzora::lang::t_or("inspector.info.entity", "Entity"), &facts.id),
        heading(
            commands,
            fonts,
            &format!(
                "{} ({})",
                renzora::lang::t_or("inspector.info.components", "Components"),
                facts.components.len()
            ),
        ),
    ];
    for name in &facts.components {
        rows.push(list_line(commands, fonts, name));
    }
    vec![page_card(commands, rows)]
}

/// The Relations page: the entity's parent and children.
pub(crate) fn build_relations_page(
    commands: &mut Commands,
    fonts: &EmberFonts,
    facts: &EntityFacts,
) -> Vec<Entity> {
    let parent = facts
        .parent
        .clone()
        .unwrap_or_else(|| renzora::lang::t_or("inspector.relations.no_parent", "None (scene root)"));
    let mut rows = vec![
        fact_row(commands, fonts, &renzora::lang::t_or("inspector.relations.parent", "Parent"), &parent),
        heading(
            commands,
            fonts,
            &format!(
                "{} ({})",
                renzora::lang::t_or("inspector.relations.children", "Children"),
                facts.children.len()
            ),
        ),
    ];
    for name in facts.children.iter().take(CHILDREN_SHOWN) {
        rows.push(list_line(commands, fonts, name));
    }
    if facts.children.len() > CHILDREN_SHOWN {
        let more = format!("+{}", facts.children.len() - CHILDREN_SHOWN);
        rows.push(list_line(commands, fonts, &more));
    }
    vec![page_card(commands, rows)]
}

fn page_card(commands: &mut Commands, rows: Vec<Entity>) -> Entity {
    let card = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(10.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(mix(panel_bg(), text_primary(), 0.03)),
            Name::new("inspector-info-card"),
        ))
        .id();
    commands.entity(card).add_children(&rows);
    card
}

fn fact_row(commands: &mut Commands, fonts: &EmberFonts, label: &str, value: &str) -> Entity {
    let l = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
            Node { width: Val::Px(72.0), flex_shrink: 0.0, ..default() },
        ))
        .id();
    let v = commands
        .spawn((Text::new(value), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary()))))
        .id();
    renzora_ember::widgets::ellipsize(commands, v, value, None);
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            padding: UiRect::bottom(Val::Px(4.0)),
            ..default()
        })
        .id();
    commands.entity(row).add_children(&[l, v]);
    row
}

fn heading(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
            Node { margin: UiRect::top(Val::Px(4.0)), ..default() },
        ))
        .id()
}

fn list_line(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    let t = commands
        .spawn((Text::new(text), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary()))))
        .id();
    renzora_ember::widgets::ellipsize(commands, t, text, None);
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(page: InspectorPage) -> PageEntry {
        PageEntry { page, count: 1, matches: 1 }
    }

    #[test]
    fn a_page_the_entity_has_stays_selected() {
        let pages = [entry(InspectorPage::Object), entry(InspectorPage::Physics)];
        let choice = RailSelection::Page(InspectorPage::Physics);
        assert_eq!(effective_selection(choice, &pages), choice);
    }

    #[test]
    fn a_missing_page_falls_back_to_all() {
        let pages = [entry(InspectorPage::Object), entry(InspectorPage::Lighting)];
        assert_eq!(
            effective_selection(RailSelection::Page(InspectorPage::Physics), &pages),
            RailSelection::All
        );
    }

    #[test]
    fn info_and_relations_never_fall_back() {
        let pages = [entry(InspectorPage::Object)];
        assert_eq!(effective_selection(RailSelection::Info, &pages), RailSelection::Info);
        assert_eq!(effective_selection(RailSelection::Relations, &pages), RailSelection::Relations);
    }
}
