//! FFI-safe types for stable ABI plugin communication.
//!
//! This module provides C-compatible types that work reliably across DLL boundaries.
//! Callbacks are passed through the HostApi struct which is included in EditorApiHandle.

use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr;

use crate::abi::PluginManifest;

/// API version for FFI compatibility checking
pub const FFI_API_VERSION: u32 = 1;

/// Opaque handle to plugin state (allocated by plugin, passed around by host)
pub type PluginHandle = *mut c_void;

/// Opaque handle to the editor API context (provided by host)
/// This is actually a pointer to HostApi
pub type EditorApiHandle = *mut c_void;

// ============================================================================
// Callback type definitions for HostApi
// ============================================================================

// Logging
pub type LogFn = unsafe extern "C" fn(ctx: *mut c_void, message: *const c_char);

// Status bar
pub type SetStatusItemFn = unsafe extern "C" fn(ctx: *mut c_void, item: *const FfiStatusBarItem);
pub type RemoveStatusItemFn = unsafe extern "C" fn(ctx: *mut c_void, id: *const c_char);

// Undo/Redo
pub type UndoFn = unsafe extern "C" fn(ctx: *mut c_void) -> bool;
pub type RedoFn = unsafe extern "C" fn(ctx: *mut c_void) -> bool;
pub type CanUndoFn = unsafe extern "C" fn(ctx: *mut c_void) -> bool;
pub type CanRedoFn = unsafe extern "C" fn(ctx: *mut c_void) -> bool;

// Toolbar operations
pub type AddToolbarButtonFn = unsafe extern "C" fn(ctx: *mut c_void, id: u64, icon: *const c_char, tooltip: *const c_char);
pub type RemoveToolbarItemFn = unsafe extern "C" fn(ctx: *mut c_void, id: u64);

// Menu operations
pub type AddMenuItemFn = unsafe extern "C" fn(ctx: *mut c_void, menu: u8, item: *const FfiMenuItem);
pub type RemoveMenuItemFn = unsafe extern "C" fn(ctx: *mut c_void, id: u64);
pub type SetMenuItemEnabledFn = unsafe extern "C" fn(ctx: *mut c_void, id: u64, enabled: bool);
pub type SetMenuItemCheckedFn = unsafe extern "C" fn(ctx: *mut c_void, id: u64, checked: bool);

// Panel operations
pub type RegisterPanelFn = unsafe extern "C" fn(ctx: *mut c_void, panel: *const FfiPanelDefinition) -> bool;
pub type UnregisterPanelFn = unsafe extern "C" fn(ctx: *mut c_void, id: *const c_char);
pub type SetPanelContentFn = unsafe extern "C" fn(ctx: *mut c_void, panel_id: *const c_char, widgets_json: *const c_char);
pub type SetPanelVisibleFn = unsafe extern "C" fn(ctx: *mut c_void, panel_id: *const c_char, visible: bool);
pub type IsPanelVisibleFn = unsafe extern "C" fn(ctx: *mut c_void, panel_id: *const c_char) -> bool;

// Tab operations (docked tabs in panel areas)
pub type RegisterTabFn = unsafe extern "C" fn(ctx: *mut c_void, tab: *const FfiTabDefinition) -> bool;
pub type UnregisterTabFn = unsafe extern "C" fn(ctx: *mut c_void, id: *const c_char);
pub type SetTabContentFn = unsafe extern "C" fn(ctx: *mut c_void, tab_id: *const c_char, widgets_json: *const c_char);
pub type SetActiveTabFn = unsafe extern "C" fn(ctx: *mut c_void, location: u8, tab_id: *const c_char);
pub type GetActiveTabFn = unsafe extern "C" fn(ctx: *mut c_void, location: u8) -> FfiOwnedString;

