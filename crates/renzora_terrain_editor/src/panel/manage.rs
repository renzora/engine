//! The Manage mode: the whole terrain in one place. The chunk map and its
//! stats, the size and streaming card, the generator graph, and heightmap
//! import and export.
//!
//! It replaces three entry points that each opened a different kind of UI:
//! the Resize scene tool (now the chunk map), the Edit Terrain dialog (now the
//! card) and the Generate toolbar (now the generator graph card).
//!
//! **Chunk size and resolution wait for Apply.** Either one rebuilds every
//! chunk with a fresh collider, and a drag writes on every tick: dragging the
//! resolution through its values would build the terrain once per value. So
//! the card stages those two in [`ManageDraft`] and writes them in one change.
//! Height range, streaming and detail levels take the cheap in-place path in
//! `terrain_data_changed_system` and apply live.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::reactive::tracked::{bind_display, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::collapsible;
use renzora_ember::UiInteraction;

use renzora_terrain::data::TerrainData;
use renzora_terrain::grid::{estimate_cost, TerrainCost, COST_WARN_VERTICES};
use renzora_terrain_graph::{exposed_params, BuildState, TerrainGenerator, TerrainGraphBuilds};
use renzora_terrain_graph_editor::generator_inspector::GraphFiles;

use super::chunk_map::{ChunkMap, ChunkMapView};
use super::widgets::{
    button, card, column, group_label, hint, live_text, num_field, num_row, row, segmented_row, spacer,
    switch_row, unit, wrap_row,
};
use super::{file_stem, hasher, AssetSlot, ManageAction};

/// Resolutions offered. Powers of two plus one, so neighbouring chunks share
/// their edge vertices exactly; a free value would tear the seams.
pub(crate) const RESOLUTIONS: [u32; 4] = [33, 65, 129, 257];
const RESOLUTION_LABELS: [&str; 4] = ["33", "65", "129", "257"];
/// Chunk size limits, in metres.
pub(crate) const CHUNK_SIZE_RANGE: (f32, f32) = (8.0, 512.0);
/// The most coarser levels a chunk can have: six halvings take a 129-vertex
/// chunk down to two cells a side.
const MAX_LOD_LEVELS: u32 = 6;

// ── The staged edit ──────────────────────────────────────────────────────────

/// Chunk size and resolution as edited but not applied. `None` means the
/// terrain's own value; a staged value equal to the terrain's is dropped, so
/// "pending" is exactly "Apply would change something".
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub(crate) struct ManageDraft {
    pub target: Option<Entity>,
    pub chunk_size: Option<f32>,
    pub resolution: Option<u32>,
}

impl ManageDraft {
    /// Forget a draft made for another terrain.
    pub fn retarget(&mut self, terrain: Entity) {
        if self.target != Some(terrain) {
            *self = Self { target: Some(terrain), ..default() };
        }
    }

    pub fn stage_chunk_size(&mut self, data: &TerrainData, v: f32) {
        let v = v.clamp(CHUNK_SIZE_RANGE.0, CHUNK_SIZE_RANGE.1);
        self.chunk_size = (v != data.chunk_size).then_some(v);
    }

    pub fn stage_resolution(&mut self, data: &TerrainData, v: u32) {
        self.resolution = (v != data.chunk_resolution && RESOLUTIONS.contains(&v)).then_some(v);
    }

    pub fn chunk_size(&self, data: &TerrainData) -> f32 {
        self.chunk_size.unwrap_or(data.chunk_size)
    }

    pub fn resolution(&self, data: &TerrainData) -> u32 {
        self.resolution.unwrap_or(data.chunk_resolution)
    }

    pub fn is_pending(&self) -> bool {
        self.chunk_size.is_some() || self.resolution.is_some()
    }

    /// What the terrain would cost to build with the staged values.
    pub fn cost(&self, data: &TerrainData) -> TerrainCost {
        estimate_cost(data.chunks_x, data.chunks_z, self.resolution(data))
    }

