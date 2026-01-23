//! Editor API exposed to plugins.
//!
//! This module defines the interface that plugins use to interact with the editor.
//!
//! # Icons
//!
//! Plugins can use icons from `egui_phosphor` which is re-exported:
//! ```rust,ignore
//! use editor_plugin_api::egui_phosphor::regular::*;
//!
//! StatusBarItem::new("cpu", "CPU 45%").icon(CPU)
//! ```

use crate::abi::{AssetHandle, AssetStatus, EntityId, PluginTransform};
use crate::events::{EditorEventType, UiEvent};
use crate::ui::{UiId, Widget};

/// Location for menu items
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MenuLocation {
    File,
    Edit,
    View,
    Scene,
    Tools,
    Help,
    Custom(String),
}

/// Menu item definition
#[derive(Clone, Debug)]
pub struct MenuItem {
    pub id: UiId,
    pub label: String,
    pub shortcut: Option<String>,
    /// Icon string (use egui_phosphor constants like `egui_phosphor::regular::GEAR`)
    pub icon: Option<String>,
    pub enabled: bool,
    pub children: Vec<MenuItem>,
}

impl MenuItem {
    pub fn new(label: impl Into<String>, id: UiId) -> Self {
        Self {
            id,
            label: label.into(),
            shortcut: None,
            icon: None,
            enabled: true,
            children: Vec::new(),
        }
    }

    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Set icon (use egui_phosphor constants)
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn submenu(mut self, children: Vec<MenuItem>) -> Self {
        self.children = children;
        self
    }
}

/// Panel definition for dockable windows
#[derive(Clone, Debug)]
pub struct PanelDefinition {
    pub id: String,
    pub title: String,
    /// Icon string (use egui_phosphor constants)
    pub icon: Option<String>,
    pub default_location: PanelLocation,
    pub min_size: [f32; 2],
    pub closable: bool,
}

impl PanelDefinition {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            icon: None,
            default_location: PanelLocation::Right,
            min_size: [200.0, 100.0],
            closable: true,
        }
    }

    /// Set icon (use egui_phosphor constants)
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn location(mut self, location: PanelLocation) -> Self {
        self.default_location = location;
        self
    }

    pub fn min_size(mut self, width: f32, height: f32) -> Self {
        self.min_size = [width, height];
        self
    }
}

/// Panel location in the editor
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PanelLocation {
    Left,
    #[default]
    Right,
    Bottom,
    Floating,
    Center,
}

/// Inspector section definition
#[derive(Clone, Debug)]
pub struct InspectorDefinition {
    pub type_id: String,
    pub label: String,
    pub priority: i32,
}

/// Toolbar item definition
#[derive(Clone, Debug)]
pub struct ToolbarItem {
    pub id: UiId,
    /// Icon string (use egui_phosphor constants)
    pub icon: String,
    pub tooltip: String,
    pub group: Option<String>,
}

impl ToolbarItem {
    /// Create a toolbar item with an icon
    pub fn new(id: UiId, icon: impl Into<String>, tooltip: impl Into<String>) -> Self {
        Self {
            id,
            icon: icon.into(),
            tooltip: tooltip.into(),
            group: None,
        }
    }

    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }
}

/// Status bar item alignment
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StatusBarAlign {
    #[default]
    Left,
    Right,
}

/// Status bar item definition
#[derive(Clone, Debug)]
pub struct StatusBarItem {
    /// Unique identifier for this status item
    pub id: String,
    /// Icon string (use egui_phosphor constants, e.g. `egui_phosphor::regular::CPU`)
    pub icon: Option<String>,
    /// Display text
    pub text: String,
    /// Tooltip shown on hover
    pub tooltip: Option<String>,
    /// Alignment in the status bar
    pub align: StatusBarAlign,
    /// Priority for ordering (higher = further from edge)
    pub priority: i32,
}

impl StatusBarItem {
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            icon: None,
            text: text.into(),
            tooltip: None,
            align: StatusBarAlign::Left,
            priority: 0,
        }
    }

    /// Set the icon (use egui_phosphor constants)
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn align(mut self, align: StatusBarAlign) -> Self {
        self.align = align;
        self
    }

    pub fn align_right(mut self) -> Self {
        self.align = StatusBarAlign::Right;
        self
    }

    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }
}

/// Context menu location
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ContextMenuLocation {
    Hierarchy,
    Inspector,
    Viewport,
    Assets,
    SceneTab,
}

/// Settings value types
#[derive(Clone, Debug)]
pub enum SettingValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<SettingValue>),
}

/// Command for undo/redo system
#[derive(Clone, Debug)]
pub struct Command {
    pub name: String,
    pub data: Vec<u8>,
}

