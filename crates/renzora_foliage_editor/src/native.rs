//! Bevy-native (ember) port of the egui `FoliagePanel` (panel id
//! "foliage_painting"): an enable toggle over three collapsible sections —
//! Foliage Types (a selectable list + Add/Remove), Brush (Paint/Erase toggle +
//! Size/Strength/Falloff scrub fields) and Properties (the selected type's Name,
//! Density, Height Range, Wind Strength and Enabled). Every control writes back
//! into the same resources the egui panel mutates: [`FoliageToolState`],
//! [`FoliagePaintSettings`] and [`FoliageConfig`].

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_editor::SplashState;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{
    bind_2way, bind_bg, bind_display, keyed_list, KeyedSnapshot,
};
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    bind_text_input, checkbox, collapsible, drag_value, text_input, DragRange,
};

use renzora_terrain::data::TerrainChunkData;
use renzora_terrain::foliage::{
    FoliageBrushType, FoliageConfig, FoliageDensityMap, FoliagePaintSettings, FoliageType,
};

use crate::systems::FoliageToolState;

const LABEL_W: f32 = 96.0;

pub struct NativeFoliage;

impl Plugin for NativeFoliage {
    fn build(&self, app: &mut App) {
        app.register_panel_content("foliage_painting", true, build);
        app.add_systems(
            Update,
            (
                foliage_type_select,
                foliage_add_type,
                foliage_remove_type,
                foliage_brush_select,
                foliage_ensure_density_maps,
            )
                .run_if(in_state(SplashState::Editor)),
        );
    }
}

// ── State accessors (mirror the egui panel's `get_resource` reads) ───────────

fn tool_active(w: &World) -> bool {
    w.get_resource::<FoliageToolState>().map(|t| t.active).unwrap_or_default()
}

fn set_tool_active(w: &mut World, active: bool) {
    let mut t = w.get_resource::<FoliageToolState>().copied().unwrap_or_default();
    t.active = active;
    w.insert_resource(t);
}

fn settings(w: &World) -> FoliagePaintSettings {
    w.get_resource::<FoliagePaintSettings>().cloned().unwrap_or_default()
}

fn set_settings(w: &mut World, f: impl FnOnce(&mut FoliagePaintSettings)) {
    let mut s = settings(w);
    f(&mut s);
    w.insert_resource(s);
}

fn config(w: &World) -> FoliageConfig {
    w.get_resource::<FoliageConfig>().cloned().unwrap_or_default()
}

fn set_config(w: &mut World, f: impl FnOnce(&mut FoliageConfig)) {
    let mut c = config(w);
    f(&mut c);
    w.insert_resource(c);
}

fn active_type(w: &World) -> usize {
    settings(w).active_type
}

fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
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
            Name::new("native-foliage"),
        ))
        .id();

    // ── Enable toggle row ────────────────────────────────────────────────────
    let header = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    // `false` is a transient seed — `bind_2way` corrects it from the live world
    // on its first run (state → model wins the initial tie).
    let enable = checkbox(commands, false);
    bind_2way(commands, enable, tool_active, |w, v: &bool| set_tool_active(w, *v));
    let tree_icon = icon_text(commands, &fonts.phosphor, "tree", text_primary(), 14.0);
    let title = commands
        .spawn((
            Text::new("Foliage Painting"),
            ui_font(&fonts.ui, 14.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    commands.entity(header).add_children(&[enable, tree_icon, title]);

    // ── Inactive hint (shown only when the tool is off) ──────────────────────
    let hint = commands
        .spawn((
            Text::new("Enable to paint foliage on terrain."),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node { margin: UiRect::top(Val::Px(2.0)), ..default() },
        ))
        .id();
    bind_display(commands, hint, |w| !tool_active(w));

    // ── Sections wrapper (shown only when the tool is on) ────────────────────
    let sections = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .id();
    bind_display(commands, sections, tool_active);

    let types_sec = types_section(commands, fonts);
    let brush_sec = brush_section(commands, fonts);
    let props_sec = properties_section(commands, fonts);
    commands.entity(sections).add_children(&[types_sec, brush_sec, props_sec]);

    commands.entity(root).add_children(&[header, hint, sections]);
    root
}

// ── Foliage Types section ────────────────────────────────────────────────────

#[derive(Component)]
struct TypeRow {
    index: usize,
}
#[derive(Component)]
struct AddTypeBtn;
#[derive(Component)]
struct RemoveTypeBtn;

fn types_section(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = collapsible(commands, fonts, None, "Foliage Types", true);

    // The reactive list of types.
    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            ..default()
        })
        .id();
    keyed_list(commands, list, types_snapshot);

    // Add / Remove buttons.
    let btns = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        })
        .id();
    let add = action_button(commands, fonts, "plus", "Add");
    commands.entity(add).insert(AddTypeBtn);
    let remove = action_button(commands, fonts, "trash", "Remove");
    commands.entity(remove).insert(RemoveTypeBtn);
    // Hide Remove when only one type remains (matches egui's `len() > 1` guard).
    bind_display(commands, remove, |w| config(w).types.len() > 1);
    commands.entity(btns).add_children(&[add, remove]);

    commands.entity(body).add_children(&[list, btns]);
    root
}

