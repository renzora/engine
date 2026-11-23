//! The World Partition panel's map: every cell of the grid seen from above,
//! coloured by whether it is loaded, loading or unloaded, pinned cells
//! outlined, and the viewport camera's cell framed. Clicking a cell pins it.
//!
//! One image redrawn when [`PartitionMap`] changes, as the terrain chunk map
//! does: a city is a thousand cells, and as UI nodes each would be an entity
//! with its own reactive binding.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use renzora_scene_api::{EditorCamera, PartitionCell, PartitionCellPinned, WorldPartition};
use renzora_ember::theme::*;
use renzora_ember::{UiCursor, UiInteraction};
use renzora_engine::scene_stream::SceneStreams;

/// What one cell is doing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum CellState {
    Unloaded,
    Loading,
    Loaded,
}

/// The scene's partition as the panel shows it. Rewritten only when something
/// on it changed, so the panel's readouts stay quiet while nothing moves.
#[derive(Resource, Default, Clone, PartialEq, Debug)]
pub(crate) struct PartitionMap {
    pub present: bool,
    pub cell_size: f32,
    pub stream_in_editor: bool,
    pub load_all: bool,
    /// Grid bounds, inclusive.
    pub min: IVec2,
    pub max: IVec2,
    /// Every cell: coordinate, state, pinned.
    pub cells: Vec<(IVec2, CellState, bool)>,
    pub camera: Option<IVec2>,
    pub hover: Option<IVec2>,
    /// Cells with a distant stand-in built.
    pub with_hlod: usize,
}

impl PartitionMap {
    pub fn count(&self, state: CellState) -> usize {
        self.cells.iter().filter(|(_, s, _)| *s == state).count()
    }
    pub fn pinned(&self) -> usize {
        self.cells.iter().filter(|(_, _, p)| *p).count()
    }
    fn cols(&self) -> i32 {
        self.max.x - self.min.x + 1
    }
    fn rows(&self) -> i32 {
        self.max.y - self.min.y + 1
    }
}

/// Marker on the map's image node.
#[derive(Component, Default)]
pub(crate) struct PartitionMapView {
    drawn: Option<(PartitionMap, [u8; 4])>,
}

/// Rebuild [`PartitionMap`] from the world, and track the pointer.
pub(crate) fn refresh_partition_map(
    partitions: Query<&WorldPartition>,
    cells: Query<(Entity, &PartitionCell, Has<PartitionCellPinned>)>,
    children: Query<&Children>,
    streams: Res<SceneStreams>,
    camera: Query<&GlobalTransform, With<EditorCamera>>,
    views: Query<&UiCursor, With<PartitionMapView>>,
    mut map: ResMut<PartitionMap>,
) {
    let mut next = PartitionMap::default();
    if let Some(p) = partitions.iter().next() {
        next.present = true;
        next.cell_size = p.cell_size;
        next.stream_in_editor = p.stream_in_editor;
        next.load_all = p.editor_load_all;
        for (entity, cell, pinned) in &cells {
            if cell.has_hlod {
                next.with_hlod += 1;
            }
            let state = if children.get(entity).is_ok_and(|c| !c.is_empty()) {
                CellState::Loaded
            } else if streams.has_stream_under(entity) {
                CellState::Loading
            } else {
                CellState::Unloaded
            };
            next.cells.push((cell.coord(), state, pinned));
        }
        next.cells.sort_by_key(|(c, _, _)| (c.y, c.x));
        next.camera = camera.iter().next().map(|t| p.cell_of(t.translation()));
        if let Some(first) = next.cells.first().map(|c| c.0).or(next.camera) {
            let (mut min, mut max) = (first, first);
            for c in next.cells.iter().map(|c| c.0).chain(next.camera) {
                min = min.min(c);
                max = max.max(c);
            }
            next.min = min;
            next.max = max;
        }
        next.hover = views.iter().find_map(|cursor| {
            let n = cursor.normalized.filter(|_| cursor.cursor_over)? + Vec2::splat(0.5);
            Some(IVec2::new(
                next.min.x + (n.x * next.cols() as f32).floor() as i32,
                next.min.y + (n.y * next.rows() as f32).floor() as i32,
            ))
        });
    }
    if *map != next {
        *map = next;
    }
}

