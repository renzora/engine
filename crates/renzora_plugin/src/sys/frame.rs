//! One invocation of a plugin system, and the per-call "sources" that hand data
//! back across the boundary.
//!
//! A source exists rather than an [`Interface`] function whenever answering the
//! question needs the world: [`SystemCall::host`] is null while a system runs, so
//! anything that would otherwise take a `Host` handle is created for the call and
//! dead when it returns.

use core::ffi::c_void;

use super::{
    AssetHandle, CommandSink, ComponentId, Entity, Host, ImageSource, InputState, Interface,
    MeshDataDesc, QueryView, RemovedSource, StrRef, Vec3,
};

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
///
/// [`QueryDesc`]: super::QueryDesc
/// [`Access::With`]: super::Access::With
/// [`Access::Without`]: super::Access::Without
#[repr(C)]
pub struct SystemCall {
    /// One entry per [`Query`](super::QueryDesc) the system declared, in
    /// declaration order.
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
    ///
    /// [`CommandKind::Service`]: super::CommandKind::Service
    pub replies: *mut ReplySource,

    // ── Added in MINOR 4.8 ────────────────────────────────────────────────
    // NOTHING MAY BE INSERTED ABOVE THIS POINT.
    /// This frame's measurements — frame time, FPS, entity count, per-render-pass
    /// GPU and CPU times, system CPU and memory. Null if the host keeps no
    /// diagnostics (a shipped game usually does not).
    ///
    /// A source rather than an [`Interface`] function, for the reason given on
    /// [`host`](Self::host): reading the store needs the world, and the `Host`
    /// handle is null while a system runs.
    ///
    /// A source rather than a [`ReplySource`] domain despite that being the
    /// designated home for new host-to-plugin data — see the MINOR 8 note on
    /// [`VERSION_MINOR`](super::VERSION_MINOR). Replies arrive a frame after the
    /// call that asked for them, and a profiler reading one-frame-stale numbers
    /// is measuring the wrong frame.
    pub diagnostics: *mut DiagnosticSource,
}

/// One measurement, borrowed for the duration of the call that produced it.
///
/// The host writes these into a buffer the *plugin* owns, which is what keeps
/// the two allocators apart — see [`DiagnosticSource::read`].
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DiagnosticEntry {
    /// The measurement's path, e.g. `"fps"` or `"render/main_opaque_pass_3d/elapsed_gpu"`.
    ///
    /// Borrowed from the host's store and valid **only until `read` returns** —
    /// copy it if you need to keep it. This is the ordinary [`StrRef`] contract
    /// and it bites harder here than elsewhere, because the obvious use is to
    /// cache the path as a key and the obvious bug is to cache the pointer.
    pub path: StrRef,
    /// The most recent sample. `f64::NAN` if the measurement exists but has not
    /// been taken yet, which is the normal state for the first frames.
    pub value: f64,
    /// The measurement's own smoothed average, or the same as `value` for a
    /// diagnostic that does not keep a history.
    pub smoothed: f64,
}

/// Reads the host's diagnostic store. Created for the call, dead when it returns.
#[repr(C)]
pub struct DiagnosticSource {
    /// Copy up to `cap` entries into `out`, returning **how many the host has** —
    /// which may exceed `cap`, so a caller that cares about completeness compares
    /// the two and grows.
    ///
    /// `out` may be null when `cap` is 0, which is how you ask for the count
    /// without allocating. The set is small (a few dozen) and stable after the
    /// first frames, so one probe at startup and a reused buffer is the intended
    /// shape rather than a probe every frame.
    ///
    /// Entry order is unspecified and not stable between calls. Diagnostics are
    /// identified by path, and a plugin that indexes them positionally will read
    /// FPS as GPU time the first time the host registers a new one.
    pub read: unsafe extern "C" fn(
        src: *mut DiagnosticSource,
        out: *mut DiagnosticEntry,
        cap: u32,
    ) -> u32,
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
///
/// [`CommandKind::Service`]: super::CommandKind::Service
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
