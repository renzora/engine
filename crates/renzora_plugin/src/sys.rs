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
//!
//! # Editing anything in this file
//!
//! Which rule applies depends on **how the type crosses**, not on how small the
//! edit looks. Get it backwards and nothing tells you: both sides compile their
//! own copy of this file from independent source trees, so there is no link
//! error, no symbol to mismatch, and no version number that moves on its own.
//!
//! ## A table read by offset MAY be appended to
//!
//! [`Interface`], [`SystemCall`], [`FrameCtx`], [`CommandSink`], [`MeshSource`],
//! [`ImageSource`], [`HttpSource`] and [`PanelAction`] are handed over as a
//! pointer and read field by field. A build compiled against an older layout
//! reads the prefix it knows and never touches what follows, so a field added
//! **at the very end** is invisible to it.
//!
//! Append it, append its `"name: type"` to the golden list in
//! `tests/abi_order.rs`, and bump [`VERSION_MINOR`] in the same commit.
//!
//! Anything else — inserting, reordering, removing, retyping, or changing a
//! function pointer's signature in place — moves a slot an already-built plugin
//! is compiled to call, and needs [`VERSION_MAJOR`]. The signature change is the
//! one that hides: a fn pointer is one `usize` whatever its arity, so neither a
//! field-name list nor `size_of` moves when its shape does.
//!
//! ## Every other type here is FROZEN — not even appendable
//!
//! A struct crossing through a pointer ([`MeshDataDesc`], [`ComponentDesc`],
//! [`FieldDesc`], [`Command`], [`QueryView`], [`InputState`], every `*Desc`), by
//! value inside another ([`StrRef`], [`Vec3`], [`Quat`], [`Transform`]), or as
//! opaque payload bytes ([`SpawnMeshDesc`], [`ServiceCall`], the domain commands)
//! is **not read prefix-first**. The reader dereferences every field, walks an
//! array at its own `size_of`, or memcpys the whole thing. There is no prefix to
//! preserve, so appending is exactly as fatal as reordering.
//!
//! Four guards exist and all four are blind to this. `VERSION_MINOR` does not
//! move, because you did not touch a table. The load-time [`Interface`] prefix
//! hash covers the *spelling* of `*const MeshDataDesc`, not its contents. The
//! golden test cannot see inside a pointer. And the payload length checks are
//! `<` minimums, which a reorder passes and a growth passes.
//!
//! ## Instead: mint a new type beside the old one
//!
//! Add `MeshDataDesc2` next to [`MeshDataDesc`], add `add_mesh_data2` at the
//! **end** of [`Interface`], and leave the old struct and old slot untouched
//! forever. Freezing costs one dead struct; editing costs every prebuilt plugin,
//! silently.
//!
//! This is already the practice here twice, both times arrived at the hard way.
//! [`MeshColors`] is a separate *argument* to [`MeshSource::write`] rather than a
//! field on [`MeshDataDesc`], because that struct is shared with `add_mesh_data`.
//! [`FieldRange`] is a separate struct reached by its own `set_field_range`
//! rather than three fields on [`FieldDesc`], because widening `FieldDesc`
//! changes the stride of an array the host walks and yields garbage from element
//! one onward.
//!
//! ## By-value embedding is the trap that catches careful people
//!
//! [`StrRef`] is a field of seven boundary structs *and* a by-value argument of
//! two [`Interface`] functions. [`Vec3`] is a field of six *and* the element type
//! of four pointer arrays. Growing either is an ABI break authored in a different
//! part of the file from the struct it breaks: every golden list stays identical,
//! `size_of::<Interface>()` stays identical, and both sides compile.
//!
//! **Before editing any type here, grep for it as a field type.** If it appears
//! inside another boundary struct, you are editing that struct too.
//!
//! ## Some contracts are numbering, not shape
//!
//! A [`Key`]'s value **is** its bit index into [`InputState`]. Each domain's
//! `SERVICE` id is baked into every plugin ever shipped. `STR_CAP` and `NAME_CAP`
//! are inline capacities other crates reconstruct by hand. Renumbering or
//! resizing any of them compiles cleanly on both sides and silently remaps
//! behaviour — as frozen as a field order, and no layout test can see them.

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
///
/// Went 2 -> 3 to repair the [`Interface`] table, which had been **corrupted by
/// two mid-struct insertions shipped as MINOR bumps**. `add_mesh_data` was
/// correctly appended at MINOR 5; `add_material_shader` (MINOR 9) and
/// `add_image` (MINOR 11) were then each inserted *above* it. Function-pointer
/// tables are read by offset, so a plugin built at MINOR 5-10 would call the
/// slot it compiled against and land in a different function — passing, for
/// instance, a `MeshDataDesc*` to something that reads it as an `ImageDesc*` or
/// runs `from_utf8_unchecked` over vertex positions. `guard_host` catches
/// panics, not that.
///
/// This had to be MAJOR rather than a quiet reorder, because **no field order
/// makes every historical MINOR correct**: MINOR 5-8 expects `add_mesh_data` in
/// the slot MINOR 9-10 expects `add_material_shader` in. One of them is always
/// wrong, so the only honest fix is to reject them all by name. The fields are
/// now in true append order and a test pins it.
///
/// Went 3 -> 4 when [`SystemStatus`], [`RegisterStatus`] and [`InitResult`] became
/// `#[repr(transparent)]` newtypes. All three were real enums whose *values*
/// cross the boundary, which this file's own rule (above) says they must not be:
/// materialising an out-of-range discriminant is undefined behaviour, and rustc
/// attaches `!range` metadata to the load, so a `match` may take an arbitrary arm.
///
/// `RegisterStatus` is the one that made this urgent rather than tidy. It travels
/// host -> plugin, and the handshake **deliberately accepts a newer host** — so
/// appending a status under a MINOR would hand every already-built plugin a
/// discriminant it has no variant for. The rule as written only ever considered
/// the plugin -> host direction, which is how all three were missed.
///
/// Wire-identical: the bytes were `i32`/`u32` before and are `i32`/`u32` now, and
/// every `Status::Ok` call site still compiles. It is MAJOR only because the
/// *validity* contract changed, and because a host that has not been rebuilt
/// still treats an unknown value as a real variant.
pub const VERSION_MAJOR: u32 = 4;

/// Bumped when something is *appended*. Older plugins keep working; a plugin
/// needing the new function declares this as its minimum.
///
/// **Reset to 0 by the MAJOR bump above** — a MINOR only means anything relative
/// to one MAJOR. This sentence was here through the 2 -> 3 bump and was not
/// honoured then; the history below therefore runs 0 -> 13 across MAJOR 2 *and*
/// 3, and is kept as the record of what those releases actually claimed. MAJOR 4
/// starts at 0 for real.
///
/// ## MAJOR 4
///
/// 0 -> 1 appended `SystemCall::removed`.
/// 1 -> 2 appended `Access::Added` and `Access::Changed`.
/// 2 -> 3 appended `add_script_backend`.
/// 3 -> 4 appended `HttpSource::poll_stream` and [`HttpChunkRead`], for
///          responses that arrive in pieces rather than all at once.
/// 5 -> 6 appended `PanelAction::text`, so a panel's text inputs can reach the
///          plugin at all — before it, `value: f32` was the only channel.
/// 4 -> 5 appended `SystemCall::replies`, [`ReplySource`] and [`ReplyRead`] —
///          a generic host-to-plugin answer channel, so a domain needing a
///          reply no longer costs a bump of its own. Intended to be the LAST
///          per-domain source ever added.
///
/// ## MAJOR 2 and 3, for the record
///
/// 0 -> 1 appended `add_panel`.
/// 1 -> 2 appended `SystemCall::input`.
/// 2 -> 3 appended `set_field_range`.
/// 3 -> 4 appended `CommandKind::Service`.
/// 4 -> 5 appended `add_mesh_data`.
/// 5 -> 6 appended `FieldKind::Str` and [`Str256`].
/// 6 -> 7 appended `SystemCall::meshes`.
/// 7 -> 8 appended `SystemCall::http`.
/// 8 -> 9 appended `add_material_shader`.
/// 9 -> 10 appended `MeshSource::write`.
/// 10 -> 11 appended `add_image`, `SystemCall::images`, and material textures.
/// 11 -> 12 appended `CommandKind::SetMaterial`.
/// 12 -> 13 appended `prefix_hashes` and `prefix_count`, which let a plugin
///          verify the table's shape rather than trusting the two numbers above.
///
/// Two of those lines were false when written, which is the whole reason
/// [`VERSION_MAJOR`] is now 3: MINOR 9 and MINOR 11 *inserted* their functions
/// into the middle of [`Interface`] and recorded it here as "appended". The
/// history above is left intact rather than rewritten — it is what those
/// releases actually claimed, and the correction belongs beside the guarantee it
/// broke.
///
/// Note what is NOT in that list: animation. It shipped in the same release, as
/// `crate::anim` — a domain module riding on the generic service command above,
/// not boundary surface. That is the point of the split: a domain moves the
/// crate's own semver, and only a change to the *mechanism* moves this. A plugin
/// that wants audio some day should not have to declare a minimum ABI that also
/// encodes animation's history.
pub const VERSION_MINOR: u32 = 6;

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