/// Click on the map: pin or unpin the cell under the pointer.
pub(crate) fn partition_map_click(
    views: Query<&UiInteraction, (With<PartitionMapView>, Changed<UiInteraction>)>,
    map: Res<PartitionMap>,
    mut commands: Commands,
) {
    if !views.iter().any(|i| *i == UiInteraction::Pressed) {
        return;
    }
    let Some(hover) = map.hover else { return };
    if !map.cells.iter().any(|(c, _, _)| *c == hover) {
        return;
    }
    commands.queue(move |world: &mut World| super::toggle_cell_pin(world, hover));
}

/// Pixels per cell: large enough to click on a small grid, small enough that
/// a grid of a few hundred cells a side still fits one texture.
fn cell_px(map: &PartitionMap) -> i32 {
    (480 / map.cols().max(map.rows()).max(1)).clamp(3, 14)
}

/// The map's pixels for `map`, in the theme's colours.
pub(crate) fn map_pixels(map: &PartitionMap) -> Pixels {
    let cell = cell_px(map);
    let gap = if cell >= 6 { 1 } else { 0 };
    let (cols, rows) = (map.cols().max(1), map.rows().max(1));
    let mut px = Pixels::new(
        (cols * (cell + gap) + gap) as u32,
        (rows * (cell + gap) + gap) as u32,
    );
    let loaded = rgba8(rgb(accent()), 1.0);
    let loading = rgba8(rgb(accent()), 0.45);
    let unloaded = rgba8(rgb(border()), 0.9);
    let pin = rgba8(rgb(warn_amber()), 1.0);
    let camera = rgba8(rgb(text_primary()), 1.0);
    let hover = rgba8(rgb(text_muted()), 1.0);

    let at = |c: IVec2| {
        (
            (c.x - map.min.x) * (cell + gap) + gap,
            (c.y - map.min.y) * (cell + gap) + gap,
        )
    };
    for &(c, state, pinned) in &map.cells {
        let (x, y) = at(c);
        let colour = match state {
            CellState::Loaded => loaded,
            CellState::Loading => loading,
            CellState::Unloaded => unloaded,
        };
        px.rect(x, y, cell, cell, colour);
        if pinned && cell >= 5 {
            px.frame(x + 1, y + 1, cell - 2, cell - 2, pin);
        }
    }
    if let Some(h) = map.hover.filter(|h| map.cells.iter().any(|(c, _, _)| c == h)) {
        let (x, y) = at(h);
        px.frame(x, y, cell, cell, hover);
    }
    if let Some(c) = map.camera {
        let (x, y) = at(c);
        px.frame(x - 1, y - 1, cell + 2, cell + 2, camera);
    }
    px
}

/// Redraw the map when it or the theme changed.
pub(crate) fn draw_partition_map(
    mut views: Query<(&mut PartitionMapView, &ImageNode, &mut Node)>,
    map: Res<PartitionMap>,
    mut images: ResMut<Assets<Image>>,
) {
    let accent_px = rgba8(rgb(accent()), 1.0);
    for (mut view, image, mut node) in &mut views {
        if view.drawn.as_ref().is_some_and(|(m, a)| m == &*map && *a == accent_px) {
            continue;
        }
        let px = map_pixels(&map);
        let ratio = px.w as f32 / px.h.max(1) as f32;
        if node.aspect_ratio != Some(ratio) {
            node.aspect_ratio = Some(ratio);
        }
        write_pixels(&mut images, image, px.w, px.h, px.data);
        view.drawn = Some((map.clone(), accent_px));
    }
}

// ── Canvas ──────────────────────────────────────────────────────────────────
//
// The same few operations as the terrain editor's canvas module, which is
// private to that crate. Small enough that sharing them would cost more
// coupling than it saves.

