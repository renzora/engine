//! Flatten the cached entity tree into owned [`RowSnapshot`]s and expose it as a
//! [`KeyedSnapshot`] for the reactive keyed list (the list driver diffs
//! `(key, hash)` and rebuilds only changed rows).
//!
//! The flatten is O(visible rows) and allocates per row (name + parent-lines +
//! hash), so doing it every frame inside the snapshot (which the keyed-list
//! driver calls unconditionally) tanks the framerate once a big model is
//! expanded. Instead, [`update_flatten_cache`] rebuilds the flattened rows only
//! when something that affects them actually changes (tree cache version,
//! expansion, filter, search, rename), stashing the result in [`HierFlatCache`].
//! The per-frame [`hierarchy_snapshot`] then just hands back the cached
//! `(key, hash)` list + a builder over the cached rows.

use std::sync::Arc;

use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use renzora_ember::reactive::KeyedSnapshot;

use crate::cache::HierarchyTreeCache;
use crate::state::EntityNode;

use super::row::{build_row, RowSnapshot, ROW_H};
use super::HierExpanded;

/// Rows rendered above/below the visible window so a fast scroll (the metrics
/// can lag the snapshot by a frame) never shows a gap before the window catches
/// up.
const OVERSCAN: usize = 6;

/// Scroll geometry of the hierarchy's own viewport, refreshed each frame by
/// [`update_scroll_metrics`] so the (read-only) snapshot can window the list
/// without running a query itself.
#[derive(Resource, Default)]
pub(crate) struct HierScrollMetrics {
    pub offset: f32,
    pub viewport_h: f32,
}

/// Publish the hierarchy viewport's scroll offset + height for the snapshot's
/// windowing. The viewport is the parent of the marked content node.
pub(crate) fn update_scroll_metrics(
    content: Query<Entity, With<super::HierScrollContent>>,
    parents: Query<&ChildOf>,
    viewports: Query<(&bevy::ui::ScrollPosition, &bevy::ui::ComputedNode)>,
    mut metrics: ResMut<HierScrollMetrics>,
) {
    let Some(list) = content.iter().next() else {
        return;
    };
    let Ok(vp) = parents.get(list).map(|c| c.parent()) else {
        return;
    };
    let Ok((sp, cn)) = viewports.get(vp) else {
        return;
    };
    metrics.offset = sp.y;
    metrics.viewport_h = cn.size().y * cn.inverse_scale_factor();
}

/// Cached flatten of the tree, rebuilt only on change by [`update_flatten_cache`].
#[derive(Resource, Default)]
pub(crate) struct HierFlatCache {
    /// `(key, content-hash)` per visible row, in display order — what the keyed
    /// list diffs against. Cloned cheaply each frame by [`hierarchy_snapshot`].
    pub items: Vec<(u64, u64)>,
    /// The owned row data, shared into the build closure via `Arc` (no per-frame
    /// clone of the row `String`s / `Vec`s).
    pub rows: Arc<Vec<RowSnapshot>>,
    /// Visible entity order (parallel to `rows`) so per-frame consumers (parent
    /// stacking) get it without re-flattening the tree.
    pub order: Vec<Entity>,
}

fn c32([r, g, b]: [u8; 3]) -> Color {
    Color::srgba_u8(r, g, b, 255)
}

#[allow(clippy::too_many_arguments)]
fn flatten(
    nodes: &[EntityNode],
    expanded: &HashSet<Entity>,
    renaming: Option<Entity>,
    parent_lines: &mut Vec<bool>,
    out: &mut Vec<RowSnapshot>,
) {
    let count = nodes.len();
    for (i, node) in nodes.iter().enumerate() {
        let is_last = i == count - 1;
        let has_children = !node.children.is_empty();
        let is_expanded = expanded.contains(&node.entity);
        out.push(RowSnapshot {
            entity: node.entity,
            name: node.name.clone(),
            icon: node.icon,
            icon_color: c32(node.icon_color),
            label_color: node.label_color,
            is_visible: node.is_visible,
            is_locked: node.is_locked,
            is_default_camera: node.is_default_camera,
            depth: parent_lines.len(),
            is_last,
            parent_lines: parent_lines.clone(),
            is_expanded,
            has_children,
            is_renaming: renaming == Some(node.entity),
        });
        if has_children && is_expanded {
            parent_lines.push(!is_last);
            flatten(&node.children, expanded, renaming, parent_lines, out);
            parent_lines.pop();
        }
    }
}

/// Collect every entity in `nodes` (recursively) — used to force-expand the
/// filtered tree while searching.
fn collect_entities(nodes: &[crate::state::EntityNode], out: &mut HashSet<Entity>) {
    for n in nodes {
        out.insert(n.entity);
        collect_entities(&n.children, out);
    }
}

