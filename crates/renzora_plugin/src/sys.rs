//! The raw C ABI between the Renzora host and a `dlopen`'d plugin.
//!
//! Plugin authors never touch this module — they use the ergonomic layer in the
//! crate root, which is a thin shim over these calls. This is the layer that has
//! to stay stable forever, so it is deliberately small, boring, and hand-written.
//!
//! ## The one rule
//!
//! **A plugin exports exactly one symbol ([`INIT_SYMBOL`]) and imports nothing
//! from the host.** The host passes [`Interface`] *in* at load time. That is the
//! whole reason a plugin can be built on any machine with any rustc: there is no
//! dynamic symbol to resolve against `renzora.exe`, so there is no filename to
//! match, no `bevy_dylib-<hash>` to find, and no `TypeId` to line up. The only
//! thing both sides must agree on is the layout of the `#[repr(C)]` types below.
//!
//! ## Why host components are copied rather than pointed at
//!
//! It is tempting to hand a plugin a raw pointer into Bevy's component storage
//! and let it cast to a mirror struct. For the host's own types that is
//! **undefined behaviour**: `bevy::Transform` is not `#[repr(C)]` (it derives
//! only `Debug, PartialEq, Clone, Copy`), so its field order is unspecified and
//! a compiler update may reorder it. `glam::Quat` is worse — it has three
//! different representations depending on SIMD backend (`repr(transparent)` over
//! `f32x4` on coresimd, over `float32x4_t` on NEON, `repr(align(16))` on
//! scalar), so its size and alignment vary by target.
//!
//! So host types cross as the frozen mirrors below, copied in and out around the
//! call. **Plugin-owned** components have no such problem — the plugin defines
//! the layout, registers it via [`ComponentDesc`], and gets a direct pointer. In
//! practice most component data is plugin-owned, so most access is copy-free.
//!
//! ## Versioning
//!
//! A plugin accepts any host whose MAJOR matches and whose MINOR is at least the
//! one it was built against. That rule only holds if MINOR changes are strictly
//! **append-only**, so be precise about which is which:
//!
//! * **MINOR** — a new function appended to the end of [`Interface`], or a new
//!   field appended to the end of a struct the plugin only reads. An older
//!   plugin never touches it and keeps working.
//! * **MAJOR** — anything else. Changing a function's signature, reordering or
//!   removing a field, or changing what an existing field *means*.
//!
//! Getting this wrong is not a cosmetic mistake. [`SystemEntry`] once returned
//! `()` and was changed to return [`SystemStatus`] under a MINOR bump; a plugin
//! built before that passed the handshake, and the host then called a `void`
//! function and read a return value nothing had written. The process died with
//! no useful diagnostic. When in doubt, bump MAJOR — the cost is rebuilding
//! plugins, and the alternative is memory corruption.
//!
//! ### Adding a value to a plugin-written enum
//!
//! [`Schedule`], [`FieldKind`], [`Access`], [`RenderPhase`], [`Primitive`],
//! [`CommandKind`] and [`LogLevel`] are all written by the plugin and read by
//! the host. They are **newtypes over `u32`, not `#[repr(u32)]` enums**, and
//! that is load-bearing rather than stylistic.
//!
//! A plugin built against a newer ABI writes a value this build has no name for.
//! With a real `enum` that is undefined behaviour at the moment the host reads
//! it — and not the harmless kind, because rustc attaches `!range` metadata to
//! the load, so LLVM may assume the impossible and a `match` can take an
//! arbitrary arm. The read happens inside `from_raw_parts` over plugin memory
//! for the ones that live in structs, and straight off the boundary for the ones
//! passed by value, so there is no point at which the host could check first.
//!
//! The version handshake is *supposed* to refuse such a plugin, and it does.
//! But building on that means the soundness of every appended value rests on the
//! handshake staying bug-free forever, and the handshake has been wrong before —
//! see the [`SystemEntry`] story above. A newtype removes the question: every
//! `u32` is a valid value, unknown ones fall to a `_` arm, and "appending a
//! value is MINOR" is *true* rather than usually true.
//!
//! So: append a constant, bump MINOR in the same commit, and make sure every
//! `match` on it does something defensible with an unrecognised value. The host
//! generally warns and degrades — an unknown [`Schedule`] runs in `Update`, an
//! unknown [`Primitive`] draws a cube — except for [`Access`], where a term the
//! host cannot interpret would shift every later cell index and hand the plugin
//! its own data at the wrong offsets. That one refuses the system outright.

use core::ffi::c_void;

/// Bumped for any change that is not purely additive — a signature change, a
/// reordered or removed field, or a field whose meaning changed. Every existing
/// plugin is refused when this moves, which is the point.
///
/// Went 0 -> 1 when `SystemEntry` gained a return value; see the module docs for
/// why that had to be MAJOR.
///
/// Went 1 -> 2 when a system stopped being limited to one query. `SystemCall`
/// swapped its single cell array for a [`QueryView`] per query, and `add_system`
/// took a [`SystemDesc`] and started returning [`RegisterStatus`]. Both are
/// deliberate MAJOR changes made while the ABI is still unpublished: a MAJOR
/// bump today costs rebuilding the example plugins, and after the first release
/// it costs every plugin every user has installed.
pub const VERSION_MAJOR: u32 = 2;

/// Bumped when something is *appended*. Older plugins keep working; a plugin
/// needing the new function declares this as its minimum.
///
/// Reset to 0 by the MAJOR bump above — a MINOR only means anything relative to
/// one MAJOR.
///
/// 0 -> 1 appended `add_panel`.
pub const VERSION_MINOR: u32 = 1;

/// The single symbol a plugin cdylib must export. See [`ExtensionInit`].
pub const INIT_SYMBOL: &str = "renzora_plugin_init";

/// Optional symbol declaring which binary a plugin belongs in. See
/// [`PluginScope`].
pub const SCOPE_SYMBOL: &str = "renzora_plugin_scope";

/// Where a plugin runs.
///
/// The engine ships two binaries: the editor, which contains the runtime, and a
/// standalone runtime that ships with a game. A plugin belongs to exactly one
/// scope — there is deliberately no "both". A feature needing editor tooling on
/// top of runtime behaviour is two plugins, so the editor half can be absent
/// from a shipped game rather than merely inactive in it.
///
/// A plugin that exports no [`SCOPE_SYMBOL`] is treated as [`Runtime`]. That
/// matches `renzora::add!`'s default and fails in the direction that is easy to
/// notice: an editor plugin that forgot to declare itself turns up in a game,
/// which is visible, whereas the other default would make a gameplay plugin
/// silently do nothing in the shipped build.
///
/// [`Runtime`]: PluginScope::Runtime
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PluginScope(pub u32);

