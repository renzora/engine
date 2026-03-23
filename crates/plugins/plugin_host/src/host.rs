//! Plugin host for discovering, loading, and managing plugins.

#![allow(dead_code)]

use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, CString, OsStr};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;

use bevy::prelude::*;
use libloading::Library;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

use super::abi::{EntityId, PluginTransform};
use super::api::{
    EditorApiImpl, MenuLocation, MenuItem, PendingOperation, PanelDefinition, PanelLocation,
    StatusBarAlign, StatusBarItem, TabLocation, ToolbarItem,
};
use super::dependency::DependencyGraph;
use editor_plugin_api::abi::{PluginError, PluginManifest, EDITOR_API_VERSION};
use editor_plugin_api::events::{EditorEvent, UiEvent};
use editor_plugin_api::ffi::{
    FfiAssetInfo, FfiAssetList, FfiAssetType, FfiBytes, FfiEntityId, FfiEntityList, FfiMenuItem,
    FfiOwnedString, FfiPanelDefinition, FfiPanelLocation, FfiStatusBarItem, FfiTabDefinition,
    FfiTabLocation, FfiTransform, HostApi, PluginExport, PluginHandle, PluginVTable,
    FFI_API_VERSION,
};
use editor_plugin_api::ui::UiId;
use renzora_core::console_log::{console_log, LogLevel};

use super::api::PluginTab;

/// Type for the FFI create_plugin function.
type CreatePluginFn = unsafe extern "C" fn() -> PluginExport;

// ============================================================================
// Host callback implementations - called by plugins via FFI
// ============================================================================

unsafe extern "C" fn host_log_info(_ctx: *mut c_void, message: *const c_char) {
    if message.is_null() {
        return;
    }
    let msg = CStr::from_ptr(message).to_string_lossy();
    info!("[Plugin] {}", msg);
}

unsafe extern "C" fn host_log_warn(_ctx: *mut c_void, message: *const c_char) {
    if message.is_null() {
        return;
    }
    let msg = CStr::from_ptr(message).to_string_lossy();
    warn!("[Plugin] {}", msg);
}

unsafe extern "C" fn host_log_error(_ctx: *mut c_void, message: *const c_char) {
    if message.is_null() {
        return;
    }
    let msg = CStr::from_ptr(message).to_string_lossy();
    error!("[Plugin] {}", msg);
}