// Entity operations
pub type GetEntityByNameFn = unsafe extern "C" fn(ctx: *mut c_void, name: *const c_char) -> FfiEntityId;
pub type GetEntityTransformFn = unsafe extern "C" fn(ctx: *mut c_void, entity: FfiEntityId) -> FfiTransform;
pub type SetEntityTransformFn = unsafe extern "C" fn(ctx: *mut c_void, entity: FfiEntityId, transform: *const FfiTransform);
pub type GetEntityNameFn = unsafe extern "C" fn(ctx: *mut c_void, entity: FfiEntityId) -> FfiOwnedString;
pub type SetEntityNameFn = unsafe extern "C" fn(ctx: *mut c_void, entity: FfiEntityId, name: *const c_char);
pub type GetEntityVisibleFn = unsafe extern "C" fn(ctx: *mut c_void, entity: FfiEntityId) -> bool;
pub type SetEntityVisibleFn = unsafe extern "C" fn(ctx: *mut c_void, entity: FfiEntityId, visible: bool);
pub type SpawnEntityFn = unsafe extern "C" fn(ctx: *mut c_void, name: *const c_char, transform: *const FfiTransform) -> FfiEntityId;
pub type DespawnEntityFn = unsafe extern "C" fn(ctx: *mut c_void, entity: FfiEntityId);
pub type GetEntityParentFn = unsafe extern "C" fn(ctx: *mut c_void, entity: FfiEntityId) -> FfiEntityId;
pub type GetEntityChildrenFn = unsafe extern "C" fn(ctx: *mut c_void, entity: FfiEntityId) -> FfiEntityList;
pub type ReparentEntityFn = unsafe extern "C" fn(ctx: *mut c_void, entity: FfiEntityId, new_parent: FfiEntityId);
pub type GetSelectedEntityFn = unsafe extern "C" fn(ctx: *mut c_void) -> FfiEntityId;
pub type SetSelectedEntityFn = unsafe extern "C" fn(ctx: *mut c_void, entity: FfiEntityId);
pub type ClearSelectionFn = unsafe extern "C" fn(ctx: *mut c_void);

// ============================================================================
// Host API vtable - all callbacks the host provides to plugins
// ============================================================================

/// Host API vtable - passed to plugins so they can call back into the host
#[repr(C)]
pub struct HostApi {
    /// Context pointer (EditorApiImpl*)
    pub ctx: *mut c_void,

    // === Logging ===
    pub log_info: LogFn,
    pub log_warn: LogFn,
    pub log_error: LogFn,

    // === Status Bar ===
    pub set_status_item: SetStatusItemFn,
    pub remove_status_item: RemoveStatusItemFn,

    // === Undo/Redo ===
    pub undo: UndoFn,
    pub redo: RedoFn,
    pub can_undo: CanUndoFn,
    pub can_redo: CanRedoFn,

    // === Panel System ===
    pub register_panel: RegisterPanelFn,
    pub unregister_panel: UnregisterPanelFn,
    pub set_panel_content: SetPanelContentFn,
    pub set_panel_visible: SetPanelVisibleFn,
    pub is_panel_visible: IsPanelVisibleFn,

    // === Entity Operations ===
    pub get_entity_by_name: GetEntityByNameFn,
    pub get_entity_transform: GetEntityTransformFn,
    pub set_entity_transform: SetEntityTransformFn,
    pub get_entity_name: GetEntityNameFn,
    pub set_entity_name: SetEntityNameFn,
    pub get_entity_visible: GetEntityVisibleFn,
    pub set_entity_visible: SetEntityVisibleFn,
    pub spawn_entity: SpawnEntityFn,
    pub despawn_entity: DespawnEntityFn,
    pub get_entity_parent: GetEntityParentFn,
    pub get_entity_children: GetEntityChildrenFn,
    pub reparent_entity: ReparentEntityFn,

    // === Selection ===
    pub get_selected_entity: GetSelectedEntityFn,
    pub set_selected_entity: SetSelectedEntityFn,
    pub clear_selection: ClearSelectionFn,

    // === Toolbar ===
    pub add_toolbar_button: AddToolbarButtonFn,
    pub remove_toolbar_item: RemoveToolbarItemFn,

    // === Menu ===
    pub add_menu_item: AddMenuItemFn,
    pub remove_menu_item: RemoveMenuItemFn,
    pub set_menu_item_enabled: SetMenuItemEnabledFn,
    pub set_menu_item_checked: SetMenuItemCheckedFn,

