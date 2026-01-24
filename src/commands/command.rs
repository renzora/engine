//! Core command trait and types for the undo/redo system.

use bevy::prelude::*;
use std::any::Any;

/// Context provided to commands for execution
pub struct CommandContext<'a> {
    pub world: &'a mut World,
}

/// Result of executing a command
pub enum CommandResult {
    /// Command executed successfully
    Success,
    /// Command had no effect (don't add to history)
    NoOp,
    /// Command failed with an error message
    Failed(String),
}

/// A command that can be executed, undone, and redone.
///
/// Commands should be self-contained and store all data needed for undo/redo.
/// They should NOT store references to world data - only entity IDs and values.
pub trait Command: Send + Sync + Any {
    /// Returns a description of this command for display in UI
    fn description(&self) -> String;

    /// Execute the command. Called when the command is first run.
    /// Should store any data needed for undo.
    fn execute(&mut self, ctx: &mut CommandContext) -> CommandResult;

    /// Undo the command. Restore state to before execute() was called.
    fn undo(&mut self, ctx: &mut CommandContext) -> CommandResult;

    /// Redo the command. Re-apply the command after it was undone.
    /// Default implementation just calls execute() again.
    fn redo(&mut self, ctx: &mut CommandContext) -> CommandResult {
        self.execute(ctx)
    }

    /// Returns true if this command can be merged with the given command.
    /// Used for combining rapid successive changes (e.g., dragging a slider).
    fn can_merge(&self, _other: &dyn Command) -> bool {
        false
    }

    /// Merge another command into this one. Only called if can_merge() returns true.
    fn merge(&mut self, _other: Box<dyn Command>) {
        // Default: do nothing
    }

    /// Get the command type name for debugging
    fn command_type(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// A group of commands that are executed/undone together as a single unit.
pub struct CommandGroup {
    description: String,
    commands: Vec<Box<dyn Command>>,
}

impl CommandGroup {
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            commands: Vec::new(),
        }
    }

    pub fn add(&mut self, command: Box<dyn Command>) {
        self.commands.push(command);
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

impl Command for CommandGroup {
    fn description(&self) -> String {
        self.description.clone()
    }

    fn execute(&mut self, ctx: &mut CommandContext) -> CommandResult {
        for cmd in &mut self.commands {
            if let CommandResult::Failed(e) = cmd.execute(ctx) {
                return CommandResult::Failed(e);
            }
        }
        if self.commands.is_empty() {
            CommandResult::NoOp
        } else {
            CommandResult::Success
        }
    }

    fn undo(&mut self, ctx: &mut CommandContext) -> CommandResult {
        // Undo in reverse order
        for cmd in self.commands.iter_mut().rev() {
            if let CommandResult::Failed(e) = cmd.undo(ctx) {
                return CommandResult::Failed(e);
            }
        }
        CommandResult::Success
    }

    fn redo(&mut self, ctx: &mut CommandContext) -> CommandResult {
        for cmd in &mut self.commands {
            if let CommandResult::Failed(e) = cmd.redo(ctx) {
                return CommandResult::Failed(e);
            }
        }
        CommandResult::Success
    }
}
