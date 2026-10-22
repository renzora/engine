//! Search list — a reusable, category-grouped, filterable picker (the ember port
//! of the egui search overlay). A search box over a two-pane body: a left
//! category sidebar and a right pane of collapsible category sections with
//! accent-colored headers and per-item actions. Typing filters (and
//! force-expands); Enter picks the first match; clicking an item runs its
//! `&mut World` action and, if the list is inside an
//! [`super::overlay::Overlay`], closes it.
//!
//! The sidebar is a table of contents, not a filter: clicking a category
//! scrolls its section to the top of the list (opening it if it was
//! collapsed) and leaves every other section as it was, and the sidebar marks
//! whichever section is at the top as you scroll. It used to isolate the
//! category by collapsing the rest, which hid what was next to it and made
//! the long list feel like a set of tabs. A section header still collapses
//! its own section, and the section slides shut rather than vanishing.
//!
//! The centred overlay ([`search_overlay`], Add Component and Add Entity) lays
//! each section out as a wrapping grid of tiles; the compact cursor-anchored
//! palette ([`search_menu`]) keeps one row per item, since a 300px panel has no
//! room for a grid. Neither is striped: the zebra rows and indent guides went
//! with the rest of the editor's striping, and a hue-washed tile already marks
//! where one item ends and the next begins.

use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;

use crate::font::{icon_glyph, ui_font, EmberFonts};
use crate::interaction::{BLOCK, PASS_THROUGH};
use crate::reactive::tracked::bind_bg;
use crate::theme::*;
use crate::UiInteraction;

use super::overlay::Overlay;
use super::scroll_area::scroll_view_bar;
use super::text_input::{text_input, EmberTextInput};

const TEXT: (u8, u8, u8) = (224, 224, 234);
const MUTED: (u8, u8, u8) = (150, 150, 164);
const SIDEBAR_BG: (u8, u8, u8) = crate::theme::WINDOW_BG;

/// A grid chip's share of its row: three to a line, sized as a percentage so
/// the grid always fills the list's width instead of leaving a ragged strip
/// on the right. The spacing between chips is each cell's own padding, since
/// a gap would push three 33% cells past 100%.
const TILE_PCT: f32 = 100.0 / 3.0;

/// A stable accent color per category (ember port of `category_colors`).
///
/// Public so anything presenting the *same* categories in another shape — the
/// hierarchy's right-click quick-add menu, say — tints them identically to this
/// overlay instead of inventing a second palette.
///
/// A name that is an Inspector page ("Post Process", "lighting", ...) takes
/// that page's hue from `renzora::inspector_pages`, so a category is the same
/// colour in Add Component, Add Entity and the Inspector's page rail.
pub fn category_color(name: &str) -> (u8, u8, u8) {
    let norm = name.trim().to_lowercase().replace([' ', '-'], "_");
    if let Some(page) = renzora::inspector_pages::InspectorPage::from_id(&norm) {
        return page.hue();
    }
    match norm.as_str() {
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

/// One pickable entry in a [`search_list`].
pub struct SearchEntry {
    pub icon: String,
    pub label: String,
    pub category: String,
    pub color: (u8, u8, u8),
    pub action: Box<dyn Fn(&mut World) + Send + Sync>,
    /// A short hint drawn at the right of the row in small muted type, such
    /// as a node's pins ("UV > RGBA"). `None` draws nothing.
    pub detail: Option<String>,
    /// One line about the item, shown in the footer of a [`search_menu`]
    /// while the item is hovered (or is the first match).
    pub description: Option<String>,
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
            detail: None,
            description: None,
        }
    }

    pub fn colored(mut self, color: (u8, u8, u8)) -> Self {
        self.color = color;
        self
    }

    /// See [`SearchEntry::detail`].
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        let d = detail.into();
        self.detail = (!d.is_empty()).then_some(d);
        self
    }

    /// See [`SearchEntry::description`].
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        let d = description.into();
        self.description = (!d.is_empty()).then_some(d);
        self
    }
}

/// Convenience: a centered [`Overlay`] titled `title` whose content is a
/// [`search_list`] laid out as tiles, one wrapping grid per category.
///
/// A grid rather than the list it used to be: these pickers offer a hundred
/// or more things, and a column of 12px rows under coloured headers read as a
/// wall of text to scan top to bottom. Tiles carry the category's colour in
/// their icon badge, fit five to a line, and put three times as many choices
/// on screen at once.
pub fn search_overlay(
    commands: &mut Commands,
    fonts: &EmberFonts,
    title: &str,
    entries: Vec<SearchEntry>,
) -> Entity {
    search_overlay_focused(commands, fonts, title, entries, None)
}

