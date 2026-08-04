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
    ScriptTime, GAMEPAD_BUTTON_NAMES,
};
pub use reply::ScriptReply;
pub use value::{ActionValue, DrawCmd, PropValue, ScriptValue, VarDef};
pub use wire::{Reader, WireError, Writer};

use crate::sys::{Str256, StrRef};
use core::ffi::c_void;

/// Which hook the host is invoking.
///
/// Newtype rather than an `enum` for the same soundness reason every other
/// boundary discriminant in this crate is — the value crosses between binaries
/// and materialising an out-of-range one into a Rust enum is undefined
/// behaviour. Unknown values fall to the `_` arm and become
/// [`ScriptStatus::UnknownOp`].
///
/// **Append only.** Renumbering repoints an already-built plugin's `on_update`
/// at somebody else's hook.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScriptOp(pub u32);

#[allow(non_upper_case_globals)]
impl ScriptOp {
    /// The engine's declared bindings changed. Payload is the encoded
    /// [`Binding`] list in `args`; the plugin stores them and rebuilds its
    /// function table on the next VM it creates.
    pub const Bindings: Self = Self(0);
    /// Parse the props a script declares. Reply carries [`VarDef`]s.
    pub const Props: Self = Self(1);
    pub const OnReady: Self = Self(2);
    pub const OnUpdate: Self = Self(3);
    pub const OnRpc: Self = Self(4);
    pub const OnUi: Self = Self(5);
    pub const OnDraw: Self = Self(6);
    pub const OnAnimationEvent: Self = Self(7);
    pub const OnHttp: Self = Self(8);
    pub const OnPlayerEvent: Self = Self(9);
    /// Evaluate an expression for the console REPL.
    pub const Eval: Self = Self(10);
    /// Drop any cached state for this `(path, entity)`. Sent when an entity is
    /// despawned or a script is detached, so a long-lived VM does not outlive
    /// the thing it was scripting.
    pub const Evict: Self = Self(11);

    pub const fn is_known(self) -> bool {
        self.0 < 12
    }

    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "Bindings",
            1 => "Props",
            2 => "OnReady",
            3 => "OnUpdate",
            4 => "OnRpc",
            5 => "OnUi",
            6 => "OnDraw",
            7 => "OnAnimationEvent",
            8 => "OnHttp",
            9 => "OnPlayerEvent",
            10 => "Eval",
            11 => "Evict",
            _ => "?",
        }
    }
}

impl core::fmt::Debug for ScriptOp {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_known() {
            f.write_str(self.name())
        } else {
            write!(f, "ScriptOp({})", self.0)
        }
    }
}

/// How a call went.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ScriptStatus(pub i32);

#[allow(non_upper_case_globals)]
impl ScriptStatus {
    pub const Ok: Self = Self(0);
    /// The script does not define this hook. Not an error — most scripts define
    /// two of the nine — and the host must not log it.
    pub const NoHook: Self = Self(1);
    /// This backend does not implement this op. Treated exactly like
    /// [`Self::NoHook`], which is what makes appending an op a non-breaking
    /// change for plugins built before it existed.
    pub const UnknownOp: Self = Self(2);
    /// The script raised an error. The message is in the reply.
    pub const Error: Self = Self(3);
    /// The plugin panicked and the guard caught it. The host disables the
    /// script rather than calling it again every frame.
    pub const Panicked: Self = Self(4);

    pub const fn is_known(self) -> bool {
        self.0 >= 0 && self.0 < 5
    }
}

/// A borrowed byte slice. [`StrRef`] without the UTF-8 promise.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct BlobRef {
    pub ptr: *const u8,
    pub len: usize,
}

impl BlobRef {
    pub const EMPTY: Self = Self {
        ptr: core::ptr::null(),
        len: 0,
    };

    pub fn new(bytes: &[u8]) -> Self {
        Self {
            ptr: bytes.as_ptr(),
            len: bytes.len(),
        }
    }

    /// # Safety
    /// The bytes must be alive for `'a`.
    pub unsafe fn as_slice<'a>(self) -> &'a [u8] {
        if self.ptr.is_null() || self.len == 0 {
            &[]
        } else {
            core::slice::from_raw_parts(self.ptr, self.len)
        }
    }
}

/// Somewhere to put bytes, owned by whoever built it.
///
/// One type for both directions: the host builds one for the plugin's reply,
/// the plugin builds one for a host call's answer. Each side allocates into its
/// own `Vec` through its own function pointer, which is what keeps the two
/// allocators from ever meeting — the recurring hazard when a `cdylib` and a
/// host may not even share a rustc version.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ByteSink {
    pub ctx: *mut c_void,
    pub write: unsafe extern "C" fn(ctx: *mut c_void, bytes: *const u8, len: usize),
}

