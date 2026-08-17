//! The Terrain Settings overlay — the deferred-apply editor for a terrain's
//! grid, resolution and height range.
//!
//! These used to be live inspector fields, and Chunks X/Z in particular were
//! sliders. That combination is the bug: every tick of the drag wrote
//! `TerrainData`, and every write ran `terrain_data_changed_system`'s slow path,
//! which despawns and respawns *every* chunk with resampled heights and a fresh
//! trimesh collider. Dragging 1 → 8 doesn't build an 8×8 terrain; it builds a
//! 1×1, then a 2×2, then a 3×3, all the way up, and at 257² resolution the tail
//! of that sequence is millions of vertices per step. The editor stops
//! responding, which reads as a crash.
//!
//! Two changes fix it, and both are needed:
//!
//! * **Nothing is written until Apply.** The overlay edits a
//!   [`TerrainSettingsDraft`], not the component, so the expensive rebuild
//!   happens exactly once, for the size you actually chose.
//! * **The cost is shown before you commit.** Vertices, colliders and memory,
//!   live, with a warning past [`COST_WARN_VERTICES`]. The old sliders let you
//!   walk into a multi-million-vertex terrain with nothing to suggest that was
//!   different from a small one.
//!
//! The grid itself is picked by clicking a cell rather than by two numbers,
//! because "4 × 3" is a shape and a shape is easier to point at than to type.
//! Anything past [`PICKER_MAX`] is rarer and goes through the numeric fields
//! beside it.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_display, bind_text};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    drag_value, dropdown_compact, toggle_switch, DragRange, DragSnap, OverlaySurface,
};

use renzora_terrain::data::TerrainData;
use renzora_terrain::grid::{estimate_cost, TerrainCost, COST_WARN_VERTICES, MAX_CHUNKS_PER_AXIS};

/// The clickable picker runs to this many chunks per side. Past it, the two
/// numeric fields take over — a 32×32 picker would be 1024 cells for a size
/// almost nobody builds.
const PICKER_MAX: u32 = 12;
/// Cell edge, in logical px.
const CELL: f32 = 15.0;
const CELL_GAP: f32 = 2.0;

/// Resolutions offered. Powers of two plus one, so neighbouring chunks share
/// their edge vertices exactly; a free-typed value would tear the seams.
const RESOLUTIONS: [u32; 4] = [33, 65, 129, 257];
const RESOLUTION_LABELS: [&str; 4] = ["33", "65", "129", "257"];

/// The staged edit. Nothing here touches the world until [`ApplyBtn`] is clicked.
#[derive(Resource, Default)]
pub struct TerrainSettingsDraft {
    pub visible: bool,
    /// The terrain being edited. Cleared with the overlay.
    pub target: Option<Entity>,
    pub chunks_x: u32,
    pub chunks_z: u32,
    pub chunk_size: f32,
    pub resolution: u32,
    pub min_height: f32,
    pub max_height: f32,
    pub stream_chunks: bool,
    pub stream_radius: f32,
}

impl TerrainSettingsDraft {
    fn seed(&mut self, entity: Entity, data: &TerrainData) {
        self.visible = true;
        self.target = Some(entity);
        self.chunks_x = data.chunks_x;
        self.chunks_z = data.chunks_z;
        self.chunk_size = data.chunk_size;
        self.resolution = data.chunk_resolution;
        self.min_height = data.min_height;
        self.max_height = data.max_height;
        self.stream_chunks = data.stream_chunks;
        self.stream_radius = data.stream_radius;
    }

    fn cost(&self) -> TerrainCost {
        estimate_cost(self.chunks_x, self.chunks_z, self.resolution)
    }
}

/// Open the overlay on `entity`. Called from the inspector's "Edit Terrain…"
/// button.
pub fn open(world: &mut World, entity: Entity) {
    let Some(data) = world.get::<TerrainData>(entity).cloned() else {
        return;
    };
    let mut draft = world.get_resource_or_insert_with(TerrainSettingsDraft::default);
    draft.seed(entity, &data);
}

pub fn register(app: &mut App) {
    app.init_resource::<TerrainSettingsDraft>().add_systems(
        Update,
        (
            grid_cell_click,
            close_click,
            apply_click,
            escape_closes,
        ),
    );
    app.add_systems(Update, manage_modal);
}

// ── Markers ─────────────────────────────────────────────────────────────────

#[derive(Component)]
struct OverlayRoot;
#[derive(Component, Clone, Copy)]
struct GridCell {
    x: u32,
    z: u32,
}
#[derive(Component)]
struct CloseBtn;
#[derive(Component)]
struct ApplyBtn;