    // === Tabs (docked in panel areas) ===
    pub register_tab: RegisterTabFn,
    pub unregister_tab: UnregisterTabFn,
    pub set_tab_content: SetTabContentFn,
    pub set_active_tab: SetActiveTabFn,
    pub get_active_tab: GetActiveTabFn,
}

// ============================================================================
// FFI-safe types
// ============================================================================

/// FFI-safe entity ID
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FfiEntityId(pub u64);

impl FfiEntityId {
    pub const INVALID: Self = Self(u64::MAX);

    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn is_valid(&self) -> bool {
        *self != Self::INVALID
    }
}

/// FFI-safe transform
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FfiTransform {
    pub translation: [f32; 3],
    pub rotation: [f32; 4], // Quaternion as [x, y, z, w]
    pub scale: [f32; 3],
}

impl Default for FfiTransform {
    fn default() -> Self {
        Self {
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0], // Identity quaternion
            scale: [1.0, 1.0, 1.0],
        }
    }
}

impl FfiTransform {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_translation(mut self, x: f32, y: f32, z: f32) -> Self {
        self.translation = [x, y, z];
        self
    }

    pub fn with_scale(mut self, x: f32, y: f32, z: f32) -> Self {
        self.scale = [x, y, z];
        self
    }
}

/// FFI-safe menu location
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FfiMenuLocation {
    File = 0,
    Edit = 1,
    View = 2,
    Scene = 3,
    Tools = 4,
    Help = 5,
}

/// FFI-safe menu item
#[repr(C)]
pub struct FfiMenuItem {
    pub id: u64,
    pub label: FfiString,
    pub shortcut: FfiString,  // Empty if no shortcut
    pub icon: FfiString,      // Empty if no icon
    pub enabled: bool,
    pub checked: bool,
    pub is_separator: bool,
}

impl FfiMenuItem {
    pub fn new(id: u64, label: &str) -> Self {
        Self {
            id,
            label: FfiString::from_str(label),
            shortcut: FfiString::empty(),
            icon: FfiString::empty(),
            enabled: true,
            checked: false,
            is_separator: false,
        }
    }

    pub fn separator() -> Self {
        Self {
            id: 0,
            label: FfiString::empty(),
            shortcut: FfiString::empty(),
            icon: FfiString::empty(),
            enabled: true,
            checked: false,
            is_separator: true,
        }
    }
}

/// FFI-safe panel location
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FfiPanelLocation {
    Left = 0,
    Right = 1,
    Bottom = 2,
    Floating = 3,
}

/// FFI-safe tab location for docked tabs
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FfiTabLocation {
    Left = 0,   // Alongside Hierarchy
    Right = 1,  // Alongside Inspector
    Bottom = 2, // Alongside Assets/Console
}

/// FFI-safe tab definition
#[repr(C)]
pub struct FfiTabDefinition {
    pub id: FfiString,
    pub title: FfiString,
    pub icon: FfiString,  // Empty if no icon
    pub location: FfiTabLocation,
}

/// FFI-safe panel definition
#[repr(C)]
pub struct FfiPanelDefinition {
    pub id: FfiString,
    pub title: FfiString,
    pub icon: FfiString,  // Empty if no icon
    pub location: FfiPanelLocation,
    pub min_width: f32,
    pub min_height: f32,
    pub closable: bool,
}

impl FfiPanelDefinition {
    pub fn new(id: &str, title: &str, location: FfiPanelLocation) -> Self {
        Self {
            id: FfiString::from_str(id),
            title: FfiString::from_str(title),
            icon: FfiString::empty(),
            location,
            min_width: 200.0,
            min_height: 100.0,
            closable: true,
        }
    }
}

/// FFI-safe entity list (for returning multiple entities)
#[repr(C)]
pub struct FfiEntityList {
    pub ptr: *mut FfiEntityId,
    pub len: usize,
    pub capacity: usize,
}

impl FfiEntityList {
    /// Create an empty list
    pub fn empty() -> Self {
        Self {
            ptr: ptr::null_mut(),
            len: 0,
            capacity: 0,
        }
    }