    /// Write the staged values into `data` and clear the draft. Returns
    /// whether anything changed, so the caller writes the component only then.
    pub fn apply_to(&mut self, data: &mut TerrainData) -> bool {
        let mut changed = false;
        if let Some(s) = self.chunk_size.take() {
            changed |= data.chunk_size != s;
            data.chunk_size = s;
        }
        if let Some(r) = self.resolution.take() {
            changed |= data.chunk_resolution != r;
            data.chunk_resolution = r;
        }
        changed
    }

    pub fn revert(&mut self) {
        self.chunk_size = None;
        self.resolution = None;
    }
}

/// Apply the draft to its terrain: one write, one rebuild.
pub(crate) fn apply_draft(world: &mut World) {
    let Some(mut draft) = world.get_resource::<ManageDraft>().cloned() else { return };
    let Some(target) = draft.target else { return };
    let Some(mut data) = world.get::<TerrainData>(target).cloned() else { return };
    if draft.apply_to(&mut data) {
        if let Some(mut live) = world.get_mut::<TerrainData>(target) {
            *live = data;
        }
    }
    world.insert_resource(draft);
}

// ── Formatting ───────────────────────────────────────────────────────────────

/// A footprint the way a person says it: metres below a kilometre,
/// kilometres with one decimal above.
pub(crate) fn fmt_size(w: f32, d: f32) -> String {
    if w >= 1000.0 || d >= 1000.0 {
        format!("{:.1} x {:.1} km", w / 1000.0, d / 1000.0)
    } else {
        format!("{w:.0} x {d:.0} m")
    }
}

pub(crate) fn fmt_bytes(n: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    if n as f64 >= 1024.0 * MB {
        format!("{:.1} GB", n as f64 / (1024.0 * MB))
    } else if n as f64 >= MB {
        format!("{:.0} MB", n as f64 / MB)
    } else {
        format!("{:.0} KB", n as f64 / 1024.0)
    }
}

pub(crate) fn fmt_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1} M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.0} k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// "2 min ago" from a gap in seconds.
pub(crate) fn fmt_ago(secs: f64) -> String {
    let s = secs.max(0.0);
    if s < 60.0 {
        "just now".into()
    } else if s < 3600.0 {
        format!("{:.0} min ago", (s / 60.0).floor())
    } else {
        format!("{:.0} h ago", (s / 3600.0).floor())
    }
}

// ── Accessors ────────────────────────────────────────────────────────────────

fn data<T: Default>(w: &Rx, terrain: Entity, f: impl Fn(&TerrainData) -> T) -> T {
    w.get::<TerrainData>(terrain).map(f).unwrap_or_default()
}

fn set_data(w: &mut World, terrain: Entity, f: impl FnOnce(&mut TerrainData)) {
    // Through a copy, and written only when it moved: every real write to
    // `TerrainData` runs the chunk rebuild check.
    let Some(mut d) = w.get::<TerrainData>(terrain).cloned() else { return };
    f(&mut d);
    if let Some(mut live) = w.get_mut::<TerrainData>(terrain) {
        let same = live.min_height == d.min_height
            && live.max_height == d.max_height
            && live.stream_chunks == d.stream_chunks
            && live.stream_radius == d.stream_radius
            && live.lod_levels == d.lod_levels
            && live.lod_distance == d.lod_distance;
        if !same {
            *live = d;
        }
    }
}

fn draft_get<T>(w: &Rx, terrain: Entity, f: impl Fn(&ManageDraft, &TerrainData) -> T, fallback: T) -> T {
    let Some(d) = w.get::<TerrainData>(terrain) else { return fallback };
    match w.get_resource::<ManageDraft>() {
        Some(draft) if draft.target == Some(terrain) => f(draft, d),
        _ => f(&ManageDraft::default(), d),
    }
}