/// [`search_overlay`] opened on one category: the list starts scrolled to
/// `focus` (a category name as the entries carry it), exactly as if its
/// sidebar entry had been clicked. Every other section is still there above
/// and below it. A `focus` that no entry carries opens at the top.
///
/// The Inspector opens Add Component this way, on the page you are looking
/// at: adding a collider while on Physics should not start from a list of
/// lights.
pub fn search_overlay_focused(
    commands: &mut Commands,
    fonts: &EmberFonts,
    title: &str,
    entries: Vec<SearchEntry>,
    focus: Option<&str>,
) -> Entity {
    let (root, content) = super::overlay::overlay_sized(commands, fonts, title, 720.0, 580.0, true);
    let has_focus = focus.is_some_and(|f| entries.iter().any(|e| e.category == f));
    let (list, input) = search_list_impl(commands, fonts, entries, true);
    commands.entity(content).add_child(list);
    if let (true, Some(focus)) = (has_focus, focus) {
        let focus = focus.to_string();
        commands.queue(move |world: &mut World| {
            let mut headers = world.query::<(Entity, &SearchCat)>();
            let Some(header) = headers
                .iter(world)
                .find(|(_, c)| c.input == input && c.category == focus)
                .map(|(e, _)| e)
            else {
                return;
            };
            let mut panes = world.query::<&mut SearchPane>();
            if let Some(mut pane) = panes.iter_mut(world).find(|p| p.input == input) {
                pane.jump = Jump::To(header);
                pane.clicked = Some(focus);
            }
        });
    }
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

/// A sidebar button that scrolls to a category's section; `None` is the
/// "All" entry (opens every section and scrolls to the top). `input` ties it
/// to its list, so two pickers open at once each paint their own selection.
/// `header` is the section header it scrolls to.
#[derive(Component)]
pub(crate) struct SearchSidebarCat {
    input: Entity,
    category: Option<String>,
    header: Option<Entity>,
}

/// One category's items, under its header. Clips its content and animates its
/// own height between nothing and the content's, which is the slide.
///
/// `open` runs 0 (shut) to 1 (open). At 1 the height goes back to `Auto`, so
/// a section that is not moving follows its content (search hiding tiles)
/// without anyone measuring it.
#[derive(Component)]
pub(crate) struct SearchSection {
    input: Entity,
    category: String,
    open: f32,
}

/// Where a list has been asked to scroll.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Jump {
    Idle,
    Top,
    To(Entity),
}

/// The scrolling list's state, on the list node (the scroll viewport is its
/// parent).
///
/// `jump` is held rather than applied once: the header's position is only
/// known after layout, and sections above it may still be sliding, which moves
/// it. [`search_scroll_spy`] re-aims every frame until both have settled.
/// `clicked` is the category last picked in the sidebar: it stays selected
/// while its header is on screen, which covers the sections near the bottom
/// that can never scroll all the way to the top.
#[derive(Component)]
pub(crate) struct SearchPane {
    input: Entity,
    jump: Jump,
    clicked: Option<String>,
    /// The sidebar entry to mark: `None` is "All".
    selected: Option<String>,
}

/// How long a section takes to slide open or shut.
const SLIDE_SECS: f32 = 0.16;

#[derive(Component)]
pub(crate) struct SearchRow {
    input: Entity,
    category: String,
    label_lc: String,
    order: usize,
    /// For a [`search_menu`]'s footer: the shown name and its description.
    label: String,
    description: String,
}

/// A [`search_menu`]'s footer: the name and description of the item under
/// the cursor, or of the first match when nothing is hovered (the one Enter
/// would pick).
#[derive(Component)]
pub(crate) struct SearchFooter {
    input: Entity,
    title: Entity,
    body: Entity,
}

#[derive(Component)]
pub(crate) struct SearchAction(Box<dyn Fn(&mut World) + Send + Sync>);

/// Build a search box + two-pane category-grouped filterable list, one row per
/// item. Returns the container.
pub fn search_list(commands: &mut Commands, fonts: &EmberFonts, entries: Vec<SearchEntry>) -> Entity {
    search_list_impl(commands, fonts, entries, false).0
}

