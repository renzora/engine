//! The boundary half of `crate::script`.
//!
//! These live here rather than beside the codec for the same reason
//! [`PanelDesc`] and [`MeshDesc`] do: anything named in [`Interface`] is part of
//! the frozen mechanism, and the mechanism is this module. The vocabulary that
//! rides on top — the command list, the contexts, the encoder — stays in
//! `crate::script`, where it can grow without any of this moving.
//! `crate::script` re-exports everything below so a plugin author only ever
//! names one module.
//!
//! [`PanelDesc`]: super::PanelDesc
//! [`MeshDesc`]: super::MeshDesc
//! [`Interface`]: super::Interface

use core::ffi::c_void;

use super::{Str256, StrRef};

/// Which hook the host is invoking.
///
/// Newtype rather than an `enum` for the same soundness reason every other
/// boundary discriminant here is — the value crosses between binaries, and
/// materialising an out-of-range discriminant into a Rust enum is undefined
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
    /// The engine's declared bindings changed. `args` holds the encoded list.
    pub const Bindings: Self = Self(0);
    /// Parse the props a script declares.
    pub const Props: Self = Self(1);
    pub const OnReady: Self = Self(2);
    pub const OnUpdate: Self = Self(3);
    pub const OnRpc: Self = Self(4);
    pub const OnUi: Self = Self(5);
    pub const OnDraw: Self = Self(6);
    pub const OnAnimationEvent: Self = Self(7);
    pub const OnHttp: Self = Self(8);
    pub const OnPlayerEvent: Self = Self(9);
    /// Evaluate an expression, for the console REPL.
    pub const Eval: Self = Self(10);
    /// Drop cached state for this `(path, entity)` — the entity went away.
    pub const Evict: Self = Self(11);
    /// A scene finished loading, or failed to. Carries
    /// [`crate::script::HookArgs::SceneEvent`].
    pub const OnSceneEvent: Self = Self(12);
    /// A broadcast game event. Carries [`crate::script::HookArgs::Event`].
    pub const OnEvent: Self = Self(13);

    pub const fn is_known(self) -> bool {
        self.0 < 14
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
            12 => "OnSceneEvent",
            13 => "OnEvent",
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

/// How a call into a backend went.
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
    /// The script raised an error; the message is in the reply.
    pub const Error: Self = Self(3);
    /// The plugin panicked and its guard caught it. The host disables the
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
/// allocators from ever meeting — the recurring hazard when a `cdylib` and its
/// host may not share a rustc version, let alone a CRT.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ByteSink {
    pub ctx: *mut c_void,
    pub write: unsafe extern "C" fn(ctx: *mut c_void, bytes: *const u8, len: usize),
}

/// Reads a plugin can make *during* a call.
///
/// Valid only for the duration of the call that carried it — the host holds a
/// `&World` behind `ctx` and drops it when the hook returns. A plugin that
/// stashed one and used it next frame would read freed memory, which is why
/// `crate::script`'s wrapper hands it out as a borrow with the call's lifetime.
///
/// Every function answers by writing an encoded value into `reply` rather than
/// returning one: all the answers are variable-length, and a returned pointer
/// would raise the question of who frees it.
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
    /// Type names of every reflected component on an entity. Replies with an
    /// encoded `Vec<String>`.
    pub get_components:
        unsafe extern "C" fn(ctx: *mut c_void, entity: StrRef, reply: *const ByteSink),
    /// Asset-load progress. Replies with an encoded `Option<AssetProgress>`.
    pub asset_progress: unsafe extern "C" fn(ctx: *mut c_void, reply: *const ByteSink),
    /// Localization lookup. Replies with an encoded `String`.
    pub translate: unsafe extern "C" fn(ctx: *mut c_void, key: StrRef, reply: *const ByteSink),
    /// Scene-load phase/path/progress. Replies with an encoded
    /// `Option<SceneLoad>`.
    ///
    /// **Append only**, same rule as [`ScriptOp`]: a plugin built before a
    /// field existed reads the ones it knows at unchanged offsets and never
    /// calls the new pointer. Inserting one in the middle would repoint an
    /// already-built plugin's `translate` at somebody else's function.
    pub scene_load_state: unsafe extern "C" fn(ctx: *mut c_void, reply: *const ByteSink),
}

/// One invocation of a language backend.
///
/// Several fields are empty for most ops — no `source` for [`ScriptOp::Evict`],
/// no `entity_ctx` for [`ScriptOp::Eval`]. That is cheaper than a union and far
/// easier to reason about than a struct per op, since an empty [`BlobRef`]
/// costs two words.
#[repr(C)]
pub struct ScriptCall {
    pub op: ScriptOp,
    /// Keeps the pointer fields below 8-byte aligned on every target.
    pub _pad: u32,

    /// The script's resolved path. **Identity, not an instruction** — the
    /// plugin uses it as a cache key and must not open it. The host owns file
    /// I/O so that exported builds, which read scripts out of an rpak archive,
    /// work without every language plugin knowing about archives.
    pub path: StrRef,
    /// The script's source, already read by the host.
    pub source: StrRef,
    /// Changes whenever `source` does. A plugin holding a compiled VM compares
    /// this and rebuilds on mismatch; that is the whole of hot-reload support.
    pub version: u64,
    /// `Entity::to_bits()` of the scripted entity, 0 when there is none.
    pub entity: u64,

    /// Identifies the frame `frame` belongs to. Every call in a frame carries
    /// the same value and the same bytes, so a plugin decodes the frame context
    /// once — without this the per-entity saving the frame/entity split exists
    /// for would be handed straight back.
    pub frame_seq: u64,
    /// Encoded `FrameContext`.
    pub frame: BlobRef,
    /// Encoded `EntityContext`.
    pub entity_ctx: BlobRef,
    /// Encoded `HookArgs`, or the binding list for [`ScriptOp::Bindings`].
    pub args: BlobRef,
    /// Encoded `Vec<(String, ScriptValue)>` — the script's current prop values.
    pub vars: BlobRef,

    /// Where the plugin writes its encoded reply.
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
    /// Extensions claimed, without the dot — `["lua", "blueprint"]`.
    ///
    /// The engine routes a script to a backend by extension, which is what lets
    /// two language plugins coexist: one game can have `.lua` and `.wren`
    /// entities side by side. Two backends claiming one extension is resolved
    /// by the host, which keeps the first and logs the collision.
    ///
    /// Only borrowed for the duration of the registration call — the host
    /// copies the strings before returning, so this may point at a local.
    pub extensions: *const Str256,
    pub extension_count: usize,
    pub entry: ScriptEntry,
}

// SAFETY: plain data plus a pointer the host only ever reads, and a function
// pointer. Needed because a raw pointer is not `Send`/`Sync` by default and the
// host keeps registered descriptors in a Bevy resource.
unsafe impl Send for ScriptBackendDesc {}
unsafe impl Sync for ScriptBackendDesc {}
