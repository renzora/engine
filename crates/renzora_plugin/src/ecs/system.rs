//! System parameters, and how a Rust function becomes a C entry point.
//!
//! `SystemParam::fetch` takes a **raw pointer**, not a reference, and that is
//! load-bearing rather than sloppy. The obvious signature — `fn fetch<'a>(&'a
//! SystemCall) -> Self::Item<'a>` — makes the parameter type an associated
//! type, and the compiler cannot run an associated type backwards: given `fn
//! spin(Query<..>, Res<Time>)` it has no way to work out which `SystemParam`
//! produced those items, so every `add_systems` call fails with "type
//! annotations needed". Returning `Self` makes the parameter type *the* type in
//! the signature, which unifies directly.
//!
//! [`catch`] is the entire cost of `no_std`: `catch_unwind` lives in `std` and
//! has no `core` equivalent, so a `no_std` plugin must also set
//! `panic = "abort"`.

use core::marker::PhantomData;

use alloc::vec;
use alloc::vec::Vec;

use crate::sys;

use super::app::{App, Plugin, Schedule};
use super::commands::Commands;
use super::component::{Component, Vec3};
use super::init::{component_id_of, InitCtx};
use super::query::{Query, QueryData, QueryFilter};
use super::resource::{Res, ResMut, ResourceParam};

/// One argument a system can take. Mirrors `bevy::SystemParam`.
///
/// Systems used to be enumerated by parameter shape — `Query`, `Query + Time`,
/// `Query + Commands`, and so on — which needs a new impl for every combination
/// and explodes the moment a resource joins in. This is the same fix Bevy uses:
/// describe one parameter, derive every combination from tuples.
///
/// `fetch` takes a **raw pointer**, not a reference, and that is load-bearing
/// rather than sloppy. The obvious signature — `fn fetch<'a>(&'a SystemCall) ->
/// Self::Item<'a>` — makes the parameter type an associated type, and the
/// compiler cannot run an associated type backwards: given `fn spin(Query<..>,
/// Res<Time>)` it has no way to work out which `SystemParam` produced those
/// items, so every `add_systems` call fails with "type annotations needed".
/// Returning `Self` makes the parameter type *the* type in the signature, which
/// unifies directly. The lifetime inside `Self` is then unchecked, which is why
/// this trait is `unsafe`: the call outlives the system body, so borrowing from
/// it is sound, but nothing in the types enforces that.
///
/// # Safety
/// `terms` must declare every component `fetch` reads, or the host will build a
/// query that does not match what the system touches. `fetch` must not retain
/// anything past the call.
pub unsafe trait SystemParam: Sized {
    /// Declare what this parameter needs.
    ///
    /// A `Query` pushes its own term list as a **separate** query; everything
    /// else pushes resource terms, which are per-system rather than per-query. A
    /// single merged list was what limited a system to one query: two `Query`
    /// parameters had nowhere to be separate, so their terms AND-ed together and
    /// both read the same cells.
    fn declare(ctx: &mut InitCtx, out: &mut SystemBuilder);

    /// # Safety
    /// `call` must be live for at least as long as the returned value is used.
    /// `views` is the running index of query parameters seen so far, and must be
    /// advanced by exactly the number this parameter declared — the host returns
    /// one view per declared query, in declaration order, and the two walks have
    /// to stay in step.
    unsafe fn fetch(call: *const sys::SystemCall, views: &mut usize) -> Self;
}

/// Collects what a system's parameters declare, before it is registered.
#[derive(Default)]
pub struct SystemBuilder {
    pub(crate) queries: alloc::vec::Vec<alloc::vec::Vec<sys::Term>>,
    pub(crate) resources: alloc::vec::Vec<sys::Term>,
}

unsafe impl<D: QueryData, F: QueryFilter> SystemParam for Query<'_, D, F> {
    fn declare(ctx: &mut InitCtx, out: &mut SystemBuilder) {
        let mut terms = alloc::vec::Vec::new();
        D::terms(ctx, &mut terms);
        F::terms(ctx, &mut terms);
        out.queries.push(terms);
    }
    unsafe fn fetch(call: *const sys::SystemCall, views: &mut usize) -> Self {
        let index = *views;
        *views += 1;
        Query::new(call, index)
    }
}

