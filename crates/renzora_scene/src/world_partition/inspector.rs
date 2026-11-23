//! The World Partition section of the Inspector: the grid's settings, the cell
//! map, what is resident, the editor streaming switches, and the stand-in
//! builder. Select the World Partition entity to see it.
//!
//! This used to be a panel of its own, beside an Inspector section that showed
//! only the three numbers. Everything about the partition now lives on the
//! entity that holds it, which is also the folder its cells are filed under.
//! Partitioning more content is on the Hierarchy's right-click menu ("Partition
//! into cells"), where the content is selected anyway.
//!
//! The readouts are reactive bindings over [`PartitionMap`], which is
//! refreshed only while a map is on screen.

use bevy::prelude::*;
use renzora_scene_api::WorldPartition;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::inspector::{inspector_body, inspector_row};
use renzora_ember::reactive::tracked::{bind_2way, bind_text};
use renzora_ember::reactive::{Bound, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::{drag_value, icon_label_button, toggle_switch, DragRange};
use renzora_ember::{UiCursor, UiInteraction};

use super::map::{self, CellState, PartitionMap, PartitionMapView};

/// What a section button does.
#[derive(Component, Clone, Copy)]
enum PartitionAction {
    UnpinAll,
    BuildDistantLods,
}

fn snap<R: Default>(w: &Rx, f: impl FnOnce(&PartitionMap) -> R) -> R {
    w.get_resource::<PartitionMap>().map(f).unwrap_or_default()
}

fn text(commands: &mut Commands, fonts: &EmberFonts, s: &str, size: f32, colour: (u8, u8, u8)) -> Entity {
    commands
        .spawn((Text::new(s), ui_font(&fonts.ui, size), TextColor(rgb(colour))))
        .id()
}

fn row(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id()
}

fn spacer(commands: &mut Commands) -> Entity {
    commands.spawn(Node { flex_grow: 1.0, ..default() }).id()
}

/// A label on the left and a live value on the right.
fn stat<F>(commands: &mut Commands, fonts: &EmberFonts, label: &str, value: F) -> Entity
where
    F: for<'w> Fn(&Rx<'w>) -> String + Send + Sync + 'static,
{
    let r = row(commands);
    let l = text(commands, fonts, label, 11.0, text_muted());
    let gap = spacer(commands);
    let v = commands
        .spawn((Text::new(""), ui_font(&fonts.mono, 11.0), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, v, value);
    commands.entity(r).add_children(&[l, gap, v]);
    r
}

/// Which of the partition's flags a switch shows.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum PartitionFlag {
    StreamInEditor,
    LoadAll,
}

impl PartitionFlag {
    fn get(self, p: &WorldPartition) -> bool {
        match self {
            Self::StreamInEditor => p.stream_in_editor,
            Self::LoadAll => p.editor_load_all,
        }
    }
    fn toggle(self, p: &mut WorldPartition) {
        match self {
            Self::StreamInEditor => p.stream_in_editor = !p.stream_in_editor,
            Self::LoadAll => p.editor_load_all = !p.editor_load_all,
        }
    }
}

/// A switch showing one of the partition's flags.
///
/// Deliberately not a two-way binding. Those write the switch's value back to
/// the scene whenever the two disagree, and a switch built before it has read
/// the partition starts out "off": the scene's "stream in editor" was saved
/// off that way, and the next open expanded every cell of a city at once.
/// Here the scene is written only by a click, which flips the real value; the
/// switch just shows it ([`sync_partition_switches`]).
fn switch(commands: &mut Commands, fonts: &EmberFonts, label: &str, flag: PartitionFlag) -> Entity {
    let sw = toggle_switch(commands, false);
    commands.entity(sw).insert(flag);
    inspector_row(commands, &fonts.ui, label, sw)
}

/// Show each flag's real value on its switch, and apply clicks to the
/// partition itself.
fn sync_partition_switches(
    mut switches: Query<(&PartitionFlag, &mut Bound<bool>, Ref<UiInteraction>)>,
    mut partitions: Query<&mut WorldPartition>,
) {
    let Some(mut partition) = partitions.iter_mut().next() else {
        return;
    };
    for (flag, mut bound, interaction) in &mut switches {
        if interaction.is_changed() && *interaction == UiInteraction::Pressed {
            flag.toggle(&mut partition);
        }
        let value = flag.get(&partition);
        if bound.0 != value {
            bound.0 = value;
        }
    }
}

/// One of the partition's distances as a scrubbable value. Written only when
/// it actually changes, so an untouched field never marks the scene modified.
#[allow(clippy::too_many_arguments)]
fn distance_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    label: &str,
    init: f32,
    min: f32,
    get: fn(&WorldPartition) -> f32,
    set: fn(&mut WorldPartition, f32),
) -> Entity {
    let dv = drag_value(commands, &fonts.ui, "", (210, 210, 220), init, 10.0);
    commands.entity(dv).insert(DragRange { min, max: 100_000.0 });
    bind_2way(
        commands,
        dv,
        move |w| w.get::<WorldPartition>(entity).map(get).unwrap_or(0.0),
        move |w, v: &f32| {
            let v = v.max(min);
            if let Some(mut p) = w.get_mut::<WorldPartition>(entity) {
                if get(&p) != v {
                    set(&mut p, v);
                }
            }
        },
    );
    inspector_row(commands, &fonts.ui, label, dv)
}

