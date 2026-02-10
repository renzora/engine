//! Plugin host for discovering, loading, and managing plugins.

#![allow(dead_code)]

use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, OsStr};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;

use bevy::prelude::*;
use libloading::Library;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

use super::abi::{PluginError, PluginManifest, EDITOR_API_VERSION, EntityId, PluginTransform};
use super::api::{EditorApiImpl, PendingOperation};
use super::dependency::DependencyGraph;
use super::traits::EditorEvent;
use editor_plugin_api::ffi::{PluginExport, PluginVTable, PluginHandle, FfiStatusBarItem, HostApi, FFI_API_VERSION, FfiEntityId, FfiTransform, FfiEntityList, FfiOwnedString, FfiPanelDefinition, FfiPanelLocation, FfiMenuItem, FfiTabDefinition, FfiTabLocation, FfiAssetList, FfiAssetInfo, FfiAssetType, FfiBytes};
use crate::plugin_core::{StatusBarAlign, StatusBarItem, ToolbarItem, MenuItem, MenuLocation, TabLocation, PluginTab};
use editor_plugin_api::ui::UiId;
use crate::core::resources::console::{console_log, LogLevel};

/// Type for the FFI create_plugin function
type CreatePluginFn = unsafe extern "C" fn() -> PluginExport;

// ============================================================================
// Host callback implementations - these are called by plugins via FFI
// ============================================================================

unsafe extern "C" fn host_log_info(ctx: *mut c_void, message: *const c_char) {
    if message.is_null() { return; }
    let msg = CStr::from_ptr(message).to_string_lossy();
    info!("[Plugin] {}", msg);
    let _ = ctx; // ctx is EditorApiImpl but we don't need it for logging
}

unsafe extern "C" fn host_log_warn(ctx: *mut c_void, message: *const c_char) {
    if message.is_null() { return; }
    let msg = CStr::from_ptr(message).to_string_lossy();
    warn!("[Plugin] {}", msg);
    let _ = ctx;
}

unsafe extern "C" fn host_log_error(ctx: *mut c_void, message: *const c_char) {
    if message.is_null() { return; }
    let msg = CStr::from_ptr(message).to_string_lossy();
    error!("[Plugin] {}", msg);
    let _ = ctx;
}

