//! Renzora Keybindings — configurable keyboard shortcuts for the editor.
//!
//! The type definitions (EditorAction, KeyBinding, KeyBindings) now live in
//! `renzora_core::keybindings` so that other editor plugin DLLs can use them
//! without depending on this crate. This crate re-exports everything and
//! provides the plugin that initializes the resource.

use bevy::prelude::*;

// Re-export all types from renzora_core::keybindings
pub use renzora::core::keybindings::*;

#[derive(Default)]
pub struct KeybindingsPlugin;

impl Plugin for KeybindingsPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] KeybindingsPlugin");
        app.init_resource::<KeyBindings>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_actions_have_display_names() {
        for action in EditorAction::all() {
            let name = action.display_name();
            assert!(!name.is_empty(), "{:?} has empty display_name", action);
        }
    }

    #[test]
    fn test_all_actions_have_categories() {
        for action in EditorAction::all() {
            let cat = action.category();
            assert!(!cat.is_empty(), "{:?} has empty category", action);
        }
    }

    #[test]
    fn test_editor_action_count() {
        let all = EditorAction::all();
        assert!(all.len() >= 30, "Expected at least 30 actions, got {}", all.len());
    }

    #[test]
    fn test_valid_categories() {
        let valid = ["Camera", "Tools", "Transform", "Selection", "Edit", "File", "View", "Play"];
        for action in EditorAction::all() {
            let cat = action.category();
            assert!(valid.contains(&cat), "{:?} has unexpected category '{}'", action, cat);
        }
    }

    #[test]
    fn test_keybinding_new_no_modifiers() {
        let kb = KeyBinding::new(KeyCode::KeyA);
        assert_eq!(kb.key, KeyCode::KeyA);
        assert!(!kb.ctrl);
        assert!(!kb.shift);
        assert!(!kb.alt);
    }

    #[test]
    fn test_keybinding_ctrl() {
        let kb = KeyBinding::new(KeyCode::KeyZ).ctrl();
        assert!(kb.ctrl);
        assert!(!kb.shift);
        assert!(!kb.alt);
    }

    #[test]
    fn test_keybinding_shift() {
        let kb = KeyBinding::new(KeyCode::KeyS).shift();
        assert!(!kb.ctrl);
        assert!(kb.shift);
        assert!(!kb.alt);
    }

    #[test]
    fn test_keybinding_alt() {
        let kb = KeyBinding::new(KeyCode::KeyD).alt();
        assert!(!kb.ctrl);
        assert!(!kb.shift);
        assert!(kb.alt);
    }

    #[test]
    fn test_keybinding_chaining() {
        let kb = KeyBinding::new(KeyCode::KeyS).ctrl().shift();
        assert!(kb.ctrl);
        assert!(kb.shift);
        assert!(!kb.alt);
    }

    #[test]
    fn test_display_simple_key() {
        let kb = KeyBinding::new(KeyCode::KeyA);
        assert_eq!(kb.display(), "A");
    }

    #[test]
    fn test_display_ctrl_z() {
        let kb = KeyBinding::new(KeyCode::KeyZ).ctrl();
        assert_eq!(kb.display(), "Ctrl + Z");
    }

    #[test]
    fn test_display_ctrl_shift_s() {
        let kb = KeyBinding::new(KeyCode::KeyS).ctrl().shift();
        assert_eq!(kb.display(), "Ctrl + Shift + S");
    }

    #[test]
    fn test_display_f5() {
        let kb = KeyBinding::new(KeyCode::F5);
        assert_eq!(kb.display(), "F5");
    }

    #[test]
    fn test_display_all_modifiers() {
        let kb = KeyBinding::new(KeyCode::KeyX).ctrl().alt().shift();
        assert_eq!(kb.display(), "Ctrl + Alt + Shift + X");
    }

    #[test]
    fn test_all_actions_have_bindings() {
        let bindings = KeyBindings::default();
        let mut missing = Vec::new();
        for action in EditorAction::all() {
            if bindings.get(action).is_none() {
                missing.push(format!("{:?}", action));
            }
        }
        assert!(missing.is_empty(), "Actions missing default bindings: {}", missing.join(", "));
    }

    #[test]
    fn test_undo_is_ctrl_z() {
        let bindings = KeyBindings::default();
        let kb = bindings.get(EditorAction::Undo).unwrap();
        assert_eq!(kb.key, KeyCode::KeyZ);
        assert!(kb.ctrl);
        assert!(!kb.shift);
    }

    #[test]
    fn test_save_scene_is_ctrl_s() {
        let bindings = KeyBindings::default();
        let kb = bindings.get(EditorAction::SaveScene).unwrap();
        assert_eq!(kb.key, KeyCode::KeyS);
        assert!(kb.ctrl);
        assert!(!kb.shift);
    }

    #[test]
    fn test_play_stop_is_f5() {
        let bindings = KeyBindings::default();
        let kb = bindings.get(EditorAction::PlayStop).unwrap();
        assert_eq!(kb.key, KeyCode::F5);
        assert!(!kb.ctrl);
        assert!(!kb.shift);
    }

    #[test]
    fn test_key_name_letters() {
        assert_eq!(key_name(KeyCode::KeyA), "A");
        assert_eq!(key_name(KeyCode::KeyZ), "Z");
    }

    #[test]
    fn test_key_name_special() {
        assert_eq!(key_name(KeyCode::Escape), "Esc");
        assert_eq!(key_name(KeyCode::Space), "Space");
        assert_eq!(key_name(KeyCode::Delete), "Delete");
        assert_eq!(key_name(KeyCode::F1), "F1");
        assert_eq!(key_name(KeyCode::F12), "F12");
    }

    #[test]
    fn test_key_name_numpad() {
        assert_eq!(key_name(KeyCode::Numpad0), "Num0");
        assert_eq!(key_name(KeyCode::Numpad9), "Num9");
    }

    #[test]
    fn test_set_binding() {
        let mut bindings = KeyBindings::default();
        let new_binding = KeyBinding::new(KeyCode::KeyP).ctrl();
        bindings.set(EditorAction::PlayStop, new_binding);
        let kb = bindings.get(EditorAction::PlayStop).unwrap();
        assert_eq!(kb.key, KeyCode::KeyP);
        assert!(kb.ctrl);
    }

    #[test]
    fn test_default_no_rebinding() {
        let bindings = KeyBindings::default();
        assert!(bindings.rebinding.is_none());
    }
}

renzora::add!(KeybindingsPlugin);