fn draft_set(w: &mut World, terrain: Entity, f: impl FnOnce(&mut ManageDraft, &TerrainData)) {
    let Some(d) = w.get::<TerrainData>(terrain).cloned() else { return };
    let mut draft = w.get_resource::<ManageDraft>().cloned().unwrap_or_default();
    draft.retarget(terrain);
    f(&mut draft, &d);
    if w.get_resource::<ManageDraft>() != Some(&draft) {
        w.insert_resource(draft);
    }
}

fn pending(w: &Rx, terrain: Entity) -> bool {
    w.get_resource::<ManageDraft>().is_some_and(|d| d.target == Some(terrain) && d.is_pending())
}

// ── Layout ───────────────────────────────────────────────────────────────────

pub(super) fn manage_content(commands: &mut Commands, fonts: &EmberFonts, terrain: Entity) -> Entity {
    let root = column(commands, 10.0);
    let world = world_group(commands, fonts, terrain);
    let settings = settings_card(commands, fonts, terrain);
    let generator = generator_group(commands, fonts, terrain);
    let whole = whole_terrain_group(commands, fonts);
    let (hm, hm_body) = collapsible(commands, fonts, None, "Heightmap import and export", false);
    let hm_note = hint(commands, fonts, "Import a PNG or RAW16 heightmap into this terrain's chunks, or export its heights as a 16-bit PNG.");
    let hm_row = wrap_row(commands, 6.0);
    let import = button(commands, fonts, Some("download-simple"), "Import...", false);
    commands.entity(import).insert(ManageAction::ImportHeightmap);
    let export = button(commands, fonts, Some("upload-simple"), "Export...", false);
    commands.entity(export).insert(ManageAction::ExportHeightmap);
    commands.entity(hm_row).add_children(&[import, export]);
    commands.entity(hm_body).add_children(&[hm_note, hm_row]);
    commands.entity(root).add_children(&[world, settings, generator, whole, hm]);
    root
}

fn world_group(commands: &mut Commands, fonts: &EmberFonts, terrain: Entity) -> Entity {
    let col = column(commands, 6.0);
    let label = group_label(commands, fonts, "World", "click beside the grid to add, Ctrl+click an edge to remove");
    let view = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                max_height: Val::Px(280.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            BorderColor::all(rgb(border())),
            UiInteraction::default(),
            RelativeCursorPosition::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            ChunkMapView::default(),
            Name::new("terrain-chunk-map"),
        ))
        .id();
    super::canvas::attach_canvas(commands, view, 1, 1);

    let legend = wrap_row(commands, 12.0);
    let items = [
        (rgb(accent()), "Full detail"),
        (rgb(accent()).with_alpha(0.45), "Stand-in"),
        (rgb(border()), "Empty"),
    ];
    for (c, text) in items {
        let item = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(5.0), ..default() }).id();
        let sw = commands
            .spawn((Node { width: Val::Px(9.0), height: Val::Px(9.0), border_radius: BorderRadius::all(Val::Px(2.0)), ..default() }, BackgroundColor(c)))
            .id();
        let t = commands.spawn((Text::new(text), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_muted())))).id();
        commands.entity(item).add_children(&[sw, t]);
        commands.entity(legend).add_child(item);
    }
    let cam = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(5.0), ..default() }).id();
    let cam_sw = commands
        .spawn((
            Node { width: Val::Px(9.0), height: Val::Px(9.0), border: UiRect::all(Val::Px(1.5)), border_radius: BorderRadius::all(Val::Px(2.0)), ..default() },
            BorderColor::all(rgb(text_primary())),
        ))
        .id();
    let cam_t = commands.spawn((Text::new("Camera"), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_muted())))).id();
    commands.entity(cam).add_children(&[cam_sw, cam_t]);
    commands.entity(legend).add_child(cam);

    let stats = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), ..default() })
        .id();
    let size = stat(commands, fonts, "Size", move |w| data(w, terrain, |d| fmt_size(d.total_width(), d.total_depth())));
    let resident = stat(commands, fonts, "Resident", |w| {
        w.get_resource::<ChunkMap>().map(|m| format!("{} / {}", m.resident(), m.total())).unwrap_or_default()
    });
    let memory = stat(commands, fonts, "Memory", move |w| {
        let res = data(w, terrain, |d| d.chunk_resolution);
        let resident = w.get_resource::<ChunkMap>().map_or(0, |m| m.resident()) as u32;
        format!("~{}", fmt_bytes(estimate_cost(resident, 1, res).bytes))
    });
    commands.entity(stats).add_children(&[size, resident, memory]);
    commands.entity(col).add_children(&[label, view, legend, stats]);
    col
}

