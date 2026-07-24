//! Search list — a reusable, category-grouped, filterable picker (the ember port
//! of the egui search overlay). A search box over a two-pane body: a left
//! category sidebar (click to jump to a category) and a right list of
//! collapsible category sections with accent-colored headers, indent guides,
//! zebra rows, and per-row actions. Typing filters (and force-expands); Enter
//! picks the first match; clicking a row runs its `&mut World` action and, if
//! the list is inside an [`super::overlay::Overlay`], closes it.

use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;

use crate::font::{icon_glyph, ui_font, EmberFonts};
use crate::reactive::bind_bg;
use crate::theme::*;

use super::overlay::Overlay;
use super::scroll_area::scroll_view_bar;
use super::text_input::{text_input, EmberTextInput};

const TEXT: (u8, u8, u8) = (224, 224, 234);
const MUTED: (u8, u8, u8) = (150, 150, 164);
const ZEBRA_ODD: (u8, u8, u8) = (36, 36, 43);
const GUIDE: (u8, u8, u8) = (62, 62, 74);
const SIDEBAR_BG: (u8, u8, u8) = crate::theme::WINDOW_BG;

/// A stable accent color per category (ember port of `category_colors`).
///
/// Public so anything presenting the *same* categories in another shape — the
/// hierarchy's right-click quick-add menu, say — tints them identically to this
/// overlay instead of inventing a second palette.
pub fn category_color(name: &str) -> (u8, u8, u8) {
    match name.to_lowercase().as_str() {
        "transform" => (150, 200, 255),
        "environment" => (130, 210, 160),
        "light" | "lighting" => (255, 210, 120),
        "camera" => (180, 160, 255),
        "script" | "scripting" => (130, 230, 180),
        "physics" => (255, 150, 120),
        "plugin" | "plugins" => (200, 160, 255),
        "nodes2d" | "nodes_2d" | "2d" => (120, 200, 255),
        "ui" => (255, 180, 200),
        "rendering" => (160, 200, 255),
        "effects" | "particles" => (255, 170, 210),
        "post_process" => (210, 170, 255),
        "audio" => (200, 140, 230),
        "shapes" | "primitives" | "mesh" | "meshes" => (255, 190, 110),
        _ => (170, 175, 200),
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().to_string() + c.as_str(),
    }
}

/// Composite `top` over `base` (straight sRGBA) — for the hover wash over zebra.
fn over(base: Color, top: Color) -> Color {
    let b = base.to_srgba();
    let t = top.to_srgba();
    let a = t.alpha + b.alpha * (1.0 - t.alpha);
    if a <= 0.0 {
        return Color::NONE;
    }
    Color::srgba(
        (t.red * t.alpha + b.red * b.alpha * (1.0 - t.alpha)) / a,
        (t.green * t.alpha + b.green * b.alpha * (1.0 - t.alpha)) / a,
        (t.blue * t.alpha + b.blue * b.alpha * (1.0 - t.alpha)) / a,
        a,
    )
}

/// One pickable entry in a [`search_list`].
pub struct SearchEntry {
    pub icon: String,
    pub label: String,
    pub category: String,
    pub color: (u8, u8, u8),
    pub action: Box<dyn Fn(&mut World) + Send + Sync>,
}

impl SearchEntry {
    pub fn new<F>(
        icon: impl Into<String>,
        label: impl Into<String>,
        category: impl Into<String>,
        action: F,
    ) -> Self
    where
        F: Fn(&mut World) + Send + Sync + 'static,
    {
        Self {
            icon: icon.into(),
            label: label.into(),
            category: category.into(),
            color: MUTED,
            action: Box::new(action),
        }
    }

    pub fn colored(mut self, color: (u8, u8, u8)) -> Self {
        self.color = color;
        self
    }
}

/// Convenience: a centered [`Overlay`] titled `title` whose content is a
/// [`search_list`]. The two are independently reusable; this just composes them.
pub fn search_overlay(
    commands: &mut Commands,
    fonts: &EmberFonts,
    title: &str,
    entries: Vec<SearchEntry>,
) -> Entity {
    let (root, content) = super::overlay::overlay_sized(commands, fonts, title, 680.0, 560.0, true);
    let list = search_list(commands, fonts, entries);
    commands.entity(content).add_child(list);
    root
}