    /// Create from a Vec (takes ownership)
    pub fn from_vec(mut vec: Vec<FfiEntityId>) -> Self {
        let len = vec.len();
        let capacity = vec.capacity();
        let ptr = vec.as_mut_ptr();
        std::mem::forget(vec);
        Self { ptr, len, capacity }
    }

    /// Convert back to Vec (takes ownership)
    /// # Safety
    /// Must only be called once
    pub unsafe fn into_vec(self) -> Vec<FfiEntityId> {
        if self.ptr.is_null() {
            return Vec::new();
        }
        Vec::from_raw_parts(self.ptr, self.len, self.capacity)
    }

    /// Free the list memory
    /// # Safety
    /// Must only be called once
    pub unsafe fn free(self) {
        if !self.ptr.is_null() {
            let _ = Vec::from_raw_parts(self.ptr, self.len, self.capacity);
        }
    }
}

/// FFI-safe string that can cross DLL boundaries (borrowed)
#[repr(C)]
pub struct FfiString {
    pub ptr: *const c_char,
    pub len: usize,
}

impl FfiString {
    /// Create from a &str (borrows, does not allocate)
    pub fn from_str(s: &str) -> Self {
        Self {
            ptr: s.as_ptr() as *const c_char,
            len: s.len(),
        }
    }

    /// Create an empty string
    pub fn empty() -> Self {
        Self {
            ptr: ptr::null(),
            len: 0,
        }
    }

    /// Convert to Rust String (copies)
    /// # Safety
    /// The FfiString must point to valid UTF-8 data
    pub unsafe fn to_string(&self) -> String {
        if self.ptr.is_null() || self.len == 0 {
            return String::new();
        }
        let slice = std::slice::from_raw_parts(self.ptr as *const u8, self.len);
        String::from_utf8_lossy(slice).into_owned()
    }
}

/// FFI-safe owned string (for returning strings from plugin to host)
#[repr(C)]
pub struct FfiOwnedString {
    pub ptr: *mut c_char,
    pub len: usize,
    pub capacity: usize,
}

impl FfiOwnedString {
    /// Create from a Rust String (takes ownership)
    pub fn from_string(s: String) -> Self {
        let bytes = s.into_bytes();
        let len = bytes.len();
        let capacity = bytes.capacity();
        let mut bytes = std::mem::ManuallyDrop::new(bytes);
        Self {
            ptr: bytes.as_mut_ptr() as *mut c_char,
            len,
            capacity,
        }
    }

    /// Create an empty string
    pub fn empty() -> Self {
        Self {
            ptr: ptr::null_mut(),
            len: 0,
            capacity: 0,
        }
    }

    /// Convert to Rust String (takes ownership back)
    /// # Safety
    /// Must only be called once
    pub unsafe fn into_string(self) -> String {
        if self.ptr.is_null() {
            return String::new();
        }
        let bytes = Vec::from_raw_parts(self.ptr as *mut u8, self.len, self.capacity);
        String::from_utf8_unchecked(bytes)
    }
}

/// FFI-safe result type
#[repr(C)]
pub struct FfiResult {
    pub success: bool,
    pub error_message: FfiOwnedString,
}

impl FfiResult {
    pub fn ok() -> Self {
        Self {
            success: true,
            error_message: FfiOwnedString::empty(),
        }
    }

    pub fn err(message: String) -> Self {
        Self {
            success: false,
            error_message: FfiOwnedString::from_string(message),
        }
    }
}

/// FFI-safe manifest returned by plugin
#[repr(C)]
pub struct FfiManifest {
    pub id: FfiOwnedString,
    pub name: FfiOwnedString,
    pub version: FfiOwnedString,
    pub author: FfiOwnedString,
    pub description: FfiOwnedString,
    pub min_api_version: u32,
}

impl FfiManifest {
    pub fn from_manifest(m: PluginManifest) -> Self {
        Self {
            id: FfiOwnedString::from_string(m.id),
            name: FfiOwnedString::from_string(m.name),
            version: FfiOwnedString::from_string(m.version),
            author: FfiOwnedString::from_string(m.author),
            description: FfiOwnedString::from_string(m.description),
            min_api_version: m.min_api_version,
        }
    }

