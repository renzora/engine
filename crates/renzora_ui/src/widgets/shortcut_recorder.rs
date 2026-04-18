//! Keyboard-shortcut recorder — clicks into record mode, next pressed chord
//! is captured and returned.

use bevy_egui::egui::{self, Color32, Key, Modifiers};
use renzora_theme::Theme;

/// A recorded chord: modifiers + key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shortcut {
    pub modifiers: Modifiers,
    pub key: Key,
}

impl Shortcut {
    pub fn format(&self) -> String {
        let mut parts = Vec::new();
        if self.modifiers.ctrl {
            parts.push("Ctrl");
        }
        if self.modifiers.shift {
            parts.push("Shift");
        }
        if self.modifiers.alt {
            parts.push("Alt");
        }
        if self.modifiers.mac_cmd || self.modifiers.command {
            parts.push("Cmd");
        }
        let mut s = parts.join("+");
        if !s.is_empty() {
            s.push('+');
        }
        s.push_str(&format!("{:?}", self.key));
        s
    }
}

/// Render a shortcut recorder button. Click to enter record mode; next
/// keypress is captured. Returns the captured shortcut this frame (or `None`).
pub fn shortcut_recorder(
    ui: &mut egui::Ui,
    id: egui::Id,
    current: Option<Shortcut>,
    theme: &Theme,
) -> Option<Shortcut> {
    let mut recording = ui
        .ctx()
        .memory_mut(|m| m.data.get_temp::<bool>(id).unwrap_or(false));
    let label = if recording {
        "Press a key…".to_string()
    } else {
        current.map(|s| s.format()).unwrap_or_else(|| "(unbound)".to_string())
    };

    let resp = ui.add(
        egui::Button::new(
            egui::RichText::new(label).color(if recording {
                theme.widgets.active_bg.to_color32()
            } else {
                theme.text.primary.to_color32()
            }),
        )
        .min_size(egui::vec2(140.0, 20.0)),
    );
    if resp.clicked() {
        recording = !recording;
    }

    let mut captured = None;
    if recording {
        let input = ui.input(|i| i.clone());
        for event in &input.events {
            if let egui::Event::Key { key, pressed: true, modifiers, .. } = event {
                captured = Some(Shortcut { modifiers: *modifiers, key: *key });
                recording = false;
                break;
            }
        }
        // Escape cancels
        if input.key_pressed(Key::Escape) {
            recording = false;
            captured = None;
        }
    }
    ui.ctx().memory_mut(|m| m.data.insert_temp(id, recording));
    let _ = Color32::WHITE;
    captured
}
