use bevy::prelude::*;
use bevy_egui::egui::{self, TextureId};
use egui_dock::{DockState, NodeIndex, Style};

use crate::core::{
    EditorEntity, KeyBindings, SceneTabId,
    SelectionState, HierarchyState, ViewportState, SceneManagerState, AssetBrowserState, OrbitCameraState,
};
use crate::node_system::NodeRegistry;
use crate::project::CurrentProject;
use crate::scripting::{ScriptRegistry, RhaiScriptEngine};

/// Represents different panel types that can be docked
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PanelTab {
    Hierarchy,
    Inspector,
    Viewport,
    Assets,
    ScriptEditor,
}

impl PanelTab {
    pub fn title(&self) -> &'static str {
        match self {
            PanelTab::Hierarchy => "Hierarchy",
            PanelTab::Inspector => "Inspector",
            PanelTab::Viewport => "Viewport",
            PanelTab::Assets => "Assets",
            PanelTab::ScriptEditor => "Script Editor",
        }
    }
}

/// Resource storing the dock layout state
#[derive(Resource)]
pub struct EditorDockState {
    pub state: DockState<PanelTab>,
}

impl Default for EditorDockState {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorDockState {
    pub fn new() -> Self {
        // Create default layout:
        // +-------------------+-------------------+
        // | Hierarchy         |     Viewport      | Inspector |
        // |                   |                   |           |
        // +-------------------+-------------------+-----------+
        // |                   Assets                          |
        // +---------------------------------------------------+

        let mut state = DockState::new(vec![PanelTab::Viewport]);

        // Get the root surface
        let surface = state.main_surface_mut();

        // Split the root to add left panel (Hierarchy)
        let [_hierarchy, center_right] = surface.split_left(
            NodeIndex::root(),
            0.18,
            vec![PanelTab::Hierarchy],
        );

        // Split center_right to add right panel (Inspector)
        let [center, _inspector] = surface.split_right(
            center_right,
            0.75,
            vec![PanelTab::Inspector],
        );

        // Split center to add bottom panel (Assets)
        let [_viewport, _assets] = surface.split_below(
            center,
            0.7,
            vec![PanelTab::Assets],
        );

        Self { state }
    }

    /// Add the script editor tab if not already present
    pub fn open_script_editor(&mut self) {
        // Check if script editor is already open
        for (_surface_idx, node) in self.state.iter_all_nodes() {
            if let Some(tabs) = node.tabs() {
                if tabs.iter().any(|t| matches!(t, PanelTab::ScriptEditor)) {
                    return; // Already open
                }
            }
        }

        // Find viewport node and add script editor as a tab there
        for (surface_idx, node_idx) in self.state.iter_all_nodes().map(|(s, n)| (s, n.node_index())).collect::<Vec<_>>() {
            if let Some(node) = self.state.get_surface_mut(surface_idx).and_then(|s| s.node_mut(node_idx)) {
                if let Some(tabs) = node.tabs_mut() {
                    if tabs.iter().any(|t| matches!(t, PanelTab::Viewport)) {
                        tabs.push(PanelTab::ScriptEditor);
                        return;
                    }
                }
            }
        }
    }
}

/// Context passed to the TabViewer for rendering panels
pub struct DockContext<'a> {
    pub selection: &'a mut SelectionState,
    pub hierarchy: &'a mut HierarchyState,
    pub viewport: &'a mut ViewportState,
    pub scene_state: &'a mut SceneManagerState,
    pub assets: &'a mut AssetBrowserState,
    pub orbit: &'a OrbitCameraState,
    pub keybindings: &'a mut KeyBindings,
    pub commands: &'a mut Commands<'a, 'a>,
    pub entities: &'a Query<'a, 'a, (Entity, &'static EditorEntity, Option<&'static ChildOf>, Option<&'static Children>, Option<&'static SceneTabId>)>,
    pub entities_for_inspector: &'a Query<'a, 'a, (Entity, &'static EditorEntity)>,
    pub inspector_queries: &'a mut super::panels::InspectorQueries<'a, 'a>,
    pub meshes: &'a mut Assets<Mesh>,
    pub materials: &'a mut Assets<StandardMaterial>,
    pub current_project: Option<&'a CurrentProject>,
    pub node_registry: &'a NodeRegistry,
    pub script_registry: &'a ScriptRegistry,
    pub rhai_engine: &'a RhaiScriptEngine,
    pub viewport_texture_id: Option<TextureId>,
    pub camera_preview_texture_id: Option<TextureId>,
}

/// TabViewer implementation for the editor dock
pub struct EditorTabViewer<'a> {
    pub ctx: DockContext<'a>,
}

impl<'a> TabViewer for EditorTabViewer<'a> {
    type Tab = PanelTab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.title().into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            PanelTab::Hierarchy => {
                self.render_hierarchy_content(ui);
            }
            PanelTab::Inspector => {
                self.render_inspector_content(ui);
            }
            PanelTab::Viewport => {
                self.render_viewport_content(ui);
            }
            PanelTab::Assets => {
                self.render_assets_content(ui);
            }
            PanelTab::ScriptEditor => {
                self.render_script_editor_content(ui);
            }
        }
    }

    fn closeable(&mut self, _tab: &mut Self::Tab) -> bool {
        // All tabs can be closed
        true
    }

    fn on_close(&mut self, tab: &mut Self::Tab) -> bool {
        // Script editor has special close behavior
        if matches!(tab, PanelTab::ScriptEditor) {
            self.ctx.scene_state.open_scripts.clear();
        }
        true
    }
}

impl<'a> EditorTabViewer<'a> {
    fn render_hierarchy_content(&mut self, ui: &mut egui::Ui) {
        let active_tab = self.ctx.scene_state.active_scene_tab;
        super::panels::hierarchy::render_hierarchy_content(
            ui,
            self.ctx.selection,
            self.ctx.hierarchy,
            self.ctx.entities,
            self.ctx.commands,
            self.ctx.meshes,
            self.ctx.materials,
            self.ctx.node_registry,
            active_tab,
        );
    }

    fn render_inspector_content(&mut self, ui: &mut egui::Ui) {
        super::panels::inspector::render_inspector_content(
            ui,
            self.ctx.selection,
            self.ctx.entities_for_inspector,
            self.ctx.inspector_queries,
            self.ctx.script_registry,
            self.ctx.rhai_engine,
            self.ctx.camera_preview_texture_id,
        );
    }

    fn render_viewport_content(&mut self, ui: &mut egui::Ui) {
        super::panels::viewport::render_viewport_content(
            ui,
            self.ctx.viewport,
            self.ctx.assets,
            self.ctx.orbit,
            self.ctx.viewport_texture_id,
        );
    }

    fn render_assets_content(&mut self, ui: &mut egui::Ui) {
        super::panels::assets::render_assets_content(
            ui,
            self.ctx.current_project,
            self.ctx.viewport,
            self.ctx.assets,
            self.ctx.scene_state,
        );
    }

    #[allow(dead_code)]
    fn render_script_editor_content(&mut self, _ui: &mut egui::Ui) {
        // TODO: Script editor content rendering in dock mode
        // Needs the render_script_editor_content function to be implemented
    }
}

/// Create a custom dock style matching the editor theme
pub fn create_dock_style(ctx: &egui::Context) -> Style {
    let mut style = Style::from_egui(ctx.style().as_ref());

    // Customize tab bar
    style.tab_bar.fill_tab_bar = true;
    style.tab_bar.show_scroll_arrows_when_many_tabs = true;

    // Tab styling
    style.tab.tab_body.inner_margin = egui::Margin::same(0);

    style
}
