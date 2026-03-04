//! Deferred editor commands — queued during `&World` panel rendering, executed after.

use std::sync::Mutex;

use bevy::prelude::*;

/// A queue of deferred world-mutation closures.
///
/// Panels render with `&World` but sometimes need to write (e.g. drag a float → update Transform).
/// They push closures here; `editor_ui_system` drains and executes them after all panels finish.
#[derive(Resource)]
pub struct EditorCommands {
    queue: Mutex<Vec<Box<dyn FnOnce(&mut World) + Send>>>,
}

impl Default for EditorCommands {
    fn default() -> Self {
        Self {
            queue: Mutex::new(Vec::new()),
        }
    }
}

impl EditorCommands {
    /// Push a deferred command to be executed after panel rendering.
    pub fn push(&self, cmd: impl FnOnce(&mut World) + Send + 'static) {
        self.queue.lock().unwrap().push(Box::new(cmd));
    }

    /// Drain all queued commands. Called by `editor_ui_system`.
    pub fn drain(&self) -> Vec<Box<dyn FnOnce(&mut World) + Send>> {
        std::mem::take(&mut *self.queue.lock().unwrap())
    }
}