fn stat<F>(commands: &mut Commands, fonts: &EmberFonts, key: &str, value: F) -> Entity
where
    F: for<'w> Fn(&Rx<'w>) -> String + Send + Sync + 'static,
{
    let b = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                min_width: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id();
    let k = commands.spawn((Text::new(key.to_string()), ui_font(&fonts.ui, 10.0), TextColor(rgb(placeholder())))).id();
    let v = commands.spawn((Text::new(""), ui_font(&fonts.mono, 11.5), TextColor(rgb(text_primary())))).id();
    renzora_ember::reactive::tracked::bind_text(commands, v, value);
    commands.entity(b).add_children(&[k, v]);
    b
}

fn settings_card(commands: &mut Commands, fonts: &EmberFonts, terrain: Entity) -> Entity {
    let c = card(commands);
    let chunk_size = num_row(commands, fonts, "Chunk size", "m", CHUNK_SIZE_RANGE.0, CHUNK_SIZE_RANGE.1, 0.5,
        move |w| draft_get(w, terrain, |d, t| d.chunk_size(t), 64.0),
        move |w, v| draft_set(w, terrain, |d, t| d.stage_chunk_size(t, *v)),
    );
    let resolution = segmented_row(commands, fonts, "Resolution", &RESOLUTION_LABELS,
        move |w| {
            let r = draft_get(w, terrain, |d, t| d.resolution(t), 129);
            RESOLUTIONS.iter().position(|x| *x == r).unwrap_or(2)
        },
        move |w, i| draft_set(w, terrain, |d, t| d.stage_resolution(t, RESOLUTIONS[(*i).min(3)])),
    );

    let (heights, hc) = row(commands, fonts, "Height range");
    let lo = num_field(commands, fonts, "min", -2000.0, 8000.0, 0.25, None,
        move |w| data(w, terrain, |d| d.min_height),
        // Keep at least 1 m of range, or every normal comes out NaN.
        move |w, v| set_data(w, terrain, |d| d.min_height = v.min(d.max_height - 1.0)),
    );
    let hi = num_field(commands, fonts, "max", -2000.0, 8000.0, 0.25, None,
        move |w| data(w, terrain, |d| d.max_height),
        move |w, v| set_data(w, terrain, |d| d.max_height = v.max(d.min_height + 1.0)),
    );
    let hu = unit(commands, fonts, "m");
    commands.entity(hc).add_children(&[lo, hi, hu]);

    let (streaming, sc) = switch_row(commands, fonts, "Streaming",
        move |w| data(w, terrain, |d| d.stream_chunks),
        move |w, on| set_data(w, terrain, |d| d.stream_chunks = *on),
    );
    let radius = num_field(commands, fonts, "radius", 32.0, 4096.0, 1.0, None,
        move |w| data(w, terrain, |d| d.stream_radius),
        move |w, v| set_data(w, terrain, |d| d.stream_radius = *v),
    );
    let ru = unit(commands, fonts, "m");
    bind_display(commands, radius, move |w| data(w, terrain, |d| d.stream_chunks));
    bind_display(commands, ru, move |w| data(w, terrain, |d| d.stream_chunks));
    commands.entity(sc).add_children(&[radius, ru]);

    let (detail, dc) = row(commands, fonts, "Distant detail");
    let levels = num_field(commands, fonts, "levels", 0.0, MAX_LOD_LEVELS as f32, 0.05, Some(1.0),
        move |w| data(w, terrain, |d| d.lod_levels as f32),
        move |w, v| set_data(w, terrain, |d| d.lod_levels = v.round().clamp(0.0, MAX_LOD_LEVELS as f32) as u32),
    );
    let reach = num_field(commands, fonts, "reach", 25.0, 800.0, 1.0, None,
        move |w| data(w, terrain, |d| d.lod_distance * 100.0),
        move |w, v| set_data(w, terrain, |d| d.lod_distance = (*v / 100.0).clamp(0.25, 8.0)),
    );
    let du = unit(commands, fonts, "%");
    commands.entity(dc).add_children(&[levels, reach, du]);

    let cost = live_text(commands, fonts, 10.5, text_muted(), move |w| {
        let Some(d) = w.get::<TerrainData>(terrain) else { return String::new() };
        let draft = w.get_resource::<ManageDraft>().filter(|x| x.target == Some(terrain)).cloned().unwrap_or_default();
        let c = draft.cost(d);
        format!(
            "{} chunks of {} m: {} vertices, {}",
            c.chunks,
            draft.chunk_size(d),
            fmt_count(c.vertices),
            fmt_bytes(c.bytes)
        )
    });
    let warn = live_text(commands, fonts, 10.5, warn_amber(), |_| {
        "That many vertices takes a while to build: every chunk also gets a collider.".into()
    });
    bind_display(commands, warn, move |w| {
        w.get::<TerrainData>(terrain).is_some_and(|d| {
            let draft = w.get_resource::<ManageDraft>().filter(|x| x.target == Some(terrain)).cloned().unwrap_or_default();
            draft.cost(d).vertices > COST_WARN_VERTICES
        })
    });
    let note = hint(commands, fonts, "Chunk size and resolution rebuild every chunk, so they wait for Apply. Everything else is live.");
    let buttons = wrap_row(commands, 6.0);
    let sp = spacer(commands);
    let revert = button(commands, fonts, None, "Revert", false);
    commands.entity(revert).insert(ManageAction::Revert);
    let apply = button(commands, fonts, Some("check"), "Apply", true);
    commands.entity(apply).insert(ManageAction::Apply);
    commands.entity(buttons).add_children(&[sp, revert, apply]);
    bind_display(commands, buttons, move |w| pending(w, terrain));

    commands.entity(c).add_children(&[chunk_size, resolution, heights, streaming, detail, cost, warn, note, buttons]);
    c
}