unsafe impl<T: ResourceParam> SystemParam for Res<'_, T> {
    fn declare(ctx: &mut InitCtx, out: &mut SystemBuilder) {
        T::res_term(ctx, &mut out.resources, sys::Access::ResRead);
    }
    unsafe fn fetch(call: *const sys::SystemCall, _: &mut usize) -> Self {
        Res(T::res_ptr(call), PhantomData)
    }
}

unsafe impl<T: ResourceParam> SystemParam for ResMut<'_, T> {
    fn declare(ctx: &mut InitCtx, out: &mut SystemBuilder) {
        T::res_term(ctx, &mut out.resources, sys::Access::ResWrite);
    }
    unsafe fn fetch(call: *const sys::SystemCall, _: &mut usize) -> Self {
        ResMut(T::res_ptr(call), PhantomData)
    }
}

/// `Option<Res<T>>` for a resource that may not exist.
///
/// Bevy declines to *run* a system whose `Res<T>` is missing. The host cannot do
/// that — a plugin system's parameters are resolved per call, not per schedule —
/// so a bare `Res<T>` whose resource is absent panics on first deref, which costs
/// the plugin its system for the session. `Option` is the way to ask without
/// risking that, and it is spelled exactly as it is in Bevy.
unsafe impl<T: ResourceParam> SystemParam for Option<Res<'_, T>> {
    fn declare(ctx: &mut InitCtx, out: &mut SystemBuilder) {
        // Declared exactly as the non-optional form: the host resolves the id
        // and reserves the slot either way, and absence is a null pointer at
        // fetch time rather than a different declaration.
        T::res_term(ctx, &mut out.resources, sys::Access::ResRead);
    }
    unsafe fn fetch(call: *const sys::SystemCall, n: &mut usize) -> Self {
        let ptr = T::res_ptr(call);
        (!ptr.is_null()).then(|| Res::<T>::fetch(call, n))
    }
}

/// `Option<ResMut<T>>`. See [`Option<Res<T>>`].
unsafe impl<T: ResourceParam> SystemParam for Option<ResMut<'_, T>> {
    fn declare(ctx: &mut InitCtx, out: &mut SystemBuilder) {
        T::res_term(ctx, &mut out.resources, sys::Access::ResWrite);
    }
    unsafe fn fetch(call: *const sys::SystemCall, n: &mut usize) -> Self {
        let ptr = T::res_ptr(call);
        (!ptr.is_null()).then(|| ResMut::<T>::fetch(call, n))
    }
}

/// Build the header + payload into one contiguous buffer and push it.
///
/// One buffer because the sink copies exactly one `data` pointer. The stack
/// fast path covers every per-frame caller — an animation or physics command is
/// well under 128 bytes — and the heap fallback exists for the ones that are
/// genuinely variable-length, like an HTTP body.
pub(crate) fn push_service(
    sink: *mut sys::CommandSink,
    entity: sys::Entity,
    service: u64,
    op: u32,
    payload: &[u8],
) {
    const INLINE: usize = 128;
    let header = sys::ServiceCall { service, op, _pad: 0 };
    let hdr_len = core::mem::size_of::<sys::ServiceCall>();
    let total = hdr_len + payload.len();

    let mut stack = [0u8; INLINE];
    let mut heap;
    let buf: &mut [u8] = if total <= INLINE {
        &mut stack[..total]
    } else {
        heap = vec![0u8; total];
        &mut heap[..]
    };
    unsafe {
        core::ptr::copy_nonoverlapping(
            (&header as *const sys::ServiceCall).cast::<u8>(),
            buf.as_mut_ptr(),
            hdr_len,
        );
    }
    buf[hdr_len..].copy_from_slice(payload);

    let cmd = sys::Command {
        kind: sys::CommandKind::Service,
        entity,
        component: sys::ComponentId::INVALID,
        data: buf.as_ptr(),
        data_len: total,
    };
    unsafe { ((*sink).push)(sink, &cmd) };
}

