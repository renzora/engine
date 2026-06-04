//! Bevy-native (ember) port of the egui `GaugesPanel`: a live debug view of every
//! entity carrying the `Gauges` component, with all its attribute values shown as
//! gauge bars. A filter box at the top narrows the list by entity id / name; each
//! entity is a collapsible card whose body lists its attributes as
//! `name · bar(value) · value` rows. Read-only — it mirrors `GaugesSnapshot`,
//! which is rebuilt every frame from the world.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_editor::SplashState;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::inspector::inspector_stripe;
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{
    bind_bg, bind_display, bind_text, bind_text_color, bind_with, keyed_list, KeyedSnapshot,
};
use renzora_ember::theme::*;
use renzora_ember::widgets::{bind_text_input, text_input, EmberTextInput};

use crate::{GaugeEntitySnapshot, GaugesSnapshot};

/// Native plugin that overrides the egui gauges panel body with an ember one.
pub struct NativeGauges;

impl Plugin for NativeGauges {
    fn build(&self, app: &mut App) {
        app.init_resource::<GaugesFilter>();
        app.register_panel_content("gauges", true, build);
        app.add_systems(
            Update,
            (clear_filter_click, sync_filter_from_inputs).run_if(in_state(SplashState::Editor)),
        );
    }
}

/// The current filter string (egui kept this in `PanelState.filter`).
#[derive(Resource, Default)]
struct GaugesFilter(String);

#[derive(Component)]
struct FilterInput;
#[derive(Component)]
struct ClearFilterBtn;

// ── Filtering ────────────────────────────────────────────────────────────────

/// The entries that pass the current filter (mirrors the egui `entries` filter).
fn filtered_entries(world: &World) -> Vec<GaugeEntitySnapshot> {
    let Some(snapshot) = world.get_resource::<GaugesSnapshot>() else {
        return Vec::new();
    };
    let filter = world
        .get_resource::<GaugesFilter>()
        .map(|f| f.0.to_lowercase())
        .unwrap_or_default();
    snapshot
        .entries
        .iter()
        .filter(|e| {
            if filter.is_empty() {
                return true;
            }
            let entity_str = format!("{:?}", e.entity).to_lowercase();
            let name_str = e.name.as_deref().unwrap_or("").to_lowercase();
            entity_str.contains(&filter) || name_str.contains(&filter)
        })
        .cloned()
        .collect()
}

// ── Panel ────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(6.0)),
                ..default()
            },
            Name::new("native-gauges"),
        ))
        .id();

    // Header: magnifying-glass · filter input · clear (×).
    let header = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    let search_icon = icon_text(commands, &fonts.phosphor, "magnifying-glass", text_muted(), 12.0);
    let input = text_input(commands, &fonts.ui, "Filter entities...", "");
    commands.entity(input).insert((
        FilterInput,
        Node {
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
    ));
    bind_text_input(
        commands,
        input,
        |w| w.get_resource::<GaugesFilter>().map(|f| f.0.clone()).unwrap_or_default(),
        |w, v| {
            if let Some(mut f) = w.get_resource_mut::<GaugesFilter>() {
                f.0 = v;
            }
        },
    );
    let clear = clear_button(commands, fonts);
    commands.entity(header).add_children(&[search_icon, input, clear]);

    // Separator.
    let sep = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(1.0), ..default() },
            BackgroundColor(rgb(border())),
        ))
        .id();

    // Empty-state note (shown when no entries pass the filter).
    let empty = empty_state(commands, fonts);
    bind_display(commands, empty, |w| filtered_entries(w).is_empty());

    // Entity list.
    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    bind_display(commands, list, |w| !filtered_entries(w).is_empty());
    keyed_list(commands, list, entities_snapshot);

    commands.entity(root).add_children(&[header, sep, empty, list]);
    root
}

