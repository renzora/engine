//! Generic list/grid virtualization for ember panels.
//!
//! A scrollable panel that builds one UI entity per item pays for *every* item
//! each frame — layout, change-detection, render — even the thousands scrolled
//! off screen. [`virtual_scroll`] fixes that for any panel: it wraps a normal
//! [`keyed_list`] snapshot so only the items inside (or near) the viewport are
//! actually built, with two empty spacer nodes standing in for the rows above
//! and below so the scrollbar and total height stay correct.
//!
//! It's the one shared implementation — the hierarchy and the asset browser both
//! build on it rather than hand-rolling their own windowing.
//!
//! ### Self-measuring
//! Item heights vary (the asset grid's tiles scale with a zoom slider) and the
//! column count depends on the panel width, so the panel doesn't declare any of
//! that. Instead [`update_virtual_metrics`] reads the *laid-out* children each
//! frame and derives the row stride and column count from their actual
//! positions. Until it has seen at least two rows it renders everything (the
//! first-frame fallback), then windows from the measurement — so it adapts to
//! zoom, DPI and resizes for free.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, ScrollPosition, UiGlobalTransform};

use crate::reactive::{keyed_list, keyed_list_tokened, KeyedSnapshot};

/// Sentinel keys for the top/bottom spacer rows. Astronomically unlikely to
/// collide with a real item key (an entity bits / path hash), and each list has
/// its own diff map anyway.
const SPACER_TOP_KEY: u64 = u64::MAX;
const SPACER_BOT_KEY: u64 = u64::MAX - 1;

/// Marks a content node as a virtualized list. Added by [`virtual_scroll`].
#[derive(Component, Clone, Copy)]
pub struct VirtualList {
    /// Extra rows built above and below the viewport so a fast scroll doesn't
    /// flash empty before the next frame's window catches up.
    pub overscan: usize,
}

/// Live scroll/layout measurement for a [`VirtualList`], refreshed each frame by
/// [`update_virtual_metrics`] and read by the windowing snapshot.
#[derive(Component, Clone, Copy, Default)]
pub struct VirtualMetrics {
    /// Scroll offset of the viewport (logical px).
    pub offset: f32,
    /// Visible height of the viewport (logical px).
    pub viewport_h: f32,
    /// Measured vertical stride between rows incl. gap (logical px).
    pub row_h: f32,
    /// Measured number of items per row.
    pub columns: usize,
    /// True once at least two rows have been measured (so `row_h`/`columns` are
    /// trustworthy). While false the list renders everything.
    pub measured: bool,
}

/// Tags a spacer node so the measurement pass skips it.
#[derive(Component)]
pub struct VirtualSpacer;

/// Wrap a keyed-list `snapshot` so only the visible window is built.
///
/// `snapshot` is the panel's normal full-list snapshot: `items` holds one
/// `(key, hash)` per item in display order, and `build(commands, fonts, i)`
/// builds the item at the **full** index `i`. `virtual_scroll` slices that to
/// the viewport and remaps indices, so a panel adopts it by swapping its
/// `keyed_list(.., snapshot)` call for `virtual_scroll(.., overscan, snapshot)`
/// — no change to the snapshot itself. `container` is the same content node it
/// would pass to `keyed_list` (the one wrapped in a `scroll_view`).
pub fn virtual_scroll<F>(
    commands: &mut Commands,
    container: Entity,
    overscan: usize,
    snapshot: F,
) where
    F: Fn(&World) -> KeyedSnapshot + Send + Sync + 'static,
{
    virtual_scroll_impl(commands, container, overscan, None, snapshot);
}

/// Like [`virtual_scroll`], but skips re-running `snapshot` on frames where
/// neither the data nor the scroll window changed. `version` is a cheap
/// `&World -> u64` that bumps whenever the underlying list contents change;
/// the scroll position and viewport size are folded in automatically, so the
/// window still rebuilds while scrolling/resizing. Use this when the snapshot
/// is expensive to build (large lists) and the consumer can supply a version.
pub fn virtual_scroll_versioned<V, F>(
    commands: &mut Commands,
    container: Entity,
    overscan: usize,
    version: V,
    snapshot: F,
) where
    V: Fn(&World) -> u64 + Send + Sync + 'static,
    F: Fn(&World) -> KeyedSnapshot + Send + Sync + 'static,
{
    virtual_scroll_impl(commands, container, overscan, Some(Box::new(version)), snapshot);
}

