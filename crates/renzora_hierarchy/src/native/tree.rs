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

use super::row::{build_row, RowSnapshot};
use super::HierExpanded;


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
    /// Bumped every time the flattened rows are rebuilt. The virtualized list
    /// reads this as its dirty token so it skips re-running the snapshot on
    /// frames where the visible rows didn't change (see [`hier_flat_version`]).
    pub version: u64,
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
            has_script: node.has_script,
            has_blueprint: node.has_blueprint,
            has_material: node.has_material,
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
    flat.version = flat.version.wrapping_add(1);
}

/// Dirty token for the hierarchy's virtualized list: the flatten version, which
/// bumps whenever the visible rows are rebuilt. The scroll window is folded in
/// by `virtual_scroll_versioned`, so this only needs to track content changes.
pub(crate) fn hier_flat_version(world: &World) -> u64 {
    world
        .get_resource::<HierFlatCache>()
        .map(|f| f.version)
        .unwrap_or(0)
}

/// The keyed-list snapshot: the **full** row list (one `(key, hash)` per visible
/// tree row) plus a builder over the cached rows. Virtualization — building only
/// the rows in the scroll window — is handled generically by
/// [`renzora_ember::virtual_scroll`], which wraps this. Cheap per frame: a clone
/// of the cached `items` + an `Arc` clone of the row data.
pub(crate) fn hierarchy_snapshot(world: &World) -> KeyedSnapshot {
    let Some(flat) = world.get_resource::<HierFlatCache>() else {
        return empty_snapshot();
    };
    if flat.rows.is_empty() {
        return empty_snapshot();
    }

    let items = flat.items.clone();
    let rows = flat.rows.clone();
    KeyedSnapshot {
        items,
        // `i` is the full row index — keeps the zebra stripe stable across scroll.
        build: Box::new(move |commands, fonts, i| build_row(commands, fonts, &rows[i], i)),
    }
}

fn empty_snapshot() -> KeyedSnapshot {
    KeyedSnapshot {
        items: Vec::new(),
        build: Box::new(|_, _, _| Entity::PLACEHOLDER),
    }
}

