mod grid;
mod state;
mod toolbar;
mod tree;

use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui::{self, Stroke};
use egui_phosphor::regular;
use renzora_editor::{AppEditorExt, EditorCommands, EditorPanel, PanelLocation};
use renzora_theme::ThemeManager;

use state::AssetBrowserState;

/// Panel that provides the asset browser UI.
pub struct AssetBrowserPanel {
    state: RwLock<AssetBrowserState>,
}

impl Default for AssetBrowserPanel {
    fn default() -> Self {
        Self {
            state: RwLock::new(AssetBrowserState::default()),
        }
    }
}

impl EditorPanel for AssetBrowserPanel {
    fn id(&self) -> &str {
        "assets"
    }

    fn title(&self) -> &str {
        "Assets"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::FOLDER_OPEN)
    }

    fn closable(&self) -> bool {
        true
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Bottom
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };

        let mut state = self.state.write().unwrap();

        // Use project directory if available
        if let Some(project) = world.get_resource::<renzora_core::CurrentProject>() {
            if state.project_root.as_ref() != Some(&project.path) {
                state.project_root = Some(project.path.clone());
                state.current_folder = Some(project.path.clone());
            }
        }

        // Initialize current folder on first render
        if state.current_folder.is_none() {
            let root = state.root();
            state.current_folder = Some(root);
        }

        let available = ui.available_rect_before_wrap();

        // Toolbar
        let toolbar_height = 28.0;
        let toolbar_rect =
            egui::Rect::from_min_size(available.min, egui::vec2(available.width(), toolbar_height));
        let mut toolbar_ui = ui.new_child(egui::UiBuilder::new().max_rect(toolbar_rect));
        toolbar::toolbar_ui(&mut toolbar_ui, &mut state, &theme);

        // Separator line below toolbar
        let sep_y = available.min.y + toolbar_height;
        ui.painter().hline(
            available.min.x..=available.max.x,
            sep_y,
            Stroke::new(1.0, theme.widgets.border.to_color32()),
        );

        // Content area below toolbar
        let content_top = sep_y + 1.0;
        let content_rect = egui::Rect::from_min_max(
            egui::pos2(available.min.x, content_top),
            available.max,
        );

        if content_rect.height() < 10.0 {
            return;
        }

        // Split: tree (left) + grid (right) with draggable splitter
        let tree_width = state.tree_width.clamp(100.0, (content_rect.width() - 100.0).max(100.0));
        state.tree_width = tree_width;
        let splitter_width = 4.0;

        let tree_rect = egui::Rect::from_min_max(
            content_rect.min,
            egui::pos2(content_rect.min.x + tree_width, content_rect.max.y),
        );
        let splitter_rect = egui::Rect::from_min_max(
            egui::pos2(tree_rect.max.x, content_rect.min.y),
            egui::pos2(tree_rect.max.x + splitter_width, content_rect.max.y),
        );
        let grid_rect = egui::Rect::from_min_max(
            egui::pos2(splitter_rect.max.x, content_rect.min.y),
            content_rect.max,
        );

        // Draggable splitter
        let splitter_id = ui.id().with("asset_splitter");
        let splitter_resp =
            ui.interact(splitter_rect, splitter_id, egui::Sense::click_and_drag());
        if splitter_resp.dragged() {
            state.tree_width = (state.tree_width + splitter_resp.drag_delta().x)
                .clamp(100.0, (content_rect.width() - 100.0).max(100.0));
        }
        if splitter_resp.hovered() || splitter_resp.dragged() {
            ui.ctx()
                .set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }

        // Draw splitter line
        let splitter_color = if splitter_resp.hovered() || splitter_resp.dragged() {
            theme.semantic.accent.to_color32()
        } else {
            theme.widgets.border.to_color32()
        };
        ui.painter().vline(
            splitter_rect.center().x,
            splitter_rect.y_range(),
            Stroke::new(1.0, splitter_color),
        );

        // Tree pane
        let mut tree_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(tree_rect),
        );
        tree::tree_ui(&mut tree_ui, &mut state, &theme);

        // Grid pane
        let mut grid_child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(grid_rect),
        );
        if let Some(payload) = grid::grid_ui_interactive(&mut grid_child, &mut state, &theme) {
            if let Some(cmds) = world.get_resource::<EditorCommands>() {
                cmds.push(move |world: &mut bevy::prelude::World| {
                    world.insert_resource(payload);
                });
            }
        }
    }
}

/// Plugin that registers the `AssetBrowserPanel` with the editor.
pub struct AssetBrowserPlugin;

impl Plugin for AssetBrowserPlugin {
    fn build(&self, app: &mut App) {
        app.register_panel(AssetBrowserPanel::default());
    }
}
