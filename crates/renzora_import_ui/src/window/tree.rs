//! The scene tree: which rows are visible, what a prune would leave behind, and
//! the snapshot that turns both into widgets.

use bevy::prelude::*;

use renzora_ember::reactive::{KeyedSnapshot, Rx};

use crate::staged::thousands;

use super::lists::hash_of;
use super::rows::{list_row, staged, RowCheck, RowSpec};
use super::{ImportNav, TreeItem, TreeRow};

/// Walk the scene graph depth-first, descending only into expanded rows, and
/// return what should be visible.
///
/// Flattened rather than built as nested widgets because a scene can carry well
/// over a thousand nodes and the reactive list rebuilds on every dirty frame —
/// nesting a thousand collapsible widgets to show twenty of them is the shape
/// that makes an ember panel drop frames.
/// Each row is `(item, depth, disabled)`, where `disabled` means an ancestor is
/// unchecked — the row is coming out of the import whatever its own box says.
fn visible_tree_rows(
    stats: &renzora_import::GlbStats,
    expanded: &std::collections::HashSet<TreeItem>,
    excluded: &renzora_import::PruneSpec,
) -> Vec<(TreeItem, usize, bool)> {
    /// Guard against a pathological expand — one node with thousands of
    /// children would otherwise build thousands of rows in a 310px pane.
    const MAX_ROWS: usize = 500;
    let mut out: Vec<(TreeItem, usize, bool)> = Vec::new();

    struct Walk<'a> {
        stats: &'a renzora_import::GlbStats,
        expanded: &'a std::collections::HashSet<TreeItem>,
        excluded: &'a renzora_import::PruneSpec,
    }

    fn walk(
        w: &Walk,
        idx: usize,
        depth: usize,
        disabled: bool,
        out: &mut Vec<(TreeItem, usize, bool)>,
        max: usize,
    ) {
        if out.len() >= max {
            return;
        }
        let Some(node) = w.stats.node_list.get(idx) else {
            return;
        };
        let item = TreeItem::Node(idx);
        out.push((item, depth, disabled));
        if !w.expanded.contains(&item) {
            return;
        }
        // Unchecking a node takes its whole subtree with it, so everything
        // below this point is disabled once it is excluded.
        let below = disabled || w.excluded.nodes.contains(&idx);
        // The mesh first, then child nodes — geometry belongs to this node,
        // children are separate objects.
        if let Some(mi) = node.mesh {
            if let Some(mesh) = w.stats.mesh_list.get(mi) {
                let m_item = TreeItem::Mesh(mi);
                out.push((m_item, depth + 1, below));
                if w.expanded.contains(&m_item) {
                    let prim_disabled = below || w.excluded.meshes.contains(&mi);
                    for k in 0..mesh.primitives.len() {
                        if out.len() >= max {
                            return;
                        }
                        out.push((TreeItem::Prim(mi, k), depth + 2, prim_disabled));
                    }
                }
            }
        }
        for &child in &node.children {
            walk(w, child, depth + 1, below, out, max);
        }
    }

    let w = Walk { stats, expanded, excluded };
    for &root in &stats.roots {
        walk(&w, root, 0, false, &mut out, MAX_ROWS);
    }
    out
}

/// What an import with the current checkboxes would actually contain:
/// `(meshes, materials)`, by glTF index.
///
/// The mesh and material lists use this to show what is on its way out. They
/// have no checkboxes of their own — a material is not a thing you can uncheck,
/// it is a thing that stops being used once nothing references it — so this is
/// the same reachability walk the prune does, run for display.
pub(super) fn surviving(
    stats: &renzora_import::GlbStats,
    excluded: &renzora_import::PruneSpec,
) -> (std::collections::HashSet<usize>, std::collections::HashSet<usize>) {
    let mut meshes = std::collections::HashSet::new();
    let mut materials = std::collections::HashSet::new();
    let mut seen = std::collections::HashSet::new();
    let mut stack: Vec<usize> = stats.roots.clone();
    while let Some(n) = stack.pop() {
        if excluded.nodes.contains(&n) || !seen.insert(n) {
            continue;
        }
        let Some(node) = stats.node_list.get(n) else {
            continue;
        };
        stack.extend(node.children.iter().copied());
        let Some(mi) = node.mesh.filter(|mi| !excluded.meshes.contains(mi)) else {
            continue;
        };
        let Some(mesh) = stats.mesh_list.get(mi) else {
            continue;
        };
        let live: Vec<usize> = (0..mesh.primitives.len())
            .filter(|k| !excluded.prims.contains(&(mi, *k)))
            .collect();
        if live.is_empty() {
            continue;
        }
        meshes.insert(mi);
        materials.extend(live.iter().filter_map(|&k| mesh.primitives[k].material));
    }
    (meshes, materials)
}

