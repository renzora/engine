use bevy::prelude::*;

use crate::binding::{apply_commits, sync_bindings, BindingChanged, CommitBinding};

/// Schedules the reactive layer's two systems.
///
/// - `sync_bindings` runs in `Update`. Non-exclusive, parallelisable.
/// - `apply_commits` runs in `Last` (so edits land after all widget
///   systems have had a chance to emit them), exclusive.
///
/// Both events are registered here so panels can use them without
/// adding redundant `add_event` calls.
pub struct ReactivePlugin;

impl Plugin for ReactivePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<BindingChanged>()
            .add_message::<CommitBinding>()
            .add_systems(Update, sync_bindings)
            .add_systems(Last, apply_commits);
    }
}
