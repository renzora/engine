//! Native hierarchy "filter by component type" — the funnel button + a checkbox
//! popup, migrated from the egui panel. Toggling types fills [`HierFilter`],
//! which the tree snapshot applies via `filter_tree_by_type`.

use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora_editor::ComponentIconRegistry;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::{bind_bg, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{text_input, EmberTextInput, Popup};

/// The set of component-type names (plus the `__other__` sentinel) the tree is
/// filtered to. Empty = show everything.
#[derive(Resource, Default)]
pub(crate) struct HierFilter {
    pub types: HashSet<&'static str>,
}

/// The hierarchy's by-name text filter (the search box).
#[derive(Resource, Default)]
pub(crate) struct HierSearch {
    pub query: String,
}

#[derive(Component)]
pub(crate) struct HierSearchInput;

/// Build the header search box (filters the tree by entity name).
pub(crate) fn build_search_box(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let input = text_input(commands, &fonts.ui, "Search…", "");
    commands.entity(input).insert((
        HierSearchInput,
        Node {
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
    ));
    input
}

/// Mirror the search box's text into [`HierSearch`].
pub(crate) fn hier_search_sync(
    inputs: Query<&EmberTextInput, With<HierSearchInput>>,
    mut search: ResMut<HierSearch>,
) {
    for inp in &inputs {
        if search.query != inp.value {
            search.query = inp.value.clone();
        }
    }
}

#[derive(Component)]
pub(crate) struct FilterToggle(&'static str);

#[derive(Component)]
pub(crate) struct FilterClearBtn;

fn key(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// Build the funnel button (a [`Popup`] trigger) + its checkbox panel.
pub(crate) fn build_filter_funnel(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(100.0),
                right: Val::Px(0.0),
                margin: UiRect::top(Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                min_width: Val::Px(170.0),
                max_height: Val::Px(340.0),
                overflow: Overflow::clip(),
                padding: UiRect::all(Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::popup_bg())),
            BorderColor::all(rgb(renzora_ember::theme::border())),
            GlobalZIndex(700),
            // Absorb pointer events so hover/click never leaks to the rows behind.
            bevy::ui::FocusPolicy::Block,
            RelativeCursorPosition::default(),
            Name::new("hier-filter-panel"),
        ))
        .id();
    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .id();
    keyed_list(commands, list, filter_snapshot);
    commands.entity(panel).add_child(list);

    let icon = icon_text(commands, &fonts.phosphor, "funnel", (170, 170, 184), 13.0);
    commands.entity(icon).insert(bevy::ui::FocusPolicy::Pass);
    let funnel = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                position_type: PositionType::Relative,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            Popup::new(panel),
            Name::new("hier-filter-funnel"),
        ))
        .id();
    bind_bg(commands, funnel, move |w| {
        let active = w
            .get_resource::<HierFilter>()
            .is_some_and(|f| !f.types.is_empty());
        if active {
            return rgb(accent()).with_alpha(0.30);
        }
        match w.get::<Interaction>(funnel) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(renzora_ember::theme::hover_bg()),
            _ => Color::NONE,
        }
    });
    commands.entity(funnel).add_children(&[icon, panel]);
    funnel
}

enum FRow {
    Type(&'static str, &'static str, [u8; 3]),
    Other,
    Clear,
}

fn filter_snapshot(world: &World) -> KeyedSnapshot {
    let mut list: Vec<FRow> = Vec::new();
    if let Some(reg) = world.get_resource::<ComponentIconRegistry>() {
        let mut t: Vec<(&'static str, &'static str, [u8; 3])> =
            reg.iter().map(|e| (e.name, e.icon, e.color)).collect();
        t.sort_by_key(|x| x.0);
        t.dedup_by_key(|x| x.0);
        for (n, i, c) in t {
            list.push(FRow::Type(n, i, c));
        }
    }
    list.push(FRow::Other);
    list.push(FRow::Clear);

    let items: Vec<(u64, u64)> = list
        .iter()
        .map(|r| {
            let k = match r {
                FRow::Type(n, _, _) => key(n),
                FRow::Other => key("__other__"),
                FRow::Clear => key("__clear__"),
            };
            (k, k)
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| filter_row(c, f, &list[i])),
    }
}

fn filter_row(commands: &mut Commands, fonts: &EmberFonts, row: &FRow) -> Entity {
    match row {
        FRow::Clear => {
            let r = commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        margin: UiRect::top(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    Interaction::default(),
                    FilterClearBtn,
                    Name::new("filter-clear"),
                ))
                .id();
            let t = commands
                .spawn((
                    Text::new("Clear filter"),
                    ui_font(&fonts.ui, 11.0),
                    TextColor(rgb(renzora_ember::theme::close_red())),
                ))
                .id();
            commands.entity(r).add_child(t);
            r
        }
        FRow::Type(name, icon, color) => check_row(commands, fonts, name, Some((*icon, *color)), name),
        FRow::Other => check_row(commands, fonts, "Other", None, "__other__"),
    }
}

fn check_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    icon: Option<(&'static str, [u8; 3])>,
    type_name: &'static str,
) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            FilterToggle(type_name),
            Name::new("filter-row"),
        ))
        .id();
    // Checkbox (filled accent when this type is active).
    let check = commands
        .spawn((
            Node {
                width: Val::Px(14.0),
                height: Val::Px(14.0),
                flex_shrink: 0.0,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(rgb(renzora_ember::theme::border())),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    bind_bg(commands, check, move |w| {
        if w.get_resource::<HierFilter>().is_some_and(|f| f.types.contains(type_name)) {
            rgb(accent())
        } else {
            Color::NONE
        }
    });
    let mut kids = vec![check];
    let label_color = if let Some((ic, color)) = icon {
        let g = icon_text(commands, &fonts.phosphor, ic, (color[0], color[1], color[2]), 13.0);
        commands.entity(g).insert(bevy::ui::FocusPolicy::Pass);
        kids.push(g);
        color
    } else {
        [200, 200, 210]
    };
    let t = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb((label_color[0], label_color[1], label_color[2]))),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    kids.push(t);
    commands.entity(row).add_children(&kids);
    row
}

pub(crate) fn hier_filter_toggle(
    q: Query<(&Interaction, &FilterToggle), Changed<Interaction>>,
    mut filter: ResMut<HierFilter>,
) {
    for (interaction, t) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if filter.types.contains(t.0) {
            filter.types.remove(t.0);
        } else {
            filter.types.insert(t.0);
        }
    }
}

pub(crate) fn hier_filter_clear(
    q: Query<&Interaction, (With<FilterClearBtn>, Changed<Interaction>)>,
    mut filter: ResMut<HierFilter>,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        filter.types.clear();
    }
}