#[derive(Component)]
pub(crate) struct SearchInput;

/// A category section header (clickable to collapse/expand).
#[derive(Component)]
pub(crate) struct SearchCat {
    input: Entity,
    category: String,
    expanded: bool,
    caret: Entity,
}

/// A sidebar button that jumps to a category — `None` is the "All" entry
/// (expands every category).
#[derive(Component)]
pub(crate) struct SearchSidebarCat {
    category: Option<String>,
}

#[derive(Component)]
pub(crate) struct SearchRow {
    input: Entity,
    category: String,
    label_lc: String,
    order: usize,
}

#[derive(Component)]
pub(crate) struct SearchAction(Box<dyn Fn(&mut World) + Send + Sync>);

/// Build a search box + two-pane category-grouped filterable list. Returns the
/// container.
pub fn search_list(commands: &mut Commands, fonts: &EmberFonts, entries: Vec<SearchEntry>) -> Entity {
    // Group entries by first-seen category order.
    let mut order: Vec<String> = Vec::new();
    let mut groups: Vec<Vec<SearchEntry>> = Vec::new();
    for entry in entries {
        if let Some(idx) = order.iter().position(|c| c == &entry.category) {
            groups[idx].push(entry);
        } else {
            order.push(entry.category.clone());
            groups.push(vec![entry]);
        }
    }

    let col = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                ..default()
            },
            Name::new("search-list"),
        ))
        .id();

    // Fixed search bar just above the list (padded so the field never overflows).
    let search_ph = renzora::lang::t("common.search");
    let input = text_input(commands, &fonts.ui, &search_ph, "");
    commands.entity(input).insert((
        SearchInput,
        Node {
            width: Val::Percent(100.0),
            min_width: Val::Px(0.0),
            flex_grow: 1.0,
            padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
    ));
    let search_bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_shrink: 0.0,
                padding: UiRect::axes(Val::Px(8.0), Val::Px(8.0)),
                ..default()
            },
            Name::new("search-bar"),
        ))
        .id();
    commands.entity(search_bar).add_child(input);

    // Two-pane body: sidebar + scrollable list.
    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            ..default()
        })
        .id();

    // Right list first (so sidebar buttons exist alongside their categories).
    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            padding: UiRect::vertical(Val::Px(2.0)),
            ..default()
        })
        .id();

    let mut list_kids: Vec<Entity> = Vec::new();
    let mut sidebar_kids: Vec<Entity> = Vec::new();
    let mut row_order: usize = 0;
    let mut zebra: usize = 0;

    if order.is_empty() {
        list_kids.push(
            commands
                .spawn((
                    Text::new("No results found."),
                    ui_font(&fonts.ui, 11.0),
                    TextColor(rgb(MUTED)),
                    Node {
                        padding: UiRect::all(Val::Px(12.0)),
                        ..default()
                    },
                ))
                .id(),
        );
    }

    // "All" jumps every category open.
    let all_label = renzora::lang::t_or("entity.cat.all", "All");
    sidebar_kids.push(sidebar_button(commands, fonts, &all_label, (214, 214, 224), None));

    for (cat, items) in order.iter().zip(groups) {
        let accent = category_color(cat);

        // Sidebar button.
        sidebar_kids.push(sidebar_button(commands, fonts, &capitalize(cat), accent, Some(cat.clone())));

        // Category header (caret + accent label, zebra bg, clickable).
        let header = category_header(commands, fonts, input, cat, accent, zebra);
        zebra += 1;
        list_kids.push(header);

        for entry in items {
            let row = item_row(commands, fonts, input, cat, accent, zebra, row_order, entry);
            zebra += 1;
            row_order += 1;
            list_kids.push(row);
        }
    }

    commands.entity(list).add_children(&list_kids);
    let scroll = scroll_view_bar(commands, list);

    let sidebar = commands
        .spawn((
            Node {
                width: Val::Px(120.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(Val::Px(6.0), Val::Px(8.0)),
                row_gap: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(rgb(SIDEBAR_BG)),
            Name::new("search-sidebar"),
        ))
        .id();
    let cats_label = commands
        .spawn((
            Text::new(renzora::lang::t("common.categories")),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(MUTED)),
            Node {
                margin: UiRect::bottom(Val::Px(2.0)),
                ..default()
            },
        ))
        .id();
    let mut sb = vec![cats_label];
    sb.extend(sidebar_kids);
    commands.entity(sidebar).add_children(&sb);

    commands.entity(body).add_children(&[sidebar, scroll]);
    commands.entity(col).add_children(&[search_bar, body]);
    col
}