/// Rebuild [`HierFlatCache`] when (and only when) the flatten inputs change.
/// Runs every frame but early-returns in O(1) unless the tree cache version,
/// expansion set, filter, search, or rename target changed since last run.
pub(crate) fn update_flatten_cache(
    mut flat: ResMut<HierFlatCache>,
    cache: Res<HierarchyTreeCache>,
    expanded: Res<HierExpanded>,
    filter: Res<super::filter::HierFilter>,
    search: Res<super::filter::HierSearch>,
    rename: Res<super::rename::HierRename>,
    mut inited: Local<bool>,
    mut last_ver: Local<u64>,
) {
    let dirty = !*inited
        || cache.version != *last_ver
        || expanded.is_changed()
        || filter.is_changed()
        || search.is_changed()
        || rename.is_changed();
    if !dirty {
        return;
    }
    *inited = true;
    *last_ver = cache.version;

    // Active filters: the funnel's component-type set + the search box's name
    // text. Either prunes the tree (and search force-expands so matches show).
    let type_set = &filter.types;
    let query = search.query.trim();
    let type_active = !type_set.is_empty();
    let search_active = !query.is_empty();
    let filtered = (type_active || search_active).then(|| {
        let mut nodes = cache.nodes.clone();
        if type_active {
            nodes = crate::state::filter_tree_by_type(nodes, type_set);
        }
        if search_active {
            nodes = crate::state::filter_tree(nodes, query);
        }
        nodes
    });
    let nodes: &[EntityNode] = match &filtered {
        Some(v) => v.as_slice(),
        None => cache.nodes.as_slice(),
    };
    // While searching, force every (matching) node open so deep hits are visible.
    let mut all_exp;
    let exp_ref: &HashSet<Entity> = if search_active {
        all_exp = HashSet::new();
        collect_entities(nodes, &mut all_exp);
        &all_exp
    } else {
        &expanded.0
    };
    let renaming = rename.0;

    let mut rows = Vec::new();
    {
        let mut parent_lines = Vec::new();
        flatten(nodes, exp_ref, renaming, &mut parent_lines, &mut rows);
    }
    // Fold the row's parity into its content hash so a row that changes odd/even
    // (e.g. when a sibling above is added/removed) repaints with the right stripe.
    flat.items = rows
        .iter()
        .enumerate()
        .map(|(i, r)| (r.key(), r.content_hash().wrapping_mul(31).wrapping_add((i & 1) as u64)))
        .collect();
    flat.order = rows.iter().map(|r| r.entity).collect();
    flat.rows = Arc::new(rows);
}

/// The keyed-list snapshot — **virtualised**. Only the rows inside the scroll
/// window (plus overscan) are emitted; the off-screen rows above and below are
/// represented by two zero-content spacer nodes that reserve their height. This
/// keeps the live UI-node count (and per-row reactions) proportional to the
/// viewport, not to how many rows are expanded — without it, expanding a large
/// model spawns tens of thousands of `bevy_ui` nodes and the framerate tanks.
///
/// The total content height is preserved (`top_spacer + window + bottom_spacer`
/// == `total * ROW_H`), so the scrollbar and scroll-to math are unaffected.
/// Cheap per frame: a slice of the cached `items` + an `Arc` clone.
pub(crate) fn hierarchy_snapshot(world: &World) -> KeyedSnapshot {
    let Some(flat) = world.get_resource::<HierFlatCache>() else {
        return empty_snapshot();
    };
    let total = flat.rows.len();
    if total == 0 {
        return empty_snapshot();
    }

    let (offset, vh) = world
        .get_resource::<HierScrollMetrics>()
        .map(|m| (m.offset, m.viewport_h))
        .unwrap_or((0.0, 0.0));

    // Window of rows to actually build. If the viewport hasn't been measured yet
    // (first frame), fall back to rendering everything so nothing is missing.
    let (start, end) = if vh <= 0.0 {
        (0, total)
    } else {
        let first = (offset / ROW_H).floor() as usize;
        let start = first.saturating_sub(OVERSCAN);
        let rows_in_view = (vh / ROW_H).ceil() as usize + 1;
        let end = (start + rows_in_view + OVERSCAN * 2).min(total);
        (start, end)
    };

    // top spacer + window rows + bottom spacer. Spacer keys are sentinels that
    // can't collide with an entity's `to_bits`; their hash is the reserved row
    // count, so the spacer rebuilds (resizes) when the window slides.
    let mut items: Vec<(u64, u64)> = Vec::with_capacity(end - start + 2);
    items.push((u64::MAX, start as u64));
    items.extend_from_slice(&flat.items[start..end]);
    items.push((u64::MAX - 1, (total - end) as u64));

    let rows = flat.rows.clone();
    let count = items.len();
    KeyedSnapshot {
        items,
        build: Box::new(move |commands, fonts, i| {
            if i == 0 {
                spacer_node(commands, start as f32 * ROW_H)
            } else if i == count - 1 {
                spacer_node(commands, (total - end) as f32 * ROW_H)
            } else {
                // Global row index — keeps the zebra stripe stable across scroll.
                let g = start + (i - 1);
                build_row(commands, fonts, &rows[g], g)
            }
        }),
    }
}

fn empty_snapshot() -> KeyedSnapshot {
    KeyedSnapshot {
        items: Vec::new(),
        build: Box::new(|_, _, _| Entity::PLACEHOLDER),
    }
}

/// A zero-content filler that reserves `height` px of scroll space for the rows
/// outside the rendered window. No `Name` (so it never dirties the tree cache).
fn spacer_node(commands: &mut Commands, height: f32) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(height.max(0.0)),
                flex_shrink: 0.0,
                ..default()
            },
            bevy::picking::Pickable::IGNORE,
        ))
        .id()
}