impl Entity {
    /// An entity that is deliberately no entity.
    ///
    /// For service calls that are not about anything in the world — an HTTP
    /// request belongs to the plugin, not to a body — so the consumer still
    /// sees one shape whether or not the call has a subject. `u64::MAX` never
    /// round-trips through `Entity::try_from_bits`, so it cannot be mistaken
    /// for a real handle.
    pub const PLACEHOLDER: Self = Self(u64::MAX);
}

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
    /// A [`Str256`] — fixed-capacity inline UTF-8.
    pub const Str: Self = Self(5);

    /// Whether this is a value this build knows. Anything else came from a
    /// plugin built against a newer ABI.
    pub const fn is_known(self) -> bool {
        self.0 < 6
    }

    /// The variant name, or `"?"` for a value from a newer ABI.
    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "F32",
            1 => "I32",
            2 => "Bool",
            3 => "Vec3",
            4 => "Quat",
            5 => "Str",
            _ => "?",
        }
    }
}

/// Payload bytes in a [`Str256`]. The `+ 4` for the length makes the whole
/// thing 256 bytes.
pub const STR_CAP: usize = 252;

/// A fixed-capacity, inline UTF-8 string a plugin component can hold.
///
/// ## Why not just `String`
///
/// Component storage is allocated by the host from a layout the plugin
/// declares, and a plugin component is refused outright if it declares a
/// destructor — a `String` whose drop never runs leaks its buffer silently for
/// the life of the process. Storage is also read as raw bytes by the query
/// path, the scene serializer and the inspector, none of which can chase a
/// pointer into the plugin's heap and none of which could free it.
///
/// So the bytes live in the component. That costs 256 bytes per field whatever
/// the content, which is the trade: a name, a path or a line of 3D text fits
/// comfortably, and anything genuinely large belongs in an asset rather than a
/// component.
///
/// A single capacity rather than a family of sizes, deliberately. The size has
/// to be recoverable from [`FieldKind`] alone — [`FieldDesc`] is walked as an
/// array at a fixed stride, so growing it to carry a per-field length would be
/// a MAJOR break.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Str256 {
    pub bytes: [u8; STR_CAP],
    /// Bytes used. Always `<= STR_CAP`; every reader clamps rather than
    /// trusting it, because this crosses from another compilation unit.
    pub len: u32,
}

impl Str256 {
    pub const EMPTY: Self = Self { bytes: [0; STR_CAP], len: 0 };

    /// Copy `s` in, or `None` if it does not fit.
    ///
    /// `None` rather than truncating: a silently cut path or name fails to
    /// resolve later, somewhere with no memory of where the string came from.
    pub const fn new(s: &str) -> Option<Self> {
        let src = s.as_bytes();
        if src.len() > STR_CAP {
            return None;
        }
        let mut out = Self::EMPTY;
        let mut i = 0;
        while i < src.len() {
            out.bytes[i] = src[i];
            i += 1;
        }
        out.len = src.len() as u32;
        Some(out)
    }

    /// Copy `s` in, truncating at the last character boundary that fits.
    ///
    /// For the inspector and other interactive paths, where refusing a keystroke
    /// is worse than not accepting the tail of an over-long paste.
    pub fn new_truncating(s: &str) -> Self {
        let mut end = s.len().min(STR_CAP);
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        Self::new(&s[..end]).unwrap_or(Self::EMPTY)
    }

    /// The bytes in use, clamped to the capacity.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..(self.len as usize).min(STR_CAP)]
    }

    /// The contents, or `""` if a plugin wrote bytes that are not UTF-8.
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(self.as_bytes()).unwrap_or_default()
    }

    pub fn is_empty(&self) -> bool {
        self.as_bytes().is_empty()
    }
}

impl Default for Str256 {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl core::fmt::Debug for Str256 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.as_str(), f)
    }
}