impl Commands<'_> {
    /// Call a host service that is not about any particular entity.
    ///
    /// The entity-scoped [`super::EntityCommands::call_service`] is the common
    /// case — animation and physics both act on a body. Some domains do not: an
    /// HTTP request belongs to the plugin, not to a thing in the world. The
    /// entity field still crosses, carrying [`sys::Entity::PLACEHOLDER`], so the
    /// consumer sees one shape either way.
    pub fn call_service(&mut self, service: u64, op: u32, payload: &[u8]) -> &mut Self {
        if !self.sink.is_null() {
            push_service(self.sink, sys::Entity::PLACEHOLDER, service, op, payload);
        }
        self
    }
}

unsafe impl SystemParam for Commands<'_> {
    fn declare(_: &mut InitCtx, _: &mut SystemBuilder) {}
    unsafe fn fetch(call: *const sys::SystemCall, _: &mut usize) -> Self {
        Commands {
            sink: (*call).commands,
            _p: PhantomData,
        }
    }
}

/// Geometry copied out of a host mesh.
///
/// Owned by the plugin — the host never hands back a pointer into its asset
/// storage, which can move or be freed the moment the call returns.
#[derive(Clone, Debug, Default)]
pub struct MeshData {
    pub positions: Vec<Vec3>,
    /// Empty if the mesh has none.
    pub normals: Vec<Vec3>,
    /// Empty if the mesh has none.
    pub uvs: Vec<[f32; 2]>,
    /// Empty for an unindexed mesh, where every three positions are one face.
    pub indices: Vec<u32>,
}

impl MeshData {
    /// Triangles as index triples, whether or not the mesh was indexed.
    ///
    /// Saves every caller writing the same branch — scattering points over a
    /// surface does not care how the mesh stored its faces.
    pub fn triangles(&self) -> Vec<[usize; 3]> {
        if self.indices.is_empty() {
            (0..self.positions.len() / 3)
                .map(|t| [t * 3, t * 3 + 1, t * 3 + 2])
                .collect()
        } else {
            self.indices
                .chunks_exact(3)
                .map(|c| [c[0] as usize, c[1] as usize, c[2] as usize])
                .collect()
        }
    }
}

/// Reads the geometry of meshes already in the world.
///
/// The counterpart to [`App::add_mesh_data`]: that emits geometry, this one
/// consumes what is already there — scattering over a surface, growing from a
/// scalp, fitting a decal to a wall.
///
/// ```ignore
/// fn scatter(q: Query<Entity, With<Scatter>>, meshes: Meshes) {
///     for e in &q {
///         let Some(mesh) = meshes.read(e) else { continue };  // still loading
///         for [a, b, c] in mesh.triangles() { /* … */ }
///     }
/// }
/// ```
pub struct Meshes<'a> {
    src: *mut sys::MeshSource,
    _p: PhantomData<&'a ()>,
}

impl Meshes<'_> {
    /// Copy the geometry of `entity`'s mesh.
    ///
    /// `None` if the entity has no mesh, or its asset has not loaded yet —
    /// which is the normal state for the first few frames after a spawn, so
    /// poll rather than treating it as failure.
    pub fn read(&self, entity: sys::Entity) -> Option<MeshData> {
        if self.src.is_null() {
            return None;
        }
        // Two passes: the first learns the sizes, the second fills buffers we
        // own. The host cannot allocate for us — it does not share our
        // allocator — so this is the only shape that works.
        let mut probe = sys::MeshRead::COUNTS_ONLY;
        unsafe {
            if !((*self.src).read)(self.src, entity, &mut probe) {
                return None;
            }
        }
        let mut out = MeshData {
            positions: vec![Vec3 { x: 0.0, y: 0.0, z: 0.0 }; probe.position_count],
            normals: vec![Vec3 { x: 0.0, y: 0.0, z: 0.0 }; probe.normal_count],
            uvs: vec![[0.0, 0.0]; probe.uv_count],
            indices: vec![0; probe.index_count],
        };
        let mut fill = sys::MeshRead {
            position_capacity: out.positions.len(),
            positions: out.positions.as_mut_ptr(),
            normal_capacity: out.normals.len(),
            normals: out.normals.as_mut_ptr(),
            uv_capacity: out.uvs.len(),
            uvs: out.uvs.as_mut_ptr(),
            index_capacity: out.indices.len(),
            indices: out.indices.as_mut_ptr(),
            ..sys::MeshRead::COUNTS_ONLY
        };
        unsafe {
            if !((*self.src).read)(self.src, entity, &mut fill) {
                return None;
            }
        }
        // The mesh can change between the two passes — an asset reload lands
        // between frames, not mid-call, but a shrink would still leave the tail
        // of our buffers holding the zeroes we allocated. Truncate to what the
        // second pass actually reported.
        out.positions.truncate(fill.position_count);
        out.normals.truncate(fill.normal_count);
        out.uvs.truncate(fill.uv_count);
        out.indices.truncate(fill.index_count);
        Some(out)
    }
}