/// [`search_list`], with `tiles` choosing a wrapping grid of tiles per
/// category instead of rows. Everything else (search, sidebar, collapse,
/// Enter, the click) is the same machinery: a tile carries the same
/// [`SearchRow`] / [`SearchAction`] a row does. Returns the container and its
/// search input, which keys every section and sidebar entry of this list.
fn search_list_impl(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entries: Vec<SearchEntry>,
    tiles: bool,
) -> (Entity, Entity) {
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
            padding: if tiles {
                UiRect::new(Val::Px(8.0), Val::Px(8.0), Val::Px(2.0), Val::Px(8.0))
            } else {
                UiRect::vertical(Val::Px(2.0))
            },
            ..default()
        })
        .id();

    let mut list_kids: Vec<Entity> = Vec::new();
    let mut sidebar_kids: Vec<Entity> = Vec::new();
    let mut row_order: usize = 0;

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

    // "All" opens every section and scrolls to the top.
    let all_label = renzora::lang::t_or("entity.cat.all", "All");
    sidebar_kids.push(sidebar_button(commands, fonts, input, &all_label, None, None, None));

    for (cat, items) in order.iter().zip(groups) {
        let accent = category_color(cat);

        // Category header (caret, accent label, muted count; clickable).
        let header = category_header(commands, fonts, input, cat, accent, items.len());
        list_kids.push(header);

        // Sidebar button, which scrolls to that header.
        let sb = sidebar_button(
            commands,
            fonts,
            input,
            &capitalize(cat),
            Some(accent),
            Some(cat.clone()),
            Some(header),
        );
        // How many items the category holds, at the entry's right edge.
        let n = commands
            .spawn((
                Text::new(items.len().to_string()),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(MUTED)),
                Node { margin: UiRect::left(Val::Auto), ..default() },
                PASS_THROUGH,
            ))
            .id();
        commands.entity(sb).add_child(n);
        sidebar_kids.push(sb);

        // The section's items: tiles wrap in a row, rows stack in a column.
        // `flex_shrink: 0` keeps the content at its natural height inside a
        // section that is mid-slide, which is the height the slide measures.
        let content = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_shrink: 0.0,
                    flex_direction: if tiles { FlexDirection::Row } else { FlexDirection::Column },
                    flex_wrap: if tiles { FlexWrap::Wrap } else { FlexWrap::NoWrap },
                    padding: if tiles {
                        UiRect::new(Val::Px(0.0), Val::Px(0.0), Val::Px(2.0), Val::Px(8.0))
                    } else {
                        UiRect::bottom(Val::Px(4.0))
                    },
                    ..default()
                },
                Name::new(if tiles { "search-tiles" } else { "search-rows" }),
            ))
            .id();
        let mut cells = Vec::new();
        for entry in items {
            cells.push(if tiles {
                item_tile(commands, fonts, input, cat, accent, row_order, entry)
            } else {
                item_row(commands, fonts, input, cat, accent, row_order, entry)
            });
            row_order += 1;
        }
        commands.entity(content).add_children(&cells);
        let section = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    overflow: Overflow::clip(),
                    ..default()
                },
                SearchSection {
                    input,
                    category: cat.clone(),
                    open: 1.0,
                },
                Name::new("search-section"),
            ))
            .id();
        commands.entity(section).add_child(content);
        list_kids.push(section);
    }

    commands.entity(list).add_children(&list_kids);
    commands.entity(list).insert(SearchPane {
        input,
        jump: Jump::Idle,
        clicked: None,
        selected: None,
    });
    let scroll = scroll_view_bar(commands, list);

    // The sidebar scrolls on its own. Add Panel has more categories than fit
    // in the overlay's height, and as a plain column the bottom of the list
    // was cut off with no way to reach it.
    let sidebar = commands
        .spawn((
            Node {
                width: Val::Px(SIDEBAR_W),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                min_height: Val::Px(0.0),
                ..default()
            },
            BackgroundColor(rgb(SIDEBAR_BG)),
            Name::new("search-sidebar"),
        ))
        .id();
    let sidebar_list = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::new(Val::Px(6.0), Val::Px(8.0), Val::Px(8.0), Val::Px(8.0)),
                row_gap: Val::Px(2.0),
                ..default()
            },
            Name::new("search-sidebar-list"),
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
    commands.entity(sidebar_list).add_children(&sb);
    let sidebar_scroll = scroll_view_bar(commands, sidebar_list);
    commands.entity(sidebar).add_child(sidebar_scroll);

    // A drag bar between the categories and the list: a long category name
    // ("Post Process", "Procedural") wants a wider column than a short list
    // does, and which matters more depends on what you are looking for.
    let split = commands
        .spawn((
            Node { width: Val::Px(5.0), height: Val::Percent(100.0), flex_shrink: 0.0, justify_content: JustifyContent::Center, ..default() },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::EwResize),
            SearchSplit { sidebar, grab: None },
            Name::new("search-sidebar-split"),
        ))
        .id();
    let split_line = commands
        .spawn((Node { width: Val::Px(1.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(rgb(border())), PASS_THROUGH))
        .id();
    bind_bg(commands, split_line, move |w| {
        let live = w.get::<SearchSplit>(split).is_some_and(|s| s.grab.is_some())
            || matches!(w.get::<UiInteraction>(split), Some(UiInteraction::Hovered | UiInteraction::Pressed));
        rgb(if live { accent() } else { border() })
    });
    commands.entity(split).add_child(split_line);

    commands.entity(body).add_children(&[sidebar, split, scroll]);
    commands.entity(col).add_children(&[search_bar, body]);
    (col, input)
}

/// The category column's starting width, and its drag limits.
const SIDEBAR_W: f32 = 150.0;
const SIDEBAR_MIN: f32 = 100.0;
const SIDEBAR_MAX: f32 = 320.0;
/// The add-node menu's smallest size under its resize grip.
const MENU_MIN: Vec2 = Vec2::new(340.0, 260.0);

/// The drag bar between a list's categories and its items. `grab` holds the
/// cursor x and column width at the press while a drag is under way.
#[derive(Component)]
pub(crate) struct SearchSplit {
    sidebar: Entity,
    grab: Option<(f32, f32)>,
}

/// A [`search_menu`]'s bottom-right resize grip. `grab` holds the cursor and
/// the menu's size at the press.
#[derive(Component)]
pub(crate) struct SearchMenuGrip {
    menu: Entity,
    grab: Option<(Vec2, Vec2)>,
}

fn px_or(v: Val, fallback: f32) -> f32 {
    match v {
        Val::Px(p) => p,
        _ => fallback,
    }
}

/// Drag the category split and the menu's resize grip. Tracked against the
/// raw mouse button once grabbed, so the drag keeps going when the cursor
/// runs ahead of the handle.
pub(crate) fn search_resize(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    ui_scale: Option<Res<UiScale>>,
    mut splits: Query<(&UiInteraction, &mut SearchSplit)>,
    mut grips: Query<(&UiInteraction, &mut SearchMenuGrip), Without<SearchSplit>>,
    mut nodes: Query<&mut Node, (Without<SearchSplit>, Without<SearchMenuGrip>)>,
) {
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else { return };
    let scale = ui_scale.map(|s| s.0).unwrap_or(1.0).max(0.01);
    let held = mouse.pressed(MouseButton::Left);
    for (i, mut s) in &mut splits {
        if !held {
            s.grab = None;
            continue;
        }
        if s.grab.is_none() && *i == UiInteraction::Pressed && mouse.just_pressed(MouseButton::Left) {
            let w = nodes.get(s.sidebar).map(|n| px_or(n.width, SIDEBAR_W)).unwrap_or(SIDEBAR_W);
            s.grab = Some((cursor.x, w));
        }
        if let (Some((x0, w0)), Ok(mut n)) = (s.grab, nodes.get_mut(s.sidebar)) {
            let w = Val::Px((w0 + (cursor.x - x0) / scale).clamp(SIDEBAR_MIN, SIDEBAR_MAX));
            if n.width != w {
                n.width = w;
            }
        }
    }
    for (i, mut g) in &mut grips {
        if !held {
            g.grab = None;
            continue;
        }
        if g.grab.is_none() && *i == UiInteraction::Pressed && mouse.just_pressed(MouseButton::Left) {
            let size = nodes
                .get(g.menu)
                .map(|n| Vec2::new(px_or(n.width, 460.0), px_or(n.height, 420.0)))
                .unwrap_or(Vec2::new(460.0, 420.0));
            g.grab = Some((cursor, size));
        }
        if let (Some((c0, s0)), Ok(mut n)) = (g.grab, nodes.get_mut(g.menu)) {
            let s = (s0 + (cursor - c0) / scale).max(MENU_MIN);
            let (w, h) = (Val::Px(s.x), Val::Px(s.y));
            if n.width != w {
                n.width = w;
            }
            if n.height != h {
                n.height = h;
            }
        }
    }
}

/// A floating, cursor-anchored **searchable** picker — the "right-click /
/// Spacebar to add a node" palette. Same typeahead + category list as
/// [`search_list`], but in a compact [`ScreenMenu`](super::popup::ScreenMenu)
/// panel positioned at window `(x, y)` instead of a big centered overlay. Being a
/// `ScreenMenu` it gets click-outside dismiss + on-screen clamping for free, and
/// picking a row runs its action and closes the panel (see [`run_and_close`]).
///
/// 460px wide so a node's name fits on one line next to its pin hint (at
/// 300px names wrapped onto two lines), with a footer naming the item under
/// the cursor and saying what it does.
pub fn search_menu(commands: &mut Commands, fonts: &EmberFonts, x: f32, y: f32, entries: Vec<SearchEntry>) -> Entity {
    let (list, input) = search_list_impl(commands, fonts, entries, false);
    let footer = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                min_height: Val::Px(48.0),
                border: UiRect::top(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(rgb(faint_bg())),
            BorderColor::all(rgb(border())),
            Name::new("search-menu-footer"),
        ))
        .id();
    let mut title_font = ui_font(&fonts.ui, 12.0);
    title_font.weight = bevy::text::FontWeight::SEMIBOLD;
    let title = commands
        .spawn((Text::new(""), title_font, TextColor(rgb(TEXT)), bevy::text::TextLayout::no_wrap()))
        .id();
    let body = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(MUTED))))
        .id();
    commands.entity(footer).add_children(&[title, body]);
    commands.entity(footer).insert(SearchFooter { input, title, body });
    commands.entity(list).add_child(footer);
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x),
                top: Val::Px(y),
                width: Val::Px(460.0),
                height: Val::Px(420.0),
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
            BLOCK,
            bevy::ui::RelativeCursorPosition::default(),
            Name::new("search-menu"),
        ))
        .id();
    // The resize grip: a small corner mark, over the footer.
    let grip = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Px(16.0),
                height: Val::Px(16.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            UiInteraction::default(),
            crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::NwseResize),
            SearchMenuGrip { menu: root, grab: None },
            Name::new("search-menu-grip"),
        ))
        .id();
    let grip_glyph = commands
        .spawn((
            Text::new(icon_glyph("arrows-out-simple").unwrap_or('+').to_string()),
            TextFont {
                font: bevy::text::FontSource::Handle(fonts.phosphor.clone()),
                font_size: bevy::text::FontSize::Px(10.0),
                ..default()
            },
            TextColor(rgb(MUTED)),
            PASS_THROUGH,
        ))
        .id();
    commands.entity(grip).add_child(grip_glyph);
    commands.entity(root).add_children(&[list, grip]);
    root
}