impl core::fmt::Display for Str256 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl PartialEq for Str256 {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}
impl Eq for Str256 {}

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
    /// Match only rows whose component was added since this system last ran.
    /// Carries the component but produces **no cell** — it is a filter.
    pub const Added: Self = Self(11);
    /// Match only rows whose component changed since this system last ran.
    pub const Changed: Self = Self(12);

    /// Whether this is a value this build knows. Anything else came from a
    /// plugin built against a newer ABI.
    pub const fn is_known(self) -> bool {
        self.0 < 13
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
            11 => "Added",
            12 => "Changed",
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
/// Newtype rather than an `enum`, and this one crosses in the **opposite**
/// direction to most: the host produces it, the plugin materialises it.
///
/// That direction is the more dangerous of the two, and the rule as originally
/// written did not cover it — it was phrased entirely as "plugin writes, host
/// reads". The handshake **deliberately accepts a newer host**, because the table
/// is append-only. So a host that adds a fifth status hands every
/// already-compiled plugin a discriminant it has no variant for, and the plugin
/// materialises it where the host's panic guard cannot see the consequences.
///
/// The pressure to append is not hypothetical: the host already detects a
/// duplicate panel id and has to report the generic `Invalid` for it.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct RegisterStatus(pub u32);

#[allow(non_upper_case_globals)]
impl RegisterStatus {
    pub const Ok: Self = Self(0);
    /// A term named a component id the host does not have.
    pub const UnknownComponent: Self = Self(1);
    /// Bevy refused the access pattern — usually `&mut T` and `&T` on the same
    /// component in one system.
    pub const AccessConflict: Self = Self(2);
    /// The descriptor was malformed: a null pointer, no queries, or a non-zero
    /// `flags`.
    pub const Invalid: Self = Self(3);

    /// Whether this is a value this build knows. A plugin should treat anything
    /// else as a failure — the host is newer and refused for a reason this build
    /// has no name for.
    pub const fn is_known(self) -> bool {
        self.0 < 4
    }
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
    /// This frame's keyboard, mouse and cursor state. Null if the host has no
    /// input (a headless server), which a plugin sees as "nothing is pressed".
    ///
    /// Appended at the END of the struct, and it had to be: the obvious home was
    /// [`FrameCtx`], but that is embedded here by value, so growing it would shift
    /// `user`, `iface`, `host` and everything after — an old plugin would read the
    /// wrong offsets. Appending is what keeps this a MINOR bump.
    ///
    /// A whole snapshot rather than `is_pressed(key)` calls because input is read
    /// in bursts: a movement system asks about four keys and a mouse button, and
    /// five FFI calls per frame per system to answer questions the host already
    /// has in a bitset is the wrong trade.
    pub input: *const InputState,
    /// Reads the geometry of a mesh already in the world. Null if the host
    /// could not provide one (no renderer).
    ///
    /// Appended at the END, like [`input`](Self::input), and for the same
    /// reason — anything earlier shifts every field after it and an old plugin
    /// reads the wrong offsets.
    ///
    /// A per-call object rather than an [`Interface`] function, because reading
    /// a mesh needs the world and [`host`](Self::host) is null while a system
    /// runs. Same shape as [`commands`](Self::commands): created for the call,
    /// dead when it returns.
    pub meshes: *mut MeshSource,
    /// Replaces the pixels of plugin-created images. Null if the host could not
    /// provide one (no renderer).
    pub images: *mut ImageSource,
    /// Delivers completed HTTP responses. Null if the host has no HTTP.
    ///
    /// Appended at the END, like [`input`](Self::input) and
    /// [`meshes`](Self::meshes), for the same offset reason.
    pub http: *mut HttpSource,

    // ── Added in MINOR 4.1 ────────────────────────────────────────────────
    // NOTHING MAY BE INSERTED ABOVE THIS POINT.
    /// Which entities lost a component. Null in a build without the source.
    pub removed: *mut RemovedSource,

    // ── Added in MINOR 4.5 ────────────────────────────────────────────────
    // NOTHING MAY BE INSERTED ABOVE THIS POINT.
    /// Answers to [`CommandKind::Service`] calls. Null in a build with no
    /// consumer that replies.
    ///
    /// **The point of this field is that it is the last one of its kind.** Every
    /// source above it is a domain that needed the host to hand something back
    /// and paid a `VERSION_MINOR` bump for the privilege — meshes, images, HTTP,
    /// removed components. Meanwhile the *other* direction has been generic
    /// since MINOR 2.4: `call_service` carries any service id and any bytes, so
    /// adding a domain that only sends costs nothing.
    ///
    /// This closes that asymmetry. A domain that needs a reply now rides
    /// [`ReplySource`] keyed by its own service id, exactly as it rides
    /// `CommandKind::Service` going out, and adds no boundary surface at all.
    pub replies: *mut ReplySource,
}

/// One answer to a service call, copied out for the plugin.
///
/// Two-pass like [`HttpRead`]: probe for the length, allocate, fill. The payload
/// is domain-defined and arbitrarily large, and the host cannot allocate with
/// the plugin's allocator.
#[repr(C)]
pub struct ReplyRead {
    /// In: how many bytes `data` holds. Out: unchanged.
    pub data_capacity: usize,
    pub data: *mut u8,
    /// Out: the reply's full length, whatever the capacity was.
    pub data_len: usize,
    /// Out: domain-defined. A domain with one reply shape can ignore it; one
    /// with several — "picked a file" versus "cancelled" — uses it rather than
    /// inventing a sentinel inside the payload.
    pub op: u32,
    pub _pad: [u8; 4],
}

impl ReplyRead {
    /// A length-only probe — the first of the two passes.
    pub const COUNTS_ONLY: Self = Self {
        data_capacity: 0,
        data: core::ptr::null_mut(),
        data_len: 0,
        op: 0,
        _pad: [0; 4],
    };
}

/// Delivers answers to service calls during one system call.
///
/// The generic counterpart to [`CommandKind::Service`]. The host stores whatever
/// bytes a consumer produced; what they mean is the domain's business, exactly
/// as it is on the way out.
#[repr(C)]
pub struct ReplySource {
    /// Take the next reply for `service` and `tag`.
    ///
    /// Returns `false` when none is ready, which is the normal state — a reply
    /// takes at least a frame and often many. Delivered **once**: the probe pass
    /// does not consume it, the filling pass does.
    ///
    /// Keyed by `service` as well as `tag` so two domains cannot collide: tags
    /// are chosen by the plugin, and nothing stops it using `1` for both a
    /// dialog and some future domain.
    pub poll: unsafe extern "C" fn(
        src: *mut ReplySource,
        service: u64,
        tag: u64,
        out: *mut ReplyRead,
    ) -> bool,
}

/// One completed HTTP response, copied out for the plugin.
///
/// Two-pass like [`MeshRead`]: probe for the length, allocate, fill. A body is
/// arbitrarily large and the host cannot allocate with the plugin's allocator.
#[repr(C)]
pub struct HttpRead {
    /// In: how many bytes `body` holds. Out: unchanged.
    pub body_capacity: usize,
    pub body: *mut u8,
    /// Out: the response's full length, whatever the capacity was.
    pub body_len: usize,
    /// Out: HTTP status, or 0 if the request never completed — `body` then
    /// holds the error text.
    pub status: u16,
    pub _pad: [u8; 6],
}

impl HttpRead {
    /// A length-only probe — the first of the two passes.
    pub const COUNTS_ONLY: Self = Self {
        body_capacity: 0,
        body: core::ptr::null_mut(),
        body_len: 0,
        status: 0,
        _pad: [0; 6],
    };
}

/// What a [`HttpChunkRead`] is carrying.
///
/// A separate word rather than an overloaded `status`, because all three states
/// can legitimately carry a 200: a chunk, the end marker that follows the last
/// chunk, and an error that struck mid-stream after the headers were already
/// sent. Encoding "finished" as an empty body would collide with a server that
/// legitimately emits an empty chunk, which SSE keep-alives do.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct HttpChunkKind(pub u32);

#[allow(non_upper_case_globals)]
impl HttpChunkKind {
    /// Body bytes. More may follow.
    pub const Data: Self = Self(0);
    /// The stream ended normally. No more chunks for this tag.
    pub const End: Self = Self(1);
    /// The stream failed; the body holds the error text. Terminal, like `End`.
    pub const Error: Self = Self(2);

    pub const fn is_known(self) -> bool {
        self.0 < 3
    }

    /// Whether this is the last chunk for its tag.
    pub const fn is_terminal(self) -> bool {
        self.0 == 1 || self.0 == 2
    }

    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "Data",
            1 => "End",
            2 => "Error",
            _ => "?",
        }
    }
}

// Written out rather than derived, matching `HttpOp`: the value is an
// append-only integer, so an unknown one has to print as something rather than
// panic or print a bare number a reader cannot place.
impl core::fmt::Debug for HttpChunkKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}

/// One piece of a streaming HTTP response.
///
/// Deliberately a NEW struct rather than fields appended to [`HttpRead`]. That
/// struct is allocated by the *plugin* and written by the host, so growing it
/// would have a new host write past the end of an old plugin's allocation — the
/// append-only rule protects the [`Interface`] table, not plugin-owned memory.
/// A new struct has no older version to be confused with.
#[repr(C)]
pub struct HttpChunkRead {
    /// In: how many bytes `body` holds. Out: unchanged.
    pub body_capacity: usize,
    pub body: *mut u8,
    /// Out: this chunk's full length, whatever the capacity was.
    pub body_len: usize,
    /// Out: which of [`HttpChunkKind`] this is.
    pub kind: HttpChunkKind,
    /// Out: HTTP status, repeated on every chunk of a stream so a plugin that
    /// only keeps the latest chunk still knows it.
    pub status: u16,
    pub _pad: [u8; 2],
}

impl HttpChunkRead {
    /// A length-only probe — the first of the two passes.
    pub const COUNTS_ONLY: Self = Self {
        body_capacity: 0,
        body: core::ptr::null_mut(),
        body_len: 0,
        kind: HttpChunkKind::Data,
        status: 0,
        _pad: [0; 2],
    };
}

/// Delivers completed HTTP responses during one system call.
#[repr(C)]
pub struct HttpSource {
    /// Take the next completed response for `tag`.
    ///
    /// Returns `false` when none is ready, which is the normal state — a
    /// request takes many frames, so a plugin polls. A response is delivered
    /// **once**: the probe pass does not consume it, the filling pass does.
    pub poll: unsafe extern "C" fn(
        src: *mut HttpSource,
        tag: u64,
        out: *mut HttpRead,
    ) -> bool,

    // ── Added in MINOR 4.4 ────────────────────────────────────────────────────
    // NOTHING MAY BE INSERTED ABOVE THIS POINT.
    /// Take the next *chunk* for `tag`, for a request issued with one of the
    /// streaming verbs.
    ///
    /// Same two-pass, delivered-once contract as [`poll`](Self::poll). The
    /// difference is that one request yields many of these, in order, ending
    /// with a [`HttpChunkKind::End`] or [`HttpChunkKind::Error`] — so a caller
    /// polls until `is_terminal()` rather than until it gets something.
    ///
    /// Appending to this struct is safe in the direction that matters: the host
    /// allocates it, so an older plugin simply never reads this field, and a
    /// plugin *newer* than its host is refused by the `VERSION_MINOR` check
    /// before it can call anything.
    pub poll_stream: unsafe extern "C" fn(
        src: *mut HttpSource,
        tag: u64,
        out: *mut HttpChunkRead,
    ) -> bool,
}

/// Geometry copied out of a host mesh, for [`MeshSource::read`].
///
/// Two-pass by design. Call once with every capacity at 0 to learn the counts,
/// allocate, then call again with the buffers — the plugin owns its allocations
/// and the host never hands back a pointer into `Assets<Mesh>`, whose contents
/// can move or be freed the moment the call returns.
///
/// A buffer smaller than its count is filled to capacity and the count still
/// reports the true size, so a caller can detect the shortfall.
#[repr(C)]
pub struct MeshRead {
    /// In: how many the buffer holds. Out: unchanged.
    pub position_capacity: usize,
    pub positions: *mut Vec3,
    pub normal_capacity: usize,
    pub normals: *mut Vec3,
    pub uv_capacity: usize,
    pub uvs: *mut [f32; 2],
    pub index_capacity: usize,
    pub indices: *mut u32,
    /// Out: how many the mesh actually has, whatever the capacity was.
    pub position_count: usize,
    pub normal_count: usize,
    pub uv_count: usize,
    /// Out: 0 for an unindexed mesh, where every three positions are a face.
    pub index_count: usize,
}

