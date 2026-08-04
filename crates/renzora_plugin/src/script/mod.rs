//! Implement a scripting language as a plugin.
//!
//! The engine's scripting *system* is statically linked and stays that way: the
//! hooks, the command vocabulary, the context, the queue that applies commands
//! to the world. What is **not** in the engine is any particular interpreter.
//! This module is the contract between the two, so Lua, Wren, Python or
//! anything else is an ordinary `plugins/<lang>/` cdylib built with no engine
//! checkout.
//!
//! That is the whole point of the split. Before it, "which language can I
//! script in" was decided by which interpreter the engine was compiled with.
//! After it, the engine ships a scripting API and a language is a plugin.
//!
//! ## The shape of a call
//!
//! The host owns everything with a Bevy type in it. It walks the scripted
//! entities, builds the context, resolves and reads the script file, and
//! applies whatever comes back. The plugin owns exactly one thing: turning
//! source text plus a context into a list of [`ScriptCommand`]s.
//!
//! ```text
//!   host                                   plugin
//!   ────                                   ──────
//!   encode FrameContext  ── once/frame ──▶  (cached by frame_seq)
//!   encode EntityContext ── per entity ──▶
//!   read + hand over source ────────────▶   compile / reuse VM
//!                                           run the hook
//!                        ◀── ScriptReply ─  commands, vars, draws
//!   apply commands to the World
//! ```
//!
//! ### The host keeps file I/O, deliberately
//!
//! A plugin never opens a script file. It is handed `source` and a `version`
//! that changes when the source does. That is not a restriction for its own
//! sake — exported and Android builds read scripts out of an rpak archive
//! through a closure the engine owns, and a plugin doing its own `std::fs`
//! would silently work in the editor and fail in every shipped game. Hot reload
//! also stays where the file watcher already is.
//!
//! ### Synchronous reads go back through [`ScriptHostCalls`]
//!
//! Most of the API is one-way: a script asks for something to happen and the
//! command queue does it after the hook returns. Reads cannot work that way —
//! `get("Health.current")` has to answer *now* — so the call carries a small
//! table of host functions valid for exactly the duration of that call.
//!
//! ## Ops, not one function pointer per hook
//!
//! A backend registers a single `extern "C"` entry point and the hook is
//! selected by [`ScriptOp`]. The alternative, a struct of eight named function
//! pointers, would make adding a ninth hook an ABI change — every prebuilt
//! language plugin would need rebuilding to add `on_late_update`. With an op
//! code, a plugin that does not know an op returns [`ScriptStatus::UnknownOp`]
//! and the host treats it exactly like a script that does not define the hook.

pub mod command;
pub mod context;
pub mod value;
pub mod wire;

mod backend;
mod reply;

pub use backend::{desc_for, dispatch, Backend, BackendState, Ctx, Hook, HostCalls, ScriptRef};
pub use command::{decode_list, encode_list, ScriptCommand, VARIANT_COUNT};
pub use context::{
    decode_bindings, encode_bindings, AssetProgress, Binding, BindingKind, ChildNode,
    EntityContext, FrameContext, GamepadSnapshot, HookArgs, Param, ParamKind, RaycastHit,
    substitute, ScriptTime, GAMEPAD_BUTTON_NAMES,
};
pub use reply::ScriptReply;
pub use value::{ActionValue, DrawCmd, PropValue, ScriptValue, VarDef};
pub use wire::{Reader, WireError, Writer};

/// The boundary half, re-exported so a plugin author names one module.
///
/// These live in [`sys`](crate::sys) because they are named by
/// [`Interface`](crate::sys::Interface), and anything in that table is part of
/// the frozen mechanism. Everything else here — the commands, the contexts, the
/// codec — is vocabulary layered on top and can grow without moving the ABI.
pub use crate::sys::{
    BlobRef, ByteSink, ScriptBackendDesc, ScriptCall, ScriptEntry, ScriptHostCalls, ScriptOp,
    ScriptStatus,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_ops_are_contiguous_and_named() {
        for i in 0..12u32 {
            let op = ScriptOp(i);
            assert!(op.is_known(), "op {i} should be known");
            assert_ne!(op.name(), "?", "op {i} has no name");
        }
        assert!(!ScriptOp(12).is_known());
        assert_eq!(ScriptOp(12).name(), "?");
    }

    #[test]
    fn an_unknown_op_debugs_as_its_number_rather_than_a_wrong_name() {
        assert_eq!(format!("{:?}", ScriptOp(3)), "OnUpdate");
        assert_eq!(format!("{:?}", ScriptOp(99)), "ScriptOp(99)");
    }

    #[test]
    fn statuses_are_contiguous() {
        for i in 0..5 {
            assert!(ScriptStatus(i).is_known());
        }
        assert!(!ScriptStatus(5).is_known());
        assert!(!ScriptStatus(-1).is_known());
    }

    #[test]
    fn an_empty_blob_slices_to_nothing_rather_than_dereferencing_null() {
        // SAFETY: the whole point — an empty blob must not be dereferenced, and
        // the host sends one for every field an op does not use.
        assert!(unsafe { BlobRef::EMPTY.as_slice() }.is_empty());
    }

    #[test]
    fn a_blob_round_trips_through_a_slice() {
        let data = [1u8, 2, 3];
        let b = BlobRef::new(&data);
        assert_eq!(unsafe { b.as_slice() }, &data);
    }
}
