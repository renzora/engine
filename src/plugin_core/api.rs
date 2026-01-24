//! Editor API exposed to plugins.
//!
//! Re-exports types from editor_plugin_api and provides the implementation.

// Re-export all types from the shared crate
pub use editor_plugin_api::api::*;
pub use editor_plugin_api::events::{EditorEventType, UiEvent};

use super::abi::{AssetHandle, AssetStatus, EntityId, PluginTransform};
use crate::ui_api::Widget;

/// Tab location for docked plugin tabs
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum TabLocation {
    Left,   // Alongside Hierarchy
    Right,  // Alongside Inspector
    Bottom, // Alongside Assets/Console
}

/// Plugin tab definition
#[derive(Clone, Debug)]
pub struct PluginTab {
    pub id: String,
    pub title: String,
    pub icon: Option<String>,
    pub location: TabLocation,
}

/// Pending operations that will be applied to Bevy world
#[derive(Clone, Debug)]
pub enum PendingOperation {
    SetSelectedEntity(Option<EntityId>),
    SetTransform { entity: EntityId, transform: PluginTransform },
    SetEntityName { entity: EntityId, name: String },
    SetEntityVisible { entity: EntityId, visible: bool },
    SpawnEntity(EntityDefinition),
    DespawnEntity(EntityId),
    ReparentEntity { entity: EntityId, new_parent: Option<EntityId> },
    LoadAsset(String),
}

/// Default implementation for internal use
pub struct EditorApiImpl {
    // Project path (for asset operations)
    pub project_assets_path: Option<std::path::PathBuf>,

    // UI registrations (persistent) - now track which plugin owns each
    pub menu_items: Vec<(MenuLocation, MenuItem, String)>,  // (location, item, plugin_id)
    pub panels: Vec<(PanelDefinition, String)>,  // (panel, plugin_id)
    pub panel_contents: std::collections::HashMap<String, Vec<Widget>>,
    pub panel_visible: std::collections::HashMap<String, bool>,
    pub inspectors: Vec<(String, InspectorDefinition, String)>,  // (type_id, inspector, plugin_id)
    pub inspector_contents: std::collections::HashMap<String, Vec<Widget>>,
    pub toolbar_items: Vec<(ToolbarItem, String)>,  // (item, plugin_id)
    pub context_menus: Vec<(ContextMenuLocation, MenuItem, String)>,  // (location, item, plugin_id)
    pub status_bar_items: std::collections::HashMap<String, (StatusBarItem, String)>,  // id -> (item, plugin_id)

    // Tabbed panels - plugin tabs docked alongside built-in panels
    pub tabs: Vec<(PluginTab, String)>,  // (tab, plugin_id)
    pub tab_contents: std::collections::HashMap<String, Vec<Widget>>,  // tab_id -> widgets
    pub active_tabs: std::collections::HashMap<TabLocation, String>,  // location -> active tab id

    // Currently active plugin (set during plugin callbacks)
    pub current_plugin_id: Option<String>,

    // State snapshot (synced from Bevy each frame)
    pub selected_entity: Option<EntityId>,
    pub entity_transforms: std::collections::HashMap<EntityId, PluginTransform>,
    pub entity_names: std::collections::HashMap<EntityId, String>,
    pub entity_visibility: std::collections::HashMap<EntityId, bool>,
    pub entity_parents: std::collections::HashMap<EntityId, Option<EntityId>>,
    pub entity_children: std::collections::HashMap<EntityId, Vec<EntityId>>,

    // Undo/redo state (synced from CommandHistory each frame)
    pub can_undo: bool,
    pub can_redo: bool,

    // Pending undo/redo requests (processed by exclusive system)
    pub pending_undo: bool,
    pub pending_redo: bool,

    // Pending operations (applied to Bevy after plugin update)
    pub pending_operations: Vec<PendingOperation>,

    // Events
    pub pending_ui_events: Vec<UiEvent>,
    pub subscriptions: Vec<EditorEventType>,
    pub outgoing_events: Vec<CustomEvent>,