    /// Convert to Rust PluginManifest
    /// # Safety
    /// Must only be called once
    pub unsafe fn into_manifest(self) -> PluginManifest {
        PluginManifest {
            id: self.id.into_string(),
            name: self.name.into_string(),
            version: self.version.into_string(),
            author: self.author.into_string(),
            description: self.description.into_string(),
            capabilities: Vec::new(),
            dependencies: Vec::new(),
            min_api_version: self.min_api_version,
        }
    }
}

/// FFI-safe status bar item
#[repr(C)]
pub struct FfiStatusBarItem {
    pub id: FfiString,
    pub icon: FfiString,
    pub text: FfiString,
    pub tooltip: FfiString,
    pub align_right: bool,
    pub priority: i32,
}

impl FfiStatusBarItem {
    pub fn new(
        id: &str,
        text: &str,
        icon: Option<&str>,
        tooltip: Option<&str>,
        align_right: bool,
        priority: i32,
    ) -> Self {
        Self {
            id: FfiString::from_str(id),
            text: FfiString::from_str(text),
            icon: icon.map(FfiString::from_str).unwrap_or(FfiString::empty()),
            tooltip: tooltip.map(FfiString::from_str).unwrap_or(FfiString::empty()),
            align_right,
            priority,
        }
    }
}

// ============================================================================
// Plugin vtable
// ============================================================================

/// Function pointer types for the plugin vtable
pub type ManifestFn = unsafe extern "C" fn(handle: PluginHandle) -> FfiManifest;
pub type OnLoadFn = unsafe extern "C" fn(handle: PluginHandle, api: EditorApiHandle) -> FfiResult;
pub type OnUnloadFn = unsafe extern "C" fn(handle: PluginHandle, api: EditorApiHandle);
pub type OnUpdateFn = unsafe extern "C" fn(handle: PluginHandle, api: EditorApiHandle, dt: f32);
pub type OnEventFn = unsafe extern "C" fn(handle: PluginHandle, api: EditorApiHandle, event_json: *const c_char);
pub type DestroyFn = unsafe extern "C" fn(handle: PluginHandle);

/// Plugin vtable - contains function pointers for all plugin methods
#[repr(C)]
pub struct PluginVTable {
    pub manifest: ManifestFn,
    pub on_load: OnLoadFn,
    pub on_unload: OnUnloadFn,
    pub on_update: OnUpdateFn,
    pub on_event: OnEventFn,
    pub destroy: DestroyFn,
}

/// The struct returned by create_plugin
#[repr(C)]
pub struct PluginExport {
    pub ffi_version: u32,
    pub handle: PluginHandle,
    pub vtable: PluginVTable,
}

// ============================================================================
// Plugin-side API wrappers - call host callbacks
// ============================================================================

/// FFI-safe API wrapper that plugins use to call host functions
pub struct FfiEditorApi {
    host_api: *const HostApi,
}

impl FfiEditorApi {
    /// Create a new API wrapper from a HostApi pointer
    pub fn new(handle: EditorApiHandle) -> Self {
        Self { host_api: handle as *const HostApi }
    }

    fn api(&self) -> Option<&HostApi> {
        if self.host_api.is_null() {
            None
        } else {
            unsafe { Some(&*self.host_api) }
        }
    }

    // === Logging ===

    pub fn log_info(&self, message: &str) {
        if let Some(api) = self.api() {
            let c_msg = CString::new(message).unwrap_or_default();
            unsafe { (api.log_info)(api.ctx, c_msg.as_ptr()) };
        }
    }

    pub fn log_warn(&self, message: &str) {
        if let Some(api) = self.api() {
            let c_msg = CString::new(message).unwrap_or_default();
            unsafe { (api.log_warn)(api.ctx, c_msg.as_ptr()) };
        }
    }

    pub fn log_error(&self, message: &str) {
        if let Some(api) = self.api() {
            let c_msg = CString::new(message).unwrap_or_default();
            unsafe { (api.log_error)(api.ctx, c_msg.as_ptr()) };
        }
    }

    // === Status Bar ===

    pub fn set_status_item(&self, id: &str, text: &str, icon: Option<&str>, tooltip: Option<&str>, align_right: bool, priority: i32) {
        if let Some(api) = self.api() {
            let item = FfiStatusBarItem::new(id, text, icon, tooltip, align_right, priority);
            unsafe { (api.set_status_item)(api.ctx, &item) };
        }
    }