// ── Lifecycle ───────────────────────────────────────────────────────────────

fn manage_modal(world: &mut World) {
    let visible = world
        .get_resource::<TerrainSettingsDraft>()
        .is_some_and(|d| d.visible);
    let existing: Vec<Entity> = world
        .query_filtered::<Entity, With<OverlayRoot>>()
        .iter(world)
        .collect();

    if visible && existing.is_empty() {
        let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
            return;
        };
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            spawn_modal(&mut commands, &fonts);
        }
        queue.apply(world);
    } else if !visible && !existing.is_empty() {
        for e in existing {
            world.entity_mut(e).despawn();
        }
    }
}

fn close_click(
    q: Query<&Interaction, (With<CloseBtn>, Changed<Interaction>)>,
    mut draft: ResMut<TerrainSettingsDraft>,
) {
    for interaction in &q {
        if *interaction == Interaction::Pressed {
            draft.visible = false;
            draft.target = None;
        }
    }
}

fn escape_closes(keys: Res<ButtonInput<KeyCode>>, mut draft: ResMut<TerrainSettingsDraft>) {
    if draft.visible && keys.just_pressed(KeyCode::Escape) {
        draft.visible = false;
        draft.target = None;
    }
}

/// The one write. Everything the overlay edited lands on `TerrainData` in a
/// single change, so the rebuild runs once.
fn apply_click(
    q: Query<&Interaction, (With<ApplyBtn>, Changed<Interaction>)>,
    mut commands: Commands,
    mut draft: ResMut<TerrainSettingsDraft>,
    mut terrains: Query<&mut TerrainData>,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Some(target) = draft.target else { return };
    if let Ok(mut data) = terrains.get_mut(target) {
        // Compare before writing: applying with nothing changed must not flag
        // the component and trigger a pointless full rebuild.
        let want = (
            draft.chunks_x.clamp(1, MAX_CHUNKS_PER_AXIS),
            draft.chunks_z.clamp(1, MAX_CHUNKS_PER_AXIS),
            draft.chunk_size.clamp(8.0, 512.0),
            draft.resolution,
            draft.min_height,
            // Keep at least 1 m of range, or `height_range()` collapses to zero
            // and every normal comes out NaN.
            draft.max_height.max(draft.min_height + 1.0),
            draft.stream_chunks,
            draft.stream_radius.clamp(32.0, 4096.0),
        );
        let current = (
            data.chunks_x,
            data.chunks_z,
            data.chunk_size,
            data.chunk_resolution,
            data.min_height,
            data.max_height,
            data.stream_chunks,
            data.stream_radius,
        );
        if want != current {
            data.chunks_x = want.0;
            data.chunks_z = want.1;
            data.chunk_size = want.2;
            data.chunk_resolution = want.3;
            data.min_height = want.4;
            data.max_height = want.5;
            data.stream_chunks = want.6;
            data.stream_radius = want.7;
        }
    }
    let _ = &mut commands;
    draft.visible = false;
    draft.target = None;
}

fn grid_cell_click(
    q: Query<(&Interaction, &GridCell), Changed<Interaction>>,
    mut draft: ResMut<TerrainSettingsDraft>,
) {
    for (interaction, cell) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        // A cell means "make the grid this big", so the clicked cell is the far
        // corner — hence the +1.
        draft.chunks_x = cell.x + 1;
        draft.chunks_z = cell.z + 1;
    }
}

// ── Layout ──────────────────────────────────────────────────────────────────