    // Pub/Sub system - subscriptions per plugin
    pub plugin_subscriptions: std::collections::HashMap<String, Vec<String>>,
    // Pending published events (event_type, data_json, source_plugin_id)
    pub pending_published_events: Vec<(String, String, String)>,

    // Settings (persistent)
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
            project_assets_path: None,
            menu_items: Vec::new(),
            panels: Vec::new(),
            panel_contents: std::collections::HashMap::new(),
            panel_visible: std::collections::HashMap::new(),
            inspectors: Vec::new(),
            inspector_contents: std::collections::HashMap::new(),
            toolbar_items: Vec::new(),
            context_menus: Vec::new(),
            status_bar_items: std::collections::HashMap::new(),
            tabs: Vec::new(),
            tab_contents: std::collections::HashMap::new(),
            active_tabs: std::collections::HashMap::new(),
            current_plugin_id: None,
            selected_entity: None,
            entity_transforms: std::collections::HashMap::new(),
            entity_names: std::collections::HashMap::new(),
            entity_visibility: std::collections::HashMap::new(),
            entity_parents: std::collections::HashMap::new(),
            entity_children: std::collections::HashMap::new(),
            can_undo: false,
            can_redo: false,
            pending_undo: false,
            pending_redo: false,
            pending_operations: Vec::new(),
            pending_ui_events: Vec::new(),
            subscriptions: Vec::new(),
            outgoing_events: Vec::new(),
            plugin_subscriptions: std::collections::HashMap::new(),
            pending_published_events: Vec::new(),
            settings: std::collections::HashMap::new(),
        }
    }

    /// Set the current plugin ID (called before plugin callbacks)
    pub fn set_current_plugin(&mut self, plugin_id: Option<String>) {
        self.current_plugin_id = plugin_id;
    }

    /// Set the project assets path (called when project is opened)
    pub fn set_project_assets_path(&mut self, path: Option<std::path::PathBuf>) {
        self.project_assets_path = path;
    }

    /// Get the project assets path
    pub fn get_project_assets_path(&self) -> Option<&std::path::Path> {
        self.project_assets_path.as_deref()
    }

    /// Get the current plugin ID or a default
    fn current_plugin(&self) -> String {
        self.current_plugin_id.clone().unwrap_or_else(|| "unknown".to_string())
    }

    /// Take pending operations (called by sync system)
    pub fn take_pending_operations(&mut self) -> Vec<PendingOperation> {
        std::mem::take(&mut self.pending_operations)
    }

    /// Update state snapshot from Bevy
    pub fn sync_from_bevy(
        &mut self,
        selected: Option<EntityId>,
        transforms: std::collections::HashMap<EntityId, PluginTransform>,
        names: std::collections::HashMap<EntityId, String>,
        visibility: std::collections::HashMap<EntityId, bool>,
        parents: std::collections::HashMap<EntityId, Option<EntityId>>,
        children: std::collections::HashMap<EntityId, Vec<EntityId>>,
    ) {
        self.selected_entity = selected;
        self.entity_transforms = transforms;
        self.entity_names = names;
        self.entity_visibility = visibility;
        self.entity_parents = parents;
        self.entity_children = children;
    }

    /// Get entity by name (returns first match)
    pub fn get_entity_by_name(&self, name: &str) -> Option<EntityId> {
        self.entity_names.iter()
            .find(|(_, n)| *n == name)
            .map(|(id, _)| *id)
    }

    /// Get entity visibility
    pub fn get_entity_visible(&self, entity: EntityId) -> Option<bool> {
        self.entity_visibility.get(&entity).copied()
    }

    /// Get entity parent
    pub fn get_entity_parent(&self, entity: EntityId) -> Option<Option<EntityId>> {
        self.entity_parents.get(&entity).copied()
    }

    /// Get entity children
    pub fn get_entity_children(&self, entity: EntityId) -> Option<&Vec<EntityId>> {
        self.entity_children.get(&entity)
    }

    /// Push a UI event for plugins to receive
    pub fn push_ui_event(&mut self, event: UiEvent) {
        self.pending_ui_events.push(event);
    }

    /// Remove all UI elements registered by a specific plugin
    pub fn remove_plugin_elements(&mut self, plugin_id: &str) {
        self.menu_items.retain(|(_, _, id)| id != plugin_id);
        self.panels.retain(|(_, id)| id != plugin_id);
        self.inspectors.retain(|(_, _, id)| id != plugin_id);
        self.toolbar_items.retain(|(_, id)| id != plugin_id);
        self.context_menus.retain(|(_, _, id)| id != plugin_id);
        self.status_bar_items.retain(|_, (_, id)| id != plugin_id);

        // Remove panel contents for panels owned by this plugin
        let panel_ids: Vec<_> = self.panels.iter()
            .filter(|(_, id)| id == plugin_id)
            .map(|(p, _)| p.id.clone())
            .collect();
        for panel_id in panel_ids {
            self.panel_contents.remove(&panel_id);
        }

        // Remove tab contents for tabs owned by this plugin
        let tab_ids: Vec<_> = self.tabs.iter()
            .filter(|(_, id)| id == plugin_id)
            .map(|(t, _)| t.id.clone())
            .collect();
        for tab_id in tab_ids {
            self.tab_contents.remove(&tab_id);
        }
        self.tabs.retain(|(_, id)| id != plugin_id);
    }

    /// Clear all registered UI elements (called when unloading all plugins)
    pub fn clear(&mut self) {
        self.menu_items.clear();
        self.panels.clear();
        self.panel_contents.clear();
        self.panel_visible.clear();
        self.inspectors.clear();
        self.inspector_contents.clear();
        self.toolbar_items.clear();
        self.context_menus.clear();
        self.status_bar_items.clear();
        self.tabs.clear();
        self.tab_contents.clear();
        self.active_tabs.clear();
        self.current_plugin_id = None;
        self.can_undo = false;
        self.can_redo = false;
        self.pending_undo = false;
        self.pending_redo = false;
        self.pending_operations.clear();
        self.pending_ui_events.clear();
        self.subscriptions.clear();
        self.outgoing_events.clear();
        self.plugin_subscriptions.clear();
        self.pending_published_events.clear();
        // Keep settings - they persist across plugin reloads
    }

    /// Get tabs for a specific location
    pub fn get_tabs_for_location(&self, location: TabLocation) -> Vec<&PluginTab> {
        self.tabs.iter()
            .filter(|(tab, _)| tab.location == location)
            .map(|(tab, _)| tab)
            .collect()
    }

    /// Get tab content
    pub fn get_tab_content(&self, tab_id: &str) -> Option<&Vec<Widget>> {
        self.tab_contents.get(tab_id)
    }

    /// Get active tab for a location (returns None if built-in tab is active)
    pub fn get_active_tab(&self, location: TabLocation) -> Option<&str> {
        self.active_tabs.get(&location).map(|s| s.as_str())
    }

    /// Set active tab for a location
    pub fn set_active_tab(&mut self, location: TabLocation, tab_id: String) {
        self.active_tabs.insert(location, tab_id);
    }

    /// Clear active tab for a location (switch back to built-in)
    pub fn clear_active_tab(&mut self, location: TabLocation) {
        self.active_tabs.remove(&location);
    }

    // === Pub/Sub System ===

    /// Subscribe a plugin to an event type
    pub fn subscribe_plugin(&mut self, plugin_id: &str, event_type: &str) {
        let subs = self.plugin_subscriptions
            .entry(plugin_id.to_string())
            .or_insert_with(Vec::new);
        if !subs.contains(&event_type.to_string()) {
            subs.push(event_type.to_string());
        }
    }

    /// Unsubscribe a plugin from an event type
    pub fn unsubscribe_plugin(&mut self, plugin_id: &str, event_type: &str) {
        if let Some(subs) = self.plugin_subscriptions.get_mut(plugin_id) {
            subs.retain(|s| s != event_type);
        }
    }

    /// Check if a plugin is subscribed to an event type
    pub fn is_subscribed(&self, plugin_id: &str, event_type: &str) -> bool {
        if let Some(subs) = self.plugin_subscriptions.get(plugin_id) {
            // Check for exact match or wildcard match
            for sub in subs {
                if sub == event_type {
                    return true;
                }
                // Wildcard matching: "ui.*" matches "ui.button_clicked"
                if sub.ends_with(".*") {
                    let prefix = &sub[..sub.len() - 1]; // Remove "*"
                    if event_type.starts_with(prefix) {
                        return true;
                    }
                }
                // Match all: "*" matches everything
                if sub == "*" {
                    return true;
                }
            }
        }
        false
    }

    /// Publish an event from a plugin
    pub fn publish_event(&mut self, source_plugin: &str, event_type: &str, data_json: &str) {
        self.pending_published_events.push((
            event_type.to_string(),
            data_json.to_string(),
            source_plugin.to_string(),
        ));
    }

    /// Take pending published events
    pub fn take_published_events(&mut self) -> Vec<(String, String, String)> {
        std::mem::take(&mut self.pending_published_events)
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
        let plugin_id = self.current_plugin();
        self.menu_items.push((menu, item, plugin_id));
    }

    fn register_panel(&mut self, panel: PanelDefinition) {
        let plugin_id = self.current_plugin();
        self.panels.push((panel, plugin_id));
    }

    fn register_inspector(&mut self, type_id: &str, inspector: InspectorDefinition) {
        let plugin_id = self.current_plugin();
        self.inspectors.push((type_id.to_string(), inspector, plugin_id));
    }

    fn register_toolbar_item(&mut self, item: ToolbarItem) {
        let plugin_id = self.current_plugin();
        self.toolbar_items.push((item, plugin_id));
    }

    fn register_context_menu(&mut self, context: ContextMenuLocation, item: MenuItem) {
        let plugin_id = self.current_plugin();
        self.context_menus.push((context, item, plugin_id));
    }

    fn set_status_item(&mut self, item: StatusBarItem) {
        let plugin_id = self.current_plugin();
        self.status_bar_items.insert(item.id.clone(), (item, plugin_id));
    }

    fn remove_status_item(&mut self, id: &str) {
        self.status_bar_items.remove(id);
    }

    fn set_panel_content(&mut self, panel_id: &str, content: Vec<editor_plugin_api::ui::Widget>) {
        // Convert from plugin API Widget to internal Widget
        let internal_content: Vec<Widget> = content.into_iter().map(convert_widget).collect();
        self.panel_contents.insert(panel_id.to_string(), internal_content);
    }

    fn set_inspector_content(&mut self, inspector_id: &str, content: Vec<editor_plugin_api::ui::Widget>) {
        // Convert from plugin API Widget to internal Widget
        let internal_content: Vec<Widget> = content.into_iter().map(convert_widget).collect();
        self.inspector_contents.insert(inspector_id.to_string(), internal_content);
    }

    fn poll_ui_events(&mut self) -> Vec<UiEvent> {
        std::mem::take(&mut self.pending_ui_events)
    }

    fn get_selected_entity(&self) -> Option<EntityId> {
        self.selected_entity
    }

    fn set_selected_entity(&mut self, entity: Option<EntityId>) {
        self.pending_operations.push(PendingOperation::SetSelectedEntity(entity));
    }

    fn get_transform(&self, entity: EntityId) -> Option<PluginTransform> {
        self.entity_transforms.get(&entity).copied()
    }

    fn set_transform(&mut self, entity: EntityId, transform: &PluginTransform) {
        self.pending_operations.push(PendingOperation::SetTransform {
            entity,
            transform: *transform,
        });
    }

    fn get_entity_name(&self, entity: EntityId) -> Option<String> {
        self.entity_names.get(&entity).cloned()
    }

    fn set_entity_name(&mut self, entity: EntityId, name: &str) {
        self.pending_operations.push(PendingOperation::SetEntityName {
            entity,
            name: name.to_string(),
        });
    }

    fn spawn_entity(&mut self, def: &EntityDefinition) -> EntityId {
        // Queue the spawn operation - actual entity will be created by sync system
        self.pending_operations.push(PendingOperation::SpawnEntity(def.clone()));
        // Return invalid for now - in a real impl we'd use a placeholder ID
        EntityId::INVALID
    }

    fn despawn_entity(&mut self, entity: EntityId) {
        self.pending_operations.push(PendingOperation::DespawnEntity(entity));
    }

    fn query_entities(&self, query: &EntityQuery) -> Vec<EntityId> {
        // Filter entities based on query
        let mut results = Vec::new();
        for (id, name) in &self.entity_names {
            let matches = query.name_filter.as_ref()
                .map(|f| name.contains(f))
                .unwrap_or(true);
            if matches {
                results.push(*id);
            }
        }
        results
    }

    fn load_asset(&mut self, path: &str) -> AssetHandle {
        self.pending_operations.push(PendingOperation::LoadAsset(path.to_string()));
        // Return a placeholder handle
        AssetHandle::new(0)
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
        // TODO: Implement command execution via pending operations
    }

    fn undo(&mut self) {
        self.pending_undo = true;
    }

    fn redo(&mut self) {
        self.pending_redo = true;
    }
}