/// A floating, cursor-anchored **searchable** picker — the "right-click /
/// Spacebar to add a node" palette. Same typeahead + category list as
/// [`search_list`], but in a compact [`ScreenMenu`](super::popup::ScreenMenu)
/// panel positioned at window `(x, y)` instead of a big centered overlay. Being a
/// `ScreenMenu` it gets click-outside dismiss + on-screen clamping for free, and
/// picking a row runs its action and closes the panel (see [`run_and_close`]).
pub fn search_menu(commands: &mut Commands, fonts: &EmberFonts, x: f32, y: f32, entries: Vec<SearchEntry>) -> Entity {
    let list = search_list(commands, fonts, entries);
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x),
                top: Val::Px(y),
                width: Val::Px(300.0),
                height: Val::Px(380.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            GlobalZIndex(9000),
            super::popup::ScreenMenu,
            super::popup::OverlaySurface,
            bevy::ui::FocusPolicy::Block,
            bevy::ui::RelativeCursorPosition::default(),
            Name::new("search-menu"),
        ))
        .id();
    commands.entity(root).add_child(list);
    root
}

/// Convenience: a wide [`Overlay`] whose content is a [`search_grid`] — the
/// multi-column category picker (the ember port of the egui panel picker).
pub fn grid_overlay(
    commands: &mut Commands,
    fonts: &EmberFonts,
    title: &str,
    entries: Vec<SearchEntry>,
) -> Entity {
    let (root, content) = super::overlay::overlay_sized(commands, fonts, title, 840.0, 600.0, true);
    let grid = search_grid(commands, fonts, entries, 4);
    commands.entity(content).add_child(grid);
    root
}

