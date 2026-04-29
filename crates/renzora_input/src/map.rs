use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::action::InputAction;

/// The project's input action definitions.
///
/// Stored as a resource and serialized to `input_map.ron` in the project.
#[derive(Resource, Clone, Debug, Serialize, Deserialize, Reflect)]
pub struct InputMap {
    pub actions: Vec<InputAction>,
}

impl Default for InputMap {
    fn default() -> Self {
        Self {
            actions: crate::default::default_actions(),
        }
    }
}

impl InputMap {
    /// Look up an action by name.
    pub fn get(&self, name: &str) -> Option<&InputAction> {
        self.actions.iter().find(|a| a.name == name)
    }

    /// Add an action. Returns false if the name already exists.
    pub fn add(&mut self, action: InputAction) -> bool {
        if self.actions.iter().any(|a| a.name == action.name) {
            return false;
        }
        self.actions.push(action);
        true
    }

    /// Remove an action by name.
    pub fn remove(&mut self, name: &str) -> bool {
        let len = self.actions.len();
        self.actions.retain(|a| a.name != name);
        self.actions.len() < len
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{ActionKind, InputBinding};
    use bevy::prelude::KeyCode;

    fn jump_action() -> InputAction {
        InputAction::button("jump", vec![InputBinding::key(KeyCode::Space)])
    }

    #[test]
    fn default_input_map_has_actions() {
        // Default map ships with the engine's standard action set so a
        // freshly created project isn't bound to nothing.
        let map = InputMap::default();
        assert!(!map.actions.is_empty());
    }

    #[test]
    fn get_returns_matching_action() {
        let mut map = InputMap { actions: Vec::new() };
        map.actions.push(jump_action());
        let found = map.get("jump").expect("jump should exist");
        assert_eq!(found.name, "jump");
        assert_eq!(found.kind, ActionKind::Button);
    }

    #[test]
    fn get_missing_returns_none() {
        let map = InputMap { actions: Vec::new() };
        assert!(map.get("nope").is_none());
    }

    #[test]
    fn add_succeeds_on_unique_name() {
        let mut map = InputMap { actions: Vec::new() };
        assert!(map.add(jump_action()));
        assert_eq!(map.actions.len(), 1);
    }

    #[test]
    fn add_rejects_duplicate_name() {
        // The map relies on unique action names — a second `add` for the
        // same name must report failure rather than silently double up.
        let mut map = InputMap { actions: Vec::new() };
        assert!(map.add(jump_action()));
        assert!(!map.add(jump_action()));
        assert_eq!(map.actions.len(), 1);
    }

    #[test]
    fn remove_returns_true_when_present() {
        let mut map = InputMap { actions: vec![jump_action()] };
        assert!(map.remove("jump"));
        assert!(map.actions.is_empty());
    }

    #[test]
    fn remove_returns_false_when_missing() {
        let mut map = InputMap { actions: Vec::new() };
        assert!(!map.remove("never_added"));
    }

    #[test]
    fn ron_round_trip_preserves_actions() {
        // Maps live on disk as RON files. A round-trip must preserve the
        // action set exactly so users don't lose bindings on save.
        let original = InputMap {
            actions: vec![
                jump_action(),
                InputAction::axis_2d(
                    "move",
                    vec![InputBinding::composite_2d(
                        KeyCode::KeyW, KeyCode::KeyS,
                        KeyCode::KeyA, KeyCode::KeyD,
                    )],
                    0.1,
                ),
            ],
        };
        let serialized = ron::to_string(&original).expect("serialize");
        let parsed: InputMap = ron::from_str(&serialized).expect("parse");
        assert_eq!(parsed.actions.len(), original.actions.len());
        assert_eq!(parsed.actions[0].name, "jump");
        assert_eq!(parsed.actions[1].kind, ActionKind::Axis2D);
        assert!((parsed.actions[1].dead_zone - 0.1).abs() < 1e-6);
    }
}