impl Meshes<'_> {
    /// Replace the geometry of a mesh created with [`App::add_mesh_data`].
    ///
    /// `add_mesh_data` is init-only, so without this a plugin could generate
    /// geometry once and never again. Anything that rebuilds per frame — hair
    /// ribbons, a water surface, a trail — writes from a system instead.
    ///
    /// Validated exactly as `add_mesh_data` is, and refused on the same
    /// grounds. A refusal leaves the existing mesh alone rather than replacing
    /// it with something malformed, so a bad frame shows the previous geometry
    /// instead of nothing.
    ///
    /// `colors` are per-vertex linear RGBA — the built-in PBR material
    /// multiplies them into its base colour, which is how a groom gets
    /// per-strand shade variation without a custom shader.
    pub fn write(
        &self,
        handle: sys::AssetHandle,
        positions: &[Vec3],
        normals: Option<&[Vec3]>,
        uvs: Option<&[[f32; 2]]>,
        indices: Option<&[u32]>,
        colors: Option<&[[f32; 4]]>,
    ) -> bool {
        if self.src.is_null() {
            return false;
        }
        let desc = sys::MeshDataDesc {
            positions: positions.as_ptr(),
            position_count: positions.len(),
            normals: normals.map_or(core::ptr::null(), |n| n.as_ptr()),
            normal_count: normals.map_or(0, |n| n.len()),
            uvs: uvs.map_or(core::ptr::null(), |u| u.as_ptr()),
            uv_count: uvs.map_or(0, |u| u.len()),
            indices: indices.map_or(core::ptr::null(), |i| i.as_ptr()),
            index_count: indices.map_or(0, |i| i.len()),
        };
        let colors = sys::MeshColors {
            colors: colors.map_or(core::ptr::null(), |c| c.as_ptr()),
            color_count: colors.map_or(0, |c| c.len()),
        };
        unsafe { ((*self.src).write)(self.src, handle, &desc, &colors) }
    }
}

/// Replaces the pixels of images the plugin created.
///
/// The counterpart to [`App::add_image`], which is init-only. A simulation that
/// steps a heightfield every frame — the main reason a plugin wants a texture at
/// all — writes through this.
pub struct Images<'a> {
    src: *mut sys::ImageSource,
    _p: PhantomData<&'a ()>,
}

impl Images<'_> {
    /// Overwrite `handle`'s pixels.
    ///
    /// `data` must match the image's existing byte count exactly; dimensions and
    /// format are fixed at creation. A mismatch is refused and the previous
    /// pixels are left alone, so a bad frame shows the last good texture rather
    /// than garbage.
    pub fn write(&self, handle: sys::AssetHandle, data: &[u8]) -> bool {
        if self.src.is_null() {
            return false;
        }
        unsafe { ((*self.src).write)(self.src, handle, data.as_ptr(), data.len()) }
    }
}

/// Entities that lost `T` since this system last ran. Mirrors Bevy's
/// `RemovedComponents<T>`.
///
/// ```ignore
/// fn cleanup(mut removed: RemovedComponents<Hair>) {
///     for entity in removed.read() {
///         // tear down whatever this entity owned
///     }
/// }
/// ```
///
/// Before this existed, a plugin owning anything per-entity had to sweep: keep a
/// map of what it had built, walk it every frame, and diff it against the
/// entities its query still matched. Both shipped plugins did exactly that, and
/// the cost was O(tracked x live) per frame for information the engine already
/// had.
///
/// The cursor is per system, like Bevy's — two systems watching the same
/// component each see every removal, and a system that skips a frame still sees
/// that frame's removals when it next runs.
///
/// **A despawn counts as a removal**, which is the case the sweeps existed for.
pub struct RemovedComponents<'a, T: Component> {
    src: *mut sys::RemovedSource,
    _p: PhantomData<(&'a (), T)>,
}