/// Like [`search_list`] but lays the categories out as `columns` balanced
/// columns (by item count) instead of a sidebar + single list. Reuses the same
/// `SearchEntry` rows + filter/select/Enter systems.
pub fn search_grid(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entries: Vec<SearchEntry>,
    columns: usize,
) -> Entity {
    // Group by first-seen category order.
    let mut order: Vec<String> = Vec::new();
    let mut groups: Vec<Vec<SearchEntry>> = Vec::new();
    for entry in entries {
        if let Some(idx) = order.iter().position(|c| c == &entry.category) {
            groups[idx].push(entry);
        } else {
            order.push(entry.category.clone());
            groups.push(vec![entry]);
        }
    }

    let col = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                ..default()
            },
            Name::new("search-grid"),
        ))
        .id();

    let search_ph = renzora::lang::t("common.search");
    let input = text_input(commands, &fonts.ui, &search_ph, "");
    commands.entity(input).insert((
        SearchInput,
        Node {
            width: Val::Percent(100.0),
            min_width: Val::Px(0.0),
            flex_grow: 1.0,
            padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
    ));
    let search_bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_shrink: 0.0,
                padding: UiRect::axes(Val::Px(8.0), Val::Px(8.0)),
                ..default()
            },
            Name::new("search-bar"),
        ))
        .id();
    commands.entity(search_bar).add_child(input);

    // Balanced column packing: largest categories first, into the shortest column.
    let mut packed: Vec<(String, Vec<SearchEntry>)> = order.into_iter().zip(groups).collect();
    packed.sort_by_key(|(_, items)| std::cmp::Reverse(items.len()));
    let n = columns.clamp(1, packed.len().max(1));
    let mut cols: Vec<Vec<(String, Vec<SearchEntry>)>> = (0..n).map(|_| Vec::new()).collect();
    let mut heights = vec![0usize; n];
    for (cat, items) in packed {
        let idx = heights
            .iter()
            .enumerate()
            .min_by_key(|(_, h)| **h)
            .map(|(i, _)| i)
            .unwrap_or(0);
        heights[idx] += items.len() + 2;
        cols[idx].push((cat, items));
    }
    for c in &mut cols {
        c.sort_by(|a, b| a.0.cmp(&b.0));
    }

    let mut order_idx: usize = 0;
    let columns_row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FlexStart,
            column_gap: Val::Px(14.0),
            padding: UiRect::horizontal(Val::Px(4.0)),
            ..default()
        })
        .id();
    let mut col_entities: Vec<Entity> = Vec::new();
    for col_cats in cols {
        let column = commands
            .spawn(Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                ..default()
            })
            .id();
        let mut kids: Vec<Entity> = Vec::new();
        for (cat, items) in col_cats {
            let accent = category_color(&cat);
            kids.push(category_header_styled(commands, fonts, input, &cat, accent, 0, false));
            for entry in items {
                kids.push(item_row(commands, fonts, input, &cat, accent, 0, order_idx, entry));
                order_idx += 1;
            }
        }
        commands.entity(column).add_children(&kids);
        col_entities.push(column);
    }
    commands.entity(columns_row).add_children(&col_entities);
    let scroll = scroll_view_bar(commands, columns_row);

    commands.entity(col).add_children(&[search_bar, scroll]);
    col
}

fn sidebar_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    accent: (u8, u8, u8),
    category: Option<String>,
) -> Entity {
    let b = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            SearchSidebarCat { category },
            Name::new("search-sidebar-cat"),
        ))
        .id();
    bind_bg(commands, b, move |w| match w.get::<Interaction>(b) {
        Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(section_bg()),
        _ => Color::NONE,
    });
    let t = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(accent)),
        ))
        .id();
    commands.entity(b).add_child(t);
    b
}

fn category_header(
    commands: &mut Commands,
    fonts: &EmberFonts,
    input: Entity,
    cat: &str,
    accent: (u8, u8, u8),
    zebra: usize,
) -> Entity {
    category_header_styled(commands, fonts, input, cat, accent, zebra, true)
}

/// `collapsible` headers (list mode) get a caret + click-to-collapse; the grid
/// uses plain headers (no caret), matching the egui panel picker.
fn category_header_styled(
    commands: &mut Commands,
    fonts: &EmberFonts,
    input: Entity,
    cat: &str,
    accent: (u8, u8, u8),
    zebra: usize,
    collapsible: bool,
) -> Entity {
    let mut header = commands.spawn((
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            width: Val::Percent(100.0),
            column_gap: Val::Px(6.0),
            padding: UiRect::axes(Val::Px(6.0), Val::Px(5.0)),
            ..default()
        },
        BackgroundColor(zebra_bg(zebra)),
        Name::new("search-cat"),
    ));
    if collapsible {
        header.insert(Interaction::default());
    }
    let header = header.id();
    let mut kids = Vec::new();
    let caret = if collapsible {
        let c = commands
            .spawn((
                Text::new(icon_glyph("caret-down").unwrap_or('v').to_string()),
                TextFont {
                    font: bevy::text::FontSource::Handle(fonts.phosphor.clone()),
                    font_size: bevy::text::FontSize::Px(11.0),
                    ..default()
                },
                TextColor(rgb(placeholder())),
                bevy::ui::FocusPolicy::Pass,
            ))
            .id();
        kids.push(c);
        c
    } else {
        Entity::PLACEHOLDER
    };
    let label = commands
        .spawn((
            Text::new(capitalize(cat)),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(accent)),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    kids.push(label);
    commands.entity(header).add_children(&kids);
    commands.entity(header).insert(SearchCat {
        input,
        category: cat.to_string(),
        expanded: true,
        caret,
    });
    header
}