// ── Generator ────────────────────────────────────────────────────────────────

fn generator<'w>(w: &Rx<'w>, terrain: Entity) -> Option<&'w TerrainGenerator> {
    w.get::<TerrainGenerator>(terrain)
}

fn generator_group(commands: &mut Commands, fonts: &EmberFonts, terrain: Entity) -> Entity {
    let col = column(commands, 6.0);
    let label = group_label(commands, fonts, "Generator", "the ground's starting shape");
    let c = card(commands);

    // The slot: the graph this terrain is built from. Drop a .tgraph on it.
    let slot = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            RelativeCursorPosition::default(),
            AssetSlot::Graph,
            Name::new("terrain-graph-slot"),
        ))
        .id();
    let thumb = commands
        .spawn((
            Node {
                width: Val::Px(40.0),
                height: Val::Px(40.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, "mountains", text_muted(), 20.0);
    commands.entity(ic).insert(PASS_THROUGH);
    commands.entity(thumb).add_child(ic);
    let text = commands
        .spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(1.0), ..default() })
        .id();
    let name = live_text(commands, fonts, 12.5, text_primary(), move |w| match generator(w, terrain) {
        Some(g) if !g.graph.is_empty() => {
            let parsed = w.get_resource::<GraphFiles>().and_then(|f| f.get(&g.graph)).map(|gr| gr.name.clone()).filter(|n| !n.is_empty());
            parsed.unwrap_or_else(|| file_stem(&g.graph))
        }
        _ => "No generator graph".into(),
    });
    let path = commands.spawn((Text::new(""), ui_font(&fonts.mono, 10.5), TextColor(rgb(text_muted())), PASS_THROUGH)).id();
    renzora_ember::reactive::tracked::bind_text(commands, path, move |w| {
        generator(w, terrain).map(|g| g.graph.clone()).filter(|p| !p.is_empty()).unwrap_or_else(|| "drop a .tgraph here".into())
    });
    let info = live_text(commands, fonts, 10.5, placeholder(), move |w| graph_info(w, terrain));
    commands.entity(text).add_children(&[name, path, info]);
    let choose = button(commands, fonts, None, "Choose...", false);
    commands.entity(choose).insert(ManageAction::ChooseGraph);
    let edit = button(commands, fonts, Some("pencil-simple"), "Edit", false);
    commands.entity(edit).insert(ManageAction::EditGraph);
    bind_display(commands, edit, move |w| generator(w, terrain).is_some_and(|g| !g.graph.is_empty()));
    commands.entity(slot).add_children(&[thumb, text, choose, edit]);

    // Everything below needs a graph.
    let body = column(commands, 6.0);
    bind_display(commands, body, move |w| generator(w, terrain).is_some_and(|g| !g.graph.is_empty()));
    let (seed, seed_c) = row(commands, fonts, "Seed");
    let seed_f = num_field(commands, fonts, "", 0.0, 0.0, 1.0, Some(1.0),
        move |w| generator(w, terrain).map_or(0.0, |g| g.seed as f32),
        move |w, v| {
            let s = v.max(0.0).round() as u64;
            if let Some(mut g) = w.get_mut::<TerrainGenerator>(terrain) {
                if g.seed != s {
                    g.seed = s;
                }
            }
        },
    );
    let reroll = button(commands, fonts, Some("dice-five"), "Re-roll", false);
    commands.entity(reroll).insert(ManageAction::Reroll);
    commands.entity(seed_c).add_children(&[seed_f, reroll]);

    let params = column(commands, 6.0);
    keyed_list(commands, params, move |w| params_snapshot(w, terrain));

    let note = hint(commands, fonts, "Sculpting stays on its own layer above the generator, so a rebuild keeps your hand edits.");

    let progress_track = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(4.0), border_radius: BorderRadius::all(Val::Px(2.0)), overflow: Overflow::clip(), ..default() },
            BackgroundColor(rgb(border())),
        ))
        .id();
    let progress_fill = commands.spawn((Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(rgb(accent())))).id();
    commands.entity(progress_track).add_child(progress_fill);
    let progress = move |w: &Rx| w.get_resource::<TerrainGraphBuilds>().and_then(|b| b.progress(terrain));
    bind_display(commands, progress_track, move |w| progress(w).is_some());
    renzora_ember::reactive::tracked::bind_with(commands, progress_fill, move |w| (progress(w).unwrap_or(0.0) * 100.0).round() as i32, |world, e, v: &i32| {
        if let Some(mut n) = world.get_mut::<Node>(e) {
            n.width = Val::Percent(*v as f32);
        }
    });

    let actions = wrap_row(commands, 6.0);
    let status = live_text(commands, fonts, 10.5, text_muted(), move |w| build_status(w, terrain));
    commands.entity(status).insert(Node { flex_grow: 1.0, flex_shrink: 1.0, min_width: Val::Px(0.0), ..default() });
    let cancel = button(commands, fonts, Some("x"), "Cancel", false);
    commands.entity(cancel).insert(ManageAction::CancelBuild);
    bind_display(commands, cancel, move |w| progress(w).is_some());
    let build = button(commands, fonts, Some("hammer"), "Build", true);
    commands.entity(build).insert(ManageAction::Build);
    commands.entity(actions).add_children(&[status, cancel, build]);

    commands.entity(body).add_children(&[seed, params, note, progress_track, actions]);

    let none = hint(commands, fonts, "Point the terrain at a terrain graph (.tgraph) to build its ground from nodes. New graphs come from the Assets panel's create menu.");
    bind_display(commands, none, move |w| !generator(w, terrain).is_some_and(|g| !g.graph.is_empty()));

    commands.entity(c).add_children(&[slot, body, none]);
    commands.entity(col).add_children(&[label, c]);
    col
}

