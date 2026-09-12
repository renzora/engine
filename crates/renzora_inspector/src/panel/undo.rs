//! Undo commands for the structural edits the inspector makes: adding a
//! component, removing one, and flipping a section's enable switch.
//!
//! Field edits go through [`record_field_change`](super::record_field_change)
//! instead, which records `renzora_undo::FieldChangeCmd`.

use bevy::prelude::*;

use super::{Mutate, SetEnabled};

/// Undo for enabling/disabling a component from its section header toggle.
pub(crate) struct EnableToggleCmd {
    pub(crate) entity: Entity,
    pub(crate) set_enabled: SetEnabled,
    pub(crate) target: bool,
}

impl renzora_undo::UndoCommand for EnableToggleCmd {
    fn label(&self) -> &str {
        "Toggle component"
    }
    fn execute(&mut self, world: &mut World) {
        (self.set_enabled)(world, self.entity, self.target);
    }
    fn undo(&mut self, world: &mut World) {
        (self.set_enabled)(world, self.entity, !self.target);
    }
}

/// Undo for adding a component: `undo` removes it again (redo re-adds a default,
/// same as the original add).
pub(crate) struct AddComponentCmd {
    pub(crate) entity: Entity,
    pub(crate) add_fn: Mutate,
    pub(crate) remove_fn: Option<Mutate>,
}

impl renzora_undo::UndoCommand for AddComponentCmd {
    fn label(&self) -> &str {
        "Add component"
    }
    fn execute(&mut self, world: &mut World) {
        (self.add_fn)(world, self.entity);
    }
    fn undo(&mut self, world: &mut World) {
        if let Some(remove_fn) = self.remove_fn.clone() {
            remove_fn(world, self.entity);
        }
    }
}

/// Undo for removing a component: captures the component's reflected value before
/// removing, so `undo` restores it *with its edited fields* (not a default). Redo
/// (`execute`) re-captures the current value and removes again.
pub(crate) struct RemoveComponentCmd {
    pub(crate) entity: Entity,
    pub(crate) type_id: &'static str,
    pub(crate) remove_fn: Mutate,
    pub(crate) captured: Option<Box<dyn bevy::reflect::Reflect>>,
}

impl renzora_undo::UndoCommand for RemoveComponentCmd {
    fn label(&self) -> &str {
        "Remove component"
    }
    fn execute(&mut self, world: &mut World) {
        self.captured = renzora::core::reflection::capture_component(world, self.entity, self.type_id);
        (self.remove_fn)(world, self.entity);
    }
    fn undo(&mut self, world: &mut World) {
        if let Some(value) = &self.captured {
            renzora::core::reflection::insert_component_reflected(
                world,
                self.entity,
                self.type_id,
                value.as_ref(),
            );
        }
    }
}