impl MeshRead {
    /// A counts-only probe — the first of the two passes.
    pub const COUNTS_ONLY: Self = Self {
        position_capacity: 0,
        positions: core::ptr::null_mut(),
        normal_capacity: 0,
        normals: core::ptr::null_mut(),
        uv_capacity: 0,
        uvs: core::ptr::null_mut(),
        index_capacity: 0,
        indices: core::ptr::null_mut(),
        position_count: 0,
        normal_count: 0,
        uv_count: 0,
        index_count: 0,
    };
}

/// Reads geometry out of the world during one system call.
///
/// This is what lets a plugin do more than *emit* geometry — scattering points
/// over a surface, growing hair from a scalp, fitting a decal to a wall all
/// need the mesh that is already there.
#[repr(C)]
pub struct MeshSource {
    /// Fill `out` from the mesh on `entity`.
    ///
    /// Returns `false` if the entity is gone, has no mesh, or its mesh asset
    /// has not finished loading — which is a normal early-frame state, not an
    /// error, so a plugin should poll rather than give up.
    pub read: unsafe extern "C" fn(
        src: *mut MeshSource,
        entity: Entity,
        out: *mut MeshRead,
    ) -> bool,

    /// Replace the geometry of a mesh the plugin created with
    /// [`Interface::add_mesh_data`].
    ///
    /// `add_mesh_data` is init-only — it needs a `Host` handle, which is null
    /// while a system runs — so without this a plugin could generate geometry
    /// once and never again. Anything that rebuilds its mesh per frame (hair
    /// ribbons, a water surface, a deforming ribbon trail) needs to write from
    /// a system, which is where this lives.
    ///
    /// Validates exactly as `add_mesh_data` does and returns `false` on the
    /// same grounds, leaving the existing mesh untouched rather than replacing
    /// it with something malformed.
    /// `colors` may be null; it is a separate argument rather than a field on
    /// [`MeshDataDesc`] because that struct is shared with
    /// [`add_mesh_data`](Interface::add_mesh_data) and growing it would shift
    /// every field for a plugin built against the older layout.
    pub write: unsafe extern "C" fn(
        src: *mut MeshSource,
        handle: AssetHandle,
        data: *const MeshDataDesc,
        colors: *const MeshColors,
    ) -> bool,
}

/// Per-vertex colours for [`MeshSource::write`].
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MeshColors {
    /// Linear RGBA per vertex. Must match the position count.
    pub colors: *const [f32; 4],
    pub color_count: usize,
}

/// One frame of input, as the host saw it.
///
/// Bitsets rather than arrays of bools: 256 keys fit in 32 bytes, the whole struct
/// copies in one go, and "is this key down" is a shift and a mask on the plugin's
/// own side of the boundary.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct InputState {
    /// Currently held, indexed by [`Key`].
    pub keys_down: [u64; 4],
    /// Went down THIS frame.
    pub keys_just_pressed: [u64; 4],
    /// Came up this frame.
    pub keys_just_released: [u64; 4],
    /// Currently held, indexed by [`MouseButton`].
    pub mouse_down: u32,
    pub mouse_just_pressed: u32,
    pub mouse_just_released: u32,
    /// Cursor position in the primary window, in logical pixels, with the origin
    /// top-left. `NaN` when the cursor is outside the window — a plugin that
    /// forgets to check gets an obviously wrong number rather than a plausible
    /// `0, 0` in the corner.
    pub cursor_x: f32,
    pub cursor_y: f32,
    /// Movement since the previous frame. Zero when the cursor did not move, and
    /// still meaningful while the cursor is locked, which is why it is separate
    /// from the position rather than derived from it.
    pub cursor_delta_x: f32,
    pub cursor_delta_y: f32,
    /// Wheel movement this frame, in lines.
    pub scroll_x: f32,
    pub scroll_y: f32,
}

/// A keyboard key, as a stable number.
///
/// Deliberately NOT a copy of `bevy::KeyCode`'s discriminants. That enum is
/// `#[non_exhaustive]` and its values are an implementation detail, so a Bevy
/// upgrade that inserted a variant would silently remap every plugin's key
/// handling. These values are frozen here and the host maps `KeyCode` onto them,
/// which puts the cost of a Bevy change in one match statement instead of in
/// everybody's plugin.
///
/// Append-only, like every other newtype in this file. The value IS the bit index
/// into [`InputState::keys_down`], so nothing may be renumbered.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Key(pub u16);

#[allow(non_upper_case_globals)]
impl Key {
    // Letters, 0..25.
    pub const A: Self = Self(0);
    pub const B: Self = Self(1);
    pub const C: Self = Self(2);
    pub const D: Self = Self(3);
    pub const E: Self = Self(4);
    pub const F: Self = Self(5);
    pub const G: Self = Self(6);
    pub const H: Self = Self(7);
    pub const I: Self = Self(8);
    pub const J: Self = Self(9);
    pub const K: Self = Self(10);
    pub const L: Self = Self(11);
    pub const M: Self = Self(12);
    pub const N: Self = Self(13);
    pub const O: Self = Self(14);
    pub const P: Self = Self(15);
    pub const Q: Self = Self(16);
    pub const R: Self = Self(17);
    pub const S: Self = Self(18);
    pub const T: Self = Self(19);
    pub const U: Self = Self(20);
    pub const V: Self = Self(21);
    pub const W: Self = Self(22);
    pub const X: Self = Self(23);
    pub const Y: Self = Self(24);
    pub const Z: Self = Self(25);

    // Digit row, 26..35.
    pub const Digit0: Self = Self(26);
    pub const Digit1: Self = Self(27);
    pub const Digit2: Self = Self(28);
    pub const Digit3: Self = Self(29);
    pub const Digit4: Self = Self(30);
    pub const Digit5: Self = Self(31);
    pub const Digit6: Self = Self(32);
    pub const Digit7: Self = Self(33);
    pub const Digit8: Self = Self(34);
    pub const Digit9: Self = Self(35);

    // Editing and navigation, 36..51.
    pub const Space: Self = Self(36);
    pub const Enter: Self = Self(37);
    pub const Escape: Self = Self(38);
    pub const Tab: Self = Self(39);
    pub const Backspace: Self = Self(40);
    pub const Delete: Self = Self(41);
    pub const Insert: Self = Self(42);
    pub const Home: Self = Self(43);
    pub const End: Self = Self(44);
    pub const PageUp: Self = Self(45);
    pub const PageDown: Self = Self(46);
    pub const ArrowUp: Self = Self(47);
    pub const ArrowDown: Self = Self(48);
    pub const ArrowLeft: Self = Self(49);
    pub const ArrowRight: Self = Self(50);

    // Modifiers, 52..59. Left and right are distinct, because a game that binds
    // one of them cannot express that if they are merged.
    pub const ShiftLeft: Self = Self(52);
    pub const ShiftRight: Self = Self(53);
    pub const ControlLeft: Self = Self(54);
    pub const ControlRight: Self = Self(55);
    pub const AltLeft: Self = Self(56);
    pub const AltRight: Self = Self(57);
    pub const SuperLeft: Self = Self(58);
    pub const SuperRight: Self = Self(59);

    // Function row, 60..71.
    pub const F1: Self = Self(60);
    pub const F2: Self = Self(61);
    pub const F3: Self = Self(62);
    pub const F4: Self = Self(63);
    pub const F5: Self = Self(64);
    pub const F6: Self = Self(65);
    pub const F7: Self = Self(66);
    pub const F8: Self = Self(67);
    pub const F9: Self = Self(68);
    pub const F10: Self = Self(69);
    pub const F11: Self = Self(70);
    pub const F12: Self = Self(71);

    // Punctuation and numpad basics, 72..83.
    pub const Minus: Self = Self(72);
    pub const Equal: Self = Self(73);
    pub const BracketLeft: Self = Self(74);
    pub const BracketRight: Self = Self(75);
    pub const Backslash: Self = Self(76);
    pub const Semicolon: Self = Self(77);
    pub const Quote: Self = Self(78);
    pub const Comma: Self = Self(79);
    pub const Period: Self = Self(80);
    pub const Slash: Self = Self(81);
    pub const Backquote: Self = Self(82);
    pub const CapsLock: Self = Self(83);