/// "14 nodes, built 2 min ago".
fn graph_info(w: &Rx, terrain: Entity) -> String {
    let Some(g) = generator(w, terrain).filter(|g| !g.graph.is_empty()) else {
        return "Builds the base ground from a node graph".into();
    };
    let nodes = w.get_resource::<GraphFiles>().and_then(|f| f.get(&g.graph)).map(|gr| gr.nodes.len());
    let mut parts = Vec::new();
    if let Some(n) = nodes {
        parts.push(format!("{n} nodes"));
    }
    if let Some(b) = w.get_resource::<TerrainGraphBuilds>().and_then(|b| b.get(terrain)) {
        if let BuildState::Done { .. } = b.state {
            // Read untracked: a clock is "changed" every frame, and the line
            // only needs to move when something else redraws it.
            let now = w.manually_tracked().get_resource::<Time<Real>>().map_or(b.since, |t| t.elapsed_secs_f64());
            parts.push(format!("built {}", fmt_ago(now - b.since)));
        }
    }
    if parts.is_empty() {
        "not built this session".into()
    } else {
        parts.join(", ")
    }
}

fn build_status(w: &Rx, terrain: Entity) -> String {
    match w.get_resource::<TerrainGraphBuilds>().and_then(|b| b.get(terrain)).map(|b| &b.state) {
        Some(BuildState::Running { progress }) => format!("Building {:.0}%", progress * 100.0),
        Some(BuildState::Done { summary }) => summary.clone(),
        Some(BuildState::Failed { error }) => format!("Failed: {error}"),
        Some(BuildState::Cancelled) => "Cancelled".into(),
        None => String::new(),
    }
}