#[allow(non_upper_case_globals)]
impl PluginScope {
    /// Runs in the editor viewport **and** the shipped game.
    pub const Runtime: Self = Self(0);
    /// Runs only in the editor. Never linked into, or loaded by, a game.
    pub const Editor: Self = Self(1);

    pub const fn is_known(self) -> bool {
        self.0 < 2
    }

    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "Runtime",
            1 => "Editor",
            _ => "?",
        }
    }
}

impl core::fmt::Debug for PluginScope {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_known() {
            f.write_str(self.name())
        } else {
            write!(f, "PluginScope({})", self.0)
        }
    }
}

/// Reports a plugin's [`PluginScope`]. Read before `init` is called, so a plugin
/// for the wrong binary is never given the chance to register anything.
pub type ScopeEntry = unsafe extern "C" fn() -> PluginScope;

// ── Primitives ───────────────────────────────────────────────────────────────

/// An entity, as Bevy's `Entity::to_bits()`. Opaque to the plugin — only ever
/// handed back to the host.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Entity(pub u64);

/// A component's runtime id, assigned by the host. Resolved either by
/// registering a plugin-owned component ([`Interface::register_component`]) or
/// by looking up a host one ([`Interface::component_id_by_name`]).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ComponentId(pub u32);

impl ComponentId {
    /// Returned by [`Interface::component_id_by_name`] when no component with
    /// that type path is registered. A plugin that queries on this will never
    /// match anything, so it should fail loudly at registration instead.
    pub const INVALID: ComponentId = ComponentId(u32::MAX);

    pub const fn is_valid(self) -> bool {
        self.0 != u32::MAX
    }
}

/// A borrowed UTF-8 string. Never transfers ownership in either direction — the
/// callee must copy if it wants to keep the bytes. This sidesteps the "whose
/// allocator frees it" problem entirely, which matters because the host and the
/// plugin have separate allocators and may not even share a rustc version.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct StrRef {
    pub ptr: *const u8,
    pub len: usize,
}

// SAFETY: a `StrRef` is a shared borrow of immutable UTF-8 bytes — `Sync` and
// `Send` for exactly the same reason `&'static str` is. The raw pointer is only
// there to keep the type `#[repr(C)]`; nothing ever writes through it. Without
// these, a component's field schema could not live in a `static`, which is the
// only sensible place for it.
unsafe impl Send for StrRef {}
unsafe impl Sync for StrRef {}

impl StrRef {
    pub const fn new(s: &'static str) -> Self {
        Self {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }

    /// # Safety
    /// The bytes must still be alive and valid UTF-8 for `'a`.
    pub unsafe fn as_str<'a>(self) -> &'a str {
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(self.ptr, self.len))
    }
}

/// Which of the host's schedules a system runs in. `#[repr(u32)]` so the value
/// is a stable part of the ABI — append new variants, never renumber.
/// Newtype rather than an `enum`, and that is a soundness requirement rather
/// than a style choice.
///
/// The **plugin writes this value and the host reads it** — out of plugin memory
/// for the ones that live in structs, and straight off the FFI boundary for the
/// ones passed by value. Materialising an out-of-range discriminant into a Rust
/// enum is undefined behaviour, and not the harmless kind: rustc attaches
/// `!range` metadata to the load, so LLVM may legally assume the impossible and
/// a `match` can take an arbitrary arm.
///
/// That is exactly what a MINOR bump would cause. A plugin built against a newer
/// ABI writes a discriminant the older host has no variant for. The version
/// handshake is supposed to refuse that plugin — but then the soundness of every
/// appended variant rests on the handshake being bug-free, forever. A newtype
/// removes the question: any `u32` is a valid value, unknown ones fall to the
/// `_` arm, and "appending a variant is a MINOR change" is true rather than
/// merely usually true.
///
/// The constants below keep the variant names, so this is a source-compatible
/// change at every call site.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Schedule(pub u32);

#[allow(non_upper_case_globals)]
impl Schedule {
    pub const First: Self = Self(0);
    pub const PreUpdate: Self = Self(1);
    pub const Update: Self = Self(2);
    pub const PostUpdate: Self = Self(3);
    pub const Last: Self = Self(4);

    /// Whether this is a value this build knows. Anything else came from a
    /// plugin built against a newer ABI.
    pub const fn is_known(self) -> bool {
        self.0 < 5
    }

    /// The variant name, or `"?"` for a value from a newer ABI.
    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "First",
            1 => "PreUpdate",
            2 => "Update",
            3 => "PostUpdate",
            4 => "Last",
            _ => "?",
        }
    }
}

impl core::fmt::Debug for Schedule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_known() {
            f.write_str(self.name())
        } else {
            write!(f, "Schedule({})", self.0)
        }
    }
}

// ── Frozen host-type mirrors ─────────────────────────────────────────────────
//
// Once published, the layout of everything in this section is frozen forever.
// Adding a field is a MAJOR bump. Keep the list short: every type here is one
// the host has to marshal on every system call.

/// Mirrors `glam::Vec3`. Defined here rather than re-exported because glam's own
/// representation is not a stability promise to us (see the module docs).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Mirrors `glam::Quat` as plain `xyzw` floats — deliberately *not* the SIMD
/// representation, which varies by target.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

/// Mirrors `bevy::Transform`. Field order here is the contract; the host
/// converts to and from the real thing, which is free to lay itself out however
/// the compiler likes.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

// ── Registration ─────────────────────────────────────────────────────────────

/// The type of one inspectable field.
///
/// Append-only `#[repr(u32)]`. Deliberately a small closed set rather than
/// anything reflection-shaped: the editor has to render a widget for each, so a
/// kind nobody can draw is worse than no kind at all.
/// Newtype rather than an `enum`, and that is a soundness requirement rather
/// than a style choice.
///
/// The **plugin writes this value and the host reads it** — out of plugin memory
/// for the ones that live in structs, and straight off the FFI boundary for the
/// ones passed by value. Materialising an out-of-range discriminant into a Rust
/// enum is undefined behaviour, and not the harmless kind: rustc attaches
/// `!range` metadata to the load, so LLVM may legally assume the impossible and
/// a `match` can take an arbitrary arm.
///
/// That is exactly what a MINOR bump would cause. A plugin built against a newer
/// ABI writes a discriminant the older host has no variant for. The version
/// handshake is supposed to refuse that plugin — but then the soundness of every
/// appended variant rests on the handshake being bug-free, forever. A newtype
/// removes the question: any `u32` is a valid value, unknown ones fall to the
/// `_` arm, and "appending a variant is a MINOR change" is true rather than
/// merely usually true.
///
/// The constants below keep the variant names, so this is a source-compatible
/// change at every call site.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FieldKind(pub u32);

