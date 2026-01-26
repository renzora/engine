//! Workspace layout management system (DaVinci Resolve style)

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[cfg(feature = "editor")]
use egui_dock::{DockState, NodeIndex};

/// Represents different panel types that can be docked
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PanelTab {
    // Core panels
    Hierarchy,
    Inspector,
    Viewport,
    Assets,
    Console,
    ScriptEditor,
    History,

    // Animation workspace
    Animation,
    Dopesheet,

    // Scripting workspace
    FileBrowser,
    CodeEditor,
    Output,

    // Shaders workspace
    NodeGraph,
    Properties,
    Preview,

    // Blueprints workspace
    BlueprintEditor,
    Variables,

    // Video workspace
    MediaBrowser,
    Timeline,
    Effects,
}

impl PanelTab {
    pub fn title(&self) -> &'static str {
        match self {
            PanelTab::Hierarchy => "Hierarchy",
            PanelTab::Inspector => "Inspector",
            PanelTab::Viewport => "Viewport",
            PanelTab::Assets => "Assets",
            PanelTab::Console => "Console",
            PanelTab::ScriptEditor => "Script Editor",
            PanelTab::History => "History",
            PanelTab::Animation => "Animation",
            PanelTab::Dopesheet => "Dopesheet",
            PanelTab::FileBrowser => "File Browser",
            PanelTab::CodeEditor => "Code Editor",
            PanelTab::Output => "Output",
            PanelTab::NodeGraph => "Node Graph",
            PanelTab::Properties => "Properties",
            PanelTab::Preview => "Preview",
            PanelTab::BlueprintEditor => "Blueprint Editor",
            PanelTab::Variables => "Variables",
            PanelTab::MediaBrowser => "Media Browser",
            PanelTab::Timeline => "Timeline",
            PanelTab::Effects => "Effects",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            PanelTab::Hierarchy => "\u{e0c8}",      // tree structure
            PanelTab::Inspector => "\u{e1a8}",     // sliders
            PanelTab::Viewport => "\u{e0d0}",      // cube
            PanelTab::Assets => "\u{e1ca}",        // folder
            PanelTab::Console => "\u{e274}",       // terminal
            PanelTab::ScriptEditor => "\u{e0d4}",  // code
            PanelTab::History => "\u{e162}",       // clock counter clockwise
            PanelTab::Animation => "\u{e242}",     // play
            PanelTab::Dopesheet => "\u{e224}",     // dots
            PanelTab::FileBrowser => "\u{e1ca}",   // folder
            PanelTab::CodeEditor => "\u{e0d4}",    // code
            PanelTab::Output => "\u{e274}",        // terminal
            PanelTab::NodeGraph => "\u{e0d8}",     // flow arrow
            PanelTab::Properties => "\u{e1a8}",    // sliders
            PanelTab::Preview => "\u{e0dc}",       // eye
            PanelTab::BlueprintEditor => "\u{e0d8}", // flow arrow
            PanelTab::Variables => "\u{e0d4}",     // code
            PanelTab::MediaBrowser => "\u{e0e0}",  // film strip
            PanelTab::Timeline => "\u{e0e4}",      // timer
            PanelTab::Effects => "\u{e0e8}",       // magic wand
        }
    }

    /// Returns true if this is a placeholder panel (not yet implemented)
    pub fn is_placeholder(&self) -> bool {
        matches!(
            self,
            PanelTab::Dopesheet
                | PanelTab::FileBrowser
                | PanelTab::CodeEditor
                | PanelTab::Output
                | PanelTab::NodeGraph
                | PanelTab::Properties
                | PanelTab::Preview
                | PanelTab::BlueprintEditor
                | PanelTab::Variables
                | PanelTab::MediaBrowser
                | PanelTab::Timeline
                | PanelTab::Effects
        )
    }
}

/// A saved workspace layout
#[derive(Clone, Serialize, Deserialize)]
pub struct WorkspaceLayout {
    /// Display name of this layout
    pub name: String,
    /// Serialized dock state (simplified representation)
    pub dock_data: SerializedDockState,
    /// Whether this is a built-in layout that cannot be deleted
    pub is_builtin: bool,
}