fn types_snapshot(world: &World) -> KeyedSnapshot {
    let cfg = config(world);
    // Key + hash on STRUCTURE (index + name) — not on the live selection — so
    // selecting a row (a bg change driven reactively) never rebuilds the list.
    let items: Vec<(u64, u64)> = cfg
        .types
        .iter()
        .enumerate()
        .map(|(i, ft)| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            (i, &ft.name).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    let names: Vec<String> = cfg.types.iter().map(|t| t.name.clone()).collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| type_row(c, f, i, &names[i])),
    }
}

fn type_row(commands: &mut Commands, fonts: &EmberFonts, index: usize, name: &str) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(22.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            renzora_hui::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            TypeRow { index },
            Name::new(format!("foliage-type:{index}")),
        ))
        .id();
    // Selected → accent tint, hover → hover surface, else transparent.
    bind_bg(commands, row, move |w| {
        if active_type(w) == index {
            rgb(accent()).with_alpha(0.25)
        } else if matches!(
            w.get::<Interaction>(row),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        ) {
            rgb(hover_bg())
        } else {
            Color::NONE
        }
    });
    let label = commands
        .spawn((
            Text::new(format!("{}. {}", index + 1, name)),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    commands.entity(row).add_child(label);
    row
}

// ── Brush section ────────────────────────────────────────────────────────────

#[derive(Component)]
struct BrushModeBtn {
    mode: FoliageBrushType,
}

fn brush_section(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = collapsible(commands, fonts, None, "Brush", true);

    // Paint / Erase toggle buttons.
    let modes = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            margin: UiRect::bottom(Val::Px(4.0)),
            ..default()
        })
        .id();
    let paint = brush_mode_button(commands, fonts, "paint-brush", FoliageBrushType::Paint);
    let erase = brush_mode_button(commands, fonts, "eraser", FoliageBrushType::Erase);
    commands.entity(modes).add_children(&[paint, erase]);

    let size = labelled_drag(commands, fonts, "Size", 0.01, 0.5, 0.005, {
        |w| settings(w).brush_radius
    }, |w, v| set_settings(w, |s| s.brush_radius = *v));
    let strength = labelled_drag(commands, fonts, "Strength", 0.01, 1.0, 0.01, {
        |w| settings(w).brush_strength
    }, |w, v| set_settings(w, |s| s.brush_strength = *v));
    let falloff = labelled_drag(commands, fonts, "Falloff", 0.0, 1.0, 0.01, {
        |w| settings(w).brush_falloff
    }, |w, v| set_settings(w, |s| s.brush_falloff = *v));

    commands.entity(body).add_children(&[modes, size, strength, falloff]);
    root
}

fn brush_mode_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    mode: FoliageBrushType,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(32.0),
                height: Val::Px(28.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            renzora_hui::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            BrushModeBtn { mode },
            Name::new("foliage-brush-mode"),
        ))
        .id();
    // Selected → accent tint, hover → hover surface, else transparent.
    bind_bg(commands, btn, move |w| {
        if settings(w).brush_type == mode {
            rgb(accent()).with_alpha(0.30)
        } else if matches!(
            w.get::<Interaction>(btn),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        ) {
            rgb(hover_bg())
        } else {
            Color::NONE
        }
    });
    let ic = icon_text(commands, &fonts.phosphor, icon, text_primary(), 16.0);
    commands.entity(btn).add_child(ic);
    btn
}

// ── Properties section ───────────────────────────────────────────────────────

