//! Editor API exposed to plugins.
//!
//! This module defines the interface that plugins use to interact with the editor.

use super::abi::{AssetHandle, AssetStatus, EntityId, PluginTransform};
use super::traits::EditorEventType;
use crate::ui_api::{types::UiId, widgets::Widget, UiEvent};

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
    pub icon: String,
    pub tooltip: String,
    pub group: Option<String>,
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

    // === UI Content ===

    /// Set the content for a registered panel
    fn set_panel_content(&mut self, panel_id: &str, content: Vec<Widget>);

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

/// Default implementation for internal use
pub struct EditorApiImpl {
    // UI state
    pub menu_items: Vec<(MenuLocation, MenuItem)>,
    pub panels: Vec<PanelDefinition>,
    pub panel_contents: std::collections::HashMap<String, Vec<Widget>>,
    pub inspectors: Vec<(String, InspectorDefinition)>,
    pub toolbar_items: Vec<ToolbarItem>,
    pub context_menus: Vec<(ContextMenuLocation, MenuItem)>,

    // Events
    pub pending_events: Vec<UiEvent>,
    pub subscriptions: Vec<EditorEventType>,
    pub outgoing_events: Vec<CustomEvent>,

    // Settings
    pub settings: std::collections::HashMap<String, SettingValue>,
}

impl Default for EditorApiImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorApiImpl {
    pub fn new() -> Self {
        Self {
            menu_items: Vec::new(),
            panels: Vec::new(),
            panel_contents: std::collections::HashMap::new(),
            inspectors: Vec::new(),
            toolbar_items: Vec::new(),
            context_menus: Vec::new(),
            pending_events: Vec::new(),
            subscriptions: Vec::new(),
            outgoing_events: Vec::new(),
            settings: std::collections::HashMap::new(),
        }
    }
}

impl EditorApi for EditorApiImpl {
    fn log_info(&self, message: &str) {
        bevy::log::info!("[Plugin] {}", message);
    }

    fn log_warn(&self, message: &str) {
        bevy::log::warn!("[Plugin] {}", message);
    }

    fn log_error(&self, message: &str) {
        bevy::log::error!("[Plugin] {}", message);
    }

    fn register_menu_item(&mut self, menu: MenuLocation, item: MenuItem) {
        self.menu_items.push((menu, item));
    }

    fn register_panel(&mut self, panel: PanelDefinition) {
        self.panels.push(panel);
    }

    fn register_inspector(&mut self, type_id: &str, inspector: InspectorDefinition) {
        self.inspectors.push((type_id.to_string(), inspector));
    }

    fn register_toolbar_item(&mut self, item: ToolbarItem) {
        self.toolbar_items.push(item);
    }

    fn register_context_menu(&mut self, context: ContextMenuLocation, item: MenuItem) {
        self.context_menus.push((context, item));
    }

    fn set_panel_content(&mut self, panel_id: &str, content: Vec<Widget>) {
        self.panel_contents.insert(panel_id.to_string(), content);
    }

    fn poll_ui_events(&mut self) -> Vec<UiEvent> {
        std::mem::take(&mut self.pending_events)
    }

    fn get_selected_entity(&self) -> Option<EntityId> {
        // TODO: Connect to actual selection state
        None
    }

    fn set_selected_entity(&mut self, _entity: Option<EntityId>) {
        // TODO: Connect to actual selection state
    }

    fn get_transform(&self, _entity: EntityId) -> Option<PluginTransform> {
        // TODO: Connect to Bevy world
        None
    }

    fn set_transform(&mut self, _entity: EntityId, _transform: &PluginTransform) {
        // TODO: Connect to Bevy world
    }

    fn get_entity_name(&self, _entity: EntityId) -> Option<String> {
        // TODO: Connect to Bevy world
        None
    }

    fn set_entity_name(&mut self, _entity: EntityId, _name: &str) {
        // TODO: Connect to Bevy world
    }

    fn spawn_entity(&mut self, _def: &EntityDefinition) -> EntityId {
        // TODO: Connect to Bevy world
        EntityId::INVALID
    }

    fn despawn_entity(&mut self, _entity: EntityId) {
        // TODO: Connect to Bevy world
    }

    fn query_entities(&self, _query: &EntityQuery) -> Vec<EntityId> {
        // TODO: Connect to Bevy world
        Vec::new()
    }

    fn load_asset(&mut self, _path: &str) -> AssetHandle {
        // TODO: Connect to Bevy asset server
        AssetHandle::INVALID
    }

    fn asset_status(&self, handle: AssetHandle) -> AssetStatus {
        if handle.is_valid() {
            AssetStatus::Invalid
        } else {
            AssetStatus::Invalid
        }
    }

    fn subscribe(&mut self, event_type: EditorEventType) {
        if !self.subscriptions.contains(&event_type) {
            self.subscriptions.push(event_type);
        }
    }

    fn emit_event(&mut self, event: CustomEvent) {
        self.outgoing_events.push(event);
    }

    fn get_setting(&self, key: &str) -> Option<SettingValue> {
        self.settings.get(key).cloned()
    }

    fn set_setting(&mut self, key: &str, value: SettingValue) {
        self.settings.insert(key.to_string(), value);
    }

    fn execute_command(&mut self, _command: Command) {
        // TODO: Implement undo/redo system
    }

    fn undo(&mut self) {
        // TODO: Implement undo/redo system
    }

    fn redo(&mut self) {
        // TODO: Implement undo/redo system
    }
}