/// Simplified serializable representation of dock state
#[derive(Clone, Serialize, Deserialize, Default)]
pub struct SerializedDockState {
    /// Layout tree structure
    pub tree: SerializedDockTree,
}

/// Tree node for dock layout serialization
#[derive(Clone, Serialize, Deserialize)]
pub enum SerializedDockTree {
    /// Empty node
    Empty,
    /// Leaf node with tabs
    Leaf { tabs: Vec<PanelTab>, active: usize },
    /// Horizontal split
    Horizontal { fraction: f32, children: Box<[SerializedDockTree; 2]> },
    /// Vertical split
    Vertical { fraction: f32, children: Box<[SerializedDockTree; 2]> },
}

impl Default for SerializedDockTree {
    fn default() -> Self {
        SerializedDockTree::Empty
    }
}

/// Manages workspace layouts
#[derive(Resource)]
pub struct WorkspaceManager {
    /// Name of the currently active layout
    pub current_layout: String,
    /// All available layouts (built-in and custom)
    pub layouts: HashMap<String, WorkspaceLayout>,
    /// The actual dock state being used
    pub dock_state: DockState<PanelTab>,
    /// Flag to indicate layout needs to be applied
    pub pending_layout_change: Option<String>,
    /// Flag to show "Save Layout As" dialog
    pub show_save_dialog: bool,
    /// Buffer for new layout name in save dialog
    pub save_name_buffer: String,
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        let mut layouts = HashMap::new();

        // Create all preset layouts
        layouts.insert("Scene".to_string(), create_scene_layout());
        layouts.insert("Animation".to_string(), create_animation_layout());
        layouts.insert("Scripting".to_string(), create_scripting_layout());
        layouts.insert("Shaders".to_string(), create_shaders_layout());
        layouts.insert("Blueprints".to_string(), create_blueprints_layout());
        layouts.insert("Video".to_string(), create_video_layout());

        // Start with Scene layout as default
        let dock_state = create_scene_dock_state();

        Self {
            current_layout: "Scene".to_string(),
            layouts,
            dock_state,
            pending_layout_change: None,
            show_save_dialog: false,
            save_name_buffer: String::new(),
        }
    }
}

impl WorkspaceManager {
    /// Switch to a different layout by name
    pub fn switch_layout(&mut self, name: &str) {
        if self.layouts.contains_key(name) {
            self.pending_layout_change = Some(name.to_string());
        }
    }

    /// Apply pending layout change. Returns true if a layout was changed.
    pub fn apply_pending_layout(&mut self) -> bool {
        if let Some(name) = self.pending_layout_change.take() {
            if let Some(layout) = self.layouts.get(&name) {
                self.dock_state = deserialize_dock_state(&layout.dock_data);
                self.current_layout = name;
                return true;
            }
        }
        false
    }

    /// Get the preferred bottom panel tab for the current layout
    pub fn preferred_bottom_tab(&self) -> &'static str {
        match self.current_layout.as_str() {
            "Animation" => "animation",
            "Scripting" => "console",
            _ => "assets",
        }
    }

    /// Save the current dock state as a new custom layout
    pub fn save_current_as(&mut self, name: String) {
        let dock_data = serialize_dock_state(&self.dock_state);
        let layout = WorkspaceLayout {
            name: name.clone(),
            dock_data,
            is_builtin: false,
        };
        self.layouts.insert(name.clone(), layout);
        self.current_layout = name;
    }

    /// Delete a custom layout (fails silently for built-in layouts)
    pub fn delete_layout(&mut self, name: &str) {
        if let Some(layout) = self.layouts.get(name) {
            if !layout.is_builtin {
                self.layouts.remove(name);
                // If we deleted the current layout, switch to Scene
                if self.current_layout == name {
                    self.switch_layout("Scene");
                }
            }
        }
    }

    /// Reset the current layout to its default state
    pub fn reset_current_layout(&mut self) {
        let dock_state = match self.current_layout.as_str() {
            "Scene" => create_scene_dock_state(),
            "Animation" => create_animation_dock_state(),
            "Scripting" => create_scripting_dock_state(),
            "Shaders" => create_shaders_dock_state(),
            "Blueprints" => create_blueprints_dock_state(),
            "Video" => create_video_dock_state(),
            _ => create_scene_dock_state(),
        };
        self.dock_state = dock_state;
    }

    /// Get list of layout names in display order
    pub fn layout_names(&self) -> Vec<&str> {
        // Built-in layouts first, then custom
        let mut builtin: Vec<_> = self.layouts.iter()
            .filter(|(_, l)| l.is_builtin)
            .map(|(n, _)| n.as_str())
            .collect();
        let mut custom: Vec<_> = self.layouts.iter()
            .filter(|(_, l)| !l.is_builtin)
            .map(|(n, _)| n.as_str())
            .collect();

        // Sort for consistent ordering
        builtin.sort();
        custom.sort();

        builtin.extend(custom);
        builtin
    }

    /// Check if a layout is built-in
    pub fn is_builtin(&self, name: &str) -> bool {
        self.layouts.get(name).map_or(false, |l| l.is_builtin)
    }
}