#[allow(non_upper_case_globals)]
impl FieldKind {
    pub const F32: Self = Self(0);
    pub const I32: Self = Self(1);
    pub const Bool: Self = Self(2);
    pub const Vec3: Self = Self(3);
    pub const Quat: Self = Self(4);

    /// Whether this is a value this build knows. Anything else came from a
    /// plugin built against a newer ABI.
    pub const fn is_known(self) -> bool {
        self.0 < 5
    }

    /// The variant name, or `"?"` for a value from a newer ABI.
    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "F32",
            1 => "I32",
            2 => "Bool",
            3 => "Vec3",
            4 => "Quat",
            _ => "?",
        }
    }
}

impl core::fmt::Debug for FieldKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_known() {
            f.write_str(self.name())
        } else {
            write!(f, "FieldKind({})", self.0)
        }
    }
}

/// One editable field of a plugin component.
///
/// This exists because a plugin component has no `TypeRegistration` — the engine
/// knows its size and alignment but nothing about its shape, so the inspector
/// has no way to show it and nothing could ever put one on an entity. The
/// schema is what makes a plugin component *usable* rather than merely storable.
///
/// `offset` is a byte offset into the component, which the plugin gets from
/// `core::mem::offset_of!`. The host reads and writes through it directly.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FieldDesc {
    pub name: StrRef,
    pub kind: FieldKind,
    pub offset: usize,
}

/// Describes a plugin-owned component so the host can register it via
/// `World::register_component_with_descriptor`.
///
/// `drop` is `None` for plain-data components, which is the strongly preferred
/// case: a component with no destructor can be stripped from entities at unload
/// without calling back into a library that is about to be removed from the
/// address space. A component that *does* need a destructor makes the plugin
/// un-unloadable in practice.
#[repr(C)]
pub struct ComponentDesc {
    /// Fully-qualified type path, e.g. `"my_spinner::Spinner"`. This is the
    /// component's identity — it is what scenes serialize and what
    /// [`Interface::component_id_by_name`] matches on, so renaming it breaks
    /// saved scenes exactly like renaming a Rust type would.
    pub name: StrRef,
    pub size: usize,
    pub align: usize,
    pub drop: Option<unsafe extern "C" fn(*mut u8)>,
    /// Human-readable name for the editor's "Add Component" list. Empty falls
    /// back to the last segment of `name`.
    pub display_name: StrRef,
    /// Inspectable fields. May be empty — a marker component has none, and it is
    /// still addable, just with nothing to edit.
    pub fields: *const FieldDesc,
    pub field_count: usize,
    /// Writes one default-valued instance into `size` bytes of host-provided
    /// storage. Used when the editor adds the component to an entity.
    ///
    /// A function rather than a pointer to a default instance: a derive cannot
    /// build a `static` of an arbitrary user type (that needs const
    /// construction), and a pointer into a temporary would dangle the moment
    /// `descriptor()` returned. `None` falls back to zeroed memory, which is
    /// wrong for anything whose sensible default isn't all-zero — a scale of 0,
    /// a speed of 0.
    pub default_init: Option<unsafe extern "C" fn(*mut u8)>,
}

/// How a query term touches its component.
///
/// `#[repr(u32)]` and append-only, like [`Schedule`]. Filter terms (`With` /
/// `Without`) are not just sugar — without them a plugin can only express "every
/// entity that has a Transform", which in a real scene includes the editor
/// camera and every light. Filtering is how a plugin scopes itself to the
/// entities it actually owns.
/// Newtype rather than an `enum`, and that is a soundness requirement rather
/// than a style choice.
///
/// The **plugin writes this value and the host reads it** — out of plugin memory
/// for the ones that live in structs, and straight off the FFI boundary for the
/// ones passed by value. Materialising an out-of-range discriminant into a Rust
/// enum is undefined behaviour, and not the harmless kind: rustc attaches
/// `!range` metadata to the load, so LLVM may legally assume the impossible and
/// a `match` can take an arbitrary arm.
///
/// That is exactly what a MINOR bump would cause. A plugin built against a newer
/// ABI writes a discriminant the older host has no variant for. The version
/// handshake is supposed to refuse that plugin — but then the soundness of every
/// appended variant rests on the handshake being bug-free, forever. A newtype
/// removes the question: any `u32` is a valid value, unknown ones fall to the
/// `_` arm, and "appending a variant is a MINOR change" is true rather than
/// merely usually true.
///
/// The constants below keep the variant names, so this is a source-compatible
/// change at every call site.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Access(pub u32);

#[allow(non_upper_case_globals)]
impl Access {
    /// Read-only data access. Produces a cell; never written back.
    pub const Read: Self = Self(0);
    /// Mutable data access. Produces a cell, and the host copies it back after
    /// the call.
    pub const Write: Self = Self(1);
    /// Filter only — the entity must have this component. No cell is produced,
    /// so the plugin must not count it when indexing [`SystemCall::cells`].
    pub const With: Self = Self(2);
    /// Filter only — the entity must NOT have this component.
    pub const Without: Self = Self(3);
    /// Read-only access to a **resource**, not a component. Contributes no cell
    /// and no filtering; the host resolves it once per call into
    /// [`SystemCall::resources`].
    pub const ResRead: Self = Self(4);
    /// Mutable resource access.
    pub const ResWrite: Self = Self(5);
    /// Optional component data. Produces a cell like [`Access::Read`], but the
    /// cell is **null** when the entity lacks the component, and the entity
    /// still matches. Mirrors Bevy's `Option<&T>`.
    pub const ReadOptional: Self = Self(6);
    /// Optional mutable component data. Mirrors `Option<&mut T>`.
    pub const WriteOptional: Self = Self(7);
    /// Opens an `Or` group. Carries no component.
    pub const OrBegin: Self = Self(8);
    /// Separates one `Or` branch from the next. Carries no component.
    pub const OrNext: Self = Self(9);
    /// Closes an `Or` group. Carries no component.
    pub const OrEnd: Self = Self(10);