unsafe extern "C" fn host_set_status_item(ctx: *mut c_void, item: *const FfiStatusBarItem) {
    if ctx.is_null() || item.is_null() { return; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let ffi_item = &*item;

    let id = ffi_item.id.to_string();
    let text = ffi_item.text.to_string();
    let icon = if ffi_item.icon.ptr.is_null() { None } else { Some(ffi_item.icon.to_string()) };
    let tooltip = if ffi_item.tooltip.ptr.is_null() { None } else { Some(ffi_item.tooltip.to_string()) };

    let status_item = StatusBarItem {
        id: id.clone(),
        icon,
        text,
        tooltip,
        align: if ffi_item.align_right { StatusBarAlign::Right } else { StatusBarAlign::Left },
        priority: ffi_item.priority,
    };

    // Store in API - use current plugin ID if available
    let plugin_id = api_impl.current_plugin_id.clone().unwrap_or_default();
    api_impl.status_bar_items.insert(id, (status_item, plugin_id));
}

unsafe extern "C" fn host_remove_status_item(ctx: *mut c_void, id: *const c_char) {
    if ctx.is_null() || id.is_null() { return; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let id_str = CStr::from_ptr(id).to_string_lossy().into_owned();
    api_impl.status_bar_items.remove(&id_str);
}

unsafe extern "C" fn host_undo(ctx: *mut c_void) -> bool {
    if ctx.is_null() { return false; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    if api_impl.can_undo {
        api_impl.pending_undo = 1;
        true
    } else {
        false
    }
}

unsafe extern "C" fn host_redo(ctx: *mut c_void) -> bool {
    if ctx.is_null() { return false; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    if api_impl.can_redo {
        api_impl.pending_redo = 1;
        true
    } else {
        false
    }
}

unsafe extern "C" fn host_can_undo(ctx: *mut c_void) -> bool {
    if ctx.is_null() { return false; }
    let api_impl = &*(ctx as *const EditorApiImpl);
    api_impl.can_undo
}

unsafe extern "C" fn host_can_redo(ctx: *mut c_void) -> bool {
    if ctx.is_null() { return false; }
    let api_impl = &*(ctx as *const EditorApiImpl);
    api_impl.can_redo
}

// ============================================================================
// Panel callbacks
// ============================================================================

unsafe extern "C" fn host_register_panel(ctx: *mut c_void, panel: *const FfiPanelDefinition) -> bool {
    if ctx.is_null() || panel.is_null() { return false; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let ffi_panel = &*panel;

    let id = ffi_panel.id.to_string();
    let title = ffi_panel.title.to_string();
    let icon = if ffi_panel.icon.ptr.is_null() { None } else { Some(ffi_panel.icon.to_string()) };

    info!("Registering panel: id='{}', title='{}', icon={:?}", id, title, icon);

    let location = match ffi_panel.location {
        FfiPanelLocation::Left => crate::plugin_core::PanelLocation::Left,
        FfiPanelLocation::Right => crate::plugin_core::PanelLocation::Right,
        FfiPanelLocation::Bottom => crate::plugin_core::PanelLocation::Bottom,
        FfiPanelLocation::Floating => crate::plugin_core::PanelLocation::Floating,
    };

    let panel_def = crate::plugin_core::PanelDefinition {
        id: id.clone(),
        title,
        icon,
        default_location: location,
        min_size: [ffi_panel.min_width, ffi_panel.min_height],
        closable: ffi_panel.closable,
    };

    // Check if already registered
    if api_impl.panels.iter().any(|(p, _)| p.id == id) {
        warn!("Panel '{}' already registered", id);
        return false;
    }

    let plugin_id = api_impl.current_plugin_id.clone().unwrap_or_default();
    api_impl.panels.push((panel_def, plugin_id));
    api_impl.panel_visible.insert(id.clone(), true);
    info!("Panel '{}' registered successfully. Total panels: {}", id, api_impl.panels.len());
    true
}

unsafe extern "C" fn host_unregister_panel(ctx: *mut c_void, id: *const c_char) {
    if ctx.is_null() || id.is_null() { return; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let id_str = CStr::from_ptr(id).to_string_lossy().into_owned();

    api_impl.panels.retain(|(p, _)| p.id != id_str);
    api_impl.panel_contents.remove(&id_str);
    api_impl.panel_visible.remove(&id_str);
}

unsafe extern "C" fn host_set_panel_content(ctx: *mut c_void, panel_id: *const c_char, widgets_json: *const c_char) {
    if ctx.is_null() || panel_id.is_null() || widgets_json.is_null() { return; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let id_str = CStr::from_ptr(panel_id).to_string_lossy().into_owned();
    let json_str = CStr::from_ptr(widgets_json).to_string_lossy();

    // Parse JSON to widgets
    match serde_json::from_str::<Vec<crate::ui_api::Widget>>(&json_str) {
        Ok(widgets) => {
            info!("Panel '{}' content set: {} widgets", id_str, widgets.len());
            api_impl.panel_contents.insert(id_str, widgets);
        }
        Err(e) => {
            error!("Failed to parse panel content JSON for '{}': {}", id_str, e);
            error!("JSON was: {}", &json_str[..json_str.len().min(500)]);
        }
    }
}

unsafe extern "C" fn host_set_panel_visible(ctx: *mut c_void, panel_id: *const c_char, visible: bool) {
    if ctx.is_null() || panel_id.is_null() { return; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let id_str = CStr::from_ptr(panel_id).to_string_lossy().into_owned();

    api_impl.panel_visible.insert(id_str, visible);
}

unsafe extern "C" fn host_is_panel_visible(ctx: *mut c_void, panel_id: *const c_char) -> bool {
    if ctx.is_null() || panel_id.is_null() { return false; }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let id_str = CStr::from_ptr(panel_id).to_string_lossy();

    api_impl.panel_visible.get(id_str.as_ref()).copied().unwrap_or(false)
}

// ============================================================================
// Entity operation callbacks
// ============================================================================

unsafe extern "C" fn host_get_entity_by_name(ctx: *mut c_void, name: *const c_char) -> FfiEntityId {
    if ctx.is_null() || name.is_null() { return FfiEntityId::INVALID; }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let name_str = CStr::from_ptr(name).to_string_lossy();

    api_impl.get_entity_by_name(&name_str)
        .map(|id| FfiEntityId(id.0))
        .unwrap_or(FfiEntityId::INVALID)
}

unsafe extern "C" fn host_get_entity_transform(ctx: *mut c_void, entity: FfiEntityId) -> FfiTransform {
    if ctx.is_null() || !entity.is_valid() { return FfiTransform::default(); }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let entity_id = EntityId(entity.0);

    api_impl.entity_transforms.get(&entity_id)
        .map(|t| FfiTransform {
            translation: t.translation,
            rotation: t.rotation,
            scale: t.scale,
        })
        .unwrap_or_default()
}

unsafe extern "C" fn host_set_entity_transform(ctx: *mut c_void, entity: FfiEntityId, transform: *const FfiTransform) {
    if ctx.is_null() || !entity.is_valid() || transform.is_null() { return; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let entity_id = EntityId(entity.0);
    let t = &*transform;

    let plugin_transform = PluginTransform {
        translation: t.translation,
        rotation: t.rotation,
        scale: t.scale,
    };

    api_impl.pending_operations.push(PendingOperation::SetTransform {
        entity: entity_id,
        transform: plugin_transform,
    });
}

unsafe extern "C" fn host_get_entity_name(ctx: *mut c_void, entity: FfiEntityId) -> FfiOwnedString {
    if ctx.is_null() || !entity.is_valid() { return FfiOwnedString::empty(); }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let entity_id = EntityId(entity.0);

    api_impl.entity_names.get(&entity_id)
        .map(|n| FfiOwnedString::from_string(n.clone()))
        .unwrap_or_else(FfiOwnedString::empty)
}

unsafe extern "C" fn host_set_entity_name(ctx: *mut c_void, entity: FfiEntityId, name: *const c_char) {
    if ctx.is_null() || !entity.is_valid() || name.is_null() { return; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let entity_id = EntityId(entity.0);
    let name_str = CStr::from_ptr(name).to_string_lossy().into_owned();

    api_impl.pending_operations.push(PendingOperation::SetEntityName {
        entity: entity_id,
        name: name_str,
    });
}

unsafe extern "C" fn host_get_entity_visible(ctx: *mut c_void, entity: FfiEntityId) -> bool {
    if ctx.is_null() || !entity.is_valid() { return false; }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let entity_id = EntityId(entity.0);

    api_impl.entity_visibility.get(&entity_id).copied().unwrap_or(true)
}

unsafe extern "C" fn host_set_entity_visible(ctx: *mut c_void, entity: FfiEntityId, visible: bool) {
    if ctx.is_null() || !entity.is_valid() { return; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let entity_id = EntityId(entity.0);

    api_impl.pending_operations.push(PendingOperation::SetEntityVisible {
        entity: entity_id,
        visible,
    });
}

unsafe extern "C" fn host_spawn_entity(ctx: *mut c_void, name: *const c_char, transform: *const FfiTransform) -> FfiEntityId {
    if ctx.is_null() || name.is_null() || transform.is_null() { return FfiEntityId::INVALID; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let name_str = CStr::from_ptr(name).to_string_lossy().into_owned();
    let t = &*transform;

    // Create an entity definition with basic data
    let def = editor_plugin_api::api::EntityDefinition {
        name: name_str,
        node_type: String::new(), // Empty = basic node
        transform: PluginTransform {
            translation: t.translation,
            rotation: t.rotation,
            scale: t.scale,
        },
        parent: None,
    };

    api_impl.pending_operations.push(PendingOperation::SpawnEntity(def));

    // Return INVALID for now - entity will be created by sync system
    // In a future iteration we could return a placeholder ID
    FfiEntityId::INVALID
}

unsafe extern "C" fn host_despawn_entity(ctx: *mut c_void, entity: FfiEntityId) {
    if ctx.is_null() || !entity.is_valid() { return; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let entity_id = EntityId(entity.0);

    api_impl.pending_operations.push(PendingOperation::DespawnEntity(entity_id));
}

unsafe extern "C" fn host_get_entity_parent(ctx: *mut c_void, entity: FfiEntityId) -> FfiEntityId {
    if ctx.is_null() || !entity.is_valid() { return FfiEntityId::INVALID; }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let entity_id = EntityId(entity.0);

    api_impl.entity_parents.get(&entity_id)
        .and_then(|opt| *opt)
        .map(|id| FfiEntityId(id.0))
        .unwrap_or(FfiEntityId::INVALID)
}

unsafe extern "C" fn host_get_entity_children(ctx: *mut c_void, entity: FfiEntityId) -> FfiEntityList {
    if ctx.is_null() || !entity.is_valid() { return FfiEntityList::empty(); }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let entity_id = EntityId(entity.0);

    api_impl.entity_children.get(&entity_id)
        .map(|children| {
            let ffi_children: Vec<FfiEntityId> = children.iter()
                .map(|id| FfiEntityId(id.0))
                .collect();
            FfiEntityList::from_vec(ffi_children)
        })
        .unwrap_or_else(FfiEntityList::empty)
}

unsafe extern "C" fn host_reparent_entity(ctx: *mut c_void, entity: FfiEntityId, new_parent: FfiEntityId) {
    if ctx.is_null() || !entity.is_valid() { return; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let entity_id = EntityId(entity.0);
    let parent_id = if new_parent.is_valid() { Some(EntityId(new_parent.0)) } else { None };

    api_impl.pending_operations.push(PendingOperation::ReparentEntity {
        entity: entity_id,
        new_parent: parent_id,
    });
}

// ============================================================================
// Selection callbacks
// ============================================================================

unsafe extern "C" fn host_get_selected_entity(ctx: *mut c_void) -> FfiEntityId {
    if ctx.is_null() { return FfiEntityId::INVALID; }
    let api_impl = &*(ctx as *const EditorApiImpl);

    api_impl.selected_entity
        .map(|id| FfiEntityId(id.0))
        .unwrap_or(FfiEntityId::INVALID)
}

unsafe extern "C" fn host_set_selected_entity(ctx: *mut c_void, entity: FfiEntityId) {
    if ctx.is_null() { return; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);

    let entity_id = if entity.is_valid() { Some(EntityId(entity.0)) } else { None };
    api_impl.pending_operations.push(PendingOperation::SetSelectedEntity(entity_id));
}

unsafe extern "C" fn host_clear_selection(ctx: *mut c_void) {
    if ctx.is_null() { return; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);

    api_impl.pending_operations.push(PendingOperation::SetSelectedEntity(None));
}

// ============================================================================
// Toolbar callbacks
// ============================================================================

unsafe extern "C" fn host_add_toolbar_button(ctx: *mut c_void, id: u64, icon: *const c_char, tooltip: *const c_char) {
    if ctx.is_null() || icon.is_null() || tooltip.is_null() { return; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);

    let icon_str = CStr::from_ptr(icon).to_string_lossy().into_owned();
    let tooltip_str = CStr::from_ptr(tooltip).to_string_lossy().into_owned();

    let item = ToolbarItem {
        id: UiId(id),
        icon: icon_str,
        tooltip: tooltip_str,
        group: None,
    };

    let plugin_id = api_impl.current_plugin_id.clone().unwrap_or_default();
    api_impl.toolbar_items.push((item, plugin_id));
    info!("Toolbar button added: id={}", id);
}

unsafe extern "C" fn host_remove_toolbar_item(ctx: *mut c_void, id: u64) {
    if ctx.is_null() { return; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);

    api_impl.toolbar_items.retain(|(item, _)| item.id.0 != id);
    info!("Toolbar item removed: id={}", id);
}

// ============================================================================
// Menu callbacks
// ============================================================================

unsafe extern "C" fn host_add_menu_item(ctx: *mut c_void, menu: u8, item: *const FfiMenuItem) {
    if ctx.is_null() || item.is_null() { return; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let ffi_item = &*item;

    // Convert menu location
    let location = match menu {
        0 => MenuLocation::File,
        1 => MenuLocation::Edit,
        2 => MenuLocation::View,
        3 => MenuLocation::Scene,
        4 => MenuLocation::Tools,
        5 => MenuLocation::Help,
        _ => MenuLocation::Tools,
    };

    // Skip if separator
    if ffi_item.is_separator {
        // For separators, we could add a special separator item, but for now skip
        return;
    }

    let label = ffi_item.label.to_string();
    let shortcut = if ffi_item.shortcut.ptr.is_null() { None } else { Some(ffi_item.shortcut.to_string()) };
    let icon = if ffi_item.icon.ptr.is_null() { None } else { Some(ffi_item.icon.to_string()) };

    let mut menu_item = MenuItem::new(&label, UiId(ffi_item.id));
    if let Some(s) = shortcut {
        menu_item = menu_item.shortcut(s);
    }
    if let Some(i) = icon {
        menu_item = menu_item.icon(i);
    }
    if !ffi_item.enabled {
        menu_item.enabled = false;
    }

    let plugin_id = api_impl.current_plugin_id.clone().unwrap_or_default();
    info!("Menu item added: '{}' to {:?}", label, location);
    api_impl.menu_items.push((location, menu_item, plugin_id));
}

unsafe extern "C" fn host_remove_menu_item(ctx: *mut c_void, id: u64) {
    if ctx.is_null() { return; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);

    api_impl.menu_items.retain(|(_, item, _)| item.id.0 != id);
    info!("Menu item removed: id={}", id);
}

unsafe extern "C" fn host_set_menu_item_enabled(ctx: *mut c_void, id: u64, enabled: bool) {
    if ctx.is_null() { return; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);

    for (_, item, _) in &mut api_impl.menu_items {
        if item.id.0 == id {
            item.enabled = enabled;
            break;
        }
    }
}

unsafe extern "C" fn host_set_menu_item_checked(ctx: *mut c_void, _id: u64, _checked: bool) {
    if ctx.is_null() { return; }
    // MenuItem doesn't have a checked field currently
    // This would need to be added to MenuItem if needed
    warn!("set_menu_item_checked not implemented: MenuItem doesn't support checked state");
}

// ============================================================================
// Tab callbacks (docked tabs in panel areas)
// ============================================================================

unsafe extern "C" fn host_register_tab(ctx: *mut c_void, tab: *const FfiTabDefinition) -> bool {
    if ctx.is_null() || tab.is_null() { return false; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let ffi_tab = &*tab;

    let id = ffi_tab.id.to_string();
    let title = ffi_tab.title.to_string();
    let icon = if ffi_tab.icon.ptr.is_null() { None } else { Some(ffi_tab.icon.to_string()) };

    let location = match ffi_tab.location {
        FfiTabLocation::Left => TabLocation::Left,
        FfiTabLocation::Right => TabLocation::Right,
        FfiTabLocation::Bottom => TabLocation::Bottom,
    };

    // Check if already registered
    if api_impl.tabs.iter().any(|(t, _)| t.id == id) {
        warn!("Tab '{}' already registered", id);
        return false;
    }

    let plugin_tab = PluginTab {
        id: id.clone(),
        title,
        icon,
        location,
    };

    let plugin_id = api_impl.current_plugin_id.clone().unwrap_or_default();
    info!("Tab '{}' registered at {:?}", id, location);
    api_impl.tabs.push((plugin_tab, plugin_id));
    true
}

unsafe extern "C" fn host_unregister_tab(ctx: *mut c_void, id: *const c_char) {
    if ctx.is_null() || id.is_null() { return; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let id_str = CStr::from_ptr(id).to_string_lossy().into_owned();

    api_impl.tabs.retain(|(t, _)| t.id != id_str);
    api_impl.tab_contents.remove(&id_str);
    info!("Tab '{}' unregistered", id_str);
}

unsafe extern "C" fn host_set_tab_content(ctx: *mut c_void, tab_id: *const c_char, widgets_json: *const c_char) {
    if ctx.is_null() || tab_id.is_null() || widgets_json.is_null() { return; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let id_str = CStr::from_ptr(tab_id).to_string_lossy().into_owned();
    let json_str = CStr::from_ptr(widgets_json).to_string_lossy();

    // Parse JSON to widgets
    match serde_json::from_str::<Vec<crate::ui_api::Widget>>(&json_str) {
        Ok(widgets) => {
            api_impl.tab_contents.insert(id_str, widgets);
        }
        Err(e) => {
            error!("Failed to parse tab content JSON: {}", e);
        }
    }
}

unsafe extern "C" fn host_set_active_tab(ctx: *mut c_void, location: u8, tab_id: *const c_char) {
    if ctx.is_null() || tab_id.is_null() { return; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let id_str = CStr::from_ptr(tab_id).to_string_lossy().into_owned();

    let loc = match location {
        0 => TabLocation::Left,
        1 => TabLocation::Right,
        2 => TabLocation::Bottom,
        _ => return,
    };

    if id_str.is_empty() {
        api_impl.clear_active_tab(loc);
    } else {
        api_impl.set_active_tab(loc, id_str);
    }
}

unsafe extern "C" fn host_get_active_tab(ctx: *mut c_void, location: u8) -> FfiOwnedString {
    if ctx.is_null() { return FfiOwnedString::empty(); }
    let api_impl = &*(ctx as *const EditorApiImpl);

    let loc = match location {
        0 => TabLocation::Left,
        1 => TabLocation::Right,
        2 => TabLocation::Bottom,
        _ => return FfiOwnedString::empty(),
    };

    api_impl.get_active_tab(loc)
        .map(|s| FfiOwnedString::from_string(s.to_string()))
        .unwrap_or_else(FfiOwnedString::empty)
}

// ============================================================================
// Asset system callbacks
// ============================================================================

/// Validate an asset path - returns None if path is invalid (security check)
fn validate_asset_path(path: &str) -> Option<std::path::PathBuf> {
    // Reject empty paths
    if path.is_empty() {
        return Some(std::path::PathBuf::new());
    }
    // Reject absolute paths (Unix or Windows style)
    if path.starts_with('/') || path.starts_with('\\') {
        return None;
    }
    // Reject Windows drive letters
    if path.len() >= 2 && path.chars().nth(1) == Some(':') {
        return None;
    }
    // Reject parent directory traversal
    if path.contains("..") {
        return None;
    }
    // Normalize path separators
    let normalized = path.replace('\\', "/");
    Some(std::path::PathBuf::from(normalized))
}

unsafe extern "C" fn host_get_asset_list(ctx: *mut c_void, folder: *const c_char) -> FfiAssetList {
    if ctx.is_null() { return FfiAssetList::empty(); }
    let api_impl = &*(ctx as *const EditorApiImpl);

    let folder_str = if folder.is_null() {
        String::new()
    } else {
        CStr::from_ptr(folder).to_string_lossy().into_owned()
    };

    let Some(rel_path) = validate_asset_path(&folder_str) else {
        warn!("Invalid asset path: {}", folder_str);
        return FfiAssetList::empty();
    };

    let Some(assets_path) = api_impl.get_project_assets_path() else {
        return FfiAssetList::empty();
    };

    let full_path = assets_path.join(&rel_path);

    if !full_path.exists() || !full_path.is_dir() {
        return FfiAssetList::empty();
    }

    let mut assets = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&full_path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            let file_name = entry.file_name().to_string_lossy().into_owned();

            // Get relative path from assets folder
            let relative_path = if rel_path.as_os_str().is_empty() {
                file_name.clone()
            } else {
                format!("{}/{}", rel_path.display(), file_name)
            };

            let metadata = entry.metadata().ok();
            let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);

            let asset_type = if is_dir {
                FfiAssetType::Folder
            } else {
                let ext = entry_path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                FfiAssetType::from_extension(ext)
            };

            assets.push(FfiAssetInfo {
                path: FfiOwnedString::from_string(relative_path),
                name: FfiOwnedString::from_string(file_name),
                asset_type,
                size_bytes: size,
                exists: true,
            });
        }
    }

    FfiAssetList::from_vec(assets)
}

unsafe extern "C" fn host_get_asset_info(ctx: *mut c_void, path: *const c_char) -> FfiAssetInfo {
    if ctx.is_null() || path.is_null() { return FfiAssetInfo::empty(); }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let path_str = CStr::from_ptr(path).to_string_lossy().into_owned();

    let Some(rel_path) = validate_asset_path(&path_str) else {
        return FfiAssetInfo::empty();
    };

    let Some(assets_path) = api_impl.get_project_assets_path() else {
        return FfiAssetInfo::empty();
    };

    let full_path = assets_path.join(&rel_path);

    if !full_path.exists() {
        return FfiAssetInfo::empty();
    }

    let metadata = std::fs::metadata(&full_path).ok();
    let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
    let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);

    let file_name = full_path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let asset_type = if is_dir {
        FfiAssetType::Folder
    } else {
        let ext = full_path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        FfiAssetType::from_extension(ext)
    };

    FfiAssetInfo {
        path: FfiOwnedString::from_string(path_str),
        name: FfiOwnedString::from_string(file_name),
        asset_type,
        size_bytes: size,
        exists: true,
    }
}

unsafe extern "C" fn host_asset_exists(ctx: *mut c_void, path: *const c_char) -> bool {
    if ctx.is_null() || path.is_null() { return false; }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let path_str = CStr::from_ptr(path).to_string_lossy().into_owned();

    let Some(rel_path) = validate_asset_path(&path_str) else {
        return false;
    };

    let Some(assets_path) = api_impl.get_project_assets_path() else {
        return false;
    };

    assets_path.join(&rel_path).exists()
}

unsafe extern "C" fn host_read_asset_text(ctx: *mut c_void, path: *const c_char) -> FfiOwnedString {
    if ctx.is_null() || path.is_null() { return FfiOwnedString::empty(); }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let path_str = CStr::from_ptr(path).to_string_lossy().into_owned();

    let Some(rel_path) = validate_asset_path(&path_str) else {
        warn!("Invalid asset path for read: {}", path_str);
        return FfiOwnedString::empty();
    };

    let Some(assets_path) = api_impl.get_project_assets_path() else {
        return FfiOwnedString::empty();
    };

    let full_path = assets_path.join(&rel_path);

    match std::fs::read_to_string(&full_path) {
        Ok(content) => FfiOwnedString::from_string(content),
        Err(e) => {
            warn!("Failed to read asset text '{}': {}", path_str, e);
            FfiOwnedString::empty()
        }
    }
}

unsafe extern "C" fn host_read_asset_bytes(ctx: *mut c_void, path: *const c_char) -> FfiBytes {
    if ctx.is_null() || path.is_null() { return FfiBytes::empty(); }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let path_str = CStr::from_ptr(path).to_string_lossy().into_owned();

    let Some(rel_path) = validate_asset_path(&path_str) else {
        warn!("Invalid asset path for read: {}", path_str);
        return FfiBytes::empty();
    };

    let Some(assets_path) = api_impl.get_project_assets_path() else {
        return FfiBytes::empty();
    };

    let full_path = assets_path.join(&rel_path);

    match std::fs::read(&full_path) {
        Ok(content) => FfiBytes::from_vec(content),
        Err(e) => {
            warn!("Failed to read asset bytes '{}': {}", path_str, e);
            FfiBytes::empty()
        }
    }
}

unsafe extern "C" fn host_write_asset_text(ctx: *mut c_void, path: *const c_char, content: *const c_char) -> bool {
    if ctx.is_null() || path.is_null() || content.is_null() { return false; }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let path_str = CStr::from_ptr(path).to_string_lossy().into_owned();
    let content_str = CStr::from_ptr(content).to_string_lossy().into_owned();

    let Some(rel_path) = validate_asset_path(&path_str) else {
        warn!("Invalid asset path for write: {}", path_str);
        return false;
    };

    let Some(assets_path) = api_impl.get_project_assets_path() else {
        warn!("No project open, cannot write asset");
        return false;
    };

    let full_path = assets_path.join(&rel_path);

    // Create parent directories if needed
    if let Some(parent) = full_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            warn!("Failed to create directories for '{}': {}", path_str, e);
            return false;
        }
    }

    match std::fs::write(&full_path, content_str) {
        Ok(()) => {
            info!("Asset written: {}", path_str);
            true
        }
        Err(e) => {
            warn!("Failed to write asset '{}': {}", path_str, e);
            false
        }
    }
}

unsafe extern "C" fn host_write_asset_bytes(ctx: *mut c_void, path: *const c_char, data: *const FfiBytes) -> bool {
    if ctx.is_null() || path.is_null() || data.is_null() { return false; }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let path_str = CStr::from_ptr(path).to_string_lossy().into_owned();
    let ffi_bytes = &*data;

    let Some(rel_path) = validate_asset_path(&path_str) else {
        warn!("Invalid asset path for write: {}", path_str);
        return false;
    };

    let Some(assets_path) = api_impl.get_project_assets_path() else {
        warn!("No project open, cannot write asset");
        return false;
    };

    let full_path = assets_path.join(&rel_path);

    // Create parent directories if needed
    if let Some(parent) = full_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            warn!("Failed to create directories for '{}': {}", path_str, e);
            return false;
        }
    }

    // Get bytes from FfiBytes
    let bytes = ffi_bytes.as_slice();

    match std::fs::write(&full_path, bytes) {
        Ok(()) => {
            info!("Asset written: {} ({} bytes)", path_str, bytes.len());
            true
        }
        Err(e) => {
            warn!("Failed to write asset '{}': {}", path_str, e);
            false
        }
    }
}

unsafe extern "C" fn host_create_folder(ctx: *mut c_void, path: *const c_char) -> bool {
    if ctx.is_null() || path.is_null() { return false; }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let path_str = CStr::from_ptr(path).to_string_lossy().into_owned();

    let Some(rel_path) = validate_asset_path(&path_str) else {
        warn!("Invalid asset path for folder creation: {}", path_str);
        return false;
    };

    let Some(assets_path) = api_impl.get_project_assets_path() else {
        warn!("No project open, cannot create folder");
        return false;
    };

    let full_path = assets_path.join(&rel_path);

    match std::fs::create_dir_all(&full_path) {
        Ok(()) => {
            info!("Folder created: {}", path_str);
            true
        }
        Err(e) => {
            warn!("Failed to create folder '{}': {}", path_str, e);
            false
        }
    }
}

unsafe extern "C" fn host_delete_asset(ctx: *mut c_void, path: *const c_char) -> bool {
    if ctx.is_null() || path.is_null() { return false; }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let path_str = CStr::from_ptr(path).to_string_lossy().into_owned();

    let Some(rel_path) = validate_asset_path(&path_str) else {
        warn!("Invalid asset path for deletion: {}", path_str);
        return false;
    };

    // Prevent deleting root assets folder
    if rel_path.as_os_str().is_empty() {
        warn!("Cannot delete root assets folder");
        return false;
    }

    let Some(assets_path) = api_impl.get_project_assets_path() else {
        warn!("No project open, cannot delete asset");
        return false;
    };

    let full_path = assets_path.join(&rel_path);

    if !full_path.exists() {
        return false;
    }

    let result = if full_path.is_dir() {
        std::fs::remove_dir_all(&full_path)
    } else {
        std::fs::remove_file(&full_path)
    };

    match result {
        Ok(()) => {
            info!("Asset deleted: {}", path_str);
            true
        }
        Err(e) => {
            warn!("Failed to delete asset '{}': {}", path_str, e);
            false
        }
    }
}

unsafe extern "C" fn host_rename_asset(ctx: *mut c_void, old_path: *const c_char, new_path: *const c_char) -> bool {
    if ctx.is_null() || old_path.is_null() || new_path.is_null() { return false; }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let old_str = CStr::from_ptr(old_path).to_string_lossy().into_owned();
    let new_str = CStr::from_ptr(new_path).to_string_lossy().into_owned();

    let Some(old_rel) = validate_asset_path(&old_str) else {
        warn!("Invalid source asset path: {}", old_str);
        return false;
    };

    let Some(new_rel) = validate_asset_path(&new_str) else {
        warn!("Invalid destination asset path: {}", new_str);
        return false;
    };

    // Prevent renaming root folder
    if old_rel.as_os_str().is_empty() {
        warn!("Cannot rename root assets folder");
        return false;
    }

    let Some(assets_path) = api_impl.get_project_assets_path() else {
        warn!("No project open, cannot rename asset");
        return false;
    };

    let old_full = assets_path.join(&old_rel);
    let new_full = assets_path.join(&new_rel);

    if !old_full.exists() {
        warn!("Source asset does not exist: {}", old_str);
        return false;
    }

    // Create parent directories for destination if needed
    if let Some(parent) = new_full.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            warn!("Failed to create directories for '{}': {}", new_str, e);
            return false;
        }
    }

    match std::fs::rename(&old_full, &new_full) {
        Ok(()) => {
            info!("Asset renamed: {} -> {}", old_str, new_str);
            true
        }
        Err(e) => {
            warn!("Failed to rename asset '{}' to '{}': {}", old_str, new_str, e);
            false
        }
    }
}

// ============================================================================
// Pub/Sub callbacks
// ============================================================================

unsafe extern "C" fn host_subscribe_event(ctx: *mut c_void, event_type: *const c_char) {
    if ctx.is_null() || event_type.is_null() { return; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let event_str = CStr::from_ptr(event_type).to_string_lossy().into_owned();

    // Clone the plugin_id to avoid borrow issues
    let plugin_id = api_impl.current_plugin_id.clone();
    if let Some(plugin_id) = plugin_id {
        api_impl.subscribe_plugin(&plugin_id, &event_str);
        info!("Plugin '{}' subscribed to '{}'", plugin_id, event_str);
    }
}

unsafe extern "C" fn host_unsubscribe_event(ctx: *mut c_void, event_type: *const c_char) {
    if ctx.is_null() || event_type.is_null() { return; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let event_str = CStr::from_ptr(event_type).to_string_lossy().into_owned();

    // Clone the plugin_id to avoid borrow issues
    let plugin_id = api_impl.current_plugin_id.clone();
    if let Some(plugin_id) = plugin_id {
        api_impl.unsubscribe_plugin(&plugin_id, &event_str);
        info!("Plugin '{}' unsubscribed from '{}'", plugin_id, event_str);
    }
}

unsafe extern "C" fn host_publish_event(ctx: *mut c_void, event_type: *const c_char, data_json: *const c_char) {
    if ctx.is_null() || event_type.is_null() { return; }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let event_str = CStr::from_ptr(event_type).to_string_lossy().into_owned();
    let data_str = if data_json.is_null() {
        "{}".to_string()
    } else {
        CStr::from_ptr(data_json).to_string_lossy().into_owned()
    };

    // Clone the plugin_id to avoid borrow issues
    let plugin_id = api_impl.current_plugin_id.clone();
    if let Some(plugin_id) = plugin_id {
        api_impl.publish_event(&plugin_id, &event_str, &data_str);
        info!("Plugin '{}' published event '{}'", plugin_id, event_str);
    }
}

// ============================================================================
// Event serialization helpers
// ============================================================================

use editor_plugin_api::events::UiEvent;

/// Serialize a UI event to JSON
fn serialize_ui_event(event: &UiEvent) -> String {
    let json = match event {
        UiEvent::ButtonClicked(id) => serde_json::json!({
            "type": "ui",
            "event": "button_clicked",
            "id": id.0
        }),
        UiEvent::CheckboxToggled { id, checked } => serde_json::json!({
            "type": "ui",
            "event": "checkbox_toggled",
            "id": id.0,
            "checked": checked
        }),
        UiEvent::SliderChanged { id, value } => serde_json::json!({
            "type": "ui",
            "event": "slider_changed",
            "id": id.0,
            "value": value
        }),
        UiEvent::SliderIntChanged { id, value } => serde_json::json!({
            "type": "ui",
            "event": "slider_int_changed",
            "id": id.0,
            "value": value
        }),
        UiEvent::TextInputChanged { id, value } => serde_json::json!({
            "type": "ui",
            "event": "text_input_changed",
            "id": id.0,
            "value": value
        }),
        UiEvent::TextInputSubmitted { id, value } => serde_json::json!({
            "type": "ui",
            "event": "text_input_submitted",
            "id": id.0,
            "value": value
        }),
        UiEvent::DropdownSelected { id, index } => serde_json::json!({
            "type": "ui",
            "event": "dropdown_selected",
            "id": id.0,
            "index": index
        }),
        UiEvent::ColorChanged { id, color } => serde_json::json!({
            "type": "ui",
            "event": "color_changed",
            "id": id.0,
            "color": color
        }),
        UiEvent::TreeNodeToggled { id, expanded } => serde_json::json!({
            "type": "ui",
            "event": "tree_node_toggled",
            "id": id.0,
            "expanded": expanded
        }),
        UiEvent::TreeNodeSelected(id) => serde_json::json!({
            "type": "ui",
            "event": "tree_node_selected",
            "id": id.0
        }),
        UiEvent::TabSelected { id, index } => serde_json::json!({
            "type": "ui",
            "event": "tab_selected",
            "id": id.0,
            "index": index
        }),
        UiEvent::TabClosed { id, index } => serde_json::json!({
            "type": "ui",
            "event": "tab_closed",
            "id": id.0,
            "index": index
        }),
        UiEvent::TableRowSelected { id, row } => serde_json::json!({
            "type": "ui",
            "event": "table_row_selected",
            "id": id.0,
            "row": row
        }),
        UiEvent::TableSortChanged { id, column, ascending } => serde_json::json!({
            "type": "ui",
            "event": "table_sort_changed",
            "id": id.0,
            "column": column,
            "ascending": ascending
        }),
        UiEvent::CustomEvent { type_id, data } => serde_json::json!({
            "type": "ui",
            "event": "custom",
            "type_id": type_id,
            "data": data
        }),
    };
    json.to_string()
}

/// Get the event type string for a UI event
fn get_ui_event_type(event: &UiEvent) -> String {
    match event {
        UiEvent::ButtonClicked(_) => "ui.button_clicked".to_string(),
        UiEvent::CheckboxToggled { .. } => "ui.checkbox_toggled".to_string(),
        UiEvent::SliderChanged { .. } => "ui.slider_changed".to_string(),
        UiEvent::SliderIntChanged { .. } => "ui.slider_int_changed".to_string(),
        UiEvent::TextInputChanged { .. } => "ui.text_input_changed".to_string(),
        UiEvent::TextInputSubmitted { .. } => "ui.text_input_submitted".to_string(),
        UiEvent::DropdownSelected { .. } => "ui.dropdown_selected".to_string(),
        UiEvent::ColorChanged { .. } => "ui.color_changed".to_string(),
        UiEvent::TreeNodeToggled { .. } => "ui.tree_node_toggled".to_string(),
        UiEvent::TreeNodeSelected(_) => "ui.tree_node_selected".to_string(),
        UiEvent::TabSelected { .. } => "ui.tab_selected".to_string(),
        UiEvent::TabClosed { .. } => "ui.tab_closed".to_string(),
        UiEvent::TableRowSelected { .. } => "ui.table_row_selected".to_string(),
        UiEvent::TableSortChanged { .. } => "ui.table_sort_changed".to_string(),
        UiEvent::CustomEvent { .. } => "ui.custom".to_string(),
    }
}

/// Serialize an editor event to JSON
fn serialize_editor_event(event: &EditorEvent) -> String {
    let json = match event {
        EditorEvent::EntitySelected(id) => serde_json::json!({
            "type": "editor",
            "event": "entity_selected",
            "entity_id": id.0
        }),
        EditorEvent::EntityDeselected(id) => serde_json::json!({
            "type": "editor",
            "event": "entity_deselected",
            "entity_id": id.0
        }),
        EditorEvent::SceneLoaded { path } => serde_json::json!({
            "type": "editor",
            "event": "scene_loaded",
            "path": path
        }),
        EditorEvent::SceneSaved { path } => serde_json::json!({
            "type": "editor",
            "event": "scene_saved",
            "path": path
        }),
        EditorEvent::PlayStarted => serde_json::json!({
            "type": "editor",
            "event": "play_started"
        }),
        EditorEvent::PlayStopped => serde_json::json!({
            "type": "editor",
            "event": "play_stopped"
        }),
        EditorEvent::ProjectOpened { path } => serde_json::json!({
            "type": "editor",
            "event": "project_opened",
            "path": path
        }),
        EditorEvent::ProjectClosed => serde_json::json!({
            "type": "editor",
            "event": "project_closed"
        }),
        EditorEvent::UiEvent(ui_event) => {
            // UI events are serialized separately
            return serialize_ui_event(ui_event);
        }
        EditorEvent::CustomEvent { plugin_id, event_type, data } => serde_json::json!({
            "type": "plugin",
            "event": "custom",
            "source": plugin_id,
            "event_type": event_type,
            "data": data
        }),
    };
    json.to_string()
}

/// Get the event type string for an editor event
fn get_event_type(event: &EditorEvent) -> String {
    match event {
        EditorEvent::EntitySelected(_) => "editor.entity_selected".to_string(),
        EditorEvent::EntityDeselected(_) => "editor.entity_deselected".to_string(),
        EditorEvent::SceneLoaded { .. } => "editor.scene_loaded".to_string(),
        EditorEvent::SceneSaved { .. } => "editor.scene_saved".to_string(),
        EditorEvent::PlayStarted => "editor.play_started".to_string(),
        EditorEvent::PlayStopped => "editor.play_stopped".to_string(),
        EditorEvent::ProjectOpened { .. } => "editor.project_opened".to_string(),
        EditorEvent::ProjectClosed => "editor.project_closed".to_string(),
        EditorEvent::UiEvent(ui_event) => get_ui_event_type(ui_event),
        EditorEvent::CustomEvent { event_type, .. } => format!("plugin.{}", event_type),
    }
}

/// FFI-safe wrapper for a loaded plugin
pub struct FfiPluginWrapper {
    /// Plugin handle (opaque pointer to plugin state)
    handle: PluginHandle,
    /// Vtable with function pointers
    vtable: PluginVTable,
    /// Cached manifest
    manifest: PluginManifest,
    /// Whether the plugin is enabled
    enabled: bool,
}

// Safety: Plugin handles are only accessed from the main thread via Bevy's systems.
// The plugin state is owned by this wrapper and properly deallocated on drop.
unsafe impl Send for FfiPluginWrapper {}
unsafe impl Sync for FfiPluginWrapper {}

impl FfiPluginWrapper {
    pub fn new(export: PluginExport) -> Self {
        // Get manifest via FFI
        let ffi_manifest = unsafe { (export.vtable.manifest)(export.handle) };
        let manifest = unsafe { ffi_manifest.into_manifest() };

        Self {
            handle: export.handle,
            vtable: export.vtable,
            manifest,
            enabled: true,
        }
    }

    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Create a HostApi struct with callbacks pointing to host functions
    fn create_host_api(api: &mut EditorApiImpl) -> HostApi {
        HostApi {
            ctx: api as *mut EditorApiImpl as *mut c_void,
            log_info: host_log_info,
            log_warn: host_log_warn,
            log_error: host_log_error,
            set_status_item: host_set_status_item,
            remove_status_item: host_remove_status_item,
            undo: host_undo,
            redo: host_redo,
            can_undo: host_can_undo,
            can_redo: host_can_redo,
            // Panel system
            register_panel: host_register_panel,
            unregister_panel: host_unregister_panel,
            set_panel_content: host_set_panel_content,
            set_panel_visible: host_set_panel_visible,
            is_panel_visible: host_is_panel_visible,
            // Entity operations
            get_entity_by_name: host_get_entity_by_name,
            get_entity_transform: host_get_entity_transform,
            set_entity_transform: host_set_entity_transform,
            get_entity_name: host_get_entity_name,
            set_entity_name: host_set_entity_name,
            get_entity_visible: host_get_entity_visible,
            set_entity_visible: host_set_entity_visible,
            spawn_entity: host_spawn_entity,
            despawn_entity: host_despawn_entity,
            get_entity_parent: host_get_entity_parent,
            get_entity_children: host_get_entity_children,
            reparent_entity: host_reparent_entity,
            // Selection
            get_selected_entity: host_get_selected_entity,
            set_selected_entity: host_set_selected_entity,
            clear_selection: host_clear_selection,
            // Toolbar
            add_toolbar_button: host_add_toolbar_button,
            remove_toolbar_item: host_remove_toolbar_item,
            // Menu
            add_menu_item: host_add_menu_item,
            remove_menu_item: host_remove_menu_item,
            set_menu_item_enabled: host_set_menu_item_enabled,
            set_menu_item_checked: host_set_menu_item_checked,
            // Tabs
            register_tab: host_register_tab,
            unregister_tab: host_unregister_tab,
            set_tab_content: host_set_tab_content,
            set_active_tab: host_set_active_tab,
            get_active_tab: host_get_active_tab,
            // Asset system - READ
            get_asset_list: host_get_asset_list,
            get_asset_info: host_get_asset_info,
            asset_exists: host_asset_exists,
            read_asset_text: host_read_asset_text,
            read_asset_bytes: host_read_asset_bytes,
            // Asset system - WRITE
            write_asset_text: host_write_asset_text,
            write_asset_bytes: host_write_asset_bytes,
            create_folder: host_create_folder,
            delete_asset: host_delete_asset,
            rename_asset: host_rename_asset,
            // Pub/Sub
            subscribe_event: host_subscribe_event,
            unsubscribe_event: host_unsubscribe_event,
            publish_event: host_publish_event,
        }
    }

    /// Call on_load via FFI
    pub fn on_load(&mut self, api: &mut EditorApiImpl) -> Result<(), PluginError> {
        let host_api = Self::create_host_api(api);
        let host_api_ptr = &host_api as *const HostApi as *mut c_void;
        let result = unsafe { (self.vtable.on_load)(self.handle, host_api_ptr) };
        if result.success {
            Ok(())
        } else {
            let msg = unsafe { result.error_message.into_string() };
            Err(PluginError::InitFailed(msg))
        }
    }

    /// Call on_unload via FFI
    pub fn on_unload(&mut self, api: &mut EditorApiImpl) {
        let host_api = Self::create_host_api(api);
        let host_api_ptr = &host_api as *const HostApi as *mut c_void;
        unsafe { (self.vtable.on_unload)(self.handle, host_api_ptr) };
    }

    /// Call on_update via FFI
    pub fn on_update(&mut self, api: &mut EditorApiImpl, dt: f32) {
        let host_api = Self::create_host_api(api);
        let host_api_ptr = &host_api as *const HostApi as *mut c_void;
        unsafe { (self.vtable.on_update)(self.handle, host_api_ptr, dt) };
    }

    /// Call on_event via FFI (events passed as JSON)
    pub fn on_event(&mut self, api: &mut EditorApiImpl, event_json: &str) {
        let host_api = Self::create_host_api(api);
        let host_api_ptr = &host_api as *const HostApi as *mut c_void;
        let c_json = std::ffi::CString::new(event_json).unwrap_or_default();
        unsafe { (self.vtable.on_event)(self.handle, host_api_ptr, c_json.as_ptr()) };
    }
}

impl Drop for FfiPluginWrapper {
    fn drop(&mut self) {
        // Call destroy to properly deallocate the plugin
        unsafe { (self.vtable.destroy)(self.handle) };
    }
}

/// The plugin host manages the lifecycle of all loaded plugins.
#[derive(Resource)]
/// Whether a plugin was loaded from the system or project directory
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginSource {
    /// Shipped with the editor (from the system plugins directory)
    System,
    /// Installed per-project (from the project's plugins directory)
    Project,
}

/// Info about a plugin that has been disabled (unloaded) but can be re-enabled
#[derive(Clone)]
pub struct DisabledPlugin {
    pub manifest: PluginManifest,
    pub path: PathBuf,
    pub source: PluginSource,
}

#[derive(Resource)]
pub struct PluginHost {
    /// Directory to scan for project plugins
    plugin_dir: PathBuf,
    /// Directory for system plugins (next to the editor executable)
    system_plugin_dir: PathBuf,
    /// Loaded plugin libraries (kept alive to prevent unloading)
    libraries: Vec<Library>,
    /// Plugin instances (FFI-safe wrappers)
    plugins: HashMap<String, FfiPluginWrapper>,
    /// Map from plugin ID to the file path it was loaded from
    plugin_paths: HashMap<String, PathBuf>,
    /// Map from plugin ID to its source (system or project)
    plugin_sources: HashMap<String, PluginSource>,
    /// Plugins that were disabled at runtime (unloaded but remembered for re-enabling)
    disabled_plugins: HashMap<String, DisabledPlugin>,
    /// Plugin IDs the user has persistently disabled (loaded from config)
    user_disabled_ids: Vec<String>,
    /// API implementation shared with plugins
    api: EditorApiImpl,
    /// Pending events to dispatch
    pending_events: Vec<EditorEvent>,
    /// File watcher for hot reload (wrapped in Mutex for Sync)
    watcher: Option<Mutex<RecommendedWatcher>>,
    /// Receiver for file system events (wrapped in Mutex for Sync)
    watcher_rx: Option<Mutex<Receiver<Result<Event, notify::Error>>>>,
}

impl Default for PluginHost {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginHost {
    /// Create a new plugin host with the default plugins directory
    pub fn new() -> Self {
        let plugin_dir = std::env::current_dir()
            .unwrap_or_default()
            .join("plugins");

        // System plugins live next to the editor executable
        let system_plugin_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("plugins")))
            .unwrap_or_else(|| PathBuf::from("plugins"));

        Self {
            plugin_dir,
            system_plugin_dir,
            libraries: Vec::new(),
            plugins: HashMap::new(),
            plugin_paths: HashMap::new(),
            plugin_sources: HashMap::new(),
            disabled_plugins: HashMap::new(),
            user_disabled_ids: Vec::new(),
            api: EditorApiImpl::new(),
            pending_events: Vec::new(),
            watcher: None,
            watcher_rx: None,
        }
    }

    /// Create a plugin host with a custom plugin directory
    pub fn with_plugin_dir(plugin_dir: PathBuf) -> Self {
        Self {
            plugin_dir,
            ..Default::default()
        }
    }

    /// Start watching the plugin directory for changes
    pub fn start_watching(&mut self) {
        if self.watcher.is_some() {
            return; // Already watching
        }

        let (tx, rx) = channel();

        match RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default(),
        ) {
            Ok(mut watcher) => {
                if self.plugin_dir.exists() {
                    if let Err(e) = watcher.watch(&self.plugin_dir, RecursiveMode::NonRecursive) {
                        warn!("Failed to watch plugin directory: {}", e);
                        return;
                    }
                    info!("Watching plugin directory: {}", self.plugin_dir.display());
                    self.watcher = Some(Mutex::new(watcher));
                    self.watcher_rx = Some(Mutex::new(rx));
                }
            }
            Err(e) => {
                warn!("Failed to create file watcher: {}", e);
            }
        }
    }

    /// Stop watching the plugin directory
    pub fn stop_watching(&mut self) {
        self.watcher = None;
        self.watcher_rx = None;
    }

    /// Check for file system changes and hot reload plugins
    pub fn check_for_changes(&mut self) {
        let Some(rx_mutex) = &self.watcher_rx else {
            return;
        };

        let Ok(rx) = rx_mutex.lock() else {
            return;
        };

        let extension = if cfg!(windows) {
            "dll"
        } else if cfg!(target_os = "macos") {
            "dylib"
        } else {
            "so"
        };

        // Collect all events
        let mut created_files = Vec::new();
        let mut removed_files = Vec::new();

        while let Ok(result) = rx.try_recv() {
            if let Ok(event) = result {
                for path in event.paths {
                    if path.extension() != Some(OsStr::new(extension)) {
                        continue;
                    }

                    match event.kind {
                        notify::EventKind::Create(_) => {
                            created_files.push(path);
                        }
                        notify::EventKind::Remove(_) => {
                            removed_files.push(path);
                        }
                        notify::EventKind::Modify(_) => {
                            // For modifications, we'll treat it as remove + create
                            // But on Windows we can't reload while loaded, so just log
                            info!("Plugin modified: {} (restart to reload)", path.display());
                        }
                        _ => {}
                    }
                }
            }
        }

        // Drop the lock before modifying self
        drop(rx);

        // Handle removed plugins
        for path in removed_files {
            // Find plugin ID by path
            let plugin_id = self
                .plugin_paths
                .iter()
                .find(|(_, p)| **p == path)
                .map(|(id, _)| id.clone());

            if let Some(id) = plugin_id {
                info!("Plugin file removed, unloading: {}", id);
                let _ = self.unload_plugin(&id);
            }
        }

        // Handle new plugins
        for path in created_files {
            // Check if already loaded
            if self.plugin_paths.values().any(|p| *p == path) {
                continue;
            }

            info!("New plugin detected: {}", path.display());
            match self.load_plugin(&path) {
                Ok(id) => info!("Hot loaded plugin: {}", id),
                Err(e) => error!("Failed to hot load plugin: {}", e),
            }
        }
    }

    /// Get the plugin directory
    pub fn plugin_dir(&self) -> &PathBuf {
        &self.plugin_dir
    }

    /// Set the plugin directory
    pub fn set_plugin_dir(&mut self, dir: PathBuf) {
        self.plugin_dir = dir;
    }

    /// Get the system plugin directory
    pub fn system_plugin_dir(&self) -> &PathBuf {
        &self.system_plugin_dir
    }

    /// Set the list of plugin IDs the user has disabled (loaded from config)
    pub fn set_user_disabled_ids(&mut self, ids: Vec<String>) {
        self.user_disabled_ids = ids;
    }

    /// Get the source of a loaded plugin
    pub fn plugin_source(&self, plugin_id: &str) -> Option<PluginSource> {
        self.plugin_sources.get(plugin_id).copied()
    }

    /// Discover available plugins in the project plugin directory
    pub fn discover_plugins(&self) -> Vec<PathBuf> {
        self.discover_plugins_in(&self.plugin_dir)
    }

    /// Probe a plugin library to get its manifest without fully loading it
    pub fn probe_plugin(&self, path: &PathBuf) -> Result<PluginManifest, PluginError> {
        let file_name = path.file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Try to load the library
        let library = match unsafe { Library::new(path) } {
            Ok(lib) => lib,
            Err(e) => {
                let err_msg = format!("Failed to load DLL: {}", e);
                console_log(LogLevel::Error, "Plugins", format!("[{}] {}", file_name, err_msg));
                return Err(PluginError::LoadFailed(err_msg));
            }
        };

        unsafe {
            // Try to get the create_plugin symbol
            let create_fn: libloading::Symbol<CreatePluginFn> = match library.get(b"create_plugin") {
                Ok(f) => f,
                Err(e) => {
                    let err_msg = format!("Not a valid plugin (missing create_plugin): {}", e);
                    console_log(LogLevel::Error, "Plugins", format!("[{}] {}", file_name, err_msg));
                    return Err(PluginError::LoadFailed(err_msg));
                }
            };

            // Call create_plugin with panic catching
            let export = match catch_unwind(AssertUnwindSafe(|| create_fn())) {
                Ok(exp) => exp,
                Err(_) => {
                    let err_msg = "Plugin crashed during initialization";
                    console_log(LogLevel::Error, "Plugins", format!("[{}] {}", file_name, err_msg));
                    return Err(PluginError::LoadFailed(err_msg.to_string()));
                }
            };

            // Check FFI version
            if export.ffi_version != FFI_API_VERSION {
                let err_msg = format!(
                    "FFI version mismatch: plugin uses v{}, editor expects v{}",
                    export.ffi_version, FFI_API_VERSION
                );
                console_log(LogLevel::Error, "Plugins", format!("[{}] {}", file_name, err_msg));
                (export.vtable.destroy)(export.handle);
                return Err(PluginError::ApiVersionMismatch {
                    required: export.ffi_version,
                    available: FFI_API_VERSION,
                });
            }

            // Get manifest with panic catching
            let manifest = match catch_unwind(AssertUnwindSafe(|| {
                let ffi_manifest = (export.vtable.manifest)(export.handle);
                ffi_manifest.into_manifest()
            })) {
                Ok(m) => m,
                Err(_) => {
                    let err_msg = "Plugin crashed while reading manifest";
                    console_log(LogLevel::Error, "Plugins", format!("[{}] {}", file_name, err_msg));
                    (export.vtable.destroy)(export.handle);
                    return Err(PluginError::LoadFailed(err_msg.to_string()));
                }
            };

            // Destroy the plugin (we're just probing)
            let _ = catch_unwind(AssertUnwindSafe(|| {
                (export.vtable.destroy)(export.handle);
            }));

            Ok(manifest)
        }
    }

    /// Discover and load all plugins from a directory with the given source tag
    fn discover_and_load_from_dir(&mut self, dir: &PathBuf, source: PluginSource) -> Result<(), PluginError> {
        let source_label = match source {
            PluginSource::System => "system",
            PluginSource::Project => "project",
        };

        console_log(LogLevel::Info, "Plugins", format!("Scanning {} plugins: {}", source_label, dir.display()));

        // Ensure plugin directory exists
        if !dir.exists() {
            if source == PluginSource::Project {
                console_log(LogLevel::Info, "Plugins", "Creating project plugins directory");
                std::fs::create_dir_all(dir).ok();
            }
            return Ok(());
        }

        // Discover plugin files
        let plugin_paths = self.discover_plugins_in(dir);
        if plugin_paths.is_empty() {
            console_log(LogLevel::Info, "Plugins", format!("No {} plugins found", source_label));
            return Ok(());
        }

        console_log(LogLevel::Info, "Plugins", format!("Found {} {} DLL file(s)", plugin_paths.len(), source_label));

        // Probe all plugins to get manifests
        let mut manifests = Vec::new();
        let mut path_map = HashMap::new();
        let mut failed_count = 0;

        for path in &plugin_paths {
            // Skip if this exact file is already loaded
            if self.plugin_paths.values().any(|p| *p == *path) {
                continue;
            }

            // Skip if a DLL with the same file name is already loaded from another dir
            // (prevents crash from loading the same DLL twice on Windows)
            let file_name = path.file_name().unwrap_or_default();
            if self.plugin_paths.values().any(|p| p.file_name().unwrap_or_default() == file_name) {
                console_log(LogLevel::Info, "Plugins",
                    format!("Skipping {} {} (same file already loaded)", source_label, file_name.to_string_lossy()));
                continue;
            }

            match self.probe_plugin(path) {
                Ok(manifest) => {
                    // Skip if a plugin with this ID is already loaded
                    if self.plugins.contains_key(&manifest.id) {
                        console_log(LogLevel::Info, "Plugins",
                            format!("Skipping {} plugin {} (already loaded)", source_label, manifest.name));
                        continue;
                    }
                    console_log(LogLevel::Info, "Plugins",
                        format!("Detected: {} v{} ({})", manifest.name, manifest.version, manifest.id));
                    path_map.insert(manifest.id.clone(), path.clone());
                    manifests.push(manifest);
                }
                Err(_) => {
                    failed_count += 1;
                }
            }
        }

        if manifests.is_empty() {
            if failed_count > 0 {
                console_log(LogLevel::Warning, "Plugins",
                    format!("All {} {} plugin(s) failed to load", failed_count, source_label));
            }
            return Ok(());
        }

        // Build dependency graph and resolve load order
        let graph = DependencyGraph::from_manifests(&manifests);
        let load_order = match graph.topological_sort() {
            Ok(order) => order,
            Err(e) => {
                console_log(LogLevel::Error, "Plugins",
                    format!("Dependency resolution failed: {}", e));
                return Err(e);
            }
        };

        // Build manifest lookup for disabled plugin tracking
        let manifest_map: HashMap<String, PluginManifest> = manifests.iter()
            .map(|m| (m.id.clone(), m.clone()))
            .collect();

        // Load plugins in dependency order (skip user-disabled ones)
        let mut loaded_count = 0;
        for plugin_id in &load_order {
            if let Some(path) = path_map.get(plugin_id) {
                // Skip plugins the user has disabled, but remember them
                if self.user_disabled_ids.contains(plugin_id) {
                    if let Some(manifest) = manifest_map.get(plugin_id) {
                        console_log(LogLevel::Info, "Plugins",
                            format!("Skipping disabled plugin: {}", manifest.name));
                        self.disabled_plugins.insert(plugin_id.clone(), DisabledPlugin {
                            manifest: manifest.clone(),
                            path: path.clone(),
                            source,
                        });
                    }
                    continue;
                }

                match self.load_plugin(path) {
                    Ok(id) => {
                        self.plugin_sources.insert(id, source);
                        loaded_count += 1;
                    }
                    Err(_) => {}
                }
            }
        }

        if loaded_count > 0 {
            console_log(LogLevel::Success, "Plugins",
                format!("Successfully loaded {} {} plugin(s)", loaded_count, source_label));
        }
        if failed_count > 0 {
            console_log(LogLevel::Warning, "Plugins",
                format!("{} {} plugin(s) failed to load", failed_count, source_label));
        }

        Ok(())
    }

    /// Discover and load system plugins (shipped with the editor)
    pub fn discover_and_load_system_plugins(&mut self) -> Result<(), PluginError> {
        let dir = self.system_plugin_dir.clone();
        self.discover_and_load_from_dir(&dir, PluginSource::System)
    }

    /// Discover and load all project plugins in the plugin directory
    pub fn discover_and_load_plugins(&mut self) -> Result<(), PluginError> {
        let dir = self.plugin_dir.clone();
        let result = self.discover_and_load_from_dir(&dir, PluginSource::Project);

        // Start watching for hot reload
        self.start_watching();

        result
    }

    /// Discover plugin files in a specific directory
    fn discover_plugins_in(&self, dir: &PathBuf) -> Vec<PathBuf> {
        let mut plugin_paths = Vec::new();
        let extension = if cfg!(windows) { "dll" } else if cfg!(target_os = "macos") { "dylib" } else { "so" };

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension() == Some(OsStr::new(extension)) {
                    plugin_paths.push(path);
                }
            }
        }

        plugin_paths
    }

    /// Load a single plugin from a library path
    pub fn load_plugin(&mut self, path: &PathBuf) -> Result<String, PluginError> {
        let file_name = path.file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        console_log(LogLevel::Info, "Plugins", format!("Loading: {}", file_name));

        // Load library
        let library = match unsafe { Library::new(path) } {
            Ok(lib) => lib,
            Err(e) => {
                let err_msg = format!("Failed to load DLL: {}", e);
                console_log(LogLevel::Error, "Plugins", format!("[{}] {}", file_name, err_msg));
                return Err(PluginError::LoadFailed(err_msg));
            }
        };

        unsafe {
            // Get create_plugin function
            let create_fn: libloading::Symbol<CreatePluginFn> = match library.get(b"create_plugin") {
                Ok(f) => f,
                Err(e) => {
                    let err_msg = format!("Invalid plugin (no create_plugin): {}", e);
                    console_log(LogLevel::Error, "Plugins", format!("[{}] {}", file_name, err_msg));
                    return Err(PluginError::LoadFailed(err_msg));
                }
            };

            // Create plugin with panic handling
            let export = match catch_unwind(AssertUnwindSafe(|| create_fn())) {
                Ok(exp) => exp,
                Err(_) => {
                    let err_msg = "Plugin crashed during creation";
                    console_log(LogLevel::Error, "Plugins", format!("[{}] {}", file_name, err_msg));
                    return Err(PluginError::LoadFailed(err_msg.to_string()));
                }
            };

            // Check FFI version
            if export.ffi_version != FFI_API_VERSION {
                let err_msg = format!(
                    "Incompatible FFI version (plugin: v{}, editor: v{})",
                    export.ffi_version, FFI_API_VERSION
                );
                console_log(LogLevel::Error, "Plugins", format!("[{}] {}", file_name, err_msg));
                let _ = catch_unwind(AssertUnwindSafe(|| (export.vtable.destroy)(export.handle)));
                return Err(PluginError::ApiVersionMismatch {
                    required: export.ffi_version,
                    available: FFI_API_VERSION,
                });
            }

            // Create the wrapper (this also gets the manifest)
            let mut wrapper = match catch_unwind(AssertUnwindSafe(|| FfiPluginWrapper::new(export))) {
                Ok(w) => w,
                Err(_) => {
                    let err_msg = "Plugin crashed while reading manifest";
                    console_log(LogLevel::Error, "Plugins", format!("[{}] {}", file_name, err_msg));
                    return Err(PluginError::LoadFailed(err_msg.to_string()));
                }
            };

            let manifest = wrapper.manifest().clone();

            // Check API version
            if manifest.min_api_version > EDITOR_API_VERSION {
                let err_msg = format!(
                    "Incompatible API version (requires: v{}, editor: v{})",
                    manifest.min_api_version, EDITOR_API_VERSION
                );
                console_log(LogLevel::Error, "Plugins", format!("[{}] {}", manifest.name, err_msg));
                return Err(PluginError::ApiVersionMismatch {
                    required: manifest.min_api_version,
                    available: EDITOR_API_VERSION,
                });
            }

            let plugin_id = manifest.id.clone();
            let plugin_name = manifest.name.clone();
            let plugin_version = manifest.version.clone();

            // Reject if a plugin with this ID is already loaded
            if self.plugins.contains_key(&plugin_id) {
                console_log(LogLevel::Warning, "Plugins",
                    format!("[{}] Skipping duplicate plugin ID '{}'", plugin_name, plugin_id));
                return Err(PluginError::LoadFailed(format!("Plugin '{}' is already loaded", plugin_id)));
            }

            // Set current plugin for API tracking
            self.api.set_current_plugin(Some(plugin_id.clone()));

            // Initialize the plugin via FFI with panic handling
            let load_result = catch_unwind(AssertUnwindSafe(|| {
                wrapper.on_load(&mut self.api)
            }));

            self.api.set_current_plugin(None);

            match load_result {
                Ok(Ok(())) => {
                    // Success
                }
                Ok(Err(e)) => {
                    let err_msg = format!("Initialization failed: {}", e);
                    console_log(LogLevel::Error, "Plugins", format!("[{}] {}", plugin_name, err_msg));
                    return Err(e);
                }
                Err(_) => {
                    let err_msg = "Plugin crashed during initialization";
                    console_log(LogLevel::Error, "Plugins", format!("[{}] {}", plugin_name, err_msg));
                    return Err(PluginError::LoadFailed(err_msg.to_string()));
                }
            }

            // Store the library to keep it loaded
            self.libraries.push(library);

            // Store the plugin wrapper and path
            self.plugins.insert(plugin_id.clone(), wrapper);
            self.plugin_paths.insert(plugin_id.clone(), path.clone());

            console_log(LogLevel::Success, "Plugins",
                format!("Loaded: {} v{}", plugin_name, plugin_version));
            Ok(plugin_id)
        }
    }

    /// Unload a plugin by ID
    pub fn unload_plugin(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        if let Some(mut wrapper) = self.plugins.remove(plugin_id) {
            // Set current plugin for API tracking
            self.api.set_current_plugin(Some(plugin_id.to_string()));
            wrapper.on_unload(&mut self.api);
            self.api.set_current_plugin(None);

            // Remove all UI elements registered by this plugin
            self.api.remove_plugin_elements(plugin_id);

            self.plugin_paths.remove(plugin_id);
            self.plugin_sources.remove(plugin_id);
            // wrapper is dropped here, which calls destroy via FFI
            info!("Plugin unloaded: {}", plugin_id);
            Ok(())
        } else {
            Err(PluginError::NotFound(plugin_id.to_string()))
        }
    }

    /// Unload only project plugins (called when project changes, keeps system plugins)
    pub fn unload_project_plugins(&mut self) {
        // Stop watching project dir
        self.stop_watching();

        let project_ids: Vec<_> = self.plugin_sources.iter()
            .filter(|(_, source)| **source == PluginSource::Project)
            .map(|(id, _)| id.clone())
            .collect();

        for plugin_id in project_ids {
            if let Some(mut wrapper) = self.plugins.remove(&plugin_id) {
                self.api.set_current_plugin(Some(plugin_id.clone()));
                wrapper.on_unload(&mut self.api);
                self.api.set_current_plugin(None);
                self.api.remove_plugin_elements(&plugin_id);
                info!("Project plugin unloaded: {}", plugin_id);
            }
            self.plugin_paths.remove(&plugin_id);
            self.plugin_sources.remove(&plugin_id);
        }

        // Also clear disabled project plugins
        self.disabled_plugins.retain(|_, d| d.source != PluginSource::Project);

        info!("Project plugins unloaded");
    }

    /// Unload all plugins (system and project)
    pub fn unload_all_plugins(&mut self) {
        // Stop watching first
        self.stop_watching();

        let plugin_ids: Vec<_> = self.plugins.keys().cloned().collect();
        for plugin_id in plugin_ids {
            if let Some(mut wrapper) = self.plugins.remove(&plugin_id) {
                wrapper.on_unload(&mut self.api);
                // wrapper is dropped here, which calls destroy via FFI
                info!("Plugin unloaded: {}", plugin_id);
            }
        }
        // Clear libraries to unload the DLLs
        self.libraries.clear();
        // Clear plugin paths, sources, and disabled list
        self.plugin_paths.clear();
        self.plugin_sources.clear();
        self.disabled_plugins.clear();
        // Clear API state
        self.api.clear();
        info!("All plugins unloaded");
    }

    /// Update all loaded plugins (called every frame)
    pub fn update(&mut self, dt: f32) {
        // Get plugin IDs for iteration (need to avoid borrow issues)
        let plugin_ids: Vec<_> = self.plugins.keys().cloned().collect();

        // Dispatch pending editor events
        let events: Vec<_> = self.pending_events.drain(..).collect();
        for event in &events {
            let event_json = serialize_editor_event(event);
            let event_type = get_event_type(event);

            for plugin_id in &plugin_ids {
                // Check if plugin is subscribed to this event type
                if !self.api.is_subscribed(plugin_id, &event_type) {
                    continue;
                }

                if let Some(wrapper) = self.plugins.get_mut(plugin_id) {
                    if wrapper.is_enabled() {
                        self.api.set_current_plugin(Some(plugin_id.clone()));
                        wrapper.on_event(&mut self.api, &event_json);
                    }
                }
            }
        }

        // Dispatch pending UI events
        let ui_events: Vec<_> = self.api.pending_ui_events.drain(..).collect();
        for ui_event in ui_events {
            let event_json = serialize_ui_event(&ui_event);
            let event_type = get_ui_event_type(&ui_event);

            for plugin_id in &plugin_ids {
                // Check if plugin is subscribed to UI events
                if !self.api.is_subscribed(plugin_id, &event_type)
                    && !self.api.is_subscribed(plugin_id, "ui.*")
                {
                    continue;
                }

                if let Some(wrapper) = self.plugins.get_mut(plugin_id) {
                    if wrapper.is_enabled() {
                        self.api.set_current_plugin(Some(plugin_id.clone()));
                        wrapper.on_event(&mut self.api, &event_json);
                    }
                }
            }
        }

        // Dispatch published events from plugins (pub/sub)
        let published: Vec<_> = self.api.take_published_events();
        for (event_type, data_json, source_plugin) in published {
            let event_json = serde_json::json!({
                "type": "plugin",
                "event_type": event_type,
                "source": source_plugin,
                "data": serde_json::from_str::<serde_json::Value>(&data_json).unwrap_or(serde_json::Value::Null)
            }).to_string();

            for plugin_id in &plugin_ids {
                // Don't send to self
                if plugin_id == &source_plugin {
                    continue;
                }

                // Check subscriptions
                if !self.api.is_subscribed(plugin_id, &event_type)
                    && !self.api.is_subscribed(plugin_id, "plugin.*")
                    && !self.api.is_subscribed(plugin_id, &format!("plugin.{}.*", source_plugin))
                {
                    continue;
                }

                if let Some(wrapper) = self.plugins.get_mut(plugin_id) {
                    if wrapper.is_enabled() {
                        self.api.set_current_plugin(Some(plugin_id.clone()));
                        wrapper.on_event(&mut self.api, &event_json);
                    }
                }
            }
        }

        // Call update on all plugins
        for plugin_id in &plugin_ids {
            if let Some(wrapper) = self.plugins.get_mut(plugin_id) {
                if wrapper.is_enabled() {
                    self.api.set_current_plugin(Some(plugin_id.clone()));
                    wrapper.on_update(&mut self.api, dt);
                }
            }
        }

        self.api.set_current_plugin(None);
    }

    /// Update all plugins with direct World access (called every frame)
    /// Note: Direct World access is disabled in FFI mode for safety.
    /// Plugins should use the EditorApi for entity operations instead.
    pub fn update_with_world(&mut self, _world: &mut World) {
        // Direct World access is not safe across FFI boundaries
        // Plugins should use EditorApi methods instead
    }

    /// Queue an event to be dispatched to plugins
    pub fn queue_event(&mut self, event: EditorEvent) {
        self.pending_events.push(event);
    }

    /// Get the number of loaded plugins
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }

    /// Get list of loaded plugin manifests
    pub fn loaded_plugins(&self) -> Vec<PluginManifest> {
        self.plugins.values().map(|w| w.manifest().clone()).collect()
    }

    /// Enable or disable a plugin
    pub fn set_plugin_enabled(&mut self, plugin_id: &str, enabled: bool) -> Result<(), PluginError> {
        if let Some(wrapper) = self.plugins.get_mut(plugin_id) {
            wrapper.set_enabled(enabled);
            Ok(())
        } else {
            Err(PluginError::NotFound(plugin_id.to_string()))
        }
    }

    /// Check if a plugin is loaded
    pub fn is_plugin_loaded(&self, plugin_id: &str) -> bool {
        self.plugins.contains_key(plugin_id)
    }

    /// Check if a plugin is enabled (loaded and active)
    pub fn is_plugin_enabled(&self, plugin_id: &str) -> bool {
        self.plugins.contains_key(plugin_id)
    }

    /// Disable a plugin: fully unloads it but remembers it so it can be re-enabled
    pub fn disable_plugin(&mut self, plugin_id: &str) {
        // Save info before unloading
        let path = self.plugin_paths.get(plugin_id).cloned();
        let source = self.plugin_sources.get(plugin_id).copied();
        let manifest = self.plugins.get(plugin_id).map(|w| w.manifest().clone());

        if let (Some(path), Some(source), Some(manifest)) = (path, source, manifest) {
            self.disabled_plugins.insert(plugin_id.to_string(), DisabledPlugin {
                manifest,
                path,
                source,
            });
        }

        let _ = self.unload_plugin(plugin_id);
    }

    /// Re-enable a previously disabled plugin by reloading it
    pub fn enable_plugin(&mut self, plugin_id: &str) {
        if let Some(disabled) = self.disabled_plugins.remove(plugin_id) {
            match self.load_plugin(&disabled.path) {
                Ok(id) => {
                    self.plugin_sources.insert(id, disabled.source);
                }
                Err(e) => {
                    error!("Failed to re-enable plugin {}: {}", plugin_id, e);
                    // Put it back in disabled list
                    self.disabled_plugins.insert(plugin_id.to_string(), disabled);
                }
            }
        }
    }

    /// Get all known plugins: loaded + disabled, with their info
    pub fn all_plugins(&self) -> Vec<(PluginManifest, PluginSource, bool)> {
        let mut result = Vec::new();

        // Loaded plugins
        for (id, wrapper) in &self.plugins {
            let source = self.plugin_sources.get(id).copied().unwrap_or(PluginSource::Project);
            result.push((wrapper.manifest().clone(), source, true));
        }

        // Disabled plugins
        for (_id, disabled) in &self.disabled_plugins {
            result.push((disabled.manifest.clone(), disabled.source, false));
        }

        result
    }

    /// Get the disabled plugins map
    pub fn disabled_plugins(&self) -> &HashMap<String, DisabledPlugin> {
        &self.disabled_plugins
    }

    /// Get the API implementation (for internal use)
    pub fn api(&self) -> &EditorApiImpl {
        &self.api
    }

    /// Get mutable API implementation (for internal use)
    pub fn api_mut(&mut self) -> &mut EditorApiImpl {
        &mut self.api
    }
}