// ============================================================================
// Preset Layout Definitions
// ============================================================================

fn create_scene_layout() -> WorkspaceLayout {
    WorkspaceLayout {
        name: "Scene".to_string(),
        dock_data: serialize_dock_state(&create_scene_dock_state()),
        is_builtin: true,
    }
}

fn create_animation_layout() -> WorkspaceLayout {
    WorkspaceLayout {
        name: "Animation".to_string(),
        dock_data: serialize_dock_state(&create_animation_dock_state()),
        is_builtin: true,
    }
}

fn create_scripting_layout() -> WorkspaceLayout {
    WorkspaceLayout {
        name: "Scripting".to_string(),
        dock_data: serialize_dock_state(&create_scripting_dock_state()),
        is_builtin: true,
    }
}

fn create_shaders_layout() -> WorkspaceLayout {
    WorkspaceLayout {
        name: "Shaders".to_string(),
        dock_data: serialize_dock_state(&create_shaders_dock_state()),
        is_builtin: true,
    }
}

fn create_blueprints_layout() -> WorkspaceLayout {
    WorkspaceLayout {
        name: "Blueprints".to_string(),
        dock_data: serialize_dock_state(&create_blueprints_dock_state()),
        is_builtin: true,
    }
}

fn create_video_layout() -> WorkspaceLayout {
    WorkspaceLayout {
        name: "Video".to_string(),
        dock_data: serialize_dock_state(&create_video_dock_state()),
        is_builtin: true,
    }
}

// ============================================================================
// Dock State Creation Functions
// ============================================================================

/// Scene layout: Hierarchy | Viewport | Inspector + Assets/Console
fn create_scene_dock_state() -> DockState<PanelTab> {
    let mut state = DockState::new(vec![PanelTab::Viewport]);
    let surface = state.main_surface_mut();

    // Split left for Hierarchy
    let [_hierarchy, center_right] = surface.split_left(
        NodeIndex::root(),
        0.18,
        vec![PanelTab::Hierarchy],
    );

    // Split right for Inspector + History tabs
    let [center, _inspector] = surface.split_right(
        center_right,
        0.75,
        vec![PanelTab::Inspector, PanelTab::History],
    );

    // Split bottom for Assets + Console tabs
    let [_viewport, _assets] = surface.split_below(
        center,
        0.7,
        vec![PanelTab::Assets, PanelTab::Console],
    );

    state
}

/// Animation layout: Hierarchy | Viewport + Animation/Dopesheet | Inspector
fn create_animation_dock_state() -> DockState<PanelTab> {
    let mut state = DockState::new(vec![PanelTab::Viewport]);
    let surface = state.main_surface_mut();

    // Split left for Hierarchy
    let [_hierarchy, center_right] = surface.split_left(
        NodeIndex::root(),
        0.15,
        vec![PanelTab::Hierarchy],
    );

    // Split right for Inspector
    let [center, _inspector] = surface.split_right(
        center_right,
        0.78,
        vec![PanelTab::Inspector],
    );

    // Split bottom for Animation + Dopesheet tabs
    let [_viewport, _animation] = surface.split_below(
        center,
        0.6,
        vec![PanelTab::Animation, PanelTab::Dopesheet],
    );

    state
}

