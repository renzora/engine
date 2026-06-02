//! Flatten the cached entity tree into visible rows and (re)build them when the
//! tree structure or the expand state changes.

use bevy::platform::collections::HashSet;
use bevy::prelude::*;

use renzora_editor::EditorSelection;
use renzora_ember::font::EmberFonts;

use crate::cache::HierarchyTreeCache;
use crate::state::EntityNode;

use super::components::HierarchyView;
use super::row::{build_row, RowData};
use super::HierExpanded;

/// Walk the tree depth-first, emitting a [`RowData`] per visible node (children
/// only when their parent is expanded). Tracks `parent_lines` for the connector
/// rendering, exactly like the egui recursion.
#[allow(clippy::too_many_arguments)]
fn flatten<'a>(
    nodes: &'a [EntityNode],
    expanded: &HashSet<Entity>,
    selection: &EditorSelection,
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
            is_selected: selection.is_selected(node.entity),
        });
        *row_index += 1;
        if has_children && is_expanded {
            parent_lines.push(!is_last);
            flatten(&node.children, expanded, selection, parent_lines, row_index, out);
            parent_lines.pop();
        }
    }
}


/// Hash the visible content of the flattened rows — everything that affects how
/// the tree *looks* except selection/hover (those are applied in place). Used to
/// skip rebuilds when the cache is dirtied but the visible content is unchanged.
fn content_hash(rows: &[RowData]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for r in rows {
        r.node.entity.to_bits().hash(&mut h);
        r.depth.hash(&mut h);
        r.is_last.hash(&mut h);
        r.is_expanded.hash(&mut h);
        r.node.name.hash(&mut h);
        r.node.icon.hash(&mut h);
        r.node.is_visible.hash(&mut h);
        r.node.is_locked.hash(&mut h);
        r.node.is_default_camera.hash(&mut h);
        r.node.label_color.hash(&mut h);
    }
    h.finish()
}

/// Rebuild the row list when the cache version or the expanded set changes.
pub(crate) fn hierarchy_refresh(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    cache: Option<Res<HierarchyTreeCache>>,
    expanded: Res<HierExpanded>,
    selection: Option<Res<EditorSelection>>,
    mut views: Query<(Entity, &mut HierarchyView)>,
    children: Query<&Children>,
) {
    let (Some(fonts), Some(cache), Some(selection)) = (fonts, cache, selection) else {
        return;
    };

    // Flatten the visible tree every frame (cheap) and hash its content. Selection
    // is read only to bake into freshly-built rows; it is NOT part of the hash, so
    // selecting never rebuilds — `hierarchy_row_visual` highlights in place.
    let mut rows = Vec::new();
    let mut parent_lines = Vec::new();
    let mut row_index = 0;
    flatten(
        &cache.nodes,
        &expanded.0,
        &selection,
        &mut parent_lines,
        &mut row_index,
        &mut rows,
    );
    let hash = content_hash(&rows);

    for (list, mut view) in &mut views {
        if view.content_hash == hash {
            continue;
        }
        view.content_hash = hash;

        if let Ok(kids) = children.get(list) {
            for k in kids.iter() {
                commands.entity(k).despawn();
            }
        }
        let entities: Vec<Entity> = rows
            .iter()
            .map(|r| build_row(&mut commands, &fonts, r))
            .collect();
        commands.entity(list).add_children(&entities);
    }
}
