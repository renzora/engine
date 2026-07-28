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

use core::ffi::c_void;

/// Bumped for any change that is not purely additive — a signature change, a
/// reordered or removed field, or a field whose meaning changed. Every existing
/// plugin is refused when this moves, which is the point.
///
/// Went 0 -> 1 when `SystemEntry` gained a return value; see the module docs for
/// why that had to be MAJOR.
pub const VERSION_MAJOR: u32 = 1;

/// Bumped when something is *appended*. Older plugins keep working; a plugin
/// needing the new function declares this as its minimum.
pub const VERSION_MINOR: u32 = 2;

/// The single symbol a plugin cdylib must export. See [`ExtensionInit`].
pub const INIT_SYMBOL: &str = "renzora_plugin_init";

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
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Schedule {
    First = 0,
    PreUpdate = 1,
    Update = 2,
    PostUpdate = 3,
    Last = 4,
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
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FieldKind {
    F32 = 0,
    I32 = 1,
    Bool = 2,
    Vec3 = 3,
    Quat = 4,
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
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Access {
    /// Read-only data access. Produces a cell; never written back.
    Read = 0,
    /// Mutable data access. Produces a cell, and the host copies it back after
    /// the call.
    Write = 1,
    /// Filter only — the entity must have this component. No cell is produced,
    /// so the plugin must not count it when indexing [`SystemCall::cells`].
    With = 2,
    /// Filter only — the entity must NOT have this component.
    Without = 3,
}

impl Access {
    /// Whether this term contributes a cell to [`SystemCall::cells`]. Filter
    /// terms do not, which is why cell indices are *not* term indices.
    pub const fn has_cell(self) -> bool {
        matches!(self, Access::Read | Access::Write)
    }
}

/// One component a system's query touches.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Term {
    pub component: ComponentId,
    pub access: Access,
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
    pub cells: *mut *mut u8,
    pub entities: *const Entity,
    pub entity_count: usize,
    /// Number of cells per row — the count of [`Access::has_cell`] terms, which
    /// is NOT the same as the number of terms once filters are involved.
    pub cell_count: usize,
    pub frame: FrameCtx,
    /// Opaque value the plugin supplied at [`Interface::add_system`] — how a
    /// generated thunk finds its way back to the right Rust function.
    pub user: *mut c_void,
    /// The interface, so a running system can call back into the host — to log,
    /// and later to spawn or queue commands. Appended after `user`, so a plugin
    /// built before this existed simply never reads it.
    pub iface: *const Interface,
    pub host: *mut Host,
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
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RenderPhase {
    /// HDR, after the main 3D pass, before temporal AA — GI, reflections.
    Gi = 0,
    /// HDR, after temporal AA — bloom, depth of field, motion blur.
    HdrPost = 1,
    /// LDR, after tonemapping — colour grading, vignette.
    LdrPost = 2,
    /// Final overlays, after AA.
    Overlay = 3,
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

/// Severity for [`Interface::log`].
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
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
    pub add_system: unsafe extern "C" fn(
        host: *mut Host,
        schedule: Schedule,
        entry: SystemEntry,
        query: *const QueryDesc,
        user: *mut c_void,
    ),

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
}

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