    pub fn remove_status_item(&self, id: &str) {
        if let Some(api) = self.api() {
            let c_id = CString::new(id).unwrap_or_default();
            unsafe { (api.remove_status_item)(api.ctx, c_id.as_ptr()) };
        }
    }

    // === Undo/Redo ===

    /// Undo the last command. Returns true if successful.
    pub fn undo(&self) -> bool {
        if let Some(api) = self.api() {
            unsafe { (api.undo)(api.ctx) }
        } else {
            false
        }
    }

    /// Redo the last undone command. Returns true if successful.
    pub fn redo(&self) -> bool {
        if let Some(api) = self.api() {
            unsafe { (api.redo)(api.ctx) }
        } else {
            false
        }
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        if let Some(api) = self.api() {
            unsafe { (api.can_undo)(api.ctx) }
        } else {
            false
        }
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        if let Some(api) = self.api() {
            unsafe { (api.can_redo)(api.ctx) }
        } else {
            false
        }
    }

    // === Panel System ===

    /// Register a new panel
    pub fn register_panel(
        &self,
        id: &str,
        title: &str,
        location: FfiPanelLocation,
        icon: Option<&str>,
        min_size: [f32; 2],
    ) -> bool {
        if let Some(api) = self.api() {
            // We need to keep these strings alive for the duration of the call
            let panel = FfiPanelDefinition {
                id: FfiString::from_str(id),
                title: FfiString::from_str(title),
                icon: icon.map(FfiString::from_str).unwrap_or(FfiString::empty()),
                location,
                min_width: min_size[0],
                min_height: min_size[1],
                closable: true,
            };
            unsafe { (api.register_panel)(api.ctx, &panel) }
        } else {
            false
        }
    }

    /// Unregister a panel
    pub fn unregister_panel(&self, id: &str) {
        if let Some(api) = self.api() {
            let c_id = CString::new(id).unwrap_or_default();
            unsafe { (api.unregister_panel)(api.ctx, c_id.as_ptr()) };
        }
    }

    /// Set panel content using JSON-serialized widgets
    pub fn set_panel_content_json(&self, panel_id: &str, widgets_json: &str) {
        if let Some(api) = self.api() {
            let c_id = CString::new(panel_id).unwrap_or_default();
            let c_json = CString::new(widgets_json).unwrap_or_default();
            unsafe { (api.set_panel_content)(api.ctx, c_id.as_ptr(), c_json.as_ptr()) };
        }
    }

    /// Set panel visibility
    pub fn set_panel_visible(&self, panel_id: &str, visible: bool) {
        if let Some(api) = self.api() {
            let c_id = CString::new(panel_id).unwrap_or_default();
            unsafe { (api.set_panel_visible)(api.ctx, c_id.as_ptr(), visible) };
        }
    }

    /// Check if panel is visible
    pub fn is_panel_visible(&self, panel_id: &str) -> bool {
        if let Some(api) = self.api() {
            let c_id = CString::new(panel_id).unwrap_or_default();
            unsafe { (api.is_panel_visible)(api.ctx, c_id.as_ptr()) }
        } else {
            false
        }
    }

    // === Entity Operations ===

    /// Get an entity by name. Returns INVALID if not found.
    pub fn get_entity_by_name(&self, name: &str) -> FfiEntityId {
        if let Some(api) = self.api() {
            let c_name = CString::new(name).unwrap_or_default();
            unsafe { (api.get_entity_by_name)(api.ctx, c_name.as_ptr()) }
        } else {
            FfiEntityId::INVALID
        }
    }

    /// Get an entity's transform
    pub fn get_entity_transform(&self, entity: FfiEntityId) -> FfiTransform {
        if let Some(api) = self.api() {
            unsafe { (api.get_entity_transform)(api.ctx, entity) }
        } else {
            FfiTransform::default()
        }
    }

    /// Set an entity's transform
    pub fn set_entity_transform(&self, entity: FfiEntityId, transform: &FfiTransform) {
        if let Some(api) = self.api() {
            unsafe { (api.set_entity_transform)(api.ctx, entity, transform) };
        }
    }

