//! Plugin-facing keyboard shortcut registry.
//!
//! Plugins register shortcuts with [`App::register_shortcut`] (see
//! [`crate::AppEditorExt`]). Each registered entry:
//!
//!   1. Records display metadata (name + category) in [`ShortcutRegistry`]
//!      so the Settings → Shortcuts tab can list it alongside built-in actions.
//!   2. Seeds its default binding into `KeyBindings.plugin_bindings` so
//!      users can rebind it via the same UI that handles built-ins.
//!   3. Installs a handler that fires when the (currently bound, user may
//!      have rebound it) key chord is pressed.

use bevy::prelude::*;
use renzora::keybindings::{KeyBinding, KeyBindings};
use std::sync::Arc;

pub type ShortcutHandler = Arc<dyn Fn(&mut World) + Send + Sync>;

/// Metadata + handler for one plugin-registered shortcut.
#[derive(Clone)]
pub struct ShortcutEntry {
    /// Stable id (e.g. `"mesh_draw.box"`). Key for lookup in
    /// [`KeyBindings::plugin_bindings`].
    pub id: &'static str,
    /// Display name shown in Settings → Shortcuts.
    pub display_name: &'static str,
    /// Category label shown in Settings → Shortcuts. Use one of the
    /// existing categories ("Camera", "Tools", "Transform", ...) for grouping
    /// with built-ins, or any string for a plugin-only category.
    pub category: &'static str,
    /// Default binding inserted into [`KeyBindings`] if no user-set binding
    /// exists yet.
    pub default_binding: KeyBinding,
    /// Handler invoked with `&mut World` when the (possibly user-rebound)
    /// chord fires. Free to mutate resources directly or push
    /// `EditorCommands`.
    pub handler: ShortcutHandler,
}

impl ShortcutEntry {
    pub fn new(
        id: &'static str,
        display_name: &'static str,
        category: &'static str,
        default_binding: KeyBinding,
        handler: impl Fn(&mut World) + Send + Sync + 'static,
    ) -> Self {
        Self {
            id,
            display_name,
            category,
            default_binding,
            handler: Arc::new(handler),
        }
    }
}

#[derive(Resource, Default, Clone)]
pub struct ShortcutRegistry {
    entries: Vec<ShortcutEntry>,
}

impl ShortcutRegistry {
    pub fn register(&mut self, entry: ShortcutEntry) {
        // Dedup by id — if a plugin is reloaded, replace in place.
        if let Some(idx) = self.entries.iter().position(|e| e.id == entry.id) {
            self.entries[idx] = entry;
        } else {
            self.entries.push(entry);
        }
    }

    pub fn entries(&self) -> &[ShortcutEntry] {
        &self.entries
    }
}

/// Fire handlers whose currently-bound chord was just pressed this frame.
/// Skips firing while egui has keyboard focus (typing in text fields).
pub fn shortcut_dispatch_system(world: &mut World) {
    let has_focus = world
        .get_resource::<renzora::InputFocusState>()
        .map_or(false, |f| f.egui_wants_keyboard);
    if has_focus {
        return;
    }

    let (ctrl, shift, alt) = {
        let keys = world.resource::<ButtonInput<KeyCode>>();
        (
            keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight),
            keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight),
            keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight),
        )
    };

    // Collect matching handlers (snapshot before invoking — each handler
    // gets `&mut World` so we can't hold registry / keybindings borrows).
    let fired: Vec<ShortcutHandler> = {
        let Some(registry) = world.get_resource::<ShortcutRegistry>() else { return };
        let Some(bindings) = world.get_resource::<KeyBindings>() else { return };
        let keys = world.resource::<ButtonInput<KeyCode>>();

        registry
            .entries
            .iter()
            .filter_map(|e| {
                // Programmatic dispatch (command palette etc.) fires
                // handlers directly without needing the chord to be pressed.
                if bindings.is_plugin_dispatched(e.id) {
                    return Some(e.handler.clone());
                }
                let b = bindings.plugin_bindings.get(e.id).unwrap_or(&e.default_binding);
                let matches = keys.just_pressed(b.key)
                    && b.ctrl == ctrl
                    && b.shift == shift
                    && b.alt == alt;
                if matches { Some(e.handler.clone()) } else { None }
            })
            .collect()
    };

    for handler in fired {
        (handler)(world);
    }
}
