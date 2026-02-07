//! Tests for the command/undo system
//!
//! Covers CommandHistory state queries and CommandGroup.
//! Uses pure state checks without needing a Bevy World.

use super::command::{Command, CommandGroup, CommandResult, CommandContext};
use super::history::CommandHistory;

// =============================================================================
// A. CommandHistory state
// =============================================================================

#[test]
fn new_history_cannot_undo() {
    let history = CommandHistory::new();
    assert!(!history.can_undo());
}

#[test]
fn new_history_cannot_redo() {
    let history = CommandHistory::new();
    assert!(!history.can_redo());
}

#[test]
fn new_history_is_empty() {
    let history = CommandHistory::new();
    assert_eq!(history.undo_count(), 0);
    assert_eq!(history.redo_count(), 0);
}

#[test]
fn new_history_undo_description_is_none() {
    let history = CommandHistory::new();
    assert!(history.undo_description().is_none());
}

#[test]
fn new_history_redo_description_is_none() {
    let history = CommandHistory::new();
    assert!(history.redo_description().is_none());
}

#[test]
fn new_history_undo_descriptions_empty() {
    let history = CommandHistory::new();
    assert!(history.undo_descriptions().is_empty());
}

#[test]
fn new_history_redo_descriptions_empty() {
    let history = CommandHistory::new();
    assert!(history.redo_descriptions().is_empty());
}

#[test]
fn history_clear_resets_state() {
    let mut history = CommandHistory::new();
    // We can't push commands without a World, but we can verify clear on empty
    history.clear();
    assert!(!history.can_undo());
    assert!(!history.can_redo());
}

// =============================================================================
// B. CommandGroup
// =============================================================================

#[test]
fn command_group_new_has_description() {
    let group = CommandGroup::new("Test Group");
    assert_eq!(group.description(), "Test Group");
}

#[test]
fn command_group_new_is_empty() {
    let group = CommandGroup::new("Test Group");
    assert!(group.is_empty());
}

#[test]
fn command_group_description_preserved() {
    let group = CommandGroup::new("My complex operation");
    // The description trait method should return what we passed
    let desc = Command::description(&group);
    assert_eq!(desc, "My complex operation");
}

// A no-op command for testing CommandGroup.add()
struct NoOpCommand(String);
impl Command for NoOpCommand {
    fn description(&self) -> String { self.0.clone() }
    fn execute(&mut self, _ctx: &mut CommandContext) -> CommandResult { CommandResult::NoOp }
    fn undo(&mut self, _ctx: &mut CommandContext) -> CommandResult { CommandResult::NoOp }
}

#[test]
fn command_group_add_makes_non_empty() {
    let mut group = CommandGroup::new("Test");
    assert!(group.is_empty());
    group.add(Box::new(NoOpCommand("test".into())));
    assert!(!group.is_empty());
}