/// The panel picker: the same chip-grid overlay as Add Component and Add
/// Entity ([`search_overlay`]). It used to be its own four-column layout,
/// which made the three pickers that add things look like three different
/// widgets; one look means one set of habits.
pub fn grid_overlay(
    commands: &mut Commands,
    fonts: &EmberFonts,
    title: &str,
    entries: Vec<SearchEntry>,
) -> Entity {
    search_overlay(commands, fonts, title, entries)
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
            kids.push(category_header_styled(commands, fonts, input, &cat, accent, items.len(), false));
            for entry in items {
                kids.push(item_row(commands, fonts, input, &cat, accent, order_idx, entry));
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

/// A sidebar entry: a dot in the category's hue, then its name. `accent` is
/// `None` for "All", which has no hue of its own and so no dot (its label
/// keeps the dot's room, so the names line up).
///
/// The names used to be the coloured thing, which made a column of fourteen
/// differently coloured words. The dot carries the colour now and the names
/// are ordinary text, and the selected category is a soft accent pill, painted
/// by [`search_sidebar_paint`].
fn sidebar_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    input: Entity,
    label: &str,
    accent: Option<(u8, u8, u8)>,
    category: Option<String>,
    header: Option<Entity>,
) -> Entity {
    let b = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(7.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            SearchSidebarCat { input, category, header },
            Name::new("search-sidebar-cat"),
        ))
        .id();
    let dot = commands
        .spawn((
            Node {
                width: Val::Px(7.0),
                height: Val::Px(7.0),
                flex_shrink: 0.0,
                border_radius: BorderRadius::all(Val::Px(3.5)),
                ..default()
            },
            BackgroundColor(accent.map_or(Color::NONE, rgb)),
            PASS_THROUGH,
        ))
        .id();
    let t = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(TEXT)),
            bevy::text::TextLayout::no_wrap(),
            PASS_THROUGH,
        ))
        .id();
    commands.entity(b).add_children(&[dot, t]);
    b
}

