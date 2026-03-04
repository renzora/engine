use std::collections::HashSet;

use bevy::prelude::Entity;
use bevy_egui::egui;
use renzora_editor::{tree_row, TreeRowConfig};
use renzora_theme::Theme;

use crate::state::EntityNode;

/// Render the entity tree recursively using `tree_row`.
pub fn render_tree(
    ui: &mut egui::Ui,
    nodes: &[EntityNode],
    expanded: &mut HashSet<Entity>,
    selected: &mut Option<Entity>,
    theme: &Theme,
) {
    let mut row_index = 0;
    let mut parent_lines = Vec::new();
    let count = nodes.len();

    for (i, node) in nodes.iter().enumerate() {
        let is_last = i == count - 1;
        render_node(
            ui,
            node,
            0,
            is_last,
            &mut parent_lines,
            &mut row_index,
            expanded,
            selected,
            theme,
        );
    }
}

fn render_node(
    ui: &mut egui::Ui,
    node: &EntityNode,
    depth: usize,
    is_last: bool,
    parent_lines: &mut Vec<bool>,
    row_index: &mut usize,
    expanded: &mut HashSet<Entity>,
    selected: &mut Option<Entity>,
    theme: &Theme,
) {
    let has_children = !node.children.is_empty();
    let is_expanded = expanded.contains(&node.entity);
    let is_selected = *selected == Some(node.entity);

    let result = tree_row(
        ui,
        &TreeRowConfig {
            depth,
            is_last,
            parent_lines,
            row_index: *row_index,
            has_children,
            is_expanded,
            is_selected,
            icon: Some(node.icon),
            icon_color: Some(node.icon_color),
            label: &node.name,
            label_color: None,
            theme,
        },
    );
    *row_index += 1;

    if result.expand_toggled {
        if expanded.contains(&node.entity) {
            expanded.remove(&node.entity);
        } else {
            expanded.insert(node.entity);
        }
    }

    if result.clicked {
        *selected = Some(node.entity);
    }

    if is_expanded {
        let child_count = node.children.len();
        for (i, child) in node.children.iter().enumerate() {
            let child_is_last = i == child_count - 1;
            parent_lines.push(!is_last);
            render_node(
                ui,
                child,
                depth + 1,
                child_is_last,
                parent_lines,
                row_index,
                expanded,
                selected,
                theme,
            );
            parent_lines.pop();
        }
    }
}