    /// Get an entity's name
    pub fn get_entity_name(&self, entity: FfiEntityId) -> String {
        if let Some(api) = self.api() {
            let owned = unsafe { (api.get_entity_name)(api.ctx, entity) };
            unsafe { owned.into_string() }
        } else {
            String::new()
        }
    }

    /// Set an entity's name
    pub fn set_entity_name(&self, entity: FfiEntityId, name: &str) {
        if let Some(api) = self.api() {
            let c_name = CString::new(name).unwrap_or_default();
            unsafe { (api.set_entity_name)(api.ctx, entity, c_name.as_ptr()) };
        }
    }

    /// Get an entity's visibility
    pub fn get_entity_visible(&self, entity: FfiEntityId) -> bool {
        if let Some(api) = self.api() {
            unsafe { (api.get_entity_visible)(api.ctx, entity) }
        } else {
            false
        }
    }

    /// Set an entity's visibility
    pub fn set_entity_visible(&self, entity: FfiEntityId, visible: bool) {
        if let Some(api) = self.api() {
            unsafe { (api.set_entity_visible)(api.ctx, entity, visible) };
        }
    }

    /// Spawn a new entity with the given name and transform
    pub fn spawn_entity(&self, name: &str, transform: &FfiTransform) -> FfiEntityId {
        if let Some(api) = self.api() {
            let c_name = CString::new(name).unwrap_or_default();
            unsafe { (api.spawn_entity)(api.ctx, c_name.as_ptr(), transform) }
        } else {
            FfiEntityId::INVALID
        }
    }

    /// Despawn an entity
    pub fn despawn_entity(&self, entity: FfiEntityId) {
        if let Some(api) = self.api() {
            unsafe { (api.despawn_entity)(api.ctx, entity) };
        }
    }

    /// Get an entity's parent. Returns INVALID if no parent.
    pub fn get_entity_parent(&self, entity: FfiEntityId) -> FfiEntityId {
        if let Some(api) = self.api() {
            unsafe { (api.get_entity_parent)(api.ctx, entity) }
        } else {
            FfiEntityId::INVALID
        }
    }

    /// Get an entity's children
    pub fn get_entity_children(&self, entity: FfiEntityId) -> Vec<FfiEntityId> {
        if let Some(api) = self.api() {
            let list = unsafe { (api.get_entity_children)(api.ctx, entity) };
            unsafe { list.into_vec() }
        } else {
            Vec::new()
        }
    }

    /// Reparent an entity to a new parent. Use INVALID for no parent (root).
    pub fn reparent_entity(&self, entity: FfiEntityId, new_parent: FfiEntityId) {
        if let Some(api) = self.api() {
            unsafe { (api.reparent_entity)(api.ctx, entity, new_parent) };
        }
    }

    // === Selection ===

    /// Get the currently selected entity. Returns INVALID if none selected.
    pub fn get_selected_entity(&self) -> FfiEntityId {
        if let Some(api) = self.api() {
            unsafe { (api.get_selected_entity)(api.ctx) }
        } else {
            FfiEntityId::INVALID
        }
    }

    /// Set the selected entity
    pub fn set_selected_entity(&self, entity: FfiEntityId) {
        if let Some(api) = self.api() {
            unsafe { (api.set_selected_entity)(api.ctx, entity) };
        }
    }

    /// Clear the current selection
    pub fn clear_selection(&self) {
        if let Some(api) = self.api() {
            unsafe { (api.clear_selection)(api.ctx) };
        }
    }

    // === Toolbar ===

    /// Add a toolbar button
    pub fn add_toolbar_button(&self, id: u64, icon: &str, tooltip: &str) {
        if let Some(api) = self.api() {
            let c_icon = CString::new(icon).unwrap_or_default();
            let c_tooltip = CString::new(tooltip).unwrap_or_default();
            unsafe { (api.add_toolbar_button)(api.ctx, id, c_icon.as_ptr(), c_tooltip.as_ptr()) };
        }
    }