#[allow(clippy::too_many_arguments)]
fn item_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    input: Entity,
    cat: &str,
    accent: (u8, u8, u8),
    zebra: usize,
    order: usize,
    entry: SearchEntry,
) -> Entity {
    let base = zebra_bg(zebra);
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                width: Val::Percent(100.0),
                column_gap: Val::Px(8.0),
                padding: UiRect::new(Val::Px(26.0), Val::Px(8.0), Val::Px(5.0), Val::Px(5.0)),
                position_type: PositionType::Relative,
                ..default()
            },
            BackgroundColor(base),
            Interaction::default(),
            SearchRow {
                input,
                category: cat.to_string(),
                label_lc: entry.label.to_lowercase(),
                order,
            },
            SearchAction(entry.action),
            Name::new("search-row"),
        ))
        .id();
    bind_bg(commands, row, move |w| {
        if matches!(
            w.get::<Interaction>(row),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        ) {
            over(base, Color::srgba(1.0, 1.0, 1.0, 0.06))
        } else {
            base
        }
    });
    // Indent guide line.
    let guide = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Px(1.5),
                ..default()
            },
            BackgroundColor(rgb(GUIDE)),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    let glyph = icon_glyph(&entry.icon).unwrap_or_else(|| entry.icon.chars().next().unwrap_or('\u{E4C6}'));
    let ic = commands
        .spawn((
            Text::new(glyph.to_string()),
            TextFont {
                font: bevy::text::FontSource::Handle(fonts.phosphor.clone()),
                font_size: bevy::text::FontSize::Px(14.0),
                ..default()
            },
            TextColor(rgb(accent)),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    let label = commands
        .spawn((
            Text::new(entry.label),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(TEXT)),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    commands.entity(row).add_children(&[guide, ic, label]);
    row
}

fn zebra_bg(i: usize) -> Color {
    if i % 2 == 1 {
        rgb(ZEBRA_ODD)
    } else {
        Color::NONE
    }
}

// ── Systems ──────────────────────────────────────────────────────────────────

/// Auto-focus the search field when a list is first built.
pub(crate) fn search_list_focus(mut q: Query<&mut EmberTextInput, Added<SearchInput>>) {
    for mut inp in &mut q {
        inp.focused = true;
    }
}

/// Filter rows by search text, honor category collapse (search force-expands),
/// hide empty category headers, and keep carets in sync.
pub(crate) fn search_list_visibility(
    inputs: Query<(Entity, &EmberTextInput), With<SearchInput>>,
    cats: Query<&SearchCat>,
    mut rows: Query<(&SearchRow, &mut Node), Without<SearchCat>>,
    mut cat_nodes: Query<(&SearchCat, &mut Node), Without<SearchRow>>,
    mut texts: Query<&mut Text>,
) {
    if inputs.is_empty() {
        return;
    }
    let queries: HashMap<Entity, String> = inputs
        .iter()
        .map(|(e, i)| (e, i.value.to_lowercase()))
        .collect();
    let expanded: HashMap<(Entity, String), bool> = cats
        .iter()
        .map(|c| ((c.input, c.category.clone()), c.expanded))
        .collect();

    // Rows: show if it matches AND (search active OR its category is expanded).
    let mut has_match: HashSet<(Entity, String)> = HashSet::new();
    for (row, mut node) in &mut rows {
        let q = queries.get(&row.input);
        let matches = q.map(|q| q.is_empty() || row.label_lc.contains(q)).unwrap_or(true);
        let search_active = q.map(|q| !q.is_empty()).unwrap_or(false);
        let exp = *expanded.get(&(row.input, row.category.clone())).unwrap_or(&true);
        let show = matches && (search_active || exp);
        let want = if show { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
        if matches {
            has_match.insert((row.input, row.category.clone()));
        }
    }
    // Headers: visible if their category has any matching row; caret reflects state.
    for (cat, mut node) in &mut cat_nodes {
        let want = if has_match.contains(&(cat.input, cat.category.clone())) {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != want {
            node.display = want;
        }
        let search_active = queries.get(&cat.input).map(|q| !q.is_empty()).unwrap_or(false);
        let open = search_active || cat.expanded;
        let glyph = icon_glyph(if open { "caret-down" } else { "caret-right" }).unwrap_or('v');
        if let Ok(mut t) = texts.get_mut(cat.caret) {
            let s = glyph.to_string();
            if t.0 != s {
                t.0 = s;
            }
        }
    }
}

/// Click a category header → toggle its collapse (ignored while searching).
pub(crate) fn search_cat_toggle(
    changed: Query<(Entity, &Interaction), (Changed<Interaction>, With<SearchCat>)>,
    inputs: Query<&EmberTextInput, With<SearchInput>>,
    mut cats: Query<&mut SearchCat>,
) {
    for (e, interaction) in &changed {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Ok(mut cat) = cats.get_mut(e) else {
            continue;
        };
        let searching = inputs.get(cat.input).map(|i| !i.value.is_empty()).unwrap_or(false);
        if !searching {
            cat.expanded = !cat.expanded;
        }
    }
}

/// Click a sidebar category → isolate it (expand it, collapse the others).
pub(crate) fn search_sidebar_jump(
    changed: Query<(&Interaction, &SearchSidebarCat), Changed<Interaction>>,
    mut cats: Query<&mut SearchCat>,
) {
    for (interaction, sc) in &changed {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match &sc.category {
            // "All" → expand everything.
            None => {
                for mut cat in &mut cats {
                    cat.expanded = true;
                }
            }
            // A category → isolate it (expand it, collapse the rest).
            Some(target) => {
                for mut cat in &mut cats {
                    cat.expanded = &cat.category == target;
                }
            }
        }
    }
}

/// Click a row → run its action (deferred); if inside an [`Overlay`], close it.
pub(crate) fn search_list_select(
    q: Query<(Entity, &Interaction), (Changed<Interaction>, With<SearchAction>)>,
    parents: Query<&ChildOf>,
    overlays: Query<(), With<Overlay>>,
    menus: Query<(), With<super::popup::ScreenMenu>>,
    mut commands: Commands,
) {
    for (e, interaction) in &q {
        if *interaction == Interaction::Pressed {
            run_and_close(e, &parents, &overlays, &menus, &mut commands);
        }
    }
}

/// Enter → select the first visible row (by build order).
pub(crate) fn search_list_enter(
    keys: Res<ButtonInput<KeyCode>>,
    rows: Query<(Entity, &Node, &SearchRow)>,
    parents: Query<&ChildOf>,
    overlays: Query<(), With<Overlay>>,
    menus: Query<(), With<super::popup::ScreenMenu>>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::Enter) {
        return;
    }
    let mut best: Option<(usize, Entity)> = None;
    for (e, node, row) in &rows {
        if node.display == Display::None {
            continue;
        }
        if best.is_none_or(|(o, _)| row.order < o) {
            best = Some((row.order, e));
        }
    }
    if let Some((_, e)) = best {
        run_and_close(e, &parents, &overlays, &menus, &mut commands);
    }
}

fn run_and_close(
    e: Entity,
    parents: &Query<&ChildOf>,
    overlays: &Query<(), With<Overlay>>,
    menus: &Query<(), With<super::popup::ScreenMenu>>,
    commands: &mut Commands,
) {
    commands.queue(move |world: &mut World| {
        let action = world
            .get_entity_mut(e)
            .ok()
            .and_then(|mut em| em.take::<SearchAction>());
        if let Some(action) = action {
            (action.0)(world);
        }
    });
    // Close the enclosing floating surface — a centered `Overlay` or a
    // cursor-anchored `ScreenMenu` (the search_menu palette), whichever we hit
    // first walking up to the root.
    let mut cur = e;
    loop {
        if overlays.get(cur).is_ok() || menus.get(cur).is_ok() {
            commands.entity(cur).despawn();
            break;
        }
        match parents.get(cur) {
            Ok(c) => cur = c.parent(),
            Err(_) => break,
        }
    }
}