unsafe extern "C" fn host_set_status_item(ctx: *mut c_void, item: *const FfiStatusBarItem) {
    if ctx.is_null() || item.is_null() {
        return;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let ffi_item = &*item;

    let id = ffi_item.id.to_string();
    let text = ffi_item.text.to_string();
    let icon = if ffi_item.icon.ptr.is_null() {
        None
    } else {
        Some(ffi_item.icon.to_string())
    };
    let tooltip = if ffi_item.tooltip.ptr.is_null() {
        None
    } else {
        Some(ffi_item.tooltip.to_string())
    };

    let status_item = StatusBarItem {
        id: id.clone(),
        icon,
        text,
        tooltip,
        align: if ffi_item.align_right {
            StatusBarAlign::Right
        } else {
            StatusBarAlign::Left
        },
        priority: ffi_item.priority,
    };

    let plugin_id = api_impl.current_plugin_id.clone().unwrap_or_default();
    api_impl.status_bar_items.insert(id, (status_item, plugin_id));
}

unsafe extern "C" fn host_remove_status_item(ctx: *mut c_void, id: *const c_char) {
    if ctx.is_null() || id.is_null() {
        return;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let id_str = CStr::from_ptr(id).to_string_lossy().into_owned();
    api_impl.status_bar_items.remove(&id_str);
}

unsafe extern "C" fn host_undo(ctx: *mut c_void) -> bool {
    if ctx.is_null() {
        return false;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    if api_impl.can_undo {
        api_impl.pending_undo = 1;
        true
    } else {
        false
    }
}

unsafe extern "C" fn host_redo(ctx: *mut c_void) -> bool {
    if ctx.is_null() {
        return false;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    if api_impl.can_redo {
        api_impl.pending_redo = 1;
        true
    } else {
        false
    }
}

unsafe extern "C" fn host_can_undo(ctx: *mut c_void) -> bool {
    if ctx.is_null() {
        return false;
    }
    let api_impl = &*(ctx as *const EditorApiImpl);
    api_impl.can_undo
}

unsafe extern "C" fn host_can_redo(ctx: *mut c_void) -> bool {
    if ctx.is_null() {
        return false;
    }
    let api_impl = &*(ctx as *const EditorApiImpl);
    api_impl.can_redo
}

// ============================================================================
// Panel callbacks
// ============================================================================

unsafe extern "C" fn host_register_panel(
    ctx: *mut c_void,
    panel: *const FfiPanelDefinition,
) -> bool {
    if ctx.is_null() || panel.is_null() {
        return false;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let ffi_panel = &*panel;

    let id = ffi_panel.id.to_string();
    let title = ffi_panel.title.to_string();
    let icon = if ffi_panel.icon.ptr.is_null() {
        None
    } else {
        Some(ffi_panel.icon.to_string())
    };

    let location = match ffi_panel.location {
        FfiPanelLocation::Left => PanelLocation::Left,
        FfiPanelLocation::Right => PanelLocation::Right,
        FfiPanelLocation::Bottom => PanelLocation::Bottom,
        FfiPanelLocation::Floating => PanelLocation::Floating,
    };

    let panel_def = PanelDefinition {
        id: id.clone(),
        title,
        icon,
        default_location: location,
        min_size: [ffi_panel.min_width, ffi_panel.min_height],
        closable: ffi_panel.closable,
    };

    if api_impl.panels.iter().any(|(p, _)| p.id == id) {
        warn!("Panel '{}' already registered", id);
        return false;
    }

    let plugin_id = api_impl.current_plugin_id.clone().unwrap_or_default();
    api_impl.panels.push((panel_def, plugin_id));
    api_impl.panel_visible.insert(id.clone(), true);
    info!("Panel '{}' registered successfully", id);
    true
}

unsafe extern "C" fn host_unregister_panel(ctx: *mut c_void, id: *const c_char) {
    if ctx.is_null() || id.is_null() {
        return;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let id_str = CStr::from_ptr(id).to_string_lossy().into_owned();

    api_impl.panels.retain(|(p, _)| p.id != id_str);
    api_impl.panel_contents.remove(&id_str);
    api_impl.panel_visible.remove(&id_str);
}

unsafe extern "C" fn host_set_panel_content(
    ctx: *mut c_void,
    panel_id: *const c_char,
    widgets_json: *const c_char,
) {
    if ctx.is_null() || panel_id.is_null() || widgets_json.is_null() {
        return;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let id_str = CStr::from_ptr(panel_id).to_string_lossy().into_owned();
    let json_str = CStr::from_ptr(widgets_json).to_string_lossy().into_owned();

    // Store as raw JSON — rendering layer will parse
    api_impl.panel_contents.insert(id_str, json_str);
}

unsafe extern "C" fn host_set_panel_visible(
    ctx: *mut c_void,
    panel_id: *const c_char,
    visible: bool,
) {
    if ctx.is_null() || panel_id.is_null() {
        return;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let id_str = CStr::from_ptr(panel_id).to_string_lossy().into_owned();
    api_impl.panel_visible.insert(id_str, visible);
}

unsafe extern "C" fn host_is_panel_visible(ctx: *mut c_void, panel_id: *const c_char) -> bool {
    if ctx.is_null() || panel_id.is_null() {
        return false;
    }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let id_str = CStr::from_ptr(panel_id).to_string_lossy();
    api_impl
        .panel_visible
        .get(id_str.as_ref())
        .copied()
        .unwrap_or(false)
}

// ============================================================================
// Entity operation callbacks
// ============================================================================

unsafe extern "C" fn host_get_entity_by_name(
    ctx: *mut c_void,
    name: *const c_char,
) -> FfiEntityId {
    if ctx.is_null() || name.is_null() {
        return FfiEntityId::INVALID;
    }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let name_str = CStr::from_ptr(name).to_string_lossy();

    api_impl
        .get_entity_by_name(&name_str)
        .map(|id| FfiEntityId(id.0))
        .unwrap_or(FfiEntityId::INVALID)
}

unsafe extern "C" fn host_get_entity_transform(
    ctx: *mut c_void,
    entity: FfiEntityId,
) -> FfiTransform {
    if ctx.is_null() || !entity.is_valid() {
        return FfiTransform::default();
    }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let entity_id = EntityId(entity.0);

    api_impl
        .entity_transforms
        .get(&entity_id)
        .map(|t| FfiTransform {
            translation: t.translation,
            rotation: t.rotation,
            scale: t.scale,
        })
        .unwrap_or_default()
}

unsafe extern "C" fn host_set_entity_transform(
    ctx: *mut c_void,
    entity: FfiEntityId,
    transform: *const FfiTransform,
) {
    if ctx.is_null() || !entity.is_valid() || transform.is_null() {
        return;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let entity_id = EntityId(entity.0);
    let t = &*transform;

    let plugin_transform = PluginTransform {
        translation: t.translation,
        rotation: t.rotation,
        scale: t.scale,
    };

    api_impl
        .pending_operations
        .push(PendingOperation::SetTransform {
            entity: entity_id,
            transform: plugin_transform,
        });
}

unsafe extern "C" fn host_get_entity_name(
    ctx: *mut c_void,
    entity: FfiEntityId,
) -> FfiOwnedString {
    if ctx.is_null() || !entity.is_valid() {
        return FfiOwnedString::empty();
    }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let entity_id = EntityId(entity.0);

    api_impl
        .entity_names
        .get(&entity_id)
        .map(|n| FfiOwnedString::from_string(n.clone()))
        .unwrap_or_else(FfiOwnedString::empty)
}

unsafe extern "C" fn host_set_entity_name(
    ctx: *mut c_void,
    entity: FfiEntityId,
    name: *const c_char,
) {
    if ctx.is_null() || !entity.is_valid() || name.is_null() {
        return;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let entity_id = EntityId(entity.0);
    let name_str = CStr::from_ptr(name).to_string_lossy().into_owned();

    api_impl
        .pending_operations
        .push(PendingOperation::SetEntityName {
            entity: entity_id,
            name: name_str,
        });
}

unsafe extern "C" fn host_get_entity_visible(ctx: *mut c_void, entity: FfiEntityId) -> bool {
    if ctx.is_null() || !entity.is_valid() {
        return false;
    }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let entity_id = EntityId(entity.0);
    api_impl
        .entity_visibility
        .get(&entity_id)
        .copied()
        .unwrap_or(true)
}

unsafe extern "C" fn host_set_entity_visible(
    ctx: *mut c_void,
    entity: FfiEntityId,
    visible: bool,
) {
    if ctx.is_null() || !entity.is_valid() {
        return;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let entity_id = EntityId(entity.0);

    api_impl
        .pending_operations
        .push(PendingOperation::SetEntityVisible {
            entity: entity_id,
            visible,
        });
}

unsafe extern "C" fn host_spawn_entity(
    ctx: *mut c_void,
    name: *const c_char,
    transform: *const FfiTransform,
) -> FfiEntityId {
    if ctx.is_null() || name.is_null() || transform.is_null() {
        return FfiEntityId::INVALID;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let name_str = CStr::from_ptr(name).to_string_lossy().into_owned();
    let t = &*transform;

    let def = editor_plugin_api::api::EntityDefinition {
        name: name_str,
        node_type: String::new(),
        transform: PluginTransform {
            translation: t.translation,
            rotation: t.rotation,
            scale: t.scale,
        },
        parent: None,
    };

    api_impl
        .pending_operations
        .push(PendingOperation::SpawnEntity(def));
    FfiEntityId::INVALID
}

unsafe extern "C" fn host_despawn_entity(ctx: *mut c_void, entity: FfiEntityId) {
    if ctx.is_null() || !entity.is_valid() {
        return;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let entity_id = EntityId(entity.0);
    api_impl
        .pending_operations
        .push(PendingOperation::DespawnEntity(entity_id));
}

unsafe extern "C" fn host_get_entity_parent(ctx: *mut c_void, entity: FfiEntityId) -> FfiEntityId {
    if ctx.is_null() || !entity.is_valid() {
        return FfiEntityId::INVALID;
    }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let entity_id = EntityId(entity.0);

    api_impl
        .entity_parents
        .get(&entity_id)
        .and_then(|opt| *opt)
        .map(|id| FfiEntityId(id.0))
        .unwrap_or(FfiEntityId::INVALID)
}

unsafe extern "C" fn host_get_entity_children(
    ctx: *mut c_void,
    entity: FfiEntityId,
) -> FfiEntityList {
    if ctx.is_null() || !entity.is_valid() {
        return FfiEntityList::empty();
    }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let entity_id = EntityId(entity.0);

    api_impl
        .entity_children
        .get(&entity_id)
        .map(|children| {
            let ffi_children: Vec<FfiEntityId> =
                children.iter().map(|id| FfiEntityId(id.0)).collect();
            FfiEntityList::from_vec(ffi_children)
        })
        .unwrap_or_else(FfiEntityList::empty)
}

unsafe extern "C" fn host_reparent_entity(
    ctx: *mut c_void,
    entity: FfiEntityId,
    new_parent: FfiEntityId,
) {
    if ctx.is_null() || !entity.is_valid() {
        return;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let entity_id = EntityId(entity.0);
    let parent_id = if new_parent.is_valid() {
        Some(EntityId(new_parent.0))
    } else {
        None
    };

    api_impl
        .pending_operations
        .push(PendingOperation::ReparentEntity {
            entity: entity_id,
            new_parent: parent_id,
        });
}

// ============================================================================
// Selection callbacks
// ============================================================================

unsafe extern "C" fn host_get_selected_entity(ctx: *mut c_void) -> FfiEntityId {
    if ctx.is_null() {
        return FfiEntityId::INVALID;
    }
    let api_impl = &*(ctx as *const EditorApiImpl);
    api_impl
        .selected_entity
        .map(|id| FfiEntityId(id.0))
        .unwrap_or(FfiEntityId::INVALID)
}

unsafe extern "C" fn host_set_selected_entity(ctx: *mut c_void, entity: FfiEntityId) {
    if ctx.is_null() {
        return;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let entity_id = if entity.is_valid() {
        Some(EntityId(entity.0))
    } else {
        None
    };
    api_impl
        .pending_operations
        .push(PendingOperation::SetSelectedEntity(entity_id));
}

unsafe extern "C" fn host_clear_selection(ctx: *mut c_void) {
    if ctx.is_null() {
        return;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    api_impl
        .pending_operations
        .push(PendingOperation::SetSelectedEntity(None));
}

// ============================================================================
// Toolbar callbacks
// ============================================================================

unsafe extern "C" fn host_add_toolbar_button(
    ctx: *mut c_void,
    id: u64,
    icon: *const c_char,
    tooltip: *const c_char,
) {
    if ctx.is_null() || icon.is_null() || tooltip.is_null() {
        return;
    }
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
}

unsafe extern "C" fn host_remove_toolbar_item(ctx: *mut c_void, id: u64) {
    if ctx.is_null() {
        return;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    api_impl.toolbar_items.retain(|(item, _)| item.id.0 != id);
}

// ============================================================================
// Menu callbacks
// ============================================================================

unsafe extern "C" fn host_add_menu_item(ctx: *mut c_void, menu: u8, item: *const FfiMenuItem) {
    if ctx.is_null() || item.is_null() {
        return;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let ffi_item = &*item;

    let location = match menu {
        0 => MenuLocation::File,
        1 => MenuLocation::Edit,
        2 => MenuLocation::View,
        3 => MenuLocation::Scene,
        4 => MenuLocation::Tools,
        5 => MenuLocation::Help,
        _ => MenuLocation::Tools,
    };

    if ffi_item.is_separator {
        return;
    }

    let label = ffi_item.label.to_string();
    let shortcut = if ffi_item.shortcut.ptr.is_null() {
        None
    } else {
        Some(ffi_item.shortcut.to_string())
    };
    let icon = if ffi_item.icon.ptr.is_null() {
        None
    } else {
        Some(ffi_item.icon.to_string())
    };

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
    api_impl.menu_items.push((location, menu_item, plugin_id));
}

unsafe extern "C" fn host_remove_menu_item(ctx: *mut c_void, id: u64) {
    if ctx.is_null() {
        return;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    api_impl.menu_items.retain(|(_, item, _)| item.id.0 != id);
}

unsafe extern "C" fn host_set_menu_item_enabled(ctx: *mut c_void, id: u64, enabled: bool) {
    if ctx.is_null() {
        return;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    for (_, item, _) in &mut api_impl.menu_items {
        if item.id.0 == id {
            item.enabled = enabled;
            break;
        }
    }
}

unsafe extern "C" fn host_set_menu_item_checked(_ctx: *mut c_void, _id: u64, _checked: bool) {
    // MenuItem doesn't have a checked field currently
}

// ============================================================================
// Tab callbacks (docked tabs in panel areas)
// ============================================================================

unsafe extern "C" fn host_register_tab(ctx: *mut c_void, tab: *const FfiTabDefinition) -> bool {
    if ctx.is_null() || tab.is_null() {
        return false;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let ffi_tab = &*tab;

    let id = ffi_tab.id.to_string();
    let title = ffi_tab.title.to_string();
    let icon = if ffi_tab.icon.ptr.is_null() {
        None
    } else {
        Some(ffi_tab.icon.to_string())
    };

    let location = match ffi_tab.location {
        FfiTabLocation::Left => TabLocation::Left,
        FfiTabLocation::Right => TabLocation::Right,
        FfiTabLocation::Bottom => TabLocation::Bottom,
    };

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
    api_impl.tabs.push((plugin_tab, plugin_id));
    true
}

unsafe extern "C" fn host_unregister_tab(ctx: *mut c_void, id: *const c_char) {
    if ctx.is_null() || id.is_null() {
        return;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let id_str = CStr::from_ptr(id).to_string_lossy().into_owned();

    api_impl.tabs.retain(|(t, _)| t.id != id_str);
    api_impl.tab_contents.remove(&id_str);
}

unsafe extern "C" fn host_set_tab_content(
    ctx: *mut c_void,
    tab_id: *const c_char,
    widgets_json: *const c_char,
) {
    if ctx.is_null() || tab_id.is_null() || widgets_json.is_null() {
        return;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let id_str = CStr::from_ptr(tab_id).to_string_lossy().into_owned();
    let json_str = CStr::from_ptr(widgets_json).to_string_lossy().into_owned();

    api_impl.tab_contents.insert(id_str, json_str);
}

unsafe extern "C" fn host_set_active_tab(ctx: *mut c_void, location: u8, tab_id: *const c_char) {
    if ctx.is_null() || tab_id.is_null() {
        return;
    }
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
    if ctx.is_null() {
        return FfiOwnedString::empty();
    }
    let api_impl = &*(ctx as *const EditorApiImpl);

    let loc = match location {
        0 => TabLocation::Left,
        1 => TabLocation::Right,
        2 => TabLocation::Bottom,
        _ => return FfiOwnedString::empty(),
    };

    api_impl
        .get_active_tab(loc)
        .map(|s| FfiOwnedString::from_string(s.to_string()))
        .unwrap_or_else(FfiOwnedString::empty)
}

// ============================================================================
// Asset system callbacks
// ============================================================================

/// Validate an asset path — returns None if path is invalid (security check).
fn validate_asset_path(path: &str) -> Option<std::path::PathBuf> {
    if path.is_empty() {
        return Some(std::path::PathBuf::new());
    }
    if path.starts_with('/') || path.starts_with('\\') {
        return None;
    }
    if path.len() >= 2 && path.chars().nth(1) == Some(':') {
        return None;
    }
    if path.contains("..") {
        return None;
    }
    let normalized = path.replace('\\', "/");
    Some(std::path::PathBuf::from(normalized))
}

unsafe extern "C" fn host_get_asset_list(ctx: *mut c_void, folder: *const c_char) -> FfiAssetList {
    if ctx.is_null() {
        return FfiAssetList::empty();
    }
    let api_impl = &*(ctx as *const EditorApiImpl);

    let folder_str = if folder.is_null() {
        String::new()
    } else {
        CStr::from_ptr(folder).to_string_lossy().into_owned()
    };

    let Some(rel_path) = validate_asset_path(&folder_str) else {
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
                let ext = entry_path
                    .extension()
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
    if ctx.is_null() || path.is_null() {
        return FfiAssetInfo::empty();
    }
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
    let file_name = full_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    let asset_type = if is_dir {
        FfiAssetType::Folder
    } else {
        let ext = full_path
            .extension()
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
    if ctx.is_null() || path.is_null() {
        return false;
    }
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

unsafe extern "C" fn host_read_asset_text(
    ctx: *mut c_void,
    path: *const c_char,
) -> FfiOwnedString {
    if ctx.is_null() || path.is_null() {
        return FfiOwnedString::empty();
    }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let path_str = CStr::from_ptr(path).to_string_lossy().into_owned();

    let Some(rel_path) = validate_asset_path(&path_str) else {
        return FfiOwnedString::empty();
    };
    let Some(assets_path) = api_impl.get_project_assets_path() else {
        return FfiOwnedString::empty();
    };

    match std::fs::read_to_string(assets_path.join(&rel_path)) {
        Ok(content) => FfiOwnedString::from_string(content),
        Err(_) => FfiOwnedString::empty(),
    }
}

unsafe extern "C" fn host_read_asset_bytes(ctx: *mut c_void, path: *const c_char) -> FfiBytes {
    if ctx.is_null() || path.is_null() {
        return FfiBytes::empty();
    }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let path_str = CStr::from_ptr(path).to_string_lossy().into_owned();

    let Some(rel_path) = validate_asset_path(&path_str) else {
        return FfiBytes::empty();
    };
    let Some(assets_path) = api_impl.get_project_assets_path() else {
        return FfiBytes::empty();
    };

    match std::fs::read(assets_path.join(&rel_path)) {
        Ok(content) => FfiBytes::from_vec(content),
        Err(_) => FfiBytes::empty(),
    }
}

unsafe extern "C" fn host_write_asset_text(
    ctx: *mut c_void,
    path: *const c_char,
    content: *const c_char,
) -> bool {
    if ctx.is_null() || path.is_null() || content.is_null() {
        return false;
    }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let path_str = CStr::from_ptr(path).to_string_lossy().into_owned();
    let content_str = CStr::from_ptr(content).to_string_lossy().into_owned();

    let Some(rel_path) = validate_asset_path(&path_str) else {
        return false;
    };
    let Some(assets_path) = api_impl.get_project_assets_path() else {
        return false;
    };

    let full_path = assets_path.join(&rel_path);
    if let Some(parent) = full_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    std::fs::write(&full_path, content_str).is_ok()
}

unsafe extern "C" fn host_write_asset_bytes(
    ctx: *mut c_void,
    path: *const c_char,
    data: *const FfiBytes,
) -> bool {
    if ctx.is_null() || path.is_null() || data.is_null() {
        return false;
    }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let path_str = CStr::from_ptr(path).to_string_lossy().into_owned();
    let ffi_bytes = &*data;

    let Some(rel_path) = validate_asset_path(&path_str) else {
        return false;
    };
    let Some(assets_path) = api_impl.get_project_assets_path() else {
        return false;
    };

    let full_path = assets_path.join(&rel_path);
    if let Some(parent) = full_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let bytes = ffi_bytes.as_slice();
    std::fs::write(&full_path, bytes).is_ok()
}

unsafe extern "C" fn host_create_folder(ctx: *mut c_void, path: *const c_char) -> bool {
    if ctx.is_null() || path.is_null() {
        return false;
    }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let path_str = CStr::from_ptr(path).to_string_lossy().into_owned();

    let Some(rel_path) = validate_asset_path(&path_str) else {
        return false;
    };
    let Some(assets_path) = api_impl.get_project_assets_path() else {
        return false;
    };

    std::fs::create_dir_all(assets_path.join(&rel_path)).is_ok()
}

unsafe extern "C" fn host_delete_asset(ctx: *mut c_void, path: *const c_char) -> bool {
    if ctx.is_null() || path.is_null() {
        return false;
    }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let path_str = CStr::from_ptr(path).to_string_lossy().into_owned();

    let Some(rel_path) = validate_asset_path(&path_str) else {
        return false;
    };
    if rel_path.as_os_str().is_empty() {
        return false;
    }
    let Some(assets_path) = api_impl.get_project_assets_path() else {
        return false;
    };

    let full_path = assets_path.join(&rel_path);
    if !full_path.exists() {
        return false;
    }

    if full_path.is_dir() {
        std::fs::remove_dir_all(&full_path).is_ok()
    } else {
        std::fs::remove_file(&full_path).is_ok()
    }
}

unsafe extern "C" fn host_rename_asset(
    ctx: *mut c_void,
    old_path: *const c_char,
    new_path: *const c_char,
) -> bool {
    if ctx.is_null() || old_path.is_null() || new_path.is_null() {
        return false;
    }
    let api_impl = &*(ctx as *const EditorApiImpl);
    let old_str = CStr::from_ptr(old_path).to_string_lossy().into_owned();
    let new_str = CStr::from_ptr(new_path).to_string_lossy().into_owned();

    let Some(old_rel) = validate_asset_path(&old_str) else {
        return false;
    };
    let Some(new_rel) = validate_asset_path(&new_str) else {
        return false;
    };
    if old_rel.as_os_str().is_empty() {
        return false;
    }
    let Some(assets_path) = api_impl.get_project_assets_path() else {
        return false;
    };

    let old_full = assets_path.join(&old_rel);
    let new_full = assets_path.join(&new_rel);

    if !old_full.exists() {
        return false;
    }
    if let Some(parent) = new_full.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    std::fs::rename(&old_full, &new_full).is_ok()
}

// ============================================================================
// Pub/Sub callbacks
// ============================================================================

unsafe extern "C" fn host_subscribe_event(ctx: *mut c_void, event_type: *const c_char) {
    if ctx.is_null() || event_type.is_null() {
        return;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let event_str = CStr::from_ptr(event_type).to_string_lossy().into_owned();

    let plugin_id = api_impl.current_plugin_id.clone();
    if let Some(plugin_id) = plugin_id {
        api_impl.subscribe_plugin(&plugin_id, &event_str);
    }
}

unsafe extern "C" fn host_unsubscribe_event(ctx: *mut c_void, event_type: *const c_char) {
    if ctx.is_null() || event_type.is_null() {
        return;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let event_str = CStr::from_ptr(event_type).to_string_lossy().into_owned();

    let plugin_id = api_impl.current_plugin_id.clone();
    if let Some(plugin_id) = plugin_id {
        api_impl.unsubscribe_plugin(&plugin_id, &event_str);
    }
}

unsafe extern "C" fn host_publish_event(
    ctx: *mut c_void,
    event_type: *const c_char,
    data_json: *const c_char,
) {
    if ctx.is_null() || event_type.is_null() {
        return;
    }
    let api_impl = &mut *(ctx as *mut EditorApiImpl);
    let event_str = CStr::from_ptr(event_type).to_string_lossy().into_owned();
    let data_str = if data_json.is_null() {
        "{}".to_string()
    } else {
        CStr::from_ptr(data_json).to_string_lossy().into_owned()
    };

    let plugin_id = api_impl.current_plugin_id.clone();
    if let Some(plugin_id) = plugin_id {
        api_impl.publish_event(&plugin_id, &event_str, &data_str);
    }
}

// ============================================================================
// Event serialization helpers
// ============================================================================

fn serialize_ui_event(event: &UiEvent) -> String {
    let json = match event {
        UiEvent::ButtonClicked(id) => serde_json::json!({"type":"ui","event":"button_clicked","id":id.0}),
        UiEvent::CheckboxToggled { id, checked } => serde_json::json!({"type":"ui","event":"checkbox_toggled","id":id.0,"checked":checked}),
        UiEvent::SliderChanged { id, value } => serde_json::json!({"type":"ui","event":"slider_changed","id":id.0,"value":value}),
        UiEvent::SliderIntChanged { id, value } => serde_json::json!({"type":"ui","event":"slider_int_changed","id":id.0,"value":value}),
        UiEvent::TextInputChanged { id, value } => serde_json::json!({"type":"ui","event":"text_input_changed","id":id.0,"value":value}),
        UiEvent::TextInputSubmitted { id, value } => serde_json::json!({"type":"ui","event":"text_input_submitted","id":id.0,"value":value}),
        UiEvent::DropdownSelected { id, index } => serde_json::json!({"type":"ui","event":"dropdown_selected","id":id.0,"index":index}),
        UiEvent::ColorChanged { id, color } => serde_json::json!({"type":"ui","event":"color_changed","id":id.0,"color":color}),
        UiEvent::TreeNodeToggled { id, expanded } => serde_json::json!({"type":"ui","event":"tree_node_toggled","id":id.0,"expanded":expanded}),
        UiEvent::TreeNodeSelected(id) => serde_json::json!({"type":"ui","event":"tree_node_selected","id":id.0}),
        UiEvent::TabSelected { id, index } => serde_json::json!({"type":"ui","event":"tab_selected","id":id.0,"index":index}),
        UiEvent::TabClosed { id, index } => serde_json::json!({"type":"ui","event":"tab_closed","id":id.0,"index":index}),
        UiEvent::TableRowSelected { id, row } => serde_json::json!({"type":"ui","event":"table_row_selected","id":id.0,"row":row}),
        UiEvent::TableSortChanged { id, column, ascending } => serde_json::json!({"type":"ui","event":"table_sort_changed","id":id.0,"column":column,"ascending":ascending}),
        UiEvent::CustomEvent { type_id, data } => serde_json::json!({"type":"ui","event":"custom","type_id":type_id,"data":data}),
    };
    json.to_string()
}

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

fn serialize_editor_event(event: &EditorEvent) -> String {
    let json = match event {
        EditorEvent::EntitySelected(id) => serde_json::json!({"type":"editor","event":"entity_selected","entity_id":id.0}),
        EditorEvent::EntityDeselected(id) => serde_json::json!({"type":"editor","event":"entity_deselected","entity_id":id.0}),
        EditorEvent::SceneLoaded { path } => serde_json::json!({"type":"editor","event":"scene_loaded","path":path}),
        EditorEvent::SceneSaved { path } => serde_json::json!({"type":"editor","event":"scene_saved","path":path}),
        EditorEvent::PlayStarted => serde_json::json!({"type":"editor","event":"play_started"}),
        EditorEvent::PlayStopped => serde_json::json!({"type":"editor","event":"play_stopped"}),
        EditorEvent::ProjectOpened { path } => serde_json::json!({"type":"editor","event":"project_opened","path":path}),
        EditorEvent::ProjectClosed => serde_json::json!({"type":"editor","event":"project_closed"}),
        EditorEvent::UiEvent(ui_event) => return serialize_ui_event(ui_event),
        EditorEvent::CustomEvent { plugin_id, event_type, data } => serde_json::json!({"type":"plugin","event":"custom","source":plugin_id,"event_type":event_type,"data":data}),
    };
    json.to_string()
}

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

// ============================================================================
// FFI Plugin Wrapper
// ============================================================================

/// FFI-safe wrapper for a loaded plugin.
pub struct FfiPluginWrapper {
    handle: PluginHandle,
    vtable: PluginVTable,
    manifest: PluginManifest,
    enabled: bool,
}

unsafe impl Send for FfiPluginWrapper {}
unsafe impl Sync for FfiPluginWrapper {}

impl FfiPluginWrapper {
    pub fn new(export: PluginExport) -> Self {
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

    /// Create a HostApi struct with callbacks pointing to host functions.
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
            register_panel: host_register_panel,
            unregister_panel: host_unregister_panel,
            set_panel_content: host_set_panel_content,
            set_panel_visible: host_set_panel_visible,
            is_panel_visible: host_is_panel_visible,
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
            get_selected_entity: host_get_selected_entity,
            set_selected_entity: host_set_selected_entity,
            clear_selection: host_clear_selection,
            add_toolbar_button: host_add_toolbar_button,
            remove_toolbar_item: host_remove_toolbar_item,
            add_menu_item: host_add_menu_item,
            remove_menu_item: host_remove_menu_item,
            set_menu_item_enabled: host_set_menu_item_enabled,
            set_menu_item_checked: host_set_menu_item_checked,
            register_tab: host_register_tab,
            unregister_tab: host_unregister_tab,
            set_tab_content: host_set_tab_content,
            set_active_tab: host_set_active_tab,
            get_active_tab: host_get_active_tab,
            get_asset_list: host_get_asset_list,
            get_asset_info: host_get_asset_info,
            asset_exists: host_asset_exists,
            read_asset_text: host_read_asset_text,
            read_asset_bytes: host_read_asset_bytes,
            write_asset_text: host_write_asset_text,
            write_asset_bytes: host_write_asset_bytes,
            create_folder: host_create_folder,
            delete_asset: host_delete_asset,
            rename_asset: host_rename_asset,
            subscribe_event: host_subscribe_event,
            unsubscribe_event: host_unsubscribe_event,
            publish_event: host_publish_event,
        }
    }

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

    pub fn on_unload(&mut self, api: &mut EditorApiImpl) {
        let host_api = Self::create_host_api(api);
        let host_api_ptr = &host_api as *const HostApi as *mut c_void;
        unsafe { (self.vtable.on_unload)(self.handle, host_api_ptr) };
    }

    pub fn on_update(&mut self, api: &mut EditorApiImpl, dt: f32) {
        let host_api = Self::create_host_api(api);
        let host_api_ptr = &host_api as *const HostApi as *mut c_void;
        unsafe { (self.vtable.on_update)(self.handle, host_api_ptr, dt) };
    }

    pub fn on_event(&mut self, api: &mut EditorApiImpl, event_json: &str) {
        let host_api = Self::create_host_api(api);
        let host_api_ptr = &host_api as *const HostApi as *mut c_void;
        let c_json = CString::new(event_json).unwrap_or_default();
        unsafe { (self.vtable.on_event)(self.handle, host_api_ptr, c_json.as_ptr()) };
    }
}

impl Drop for FfiPluginWrapper {
    fn drop(&mut self) {
        unsafe { (self.vtable.destroy)(self.handle) };
    }
}

// ============================================================================
// Plugin Host
// ============================================================================

/// Whether a plugin was loaded from the system or project directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginSource {
    System,
    Project,
}

/// Info about a plugin that has been disabled but can be re-enabled.
#[derive(Clone)]
pub struct DisabledPlugin {
    pub manifest: PluginManifest,
    pub path: PathBuf,
    pub source: PluginSource,
}

/// The plugin host manages the lifecycle of all loaded plugins.
#[derive(Resource)]
pub struct PluginHost {
    plugin_dir: PathBuf,
    system_plugin_dir: PathBuf,
    libraries: Vec<Library>,
    plugins: HashMap<String, FfiPluginWrapper>,
    plugin_paths: HashMap<String, PathBuf>,
    plugin_sources: HashMap<String, PluginSource>,
    disabled_plugins: HashMap<String, DisabledPlugin>,
    user_disabled_ids: Vec<String>,
    api: EditorApiImpl,
    pending_events: Vec<EditorEvent>,
    watcher: Option<Mutex<RecommendedWatcher>>,
    watcher_rx: Option<Mutex<Receiver<Result<Event, notify::Error>>>>,
}

impl Default for PluginHost {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginHost {
    pub fn new() -> Self {
        let plugin_dir = std::env::current_dir()
            .unwrap_or_default()
            .join("plugins");

        let system_plugin_dir = std::env::current_dir()
            .unwrap_or_default()
            .join("plugins");

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

    /// Start watching the plugin directory for changes.
    pub fn start_watching(&mut self) {
        if self.watcher.is_some() {
            return;
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
                    if let Err(e) =
                        watcher.watch(&self.plugin_dir, RecursiveMode::NonRecursive)
                    {
                        warn!("Failed to watch plugin directory: {}", e);
                        return;
                    }
                    info!(
                        "Watching plugin directory: {}",
                        self.plugin_dir.display()
                    );
                    self.watcher = Some(Mutex::new(watcher));
                    self.watcher_rx = Some(Mutex::new(rx));
                }
            }
            Err(e) => {
                warn!("Failed to create file watcher: {}", e);
            }
        }
    }

    /// Stop watching the plugin directory.
    pub fn stop_watching(&mut self) {
        self.watcher = None;
        self.watcher_rx = None;
    }

    /// Check for file system changes and hot reload plugins.
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
                            info!(
                                "Plugin modified: {} (restart to reload)",
                                path.display()
                            );
                        }
                        _ => {}
                    }
                }
            }
        }

        drop(rx);

        for path in removed_files {
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

        for path in created_files {
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

    pub fn plugin_dir(&self) -> &PathBuf {
        &self.plugin_dir
    }

    pub fn set_plugin_dir(&mut self, dir: PathBuf) {
        self.plugin_dir = dir;
    }

    pub fn system_plugin_dir(&self) -> &PathBuf {
        &self.system_plugin_dir
    }

    pub fn set_user_disabled_ids(&mut self, ids: Vec<String>) {
        self.user_disabled_ids = ids;
    }

    pub fn plugin_source(&self, plugin_id: &str) -> Option<PluginSource> {
        self.plugin_sources.get(plugin_id).copied()
    }

    /// Discover plugin files in a specific directory.
    fn discover_plugins_in(&self, dir: &PathBuf) -> Vec<PathBuf> {
        let mut plugin_paths = Vec::new();
        let extension = if cfg!(windows) {
            "dll"
        } else if cfg!(target_os = "macos") {
            "dylib"
        } else {
            "so"
        };

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

    /// Probe a plugin library to get its manifest without fully loading it.
    pub fn probe_plugin(&self, path: &PathBuf) -> Result<PluginManifest, PluginError> {
        let file_name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let library = match unsafe { Library::new(path) } {
            Ok(lib) => lib,
            Err(e) => {
                let err_msg = format!("Failed to load DLL: {}", e);
                console_log(
                    LogLevel::Error,
                    "Plugins",
                    format!("[{}] {}", file_name, err_msg),
                );
                return Err(PluginError::LoadFailed(err_msg));
            }
        };

        unsafe {
            let create_fn: libloading::Symbol<CreatePluginFn> = match library.get(b"create_plugin")
            {
                Ok(f) => f,
                Err(e) => {
                    let err_msg =
                        format!("Not a valid plugin (missing create_plugin): {}", e);
                    console_log(
                        LogLevel::Error,
                        "Plugins",
                        format!("[{}] {}", file_name, err_msg),
                    );
                    return Err(PluginError::LoadFailed(err_msg));
                }
            };

            let export = match catch_unwind(AssertUnwindSafe(|| create_fn())) {
                Ok(exp) => exp,
                Err(_) => {
                    let err_msg = "Plugin crashed during initialization";
                    console_log(
                        LogLevel::Error,
                        "Plugins",
                        format!("[{}] {}", file_name, err_msg),
                    );
                    return Err(PluginError::LoadFailed(err_msg.to_string()));
                }
            };

            if export.ffi_version != FFI_API_VERSION {
                let err_msg = format!(
                    "FFI version mismatch: plugin uses v{}, editor expects v{}",
                    export.ffi_version, FFI_API_VERSION
                );
                console_log(
                    LogLevel::Error,
                    "Plugins",
                    format!("[{}] {}", file_name, err_msg),
                );
                (export.vtable.destroy)(export.handle);
                return Err(PluginError::ApiVersionMismatch {
                    required: export.ffi_version,
                    available: FFI_API_VERSION,
                });
            }

            let manifest = match catch_unwind(AssertUnwindSafe(|| {
                let ffi_manifest = (export.vtable.manifest)(export.handle);
                ffi_manifest.into_manifest()
            })) {
                Ok(m) => m,
                Err(_) => {
                    let err_msg = "Plugin crashed while reading manifest";
                    console_log(
                        LogLevel::Error,
                        "Plugins",
                        format!("[{}] {}", file_name, err_msg),
                    );
                    (export.vtable.destroy)(export.handle);
                    return Err(PluginError::LoadFailed(err_msg.to_string()));
                }
            };

            let _ = catch_unwind(AssertUnwindSafe(|| {
                (export.vtable.destroy)(export.handle);
            }));

            Ok(manifest)
        }
    }

    /// Discover and load all plugins from a directory with the given source tag.
    fn discover_and_load_from_dir(
        &mut self,
        dir: &PathBuf,
        source: PluginSource,
    ) -> Result<(), PluginError> {
        let source_label = match source {
            PluginSource::System => "system",
            PluginSource::Project => "project",
        };

        console_log(
            LogLevel::Info,
            "Plugins",
            format!("Scanning {} plugins: {}", source_label, dir.display()),
        );

        if !dir.exists() {
            if source == PluginSource::Project {
                std::fs::create_dir_all(dir).ok();
            }
            return Ok(());
        }

        let plugin_paths = self.discover_plugins_in(dir);
        if plugin_paths.is_empty() {
            console_log(
                LogLevel::Info,
                "Plugins",
                format!("No {} plugins found", source_label),
            );
            return Ok(());
        }

        console_log(
            LogLevel::Info,
            "Plugins",
            format!(
                "Found {} {} DLL file(s)",
                plugin_paths.len(),
                source_label
            ),
        );

        // Probe all plugins to get manifests
        let mut manifests = Vec::new();
        let mut path_map = HashMap::new();
        let mut failed_count = 0;

        for path in &plugin_paths {
            if self.plugin_paths.values().any(|p| *p == *path) {
                continue;
            }
            let file_name = path.file_name().unwrap_or_default();
            if self
                .plugin_paths
                .values()
                .any(|p| p.file_name().unwrap_or_default() == file_name)
            {
                continue;
            }

            match self.probe_plugin(path) {
                Ok(manifest) => {
                    if self.plugins.contains_key(&manifest.id) {
                        continue;
                    }
                    console_log(
                        LogLevel::Info,
                        "Plugins",
                        format!(
                            "Detected: {} v{} ({})",
                            manifest.name, manifest.version, manifest.id
                        ),
                    );
                    path_map.insert(manifest.id.clone(), path.clone());
                    manifests.push(manifest);
                }
                Err(_) => {
                    failed_count += 1;
                }
            }
        }

        if manifests.is_empty() {
            return Ok(());
        }

        // Build dependency graph and resolve load order
        let graph = DependencyGraph::from_manifests(&manifests);
        let load_order = match graph.topological_sort() {
            Ok(order) => order,
            Err(e) => {
                console_log(
                    LogLevel::Error,
                    "Plugins",
                    format!("Dependency resolution failed: {}", e),
                );
                return Err(e);
            }
        };

        let manifest_map: HashMap<String, PluginManifest> =
            manifests.iter().map(|m| (m.id.clone(), m.clone())).collect();

        // Load plugins in dependency order
        let mut loaded_count = 0;
        for plugin_id in &load_order {
            if let Some(path) = path_map.get(plugin_id) {
                if self.user_disabled_ids.contains(plugin_id) {
                    if let Some(manifest) = manifest_map.get(plugin_id) {
                        console_log(
                            LogLevel::Info,
                            "Plugins",
                            format!("Skipping disabled plugin: {}", manifest.name),
                        );
                        self.disabled_plugins.insert(
                            plugin_id.clone(),
                            DisabledPlugin {
                                manifest: manifest.clone(),
                                path: path.clone(),
                                source,
                            },
                        );
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
            console_log(
                LogLevel::Success,
                "Plugins",
                format!(
                    "Successfully loaded {} {} plugin(s)",
                    loaded_count, source_label
                ),
            );
        }
        if failed_count > 0 {
            console_log(
                LogLevel::Warning,
                "Plugins",
                format!(
                    "{} {} plugin(s) failed to load",
                    failed_count, source_label
                ),
            );
        }

        Ok(())
    }

    /// Discover and load system plugins (shipped with the editor).
    pub fn discover_and_load_system_plugins(&mut self) -> Result<(), PluginError> {
        let dir = self.system_plugin_dir.clone();
        self.discover_and_load_from_dir(&dir, PluginSource::System)
    }

    /// Discover and load all project plugins in the plugin directory.
    pub fn discover_and_load_plugins(&mut self) -> Result<(), PluginError> {
        let dir = self.plugin_dir.clone();
        let result = self.discover_and_load_from_dir(&dir, PluginSource::Project);
        self.start_watching();
        result
    }

    /// Load a single plugin from a library path.
    pub fn load_plugin(&mut self, path: &PathBuf) -> Result<String, PluginError> {
        let file_name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        console_log(
            LogLevel::Info,
            "Plugins",
            format!("Loading: {}", file_name),
        );

        let library = match unsafe { Library::new(path) } {
            Ok(lib) => lib,
            Err(e) => {
                let err_msg = format!("Failed to load DLL: {}", e);
                console_log(
                    LogLevel::Error,
                    "Plugins",
                    format!("[{}] {}", file_name, err_msg),
                );
                return Err(PluginError::LoadFailed(err_msg));
            }
        };

        unsafe {
            let create_fn: libloading::Symbol<CreatePluginFn> = match library.get(b"create_plugin")
            {
                Ok(f) => f,
                Err(e) => {
                    let err_msg = format!("Invalid plugin (no create_plugin): {}", e);
                    console_log(
                        LogLevel::Error,
                        "Plugins",
                        format!("[{}] {}", file_name, err_msg),
                    );
                    return Err(PluginError::LoadFailed(err_msg));
                }
            };

            let export = match catch_unwind(AssertUnwindSafe(|| create_fn())) {
                Ok(exp) => exp,
                Err(_) => {
                    let err_msg = "Plugin crashed during creation";
                    console_log(
                        LogLevel::Error,
                        "Plugins",
                        format!("[{}] {}", file_name, err_msg),
                    );
                    return Err(PluginError::LoadFailed(err_msg.to_string()));
                }
            };

            if export.ffi_version != FFI_API_VERSION {
                let err_msg = format!(
                    "Incompatible FFI version (plugin: v{}, editor: v{})",
                    export.ffi_version, FFI_API_VERSION
                );
                console_log(
                    LogLevel::Error,
                    "Plugins",
                    format!("[{}] {}", file_name, err_msg),
                );
                let _ = catch_unwind(AssertUnwindSafe(|| (export.vtable.destroy)(export.handle)));
                return Err(PluginError::ApiVersionMismatch {
                    required: export.ffi_version,
                    available: FFI_API_VERSION,
                });
            }

            let mut wrapper = match catch_unwind(AssertUnwindSafe(|| FfiPluginWrapper::new(export)))
            {
                Ok(w) => w,
                Err(_) => {
                    let err_msg = "Plugin crashed while reading manifest";
                    console_log(
                        LogLevel::Error,
                        "Plugins",
                        format!("[{}] {}", file_name, err_msg),
                    );
                    return Err(PluginError::LoadFailed(err_msg.to_string()));
                }
            };

            let manifest = wrapper.manifest().clone();

            if manifest.min_api_version > EDITOR_API_VERSION {
                let err_msg = format!(
                    "Incompatible API version (requires: v{}, editor: v{})",
                    manifest.min_api_version, EDITOR_API_VERSION
                );
                console_log(
                    LogLevel::Error,
                    "Plugins",
                    format!("[{}] {}", manifest.name, err_msg),
                );
                return Err(PluginError::ApiVersionMismatch {
                    required: manifest.min_api_version,
                    available: EDITOR_API_VERSION,
                });
            }

            let plugin_id = manifest.id.clone();
            let plugin_name = manifest.name.clone();
            let plugin_version = manifest.version.clone();

            if self.plugins.contains_key(&plugin_id) {
                return Err(PluginError::LoadFailed(format!(
                    "Plugin '{}' is already loaded",
                    plugin_id
                )));
            }

            self.api.set_current_plugin(Some(plugin_id.clone()));

            let load_result =
                catch_unwind(AssertUnwindSafe(|| wrapper.on_load(&mut self.api)));

            self.api.set_current_plugin(None);

            match load_result {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    let err_msg = format!("Initialization failed: {}", e);
                    console_log(
                        LogLevel::Error,
                        "Plugins",
                        format!("[{}] {}", plugin_name, err_msg),
                    );
                    return Err(e);
                }
                Err(_) => {
                    let err_msg = "Plugin crashed during initialization";
                    console_log(
                        LogLevel::Error,
                        "Plugins",
                        format!("[{}] {}", plugin_name, err_msg),
                    );
                    return Err(PluginError::LoadFailed(err_msg.to_string()));
                }
            }

            self.libraries.push(library);
            self.plugins.insert(plugin_id.clone(), wrapper);
            self.plugin_paths.insert(plugin_id.clone(), path.clone());

            console_log(
                LogLevel::Success,
                "Plugins",
                format!("Loaded: {} v{}", plugin_name, plugin_version),
            );
            Ok(plugin_id)
        }
    }

    /// Unload a plugin by ID.
    pub fn unload_plugin(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        if let Some(mut wrapper) = self.plugins.remove(plugin_id) {
            self.api.set_current_plugin(Some(plugin_id.to_string()));
            wrapper.on_unload(&mut self.api);
            self.api.set_current_plugin(None);
            self.api.remove_plugin_elements(plugin_id);
            self.plugin_paths.remove(plugin_id);
            self.plugin_sources.remove(plugin_id);
            info!("Plugin unloaded: {}", plugin_id);
            Ok(())
        } else {
            Err(PluginError::NotFound(plugin_id.to_string()))
        }
    }

    /// Unload only project plugins (keeps system plugins).
    pub fn unload_project_plugins(&mut self) {
        self.stop_watching();

        let project_ids: Vec<_> = self
            .plugin_sources
            .iter()
            .filter(|(_, source)| **source == PluginSource::Project)
            .map(|(id, _)| id.clone())
            .collect();

        for plugin_id in project_ids {
            if let Some(mut wrapper) = self.plugins.remove(&plugin_id) {
                self.api.set_current_plugin(Some(plugin_id.clone()));
                wrapper.on_unload(&mut self.api);
                self.api.set_current_plugin(None);
                self.api.remove_plugin_elements(&plugin_id);
            }
            self.plugin_paths.remove(&plugin_id);
            self.plugin_sources.remove(&plugin_id);
        }

        self.disabled_plugins
            .retain(|_, d| d.source != PluginSource::Project);
    }

    /// Unload all plugins (system and project).
    pub fn unload_all_plugins(&mut self) {
        self.stop_watching();

        let plugin_ids: Vec<_> = self.plugins.keys().cloned().collect();
        for plugin_id in plugin_ids {
            if let Some(mut wrapper) = self.plugins.remove(&plugin_id) {
                wrapper.on_unload(&mut self.api);
            }
        }
        self.libraries.clear();
        self.plugin_paths.clear();
        self.plugin_sources.clear();
        self.disabled_plugins.clear();
        self.api.clear();
    }

    /// Update all loaded plugins (called every frame).
    pub fn update(&mut self, dt: f32) {
        let plugin_ids: Vec<_> = self.plugins.keys().cloned().collect();

        // Dispatch pending editor events
        let events: Vec<_> = self.pending_events.drain(..).collect();
        for event in &events {
            let event_json = serialize_editor_event(event);
            let event_type = get_event_type(event);

            for plugin_id in &plugin_ids {
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
                "data": serde_json::from_str::<serde_json::Value>(&data_json)
                    .unwrap_or(serde_json::Value::Null)
            })
            .to_string();

            for plugin_id in &plugin_ids {
                if plugin_id == &source_plugin {
                    continue;
                }
                if !self.api.is_subscribed(plugin_id, &event_type)
                    && !self.api.is_subscribed(plugin_id, "plugin.*")
                    && !self
                        .api
                        .is_subscribed(plugin_id, &format!("plugin.{}.*", source_plugin))
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

    /// Queue an event to be dispatched to plugins.
    pub fn queue_event(&mut self, event: EditorEvent) {
        self.pending_events.push(event);
    }

    /// Get the number of loaded plugins.
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }

    /// Get list of loaded plugin manifests.
    pub fn loaded_plugins(&self) -> Vec<PluginManifest> {
        self.plugins
            .values()
            .map(|w| w.manifest().clone())
            .collect()
    }

    /// Enable or disable a plugin.
    pub fn set_plugin_enabled(
        &mut self,
        plugin_id: &str,
        enabled: bool,
    ) -> Result<(), PluginError> {
        if let Some(wrapper) = self.plugins.get_mut(plugin_id) {
            wrapper.set_enabled(enabled);
            Ok(())
        } else {
            Err(PluginError::NotFound(plugin_id.to_string()))
        }
    }

    /// Check if a plugin is loaded.
    pub fn is_plugin_loaded(&self, plugin_id: &str) -> bool {
        self.plugins.contains_key(plugin_id)
    }

    /// Disable a plugin: fully unloads it but remembers it so it can be re-enabled.
    pub fn disable_plugin(&mut self, plugin_id: &str) {
        let path = self.plugin_paths.get(plugin_id).cloned();
        let source = self.plugin_sources.get(plugin_id).copied();
        let manifest = self.plugins.get(plugin_id).map(|w| w.manifest().clone());

        if let (Some(path), Some(source), Some(manifest)) = (path, source, manifest) {
            self.disabled_plugins.insert(
                plugin_id.to_string(),
                DisabledPlugin {
                    manifest,
                    path,
                    source,
                },
            );
        }

        let _ = self.unload_plugin(plugin_id);
    }

    /// Re-enable a previously disabled plugin by reloading it.
    pub fn enable_plugin(&mut self, plugin_id: &str) {
        if let Some(disabled) = self.disabled_plugins.remove(plugin_id) {
            match self.load_plugin(&disabled.path) {
                Ok(id) => {
                    self.plugin_sources.insert(id, disabled.source);
                }
                Err(e) => {
                    error!("Failed to re-enable plugin {}: {}", plugin_id, e);
                    self.disabled_plugins
                        .insert(plugin_id.to_string(), disabled);
                }
            }
        }
    }

    /// Get all known plugins: loaded + disabled, with their info.
    pub fn all_plugins(&self) -> Vec<(PluginManifest, PluginSource, bool)> {
        let mut result = Vec::new();

        for (id, wrapper) in &self.plugins {
            let source = self
                .plugin_sources
                .get(id)
                .copied()
                .unwrap_or(PluginSource::Project);
            result.push((wrapper.manifest().clone(), source, true));
        }

        for (_id, disabled) in &self.disabled_plugins {
            result.push((disabled.manifest.clone(), disabled.source, false));
        }

        result
    }

    /// Get the disabled plugins map.
    pub fn disabled_plugins(&self) -> &HashMap<String, DisabledPlugin> {
        &self.disabled_plugins
    }

    /// Get the API implementation (for internal use).
    pub fn api(&self) -> &EditorApiImpl {
        &self.api
    }

    /// Get mutable API implementation (for internal use).
    pub fn api_mut(&mut self) -> &mut EditorApiImpl {
        &mut self.api
    }
}
