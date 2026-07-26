#[cfg(all(feature = "lua", not(target_arch = "wasm32")))]
pub mod lua;

#[cfg(feature = "rhai")]
pub mod rhai;

use crate::command::ScriptCommand;

thread_local! {
    /// Shared command buffer used by all backends and extensions.
    pub(crate) static COMMAND_BUFFER: std::cell::RefCell<Vec<ScriptCommand>> = const { std::cell::RefCell::new(Vec::new()) };
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

thread_local! {
    /// Immediate-mode draw commands accumulated during an `on_draw(g)` pass. Kept
    /// separate from [`COMMAND_BUFFER`] because draws aren't ECS commands — they're
    /// a per-frame list the UI vector renderer reconciles, not applied through the
    /// command queue.
    pub(crate) static DRAW_BUFFER: std::cell::RefCell<Vec<renzora::DrawCmd>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Record one draw command from a `g` context method.
pub(crate) fn push_draw(cmd: renzora::DrawCmd) {
    DRAW_BUFFER.with(|buf| buf.borrow_mut().push(cmd));
}

/// Drain the frame's draw commands. Called by a backend after `on_draw` returns.
pub(crate) fn drain_draws() -> Vec<renzora::DrawCmd> {
    DRAW_BUFFER.with(|buf| buf.borrow_mut().drain(..).collect())
}
