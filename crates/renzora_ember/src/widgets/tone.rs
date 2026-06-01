//! Semantic tone shared by badges / alerts / toasts / validation.

use crate::theme::{ACCENT_BLUE, CLOSE_RED, PLAY_GREEN, WARN_AMBER};

/// Semantic tone for feedback components.
#[derive(Clone, Copy)]
pub enum Tone {
    Neutral,
    Info,
    Success,
    Warn,
    Error,
}

impl Tone {
    pub(crate) fn color(self) -> (u8, u8, u8) {
        match self {
            Tone::Neutral => (120, 120, 134),
            Tone::Info => ACCENT_BLUE,
            Tone::Success => PLAY_GREEN,
            Tone::Warn => WARN_AMBER,
            Tone::Error => CLOSE_RED,
        }
    }
    pub(crate) fn icon(self) -> &'static str {
        match self {
            Tone::Neutral => "info",
            Tone::Info => "info",
            Tone::Success => "check-circle",
            Tone::Warn => "warning",
            Tone::Error => "x-circle",
        }
    }
}