/// A transparent RGBA image, sampled nearest so magnified cells stay crisp.
fn blank_image(w: u32, h: u32) -> Image {
    let mut image = Image::new(
        Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
        TextureDimension::D2,
        vec![0; (w.max(1) * h.max(1) * 4) as usize],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = bevy::image::ImageSampler::nearest();
    image
}

/// Give `node` an image of its own to draw into, once the spawn applies.
pub(crate) fn attach_canvas(commands: &mut Commands, node: Entity) {
    commands.queue(move |world: &mut World| {
        let handle = world.resource_mut::<Assets<Image>>().add(blank_image(1, 1));
        if let Ok(mut e) = world.get_entity_mut(node) {
            e.insert(ImageNode::new(handle));
        }
    });
}

fn write_pixels(images: &mut Assets<Image>, node: &ImageNode, w: u32, h: u32, pixels: Vec<u8>) {
    let Some(mut img) = images.get_mut(&node.image) else { return };
    let size = img.texture_descriptor.size;
    if size.width == w && size.height == h {
        img.data = Some(pixels);
    } else {
        let mut fresh = blank_image(w, h);
        fresh.data = Some(pixels);
        *img = fresh;
    }
}

fn rgba8(c: Color, alpha: f32) -> [u8; 4] {
    let s = c.to_srgba();
    [
        (s.red.clamp(0.0, 1.0) * 255.0).round() as u8,
        (s.green.clamp(0.0, 1.0) * 255.0).round() as u8,
        (s.blue.clamp(0.0, 1.0) * 255.0).round() as u8,
        (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

pub(crate) struct Pixels {
    pub w: u32,
    pub h: u32,
    pub data: Vec<u8>,
}

impl Pixels {
    fn new(w: u32, h: u32) -> Self {
        Self { w, h, data: vec![0; (w * h * 4) as usize] }
    }
    fn put(&mut self, x: i32, y: i32, c: [u8; 4]) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let i = ((y as u32 * self.w + x as u32) * 4) as usize;
        self.data[i..i + 4].copy_from_slice(&c);
    }
    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: [u8; 4]) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.put(xx, yy, c);
            }
        }
    }
    fn frame(&mut self, x: i32, y: i32, w: i32, h: i32, c: [u8; 4]) {
        for xx in x..x + w {
            self.put(xx, y, c);
            self.put(xx, y + h - 1, c);
        }
        for yy in y..y + h {
            self.put(x, yy, c);
            self.put(x + w - 1, yy, c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(cells: &[(i32, i32, CellState, bool)]) -> PartitionMap {
        let mut m = PartitionMap {
            present: true,
            cells: cells.iter().map(|&(x, z, s, p)| (IVec2::new(x, z), s, p)).collect(),
            ..Default::default()
        };
        m.min = m.cells.iter().map(|c| c.0).reduce(IVec2::min).unwrap_or_default();
        m.max = m.cells.iter().map(|c| c.0).reduce(IVec2::max).unwrap_or_default();
        m
    }

    #[test]
    fn counts_follow_cell_states() {
        let m = map(&[
            (0, 0, CellState::Loaded, true),
            (1, 0, CellState::Loading, false),
            (-2, 3, CellState::Unloaded, false),
        ]);
        assert_eq!(m.count(CellState::Loaded), 1);
        assert_eq!(m.count(CellState::Loading), 1);
        assert_eq!(m.count(CellState::Unloaded), 1);
        assert_eq!(m.pinned(), 1);
        assert_eq!((m.cols(), m.rows()), (4, 4));
    }

    /// A city-sized grid must still fit a sane texture, and a tiny one must be
    /// big enough to click.
    #[test]
    fn the_image_stays_a_sensible_size() {
        let small = map(&[(0, 0, CellState::Loaded, false), (1, 1, CellState::Loaded, false)]);
        assert_eq!(cell_px(&small), 14);
        let mut cells = Vec::new();
        for x in 0..300 {
            cells.push((x, 0, CellState::Unloaded, false));
        }
        let big = map(&cells);
        let px = map_pixels(&big);
        assert!(px.w <= 1024, "{}", px.w);
    }
}