impl<T: Component> RemovedComponents<'_, T> {
    /// Take the removals this system has not seen yet.
    ///
    /// Returns owned entities rather than an iterator borrowing the host: the
    /// bytes are copied out during the call, and holding a borrow across it would
    /// outlive the pointer the host lent us.
    pub fn read(&mut self) -> alloc::vec::Vec<sys::Entity> {
        if self.src.is_null() {
            return alloc::vec::Vec::new();
        }
        let component = component_id_of::<T>();
        // Two passes: learn the count, then fill a buffer we own. The host does
        // not share our allocator, so it cannot hand back a `Vec`.
        let mut probe = sys::RemovedRead::COUNTS_ONLY;
        unsafe {
            if !((*self.src).read)(self.src, component, &mut probe) {
                return alloc::vec::Vec::new();
            }
        }
        if probe.entity_count == 0 {
            return alloc::vec::Vec::new();
        }
        let mut out = alloc::vec![sys::Entity(u64::MAX); probe.entity_count];
        let mut fill = sys::RemovedRead {
            entity_capacity: out.len(),
            entities: out.as_mut_ptr(),
            entity_count: 0,
        };
        unsafe {
            if !((*self.src).read)(self.src, component, &mut fill) {
                return alloc::vec::Vec::new();
            }
        }
        // The second pass consumes, so its count is authoritative — and it can
        // legitimately differ from the probe's if a removal landed between them.
        out.truncate(fill.entity_count.min(out.len()));
        out
    }
}

unsafe impl<T: Component> SystemParam for RemovedComponents<'_, T> {
    // Declares nothing. Removal tracking is not component access — the host's
    // source reads a message buffer, not storage — so this can never conflict
    // with another system, which is also true of Bevy's own param.
    fn declare(ctx: &mut InitCtx, _: &mut SystemBuilder) {
        // Resolve the id, though: a plugin may only ever *remove* a component,
        // and then nothing else would have taught the host its name.
        let _ = ctx.id_of::<T>();
    }
    unsafe fn fetch(call: *const sys::SystemCall, _: &mut usize) -> Self {
        RemovedComponents {
            src: (*call).removed,
            _p: PhantomData,
        }
    }
}

unsafe impl SystemParam for Images<'_> {
    fn declare(_: &mut InitCtx, _: &mut SystemBuilder) {}
    unsafe fn fetch(call: *const sys::SystemCall, _: &mut usize) -> Self {
        Images {
            src: (*call).images,
            _p: PhantomData,
        }
    }
}

unsafe impl SystemParam for Meshes<'_> {
    fn declare(_: &mut InitCtx, _: &mut SystemBuilder) {}
    unsafe fn fetch(call: *const sys::SystemCall, _: &mut usize) -> Self {
        Meshes {
            src: (*call).meshes,
            _p: PhantomData,
        }
    }
}

/// Collects answers to [`Commands::call_service`] calls.
///
/// The generic return path. A domain sends with `call_service` and collects
/// here, keyed by the same service id plus whatever tag it chose — so a new
/// domain needs no boundary surface in either direction. See
/// [`sys::ReplySource`] for why this exists and what it replaced.
///
/// Most plugins will not name this type: a domain module wraps it in something
/// that speaks its own vocabulary, the way [`crate::dialog::Dialogs`] does.
pub struct Replies<'a> {
    src: *mut sys::ReplySource,
    _p: PhantomData<&'a ()>,
}

