//! Runtime warning capture for the Scene Diagnostics panel.
//!
//! Installs a `tracing_subscriber::Layer` (via `LogPlugin::custom_layer`
//! in renzora_runtime) that intercepts WARN and ERROR events from
//! Bevy's tracing subscriber and pushes them into a global ring buffer.
//! The diagnostics panel reads the buffer each frame and renders the
//! most recent entries.
//!
//! Capturing tracing events (rather than only explicit `console_log!`
//! calls) is what lets us surface things we don't control:
//! `bevy_ecs::hierarchy` B0004 warnings, `bevy_render` GPU validation
//! errors, gltf loader warnings, `try_insert` failures from command
//! handlers, etc.
//!
//! Output is **separate** from the existing `console_log` buffer so the
//! editor's console panel doesn't get spammed with every bevy log line.

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use bevy::log::tracing::field::{Field, Visit};
use bevy::log::tracing::{Event, Subscriber};
use bevy::log::tracing_subscriber::layer::{Context, Layer};
use bevy::log::BoxedLayer;
use bevy::prelude::*;

/// How many warning entries to retain. Old entries are dropped when the
/// buffer hits this size. 200 is enough for the panel to show a healthy
/// scroll of history without growing unbounded over a long session.
pub const MAX_WARNINGS: usize = 200;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WarningLevel {
    Warn,
    Error,
}

#[derive(Clone, Debug)]
pub struct CapturedWarning {
    /// `Instant` (not wall-clock) so we can compute age cheaply for
    /// the panel without dragging a Time dependency through every
    /// caller.
    pub timestamp: Instant,
    pub level: WarningLevel,
    /// `tracing` target — usually the source crate's module path,
    /// e.g. `bevy_ecs::hierarchy`. Lets the panel group/filter.
    pub target: String,
    pub message: String,
}

impl CapturedWarning {
    pub fn age(&self) -> Duration {
        self.timestamp.elapsed()
    }
}

/// Global capture buffer. `OnceLock<Mutex<VecDeque>>` so the layer
/// (which runs from arbitrary threads, including bevy's render world)
/// and the panel (which runs on the main thread) can both touch it
/// without each crate holding a `Resource` clone.
static BUFFER: OnceLock<Mutex<VecDeque<CapturedWarning>>> = OnceLock::new();

fn buffer() -> &'static Mutex<VecDeque<CapturedWarning>> {
    BUFFER.get_or_init(|| Mutex::new(VecDeque::with_capacity(MAX_WARNINGS)))
}

/// Snapshot the buffer's entries (newest last). Cheap clone — entries
/// are small. Called by the diagnostics panel each frame.
pub fn recent_warnings() -> Vec<CapturedWarning> {
    buffer()
        .lock()
        .map(|b| b.iter().cloned().collect())
        .unwrap_or_default()
}

struct WarningCaptureLayer;

impl<S: Subscriber> Layer<S> for WarningCaptureLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        use bevy::log::tracing::Level;

        let metadata = event.metadata();
        let level = match *metadata.level() {
            Level::WARN => WarningLevel::Warn,
            Level::ERROR => WarningLevel::Error,
            // Skip INFO/DEBUG/TRACE — only true problems land in the
            // diagnostics feed.
            _ => return,
        };

        let mut message = String::new();
        event.record(&mut MessageVisitor(&mut message));
        if message.is_empty() {
            // Some events carry no `message` field (only structured
            // fields). Fall back to the event's name for context.
            message = metadata.name().to_string();
        }

        let entry = CapturedWarning {
            timestamp: Instant::now(),
            level,
            target: metadata.target().to_string(),
            message,
        };

        if let Ok(mut buf) = buffer().lock() {
            buf.push_back(entry);
            while buf.len() > MAX_WARNINGS {
                buf.pop_front();
            }
        }
    }
}

/// Field visitor that extracts the `message` field from a tracing
/// event. Tracing routes the format string through either `record_str`
/// (for plain `&str` Display values) or `record_debug` (Debug
/// formatting), so we implement both.
struct MessageVisitor<'a>(&'a mut String);

impl<'a> Visit for MessageVisitor<'a> {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.0.push_str(value);
        }
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            // `{:?}` on a `Display`-backed argument writes the message
            // with surrounding quotes (Bevy emits messages this way).
            // Format directly and trim the quote pair.
            let raw = format!("{value:?}");
            let trimmed = raw
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .unwrap_or(&raw);
            self.0.push_str(trimmed);
        }
    }
}

/// Factory for `LogPlugin::custom_layer`. Returns the capture layer so
/// renzora_runtime can install it alongside Bevy's default log
/// formatter. Returns `Some` unconditionally; no runtime config needed.
pub fn runtime_warnings_layer(_app: &mut App) -> Option<BoxedLayer> {
    Some(Box::new(WarningCaptureLayer))
}
