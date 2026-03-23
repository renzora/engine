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