fn virtual_scroll_impl<F>(
    commands: &mut Commands,
    container: Entity,
    overscan: usize,
    version: Option<Box<dyn Fn(&World) -> u64 + Send + Sync>>,
    snapshot: F,
) where
    F: Fn(&World) -> KeyedSnapshot + Send + Sync + 'static,
{
    commands
        .entity(container)
        .insert((VirtualList { overscan }, VirtualMetrics::default()));

    let windowed = move |world: &World| {
        let m = world
            .get::<VirtualMetrics>(container)
            .copied()
            .unwrap_or_default();
        let full = snapshot(world);
        let total = full.items.len();

        // Render everything until we've measured a real row stride (first
        // frames, or a list short enough to never wrap to a second row).
        if !m.measured || m.row_h <= 0.0 || m.columns == 0 || m.viewport_h <= 0.0 || total == 0 {
            return full;
        }

        let cols = m.columns;
        let row_h = m.row_h;
        let total_rows = total.div_ceil(cols);

        let first_row = (m.offset / row_h).floor().max(0.0) as usize;
        let start_row = first_row.saturating_sub(overscan);
        let rows_in_view = (m.viewport_h / row_h).ceil() as usize + 1;
        let end_row = (start_row + rows_in_view + overscan * 2).min(total_rows);

        let start = (start_row * cols).min(total);
        let end = (end_row * cols).min(total);
        let top_h = start_row as f32 * row_h;
        let bot_h = total_rows.saturating_sub(end_row) as f32 * row_h;

        let mut items: Vec<(u64, u64)> = Vec::with_capacity(end - start + 2);
        items.push((SPACER_TOP_KEY, top_h.to_bits() as u64));
        items.extend_from_slice(&full.items[start..end]);
        items.push((SPACER_BOT_KEY, bot_h.to_bits() as u64));

        let inner = full.build;
        let count = items.len();
        KeyedSnapshot {
            items,
            build: Box::new(move |c, f, i| {
                if i == 0 {
                    spacer(c, top_h)
                } else if i + 1 == count {
                    spacer(c, bot_h)
                } else {
                    // Map the windowed index back to the full list index so
                    // zebra striping / row data stay stable across scroll.
                    inner(c, f, start + (i - 1))
                }
            }),
        }
    };

    match version {
        Some(v) => keyed_list_tokened(
            commands,
            container,
            move |world: &World| {
                use std::hash::{Hash, Hasher};
                let m = world
                    .get::<VirtualMetrics>(container)
                    .copied()
                    .unwrap_or_default();
                // Rebuild only when the data version or the visible window moves.
                let mut h = std::collections::hash_map::DefaultHasher::new();
                v(world).hash(&mut h);
                m.offset.to_bits().hash(&mut h);
                m.viewport_h.to_bits().hash(&mut h);
                m.row_h.to_bits().hash(&mut h);
                m.columns.hash(&mut h);
                m.measured.hash(&mut h);
                h.finish()
            },
            windowed,
        ),
        None => keyed_list(commands, container, windowed),
    }
}

/// A zero-content filler reserving `height` px for the rows outside the window.
/// No `Name`, so it never shows in a hierarchy or dirties a tree cache.
fn spacer(commands: &mut Commands, height: f32) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(height.max(0.0)),
                flex_shrink: 0.0,
                ..default()
            },
            VirtualSpacer,
        ))
        .id()
}

/// Plugin: registers the per-frame measurement system.
pub struct VirtualScrollPlugin;

impl Plugin for VirtualScrollPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_virtual_metrics);
    }
}

/// Measure each virtual list's scroll offset, viewport height, row stride and
/// column count from the laid-out children, into its [`VirtualMetrics`].
///
/// Row stride and columns come from the children's actual positions, so they're
/// exact regardless of zoom, gaps, padding or DPI — no per-panel constants.
/// Reads the previous frame's computed layout (a frame of lag the overscan
/// absorbs).
fn update_virtual_metrics(
    mut lists: Query<(Entity, &VirtualList, &mut VirtualMetrics, &Children)>,
    parents: Query<&ChildOf>,
    viewports: Query<(&ScrollPosition, &ComputedNode)>,
    items: Query<(&ComputedNode, &UiGlobalTransform), Without<VirtualSpacer>>,
) {
    for (entity, _vl, mut m, children) in &mut lists {
        let Some((offset, viewport_h, inv)) = nearest_viewport(entity, &parents, &viewports) else {
            continue;
        };
        m.offset = offset;
        m.viewport_h = viewport_h;

        // Collect (top-edge y, height) for the real (non-spacer) children, in
        // logical px. UiGlobalTransform.translation is the node centre.
        let mut rows: Vec<(f32, f32)> = Vec::new();
        for child in children.iter() {
            if let Ok((cn, ugt)) = items.get(child) {
                let size = cn.size() * inv;
                let top = ugt.translation.y * inv - size.y * 0.5;
                rows.push((top, size.y));
            }
        }
        if rows.is_empty() {
            continue;
        }

        // First row = the minimum top edge; columns = how many share it.
        let row0 = rows.iter().map(|r| r.0).fold(f32::INFINITY, f32::min);
        const EPS: f32 = 2.0;
        let columns = rows.iter().filter(|r| (r.0 - row0).abs() < EPS).count().max(1);
        // Stride = distance to the next distinct row. Only trust the window
        // (measured = true) once we've actually seen a second row.
        let row1 = rows
            .iter()
            .map(|r| r.0)
            .filter(|&y| y > row0 + EPS)
            .fold(f32::INFINITY, f32::min);

        m.columns = columns;
        if row1.is_finite() {
            m.row_h = (row1 - row0).max(1.0);
            m.measured = true;
        } else {
            // Single visible row → can't derive a stride; keep rendering all.
            m.measured = false;
        }
    }
}

/// Walk up from `node` to the nearest ancestor that is a scroll viewport
/// (carries `ScrollPosition` + `ComputedNode`). Returns
/// `(scroll_offset, viewport_height_logical, inverse_scale_factor)`.
fn nearest_viewport(
    node: Entity,
    parents: &Query<&ChildOf>,
    viewports: &Query<(&ScrollPosition, &ComputedNode)>,
) -> Option<(f32, f32, f32)> {
    let mut e = node;
    for _ in 0..8 {
        let parent = parents.get(e).ok()?.parent();
        if let Ok((sp, cn)) = viewports.get(parent) {
            let inv = cn.inverse_scale_factor();
            return Some((sp.y, cn.size().y * inv, inv));
        }
        e = parent;
    }
    None
}