/// Custom event for plugin-to-plugin communication
#[derive(Clone, Debug)]
pub struct CustomEvent {
    pub target_plugin: Option<String>,
    pub event_type: String,
    pub data: Vec<u8>,
}

/// Entity query for filtering entities
#[derive(Clone, Debug, Default)]
pub struct EntityQuery {
    pub component_types: Vec<String>,
    pub name_filter: Option<String>,
}

/// Entity definition for spawning
#[derive(Clone, Debug)]
pub struct EntityDefinition {
    pub name: String,
    pub node_type: String,
    pub transform: PluginTransform,
    pub parent: Option<EntityId>,
}

impl EntityDefinition {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            node_type: "Empty".to_string(),
            transform: PluginTransform::default(),
            parent: None,
        }
    }

    pub fn node_type(mut self, node_type: impl Into<String>) -> Self {
        self.node_type = node_type.into();
        self
    }

    pub fn transform(mut self, transform: PluginTransform) -> Self {
        self.transform = transform;
        self
    }

    pub fn parent(mut self, parent: EntityId) -> Self {
        self.parent = Some(parent);
        self
    }
}

/// The API exposed to plugins for interacting with the editor.
///
/// This trait provides all the functionality that plugins can use to:
/// - Register UI elements (menus, panels, inspectors)
/// - Query and modify scene entities
/// - Load and access assets
/// - Subscribe to editor events
/// - Store persistent settings
pub trait EditorApi {
    // === Logging ===

    /// Log an info message
    fn log_info(&self, message: &str);

    /// Log a warning message
    fn log_warn(&self, message: &str);

    /// Log an error message
    fn log_error(&self, message: &str);

    // === UI Registration ===

    /// Register a menu item in the editor menu bar
    fn register_menu_item(&mut self, menu: MenuLocation, item: MenuItem);

    /// Register a panel (dockable window)
    fn register_panel(&mut self, panel: PanelDefinition);

    /// Register an inspector section for a component type
    fn register_inspector(&mut self, type_id: &str, inspector: InspectorDefinition);

    /// Register a toolbar button
    fn register_toolbar_item(&mut self, item: ToolbarItem);

    /// Register a context menu item
    fn register_context_menu(&mut self, context: ContextMenuLocation, item: MenuItem);

    // === Status Bar ===

    /// Set or update a status bar item
    /// If an item with this id already exists, it will be updated
    fn set_status_item(&mut self, item: StatusBarItem);

    /// Remove a status bar item
    fn remove_status_item(&mut self, id: &str);

    // === UI Content ===

    /// Set the content for a registered panel
    fn set_panel_content(&mut self, panel_id: &str, content: Vec<Widget>);

    /// Set the content for a registered inspector section
    /// The inspector_id should match the type_id used in register_inspector
    fn set_inspector_content(&mut self, inspector_id: &str, content: Vec<Widget>);

    // === UI Queries ===

    /// Get pending UI events for this plugin
    fn poll_ui_events(&mut self) -> Vec<UiEvent>;

    // === Scene Access ===

    /// Get currently selected entity
    fn get_selected_entity(&self) -> Option<EntityId>;

    /// Set the selected entity
    fn set_selected_entity(&mut self, entity: Option<EntityId>);

    /// Get entity transform
    fn get_transform(&self, entity: EntityId) -> Option<PluginTransform>;

    /// Set entity transform
    fn set_transform(&mut self, entity: EntityId, transform: &PluginTransform);

    /// Get entity name
    fn get_entity_name(&self, entity: EntityId) -> Option<String>;

    /// Set entity name
    fn set_entity_name(&mut self, entity: EntityId, name: &str);

    /// Spawn a new entity
    fn spawn_entity(&mut self, def: &EntityDefinition) -> EntityId;

    /// Despawn an entity
    fn despawn_entity(&mut self, entity: EntityId);

    /// Query entities by component
    fn query_entities(&self, query: &EntityQuery) -> Vec<EntityId>;

    // === Asset Access ===

    /// Load an asset
    fn load_asset(&mut self, path: &str) -> AssetHandle;

    /// Get asset loading status
    fn asset_status(&self, handle: AssetHandle) -> AssetStatus;

    // === Events ===

    /// Subscribe to editor events
    fn subscribe(&mut self, event_type: EditorEventType);

    /// Emit a custom event to other plugins
    fn emit_event(&mut self, event: CustomEvent);

    // === Settings ===

    /// Get a plugin setting (persistent storage)
    fn get_setting(&self, key: &str) -> Option<SettingValue>;

    /// Set a plugin setting
    fn set_setting(&mut self, key: &str, value: SettingValue);

    // === Commands ===

    /// Execute an undoable command
    fn execute_command(&mut self, command: Command);

    /// Undo the last command
    fn undo(&mut self);

    /// Redo the last undone command
    fn redo(&mut self);
}
