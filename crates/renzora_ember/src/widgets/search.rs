//! Search list — a reusable search box over a category-grouped, filterable list
//! of action rows. Standalone (returns a container you place anywhere); typing
//! filters it, clicking a row runs its `&mut World` action. If it happens to sit
//! inside an [`super::overlay::Overlay`], selecting a row also closes that
//! overlay. Compose with `overlay` for pickers, or drop it into a panel/popup.

use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;

use crate::font::{icon_text, ui_font, EmberFonts};
use crate::reactive::bind_bg;
use crate::theme::rgb;

use super::overlay::Overlay;
use super::scroll_area::scroll_view;
use super::text_input::{text_input, EmberTextInput};

const ROW_HOVER: (u8, u8, u8) = (46, 46, 56);
const TEXT: (u8, u8, u8) = (224, 224, 234);
const MUTED: (u8, u8, u8) = (150, 150, 164);

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
    let (root, content) = super::overlay::overlay(commands, fonts, title);
    let list = search_list(commands, fonts, entries);
    commands.entity(content).add_child(list);
    root
}

#[derive(Component)]
pub(crate) struct SearchInput;

#[derive(Component)]
pub(crate) struct SearchRow {
    input: Entity,
    label_lc: String,
    category: String,
}

#[derive(Component)]
pub(crate) struct SearchCatHeader {
    input: Entity,
    category: String,
}

#[derive(Component)]
pub(crate) struct SearchAction(Box<dyn Fn(&mut World) + Send + Sync>);

/// Build a search box + category-grouped filterable list. Returns the container.
pub fn search_list(commands: &mut Commands, fonts: &EmberFonts, entries: Vec<SearchEntry>) -> Entity {
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

    let input = text_input(commands, &fonts.ui, "Search…", "");
    commands.entity(input).insert((
        SearchInput,
        Node {
            width: Val::Percent(100.0),
            min_width: Val::Px(0.0),
            margin: UiRect::all(Val::Px(8.0)),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
    ));

    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            padding: UiRect::new(Val::Px(4.0), Val::Px(4.0), Val::Px(0.0), Val::Px(6.0)),
            ..default()
        })
        .id();

    let mut seen: Vec<String> = Vec::new();
    let mut rows: Vec<Entity> = Vec::new();
    for entry in entries {
        if !seen.contains(&entry.category) {
            seen.push(entry.category.clone());
            rows.push(category_header(commands, fonts, input, &entry.category));
        }
        rows.push(entry_row(commands, fonts, input, entry));
    }
    if rows.is_empty() {
        rows.push(
            commands
                .spawn((
                    Text::new("Nothing available"),
                    ui_font(&fonts.ui, 11.0),
                    TextColor(rgb(MUTED)),
                    Node {
                        padding: UiRect::all(Val::Px(10.0)),
                        ..default()
                    },
                ))
                .id(),
        );
    }
    commands.entity(list).add_children(&rows);
    let body = scroll_view(commands, list);

    commands.entity(col).add_children(&[input, body]);
    col
}

fn category_header(commands: &mut Commands, fonts: &EmberFonts, input: Entity, category: &str) -> Entity {
    commands
        .spawn((
            Text::new(category.to_uppercase()),
            ui_font(&fonts.ui, 9.0),
            TextColor(rgb(MUTED)),
            Node {
                padding: UiRect::new(Val::Px(8.0), Val::Px(0.0), Val::Px(8.0), Val::Px(3.0)),
                ..default()
            },
            SearchCatHeader {
                input,
                category: category.to_string(),
            },
        ))
        .id()
}

fn entry_row(commands: &mut Commands, fonts: &EmberFonts, input: Entity, entry: SearchEntry) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            SearchRow {
                input,
                label_lc: entry.label.to_lowercase(),
                category: entry.category.clone(),
            },
            SearchAction(entry.action),
            Name::new("search-row"),
        ))
        .id();
    bind_bg(commands, row, move |w| match w.get::<Interaction>(row) {
        Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(ROW_HOVER),
        _ => Color::NONE,
    });
    let ic = icon_text(commands, &fonts.phosphor, &entry.icon, entry.color, 13.0);
    let label = commands
        .spawn((
            Text::new(entry.label),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(TEXT)),
        ))
        .id();
    commands.entity(row).add_children(&[ic, label]);
    row
}

/// Auto-focus the search field when a list is first built.
pub(crate) fn search_list_focus(mut q: Query<&mut EmberTextInput, Added<SearchInput>>) {
    for mut inp in &mut q {
        inp.focused = true;
    }
}

/// Filter each list's rows by its own search text; hide empty category headers.
pub(crate) fn search_list_filter(
    inputs: Query<(Entity, &EmberTextInput), With<SearchInput>>,
    mut rows: Query<(&SearchRow, &mut Node), Without<SearchCatHeader>>,
    mut headers: Query<(&SearchCatHeader, &mut Node), Without<SearchRow>>,
) {
    if inputs.is_empty() {
        return;
    }
    let queries: HashMap<Entity, String> = inputs
        .iter()
        .map(|(e, i)| (e, i.value.to_lowercase()))
        .collect();
    let mut visible: HashSet<(Entity, String)> = HashSet::new();
    for (row, mut node) in &mut rows {
        let show = queries
            .get(&row.input)
            .map(|q| q.is_empty() || row.label_lc.contains(q))
            .unwrap_or(true);
        let want = if show { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
        if show {
            visible.insert((row.input, row.category.clone()));
        }
    }
    for (header, mut node) in &mut headers {
        let want = if visible.contains(&(header.input, header.category.clone())) {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != want {
            node.display = want;
        }
    }
}

/// Click a row → run its action (deferred); if the list sits inside an
/// [`Overlay`], close that overlay too.
pub(crate) fn search_list_select(
    q: Query<(Entity, &Interaction), (Changed<Interaction>, With<SearchAction>)>,
    parents: Query<&ChildOf>,
    overlays: Query<(), With<Overlay>>,
    mut commands: Commands,
) {
    for (e, interaction) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        commands.queue(move |world: &mut World| {
            let action = world
                .get_entity_mut(e)
                .ok()
                .and_then(|mut em| em.take::<SearchAction>());
            if let Some(action) = action {
                (action.0)(world);
            }
        });
        // Close the enclosing overlay, if any.
        let mut cur = e;
        loop {
            if overlays.contains(cur) {
                commands.entity(cur).despawn();
                break;
            }
            match parents.get(cur) {
                Ok(c) => cur = c.parent(),
                Err(_) => break,
            }
        }
    }
}
