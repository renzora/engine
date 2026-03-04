use std::path::PathBuf;

use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_editor::{tree_row, TreeRowConfig};
use renzora_theme::Theme;

use crate::state::{folder_icon_color, is_hidden, AssetBrowserState};

/// Renders the folder tree in the left pane.
pub fn tree_ui(ui: &mut egui::Ui, state: &mut AssetBrowserState, theme: &Theme) {
    let root = state.root();
    let root_name = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Project")
        .to_string();

    egui::ScrollArea::vertical()
        .id_salt("asset_tree")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut row_index = 0;
            let mut parent_lines = Vec::new();

            // Root node
            let is_root_expanded = state.expanded_folders.contains(&root);
            let result = tree_row(
                ui,
                &TreeRowConfig {
                    depth: 0,
                    is_last: true,
                    parent_lines: &parent_lines,
                    row_index,
                    has_children: true,
                    is_expanded: is_root_expanded,
                    is_selected: state.current_folder.as_ref() == Some(&root),
                    icon: Some(if is_root_expanded {
                        regular::FOLDER_OPEN
                    } else {
                        regular::HOUSE
                    }),
                    icon_color: Some(folder_icon_color(&root_name)),
                    label: &root_name,
                    label_color: None,
                    theme,
                },
            );
            row_index += 1;

            if result.expand_toggled {
                toggle_expanded(&mut state.expanded_folders, &root);
            }
            if result.clicked {
                state.navigate_to(root.clone());
            }

            if is_root_expanded {
                render_folder_children(
                    ui,
                    state,
                    &root.clone(),
                    1,
                    &mut parent_lines,
                    &mut row_index,
                    theme,
                );
            }
        });
}

fn render_folder_children(
    ui: &mut egui::Ui,
    state: &mut AssetBrowserState,
    parent: &PathBuf,
    depth: usize,
    parent_lines: &mut Vec<bool>,
    row_index: &mut usize,
    theme: &Theme,
) {
    let mut folders: Vec<PathBuf> = match std::fs::read_dir(parent) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
            .filter(|e| !is_hidden(&e.path()))
            .map(|e| e.path())
            .collect(),
        Err(_) => return,
    };
    folders.sort_by(|a, b| {
        a.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase()
            .cmp(
                &b.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_lowercase(),
            )
    });

    let count = folders.len();
    for (i, folder) in folders.iter().enumerate() {
        let is_last = i == count - 1;
        let name = folder
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("???")
            .to_string();

        let has_children = has_subdirectories(folder);
        let is_expanded = state.expanded_folders.contains(folder);
        let is_selected = state.current_folder.as_ref() == Some(folder);

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
                icon: Some(if is_expanded {
                    regular::FOLDER_OPEN
                } else {
                    regular::FOLDER
                }),
                icon_color: Some(folder_icon_color(&name)),
                label: &name,
                label_color: None,
                theme,
            },
        );
        *row_index += 1;

        if result.expand_toggled {
            toggle_expanded(&mut state.expanded_folders, folder);
        }
        if result.clicked {
            state.navigate_to(folder.clone());
        }

        if is_expanded {
            parent_lines.push(!is_last);
            render_folder_children(ui, state, folder, depth + 1, parent_lines, row_index, theme);
            parent_lines.pop();
        }
    }
}

fn has_subdirectories(path: &PathBuf) -> bool {
    std::fs::read_dir(path)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .any(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false) && !is_hidden(&e.path()))
        })
        .unwrap_or(false)
}

fn toggle_expanded(set: &mut std::collections::HashSet<PathBuf>, path: &PathBuf) {
    if set.contains(path) {
        set.remove(path);
    } else {
        set.insert(path.clone());
    }
}