fn spawn_modal(commands: &mut Commands, fonts: &EmberFonts) {
    let backdrop = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.63)),
            GlobalZIndex(9300),
            // Without `OverlaySurface` the pointer state leaks through to the
            // viewport: clicks and scrolls land on the scene behind the dialog.
            OverlaySurface,
            FocusPolicy::Block,
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            OverlayRoot,
            Name::new("terrain-settings-modal"),
        ))
        .id();

    let panel = commands
        .spawn((
            Node {
                width: Val::Px(460.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(18.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            BorderColor::all(rgb(border())),
            FocusPolicy::Block,
            Name::new("terrain-settings-panel"),
        ))
        .id();
    commands.entity(backdrop).add_child(panel);

    let header = header_row(commands, fonts);
    let divider = rule(commands);
    let grid = grid_section(commands, fonts);
    let fields = field_section(commands, fonts);
    let cost = cost_section(commands, fonts);
    let buttons = button_row(commands, fonts);

    commands
        .entity(panel)
        .add_children(&[header, divider, grid, fields, cost, buttons]);
}

fn header_row(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .id();
    let left = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(7.0),
            ..default()
        })
        .id();
    let icon = icon_text(commands, &fonts.phosphor, "mountains", text_primary(), 15.0);
    let title = commands
        .spawn((
            Text::new("Terrain Settings"),
            ui_font(&fonts.ui, 14.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    commands.entity(left).add_children(&[icon, title]);

    let close = commands
        .spawn((
            Node {
                padding: UiRect::all(Val::Px(3.0)),
                ..default()
            },
            Interaction::default(),
            HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            CloseBtn,
            Name::new("terrain-settings-close"),
        ))
        .id();
    let cx = icon_text(commands, &fonts.phosphor, "x", text_muted(), 15.0);
    // The glyph must not swallow the click meant for its button.
    commands.entity(cx).insert(FocusPolicy::Pass);
    commands.entity(close).add_child(cx);

    commands.entity(row).add_children(&[left, close]);
    row
}

/// The clickable grid picker, plus the two numeric fields that cover sizes past
/// [`PICKER_MAX`].
fn grid_section(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let label = caption(commands, fonts, "Grid");
    commands.entity(col).add_child(label);

    let body = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FlexStart,
            column_gap: Val::Px(14.0),
            ..default()
        })
        .id();

    // The picker: PICKER_MAX rows of PICKER_MAX cells.
    let picker = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(CELL_GAP),
                ..default()
            },
            Name::new("terrain-grid-picker"),
        ))
        .id();
    for z in 0..PICKER_MAX {
        let row = commands
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(CELL_GAP),
                ..default()
            })
            .id();
        let cells: Vec<Entity> = (0..PICKER_MAX)
            .map(|x| {
                let cell = commands
                    .spawn((
                        Node {
                            width: Val::Px(CELL),
                            height: Val::Px(CELL),
                            border_radius: BorderRadius::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(rgb(card_bg())),
                        Interaction::default(),
                        HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                        GridCell { x, z },
                        Name::new(format!("grid-cell-{x}-{z}")),
                    ))
                    .id();
                // Filled when inside the chosen grid; a lighter fill previews
                // what hovering would select, so the shape is legible before
                // committing to it.
                bind_bg(commands, cell, move |w| {
                    let (cx, cz) = draft_grid(w);
                    let inside = x < cx && z < cz;
                    if inside {
                        rgb(accent())
                    } else if matches!(
                        w.get::<Interaction>(cell),
                        Some(Interaction::Hovered) | Some(Interaction::Pressed)
                    ) {
                        rgb(hover_bg())
                    } else {
                        rgb(card_bg())
                    }
                });
                cell
            })
            .collect();
        commands.entity(row).add_children(&cells);
        commands.entity(picker).add_child(row);
    }

    // Beside it: the readout and the two fields, for sizes the picker can't reach.
    let side = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            flex_grow: 1.0,
            ..default()
        })
        .id();
    let summary = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    bind_text(commands, summary, |w| {
        let (x, z) = draft_grid(w);
        format!("{x} × {z} chunks")
    });
    let extent = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_text(commands, extent, |w| {
        let d = w.get_resource::<TerrainSettingsDraft>();
        let (size, (x, z)) = (
            d.map(|d| d.chunk_size).unwrap_or(64.0),
            draft_grid(w),
        );
        format!("{:.0} × {:.0} m", x as f32 * size, z as f32 * size)
    });
    let fx = labelled_drag(
        commands,
        fonts,
        "Chunks X",
        1.0,
        MAX_CHUNKS_PER_AXIS as f32,
        0.1,
        Some(1.0),
        |w| draft_get(w, |d| d.chunks_x as f32),
        |w, v| draft_set(w, |d| d.chunks_x = clamp_axis(*v)),
    );
    let fz = labelled_drag(
        commands,
        fonts,
        "Chunks Z",
        1.0,
        MAX_CHUNKS_PER_AXIS as f32,
        0.1,
        Some(1.0),
        |w| draft_get(w, |d| d.chunks_z as f32),
        |w, v| draft_set(w, |d| d.chunks_z = clamp_axis(*v)),
    );
    commands
        .entity(side)
        .add_children(&[summary, extent, fx, fz]);

    commands.entity(body).add_children(&[picker, side]);
    commands.entity(col).add_child(body);
    col
}

