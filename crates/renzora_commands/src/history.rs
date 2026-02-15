//! Command history for undo/redo functionality.

#![allow(dead_code)]

use bevy::prelude::*;
use super::command::{Command, CommandContext, CommandGroup, CommandResult};

/// Maximum number of commands to keep in undo history
pub const MAX_UNDO_HISTORY: usize = 100;

/// Resource that tracks command history for undo/redo
#[derive(Resource, Default)]
pub struct CommandHistory {
    /// Stack of executed commands (most recent at the end)
    undo_stack: Vec<Box<dyn Command>>,
    /// Stack of undone commands (most recent at the end)
    redo_stack: Vec<Box<dyn Command>>,
    /// Commands queued to be executed at the end of the frame
    pub pending_commands: Vec<Box<dyn Command>>,
    /// Whether we're currently executing an undo/redo (to prevent recursion)
    in_undo_redo: bool,
    /// If set, commands will be grouped together until end_group is called
    active_group: Option<CommandGroup>,
    /// Pending undo count (from menu or other UI) - number of undos to perform
    pub pending_undo: usize,
    /// Pending redo count (from menu or other UI) - number of redos to perform
    pub pending_redo: usize,
}

impl CommandHistory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push an executed command onto the undo stack
    pub fn push_executed(&mut self, command: Box<dyn Command>) {
        // Clear redo stack when new command is executed
        if !self.in_undo_redo {
            self.redo_stack.clear();
        }

        // Try to merge with the last command if possible
        if let Some(last) = self.undo_stack.last_mut() {
            if last.can_merge(&*command) {
                last.merge(command);
                return;
            }
        }

        self.undo_stack.push(command);

        // Limit history size
        while self.undo_stack.len() > MAX_UNDO_HISTORY {
            self.undo_stack.remove(0);
        }
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Get the description of the next undo command
    pub fn undo_description(&self) -> Option<String> {
        self.undo_stack.last().map(|c| c.description())
    }

    /// Get the description of the next redo command
    pub fn redo_description(&self) -> Option<String> {
        self.redo_stack.last().map(|c| c.description())
    }

    /// Get the number of commands in the undo stack
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    /// Get the number of commands in the redo stack
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    /// Get all undo descriptions (oldest to newest)
    pub fn undo_descriptions(&self) -> Vec<String> {
        self.undo_stack.iter().map(|c| c.description()).collect()
    }

    /// Get all redo descriptions (oldest to newest, so newest undone first)
    pub fn redo_descriptions(&self) -> Vec<String> {
        self.redo_stack.iter().map(|c| c.description()).collect()
    }

    /// Pop command from undo stack for undoing
    pub fn pop_for_undo(&mut self) -> Option<Box<dyn Command>> {
        self.undo_stack.pop()
    }

    /// Push command onto redo stack after undoing
    pub fn push_to_redo(&mut self, command: Box<dyn Command>) {
        self.redo_stack.push(command);
    }

    /// Pop command from redo stack for redoing
    pub fn pop_for_redo(&mut self) -> Option<Box<dyn Command>> {
        self.redo_stack.pop()
    }

    /// Push command onto undo stack after redoing
    pub fn push_to_undo(&mut self, command: Box<dyn Command>) {
        self.undo_stack.push(command);
    }

    /// Set in_undo_redo flag
    pub fn set_in_undo_redo(&mut self, value: bool) {
        self.in_undo_redo = value;
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// Start a command group - subsequent commands will be grouped together
    pub fn begin_group(&mut self, description: impl Into<String>) {
        if self.active_group.is_none() {
            self.active_group = Some(CommandGroup::new(description));
        }
    }

    /// End the current command group
    pub fn end_group(&mut self) {
        if let Some(group) = self.active_group.take() {
            if !group.is_empty() {
                self.push_executed(Box::new(group));
            }
        }
    }

    /// Check if a group is currently active
    pub fn is_grouping(&self) -> bool {
        self.active_group.is_some()
    }
}

/// Perform undo operation
pub fn undo(world: &mut World) -> bool {
    let command = {
        let mut history = world.resource_mut::<CommandHistory>();
        if !history.can_undo() {
            return false;
        }
        history.set_in_undo_redo(true);
        history.pop_for_undo()
    };

    let Some(mut command) = command else {
        return false;
    };

    let mut ctx = CommandContext { world };
    let result = command.undo(&mut ctx);

    {
        let mut history = ctx.world.resource_mut::<CommandHistory>();
        history.set_in_undo_redo(false);
        match result {
            CommandResult::Success | CommandResult::NoOp => {
                history.push_to_redo(command);
                true
            }
            CommandResult::Failed(e) => {
                error!("Undo failed: {}", e);
                false
            }
        }
    }
}

/// Perform redo operation
pub fn redo(world: &mut World) -> bool {
    let command = {
        let mut history = world.resource_mut::<CommandHistory>();
        if !history.can_redo() {
            return false;
        }
        history.set_in_undo_redo(true);
        history.pop_for_redo()
    };

    let Some(mut command) = command else {
        return false;
    };

    let mut ctx = CommandContext { world };
    let result = command.redo(&mut ctx);

    {
        let mut history = ctx.world.resource_mut::<CommandHistory>();
        history.set_in_undo_redo(false);
        match result {
            CommandResult::Success | CommandResult::NoOp => {
                history.push_to_undo(command);
                true
            }
            CommandResult::Failed(e) => {
                error!("Redo failed: {}", e);
                false
            }
        }
    }
}
