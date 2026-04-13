#[cfg(all(feature = "lua", not(target_arch = "wasm32")))]
pub mod lua;

#[cfg(feature = "rhai")]
pub mod rhai;

use crate::command::ScriptCommand;

thread_local! {
    /// Shared command buffer used by all backends and extensions.
    pub(crate) static COMMAND_BUFFER: std::cell::RefCell<Vec<ScriptCommand>> = std::cell::RefCell::new(Vec::new());
}

/// Push a script command from any context (backends or extensions).
/// This is the public API for extensions to issue commands.
pub fn push_command(cmd: ScriptCommand) {
    COMMAND_BUFFER.with(|buf| buf.borrow_mut().push(cmd));
}

/// Drain all buffered commands. Called by backends after script execution.
pub(crate) fn drain_commands() -> Vec<ScriptCommand> {
    COMMAND_BUFFER.with(|buf| buf.borrow_mut().drain(..).collect())
}