fn field_section(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();

    let size = labelled_drag(
        commands,
        fonts,
        "Chunk Size",
        8.0,
        512.0,
        0.5,
        None,
        |w| draft_get(w, |d| d.chunk_size),
        |w, v| draft_set(w, |d| d.chunk_size = v.clamp(8.0, 512.0)),
    );

    let res_row = field_row(commands, fonts, "Resolution");
    let res = dropdown_compact(commands, fonts, &RESOLUTION_LABELS, 2, 76.0);
    bind_2way(
        commands,
        res,
        |w: &Rx| {
            let cur = draft_get(w, |d| d.resolution as f32) as u32;
            RESOLUTIONS.iter().position(|r| *r == cur).unwrap_or(2)
        },
        |w: &mut World, i: &usize| {
            let r = RESOLUTIONS.get(*i).copied().unwrap_or(129);
            draft_set(w, |d| d.resolution = r);
        },
    );
    commands.entity(res_row).add_child(res);

    let min = labelled_drag(
        commands,
        fonts,
        "Min Height",
        -500.0,
        500.0,
        0.25,
        None,
        |w| draft_get(w, |d| d.min_height),
        |w, v| draft_set(w, |d| d.min_height = *v),
    );
    let max = labelled_drag(
        commands,
        fonts,
        "Max Height",
        -500.0,
        500.0,
        0.25,
        None,
        |w| draft_get(w, |d| d.max_height),
        |w, v| draft_set(w, |d| d.max_height = *v),
    );

    let stream_row = field_row(commands, fonts, "Stream Chunks");
    let sw = toggle_switch(commands, false);
    bind_2way(
        commands,
        sw,
        |w: &Rx| {
            w.get_resource::<TerrainSettingsDraft>()
                .map(|d| d.stream_chunks)
                .unwrap_or(false)
        },
        |w: &mut World, v: &bool| draft_set(w, |d| d.stream_chunks = *v),
    );
    commands.entity(stream_row).add_child(sw);

    let radius = labelled_drag(
        commands,
        fonts,
        "Stream Radius",
        32.0,
        4096.0,
        1.0,
        None,
        |w| draft_get(w, |d| d.stream_radius),
        |w, v| draft_set(w, |d| d.stream_radius = v.clamp(32.0, 4096.0)),
    );
    // Only relevant while streaming is on.
    bind_display(commands, radius, |w| {
        w.get_resource::<TerrainSettingsDraft>()
            .map(|d| d.stream_chunks)
            .unwrap_or(false)
    });

    commands
        .entity(col)
        .add_children(&[size, res_row, min, max, stream_row, radius]);
    col
}

/// The readout that the old sliders never had.
fn cost_section(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    let divider = rule(commands);
    let line = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_text(commands, line, |w| {
        let Some(d) = w.get_resource::<TerrainSettingsDraft>() else {
            return String::new();
        };
        let c = d.cost();
        format!(
            "{} chunks · {} vertices · {} · {} colliders",
            c.chunks,
            fmt_count(c.vertices),
            fmt_bytes(c.bytes),
            c.chunks
        )
    });

    let warn = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.85, 0.35, 0.25, 0.18)),
        ))
        .id();
    let warn_text = commands
        .spawn((
            Text::new(
                "This size takes a while to build — every chunk also gets a triangle-mesh \
                 collider. Consider a lower resolution or fewer chunks.",
            ),
            ui_font(&fonts.ui, 11.0),
            TextColor(Color::srgb(0.95, 0.62, 0.5)),
        ))
        .id();
    commands.entity(warn).add_child(warn_text);
    bind_display(commands, warn, |w| {
        w.get_resource::<TerrainSettingsDraft>()
            .is_some_and(|d| d.cost().vertices > COST_WARN_VERTICES)
    });

    commands.entity(col).add_children(&[divider, line, warn]);
    col
}

fn button_row(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::FlexEnd,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let cancel = action_button(commands, fonts, "Cancel", false);
    commands.entity(cancel).insert(CloseBtn);
    let apply = action_button(commands, fonts, "Apply", true);
    commands.entity(apply).insert(ApplyBtn);
    commands.entity(row).add_children(&[cancel, apply]);
    row
}

// ── Small builders ──────────────────────────────────────────────────────────

fn action_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    primary: bool,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                height: Val::Px(26.0),
                padding: UiRect::horizontal(Val::Px(16.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(if primary { rgb(accent()) } else { rgb(card_bg()) }),
            Interaction::default(),
            HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new(format!("terrain-settings-{label}")),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        let hovered = matches!(
            w.get::<Interaction>(btn),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        );
        match (primary, hovered) {
            (true, _) => rgb(accent()),
            (false, true) => rgb(hover_bg()),
            (false, false) => rgb(card_bg()),
        }
    });
    let t = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 12.0),
            TextColor(if primary {
                Color::WHITE
            } else {
                rgb(text_primary())
            }),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(btn).add_child(t);
    btn
}