impl Replies<'_> {
    /// Take the next reply for `service` and `tag`, if one is ready.
    ///
    /// `None` is the normal state. A reply is delivered exactly once; the `u32`
    /// is the domain's own discriminator, and the bytes are whatever the
    /// consumer produced.
    pub fn poll(&self, service: u64, tag: u64) -> Option<(u32, alloc::vec::Vec<u8>)> {
        if self.src.is_null() {
            return None;
        }
        // Two passes, as everywhere else on this boundary: the probe must not
        // consume, or a caller that fails to allocate would silently drop the
        // reply and wait for it forever.
        let mut probe = sys::ReplyRead::COUNTS_ONLY;
        unsafe {
            if !((*self.src).poll)(self.src, service, tag, &mut probe) {
                return None;
            }
        }
        // `max(1)` so an empty reply is still CONSUMED — the host distinguishes
        // the two passes by "is there a buffer?", and a zero-length allocation
        // would leave the reply in the queue to be re-read every frame. A domain
        // that answers "cancelled" with no payload depends on this.
        let mut data = alloc::vec![0u8; probe.data_len.max(1)];
        let mut fill = sys::ReplyRead {
            data_capacity: data.len(),
            data: data.as_mut_ptr(),
            ..sys::ReplyRead::COUNTS_ONLY
        };
        unsafe {
            if !((*self.src).poll)(self.src, service, tag, &mut fill) {
                return None;
            }
        }
        data.truncate(fill.data_len);
        Some((fill.op, data))
    }
}

unsafe impl SystemParam for Replies<'_> {
    fn declare(_: &mut InitCtx, _: &mut SystemBuilder) {}
    unsafe fn fetch(call: *const sys::SystemCall, _: &mut usize) -> Self {
        Replies {
            src: (*call).replies,
            _p: PhantomData,
        }
    }
}

macro_rules! param_tuples {
    ($(($($p:ident),+))+) => {
        $(
            unsafe impl<$($p: SystemParam),+> SystemParam for ($($p,)+) {
                fn declare(ctx: &mut InitCtx, out: &mut SystemBuilder) {
                    $($p::declare(ctx, out);)+
                }
                unsafe fn fetch(call: *const sys::SystemCall, views: &mut usize) -> Self {
                    // Declaration order and fetch order are the same walk over
                    // the same tuple, which is what keeps view indices aligned.
                    ($($p::fetch(call, views),)+)
                }
            }
        )+
    };
}

param_tuples! {
    (A)
    (A, B)
    (A, B, C)
    (A, B, C, D)
    (A, B, C, D, E)
    (A, B, C, D, E, F)
    (A, B, C, D, E, F, G)
    (A, B, C, D, E, F, G, H)
    (A, B, C, D, E, F, G, H, I)
    (A, B, C, D, E, F, G, H, I, J)
    (A, B, C, D, E, F, G, H, I, J, K)
    (A, B, C, D, E, F, G, H, I, J, K, L)
    (A, B, C, D, E, F, G, H, I, J, K, L, M)
    (A, B, C, D, E, F, G, H, I, J, K, L, M, N)
    (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O)
    (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P)
}

/// A function that can be registered as a system.
///
/// Implemented for the *callable* rather than for `fn(..)` pointer types, which
/// matters for ergonomics: a bare `spin` is a fn **item**, a distinct
/// zero-sized type that only coerces to a pointer at a coercion site. Impl'ing
/// on the pointer would force every call to read `add_systems(Update, spin as
/// fn(_, _))`, and "identical to Bevy source" is the whole point.
///
/// The callable must be zero-sized — fn items and non-capturing closures
/// qualify, capturing closures do not. Enforced at compile time in
/// [`materialize`]; a capturing closure would need storage the host has no way
/// to own, so rejecting it is correct rather than a limitation to lift later.
pub trait IntoSystem<Marker> {
    fn build(self, ctx: &mut InitCtx) -> (SystemBuilder, sys::SystemEntry, *mut core::ffi::c_void);
}


/// One system, or a tuple of them, for [`App::add_systems`].
///
/// Exists so `add_systems(Update, (a, b))` compiles, which is how Bevy source
/// reads and therefore how a plugin's has to. The `Marker` shapes are what keep
/// the blanket single-system impl from overlapping the tuple ones: a lone system
/// is marked `(M,)` and an n-tuple is marked with an n-tuple, so no type can
/// satisfy two of these at once.
pub trait IntoSystems<Marker> {
    fn add_to(self, app: &mut App, schedule: Schedule);
}