    /// Whether this is a value this build knows. Anything else came from a
    /// plugin built against a newer ABI.
    pub const fn is_known(self) -> bool {
        self.0 < 11
    }

    /// The variant name, or `"?"` for a value from a newer ABI.
    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "Read",
            1 => "Write",
            2 => "With",
            3 => "Without",
            4 => "ResRead",
            5 => "ResWrite",
            6 => "ReadOptional",
            7 => "WriteOptional",
            8 => "OrBegin",
            9 => "OrNext",
            10 => "OrEnd",
            _ => "?",
        }
    }
}

impl core::fmt::Debug for Access {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_known() {
            f.write_str(self.name())
        } else {
            write!(f, "Access({})", self.0)
        }
    }
}

impl Access {
    /// Whether this term is a grouping marker rather than a component or
    /// resource reference. Markers carry [`ComponentId::INVALID`].
    pub const fn is_marker(self) -> bool {
        matches!(self, Access::OrBegin | Access::OrNext | Access::OrEnd)
    }

    /// Whether this term contributes a cell to [`SystemCall::cells`]. Filter and
    /// resource terms do not, which is why cell indices are *not* term indices.
    pub const fn has_cell(self) -> bool {
        matches!(
            self,
            Access::Read | Access::Write | Access::ReadOptional | Access::WriteOptional
        )
    }

    /// Whether this term names a resource rather than a component.
    pub const fn is_resource(self) -> bool {
        matches!(self, Access::ResRead | Access::ResWrite)
    }
}

/// One component a system's query touches.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Term {
    pub component: ComponentId,
    pub access: Access,
}

impl Term {
    /// A grouping marker — an `Or` bracket, which names no component.
    pub const fn marker(access: Access) -> Self {
        Self {
            component: ComponentId::INVALID,
            access,
        }
    }
}

/// The full access pattern of one system, declared up front at registration.
///
/// The host turns this into a real Bevy query via `QueryParamBuilder`, so the
/// resulting system carries proper component access and **schedules in parallel**
/// with anything it does not conflict with. Declaring access up front is what
/// buys that — a plugin that could reach arbitrary components on a whim would
/// have to be an exclusive system, serialising the whole schedule.
#[repr(C)]
pub struct QueryDesc {
    pub terms: *const Term,
    pub term_count: usize,
}

/// One query's matched rows, for one call.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct QueryView {
    /// Row-major `entity_count × cell_count`.
    pub cells: *mut *mut u8,
    pub entities: *const Entity,
    pub entity_count: usize,
    /// Cells per row — the count of [`Access::has_cell`] terms, which is NOT the
    /// number of terms once filters are involved.
    pub cell_count: usize,
}

/// Everything about one system, registered in a single call.
///
/// A struct rather than a parameter list because this is the part of the ABI
/// most likely to grow — run conditions, ordering labels and system sets all
/// land here — and a struct the plugin *writes* can only grow at the end. That
/// is also why [`flags`](Self::flags) exists now while nothing sets it: a
/// reserved word costs four bytes today and saves a MAJOR bump later.
#[repr(C)]
pub struct SystemDesc {
    pub entry: SystemEntry,
    pub schedule: Schedule,
    /// Queries, in the order the system's parameters declare them. The host
    /// hands back one [`QueryView`] per entry, in the same order.
    pub queries: *const QueryDesc,
    pub query_count: usize,
    /// Resources the system touches, as [`Access::ResRead`] / [`Access::ResWrite`]
    /// terms. Separate from the queries because a resource is per-system, not
    /// per-query.
    pub resources: *const Term,
    pub resource_count: usize,
    /// Opaque value handed back in [`SystemCall::user`].
    pub user: *mut c_void,
    /// Reserved. Must be zero.
    pub flags: u32,
}

/// Why [`Interface::add_system`] refused a system.
///
/// `add_system` used to return nothing, and it runs inside the host's panic
/// guard — so Bevy's access-conflict panic and a failed term resolution were
/// both caught, logged, and swallowed. The plugin's `init` returned `Ok` with a
/// system silently missing, which presents as "my plugin loaded and does
/// nothing": the single hardest failure to diagnose from the outside.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RegisterStatus {
    Ok = 0,
    /// A term named a component id the host does not have.
    UnknownComponent = 1,
    /// Bevy refused the access pattern — usually `&mut T` and `&T` on the same
    /// component in one system.
    AccessConflict = 2,
    /// The descriptor was malformed: a null pointer, no queries, or a non-zero
    /// `flags`.
    Invalid = 3,
}

// ── The per-frame call ───────────────────────────────────────────────────────

/// Frame-global values a system might want, passed by value so that reading the
/// clock does not cost an FFI call.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FrameCtx {
    pub delta_secs: f32,
    pub elapsed_secs: f32,
}

/// Everything one invocation of a plugin system receives.
///
/// **One call per system per frame, not per entity.** `cells` is a row-major
/// `entity_count × cell_count` array of pointers, in the order the *data* terms
/// were declared in [`QueryDesc`]. Filter terms ([`Access::With`] /
/// [`Access::Without`]) contribute no cell, so a plugin mixing filters and data
/// must index by data-term position, not by term position. The plugin's loop then runs entirely inside its own
/// address space at native speed; only crossing the boundary costs anything.
///
/// A cell points either into Bevy's component storage directly (plugin-owned
/// components) or at a host-side staging buffer holding a frozen mirror (host
/// components). The plugin cannot tell the difference and does not need to.
#[repr(C)]
pub struct SystemCall {
    /// One entry per [`Query`](QueryDesc) the system declared, in declaration
    /// order.
    ///
    /// A system used to carry exactly one query, because the boundary carried
    /// one flat term list. Two `Query` parameters therefore merged into a single
    /// builder and silently AND-ed together — `fn(Query<&A>, Query<&B>)` matched
    /// only entities with both, and each parameter read the other's cells.
    pub views: *const QueryView,
    pub view_count: usize,
    pub frame: FrameCtx,
    /// Opaque value the plugin supplied at [`Interface::add_system`] — how a
    /// generated thunk finds its way back to the right Rust function.
    pub user: *mut c_void,
    /// The interface, so a running system can log. Points at the host's
    /// `'static` table.
    pub iface: *const Interface,
    /// **Always null while a system runs.** A `Host` handle is only meaningful
    /// during plugin init, when the host holds `&mut World`; while a system runs
    /// the world is borrowed by the query and there is nothing valid for it to
    /// point at. Structural changes go through [`commands`](Self::commands).
    pub host: *mut Host,
    /// Queue for structural changes. Null if the host could not provide one.
    pub commands: *mut CommandSink,
    /// The resources this system declared. Self-describing rather than
    /// positional: a parameter finds its own slot by id, so parameter order and
    /// term order need not agree and neither side has to thread an index
    /// through the other.
    pub resources: *const ResourceSlot,
    pub resource_count: usize,
}

