//! Flatten the cached entity tree into visible rows and (re)build them when the
//! tree structure or the expand state changes.

use bevy::platform::collections::HashSet;
use bevy::prelude::*;

use renzora_ember::font::EmberFonts;

use crate::cache::HierarchyTreeCache;
use crate::state::EntityNode;

use super::components::HierarchyView;
use super::row::{build_row, RowData};
use super::HierExpanded;

/// Walk the tree depth-first, emitting a [`RowData`] per visible node (children
/// only when their parent is expanded). Tracks `parent_lines` for the connector
/// rendering, exactly like the egui recursion.
fn flatten<'a>(
    nodes: &'a [EntityNode],
    expanded: &HashSet<Entity>,
    parent_lines: &mut Vec<bool>,
    row_index: &mut usize,
    out: &mut Vec<RowData<'a>>,
) {
    let count = nodes.len();
    for (i, node) in nodes.iter().enumerate() {
        let is_last = i == count - 1;
        let has_children = !node.children.is_empty();
        let is_expanded = expanded.contains(&node.entity);
        out.push(RowData {
            node,
            depth: parent_lines.len(),
            is_last,
            parent_lines: parent_lines.clone(),
            row_index: *row_index,
            is_expanded,
        });
        *row_index += 1;
        if has_children && is_expanded {
            parent_lines.push(!is_last);
            flatten(&node.children, expanded, parent_lines, row_index, out);
            parent_lines.pop();
        }
    }
}

/// Order-independent hash of the expanded set (so reorders don't force rebuilds).
fn expanded_hash(set: &HashSet<Entity>) -> u64 {
    let mut h = set.len() as u64;
    for e in set {
        h ^= e.to_bits().wrapping_mul(0x9E3779B97F4A7C15);
    }
    h
}

/// Rebuild the row list when the cache version or the expanded set changes.
pub(crate) fn hierarchy_refresh(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    cache: Option<Res<HierarchyTreeCache>>,
    expanded: Res<HierExpanded>,
    mut views: Query<(Entity, &mut HierarchyView)>,
    children: Query<&Children>,
) {
    let (Some(fonts), Some(cache)) = (fonts, cache) else {
        return;
    };
    let ehash = expanded_hash(&expanded.0);

    for (list, mut view) in &mut views {
        if view.version == cache.version && view.expanded_hash == ehash {
            continue;
        }
        view.version = cache.version;
        view.expanded_hash = ehash;

        if let Ok(kids) = children.get(list) {
            for k in kids.iter() {
                commands.entity(k).despawn();
            }
        }
        let mut rows = Vec::new();
        let mut parent_lines = Vec::new();
        let mut row_index = 0;
        flatten(
            &cache.nodes,
            &expanded.0,
            &mut parent_lines,
            &mut row_index,
            &mut rows,
        );
        let entities: Vec<Entity> = rows
            .iter()
            .map(|r| build_row(&mut commands, &fonts, r))
            .collect();
        commands.entity(list).add_children(&entities);
    }
}