/// A `×` button that clears the filter, dimmed (disabled-look) while empty.
fn clear_button(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(20.0),
                height: Val::Px(20.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_shrink: 0.0,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            ClearFilterBtn,
            Name::new("gauges-clear-filter"),
        ))
        .id();
    let glyph = icon_text(commands, &fonts.phosphor, "x", text_muted(), 11.0);
    // Dim the × while the filter is empty (egui disabled the button).
    bind_text_color(commands, glyph, |w| {
        let empty = w.get_resource::<GaugesFilter>().is_none_or(|f| f.0.is_empty());
        rgb(if empty { placeholder() } else { text_muted() })
    });
    commands.entity(btn).add_child(glyph);
    btn
}

/// The "no entities with gauges" placeholder.
fn empty_state(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(4.0),
            padding: UiRect::top(Val::Px(16.0)),
            ..default()
        })
        .id();
    let icon = icon_text(commands, &fonts.phosphor, "gauge", text_muted(), 24.0);
    let l1 = commands
        .spawn((
            Text::new("No entities with gauges"),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let l2 = commands
        .spawn((
            Text::new("Add the Gauges component to an entity to see its attributes here."),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::new_with_justify(bevy::text::Justify::Center),
        ))
        .id();
    commands.entity(col).add_children(&[icon, l1, l2]);
    col
}

// ── Entity list (keyed <For>) ──────────────────────────────────────────────────

fn entities_snapshot(world: &World) -> KeyedSnapshot {
    let entries = filtered_entries(world);
    let items: Vec<(u64, u64)> = entries
        .iter()
        .map(|e| {
            // Key on entity identity (stable across frames).
            let mut k = hasher();
            e.entity.hash(&mut k);
            // Hash STRUCTURE only (attribute names + the entity label), NOT the
            // live values — so a changing attribute value updates the bar in place
            // (via bindings) instead of rebuilding the whole row.
            let mut h = hasher();
            e.name.hash(&mut h);
            for (name, _) in &e.attributes {
                name.hash(&mut h);
            }
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| entity_card(c, f, &entries[i])),
    }
}

/// One entity: a header (cube icon + label) over its attribute rows.
fn entity_card(commands: &mut Commands, fonts: &EmberFonts, entry: &GaugeEntitySnapshot) -> Entity {
    let fallback = format!("{:?}", entry.entity);
    let label = entry.name.clone().unwrap_or(fallback);
    let entity = entry.entity;

    let card = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            margin: UiRect::top(Val::Px(2.0)),
            ..default()
        })
        .id();

    // Header row.
    let header = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(5.0),
            ..default()
        })
        .id();
    let cube = icon_text(commands, &fonts.phosphor, "cube", accent(), 11.0);
    let title = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    commands.entity(header).add_children(&[cube, title]);

    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            margin: UiRect::left(Val::Px(4.0)),
            ..default()
        })
        .id();

    if entry.attributes.is_empty() {
        let none = commands
            .spawn((
                Text::new("No attributes defined"),
                ui_font(&fonts.ui, 10.0),
                TextColor(rgb(text_muted())),
                Node { margin: UiRect::vertical(Val::Px(2.0)), ..default() },
            ))
            .id();
        commands.entity(body).add_child(none);
    } else {
        let rows: Vec<Entity> = entry
            .attributes
            .iter()
            .enumerate()
            .map(|(i, (attr_name, _))| attr_row(commands, fonts, entity, i, attr_name))
            .collect();
        commands.entity(body).add_children(&rows);
    }

    commands.entity(card).add_children(&[header, body]);
    card
}

