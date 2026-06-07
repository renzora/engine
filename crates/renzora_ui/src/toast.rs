//! Toast notification queue — ephemeral messages surfaced by the editor shell.
//!
//! This is a pure data queue: editor systems push notifications via
//! [`Toasts::info`]/[`success`](Toasts::success)/[`warning`](Toasts::warning)/
//! [`error`](Toasts::error), and the native (bevy_ui) shell drains and renders
//! them. The legacy egui rendering lived here but was removed in the native
//! migration.

use bevy::prelude::*;

/// Severity level for a toast notification.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl ToastLevel {
    /// Accent color for this level (RGB).
    pub fn color(&self) -> [u8; 3] {
        match self {
            ToastLevel::Info => [140, 180, 220],
            ToastLevel::Success => [100, 200, 120],
            ToastLevel::Warning => [230, 180, 80],
            ToastLevel::Error => [220, 80, 80],
        }
    }

    /// Phosphor icon *name* (kebab-case) for this level. A name-based renderer
    /// (e.g. `renzora_ember::font::icon_glyph`) resolves it to a glyph.
    pub fn icon(&self) -> &'static str {
        match self {
            ToastLevel::Info => "info",
            ToastLevel::Success => "check-circle",
            ToastLevel::Warning => "warning",
            ToastLevel::Error => "x-circle",
        }
    }
}

/// A single toast notification.
#[derive(Clone)]
pub struct Toast {
    pub message: String,
    pub level: ToastLevel,
    pub created: f64,
    pub duration: f64,
}

const DEFAULT_DURATION: f64 = 3.0;

/// Resource that stores active toast notifications.
#[derive(Resource, Default)]
pub struct Toasts {
    entries: Vec<Toast>,
}

impl Toasts {
    pub fn info(&mut self, message: impl Into<String>) {
        self.add(ToastLevel::Info, message);
    }

    pub fn success(&mut self, message: impl Into<String>) {
        self.add(ToastLevel::Success, message);
    }

    pub fn warning(&mut self, message: impl Into<String>) {
        self.add(ToastLevel::Warning, message);
    }

    pub fn error(&mut self, message: impl Into<String>) {
        self.add(ToastLevel::Error, message);
    }

    pub fn add(&mut self, level: ToastLevel, message: impl Into<String>) {
        self.entries.push(Toast {
            message: message.into(),
            level,
            created: 0.0, // filled in by the consumer when first shown
            duration: DEFAULT_DURATION,
        });
    }

    /// Whether any toasts are queued.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Borrow the queued toasts (e.g. for a native renderer).
    pub fn entries(&self) -> &[Toast] {
        &self.entries
    }

    /// Mutably borrow the queued toasts (e.g. to stamp creation time or prune
    /// expired entries from a native renderer).
    pub fn entries_mut(&mut self) -> &mut Vec<Toast> {
        &mut self.entries
    }

    /// Remove and return all queued toasts.
    pub fn drain(&mut self) -> Vec<Toast> {
        std::mem::take(&mut self.entries)
    }
}