/// One resolved resource for a call.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ResourceSlot {
    pub id: ComponentId,
    /// Null when the resource does not exist. A system still runs — it is a
    /// plugin's own business whether a missing resource is fatal, and skipping
    /// the system silently would be worse than handing it a `None`.
    pub ptr: *mut u8,
}

/// Outcome of one system invocation.
///
/// A plugin system runs on the host's stack, so an escaping panic would abort
/// the whole process — the editor dies because someone indexed a slice wrong in
/// a half-written system. The ergonomic layer catches unwinds and reports
/// [`SystemStatus::Panicked`] instead, and the host **disables the system** so a
/// panic costs one frame rather than repeating forever.
#[repr(i32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SystemStatus {
    Ok = 0,
    Panicked = 1,
}

/// A plugin system. Invoked once per frame by the host.
pub type SystemEntry = unsafe extern "C" fn(call: *const SystemCall) -> SystemStatus;

// ── Rendering ────────────────────────────────────────────────────────────────
//
// Handle-based, like `wgpu-native`'s C API and Godot's RID system: every GPU
// object is an opaque integer the host maps back to the real thing. That is what
// lets a plugin drive the GPU without linking wgpu — and it means an invalid
// handle is a checked lookup failure rather than a wild pointer.
//
// TWO THINGS TO KNOW BEFORE EXTENDING THIS:
//
// 1. Any wgpu enum that crosses here (`TextureFormat`, `BufferUsages`, …) becomes
//    OUR frozen ABI. Bevy upgrades wgpu regularly, so each one added is a
//    permanent mapping-table maintenance cost. Mirror them explicitly; never
//    re-export wgpu's.
// 2. Resources a plugin creates are owned by the host on its behalf and must be
//    freed when the plugin unloads, or a reloaded plugin leaks VRAM every cycle.

/// A render pipeline the host built for a plugin.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PipelineId(pub u32);

impl PipelineId {
    pub const INVALID: PipelineId = PipelineId(u32::MAX);
    pub const fn is_valid(self) -> bool {
        self.0 != u32::MAX
    }
}

/// Opaque handle to an in-progress render pass. **Only valid inside the
/// [`RenderCallback`] that received it** — it borrows host render state that is
/// gone the moment the callback returns.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct RenderCtx(pub *mut c_void);

/// Where in the frame a plugin's pass runs. Mirrors the engine's own phase
/// ordering; `#[repr(u32)]` and append-only.
/// Newtype rather than an `enum`, and that is a soundness requirement rather
/// than a style choice.
///
/// The **plugin writes this value and the host reads it** — out of plugin memory
/// for the ones that live in structs, and straight off the FFI boundary for the
/// ones passed by value. Materialising an out-of-range discriminant into a Rust
/// enum is undefined behaviour, and not the harmless kind: rustc attaches
/// `!range` metadata to the load, so LLVM may legally assume the impossible and
/// a `match` can take an arbitrary arm.
///
/// That is exactly what a MINOR bump would cause. A plugin built against a newer
/// ABI writes a discriminant the older host has no variant for. The version
/// handshake is supposed to refuse that plugin — but then the soundness of every
/// appended variant rests on the handshake being bug-free, forever. A newtype
/// removes the question: any `u32` is a valid value, unknown ones fall to the
/// `_` arm, and "appending a variant is a MINOR change" is true rather than
/// merely usually true.
///
/// The constants below keep the variant names, so this is a source-compatible
/// change at every call site.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderPhase(pub u32);

#[allow(non_upper_case_globals)]
impl RenderPhase {
    /// HDR, after the main 3D pass, before temporal AA — GI, reflections.
    pub const Gi: Self = Self(0);
    /// HDR, after temporal AA — bloom, depth of field, motion blur.
    pub const HdrPost: Self = Self(1);
    /// LDR, after tonemapping — colour grading, vignette.
    pub const LdrPost: Self = Self(2);
    /// Final overlays, after AA.
    pub const Overlay: Self = Self(3);

    /// Whether this is a value this build knows. Anything else came from a
    /// plugin built against a newer ABI.
    pub const fn is_known(self) -> bool {
        self.0 < 4
    }

    /// The variant name, or `"?"` for a value from a newer ABI.
    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "Gi",
            1 => "HdrPost",
            2 => "LdrPost",
            3 => "Overlay",
            _ => "?",
        }
    }
}

impl core::fmt::Debug for RenderPhase {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_known() {
            f.write_str(self.name())
        } else {
            write!(f, "RenderPhase({})", self.0)
        }
    }
}

/// Records draw commands for one view. Runs inside the host's render graph.
pub type RenderCallback =
    unsafe extern "C" fn(ctx: RenderCtx, pipeline: PipelineId) -> SystemStatus;

/// A full-screen pass a plugin contributes to the frame.
///
/// The host compiles `fragment_wgsl` and builds the pipeline, because pipeline
/// creation needs the `RenderDevice` and `PipelineCache`, which live in the
/// render world. The plugin gets a [`PipelineId`] back when its callback runs.
///
/// The fragment shader is paired with the engine's fullscreen vertex shader and
/// gets the current view texture at binding 0 and a sampler at binding 1.
#[repr(C)]
pub struct RenderPassDesc {
    /// Stable id, e.g. `"my_plugin.tint"`. Shown in the editor's render-pass
    /// list and used to reorder passes.
    pub id: StrRef,
    pub fragment_wgsl: StrRef,
    pub phase: RenderPhase,
    /// Sort key within the phase — lower runs first.
    pub order: f32,
    pub callback: RenderCallback,
}