/// One attribute row: fixed-width name · live bar · live value. The bar fill
/// width and color, and the two value texts, are bound to the entity's live
/// attribute value in `GaugesSnapshot`.
fn attr_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    idx: usize,
    attr_name: &str,
) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(16.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(inspector_stripe(idx)),
        ))
        .id();

    // Attribute name (fixed-width, clipped).
    let name = commands
        .spawn((
            Text::new(attr_name.to_string()),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::new_with_no_wrap(),
            Node { width: Val::Px(70.0), flex_shrink: 0.0, overflow: Overflow::clip(), ..default() },
        ))
        .id();

    // Bar track (flex-grows) with an overlaid fill + centered value text.
    let bar = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(30.0),
                height: Val::Px(12.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
        ))
        .id();
    // The fill bar is absolutely positioned behind the centered value text.
    let attr_for_fill = attr_name.to_string();
    let fill = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Percent(0.0),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id();
    {
        let attr = attr_for_fill.clone();
        bind_with(
            commands,
            fill,
            move |w| {
                let v = attr_value(w, entity, &attr);
                ((v / 100.0).clamp(0.0, 1.0) * 1000.0) as u32
            },
            |w, target, ratio_milli: &u32| {
                if let Some(mut n) = w.get_mut::<Node>(target) {
                    n.width = Val::Percent(*ratio_milli as f32 / 10.0);
                }
            },
        );
    }
    {
        let attr = attr_for_fill;
        bind_bg(commands, fill, move |w| {
            let v = attr_value(w, entity, &attr);
            let ratio = (v / 100.0).clamp(0.0, 1.0);
            if ratio <= 0.0 {
                Color::NONE
            } else {
                bar_color(ratio)
            }
        });
    }
    // Value text overlaid on the bar.
    let attr_for_bar_text = attr_name.to_string();
    let bar_value = commands
        .spawn((
            Text::new("0.0"),
            ui_font(&fonts.ui, 9.0),
            TextColor(rgb(text_primary())),
            ZIndex(1),
        ))
        .id();
    bind_text(commands, bar_value, move |w| {
        format!("{:.1}", attr_value(w, entity, &attr_for_bar_text))
    });
    commands.entity(bar).add_children(&[fill, bar_value]);

    // Trailing value label.
    let attr_for_label = attr_name.to_string();
    let value_label = commands
        .spawn((
            Text::new("0.0"),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(value_text())),
            bevy::text::TextLayout::new_with_no_wrap(),
            Node { flex_shrink: 0.0, ..default() },
        ))
        .id();
    bind_text(commands, value_label, move |w| {
        format!("{:.1}", attr_value(w, entity, &attr_for_label))
    });

    commands.entity(row).add_children(&[name, bar, value_label]);
    row
}

/// Read one entity's live attribute value from the snapshot (0.0 if gone).
fn attr_value(w: &World, entity: Entity, attr: &str) -> f32 {
    w.get_resource::<GaugesSnapshot>()
        .and_then(|s| s.entries.iter().find(|e| e.entity == entity))
        .and_then(|e| e.attributes.iter().find(|(n, _)| n == attr))
        .map(|(_, v)| *v)
        .unwrap_or(0.0)
}

/// Bar fill color from the 0..1 ratio (matches the egui thresholds).
fn bar_color(ratio: f32) -> Color {
    if ratio < 0.25 {
        Color::srgb_u8(220, 60, 60)
    } else if ratio < 0.5 {
        rgb(warn_amber())
    } else {
        rgb(play_green())
    }
}

fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

// ── Systems ────────────────────────────────────────────────────────────────────

/// Clear the filter when the × button is pressed.
fn clear_filter_click(
    q: Query<&Interaction, (With<ClearFilterBtn>, Changed<Interaction>)>,
    mut filter: ResMut<GaugesFilter>,
    mut inputs: Query<&mut EmberTextInput, With<FilterInput>>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    if filter.0.is_empty() {
        return;
    }
    filter.0.clear();
    // Also reset the input widget's own buffer + displayed placeholder.
    for mut inp in &mut inputs {
        inp.value.clear();
        if let Ok((mut t, mut c)) = texts.get_mut(inp.text_entity) {
            *t = Text::new(inp.placeholder.clone());
            c.0 = rgb(text_muted());
        }
    }
}

/// Mirror the focused filter input's typed value into `GaugesFilter` so the list
/// re-filters live while typing (`bind_text_input` only pushes when focused; this
/// keeps the resource current for the keyed-list snapshot).
fn sync_filter_from_inputs(
    inputs: Query<&EmberTextInput, With<FilterInput>>,
    mut filter: ResMut<GaugesFilter>,
) {
    for inp in &inputs {
        if inp.focused && filter.0 != inp.value {
            filter.0 = inp.value.clone();
        }
    }
}