/// The section body, built by the Inspector when the World Partition entity is
/// selected and the section is open.
pub(crate) fn world_partition_ui(world: &mut World, entity: Entity) -> Entity {
    let Some(p) = world.get::<WorldPartition>(entity).cloned() else {
        return world.spawn(Node::default()).id();
    };
    inspector_body(world, move |commands, fonts| build(commands, fonts, entity, &p))
}

fn build(commands: &mut Commands, fonts: &EmberFonts, entity: Entity, p: &WorldPartition) -> Entity {
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            padding: UiRect::all(Val::Px(2.0)),
            ..default()
        })
        .id();

    let settings = [
        distance_row(commands, fonts, entity, "Cell Size", p.cell_size, 10.0, |p| p.cell_size, |p, v| p.cell_size = v),
        distance_row(commands, fonts, entity, "Load Radius", p.load_radius, 0.0, |p| p.load_radius, |p, v| {
            p.load_radius = v
        }),
        distance_row(commands, fonts, entity, "Unload Radius", p.unload_radius, 0.0, |p| p.unload_radius, |p, v| {
            p.unload_radius = v
        }),
        switch(commands, fonts, "Stream in editor", PartitionFlag::StreamInEditor),
        switch(commands, fonts, "Load all cells", PartitionFlag::LoadAll),
    ];

    let stats = [
        stat(commands, fonts, "Cells", |w| snap(w, |m| m.cells.len().to_string())),
        stat(commands, fonts, "Loaded", |w| {
            snap(w, |m| format!("{} / {}", m.count(CellState::Loaded), m.cells.len()))
        }),
        stat(commands, fonts, "Loading", |w| snap(w, |m| m.count(CellState::Loading).to_string())),
        stat(commands, fonts, "Pinned", |w| snap(w, |m| m.pinned().to_string())),
        stat(commands, fonts, "Distant LODs", |w| {
            snap(w, |m| format!("{} / {}", m.with_hlod, m.cells.len()))
        }),
        stat(commands, fonts, "Camera cell", |w| {
            snap(w, |m| m.camera.map(|c| format!("{}, {}", c.x, c.y)).unwrap_or_else(|| "-".into()))
        }),
    ];

    let view = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                max_height: Val::Px(360.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            BorderColor::all(rgb(border())),
            UiInteraction::default(),
            UiCursor::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            PartitionMapView::default(),
            Name::new("world-partition-map"),
        ))
        .id();
    map::attach_canvas(commands, view);
    let hint = text(commands, fonts, "Click a cell to pin it loaded while you work.", 10.5, text_muted());

    let legend = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(12.0),
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    let swatches: [(Color, bool, &str); 5] = [
        (rgb(accent()), false, "Loaded"),
        (rgb(accent()).with_alpha(0.45), false, "Loading"),
        (rgb(border()), false, "Unloaded"),
        (rgb(warn_amber()), true, "Pinned"),
        (rgb(text_primary()), true, "Camera"),
    ];
    for (colour, outline, label) in swatches {
        let item = commands
            .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(5.0), ..default() })
            .id();
        let mut sw = commands.spawn(Node {
            width: Val::Px(9.0),
            height: Val::Px(9.0),
            border: UiRect::all(Val::Px(if outline { 1.5 } else { 0.0 })),
            border_radius: BorderRadius::all(Val::Px(2.0)),
            ..default()
        });
        if outline {
            sw.insert(BorderColor::all(colour));
        } else {
            sw.insert(BackgroundColor(colour));
        }
        let sw = sw.id();
        let t = text(commands, fonts, label, 10.5, text_muted());
        commands.entity(item).add_children(&[sw, t]);
        commands.entity(legend).add_child(item);
    }

    let unpin = icon_label_button(commands, fonts, "push-pin-slash", "Unpin all");
    commands.entity(unpin).insert(PartitionAction::UnpinAll);
    let lods = icon_label_button(commands, fonts, "buildings", "Build distant LODs");
    commands.entity(lods).insert(PartitionAction::BuildDistantLods);
    let lods_note = text(
        commands,
        fonts,
        "Builds a simple stand-in for every cell, shown whenever the cell itself is not loaded, so nothing disappears in the distance. Build again after editing cells.",
        10.5,
        text_muted(),
    );
    let add_note = text(
        commands,
        fonts,
        "To add more content, right-click it in the Hierarchy and choose Partition into cells.",
        10.5,
        text_muted(),
    );

    commands.entity(root).add_children(&settings);
    commands.entity(root).add_children(&stats);
    commands
        .entity(root)
        .add_children(&[view, hint, legend, unpin, lods, lods_note, add_note]);
    root
}

fn partition_buttons(
    buttons: Query<(&PartitionAction, &UiInteraction), Changed<UiInteraction>>,
    mut commands: Commands,
) {
    for (action, interaction) in &buttons {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        match action {
            PartitionAction::UnpinAll => commands.queue(super::unpin_all_cells),
            PartitionAction::BuildDistantLods => commands.queue(super::build_distant_lods),
        }
    }
}

pub(crate) fn register(app: &mut App) {
    use renzora_editor_framework::AppEditorExt;

    app.init_resource::<PartitionMap>()
        .register_native_inspector_ui("world_partition", world_partition_ui)
        .add_systems(
            Update,
            (
                // The map and its readouts cost a walk over every cell, so they
                // run only while a map is on screen: the section is open.
                (map::refresh_partition_map, map::partition_map_click, map::draw_partition_map)
                    .chain()
                    .run_if(any_with_component::<PartitionMapView>),
                partition_buttons,
                sync_partition_switches,
            )
                .chain()
                .run_if(in_state(renzora_project_api::SplashState::Editor)),
        );
}