/// A parameterised full-screen effect.
///
/// The difference from [`RenderPassDesc`] is `settings`: a plugin component
/// whose bytes are uploaded to a uniform buffer each frame and bound at
/// `@group(0) @binding(2)`, so the shader can be *controlled* rather than fixed.
/// That is the whole gap between "a plugin can draw" and "a plugin can ship an
/// effect" — a pass with no parameters can only ever be a constant.
///
/// The host does extraction, the uniform buffer, the bind group and the draw.
/// A plugin describing an effect this way writes no render code at all; use
/// [`RenderPassDesc`] when you need to record commands yourself.
///
/// The settings component must be `#[repr(C)]` and laid out for std140 —
/// `vec3` fields need padding to 16 bytes, same as any Bevy uniform.
#[repr(C)]
pub struct PostProcessDesc {
    /// Stable id, e.g. `"my_plugin.bloom"`. Shown in the editor's render-pass
    /// list and used to reorder effects.
    pub id: StrRef,
    pub fragment_wgsl: StrRef,
    /// Plugin component carrying the uniform payload. Registered normally, so
    /// its field schema also drives the inspector.
    pub settings: ComponentId,
    /// Size of one settings instance, for the bind group layout.
    pub settings_size: u64,
    pub phase: RenderPhase,
    /// Sort key within the phase — lower runs first.
    pub order: f32,
}

// ── Assets ───────────────────────────────────────────────────────────────────

/// A mesh or material the host created for a plugin. Opaque index into a
/// host-side table — a plugin never sees a real `Handle`.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AssetHandle(pub u64);

impl AssetHandle {
    pub const INVALID: AssetHandle = AssetHandle(u64::MAX);
    pub const fn is_valid(self) -> bool {
        self.0 != u64::MAX
    }
}

/// Built-in mesh shapes.
///
/// A closed set rather than arbitrary vertex data: a plugin handing over raw
/// buffers needs the whole asset/GPU surface, whereas primitives cover the cases
/// that actually come up — spawning markers, blockout geometry, particles,
/// procedural layouts.
/// Newtype rather than an `enum`, and that is a soundness requirement rather
/// than a style choice.
///
/// The **plugin writes this value and the host reads it** — out of plugin memory
/// for the ones that live in structs, and straight off the FFI boundary for the
/// ones passed by value. Materialising an out-of-range discriminant into a Rust
/// enum is undefined behaviour, and not the harmless kind: rustc attaches
/// `!range` metadata to the load, so LLVM may legally assume the impossible and
/// a `match` can take an arbitrary arm.
///
/// That is exactly what a MINOR bump would cause. A plugin built against a newer
/// ABI writes a discriminant the older host has no variant for. The version
/// handshake is supposed to refuse that plugin — but then the soundness of every
/// appended variant rests on the handshake being bug-free, forever. A newtype
/// removes the question: any `u32` is a valid value, unknown ones fall to the
/// `_` arm, and "appending a variant is a MINOR change" is true rather than
/// merely usually true.
///
/// The constants below keep the variant names, so this is a source-compatible
/// change at every call site.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Primitive(pub u32);

#[allow(non_upper_case_globals)]
impl Primitive {
    pub const Cuboid: Self = Self(0);
    pub const Sphere: Self = Self(1);
    pub const Plane: Self = Self(2);
    pub const Cylinder: Self = Self(3);
    pub const Capsule: Self = Self(4);
    pub const Torus: Self = Self(5);

    /// Whether this is a value this build knows. Anything else came from a
    /// plugin built against a newer ABI.
    pub const fn is_known(self) -> bool {
        self.0 < 6
    }

    /// The variant name, or `"?"` for a value from a newer ABI.
    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "Cuboid",
            1 => "Sphere",
            2 => "Plane",
            3 => "Cylinder",
            4 => "Capsule",
            5 => "Torus",
            _ => "?",
        }
    }
}

impl core::fmt::Debug for Primitive {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_known() {
            f.write_str(self.name())
        } else {
            write!(f, "Primitive({})", self.0)
        }
    }
}

/// `size` is interpreted per primitive: full extents for `Cuboid` and `Plane`,
/// `x` = radius for `Sphere`, `x` = radius and `y` = height for `Cylinder` and
/// `Capsule`, `x` = major and `y` = minor radius for `Torus`.
#[repr(C)]
pub struct MeshDesc {
    pub primitive: Primitive,
    pub size: Vec3,
}

#[repr(C)]
pub struct MaterialDesc {
    /// Linear RGBA.
    pub color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: [f32; 4],
}

// ── Commands ─────────────────────────────────────────────────────────────────

/// What a queued command does.
/// Newtype rather than an `enum`, and that is a soundness requirement rather
/// than a style choice.
///
/// The **plugin writes this value and the host reads it** — out of plugin memory
/// for the ones that live in structs, and straight off the FFI boundary for the
/// ones passed by value. Materialising an out-of-range discriminant into a Rust
/// enum is undefined behaviour, and not the harmless kind: rustc attaches
/// `!range` metadata to the load, so LLVM may legally assume the impossible and
/// a `match` can take an arbitrary arm.
///
/// That is exactly what a MINOR bump would cause. A plugin built against a newer
/// ABI writes a discriminant the older host has no variant for. The version
/// handshake is supposed to refuse that plugin — but then the soundness of every
/// appended variant rests on the handshake being bug-free, forever. A newtype
/// removes the question: any `u32` is a valid value, unknown ones fall to the
/// `_` arm, and "appending a variant is a MINOR change" is true rather than
/// merely usually true.
///
/// The constants below keep the variant names, so this is a source-compatible
/// change at every call site.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommandKind(pub u32);

#[allow(non_upper_case_globals)]
impl CommandKind {
    /// Despawn `entity` and its descendants.
    pub const Despawn: Self = Self(0);
    /// Insert `component` on `entity`, copying `data_len` bytes from `data`.
    pub const Insert: Self = Self(1);
    /// Remove `component` from `entity`.
    pub const Remove: Self = Self(2);
    /// Make `entity` renderable: the host attaches the real `Mesh3d`,
    /// `MeshMaterial3d` and `Transform`.
    ///
    /// A dedicated command rather than exposing `Mesh3d` as a mirrored
    /// component, because a plugin has no `Handle` type and translating one
    /// through a raw byte copy would mean teaching the insert path about every
    /// host type that contains a handle. `data` holds a [`SpawnMeshDesc`].
    pub const SpawnMesh: Self = Self(3);
    /// Spawn an entity tree described in BSN. `data` is the UTF-8 source.
    ///
    /// The text names components rather than carrying their bytes, which is what
    /// lets one command construct **both** engine components (resolved through
    /// reflection) and the plugin's own (resolved through the field schema the
    /// host already holds for them). A plugin therefore needs no mirror for
    /// `Node`, `PointLight` or anything else it only ever constructs.
    ///
    /// `entity` is a reserved id used as the root of the **first** tree in the
    /// source; any further top-level entries spawn fresh. That is what lets
    /// `spawn_bsn` hand back a usable id in the same frame.
    pub const SpawnBsn: Self = Self(4);