fn properties_section(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (root, body) = collapsible(commands, fonts, None, "Properties", true);

    // Name (text input bound to the active type's name).
    let name_row = field_row(commands, fonts, "Name");
    let name_in = text_input(commands, &fonts.ui, "Name", "");
    commands.entity(name_in).insert(Node {
        flex_grow: 1.0,
        min_width: Val::Px(0.0),
        padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(1.0)),
        border_radius: BorderRadius::all(Val::Px(4.0)),
        ..default()
    });
    bind_text_input(
        commands,
        name_in,
        |w| {
            let i = active_type(w);
            config(w).types.get(i).map(|t| t.name.clone()).unwrap_or_default()
        },
        |w, v| {
            let i = active_type(w);
            set_config(w, |c| {
                if let Some(t) = c.types.get_mut(i) {
                    t.name = v;
                }
            });
        },
    );
    commands.entity(name_row).add_child(name_in);

    let density = labelled_drag(commands, fonts, "Density", 1.0, 50.0, 0.5, type_get(|t| t.density), type_set(|t, v| t.density = v));

    // Height Range (min / max on one row).
    let hr_label = field_label(commands, fonts, "Height Range");
    let hr_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            margin: UiRect::bottom(Val::Px(3.0)),
            ..default()
        })
        .id();
    let hr_min = num_field(commands, fonts, "min", 0.01, 2.0, 0.01, type_get(|t| t.height_range.x), type_set(|t, v| t.height_range.x = v));
    let hr_max = num_field(commands, fonts, "max", 0.01, 2.0, 0.01, type_get(|t| t.height_range.y), type_set(|t, v| t.height_range.y = v));
    commands.entity(hr_row).add_children(&[hr_min, hr_max]);

    let wind = labelled_drag(commands, fonts, "Wind Strength", 0.0, 2.0, 0.01, type_get(|t| t.wind_strength), type_set(|t, v| t.wind_strength = v));

    // Enabled checkbox.
    let en_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            margin: UiRect::top(Val::Px(2.0)),
            ..default()
        })
        .id();
    let en_box = checkbox(commands, false);
    bind_2way(
        commands,
        en_box,
        |w| {
            let i = active_type(w);
            config(w).types.get(i).map(|t| t.enabled).unwrap_or(true)
        },
        |w, v: &bool| {
            let i = active_type(w);
            set_config(w, |c| {
                if let Some(t) = c.types.get_mut(i) {
                    t.enabled = *v;
                }
            });
        },
    );
    let en_lbl = commands
        .spawn((Text::new("Enabled"), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted()))))
        .id();
    commands.entity(en_row).add_children(&[en_box, en_lbl]);

    commands.entity(body).add_children(&[name_row, density, hr_label, hr_row, wind, en_row]);
    root
}

// ── Shared row/field builders ────────────────────────────────────────────────

fn field_row(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            margin: UiRect::bottom(Val::Px(3.0)),
            ..default()
        })
        .id();
    let lbl = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node { width: Val::Px(LABEL_W), flex_shrink: 0.0, ..default() },
        ))
        .id();
    commands.entity(row).add_child(lbl);
    row
}

fn field_label(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node { margin: UiRect::bottom(Val::Px(2.0)), ..default() },
        ))
        .id()
}

/// A labelled row: a muted caption above a single scrub field bound to state.
fn labelled_drag<G, S>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    min: f32,
    max: f32,
    step: f32,
    get: G,
    set: S,
) -> Entity
where
    G: Fn(&World) -> f32 + Send + Sync + 'static,
    S: Fn(&mut World, &f32) + Send + Sync + 'static,
{
    let col = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            margin: UiRect::bottom(Val::Px(3.0)),
            ..default()
        })
        .id();
    let lbl = field_label(commands, fonts, label);
    let field = num_field(commands, fonts, "", min, max, step, get, set);
    commands.entity(col).add_children(&[lbl, field]);
    col
}

/// A scrubbable numeric field bound two-way to state, clamped to `min..=max`.
fn num_field<G, S>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    axis: &str,
    min: f32,
    max: f32,
    step: f32,
    get: G,
    set: S,
) -> Entity
where
    G: Fn(&World) -> f32 + Send + Sync + 'static,
    S: Fn(&mut World, &f32) + Send + Sync + 'static,
{
    // `min` is a transient seed — `bind_2way` corrects it from the live world on
    // its first run; the `get` closure is consumed by the binding below.
    let dv = drag_value(commands, &fonts.ui, axis, value_text(), min, step);
    if max > min {
        commands.entity(dv).insert(DragRange { min, max });
    }
    bind_2way(commands, dv, get, set);
    dv
}