fn category_header(
    commands: &mut Commands,
    fonts: &EmberFonts,
    input: Entity,
    cat: &str,
    accent: (u8, u8, u8),
    count: usize,
) -> Entity {
    category_header_styled(commands, fonts, input, cat, accent, count, true)
}

/// A section header: a small muted caret and the category's name in its hue,
/// with a quiet item count after it. Nothing else: no band, no rule.
///
/// This is the original picker's header, restored on purpose. Two attempts to
/// make sections stand out more both read worse: a hairline rule out to the
/// right edge looked like a stray line through the list, and a rounded band
/// washed in the category hue made every section a coloured slab that
/// outshouted the items in it. The hue on the name already marks where a
/// section starts; the count stays at a muted 10 so it never competes with it.
///
/// `collapsible` headers (list mode) get a caret, a hover wash and
/// click-to-collapse; the column grid uses plain headers (no caret), matching
/// the egui panel picker.
#[allow(clippy::too_many_arguments)]
fn category_header_styled(
    commands: &mut Commands,
    fonts: &EmberFonts,
    input: Entity,
    cat: &str,
    accent: (u8, u8, u8),
    count: usize,
    collapsible: bool,
) -> Entity {
    let mut header = commands.spawn((
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            width: Val::Percent(100.0),
            flex_shrink: 0.0,
            column_gap: Val::Px(6.0),
            padding: UiRect::axes(Val::Px(6.0), Val::Px(5.0)),
            margin: UiRect::top(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(Color::NONE),
        Name::new("search-cat"),
    ));
    if collapsible {
        header.insert((
            UiInteraction::default(),
            crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ));
    }
    let header = header.id();
    if collapsible {
        // The theme's hover wash, as a row gets: says "clickable" without
        // turning the header back into a coloured band.
        bind_bg(commands, header, move |w| match w.get::<UiInteraction>(header) {
            Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => menu_hover_bg(),
            _ => Color::NONE,
        });
    }
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
                PASS_THROUGH,
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
            bevy::text::TextLayout::no_wrap(),
            PASS_THROUGH,
        ))
        .id();
    kids.push(label);
    let count = commands
        .spawn((
            Text::new(count.to_string()),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(MUTED)),
            PASS_THROUGH,
        ))
        .id();
    kids.push(count);
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
    order: usize,
    entry: SearchEntry,
) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                width: Val::Percent(100.0),
                column_gap: Val::Px(8.0),
                padding: UiRect::new(Val::Px(20.0), Val::Px(8.0), Val::Px(5.0), Val::Px(5.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            SearchRow {
                input,
                category: cat.to_string(),
                label_lc: entry.label.to_lowercase(),
                order,
                label: entry.label.clone(),
                description: entry.description.clone().unwrap_or_default(),
            },
            SearchAction(entry.action),
            // Named per row, not a flat `"search-row"`: callers that need to
            // point *at a specific entry* (the onboarding tutorial's highlight
            // box asking for "World Environment" or "Demo Panel") have no other
            // handle on a row built inside this widget. The label is the
            // displayed — so localized — one; a name-based lookup that misses
            // simply doesn't highlight.
            Name::new(format!("search-row:{}", entry.label)),
        ))
        .id();
    bind_bg(commands, row, move |w| match w.get::<UiInteraction>(row) {
        Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => menu_hover_bg(),
        _ => Color::NONE,
    });
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
            PASS_THROUGH,
        ))
        .id();
    // One line, always: a name that wrapped onto two lines made the list read
    // as a stack of boxes. Long names end in "..." and show whole on hover.
    let label = commands
        .spawn((
            Text::new(entry.label.clone()),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(TEXT)),
            bevy::text::TextLayout::no_wrap(),
            Node { flex_grow: 1.0, min_width: Val::Px(0.0), overflow: Overflow::clip(), ..default() },
            PASS_THROUGH,
        ))
        .id();
    super::ellipsize(commands, label, entry.label, Some(row));
    let mut kids = vec![ic, label];
    if let Some(detail) = entry.detail {
        kids.push(
            commands
                .spawn((
                    Text::new(detail),
                    ui_font(&fonts.mono, 10.0),
                    TextColor(rgb(MUTED)),
                    bevy::text::TextLayout::no_wrap(),
                    Node { flex_shrink: 0.0, ..default() },
                    PASS_THROUGH,
                ))
                .id(),
        );
    }
    commands.entity(row).add_children(&kids);
    row
}