/// Scripting layout: File Browser | Code Editor | Console + Output
fn create_scripting_dock_state() -> DockState<PanelTab> {
    let mut state = DockState::new(vec![PanelTab::CodeEditor]);
    let surface = state.main_surface_mut();

    // Split left for File Browser
    let [_files, center_right] = surface.split_left(
        NodeIndex::root(),
        0.18,
        vec![PanelTab::FileBrowser],
    );

    // Split bottom for Console + Output
    let [_editor, _console] = surface.split_below(
        center_right,
        0.7,
        vec![PanelTab::Console, PanelTab::Output],
    );

    state
}

/// Shaders layout: Hierarchy | Node Graph (placeholder) | Properties + Preview
fn create_shaders_dock_state() -> DockState<PanelTab> {
    let mut state = DockState::new(vec![PanelTab::NodeGraph]);
    let surface = state.main_surface_mut();

    // Split left for Hierarchy
    let [_hierarchy, center_right] = surface.split_left(
        NodeIndex::root(),
        0.18,
        vec![PanelTab::Hierarchy],
    );

    // Split right for Properties + Preview
    let [_center, _right] = surface.split_right(
        center_right,
        0.72,
        vec![PanelTab::Properties, PanelTab::Preview],
    );

    state
}

/// Blueprints layout: Hierarchy | Node Editor (placeholder) | Inspector + Variables
fn create_blueprints_dock_state() -> DockState<PanelTab> {
    let mut state = DockState::new(vec![PanelTab::BlueprintEditor]);
    let surface = state.main_surface_mut();

    // Split left for Hierarchy
    let [_hierarchy, center_right] = surface.split_left(
        NodeIndex::root(),
        0.18,
        vec![PanelTab::Hierarchy],
    );

    // Split right for Inspector + Variables
    let [_center, _right] = surface.split_right(
        center_right,
        0.72,
        vec![PanelTab::Inspector, PanelTab::Variables],
    );

    state
}

/// Video layout: Media Browser | Timeline (placeholder) | Preview + Effects
fn create_video_dock_state() -> DockState<PanelTab> {
    let mut state = DockState::new(vec![PanelTab::Preview]);
    let surface = state.main_surface_mut();

    // Split left for Media Browser
    let [_media, center_right] = surface.split_left(
        NodeIndex::root(),
        0.20,
        vec![PanelTab::MediaBrowser],
    );

    // Split right for Effects
    let [center, _effects] = surface.split_right(
        center_right,
        0.75,
        vec![PanelTab::Effects],
    );

    // Split bottom for Timeline
    let [_preview, _timeline] = surface.split_below(
        center,
        0.5,
        vec![PanelTab::Timeline],
    );

    state
}

// ============================================================================
// Serialization Functions
// ============================================================================

/// Serialize dock state by just recording the layout name
/// Full dock state serialization is complex; for now we rebuild from presets
fn serialize_dock_state(state: &DockState<PanelTab>) -> SerializedDockState {
    // Collect all tabs present in the dock state
    let mut all_tabs = Vec::new();
    for (_surface_idx, node) in state.iter_all_nodes() {
        if let Some(tabs) = node.tabs() {
            all_tabs.extend(tabs.iter().cloned());
        }
    }

    SerializedDockState {
        tree: SerializedDockTree::Leaf {
            tabs: all_tabs,
            active: 0,
        },
    }
}

/// Deserialize dock state - for now just creates a basic layout with the tabs
fn deserialize_dock_state(data: &SerializedDockState) -> DockState<PanelTab> {
    match &data.tree {
        SerializedDockTree::Empty => create_scene_dock_state(),
        SerializedDockTree::Leaf { tabs, .. } => {
            if tabs.is_empty() {
                create_scene_dock_state()
            } else {
                // Recreate the Scene layout structure with the saved tabs
                // For custom layouts, we fall back to Scene layout
                create_scene_dock_state()
            }
        }
        _ => create_scene_dock_state(),
    }
}
