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
//! # Editing anything in this module
//!
//! Which rule applies depends on **how the type crosses**, not on how small the
//! edit looks. Get it backwards and nothing tells you: both sides compile their
//! own copy of these files from independent source trees, so there is no link
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
//! part of the module from the struct it breaks: every golden list stays
//! identical, `size_of::<Interface>()` stays identical, and both sides compile.
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
//!
//! # How this module is laid out
//!
//! One file per group, and the split is navigational only — every type is
//! re-exported flat, so `sys::Entity` resolves wherever it lives. `mod.rs` keeps
//! the version handshake, [`PluginScope`], and [`Interface`] itself, because the
//! table's field order is the contract and it should be readable in one place.
//!
//! `tests/abi_order.rs` walks this **directory** rather than naming files, so a
//! type in any of them is pinned exactly as one here is; see the note at the top
//! of that test.

mod assets;
mod audio;
mod commands;
mod frame;
mod input;
mod net;
mod panel;
mod primitives;
mod registration;
mod render;
mod script;

pub use assets::*;
pub use audio::*;
pub use commands::*;
pub use frame::*;
pub use input::*;
pub use net::*;
pub use panel::*;
pub use primitives::*;
pub use registration::*;
pub use render::*;
pub use script::*;

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
/// 6 -> 7 appended `add_settings_section`, so a plugin can put its configuration
///          on the Settings overlay rather than only in its own panel.
/// 5 -> 6 appended `PanelAction::text`, so a panel's text inputs can reach the
///          plugin at all — before it, `value: f32` was the only channel.
/// 4 -> 5 appended `SystemCall::replies`, [`ReplySource`] and [`ReplyRead`] —
///          a generic host-to-plugin answer channel, so a domain needing a
///          reply no longer costs a bump of its own. Intended to be the LAST
///          per-domain source ever added.
/// 7 -> 8 appended `SystemCall::diagnostics`, [`DiagnosticSource`] and
///          [`DiagnosticEntry`] — the host's measurement store, readable from a
///          plugin system. It breaks the "last per-domain source" intent one
///          line above, and the reason it is not a [`ReplySource`] domain is
///          worth recording: replies are *answers to a call the plugin made*,
///          delivered a frame later. Diagnostics are the opposite shape — a
///          plugin wants this frame's numbers during this frame, and a
///          request/response round trip would hand every reader values one
///          frame stale. A profiler that plots last frame's frame time against
///          this frame's marker is not a profiler.
/// 8 -> 9 appended `add_audio_backend`, [`AudioBackendDesc`], [`AudioCall`],
///          [`AudioOp`] and [`AudioStatus`] — the audio engine as a plugin,
///          which is the "some day" the note at the bottom of this block
///          anticipated. It is boundary surface rather than a `crate::audio`
///          domain for the reason `add_script_backend` is: the host calls into
///          the backend and needs an answer, which a command queue cannot say.
/// 9 -> 10 appended `add_net_backend`, [`NetBackendDesc`], [`NetCall`],
///          [`NetOp`] and [`NetStatus`] — the HTTP client as a plugin, on the
///          same reasoning as the line above. It is the one that finally takes
///          rustls, ring and the whole TLS stack out of the engine's dependency
///          graph: nothing in the binary makes a network request any more, it
///          asks whoever registered here. See [`crate::net`], and `sys/net.rs`
///          for why this is not the same thing as [`crate::http`].
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
pub const VERSION_MINOR: u32 = 10;

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

    // ── Added in MINOR 4.7 ────────────────────────────────────────────────
    // NOTHING MAY BE INSERTED ABOVE THIS POINT.
    /// Register a section on the Settings overlay's **Plugins** tab.
    ///
    /// Takes a [`PanelDesc`] rather than a type of its own, because a settings
    /// section *is* a panel — the same id, title, icon, markup and action thunk
    /// — that renders inside Settings instead of in the dock. Giving it a
    /// second, near-identical struct would mean two shapes to keep in step and
    /// two paths for `set_panel_content` to update, and the two would drift.
    ///
    /// A new table entry rather than a `category: "Settings"` convention on
    /// [`add_panel`]: a magic string decides behaviour invisibly, and a
    /// settings section genuinely is not a dock panel — it has no tab and no
    /// layout entry. `category` still groups sections in the sidebar.
    add_settings_section:
        unsafe extern "C" fn(host: *mut Host, desc: *const PanelDesc) -> RegisterStatus,

    // ── Added in MINOR 4.9 ────────────────────────────────────────────────
    // NOTHING MAY BE INSERTED ABOVE THIS POINT. A new function goes here, at
    // the very end, under a new header. See `boundary_layouts_are_pinned` in
    // `tests/abi_order.rs`.
    /// Register the audio backend. See [`AudioBackendDesc`].
    ///
    /// Here rather than riding on [`CommandKind::Service`] for the same reason
    /// [`Self::add_script_backend`] is: the direction of the call. Every domain
    /// on the service command is a plugin asking the engine to do something,
    /// which a queued opaque payload expresses perfectly. Audio is the reverse —
    /// the engine asks the backend for the meters, for a clip's duration, for
    /// the samples a microphone produced — and there is no way to express "call
    /// me and answer" with a command queue.
    ///
    /// One backend at a time, unlike scripting: two languages coexist because a
    /// script names one by its file extension, but there is only one pair of
    /// speakers. The host keeps the first registration and logs the second.
    add_audio_backend:
        unsafe extern "C" fn(host: *mut Host, desc: *const AudioBackendDesc) -> RegisterStatus,

    // ── Added in MINOR 4.10 ───────────────────────────────────────────────
    // NOTHING MAY BE INSERTED ABOVE THIS POINT. A new function goes here, at
    // the very end, under a new header. See `boundary_layouts_are_pinned` in
    // `tests/abi_order.rs`.
    /// Register the network backend. See [`NetBackendDesc`].
    ///
    /// The third entry here with the same justification as
    /// [`Self::add_script_backend`] and [`Self::add_audio_backend`]: the host
    /// calls into the backend and needs the answer back. Every domain riding
    /// [`CommandKind::Service`] is a plugin asking the engine for something,
    /// which a queued opaque payload expresses perfectly; "fetch this URL and
    /// hand me the bytes" is the engine asking the plugin, and a command queue
    /// has no way to say it.
    ///
    /// One backend at a time, like audio and unlike scripting. Two languages
    /// coexist because a script names one by its file extension; two HTTP
    /// clients have no such key, and picking one arbitrarily per request would
    /// mean a session's cookies and connection pool split across two of them.
    add_net_backend:
        unsafe extern "C" fn(host: *mut Host, desc: *const NetBackendDesc) -> RegisterStatus,
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