    /// One past the highest assigned value. The bitset is `[u64; 4]` = 256 bits,
    /// so there is room to append without changing the wire format.
    pub const COUNT: u16 = 84;

    /// Whether this build knows the key. A value from a newer ABI is out of range
    /// of the bitset, and testing it would read a bit belonging to another key.
    pub const fn is_known(self) -> bool {
        self.0 < Self::COUNT
    }
}

/// A mouse button, indexed into [`InputState::mouse_down`].
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct MouseButton(pub u32);

#[allow(non_upper_case_globals)]
impl MouseButton {
    pub const Left: Self = Self(0);
    pub const Right: Self = Self(1);
    pub const Middle: Self = Self(2);
    pub const Back: Self = Self(3);
    pub const Forward: Self = Self(4);

    pub const COUNT: u32 = 5;

    pub const fn is_known(self) -> bool {
        self.0 < Self::COUNT
    }
}

impl InputState {
    /// Test a bit in one of the keyboard sets.
    ///
    /// Out-of-range keys read `false` rather than wrapping into another key's bit,
    /// which is what an unchecked `1 << (k % 64)` would do with a value from a
    /// newer ABI.
    #[inline]
    fn key_bit(set: &[u64; 4], key: Key) -> bool {
        if !key.is_known() {
            return false;
        }
        let i = key.0 as usize;
        set[i / 64] & (1u64 << (i % 64)) != 0
    }

    #[inline]
    pub fn pressed(&self, key: Key) -> bool {
        Self::key_bit(&self.keys_down, key)
    }
    #[inline]
    pub fn just_pressed(&self, key: Key) -> bool {
        Self::key_bit(&self.keys_just_pressed, key)
    }
    #[inline]
    pub fn just_released(&self, key: Key) -> bool {
        Self::key_bit(&self.keys_just_released, key)
    }

    #[inline]
    pub fn mouse_pressed(&self, button: MouseButton) -> bool {
        button.is_known() && self.mouse_down & (1 << button.0) != 0
    }
    #[inline]
    pub fn mouse_just_pressed(&self, button: MouseButton) -> bool {
        button.is_known() && self.mouse_just_pressed & (1 << button.0) != 0
    }
    #[inline]
    pub fn mouse_just_released(&self, button: MouseButton) -> bool {
        button.is_known() && self.mouse_just_released & (1 << button.0) != 0
    }

    /// Set a key bit. Host-side helper; ignores anything out of range.
    pub fn set_key(set: &mut [u64; 4], key: Key) {
        if key.is_known() {
            let i = key.0 as usize;
            set[i / 64] |= 1u64 << (i % 64);
        }
    }
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
/// Newtype rather than an `enum`, for the reason the module doc gives at length:
/// the **plugin** produces this value and the **host** materialises it, once per
/// system per frame. A real enum would make an out-of-range discriminant
/// undefined behaviour on the host — rustc attaches `!range` metadata to the
/// load, so a `match` may take an arbitrary arm rather than a `_` one.
///
/// It was a real enum until MAJOR 4, which made it the exception to a rule this
/// file spends thirty lines establishing. Nothing had gone wrong yet, but the
/// pressure to append a third status was already there.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SystemStatus(pub i32);

#[allow(non_upper_case_globals)]
impl SystemStatus {
    pub const Ok: Self = Self(0);
    /// The plugin caught a panic. The host disables the system.
    pub const Panicked: Self = Self(1);

    /// Whether this is a value this build knows. Anything else came from a
    /// plugin built against a newer ABI — treat it as a failure, not as `Ok`.
    pub const fn is_known(self) -> bool {
        self.0 == 0 || self.0 == 1
    }
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

/// Geometry a plugin generated itself, for [`Interface::add_mesh_data`].
///
/// Pointers are borrowed for the duration of the call only — the host copies
/// every slice into a `Mesh` before returning — so a plugin may point at stack
/// locals or at a `Vec` it drops immediately after.
///
/// Everything but `positions` is optional, and a null pointer is the documented
/// way to say "derive it". That matters more than it looks: a plugin that
/// generates geometry procedurally usually has positions and indices and
/// nothing else, and computing flat normals correctly (per-face, then averaged
/// per vertex) is the kind of thing every author would otherwise reimplement
/// slightly differently.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MeshDataDesc {
    pub positions: *const Vec3,
    pub position_count: usize,
    /// Null to have the host compute them from the faces.
    pub normals: *const Vec3,
    pub normal_count: usize,
    /// Null for zeroed UVs. `[u, v]` per vertex.
    pub uvs: *const [f32; 2],
    pub uv_count: usize,
    /// Null for an unindexed triangle list, i.e. every three positions are one
    /// face. Indexed geometry is strongly preferred for anything non-trivial.
    pub indices: *const u32,
    pub index_count: usize,
}

/// Pixel layout of a plugin-created image.
///
/// A deliberately short list. Every format here has to be one the host can
/// validate a byte count against and one wgpu will accept as a sampled texture
/// on every backend — widening it later is additive, guessing now is not.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageFormat(pub u32);

#[allow(non_upper_case_globals)]
impl ImageFormat {
    /// 8-bit RGBA, colour data. Sampled through sRGB decode — the right choice
    /// for anything an artist authored.
    pub const Rgba8Srgb: Self = Self(0);
    /// 8-bit RGBA, raw values. For data an artist did not author: masks,
    /// normal maps, packed channels.
    pub const Rgba8: Self = Self(1);
    /// Single-channel 32-bit float. Heightfields, distance fields, simulation
    /// state a plugin steps each frame.
    pub const R32Float: Self = Self(2);

    pub const fn is_known(self) -> bool {
        self.0 < 3
    }

    /// Bytes per pixel, used to check a plugin's buffer against its dimensions.
    pub const fn bytes_per_pixel(self) -> usize {
        match self.0 {
            0 | 1 => 4,
            2 => 4,
            _ => 0,
        }
    }

    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "Rgba8Srgb",
            1 => "Rgba8",
            2 => "R32Float",
            _ => "?",
        }
    }
}

impl core::fmt::Debug for ImageFormat {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}

/// An image a plugin generated, for [`Interface::add_image`].
///
/// `data` is borrowed for the call only — the host copies it before returning —
/// so a plugin may point at a buffer it drops immediately after.
#[repr(C)]
pub struct ImageDesc {
    pub width: u32,
    pub height: u32,
    pub format: ImageFormat,
    pub data: *const u8,
    /// Must be exactly `width * height * format.bytes_per_pixel()`. A mismatch
    /// is refused rather than padded: a short buffer uploaded as a full texture
    /// is a read past the plugin's heap into a GPU upload.
    pub data_len: usize,
}

/// Replaces the pixels of an image already created, during one system call.
///
/// Same shape and same reason as [`MeshSource`]: [`Interface::add_image`] needs
/// the init-time host handle, so without this a plugin could generate a texture
/// once and never again — and a simulation that steps a heightfield every frame
/// is the main thing textures are for on this side.
#[repr(C)]
pub struct ImageSource {
    /// Overwrite `handle`'s pixels. The dimensions and format are fixed at
    /// creation; only the contents change, so `len` must still match.
    ///
    /// Returns `false` if the handle is unknown or the length is wrong, leaving
    /// the existing pixels untouched.
    pub write: unsafe extern "C" fn(
        src: *mut ImageSource,
        handle: AssetHandle,
        data: *const u8,
        len: usize,
    ) -> bool,
}

/// Textures a material binds, beyond its uniform block.
///
/// Bound from `@group(3) @binding(1)` upward, each texture followed by its
/// sampler — so the first is `1`/`2`, the second `3`/`4`, and so on.
pub const MAX_MATERIAL_TEXTURES: usize = 4;

/// How a custom material blends.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlphaMode(pub u32);

#[allow(non_upper_case_globals)]
impl AlphaMode {
    pub const Opaque: Self = Self(0);
    /// Alpha-tested: a fragment is drawn or discarded, never blended.
    pub const Mask: Self = Self(1);
    /// Sorted and blended, drawn after opaques — which is also what makes the
    /// scene's colour available for screen-space refraction.
    pub const Blend: Self = Self(2);

    pub const fn is_known(self) -> bool {
        self.0 < 3
    }
}