/// Convert plugin API Widget to internal Widget
fn convert_widget(w: editor_plugin_api::ui::Widget) -> Widget {
    use editor_plugin_api::ui::Widget as ApiWidget;

    match w {
        ApiWidget::Label { text, style } => Widget::Label {
            text,
            style: convert_text_style(style),
        },
        ApiWidget::Button { label, id, enabled } => Widget::Button {
            label,
            id: crate::ui_api::UiId(id.0),
            enabled,
        },
        ApiWidget::IconButton { icon, tooltip, id, enabled } => Widget::IconButton {
            icon,
            tooltip,
            id: crate::ui_api::UiId(id.0),
            enabled,
        },
        ApiWidget::TextInput { value, placeholder, id } => Widget::TextInput {
            value,
            placeholder,
            id: crate::ui_api::UiId(id.0),
        },
        ApiWidget::TextEdit { value, id, min_lines, max_lines } => Widget::TextEdit {
            value,
            id: crate::ui_api::UiId(id.0),
            min_lines,
            max_lines: Some(max_lines),
        },
        ApiWidget::Checkbox { checked, label, id } => Widget::Checkbox {
            checked,
            label,
            id: crate::ui_api::UiId(id.0),
        },
        ApiWidget::Slider { value, min, max, id, label } => Widget::Slider {
            value,
            min,
            max,
            id: crate::ui_api::UiId(id.0),
            label,
        },
        ApiWidget::SliderInt { value, min, max, id, label } => Widget::SliderInt {
            value,
            min,
            max,
            id: crate::ui_api::UiId(id.0),
            label,
        },
        ApiWidget::Dropdown { selected, options, id } => Widget::Dropdown {
            selected,
            options,
            id: crate::ui_api::UiId(id.0),
        },
        ApiWidget::ColorPicker { color, id, alpha } => Widget::ColorPicker {
            color,
            id: crate::ui_api::UiId(id.0),
            alpha,
        },
        ApiWidget::ProgressBar { progress, label } => Widget::ProgressBar {
            progress,
            label,
        },
        ApiWidget::Row { children, spacing, align } => Widget::Row {
            children: children.into_iter().map(convert_widget).collect(),
            spacing,
            align: convert_align(align),
        },
        ApiWidget::Column { children, spacing, align } => Widget::Column {
            children: children.into_iter().map(convert_widget).collect(),
            spacing,
            align: convert_align(align),
        },
        ApiWidget::Panel { title, children, collapsible, default_open } => Widget::Panel {
            title,
            children: children.into_iter().map(convert_widget).collect(),
            collapsible,
            default_open,
        },
        ApiWidget::ScrollArea { child, max_height, horizontal } => Widget::ScrollArea {
            child: Box::new(convert_widget(*child)),
            max_height,
            horizontal,
        },
        ApiWidget::Group { children, frame } => Widget::Group {
            children: children.into_iter().map(convert_widget).collect(),
            frame,
        },
        ApiWidget::TreeNode { label, id, children, expanded, leaf } => Widget::TreeNode {
            label,
            id: crate::ui_api::UiId(id.0),
            children: children.into_iter().map(convert_widget).collect(),
            expanded,
            leaf,
        },
        ApiWidget::Table { columns, rows, id, striped } => Widget::Table {
            columns: columns.into_iter().map(|c| crate::ui_api::TableColumn {
                header: c.header,
                width: convert_size(c.width),
                sortable: c.sortable,
                resizable: c.resizable,
            }).collect(),
            rows: rows.into_iter().map(|r| crate::ui_api::TableRow {
                cells: r.cells.into_iter().map(convert_widget).collect(),
                id: crate::ui_api::UiId(r.id.0),
            }).collect(),
            id: crate::ui_api::UiId(id.0),
            striped,
        },
        ApiWidget::Tabs { tabs, active, id } => Widget::Tabs {
            tabs: tabs.into_iter().map(|t| crate::ui_api::Tab {
                label: t.label,
                icon: t.icon,
                content: t.content.into_iter().map(convert_widget).collect(),
                closable: t.closable,
            }).collect(),
            active,
            id: crate::ui_api::UiId(id.0),
        },
        ApiWidget::Separator => Widget::Separator,
        ApiWidget::Spacer { size } => Widget::Spacer {
            size: convert_size(size),
        },
        ApiWidget::Image { path, size } => Widget::Image {
            path,
            size,
        },
        ApiWidget::Custom { type_id, data } => Widget::Custom {
            type_id,
            data,
        },
        ApiWidget::Empty => Widget::Empty,
    }
}