fn params_snapshot(w: &Rx, terrain: Entity) -> KeyedSnapshot {
    let params: Vec<(String, String, f32, f32)> = generator(w, terrain)
        .and_then(|g| {
            let graph = w.get_resource::<GraphFiles>()?.get(&g.graph)?;
            Some(exposed_params(graph, Some(g)).into_iter().map(|p| (p.name, p.label, p.min, p.max)).collect())
        })
        .unwrap_or_default();
    let items = params
        .iter()
        .map(|(name, label, min, max)| {
            let mut k = hasher();
            name.hash(&mut k);
            let mut h = hasher();
            (name, label, min.to_bits(), max.to_bits()).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (name, label, min, max) = params[i].clone();
            let read_name = name.clone();
            let label = if label.is_empty() { name.clone() } else { label };
            num_row(c, f, &label, "", min, max, ((max - min) / 400.0).max(0.01),
                move |w| {
                    generator(w, terrain)
                        .and_then(|g| {
                            let graph = w.get_resource::<GraphFiles>()?.get(&g.graph)?;
                            exposed_params(graph, Some(g)).into_iter().find(|p| p.name == read_name).map(|p| p.value)
                        })
                        .unwrap_or(min)
                },
                move |w, v| {
                    if let Some(mut g) = w.get_mut::<TerrainGenerator>(terrain) {
                        if g.override_value(&name) != Some(*v) {
                            g.set_override(&name, *v);
                        }
                    }
                },
            )
        }),
    }
}