impl Default for AlphaMode {
    fn default() -> Self {
        Self::Opaque
    }
}

/// Bytes a plugin material's uniform block may occupy.
///
/// Fixed, and it has to be: a material's bind-group layout is decided once per
/// *type* in Bevy, not per instance, and every plugin material shares one type
/// on the host. So the layout reserves this much and a plugin uses what it
/// needs — 256 bytes is comfortably more than a material's worth of parameters
/// and still one small uniform buffer per instance.
pub const MATERIAL_UNIFORM_CAP: u64 = 256;

/// A custom shaded material: WGSL plus a component whose bytes are its uniform.
///
/// The settings component is the same idea [`PostProcessDesc`] uses — the
/// plugin declares an ordinary component, and its bytes are uploaded as the
/// uniform block. That keeps one description of the parameters, editable in the
/// inspector, serialised into scenes, and readable by the plugin's own systems,
/// instead of a second parallel struct that exists only for the GPU.
#[repr(C)]
pub struct MaterialShaderDesc {
    /// Stable name, used for the shader asset and for hot-reload.
    pub id: StrRef,
    /// WGSL source. Needs a `fragment` entry point only — the vertex stage is
    /// Bevy's, so skinning, morph targets and the instance-indexed model
    /// transform come for free.
    ///
    /// Compiled through Bevy's normal pipeline rather than validated directly
    /// the way a post-process shader is, so naga_oil imports work here.
    /// `#import bevy_pbr::forward_io::VertexOutput` is required, since that is
    /// what the vertex stage hands the fragment.
    pub wgsl: StrRef,
    /// Component whose bytes become the uniform at `@group(3) @binding(0)`.
    pub settings: ComponentId,
    /// Size of that component. Refused above [`MATERIAL_UNIFORM_CAP`].
    pub settings_size: u64,
    pub alpha_mode: AlphaMode,
    /// Handles from [`Interface::add_image`], bound from `@group(3) @binding(1)`
    /// upward with each texture followed by its sampler. Null for none.
    ///
    /// Fixed at registration rather than per-instance, because the bind-group
    /// layout is decided once for the shared material type — the same
    /// constraint that fixes the uniform size. A material that needs its
    /// texture to change writes new pixels into the same handle with
    /// [`ImageSource::write`] instead of swapping the binding.
    pub textures: *const AssetHandle,
    /// Refused above [`MAX_MATERIAL_TEXTURES`].
    pub texture_count: usize,
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
    /// Hand an opaque payload to whichever engine crate claims a service.
    /// `data` is a [`ServiceCall`] header followed by the payload bytes.
    ///
    /// **This is how a domain reaches plugins without this crate learning the
    /// domain.** Animation, audio, physics and navigation are all things a
    /// plugin wants to *do*, and none of them can be a field write — "play this
    /// clip" retargets an animation graph. The obvious design gives each one its
    /// own command kind and its own structs here, and it is the wrong one: this
    /// module is the frozen mechanism, and it would accumulate a vocabulary per
    /// domain, with every domain addition bumping the ABI for plugins that use
    /// none of them.
    ///
    /// So the host carries bytes it does not interpret. A service names itself
    /// with [`service_id`], the payload layout is an agreement between the
    /// plugin and whoever drains the queue, and adding a whole new domain
    /// touches nothing here.
    ///
    /// A command rather than an [`Interface`] function because these are
    /// **structural** in the same sense a spawn is, and because deferring lets a
    /// plugin call one from inside a loop over its own query — which is where
    /// you actually decide to play something.
    ///
    /// Nothing draining a service is a valid configuration: the calls are
    /// discarded each frame, which is the right outcome for a dedicated server
    /// or a lean export that dropped the crate in question.
    pub const Service: Self = Self(5);
    /// Replace `entity`'s material, leaving its mesh and transform alone.
    /// `data` holds a [`SpawnMeshDesc`], of which only `material` is read.
    ///
    /// [`SpawnMesh`](Self::SpawnMesh) could not do this. It sets all three of
    /// mesh, material and transform, so a plugin wanting to shade geometry
    /// somebody else authored had to supply a mesh it did not have — and could
    /// not get, since `Mesh3d` is deliberately opaque and hands back no handle
    /// to pass through. The result was that a plugin material only ever worked
    /// on shapes the plugin spawned itself, which is most of the way to useless:
    /// the interesting case is a custom shader on an imported model.
    ///
    /// Reuses `SpawnMeshDesc` rather than defining a one-field struct, so the
    /// two commands stay obviously related and there is one less layout to keep
    /// frozen forever.
    pub const SetMaterial: Self = Self(6);

    /// Whether this is a value this build knows. Anything else came from a
    /// plugin built against a newer ABI.
    pub const fn is_known(self) -> bool {
        self.0 < 7
    }

    /// The variant name, or `"?"` for a value from a newer ABI.
    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "Despawn",
            1 => "Insert",
            2 => "Remove",
            3 => "SpawnMesh",
            4 => "SpawnBsn",
            5 => "Service",
            6 => "SetMaterial",
            _ => "?",
        }
    }
}

// ── Services ─────────────────────────────────────────────────────────────────

/// FNV-1a over a string, `const` so an id folds to a literal at the call site.
///
/// Generic on purpose — this is a hashing primitive, not a service registry.
/// The mechanism here never enumerates services, and adding one is not a change
/// to this file.
pub const fn fnv1a(s: &str) -> u64 {
    let bytes = s.as_bytes();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut i = 0;
    while i < bytes.len() {
        h ^= bytes[i] as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
        i += 1;
    }
    h
}

/// Names a service. Use a dotted, owner-qualified string —
/// `service_id("renzora.animation")` — so two crates cannot collide by both
/// picking `"audio"`.
pub const fn service_id(name: &str) -> u64 {
    fnv1a(name)
}

/// Header for a [`CommandKind::Service`] payload.
///
/// The payload follows this struct in the same `data` buffer, so a service
/// defines its own argument layout without this module knowing any of them. The
/// sink deep-copies `data`, which means a payload must be plain-old-data: a
/// pointer inside it would survive the copy as a pointer and be read after the
/// system returned.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ServiceCall {
    /// From [`service_id`].
    pub service: u64,
    /// Which operation, in the service's own numbering.
    pub op: u32,
    /// Keeps the payload that follows 8-byte aligned.
    pub _pad: u32,
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

/// FNV-1a over a string, folded into a running hash.
///
/// Chosen because it is trivially `const` — the whole point is a value the
/// compiler can produce for a struct declaration, and a stronger hash that needs
/// runtime code would be useless here.
const fn fnv_str(mut h: u64, s: &str) -> u64 {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        h ^= b[i] as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
        i += 1;
    }
    // A separator, and it is load-bearing rather than tidy: without it the pair
    // ("add_image", "unsafe fn(..)") hashes identically to
    // ("add_imageunsafe", " fn(..)"), so a rename that moved a character across
    // the boundary would be invisible.
    h ^= 0xff;
    h.wrapping_mul(0x0000_0100_0000_01b3)
}