impl<M, S: IntoSystem<M>> IntoSystems<(M,)> for S {
    fn add_to(self, app: &mut App, schedule: Schedule) {
        app.add_one_system(schedule, self);
    }
}

macro_rules! into_systems_tuples {
    ($(($($t:ident $m:ident),+))+) => {
        $(
            #[allow(non_snake_case)]
            impl<$($m,)+ $($t: IntoSystem<$m>,)+> IntoSystems<($($m,)+)> for ($($t,)+) {
                fn add_to(self, app: &mut App, schedule: Schedule) {
                    let ($($t,)+) = self;
                    $( app.add_one_system(schedule, $t); )+
                }
            }
        )+
    };
}

into_systems_tuples! {
    (A MA, B MB)
    (A MA, B MB, C MC)
    (A MA, B MB, C MC, D MD)
    (A MA, B MB, C MC, D MD, E ME)
    (A MA, B MB, C MC, D MD, E ME, F MF)
    (A MA, B MB, C MC, D MD, E ME, F MF, G MG)
    (A MA, B MB, C MC, D MD, E ME, F MF, G MG, H MH)
    (A MA, B MB, C MC, D MD, E ME, F MF, G MG, H MH, I MI)
    (A MA, B MB, C MC, D MD, E ME, F MF, G MG, H MH, I MI, J MJ)
    (A MA, B MB, C MC, D MD, E ME, F MF, G MG, H MH, I MI, J MJ, K MK)
    (A MA, B MB, C MC, D MD, E ME, F MF, G MG, H MH, I MI, J MJ, K MK, L ML)
}

/// Reconstruct a zero-sized callable from nothing.
///
/// Sound because a ZST has exactly one value and no bytes to be wrong about; the
/// const assertion is what keeps it that way if someone passes a capturing
/// closure.
#[inline(always)]
pub(crate) unsafe fn materialize<T>() -> T {
    const {
        assert!(
            core::mem::size_of::<T>() == 0,
            "a system must be a plain fn or a non-capturing closure — a capturing              closure has state the host cannot own",
        );
    }
    // Reading a zero-sized value through a dangling-but-aligned, non-null
    // pointer is the sanctioned way to materialise a ZST — it touches no memory,
    // because there is none to touch. `MaybeUninit::assume_init` would express
    // the same thing but is deny-by-default under clippy, which cannot see the
    // const assertion above that makes it sound.
    core::ptr::NonNull::<T>::dangling().as_ptr().read()
}

/// Marker distinguishing one parameter-tuple impl from another, so they don't
/// overlap.
pub struct ParamsMarker<P>(PhantomData<P>);

/// Run `body`, returning the panic message if it panicked.
///
/// **This function is the entire cost of `no_std`.** `catch_unwind` lives in
/// `std` and has no `core` equivalent — catching a panic needs the unwinder,
/// which is part of the standard library's runtime. So there are two versions:
/// under `std` a panic is caught and reported, and under `no_std` it cannot be,
/// which is why such a build must also set `panic = "abort"` (see the `std`
/// feature in this crate's manifest for the full trade).
///
/// Callers must therefore treat the `Err` arm as *may never happen* rather than
/// as dead code — it is live in every ordinary build.
#[cfg(feature = "std")]
pub(crate) fn catch(body: impl FnOnce()) -> Result<(), alloc::string::String> {
    // `AssertUnwindSafe` is required because these calls carry raw pointers;
    // that is sound here because a panic leaves host memory in whatever state
    // the partial call wrote — data the host owns and re-reads next frame, with
    // no plugin-side invariants to break.
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)).map_err(|e| {
        e.downcast_ref::<&str>()
            .map(alloc::string::ToString::to_string)
            .or_else(|| e.downcast_ref::<alloc::string::String>().cloned())
            .unwrap_or_else(|| alloc::string::String::from("panic"))
    })
}