/// Is this row's own box ticked, ignoring whether an ancestor overrules it?
fn item_included(excluded: &renzora_import::PruneSpec, item: TreeItem) -> bool {
    match item {
        TreeItem::Node(i) => !excluded.nodes.contains(&i),
        TreeItem::Mesh(mi) => !excluded.meshes.contains(&mi),
        TreeItem::Prim(mi, k) => !excluded.prims.contains(&(mi, k)),
    }
}

/// Whether a tree row can be opened, and the label/detail/icon it shows.
pub(super) fn tree_row_parts(
    stats: &renzora_import::GlbStats,
    item: TreeItem,
) -> (String, String, &'static str, bool) {
    match item {
        TreeItem::Node(i) => {
            let Some(n) = stats.node_list.get(i) else {
                return (format!("Node {i}"), String::new(), "cube", false);
            };
            let expandable = !n.children.is_empty() || n.mesh.is_some();
            let detail = if n.children.is_empty() {
                String::new()
            } else {
                format!("{} children", n.children.len())
            };
            let icon = if n.mesh.is_some() { "cube" } else { "circles-three" };
            (n.name.clone(), detail, icon, expandable)
        }
        TreeItem::Mesh(mi) => {
            let Some(m) = stats.mesh_list.get(mi) else {
                return (format!("Mesh {mi}"), String::new(), "polygon", false);
            };
            (
                m.name.clone(),
                format!("{} tris", thousands(m.triangles())),
                "polygon",
                m.primitives.len() > 1,
            )
        }
        TreeItem::Prim(mi, k) => {
            let name = stats
                .mesh_list
                .get(mi)
                .and_then(|m| m.primitives.get(k))
                .and_then(|p| p.material)
                .and_then(|x| stats.material_names.get(x))
                .cloned()
                .unwrap_or_else(|| format!("Surface {k}"));
            let tris = stats
                .mesh_list
                .get(mi)
                .and_then(|m| m.primitives.get(k))
                .map(|p| p.triangles)
                .unwrap_or(0);
            (name, format!("{} tris", thousands(tris)), "circle-half-tilt", false)
        }
    }
}

pub(super) fn scene_snapshot(world: &Rx) -> KeyedSnapshot {
    let empty = || KeyedSnapshot {
        items: Vec::new(),
        build: Box::new(|_, _, _| Entity::PLACEHOLDER),
    };
    let Some(st) = staged(world) else { return empty() };
    let Some(stats) = st.stats.clone() else { return empty() };
    let (expanded, selected) = world
        .get_resource::<ImportNav>()
        .map(|n| (n.expanded.clone(), n.sel_item))
        .unwrap_or_default();

    let built: Vec<TreeRowData> = visible_tree_rows(&stats, &expanded, &st.excluded)
        .into_iter()
        .map(|(item, depth, disabled)| {
            let (label, detail, _icon, expandable) = tree_row_parts(&stats, item);
            TreeRowData {
                item,
                depth,
                label,
                detail,
                caret: expandable.then(|| expanded.contains(&item)),
                selected: selected == Some(item),
                check: RowCheck {
                    item,
                    checked: item_included(&st.excluded, item),
                    enabled: !disabled,
                },
            }
        })
        .collect();

    let items: Vec<(u64, u64)> = built
        .iter()
        .enumerate()
        .map(|(i, r)| {
            (
                i as u64,
                hash_of((r.item, r.depth, &r.label, &r.detail, r.caret, r.selected, r.check)),
            )
        })
        .collect();
    let stats_for_build = stats.clone();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let r = &built[i];
            let (_, _, icon, _) = tree_row_parts(&stats_for_build, r.item);
            let row = list_row(
                c,
                f,
                RowSpec {
                    label: &r.label,
                    detail: &r.detail,
                    icon,
                    depth: r.depth,
                    caret: r.caret,
                    selected: r.selected,
                    check: Some(r.check),
                    dim: false,
                },
            );
            c.entity(row).insert(TreeRow(r.item));
            row
        }),
    }
}

/// One built scene-tree row, ready to hash and to spawn.
struct TreeRowData {
    item: TreeItem,
    depth: usize,
    label: String,
    detail: String,
    caret: Option<bool>,
    selected: bool,
    check: RowCheck,
}

/// A node and everything under it, guarded against a cycle in a malformed file.
pub(super) fn subtree_of(stats: &renzora_import::GlbStats, root: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if !seen.insert(n) {
            continue;
        }
        out.push(n);
        if let Some(node) = stats.node_list.get(n) {
            stack.extend(node.children.iter().copied());
        }
    }
    out
}
