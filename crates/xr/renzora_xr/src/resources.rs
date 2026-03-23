//! VR session state and status resources

use bevy::prelude::*;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Marker resource indicating VR mode is active.
/// Inserted when `--vr` flag is passed and XR session starts.
/// Used as a run condition for all VR systems.
#[derive(Resource, Default)]
pub struct VrModeActive;

/// Current VR session status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VrStatus {
    /// No headset connected / session not started
    Disconnected,
    /// OpenXR session is initializing
    Initializing,
    /// Session ready, waiting for user to put on headset
    Ready,
    /// Headset is on, rendering active
    Focused,
    /// Headset removed or app minimized in VR
    Visible,
    /// Session is stopping
    Stopping,
    /// Session ended (error or user quit)
    Stopped,
}

impl VrStatus {
    /// Returns true if the session is actively rendering (Focused or Visible)
    pub fn is_active(&self) -> bool {
        matches!(self, VrStatus::Focused | VrStatus::Visible)
    }

    /// Returns a string representation suitable for scripting
    pub fn as_str(&self) -> &'static str {
        match self {
            VrStatus::Disconnected => "disconnected",
            VrStatus::Initializing => "initializing",
            VrStatus::Ready => "ready",
            VrStatus::Focused => "focused",
            VrStatus::Visible => "visible",
            VrStatus::Stopping => "stopping",
            VrStatus::Stopped => "stopped",
        }
    }
}

/// Message emitted when VR session status changes
#[derive(Message, Debug, Clone)]
pub struct VrSessionStatusChanged {
    pub old_status: VrStatus,
    pub new_status: VrStatus,
}

/// Tracks the OpenXR session lifecycle state
#[derive(Resource)]
pub struct VrSessionState {
    pub status: VrStatus,
    /// Name of the connected headset (e.g. "Quest 3", "Valve Index")
    pub headset_name: String,
    /// Current refresh rate in Hz (from runtime's predicted_display_period)
    pub refresh_rate: f32,
    /// Available refresh rates reported by the headset (e.g. [72, 80, 90, 120])
    pub available_refresh_rates: Vec<f32>,
    /// Refresh rate requested by the user (0 = no preference / use runtime default)
    pub requested_refresh_rate: f32,
    /// Left controller battery level (0.0 - 1.0, negative = unknown)
    pub left_battery: f32,
    /// Right controller battery level (0.0 - 1.0, negative = unknown)
    pub right_battery: f32,
}

impl Default for VrSessionState {
    fn default() -> Self {
        Self {
            status: VrStatus::Disconnected,
            headset_name: String::new(),
            refresh_rate: 0.0,
            available_refresh_rates: Vec::new(),
            requested_refresh_rate: 0.0,
            left_battery: -1.0,
            right_battery: -1.0,
        }
    }
}

// ============================================================================
// VR Log Buffer — routes logs to the editor console
// ============================================================================

/// Log level for VR messages (mirrors editor's LogLevel)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VrLogLevel {
    Info,
    Success,
    Warning,
    Error,
}

/// A single VR log entry
#[derive(Debug, Clone)]
pub struct VrLogEntry {
    pub level: VrLogLevel,
    pub message: String,
}

/// Thread-safe shared buffer for VR log messages.
///
/// Written to by renzora_xr systems, drained by the editor's
/// `drain_vr_logs` system each frame into the editor console.
#[derive(Clone, Default)]
pub struct VrLogBuffer(pub Arc<Mutex<VecDeque<VrLogEntry>>>);

impl VrLogBuffer {
    pub fn push(&self, level: VrLogLevel, message: impl Into<String>) {
        if let Ok(mut buffer) = self.0.lock() {
            buffer.push_back(VrLogEntry {
                level,
                message: message.into(),
            });
        }
    }

    pub fn drain(&self) -> Vec<VrLogEntry> {
        if let Ok(mut buffer) = self.0.lock() {
            buffer.drain(..).collect()
        } else {
            Vec::new()
        }
    }
}

/// Global VR log buffer, accessible from anywhere in renzora_xr
static GLOBAL_VR_LOG_BUFFER: std::sync::OnceLock<VrLogBuffer> = std::sync::OnceLock::new();

/// Initialize the global VR log buffer. Returns a clone for the editor to drain.
pub fn init_vr_log_buffer() -> VrLogBuffer {
    let buffer = VrLogBuffer::default();
    let _ = GLOBAL_VR_LOG_BUFFER.set(buffer.clone());
    buffer
}

/// Get the global VR log buffer (if initialized).
pub fn get_vr_log_buffer() -> Option<&'static VrLogBuffer> {
    GLOBAL_VR_LOG_BUFFER.get()
}

/// Log an info message to both bevy log and the VR console buffer.
pub fn vr_info(msg: impl Into<String>) {
    let msg = msg.into();
    bevy::log::info!("{}", msg);
    if let Some(buffer) = get_vr_log_buffer() {
        buffer.push(VrLogLevel::Info, msg);
    }
}

/// Log a success message to both bevy log and the VR console buffer.
pub fn vr_success(msg: impl Into<String>) {
    let msg = msg.into();
    bevy::log::info!("{}", msg);
    if let Some(buffer) = get_vr_log_buffer() {
        buffer.push(VrLogLevel::Success, msg);
    }
}

/// Log a warning to both bevy log and the VR console buffer.
pub fn vr_warn(msg: impl Into<String>) {
    let msg = msg.into();
    bevy::log::warn!("{}", msg);
    if let Some(buffer) = get_vr_log_buffer() {
        buffer.push(VrLogLevel::Warning, msg);
    }
}

/// Log an error to both bevy log and the VR console buffer.
pub fn vr_error(msg: impl Into<String>) {
    let msg = msg.into();
    bevy::log::error!("{}", msg);
    if let Some(buffer) = get_vr_log_buffer() {
        buffer.push(VrLogLevel::Error, msg);
    }
}