fn whole_terrain_group(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = column(commands, 6.0);
    let label = group_label(commands, fonts, "Whole terrain", "one undo step each");
    let r = wrap_row(commands, 6.0);
    let erode = button(commands, fonts, Some("cloud-rain"), "Erode all", false);
    commands.entity(erode).insert(ManageAction::ErodeAll);
    let flatten = button(commands, fonts, Some("equals"), "Flatten all", false);
    commands.entity(flatten).insert(ManageAction::FlattenAll);
    commands.entity(r).add_children(&[erode, flatten]);
    commands.entity(col).add_children(&[label, r]);
    col
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terrain() -> TerrainData {
        TerrainData { chunks_x: 4, chunks_z: 4, chunk_size: 64.0, chunk_resolution: 129, ..TerrainData::default() }
    }

    /// Staging a value equal to the terrain's own is not a change: the Apply
    /// row stays hidden and Apply would write nothing.
    #[test]
    fn staging_the_current_value_is_not_pending() {
        let t = terrain();
        let mut d = ManageDraft::default();
        d.stage_chunk_size(&t, 64.0);
        d.stage_resolution(&t, 129);
        assert!(!d.is_pending());
        d.stage_resolution(&t, 257);
        assert!(d.is_pending());
        d.stage_resolution(&t, 129);
        assert!(!d.is_pending(), "putting it back clears it");
    }

    #[test]
    fn apply_writes_once_and_clears_the_draft() {
        let mut t = terrain();
        let mut d = ManageDraft::default();
        d.stage_chunk_size(&t, 128.0);
        d.stage_resolution(&t, 65);
        assert_eq!((d.chunk_size(&t), d.resolution(&t)), (128.0, 65));
        assert_eq!((t.chunk_size, t.chunk_resolution), (64.0, 129), "nothing written while staged");
        assert!(d.apply_to(&mut t));
        assert_eq!((t.chunk_size, t.chunk_resolution), (128.0, 65));
        assert!(!d.is_pending());
        assert!(!d.apply_to(&mut t), "a second Apply changes nothing");
    }

    #[test]
    fn revert_drops_the_staged_values() {
        let t = terrain();
        let mut d = ManageDraft::default();
        d.stage_chunk_size(&t, 200.0);
        d.revert();
        assert!(!d.is_pending());
        assert_eq!(d.chunk_size(&t), 64.0);
    }

    #[test]
    fn a_draft_does_not_follow_you_to_another_terrain() {
        let a = Entity::from_raw_u32(1).unwrap();
        let b = Entity::from_raw_u32(2).unwrap();
        let t = terrain();
        let mut d = ManageDraft::default();
        d.retarget(a);
        d.stage_resolution(&t, 33);
        d.retarget(a);
        assert!(d.is_pending());
        d.retarget(b);
        assert!(!d.is_pending());
    }

    #[test]
    fn only_seam_safe_resolutions_and_sane_sizes_are_staged() {
        let t = terrain();
        let mut d = ManageDraft::default();
        d.stage_resolution(&t, 100);
        assert!(!d.is_pending());
        d.stage_chunk_size(&t, 99_999.0);
        assert_eq!(d.chunk_size(&t), CHUNK_SIZE_RANGE.1);
    }

    #[test]
    fn the_cost_follows_the_staged_resolution() {
        let t = terrain();
        let mut d = ManageDraft::default();
        let before = d.cost(&t).vertices;
        d.stage_resolution(&t, 257);
        assert!(d.cost(&t).vertices > before);
    }

    #[test]
    fn readouts_use_real_units() {
        assert_eq!(fmt_size(2048.0, 2048.0), "2.0 x 2.0 km");
        assert_eq!(fmt_size(256.0, 128.0), "256 x 128 m");
        assert_eq!(fmt_ago(30.0), "just now");
        assert_eq!(fmt_ago(150.0), "2 min ago");
        assert_eq!(fmt_count(66_049), "66 k");
    }
}