/// Reads a plugin can make *during* a call.
///
/// Valid only for the duration of the call that carried it — the host holds a
/// `&World` behind `ctx` and it dies when the hook returns. A plugin that
/// stashes one and uses it next frame is reading freed memory, which is why the
/// ergonomic layer hands it out as a borrow with the call's lifetime.
///
/// Every function answers by writing an encoded value into `reply` rather than
/// returning one, because all of the answers are variable-length and a returned
/// pointer would raise the question of who frees it.
///
/// A null `entity.ptr` means "the script's own entity", mirroring the
/// `Option<&str>` the engine's own handler takes.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ScriptHostCalls {
    pub ctx: *mut c_void,
    /// One reflected field. Replies with an encoded `Option<PropValue>`.
    pub get: unsafe extern "C" fn(
        ctx: *mut c_void,
        entity: StrRef,
        component: StrRef,
        field: StrRef,
        reply: *const ByteSink,
    ),
    /// Every field of one component. Replies with an encoded
    /// `Option<Vec<(String, PropValue)>>`.
    pub get_component: unsafe extern "C" fn(
        ctx: *mut c_void,
        entity: StrRef,
        component: StrRef,
        reply: *const ByteSink,
    ),
    /// The type names of every reflected component on an entity. Replies with
    /// an encoded `Vec<String>`.
    pub get_components:
        unsafe extern "C" fn(ctx: *mut c_void, entity: StrRef, reply: *const ByteSink),
    /// Asset-load progress. Replies with an encoded `Option<AssetProgress>`.
    pub asset_progress: unsafe extern "C" fn(ctx: *mut c_void, reply: *const ByteSink),
    /// Localization lookup. Replies with an encoded `String`.
    pub translate: unsafe extern "C" fn(ctx: *mut c_void, key: StrRef, reply: *const ByteSink),
}

/// One invocation of a language backend.
///
/// Several fields are empty for most ops — `source` is not sent for
/// [`ScriptOp::Evict`], `entity_ctx` is not sent for [`ScriptOp::Eval`]. That is
/// cheaper than a union and far easier to reason about than op-specific structs,
/// since an empty [`BlobRef`] costs two words.
#[repr(C)]
pub struct ScriptCall {
    pub op: ScriptOp,
    /// Padding so the pointer fields below are 8-byte aligned on every target.
    pub _pad: u32,

    /// The script's resolved path. **Identity, not an instruction** — the
    /// plugin uses it as a cache key and must not open it. See the module docs.
    pub path: StrRef,
    /// The script's source text, already read by the host (from disk, or from
    /// an rpak archive in an exported build).
    pub source: StrRef,
    /// Changes whenever `source` does. A plugin holding a compiled VM compares
    /// this and rebuilds on mismatch; that is the whole of hot-reload support.
    pub version: u64,
    /// `Entity::to_bits()` of the scripted entity, 0 when there is none.
    pub entity: u64,

    /// Identifies the frame `frame` belongs to. Every call in a frame carries
    /// the same value and the same `frame` bytes, so a plugin decodes the frame
    /// context once and reuses it — without this the per-entity saving the
    /// split was built for would be given straight back.
    pub frame_seq: u64,
    /// Encoded [`FrameContext`].
    pub frame: BlobRef,
    /// Encoded [`EntityContext`].
    pub entity_ctx: BlobRef,
    /// Encoded [`HookArgs`], or the [`Binding`] list for [`ScriptOp::Bindings`].
    pub args: BlobRef,
    /// Encoded `Vec<(String, ScriptValue)>` — the script's current prop values.
    pub vars: BlobRef,

    /// Where the plugin writes its encoded [`ScriptReply`].
    pub out: *const ByteSink,
    /// Reads back into the host, valid for this call only.
    pub host: *const ScriptHostCalls,
}

/// The signature of a backend's entry point.
pub type ScriptEntry = unsafe extern "C" fn(call: *const ScriptCall) -> ScriptStatus;

/// What a plugin registers to become a scripting language.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ScriptBackendDesc {
    /// Human-readable, for logs and the editor's language picker.
    pub name: Str256,
    /// File extensions claimed, without the dot — `["lua", "blueprint"]`.
    ///
    /// The engine routes a script to a backend by extension, which is what lets
    /// two language plugins coexist: one game can have `.lua` and `.wren`
    /// entities side by side. Two backends claiming the same extension is
    /// resolved by the host, which keeps the first and logs the collision.
    pub extensions: *const Str256,
    pub extension_count: usize,
    pub entry: ScriptEntry,
}

// SAFETY: `ScriptBackendDesc` is plain data plus a pointer to a `static` array
// of `Str256` and a function pointer. Nothing writes through either. It needs
// stating only because a raw pointer is not `Send`/`Sync` by default, and the
// host stores registered descs in a Bevy resource.
unsafe impl Send for ScriptBackendDesc {}
unsafe impl Sync for ScriptBackendDesc {}

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