/// `no_std`: nothing to catch with. A panic in `body` reaches the `extern "C"`
/// frame above and aborts the process. See the `std` variant.
#[cfg(not(feature = "std"))]
pub(crate) fn catch(body: impl FnOnce()) -> Result<(), alloc::string::String> {
    body();
    Ok(())
}

/// Run the plugin's `Plugin::build` under [`catch`], so a panic during load is a
/// refused plugin rather than a dead editor. `true` if it built.
///
/// Called by `add!`'s expansion. It lives here, as a function, because whether
/// the panic is catchable depends on THIS crate's `std` feature, and a `#[cfg]`
/// written inside a macro body would be evaluated against the plugin's manifest
/// instead — where `std` is not a feature that exists.
///
/// The panic message is dropped rather than logged: `build` runs before the App
/// has been handed back to the host, so there is no established channel to
/// report on, and the host already logs the refusal it gets back.
#[must_use]
pub fn guarded_build<P: Plugin>(plugin: &P, app: &mut App) -> bool {
    catch(|| plugin.build(app)).is_ok()
}

/// Run a system body, converting a panic into a status the host can act on.
///
/// A panic unwinding out of an `extern "C"` function aborts the process, so
/// without this one bad index in a half-written system takes the editor down.
unsafe fn guard(call: &sys::SystemCall, body: impl FnOnce()) -> sys::SystemStatus {
    match catch(body) {
        Ok(()) => sys::SystemStatus::Ok,
        Err(msg) => {
            if !call.iface.is_null() {
                ((*call.iface).log)(
                    call.host,
                    sys::LogLevel::Error,
                    sys::StrRef {
                        ptr: msg.as_ptr(),
                        len: msg.len(),
                    },
                );
            }
            sys::SystemStatus::Panicked
        }
    }
}

/// One `IntoSystem` impl per arity. The thunk lives inside the expansion because
/// calling the function needs the parameters spread, not tupled — there is no
/// variadic form to write it once.
///
/// The bound is `Fn($($p),+)` over the *parameter* types rather than over fetched
/// item types, so inference runs the right way: the compiler matches `spin`'s
/// signature against it and reads `P = Query<..>, Res<Time>` straight off, then
/// looks up each one's `SystemParam` impl.
macro_rules! into_system {
    ($(($($p:ident),+))+) => {
        $(
            impl<$($p,)+ Func> IntoSystem<ParamsMarker<($($p,)+)>> for Func
            where
                $($p: SystemParam,)+
                Func: Fn($($p),+) + 'static,
            {
                fn build(
                    self,
                    ctx: &mut InitCtx,
                ) -> (SystemBuilder, sys::SystemEntry, *mut core::ffi::c_void) {
                    unsafe extern "C" fn thunk<$($p,)+ Fun>(
                        call: *const sys::SystemCall,
                    ) -> sys::SystemStatus
                    where
                        $($p: SystemParam,)+
                        Fun: Fn($($p),+) + 'static,
                    {
                        // Rust does not order argument evaluation across a call,
                        // so the view counter is advanced in a `let` chain first
                        // and the results handed over already fetched. Left to
                        // the call site, two `Query` params could take each
                        // other's view.
                        let mut views = 0usize;
                        $(#[allow(non_snake_case)] let $p = $p::fetch(call, &mut views);)+
                        guard(&*call, move || materialize::<Fun>()($($p),+))
                    }

                    let mut builder = SystemBuilder::default();
                    $($p::declare(ctx, &mut builder);)+
                    (builder, thunk::<$($p,)+ Func>, core::ptr::null_mut())
                }
            }
        )+
    };
}

into_system! {
    (A)
    (A, B)
    (A, B, C)
    (A, B, C, D)
    (A, B, C, D, E)
    (A, B, C, D, E, F)
    (A, B, C, D, E, F, G)
    (A, B, C, D, E, F, G, H)
    (A, B, C, D, E, F, G, H, I)
    (A, B, C, D, E, F, G, H, I, J)
    (A, B, C, D, E, F, G, H, I, J, K)
    (A, B, C, D, E, F, G, H, I, J, K, L)
    (A, B, C, D, E, F, G, H, I, J, K, L, M)
    (A, B, C, D, E, F, G, H, I, J, K, L, M, N)
    (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O)
    (A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P)
}