    /// Whether this is a value this build knows. Anything else came from a
    /// plugin built against a newer ABI.
    pub const fn is_known(self) -> bool {
        self.0 < 5
    }

    /// The variant name, or `"?"` for a value from a newer ABI.
    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "Despawn",
            1 => "Insert",
            2 => "Remove",
            3 => "SpawnMesh",
            4 => "SpawnBsn",
            _ => "?",
        }
    }
}

impl core::fmt::Debug for CommandKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_known() {
            f.write_str(self.name())
        } else {
            write!(f, "CommandKind({})", self.0)
        }
    }
}

/// Payload for [`CommandKind::SpawnMesh`].
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SpawnMeshDesc {
    pub mesh: AssetHandle,
    pub material: AssetHandle,
    pub transform: Transform,
}

/// One queued structural change.
///
/// `data` is only read during the [`CommandSink::push`] call — the host copies
/// what it needs, so a plugin may point at a stack local.
#[repr(C)]
pub struct Command {
    pub kind: CommandKind,
    pub entity: Entity,
    pub component: ComponentId,
    pub data: *const u8,
    pub data_len: usize,
}

/// Structural changes for one system invocation.
///
/// A separate object rather than more [`Interface`] functions, because those
/// take a `Host` handle that is only valid during plugin init — the world is
/// borrowed by the query while a system runs, so there is nothing safe for such
/// a handle to point at. The sink is created per call and dies with it.
///
/// Commands are deferred and applied after the system, exactly like Bevy's:
/// spawning mid-iteration would invalidate the very rows being walked.
#[repr(C)]
pub struct CommandSink {
    /// Allocate an entity id usable immediately, even though the entity does not
    /// exist until commands are applied. Mirrors `Commands::spawn_empty`.
    pub reserve_entity: unsafe extern "C" fn(sink: *mut CommandSink) -> Entity,
    pub push: unsafe extern "C" fn(sink: *mut CommandSink, cmd: *const Command),
}

/// Severity for [`Interface::log`].
/// Newtype rather than an `enum`, and that is a soundness requirement rather
/// than a style choice.
///
/// The **plugin writes this value and the host reads it** — out of plugin memory
/// for the ones that live in structs, and straight off the FFI boundary for the
/// ones passed by value. Materialising an out-of-range discriminant into a Rust
/// enum is undefined behaviour, and not the harmless kind: rustc attaches
/// `!range` metadata to the load, so LLVM may legally assume the impossible and
/// a `match` can take an arbitrary arm.
///
/// That is exactly what a MINOR bump would cause. A plugin built against a newer
/// ABI writes a discriminant the older host has no variant for. The version
/// handshake is supposed to refuse that plugin — but then the soundness of every
/// appended variant rests on the handshake being bug-free, forever. A newtype
/// removes the question: any `u32` is a valid value, unknown ones fall to the
/// `_` arm, and "appending a variant is a MINOR change" is true rather than
/// merely usually true.
///
/// The constants below keep the variant names, so this is a source-compatible
/// change at every call site.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct LogLevel(pub u32);

#[allow(non_upper_case_globals)]
impl LogLevel {
    pub const Trace: Self = Self(0);
    pub const Debug: Self = Self(1);
    pub const Info: Self = Self(2);
    pub const Warn: Self = Self(3);
    pub const Error: Self = Self(4);

    /// Whether this is a value this build knows. Anything else came from a
    /// plugin built against a newer ABI.
    pub const fn is_known(self) -> bool {
        self.0 < 5
    }

    /// The variant name, or `"?"` for a value from a newer ABI.
    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "Trace",
            1 => "Debug",
            2 => "Info",
            3 => "Warn",
            4 => "Error",
            _ => "?",
        }
    }
}

impl core::fmt::Debug for LogLevel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_known() {
            f.write_str(self.name())
        } else {
            write!(f, "LogLevel({})", self.0)
        }
    }
}

// ── The interface ────────────────────────────────────────────────────────────

/// Opaque handle to host state. Passed back with every call; never dereferenced
/// by the plugin.
#[repr(C)]
pub struct Host {
    _private: [u8; 0],
}

/// The function table the host hands a plugin at load.
///
/// **Append-only.** Adding a function is a [`VERSION_MINOR`] bump and older
/// plugins keep loading, because they simply never read the new field. Removing
/// or reordering one breaks every plugin ever built, which is the thing this
/// whole design exists to prevent.
#[repr(C)]
pub struct Interface {
    pub version_major: u32,
    pub version_minor: u32,

    /// Register a plugin-owned component. Idempotent per name.
    pub register_component:
        unsafe extern "C" fn(host: *mut Host, desc: *const ComponentDesc) -> ComponentId,

    /// Look up a component the *host* owns, by type path — e.g.
    /// `"bevy_transform::components::transform::Transform"`. Returns
    /// [`ComponentId::INVALID`] if nothing matches.
    pub component_id_by_name:
        unsafe extern "C" fn(host: *mut Host, name: StrRef) -> ComponentId,

    /// Register a system. The host builds a matching Bevy query and inserts a
    /// dispatcher into `schedule`.
    /// Register a system. Returns why it was refused, if it was.
    pub add_system:
        unsafe extern "C" fn(host: *mut Host, desc: *const SystemDesc) -> RegisterStatus,

    /// Write a line to the engine log.
    ///
    /// A plugin has no stdout worth using and no `tracing` subscriber of its
    /// own, so without this its only way to report anything is to return an
    /// error code with no detail. Panic messages come through here too.
    pub log: unsafe extern "C" fn(host: *mut Host, level: LogLevel, msg: StrRef),

    // ── Added in MINOR 1 ─────────────────────────────────────────────────────
    // APPENDED, not inserted. These went in above `log` first time round, which
    // silently repointed every older plugin's `log` call at `add_render_pass`
    // with mismatched arguments. "Append-only" means the END of the struct — a
    // new field in the middle is a MAJOR break wearing a MINOR's clothes.
    /// Register a full-screen render pass. See [`RenderPassDesc`].
    pub add_render_pass: unsafe extern "C" fn(host: *mut Host, desc: *const RenderPassDesc),

