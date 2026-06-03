//! Flatten the cached entity tree into owned [`RowSnapshot`]s and expose it as a
//! [`KeyedSnapshot`] for the reactive keyed list (no manual rebuild gate — the
//! list driver diffs `(key, hash)` and rebuilds only changed rows).

use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use bevy_egui::egui::Color32;
use renzora_ember::reactive::KeyedSnapshot;

use crate::cache::HierarchyTreeCache;
use crate::state::EntityNode;

use super::row::{build_row, RowSnapshot};
use super::HierExpanded;

fn c32(c: Color32) -> Color {
    Color::srgba_u8(c.r(), c.g(), c.b(), c.a())
}

fn flatten(
    nodes: &[EntityNode],
    expanded: &HashSet<Entity>,
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
        });
        if has_children && is_expanded {
            parent_lines.push(!is_last);
            flatten(&node.children, expanded, parent_lines, out);
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

/// The keyed-list snapshot: `(entity-key, content-hash)` per visible row + a
/// builder that owns the flattened rows.
pub(crate) fn hierarchy_snapshot(world: &World) -> KeyedSnapshot {
    let exp = world
        .get_resource::<HierExpanded>()
        .map(|e| e.0.clone())
        .unwrap_or_default();
    // Active filters: the funnel's component-type set + the search box's name
    // text. Either prunes the tree (and search force-expands so matches show).
    let filter = world
        .get_resource::<super::filter::HierFilter>()
        .map(|f| f.types.clone())
        .unwrap_or_default();
    let search = world
        .get_resource::<super::filter::HierSearch>()
        .map(|s| s.query.trim().to_string())
        .unwrap_or_default();
    let type_active = !filter.is_empty();
    let search_active = !search.is_empty();
    let filtered = (type_active || search_active)
        .then(|| world.get_resource::<HierarchyTreeCache>())
        .flatten()
        .map(|c| {
            let mut nodes = c.nodes.clone();
            if type_active {
                nodes = crate::state::filter_tree_by_type(nodes, &filter);
            }
            if search_active {
                nodes = crate::state::filter_tree(nodes, &search);
            }
            nodes
        });
    let mut rows = Vec::new();
    let cache = world.get_resource::<HierarchyTreeCache>();
    let nodes: &[crate::state::EntityNode] = match &filtered {
        Some(v) => v.as_slice(),
        None => cache.map_or(&[][..], |c| c.nodes.as_slice()),
    };
    // While searching, force every (matching) node open so deep hits are visible.
    let mut all_exp;
    let exp_ref: &bevy::platform::collections::HashSet<Entity> = if search_active {
        all_exp = bevy::platform::collections::HashSet::new();
        collect_entities(nodes, &mut all_exp);
        &all_exp
    } else {
        &exp
    };
    {
        let mut parent_lines = Vec::new();
        flatten(nodes, exp_ref, &mut parent_lines, &mut rows);
    }
    // Fold the row's parity into its content hash so a row that changes odd/even
    // (e.g. when a sibling above is added/removed) repaints with the right stripe.
    let items: Vec<(u64, u64)> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| (r.key(), r.content_hash().wrapping_mul(31).wrapping_add((i & 1) as u64)))
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |commands, fonts, i| build_row(commands, fonts, &rows[i], i)),
    }
}