/// One chip of the overlay's grid: a small square washed with the category's
/// hue holding the item's icon, and the name beside it on one line.
///
/// Three chips to a row. The earlier tiles put a 40px badge over a centred
/// name, five to a row: names wrapped or were cut, a section was mostly empty
/// space, and a screen held half as many items as the plain list it replaced.
/// Side by side, the name reads like a list row while the grid still fills
/// the width and keeps each category's colour. Flat at rest, with a wash of
/// the hue on hover. It carries the same [`SearchRow`] / [`SearchAction`] as a
/// row, so filtering, collapse, Enter and the click need nothing new.
fn item_tile(
    commands: &mut Commands,
    fonts: &EmberFonts,
    input: Entity,
    cat: &str,
    accent: (u8, u8, u8),
    order: usize,
    entry: SearchEntry,
) -> Entity {
    let tile = commands
        .spawn((
            Node {
                width: Val::Percent(TILE_PCT),
                flex_shrink: 0.0,
                // The gutter between chips (see `TILE_PCT`).
                padding: UiRect::all(Val::Px(1.5)),
                ..default()
            },
            UiInteraction::default(),
            crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            SearchRow {
                input,
                category: cat.to_string(),
                label_lc: entry.label.to_lowercase(),
                order,
                label: entry.label.clone(),
                description: entry.description.clone().unwrap_or_default(),
            },
            SearchAction(entry.action),
            // Per item, as the rows are: the onboarding tutorial finds an entry
            // by this name to point at it.
            Name::new(format!("search-row:{}", entry.label)),
        ))
        .id();
    // The visible chip, inside the cell's gutter. Hover reads the cell, which
    // is what carries the interaction.
    let chip = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(32.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::horizontal(Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            PASS_THROUGH,
        ))
        .id();
    bind_bg(commands, chip, move |w| match w.get::<UiInteraction>(tile) {
        Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(accent).with_alpha(0.14),
        _ => Color::NONE,
    });
    let glyph = icon_glyph(&entry.icon).unwrap_or_else(|| entry.icon.chars().next().unwrap_or('\u{E4C6}'));
    let ic = commands
        .spawn((
            Text::new(glyph.to_string()),
            TextFont {
                font: bevy::text::FontSource::Handle(fonts.phosphor.clone()),
                font_size: bevy::text::FontSize::Px(13.0),
                ..default()
            },
            TextColor(rgb(accent)),
            PASS_THROUGH,
        ))
        .id();
    let badge = commands
        .spawn((
            Node {
                width: Val::Px(22.0),
                height: Val::Px(22.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(accent).with_alpha(0.15)),
            PASS_THROUGH,
        ))
        .id();
    commands.entity(badge).add_child(ic);
    let label = commands
        .spawn((
            Text::new(entry.label.clone()),
            // 12, the list row's size: at 13 the chip names sat a ladder rung
            // above the original picker's rows (with any Font Size above the
            // default) and read as too large beside the 13px headers. The "..."
            // below handles the names that still do not fit three across.
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
            PASS_THROUGH,
        ))
        .id();
    super::ellipsize(commands, label, entry.label, Some(tile));
    commands.entity(chip).add_children(&[badge, label]);
    commands.entity(tile).add_child(chip);
    tile
}

// ── Systems ──────────────────────────────────────────────────────────────────

/// Auto-focus the search field when a list is first built.
pub(crate) fn search_list_focus(mut q: Query<&mut EmberTextInput, Added<SearchInput>>) {
    for mut inp in &mut q {
        inp.focused = true;
    }
}

/// Filter rows by search text, hide the headers and sections of categories
/// with no match, and keep carets in sync.
///
/// Collapse is not handled here: a collapsed section keeps its rows and
/// [`search_section_slide`] shuts the section around them. Hiding the rows
/// as well would leave the slide nothing to measure.
pub(crate) fn search_list_visibility(
    inputs: Query<(Entity, &EmberTextInput), With<SearchInput>>,
    mut rows: Query<(&SearchRow, &mut Node), (Without<SearchCat>, Without<SearchSection>)>,
    mut sections: Query<(&SearchSection, &mut Node), (Without<SearchCat>, Without<SearchRow>)>,
    mut cat_nodes: Query<(&SearchCat, &mut Node), (Without<SearchRow>, Without<SearchSection>)>,
    mut texts: Query<&mut Text>,
) {
    if inputs.is_empty() {
        return;
    }
    let queries: HashMap<Entity, String> = inputs
        .iter()
        .map(|(e, i)| (e, i.value.to_lowercase()))
        .collect();

    let mut has_match: HashSet<(Entity, String)> = HashSet::new();
    for (row, mut node) in &mut rows {
        let q = queries.get(&row.input);
        let matches = q.map(|q| q.is_empty() || row.label_lc.contains(q)).unwrap_or(true);
        let want = if matches { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
        if matches {
            has_match.insert((row.input, row.category.clone()));
        }
    }
    // A section with nothing left in it would still show its content's padding.
    for (section, mut node) in &mut sections {
        let want = if has_match.contains(&(section.input, section.category.clone())) {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != want {
            node.display = want;
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
    changed: Query<(Entity, &UiInteraction), (Changed<UiInteraction>, With<SearchCat>)>,
    inputs: Query<&EmberTextInput, With<SearchInput>>,
    mut cats: Query<&mut SearchCat>,
) {
    for (e, interaction) in &changed {
        if *interaction != UiInteraction::Pressed {
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

/// Slide sections open and shut: ease each one's `open` toward its header's
/// state and set its height to that share of its content's.
///
/// Searching opens every section at once, with no slide: the list is
/// reshaping under each keystroke anyway, and a slide on top of that reads as
/// lag.
pub(crate) fn search_section_slide(
    time: Res<Time>,
    inputs: Query<(Entity, &EmberTextInput), With<SearchInput>>,
    cats: Query<&SearchCat>,
    mut sections: Query<(&mut SearchSection, &mut Node, &Children)>,
    computed: Query<&ComputedNode>,
) {
    if sections.is_empty() {
        return;
    }
    let searching: HashSet<Entity> = inputs
        .iter()
        .filter(|(_, i)| !i.value.is_empty())
        .map(|(e, _)| e)
        .collect();
    let expanded: HashMap<(Entity, &str), bool> = cats
        .iter()
        .map(|c| ((c.input, c.category.as_str()), c.expanded))
        .collect();
    let step = time.delta_secs() / SLIDE_SECS;
    for (mut section, mut node, kids) in &mut sections {
        let open = searching.contains(&section.input)
            || *expanded
                .get(&(section.input, section.category.as_str()))
                .unwrap_or(&true);
        let target = if open { 1.0 } else { 0.0 };
        if searching.contains(&section.input) {
            section.open = 1.0;
        } else if section.open != target {
            section.open = if open {
                (section.open + step).min(1.0)
            } else {
                (section.open - step).max(0.0)
            };
        }
        let want = if section.open >= 1.0 {
            Val::Auto
        } else {
            let content_h = kids
                .first()
                .and_then(|k| computed.get(*k).ok())
                .map(|cn| cn.size().y * cn.inverse_scale_factor())
                .unwrap_or(0.0);
            let t = section.open;
            Val::Px(content_h * t * t * (3.0 - 2.0 * t))
        };
        if node.height != want {
            node.height = want;
        }
    }
}

/// Click a sidebar entry: open that section (leaving the others as they are)
/// and scroll to it. "All" opens every section and scrolls to the top.
pub(crate) fn search_sidebar_jump(
    changed: Query<(&UiInteraction, &SearchSidebarCat), Changed<UiInteraction>>,
    mut cats: Query<&mut SearchCat>,
    mut panes: Query<&mut SearchPane>,
) {
    for (interaction, sc) in &changed {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        // Only this list's sections: another picker open at the same time
        // keeps its own state.
        for mut cat in cats.iter_mut().filter(|c| c.input == sc.input) {
            let open = match &sc.category {
                None => true,
                Some(target) => &cat.category == target,
            };
            if open && !cat.expanded {
                cat.expanded = true;
            }
        }
        if let Some(mut pane) = panes.iter_mut().find(|p| p.input == sc.input) {
            pane.jump = sc.header.map_or(Jump::Top, Jump::To);
            pane.clicked = sc.category.clone();
        }
    }
}

/// Drive a pending sidebar jump, and work out which sidebar entry to mark.
///
/// Header positions come from layout, relative to the list's own top edge, so
/// they do not depend on how far the list is scrolled. A jump re-aims every
/// frame until the list has been laid out and no section of it is sliding,
/// since a section opening above the target pushes the target down.
///
/// The marked entry is the clicked category while its header is on screen,
/// otherwise the last section whose header has passed the top of the view, and
/// "All" at the very top.
pub(crate) fn search_scroll_spy(
    mut panes: Query<(&mut SearchPane, &ChildOf, &UiGlobalTransform, &ComputedNode)>,
    mut viewports: Query<(&mut super::scroll_area::EmberScroll, &ScrollPosition, &ComputedNode)>,
    headers: Query<(Entity, &SearchCat, &Node, &UiGlobalTransform, &ComputedNode)>,
    sections: Query<&SearchSection>,
) {
    for (mut pane, child_of, list_gt, list_cn) in &mut panes {
        let Ok((mut scroll, sp, vcn)) = viewports.get_mut(child_of.parent()) else {
            continue;
        };
        let inv = list_cn.inverse_scale_factor();
        let list_top = list_gt.translation.y * inv - list_cn.size().y * inv * 0.5;
        let vh = vcn.size().y * vcn.inverse_scale_factor();

        // Visible headers in list order, as (entity, category, top).
        let mut tops: Vec<(Entity, &str, f32)> = headers
            .iter()
            .filter(|(_, c, node, _, _)| c.input == pane.input && node.display != Display::None)
            .map(|(e, c, _, gt, cn)| {
                let top = gt.translation.y * inv - cn.size().y * inv * 0.5 - list_top;
                (e, c.category.as_str(), top)
            })
            .collect();
        tops.sort_by(|a, b| a.2.total_cmp(&b.2));

        let laid_out = list_cn.size().y > 0.0;
        let sliding = sections
            .iter()
            .any(|s| s.input == pane.input && s.open > 0.0 && s.open < 1.0);
        match pane.jump {
            Jump::Idle => {}
            Jump::Top => scroll.scroll_to(0.0),
            Jump::To(header) => match tops.iter().find(|t| t.0 == header) {
                Some(&(_, _, top)) => scroll.scroll_to(top),
                // Its header is hidden (filtered out by the search): nowhere to go.
                None => pane.jump = Jump::Idle,
            },
        }
        if pane.jump != Jump::Idle && laid_out && !sliding {
            pane.jump = Jump::Idle;
        }

        let y = sp.y;
        let on_screen = |top: f32| top >= y - 1.0 && top < y + vh - 8.0;
        let clicked_visible = pane.clicked.as_deref().is_some_and(|c| {
            tops.iter().any(|t| t.1 == c && on_screen(t.2))
        });
        // Once the clicked section has scrolled away it stops claiming the mark.
        if pane.jump == Jump::Idle && !clicked_visible && pane.clicked.is_some() {
            pane.clicked = None;
        }
        let selected = if clicked_visible {
            pane.clicked.clone()
        } else if y < 1.0 {
            None
        } else {
            tops.iter()
                .rev()
                .find(|t| t.2 <= y + 4.0)
                .map(|t| t.1.to_string())
        };
        if pane.selected != selected {
            pane.selected = selected;
        }
    }
}

/// Paint the sidebar: a soft accent pill on the selected entry, a fainter wash
/// on a hovered one.
///
/// "Selected" is the section at the top of the list, which
/// [`search_scroll_spy`] works out. Only writes a colour that changed, so an
/// idle picker costs a comparison per entry.
pub(crate) fn search_sidebar_paint(
    panes: Query<&SearchPane>,
    mut buttons: Query<(&SearchSidebarCat, &UiInteraction, &mut BackgroundColor)>,
) {
    if buttons.is_empty() {
        return;
    }
    for (sc, interaction, mut bg) in &mut buttons {
        let selected = panes
            .iter()
            .find(|p| p.input == sc.input)
            .is_some_and(|p| p.selected == sc.category);
        let want = if selected {
            menu_hover_bg()
        } else if matches!(interaction, UiInteraction::Hovered | UiInteraction::Pressed) {
            rgb(accent()).with_alpha(0.12)
        } else {
            Color::NONE
        };
        bg.set_if_neq(BackgroundColor(want));
    }
}

/// Fill each [`search_menu`] footer from the hovered row, or the first visible
/// match when nothing is hovered. Writes only on a change.
pub(crate) fn search_footer_update(
    footers: Query<&SearchFooter>,
    rows: Query<(&SearchRow, &Node, &UiInteraction)>,
    mut texts: Query<&mut Text>,
) {
    for footer in &footers {
        let mut hovered: Option<&SearchRow> = None;
        let mut first: Option<&SearchRow> = None;
        for (row, node, i) in &rows {
            if row.input != footer.input || node.display == Display::None {
                continue;
            }
            if matches!(i, UiInteraction::Hovered | UiInteraction::Pressed) {
                hovered = Some(row);
            }
            if first.is_none_or(|f| row.order < f.order) {
                first = Some(row);
            }
        }
        let (title, body) = match hovered.or(first) {
            Some(r) => (r.label.clone(), r.description.clone()),
            None => (String::new(), String::new()),
        };
        for (e, s) in [(footer.title, title), (footer.body, body)] {
            if let Ok(mut t) = texts.get_mut(e) {
                if t.0 != s {
                    t.0 = s;
                }
            }
        }
    }
}

/// Click a row → run its action (deferred); if inside an [`Overlay`], close it.
pub(crate) fn search_list_select(
    q: Query<(Entity, &UiInteraction), (Changed<UiInteraction>, With<SearchAction>)>,
    parents: Query<&ChildOf>,
    overlays: Query<(), With<Overlay>>,
    menus: Query<(), With<super::popup::ScreenMenu>>,
    mut commands: Commands,
) {
    for (e, interaction) in &q {
        if *interaction == UiInteraction::Pressed {
            run_and_close(e, &parents, &overlays, &menus, &mut commands);
        }
    }
}

/// Enter → select the first visible row (by build order).
///
/// A collapsed section keeps its rows displayed (the section clips them; see
/// [`search_list_visibility`]), so those are skipped by their header's state.
/// Searching opens every section, so then nothing is skipped.
#[allow(clippy::too_many_arguments)]
pub(crate) fn search_list_enter(
    keys: Res<ButtonInput<KeyCode>>,
    rows: Query<(Entity, &Node, &SearchRow)>,
    cats: Query<&SearchCat>,
    inputs: Query<&EmberTextInput, With<SearchInput>>,
    parents: Query<&ChildOf>,
    overlays: Query<(), With<Overlay>>,
    menus: Query<(), With<super::popup::ScreenMenu>>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::Enter) {
        return;
    }
    let collapsed: HashSet<(Entity, &str)> = cats
        .iter()
        .filter(|c| !c.expanded)
        .filter(|c| inputs.get(c.input).is_ok_and(|i| i.value.is_empty()))
        .map(|c| (c.input, c.category.as_str()))
        .collect();
    let mut best: Option<(usize, Entity)> = None;
    for (e, node, row) in &rows {
        if node.display == Display::None || collapsed.contains(&(row.input, row.category.as_str())) {
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