fn action_button(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str) -> Entity {
    let b = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            renzora_hui::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new(format!("foliage-action:{label}")),
        ))
        .id();
    let b_for_bg = b;
    bind_bg(commands, b, move |w| match w.get::<Interaction>(b_for_bg) {
        Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(hover_bg()),
        _ => rgb(card_bg()),
    });
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 12.0);
    let t = commands
        .spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary()))))
        .id();
    commands.entity(b).add_children(&[ic, t]);
    b
}

// ── Active-type get/set helpers (the egui panel's `types.get_mut(active_type)`)

/// Build a getter that reads field `f` off the active foliage type.
fn type_get<F>(f: F) -> impl Fn(&World) -> f32 + Send + Sync + 'static
where
    F: Fn(&FoliageType) -> f32 + Send + Sync + 'static,
{
    move |w| {
        let i = active_type(w);
        config(w).types.get(i).map(&f).unwrap_or(0.0)
    }
}

/// Build a setter that writes field `f` on the active foliage type. The value is
/// passed as `f32` (booleans encoded as 0.0/1.0) so one helper serves every field.
fn type_set<F>(f: F) -> impl Fn(&mut World, &f32) + Send + Sync + 'static
where
    F: Fn(&mut FoliageType, f32) + Send + Sync + 'static,
{
    move |w, v| {
        let i = active_type(w);
        set_config(w, |c| {
            if let Some(t) = c.types.get_mut(i) {
                f(t, *v);
            }
        });
    }
}

// ── Systems: selection + add/remove + density-map seeding ────────────────────

fn foliage_type_select(
    q: Query<(&Interaction, &TypeRow), Changed<Interaction>>,
    mut settings: Option<ResMut<FoliagePaintSettings>>,
) {
    let Some(settings) = settings.as_mut() else { return };
    for (interaction, row) in &q {
        if *interaction == Interaction::Pressed {
            settings.active_type = row.index;
        }
    }
}

fn foliage_add_type(
    q: Query<&Interaction, (With<AddTypeBtn>, Changed<Interaction>)>,
    mut commands: Commands,
    config: Option<Res<FoliageConfig>>,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let mut cfg = config.map(|c| c.clone()).unwrap_or_default();
    cfg.types.push(FoliageType::default());
    commands.insert_resource(cfg);
}

fn foliage_remove_type(
    q: Query<&Interaction, (With<RemoveTypeBtn>, Changed<Interaction>)>,
    mut commands: Commands,
    config: Option<Res<FoliageConfig>>,
    settings: Option<Res<FoliagePaintSettings>>,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let mut cfg = config.map(|c| c.clone()).unwrap_or_default();
    if cfg.types.len() <= 1 {
        return;
    }
    let mut active = settings.map(|s| s.active_type).unwrap_or(0);
    let idx = active.min(cfg.types.len().saturating_sub(1));
    cfg.types.remove(idx);
    if active >= cfg.types.len() {
        active = cfg.types.len().saturating_sub(1);
    }
    commands.insert_resource(cfg);
    commands.queue(move |w: &mut World| {
        set_settings(w, |s| s.active_type = active);
    });
}

fn foliage_brush_select(
    q: Query<(&Interaction, &BrushModeBtn), Changed<Interaction>>,
    mut settings: Option<ResMut<FoliagePaintSettings>>,
) {
    let Some(settings) = settings.as_mut() else { return };
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed {
            settings.brush_type = btn.mode;
        }
    }
}

/// Mirror the egui panel: while the tool is active, seed every terrain chunk that
/// lacks a [`FoliageDensityMap`] with one.
fn foliage_ensure_density_maps(
    tool: Option<Res<FoliageToolState>>,
    mut commands: Commands,
    chunks: Query<Entity, (With<TerrainChunkData>, Without<FoliageDensityMap>)>,
) {
    if !tool.is_some_and(|t| t.active) {
        return;
    }
    for entity in &chunks {
        commands.entity(entity).insert(FoliageDensityMap::new(64));
    }
}