    /// Remove a toolbar item
    pub fn remove_toolbar_item(&self, id: u64) {
        if let Some(api) = self.api() {
            unsafe { (api.remove_toolbar_item)(api.ctx, id) };
        }
    }

    // === Menu ===

    /// Add a menu item to a specific menu
    pub fn add_menu_item(&self, menu: FfiMenuLocation, id: u64, label: &str, shortcut: Option<&str>, icon: Option<&str>) {
        if let Some(api) = self.api() {
            let item = FfiMenuItem {
                id,
                label: FfiString::from_str(label),
                shortcut: shortcut.map(FfiString::from_str).unwrap_or(FfiString::empty()),
                icon: icon.map(FfiString::from_str).unwrap_or(FfiString::empty()),
                enabled: true,
                checked: false,
                is_separator: false,
            };
            unsafe { (api.add_menu_item)(api.ctx, menu as u8, &item) };
        }
    }

    /// Add a separator to a menu
    pub fn add_menu_separator(&self, menu: FfiMenuLocation, id: u64) {
        if let Some(api) = self.api() {
            let item = FfiMenuItem::separator();
            let mut sep_item = item;
            sep_item.id = id;
            unsafe { (api.add_menu_item)(api.ctx, menu as u8, &sep_item) };
        }
    }

    /// Remove a menu item
    pub fn remove_menu_item(&self, id: u64) {
        if let Some(api) = self.api() {
            unsafe { (api.remove_menu_item)(api.ctx, id) };
        }
    }

    /// Set menu item enabled state
    pub fn set_menu_item_enabled(&self, id: u64, enabled: bool) {
        if let Some(api) = self.api() {
            unsafe { (api.set_menu_item_enabled)(api.ctx, id, enabled) };
        }
    }

    /// Set menu item checked state
    pub fn set_menu_item_checked(&self, id: u64, checked: bool) {
        if let Some(api) = self.api() {
            unsafe { (api.set_menu_item_checked)(api.ctx, id, checked) };
        }
    }

    // === Tabs (docked in panel areas) ===

    /// Register a new tab in a panel area
    pub fn register_tab(
        &self,
        id: &str,
        title: &str,
        location: FfiTabLocation,
        icon: Option<&str>,
    ) -> bool {
        if let Some(api) = self.api() {
            let tab = FfiTabDefinition {
                id: FfiString::from_str(id),
                title: FfiString::from_str(title),
                icon: icon.map(FfiString::from_str).unwrap_or(FfiString::empty()),
                location,
            };
            unsafe { (api.register_tab)(api.ctx, &tab) }
        } else {
            false
        }
    }

    /// Unregister a tab
    pub fn unregister_tab(&self, id: &str) {
        if let Some(api) = self.api() {
            let c_id = CString::new(id).unwrap_or_default();
            unsafe { (api.unregister_tab)(api.ctx, c_id.as_ptr()) };
        }
    }

    /// Set tab content using JSON-serialized widgets
    pub fn set_tab_content_json(&self, tab_id: &str, widgets_json: &str) {
        if let Some(api) = self.api() {
            let c_id = CString::new(tab_id).unwrap_or_default();
            let c_json = CString::new(widgets_json).unwrap_or_default();
            unsafe { (api.set_tab_content)(api.ctx, c_id.as_ptr(), c_json.as_ptr()) };
        }
    }

    /// Set the active tab in a panel location
    pub fn set_active_tab(&self, location: FfiTabLocation, tab_id: &str) {
        if let Some(api) = self.api() {
            let c_id = CString::new(tab_id).unwrap_or_default();
            unsafe { (api.set_active_tab)(api.ctx, location as u8, c_id.as_ptr()) };
        }
    }

    /// Get the active tab ID in a panel location
    pub fn get_active_tab(&self, location: FfiTabLocation) -> String {
        if let Some(api) = self.api() {
            let owned = unsafe { (api.get_active_tab)(api.ctx, location as u8) };
            unsafe { owned.into_string() }
        } else {
            String::new()
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Helper to convert a C string to Rust String
/// # Safety
/// ptr must be a valid null-terminated C string
pub unsafe fn cstr_to_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    CStr::from_ptr(ptr).to_string_lossy().into_owned()
}