fn convert_text_style(s: editor_plugin_api::ui::TextStyle) -> crate::ui_api::TextStyle {
    match s {
        editor_plugin_api::ui::TextStyle::Body => crate::ui_api::TextStyle::Body,
        editor_plugin_api::ui::TextStyle::Heading1 => crate::ui_api::TextStyle::Heading1,
        editor_plugin_api::ui::TextStyle::Heading2 => crate::ui_api::TextStyle::Heading2,
        editor_plugin_api::ui::TextStyle::Heading3 => crate::ui_api::TextStyle::Heading3,
        editor_plugin_api::ui::TextStyle::Caption => crate::ui_api::TextStyle::Caption,
        editor_plugin_api::ui::TextStyle::Code => crate::ui_api::TextStyle::Code,
        editor_plugin_api::ui::TextStyle::Label => crate::ui_api::TextStyle::Label,
    }
}

fn convert_align(a: editor_plugin_api::ui::Align) -> crate::ui_api::Align {
    match a {
        editor_plugin_api::ui::Align::Start => crate::ui_api::Align::Start,
        editor_plugin_api::ui::Align::Center => crate::ui_api::Align::Center,
        editor_plugin_api::ui::Align::End => crate::ui_api::Align::End,
        editor_plugin_api::ui::Align::Stretch => crate::ui_api::Align::Stretch,
    }
}

fn convert_size(s: editor_plugin_api::ui::Size) -> crate::ui_api::Size {
    match s {
        editor_plugin_api::ui::Size::Auto => crate::ui_api::Size::Auto,
        editor_plugin_api::ui::Size::Fixed(v) => crate::ui_api::Size::Fixed(v),
        editor_plugin_api::ui::Size::Percent(v) => crate::ui_api::Size::Percent(v),
        editor_plugin_api::ui::Size::Fill => crate::ui_api::Size::Fill,
        editor_plugin_api::ui::Size::FillPortion(v) => crate::ui_api::Size::FillPortion(v),
    }
}
