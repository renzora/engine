//! Generic command-based undo/redo framework.
//!
//! Provides the core `Command` trait, command history tracking, and
//! undo/redo execution. Concrete command implementations live in the
//! editor crate.

#![allow(dead_code)]

mod command;
mod history;

#[cfg(test)]
mod tests;

pub use command::{Command, CommandContext, CommandGroup, CommandResult};
pub use history::{CommandHistory, undo, redo, MAX_UNDO_HISTORY};

use bevy::prelude::*;

/// Execute a command immediately and add it to history.
pub fn execute_command(world: &mut World, command: Box<dyn Command>) {
    execute_command_internal(world, command);
}

/// Queue a command to be executed at the end of the frame.
pub fn queue_command(history: &mut CommandHistory, command: Box<dyn Command>) {
    history.pending_commands.push(command);
}

/// Internal: execute a command and push to history on success.
fn execute_command_internal(world: &mut World, mut command: Box<dyn Command>) {
    let mut ctx = CommandContext { world };

    match command.execute(&mut ctx) {
        CommandResult::Success => {
            let mut history = ctx.world.resource_mut::<CommandHistory>();
            history.push_executed(command);
        }
        CommandResult::NoOp => {}
        CommandResult::Failed(err) => {
            error!("Command failed: {}", err);
        }
    }
}
