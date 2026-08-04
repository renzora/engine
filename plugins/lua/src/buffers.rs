//! Where a Lua function puts what it produced.
//!
//! Every binding in `register_api` is a plain `mlua` closure with no path back
//! to the backend that registered it — `create_function` takes a `'static`
//! callback and there is nowhere to thread a `&mut self` through. So they push
//! into a thread-local and the hook collects afterwards.
//!
//! This is exactly what the engine did before the move, and it moved unchanged.
//! It is also what makes the boundary cheap: a script issuing forty commands
//! does forty `Vec::push`es and one crossing, not forty crossings.
//!
//! Thread-local rather than a `Mutex` because a VM is never touched from two
//! threads at once — the engine runs scripts from one exclusive system — and a
//! lock per `set_position` would be pure overhead.

use std::cell::RefCell;

use renzora_plugin::script::{DrawCmd, ScriptCommand};

thread_local! {
    static COMMANDS: RefCell<Vec<ScriptCommand>> = const { RefCell::new(Vec::new()) };

    /// Kept separate from [`COMMANDS`] because draws are not ECS commands —
    /// they are a per-frame list the UI vector renderer reconciles, rebuilt
    /// from scratch every frame rather than applied once.
    static DRAWS: RefCell<Vec<DrawCmd>> = const { RefCell::new(Vec::new()) };
}

pub fn push_command(cmd: ScriptCommand) {
    COMMANDS.with(|b| b.borrow_mut().push(cmd));
}

pub fn drain_commands() -> Vec<ScriptCommand> {
    COMMANDS.with(|b| b.borrow_mut().drain(..).collect())
}

pub fn push_draw(cmd: DrawCmd) {
    DRAWS.with(|b| b.borrow_mut().push(cmd));
}

pub fn drain_draws() -> Vec<DrawCmd> {
    DRAWS.with(|b| b.borrow_mut().drain(..).collect())
}