    /// Bind the pass's pipeline. Call before [`Interface::render_draw`].
    pub render_set_pipeline: unsafe extern "C" fn(ctx: RenderCtx, pipeline: PipelineId),

    /// Issue a draw. For a fullscreen pass that is `render_draw(ctx, 3, 1)` —
    /// the engine's fullscreen vertex shader generates a covering triangle from
    /// the vertex index, so there is no vertex buffer to bind.
    pub render_draw: unsafe extern "C" fn(ctx: RenderCtx, vertices: u32, instances: u32),

    // ── Added in MINOR 2 ─────────────────────────────────────────────────────
    /// Register a parameterised full-screen effect. See [`PostProcessDesc`].
    pub add_post_process: unsafe extern "C" fn(host: *mut Host, desc: *const PostProcessDesc),

    // ── Added in MINOR 4 ─────────────────────────────────────────────────────
    /// Create a mesh asset. **Only valid during plugin init** — asset collections
    /// live behind `&mut World`, which the host holds during `renzora_plugin_init`
    /// and not while a system runs. Create what you need up front and keep the
    /// handles; that is also the cheap way round, since a primitive built once
    /// and spawned a thousand times shares one asset.
    pub add_mesh: unsafe extern "C" fn(host: *mut Host, desc: *const MeshDesc) -> AssetHandle,

    /// Create a material asset. Same init-only restriction as [`Self::add_mesh`].
    pub add_material:
        unsafe extern "C" fn(host: *mut Host, desc: *const MaterialDesc) -> AssetHandle,

    // ── Added in MINOR 5 ─────────────────────────────────────────────────────
    /// Register a plugin-owned resource and insert its default value.
    ///
    /// Takes the same [`ComponentDesc`] as a component — a resource in Bevy is a
    /// component on a hidden entity, so the layout, field schema and default all
    /// mean the same thing.
    pub register_resource:
        unsafe extern "C" fn(host: *mut Host, desc: *const ComponentDesc) -> ComponentId,
    /// Overwrite a registered resource with `len` bytes read from `value`.
    ///
    /// Separate from registration because registration is idempotent and
    /// insertion is not: two systems taking the same `ResMut` must not reset it,
    /// but `insert_resource(Config { .. })` must replace whatever is there.
    pub insert_resource: unsafe extern "C" fn(
        host: *mut Host,
        id: ComponentId,
        value: *const u8,
        len: usize,
    ),

    // ── Added in MINOR 1 ─────────────────────────────────────────────────────
    /// Register an editor panel. See [`PanelDesc`].
    pub add_panel: unsafe extern "C" fn(host: *mut Host, desc: *const PanelDesc) -> RegisterStatus,
}

// ── Editor panels ────────────────────────────────────────────────────────────

/// An editor panel, described as markup rather than built by calls.
///
/// The engine's UI is `bevy_ui` — which is exactly what this ABI hides, so a
/// plugin cannot be handed widget entities to assemble. It could be given an
/// immediate-mode call surface instead (`ui.label()`, `ui.button()`), but that
/// means one FFI call per widget per frame against a retained-mode UI that does
/// not want rebuilding, and it puts the engine's layout model into the ABI
/// permanently.
///
/// So a panel crosses as **text**, in the same block-structured shape scenes
/// use, naming widgets rather than components:
///
/// ```text
/// column {
///     label "Flock",
///     slider "Cohesion" 0.0 2.0 -> flock::FlockSettings.cohesion,
///     toggle "Enabled"          -> flock::FlockSettings.enabled,
///     row {
///         label "Boids",
///         value                 -> flock::FlockSettings.count,
///     },
///     button "Reset"            -> reset,
/// }
/// ```
///
/// `-> Type.field` **binds** a widget to a field of a plugin resource. The host
/// already knows that resource's schema — name, kind, byte offset — so it reads
/// and writes the field directly and no call into the plugin is needed for an
/// ordinary edit. `-> name` on a `button` is an **action**, and that is the only
/// thing that calls back.
///
/// The split matters: a slider dragged across a frame would otherwise be a call
/// per pixel.
#[repr(C)]
pub struct PanelDesc {
    /// Stable id, used for docking and layout persistence. Prefix it with the
    /// plugin name — two plugins claiming `settings` would fight over one dock
    /// slot and one layout entry.
    pub id: StrRef,
    pub title: StrRef,
    /// Kebab-case Phosphor icon name. Empty for the default.
    pub icon: StrRef,
    /// Section in the panel picker. Empty means "Plugins".
    pub category: StrRef,
    /// The markup above. Copied at registration; the plugin may free it after.
    pub markup: StrRef,
    /// Invoked when an action widget fires. May be null for a display-only
    /// panel.
    pub on_action: Option<PanelActionEntry>,
    /// Handed back in [`PanelAction::user`].
    pub user: *mut c_void,
}

/// One action, delivered synchronously while the click is being handled.
#[repr(C)]
pub struct PanelAction {
    /// The name after `->` on the widget that fired.
    pub name: StrRef,
    /// The widget's current value: a toggle's 0 or 1, a slider's position, 0 for
    /// a button.
    pub value: f32,
    pub user: *mut c_void,
    pub iface: *const Interface,
    /// Structural changes, same queue a system gets. Null if unavailable.
    pub commands: *mut CommandSink,
}

/// A panel's action handler. Returns [`SystemStatus::Panicked`] if the plugin
/// caught a panic, exactly like a system.
pub type PanelActionEntry = unsafe extern "C" fn(action: *const PanelAction) -> SystemStatus;

/// Result of [`ExtensionInit`].
#[repr(i32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InitResult {
    Ok = 0,
    /// The host's [`Interface`] is older than the plugin needs.
    VersionTooOld = 1,
    /// The plugin's own setup failed. It will not be loaded.
    Failed = 2,
}

/// The signature of [`INIT_SYMBOL`], the plugin's sole export.
///
/// Called once at load. The plugin registers its components and systems through
/// `iface` and returns [`InitResult::Ok`]. Anything else and the host unloads it
/// without ever calling into it again.
pub type ExtensionInit =
    unsafe extern "C" fn(iface: *const Interface, host: *mut Host) -> InitResult;