fn caption(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id()
}

fn rule(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(1.0),
                ..default()
            },
            BackgroundColor(rgb(divider())),
        ))
        .id()
}

fn field_row(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let lbl = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node {
                width: Val::Px(96.0),
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .id();
    commands.entity(row).add_child(lbl);
    row
}

#[allow(clippy::too_many_arguments)]
fn labelled_drag<G, S>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    min: f32,
    max: f32,
    step: f32,
    snap: Option<f32>,
    get: G,
    set: S,
) -> Entity
where
    G: Fn(&Rx) -> f32 + Send + Sync + 'static,
    S: Fn(&mut World, &f32) + Send + Sync + 'static,
{
    let row = field_row(commands, fonts, label);
    let dv = drag_value(commands, &fonts.ui, "", value_text(), min, step);
    commands.entity(dv).insert(DragRange { min, max });
    if let Some(s) = snap {
        commands.entity(dv).insert(DragSnap(s));
    }
    bind_2way(commands, dv, get, set);
    commands.entity(row).add_child(dv);
    row
}

// ── Draft accessors ─────────────────────────────────────────────────────────

fn draft_grid(w: &Rx) -> (u32, u32) {
    w.get_resource::<TerrainSettingsDraft>()
        .map(|d| (d.chunks_x, d.chunks_z))
        .unwrap_or((1, 1))
}

fn draft_get(w: &Rx, f: impl Fn(&TerrainSettingsDraft) -> f32) -> f32 {
    w.get_resource::<TerrainSettingsDraft>().map(f).unwrap_or(0.0)
}

fn draft_set(w: &mut World, f: impl FnOnce(&mut TerrainSettingsDraft)) {
    if let Some(mut d) = w.get_resource_mut::<TerrainSettingsDraft>() {
        f(&mut d);
    }
}

fn clamp_axis(v: f32) -> u32 {
    (v.round().max(1.0) as u32).clamp(1, MAX_CHUNKS_PER_AXIS)
}

// ── Formatting ──────────────────────────────────────────────────────────────

/// Vertex counts run to the millions; the exact digit count is never what you
/// want to read at a glance.
fn fmt_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1} M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.0} k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn fmt_bytes(n: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    if n as f64 >= MB {
        format!("{:.0} MB", n as f64 / MB)
    } else {
        format!("{:.0} KB", n as f64 / 1024.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeding_copies_every_editable_field() {
        let data = TerrainData {
            chunks_x: 5,
            chunks_z: 3,
            chunk_size: 96.0,
            chunk_resolution: 257,
            min_height: -20.0,
            max_height: 80.0,
            stream_chunks: true,
            stream_radius: 512.0,
        };
        let mut draft = TerrainSettingsDraft::default();
        draft.seed(Entity::from_raw_u32(1).unwrap(), &data);
        assert!(draft.visible);
        assert_eq!((draft.chunks_x, draft.chunks_z), (5, 3));
        assert_eq!(draft.chunk_size, 96.0);
        assert_eq!(draft.resolution, 257);
        assert_eq!((draft.min_height, draft.max_height), (-20.0, 80.0));
        assert!(draft.stream_chunks);
        assert_eq!(draft.stream_radius, 512.0);
    }

    /// The picker's cell (x, z) selects an (x+1) × (z+1) grid — clicking the
    /// top-left cell must give a 1×1 terrain, not a 0×0 one.
    #[test]
    fn axis_clamp_never_yields_zero_chunks() {
        assert_eq!(clamp_axis(0.0), 1);
        assert_eq!(clamp_axis(-4.0), 1);
        assert_eq!(clamp_axis(3.4), 3);
        assert_eq!(clamp_axis(1000.0), MAX_CHUNKS_PER_AXIS);
    }

    #[test]
    fn counts_read_as_magnitudes() {
        assert_eq!(fmt_count(512), "512");
        assert_eq!(fmt_count(66_049), "66 k");
        assert_eq!(fmt_count(4_227_136), "4.2 M");
    }

    #[test]
    fn every_offered_resolution_has_a_label() {
        assert_eq!(RESOLUTIONS.len(), RESOLUTION_LABELS.len());
        for (r, l) in RESOLUTIONS.iter().zip(RESOLUTION_LABELS.iter()) {
            assert_eq!(r.to_string(), *l);
        }
    }
}
