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
}

// ============================================================================
// FFI-safe types
// ============================================================================

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
