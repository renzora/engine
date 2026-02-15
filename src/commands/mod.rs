//! Command-based undo/redo system for the editor.
//!
//! All scene modifications should go through this system to be undoable.
//! Commands encapsulate both the action and the data needed to reverse it.

#![allow(dead_code)]

mod entity_commands;

// Re-export the generic command framework from the crate
pub use renzora_commands::{
    Command, CommandContext, CommandGroup, CommandResult,
    CommandHistory, undo, redo, execute_command, queue_command,
};
pub use entity_commands::*;

use bevy::prelude::*;

use crate::core::{KeyBindings, EditorAction};

/// Plugin that sets up the command/undo system
pub struct CommandPlugin;

impl Plugin for CommandPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CommandHistory>()
            .add_systems(
                Update,
                (
                    handle_undo_redo_shortcuts,
                    process_pending_commands,
                )
                    .chain()
                    .run_if(in_state(crate::core::AppState::Editor))
            );
    }
}

/// System that handles undo/redo keyboard shortcuts and pending requests
fn handle_undo_redo_shortcuts(world: &mut World) {
    // First, check for pending requests from menu (needs mutable borrow)
    let (pending_undo_count, pending_redo_count) = {
        let mut history = world.resource_mut::<CommandHistory>();
        let undo_count = history.pending_undo;
        let redo_count = history.pending_redo;
        history.pending_undo = 0;
        history.pending_redo = 0;
        (undo_count, redo_count)
    };

    // Then check keyboard shortcuts (needs immutable borrows)
    let (keyboard_undo, keyboard_redo) = {
        let keyboard = world.resource::<ButtonInput<KeyCode>>();
        let keybindings = world.resource::<KeyBindings>();

        // Don't process keyboard while rebinding
        if keybindings.rebinding.is_some() {
            (false, false)
        } else {
            (
                keybindings.just_pressed(EditorAction::Undo, keyboard),
                keybindings.just_pressed(EditorAction::Redo, keyboard),
            )
        }
    };

    // Calculate total undo/redo operations
    let undo_count = pending_undo_count + if keyboard_undo { 1 } else { 0 };
    let redo_count = pending_redo_count + if keyboard_redo { 1 } else { 0 };

    // Process undos
    if undo_count > 0 {
        for _ in 0..undo_count {
            if !undo(world) {
                break;
            }
        }
        if undo_count > 1 {
            info!("Undo x{}", undo_count);
        } else {
            info!("Undo");
        }
        return; // Don't process redo in the same frame
    }

    // Process redos
    if redo_count > 0 {
        for _ in 0..redo_count {
            if !redo(world) {
                break;
            }
        }
        if redo_count > 1 {
            info!("Redo x{}", redo_count);
        } else {
            info!("Redo");
        }
    }
}

/// System that processes any pending commands queued during the frame
fn process_pending_commands(world: &mut World) {
    // Extract pending commands from history
    let pending: Vec<Box<dyn Command>> = {
        let mut history = world.resource_mut::<CommandHistory>();
        std::mem::take(&mut history.pending_commands)
    };

    // Execute each pending command
    for command in pending {
        execute_command(world, command);
    }
}