/// Declares [`Interface`] and, from the same field list, the prefix hashes that
/// let a plugin verify the table's *shape* before it calls through it.
///
/// The version handshake compares two numbers a human types, so it cannot see
/// that the table was reordered — and that is not hypothetical: two functions
/// were once inserted mid-struct and shipped as MINOR bumps, which would have
/// sent an older plugin's call into a different function. A build-time test now
/// pins the order for this repository, but a plugin compiled elsewhere against an
/// older header has no such protection, and third-party prebuilt plugins are the
/// entire reason this ABI exists.
///
/// So the declaration also emits [`INTERFACE_PREFIX_HASHES`], where entry *n* is a
/// chain over the first *n* fields' `(name, written type)`. Appending a field
/// leaves every earlier entry untouched — which is exactly the property the
/// append-only rule claims — while inserting, reordering or retyping one moves
/// every entry from that point on. A plugin checks the entry for its own field
/// count and is refused if the prefix it compiled against is not the prefix the
/// host is offering.
macro_rules! interface {
    ($( $(#[$meta:meta])* $field:ident : $ty:ty ),* $(,)?) => {
        /// The function table the host hands a plugin at load.
        ///
        /// **Append-only.** Adding a function is a [`VERSION_MINOR`] bump and
        /// older plugins keep loading, because they simply never read the new
        /// field. Removing or reordering one breaks every plugin ever built,
        /// which is the thing this whole design exists to prevent — and since
        /// MINOR 13 a violation is a refused load rather than a wrong call, via
        /// [`INTERFACE_PREFIX_HASHES`].
        #[repr(C)]
        pub struct Interface {
            $( $(#[$meta])* pub $field: $ty, )*
        }

        /// Number of fields in [`Interface`] as this build declares it.
        pub const INTERFACE_FIELDS: usize = [$( ::core::stringify!($field) ),*].len();

        /// `[n]` is the hash of the first `n` fields. `[0]` is the empty prefix.
        ///
        /// Append-stable by construction: adding a field only appends an entry.
        pub static INTERFACE_PREFIX_HASHES: [u64; INTERFACE_FIELDS + 1] = {
            let names = [$( ::core::stringify!($field) ),*];
            // The type as *written*, which makes this deliberately conservative:
            // it hashes the source text, so `*const ImageDesc` and
            // `*const self::ImageDesc` differ, and so does a renamed parameter,
            // even though neither changes the ABI. Those are false positives, and
            // they are the right way to be wrong — a false positive costs a
            // rebuild, a false negative costs a call into the wrong function.
            //
            // The practical consequence, worth knowing before you tidy this
            // struct: **re-spelling a type or renaming a parameter refuses every
            // prebuilt plugin**, exactly as a real reorder would. Treat cosmetic
            // edits here as ABI edits, or do not make them.
            let types = [$( ::core::stringify!($ty) ),*];
            let mut out = [0u64; INTERFACE_FIELDS + 1];
            out[0] = 0xcbf2_9ce4_8422_2325;
            let mut i = 0;
            while i < INTERFACE_FIELDS {
                out[i + 1] = fnv_str(fnv_str(out[i], names[i]), types[i]);
                i += 1;
            }
            out
        };
    };
}

/// The table is shared across threads: the host builds one `static` and every
/// plugin system, on whichever thread the executor picks, reads through it.
///
/// Sound because it is immutable after construction and its one data pointer
/// addresses a `static` array of plain `u64`. It needs saying explicitly only
/// because a raw pointer is not `Sync`, and until MINOR 13 the struct held
/// nothing but function pointers and so derived `Sync` on its own.
unsafe impl Sync for Interface {}

// The struct's own documentation lives on the macro, since that is what emits it.
interface! {
    version_major: u32,
    version_minor: u32,

    /// Register a plugin-owned component. Idempotent per name.
    register_component:
        unsafe extern "C" fn(host: *mut Host, desc: *const ComponentDesc) -> ComponentId,

    /// Look up a component the *host* owns, by type path — e.g.
    /// `"bevy_transform::components::transform::Transform"`. Returns
    /// [`ComponentId::INVALID`] if nothing matches.
    component_id_by_name:
        unsafe extern "C" fn(host: *mut Host, name: StrRef) -> ComponentId,

    /// Register a system. The host builds a matching Bevy query and inserts a
    /// dispatcher into `schedule`.
    /// Register a system. Returns why it was refused, if it was.
    add_system:
        unsafe extern "C" fn(host: *mut Host, desc: *const SystemDesc) -> RegisterStatus,

    /// Write a line to the engine log.
    ///
    /// A plugin has no stdout worth using and no `tracing` subscriber of its
    /// own, so without this its only way to report anything is to return an
    /// error code with no detail. Panic messages come through here too.
    log: unsafe extern "C" fn(host: *mut Host, level: LogLevel, msg: StrRef),

    // ── Added in MINOR 1 ─────────────────────────────────────────────────────
    // APPENDED, not inserted. These went in above `log` first time round, which
    // silently repointed every older plugin's `log` call at `add_render_pass`
    // with mismatched arguments. "Append-only" means the END of the struct — a
    // new field in the middle is a MAJOR break wearing a MINOR's clothes.
    /// Register a full-screen render pass. See [`RenderPassDesc`].
    add_render_pass: unsafe extern "C" fn(host: *mut Host, desc: *const RenderPassDesc),

    /// Bind the pass's pipeline. Call before [`Interface::render_draw`].
    render_set_pipeline: unsafe extern "C" fn(ctx: RenderCtx, pipeline: PipelineId),

    /// Issue a draw. For a fullscreen pass that is `render_draw(ctx, 3, 1)` —
    /// the engine's fullscreen vertex shader generates a covering triangle from
    /// the vertex index, so there is no vertex buffer to bind.
    render_draw: unsafe extern "C" fn(ctx: RenderCtx, vertices: u32, instances: u32),

    // ── Added in MINOR 2 ─────────────────────────────────────────────────────
    /// Register a parameterised full-screen effect. See [`PostProcessDesc`].
    add_post_process: unsafe extern "C" fn(host: *mut Host, desc: *const PostProcessDesc),

    // ── Added in MINOR 4 ─────────────────────────────────────────────────────
    /// Create a mesh asset. **Only valid during plugin init** — asset collections
    /// live behind `&mut World`, which the host holds during `renzora_plugin_init`
    /// and not while a system runs. Create what you need up front and keep the
    /// handles; that is also the cheap way round, since a primitive built once
    /// and spawned a thousand times shares one asset.
    add_mesh: unsafe extern "C" fn(host: *mut Host, desc: *const MeshDesc) -> AssetHandle,

    /// Create a material asset. Same init-only restriction as [`Self::add_mesh`].
    add_material:
        unsafe extern "C" fn(host: *mut Host, desc: *const MaterialDesc) -> AssetHandle,

    // ── Added in MINOR 5 ─────────────────────────────────────────────────────
    /// Register a plugin-owned resource and insert its default value.
    ///
    /// Takes the same [`ComponentDesc`] as a component — a resource in Bevy is a
    /// component on a hidden entity, so the layout, field schema and default all
    /// mean the same thing.
    register_resource:
        unsafe extern "C" fn(host: *mut Host, desc: *const ComponentDesc) -> ComponentId,
    /// Overwrite a registered resource with `len` bytes read from `value`.
    ///
    /// Separate from registration because registration is idempotent and
    /// insertion is not: two systems taking the same `ResMut` must not reset it,
    /// but `insert_resource(Config { .. })` must replace whatever is there.
    insert_resource: unsafe extern "C" fn(
        host: *mut Host,
        id: ComponentId,
        value: *const u8,
        len: usize,
    ),

    // ── Added in MINOR 1 ─────────────────────────────────────────────────────
    /// Register an editor panel. See [`PanelDesc`].
    add_panel: unsafe extern "C" fn(host: *mut Host, desc: *const PanelDesc) -> RegisterStatus,

    // ── Added in MINOR 3 ─────────────────────────────────────────────────────
    /// Give a registered field an editing range, so the inspector draws a bounded
    /// slider instead of an unbounded drag.
    ///
    /// A separate call rather than fields on [`FieldDesc`], and that is forced
    /// rather than chosen: `FieldDesc` crosses as an ARRAY, which the host walks
    /// using its own `size_of`. Widening it would make the host read a plugin's
    /// array at the wrong stride from element 1 onward — garbage offsets, garbage
    /// kinds, silently. Appending a function to this table has no such problem,
    /// because a plugin only ever reads the prefix it was built against.
    ///
    /// `field` is the index into the `fields` array the component was registered
    /// with. Out of range is ignored.
    set_field_range: unsafe extern "C" fn(
        host: *mut Host,
        component: ComponentId,
        field: usize,
        range: *const FieldRange,
    ) -> RegisterStatus,

    // ── Added in MINOR 5 ──────────────────────────────────────────────────
    /// Upload geometry a plugin generated itself. See [`MeshDataDesc`].
    ///
    /// Separate from [`add_mesh`](Self::add_mesh) rather than a `Primitive`
    /// variant, because the two have nothing in common at the boundary: one
    /// passes a shape and three floats by value, the other borrows four slices
    /// that the host must copy before returning.
    ///
    /// This is what a plugin needs to be more than a consumer of built-in
    /// shapes — text meshes, procedural foliage, hair ribbons, water surfaces
    /// all generate their own vertices, and without it they cannot exist
    /// outside an engine crate.
    add_mesh_data: unsafe extern "C" fn(
        host: *mut Host,
        desc: *const MeshDataDesc,
    ) -> AssetHandle,

    // ── Added in MINOR 9 ──────────────────────────────────────────────────
    /// Register a custom shaded material. See [`MaterialShaderDesc`].
    ///
    /// The returned handle is used exactly like [`add_material`](Self::add_material)'s,
    /// so a plugin can hand it to `spawn_mesh` without caring which kind it is.
    add_material_shader: unsafe extern "C" fn(
        host: *mut Host,
        desc: *const MaterialShaderDesc,
    ) -> AssetHandle,

    // ── Added in MINOR 11 ─────────────────────────────────────────────────
    // NOTHING MAY BE INSERTED ABOVE THIS POINT. A new function goes here, at
    // the very end, under a new header. See `boundary_layouts_are_pinned` in
    // `tests/abi_order.rs`, which fails if this rule is broken again.
    /// Upload an image a plugin generated. See [`ImageDesc`].
    ///
    /// Init-only, like the other asset constructors — it needs the `Host`
    /// handle. Contents can be replaced from a system with
    /// [`ImageSource::write`]; dimensions and format cannot.
    add_image: unsafe extern "C" fn(
        host: *mut Host,
        desc: *const ImageDesc,
    ) -> AssetHandle,

    // ── Added in MINOR 13 ─────────────────────────────────────────────────
    // NOTHING MAY BE INSERTED ABOVE THIS POINT. A new function goes here, at
    // the very end, under a new header. See `boundary_layouts_are_pinned` in
    // `tests/abi_order.rs`, which fails if this rule is broken again.
    /// `[n]` is the host's hash of the first `n` fields of this struct.
    ///
    /// A plugin reads the entry for its own field count and compares it with the
    /// one its build computed. Equal means the host's table starts with exactly
    /// the fields the plugin compiled against, whatever was appended after them.
    ///
    /// Points at [`INTERFACE_PREFIX_HASHES`], which is `static`, so the pointer
    /// stays valid for the life of the process.
    prefix_hashes: *const u64,
    /// Length of [`Self::prefix_hashes`] — that is, `INTERFACE_FIELDS + 1` as the
    /// host declares it. A plugin whose own field count exceeds this is reading a
    /// table older than the one it was built for, which the version check should
    /// already have caught; it is re-checked because the consequence of not
    /// checking is an out-of-bounds read.
    prefix_count: usize,

    // ── Added in MAJOR 4, MINOR 3 ─────────────────────────────────────────
    // NOTHING MAY BE INSERTED ABOVE THIS POINT. A new function goes here, at
    // the very end, under a new header. See `boundary_layouts_are_pinned` in
    // `tests/abi_order.rs`, which fails if this rule is broken again.
    /// Register a scripting language. See [`ScriptBackendDesc`].
    ///
    /// The odd one out among the domains, and the reason it is here rather than
    /// riding on [`CommandKind::Service`] like animation and physics do: every
    /// other domain is a plugin *asking the engine* to do something, which a
    /// queued opaque payload expresses perfectly. Scripting is the reverse. The
    /// engine has to call *into* the plugin, once per scripted entity per
    /// frame, and get an answer back — and there is no way to express "call me"
    /// with a command queue. So the plugin hands over an entry point, which
    /// means a table entry, which means a MINOR bump.
    add_script_backend:
        unsafe extern "C" fn(host: *mut Host, desc: *const ScriptBackendDesc) -> RegisterStatus,
}

/// How the inspector should edit one numeric field.
///
/// Absent means "unbounded drag", which is what every field got before this
/// existed and remains the default for a plugin that says nothing.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FieldRange {
    pub min: f32,
    pub max: f32,
    /// Units per pixel of drag. `0.0` asks the host to pick from the range.
    pub speed: f32,
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

    // ── Added in MINOR 4.6 ────────────────────────────────────────────────
    // NOTHING MAY BE INSERTED ABOVE THIS POINT.
    /// The widget's text, for widgets that have any — a text input's current
    /// contents. Empty for a button, a toggle or a slider.
    ///
    /// [`value`](Self::value) being an `f32` meant a panel could *show* a text
    /// box and the plugin could never learn what was typed in it, which ruled
    /// out every form, search field and prompt. A `StrRef` rather than a
    /// [`Str256`] because a prompt has no natural cap and this borrows for the
    /// duration of the call rather than copying.
    ///
    /// **Valid only until the handler returns.** It points into host memory that
    /// the next frame may reuse; a plugin that wants to keep it must copy it,
    /// which is what `Action::text` returning `&str` makes obvious.
    pub text: StrRef,
}

/// A panel's action handler. Returns [`SystemStatus::Panicked`] if the plugin
/// caught a panic, exactly like a system.
pub type PanelActionEntry = unsafe extern "C" fn(action: *const PanelAction) -> SystemStatus;

/// Entities that lost a component, copied out for a plugin.
///
/// Two passes, like [`MeshRead`]: the first learns the count with null pointers,
/// the second fills buffers the plugin owns. The host cannot allocate for the
/// plugin — they do not share an allocator — so this is the only shape available.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RemovedRead {
    /// In: how many entities `entities` holds. Out: unchanged.
    pub entity_capacity: usize,
    pub entities: *mut Entity,
    /// Out: how many were removed, whatever the capacity was.
    pub entity_count: usize,
}

impl RemovedRead {
    /// A probe pass: no buffers, just the count.
    pub const COUNTS_ONLY: Self = Self {
        entity_capacity: 0,
        entities: core::ptr::null_mut(),
        entity_count: 0,
    };
}

/// Reads which entities lost a component since this system last ran.
///
/// A system param rather than an [`Interface`] function, for the same reason
/// [`MeshSource`] is: delivery needs host state, and `SystemCall::host` is null
/// while a system runs.
///
/// The cursor is **per system**, matching Bevy: two plugin systems watching the
/// same component each see every removal once, and a system that does not run on
/// a given frame still sees that frame's removals when it next runs. That last
/// part is why this is a cursor rather than a snapshot of the current frame —
/// the buffers rotate, and a snapshot would drop removals for any system that
/// skipped a frame.
#[repr(C)]
pub struct RemovedSource {
    /// Read removals of `component` since this system last asked.
    ///
    /// Returns `false` if nothing has ever removed that component — which is not
    /// an error, and is the normal state for most components on most frames.
    pub read: unsafe extern "C" fn(
        src: *mut RemovedSource,
        component: ComponentId,
        out: *mut RemovedRead,
    ) -> bool,
}

/// Result of [`ExtensionInit`].
/// Newtype rather than an `enum`, same reason as [`SystemStatus`]: the plugin
/// produces it and the host materialises it.
///
/// A variant was appended here under MINOR 13 on the argument that a plugin only
/// returns the new value after passing the version check, so a host too old to
/// know it is a host the plugin already refused. That argument holds for this
/// type — but it is exactly the kind of local reasoning that does not survive
/// the next edit, and it did not generalise to the two siblings.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct InitResult(pub i32);

#[allow(non_upper_case_globals)]
impl InitResult {
    pub const Ok: Self = Self(0);
    /// The host's [`Interface`] is older than the plugin needs.
    pub const VersionTooOld: Self = Self(1);
    /// The plugin's own setup failed. It will not be loaded.
    pub const Failed: Self = Self(2);
    /// The host's [`Interface`] has the right version but the wrong *shape* — a
    /// field was inserted, reordered or retyped somewhere in the range this
    /// plugin compiled against, so its calls would land in the wrong function.
    ///
    /// Distinct from [`Self::VersionTooOld`] because the fix is different: too
    /// old means update the engine, this means the two were built from headers
    /// that disagree, and rebuilding the plugin is what resolves it.
    pub const AbiMismatch: Self = Self(3);

    /// Whether this is a value this build knows. The host must check before
    /// deciding what a plugin's init meant — anything else is a failure it has
    /// no name for, never a success.
    pub const fn is_known(self) -> bool {
        self.0 >= 0 && self.0 < 4
    }
}

/// The signature of [`INIT_SYMBOL`], the plugin's sole export.
///
/// Called once at load. The plugin registers its components and systems through
/// `iface` and returns [`InitResult::Ok`]. Anything else and the host unloads it
/// without ever calling into it again.
pub type ExtensionInit =
    unsafe extern "C" fn(iface: *const Interface, host: *mut Host) -> InitResult;

// ── Scripting ────────────────────────────────────────────────────────────────
//
// The boundary half of `crate::script`. These live here rather than beside the
// codec for the same reason `PanelDesc` and `MeshDesc` do: anything named in
// [`Interface`] is part of the frozen mechanism, and the mechanism is this
// module. The vocabulary that rides on top — the command list, the contexts,
// the encoder — stays in `crate::script`, where it can grow without any of this
// moving. `crate::script` re-exports everything below so a plugin author only
// ever names one module.

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
