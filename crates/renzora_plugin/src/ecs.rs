//! The ergonomic layer: Bevy-shaped types over the raw `sys` calls.
//!
//! Everything here exists to make plugin source **identical to Bevy source**.
//! Names and signatures mirror `bevy_ecs` deliberately — a "better" name is a
//! name nobody already knows, and the whole value of this crate is that existing
//! Bevy knowledge transfers without a translation step.
//!
//! ## How a system becomes a C function
//!
//! `App::add_systems` takes any zero-sized callable. Two things are derived from
//! its signature at compile time:
//!
//! * the [`sys::QueryDesc`] — which components, and how they're accessed
//! * an `extern "C"` thunk, monomorphised per signature, that unpacks
//!   [`sys::SystemCall`] into typed arguments and calls the function
//!
//! Because the thunk is generic over the callable's *type*, the function needs
//! no runtime representation at all — see `materialize`. That is why a capturing
//! closure is rejected: its captures would need storage the host cannot own.

use crate::sys;
use core::marker::PhantomData;

// ── Component identity ───────────────────────────────────────────────────────

/// A type that can appear in a plugin query.
///
/// Implemented for two quite different things, which is why [`descriptor`] is an
/// `Option`:
///
/// * **Host components** ([`Transform`] and friends) — the engine already owns
///   these. `descriptor()` is `None` and the id is looked up by type path.
/// * **Plugin components** — the engine has never heard of them. `descriptor()`
///   returns a layout and the host allocates storage for it.
///
/// The plugin's `Transform` *is* the `#[repr(C)]` mirror the host marshals into,
/// so both kinds are fetched from a cell the same way and this trait needs no
/// further distinction.
///
/// [`descriptor`]: Component::descriptor
pub trait Component: Sized + 'static {
    /// Fully-qualified type path. This is the shared identity between host and
    /// plugin — neither side has a `TypeId` the other could use.
    const TYPE_PATH: &'static str;

    /// `None` for host components; `Some` for plugin-owned ones.
    fn descriptor() -> Option<sys::ComponentDesc> {
        None
    }

    /// Name shown in the editor's "Add Component" list. Defaults to
    /// [`TYPE_PATH`](Component::TYPE_PATH)'s last segment.
    fn display_name() -> &'static str {
        ""
    }

    /// Inspectable fields, so the editor can show and edit this component.
    ///
    /// Empty is valid and means "addable but with nothing to edit" — correct for
    /// a marker. A plugin component with no schema at all would be storable but
    /// unreachable from the UI, which is the state this whole mechanism exists
    /// to fix.
    fn fields() -> &'static [sys::FieldDesc] {
        &[]
    }
}

/// Declares a host component the engine already owns.
///
/// The path must match the engine's registered type path exactly — it is the
/// only thing tying the two sides together.
macro_rules! host_component {
    ($ty:ty, $path:literal) => {
        impl Component for $ty {
            const TYPE_PATH: &'static str = $path;
        }
    };
}

host_component!(Transform, "bevy_transform::components::transform::Transform");
host_component!(Mesh3d, "bevy_mesh::components::Mesh3d");
host_component!(Visibility, "bevy_camera::visibility::Visibility");

/// Marker for the host's `Mesh3d`. Opaque: the handle inside it is not a layout
/// a plugin may depend on, so this is filter-only — usable in [`With`] but never
/// as data.
pub struct Mesh3d(());

/// Marker for the host's `Visibility`. Filter-only for the same reason.
pub struct Visibility(());

pub use crate::sys::{Quat, Transform, Vec3};

impl Quat {
    pub const IDENTITY: Quat = Quat { x: 0.0, y: 0.0, z: 0.0, w: 1.0 };

    pub fn from_rotation_y(angle: f32) -> Quat {
        let (s, c) = (angle * 0.5).sin_cos();
        Quat { x: 0.0, y: s, z: 0.0, w: c }
    }
}

impl core::ops::Mul for Quat {
    type Output = Quat;
    fn mul(self, r: Quat) -> Quat {
        Quat {
            x: self.w * r.x + self.x * r.w + self.y * r.z - self.z * r.y,
            y: self.w * r.y - self.x * r.z + self.y * r.w + self.z * r.x,
            z: self.w * r.z + self.x * r.y - self.y * r.x + self.z * r.w,
            w: self.w * r.w - self.x * r.x - self.y * r.y - self.z * r.z,
        }
    }
}

impl Transform {
    pub fn rotate_y(&mut self, angle: f32) {
        self.rotation = self.rotation * Quat::from_rotation_y(angle);
    }
}

// ── Init context ─────────────────────────────────────────────────────────────

/// Resolves component ids during `Plugin::build`, caching so a type is
/// registered or looked up once regardless of how many systems name it.
pub struct InitCtx {
    iface: *const sys::Interface,
    host: *mut sys::Host,
    cache: alloc::vec::Vec<(&'static str, sys::ComponentId)>,
    /// Type path of the first component that failed to resolve, if any.
    ///
    /// A plugin naming a component the host does not expose must refuse to load.
    /// Carrying on would register a system whose query can never match, which
    /// presents as "my plugin loaded but does nothing" — far harder to diagnose
    /// than a refusal at startup.
    unresolved: Option<&'static str>,
}

impl InitCtx {
    fn id_of<T: Component>(&mut self) -> sys::ComponentId {
        if let Some((_, id)) = self.cache.iter().find(|(p, _)| *p == T::TYPE_PATH) {
            return *id;
        }
        // SAFETY: `iface`/`host` are valid for the whole init call.
        let id = unsafe {
            match T::descriptor() {
                Some(desc) => ((*self.iface).register_component)(self.host, &desc),
                None => ((*self.iface).component_id_by_name)(
                    self.host,
                    sys::StrRef::new(T::TYPE_PATH),
                ),
            }
        };
        if !id.is_valid() && self.unresolved.is_none() {
            self.unresolved = Some(T::TYPE_PATH);
        }
        self.cache.push((T::TYPE_PATH, id));
        id
    }
}

// ── Query data ───────────────────────────────────────────────────────────────

/// What a query reads or writes. Implemented for `&T`, `&mut T`, and tuples.
///
/// # Safety
/// `CELLS` must equal the number of terms `terms` pushes, and `fetch` must read
/// exactly that many cells. The host lays cells out row-major on that promise.
pub unsafe trait QueryData {
    type Item<'a>;
    /// Cells this contributes per row.
    const CELLS: usize;
    fn terms(ctx: &mut InitCtx, out: &mut alloc::vec::Vec<sys::Term>);
    /// # Safety
    /// `cells` points at this item's first cell in a valid row.
    unsafe fn fetch<'a>(cells: *mut *mut u8) -> Self::Item<'a>;
}

unsafe impl<T: Component> QueryData for &T {
    type Item<'a> = &'a T;
    const CELLS: usize = 1;
    fn terms(ctx: &mut InitCtx, out: &mut alloc::vec::Vec<sys::Term>) {
        out.push(sys::Term { component: ctx.id_of::<T>(), access: sys::Access::Read });
    }
    unsafe fn fetch<'a>(cells: *mut *mut u8) -> &'a T {
        &*(*cells as *const T)
    }
}

unsafe impl<T: Component> QueryData for &mut T {
    type Item<'a> = &'a mut T;
    const CELLS: usize = 1;
    fn terms(ctx: &mut InitCtx, out: &mut alloc::vec::Vec<sys::Term>) {
        out.push(sys::Term { component: ctx.id_of::<T>(), access: sys::Access::Write });
    }
    unsafe fn fetch<'a>(cells: *mut *mut u8) -> &'a mut T {
        &mut *(*cells as *mut T)
    }
}

unsafe impl<A: QueryData, B: QueryData> QueryData for (A, B) {
    type Item<'a> = (A::Item<'a>, B::Item<'a>);
    const CELLS: usize = A::CELLS + B::CELLS;
    fn terms(ctx: &mut InitCtx, out: &mut alloc::vec::Vec<sys::Term>) {
        A::terms(ctx, out);
        B::terms(ctx, out);
    }
    unsafe fn fetch<'a>(cells: *mut *mut u8) -> Self::Item<'a> {
        (A::fetch(cells), B::fetch(cells.add(A::CELLS)))
    }
}

// ── Query filters ────────────────────────────────────────────────────────────

/// Filter terms. Contribute no cells — see [`sys::Access::With`].
pub trait QueryFilter {
    fn terms(ctx: &mut InitCtx, out: &mut alloc::vec::Vec<sys::Term>);
}

impl QueryFilter for () {
    fn terms(_: &mut InitCtx, _: &mut alloc::vec::Vec<sys::Term>) {}
}

/// Matches entities that have `T`.
pub struct With<T>(PhantomData<T>);

impl<T: Component> QueryFilter for With<T> {
    fn terms(ctx: &mut InitCtx, out: &mut alloc::vec::Vec<sys::Term>) {
        out.push(sys::Term { component: ctx.id_of::<T>(), access: sys::Access::With });
    }
}

/// Matches entities that do not have `T`.
pub struct Without<T>(PhantomData<T>);

impl<T: Component> QueryFilter for Without<T> {
    fn terms(ctx: &mut InitCtx, out: &mut alloc::vec::Vec<sys::Term>) {
        out.push(sys::Term { component: ctx.id_of::<T>(), access: sys::Access::Without });
    }
}

impl<A: QueryFilter, B: QueryFilter> QueryFilter for (A, B) {
    fn terms(ctx: &mut InitCtx, out: &mut alloc::vec::Vec<sys::Term>) {
        A::terms(ctx, out);
        B::terms(ctx, out);
    }
}

// ── Query ────────────────────────────────────────────────────────────────────

/// A view over the entities this system matched. Mirrors `bevy_ecs::Query`.
pub struct Query<'a, D: QueryData, F: QueryFilter = ()> {
    call: &'a sys::SystemCall,
    _p: PhantomData<(D, F)>,
}

impl<'a, D: QueryData, F: QueryFilter> Query<'a, D, F> {
    fn new(call: &'a sys::SystemCall) -> Self {
        Self { call, _p: PhantomData }
    }

    pub fn iter(&self) -> QueryIter<'_, D, F> {
        QueryIter { call: self.call, row: 0, _p: PhantomData }
    }

    pub fn iter_mut(&mut self) -> QueryIter<'_, D, F> {
        self.iter()
    }

    pub fn len(&self) -> usize {
        self.call.entity_count
    }

    pub fn is_empty(&self) -> bool {
        self.call.entity_count == 0
    }
}

pub struct QueryIter<'a, D: QueryData, F: QueryFilter> {
    call: &'a sys::SystemCall,
    row: usize,
    _p: PhantomData<(D, F)>,
}

impl<'a, D: QueryData, F: QueryFilter> Iterator for QueryIter<'a, D, F> {
    type Item = D::Item<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.row >= self.call.entity_count {
            return None;
        }
        // SAFETY: row < entity_count, and the host laid out `cell_count` cells
        // per row in declaration order.
        let item = unsafe { D::fetch(self.call.cells.add(self.row * self.call.cell_count)) };
        self.row += 1;
        Some(item)
    }
}

impl<'a, D: QueryData, F: QueryFilter> IntoIterator for &'a Query<'_, D, F> {
    type Item = D::Item<'a>;
    type IntoIter = QueryIter<'a, D, F>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, D: QueryData, F: QueryFilter> IntoIterator for &'a mut Query<'_, D, F> {
    type Item = D::Item<'a>;
    type IntoIter = QueryIter<'a, D, F>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

// ── Resources ────────────────────────────────────────────────────────────────

/// The frame clock. Mirrors `bevy::Time` for the parts a plugin can reach.
pub struct Time(sys::FrameCtx);

impl Time {
    pub fn delta_secs(&self) -> f32 {
        self.0.delta_secs
    }
    pub fn elapsed_secs(&self) -> f32 {
        self.0.elapsed_secs
    }
}

/// Shared access to a resource. Only [`Time`] is available so far; the general
/// resource path needs interface functions that don't exist yet.
pub struct Res<'a, T>(T, PhantomData<&'a ()>);

impl<T> core::ops::Deref for Res<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

// ── Systems ──────────────────────────────────────────────────────────────────

/// A function that can be registered as a system.
///
/// Implemented for the *callable* rather than for `fn(..)` pointer types, which
/// matters for ergonomics: a bare `spin` is a fn **item**, a distinct
/// zero-sized type that only coerces to a pointer at a coercion site. Impl'ing
/// on the pointer would force every call to read `add_systems(Update, spin as
/// fn(_, _))`, and "identical to Bevy source" is the whole point.
///
/// The callable must therefore be zero-sized — fn items and non-capturing
/// closures qualify, capturing closures do not. That is enforced at compile time
/// in [`materialize`]; a capturing closure would need storage the host has no way
/// to own, so rejecting it is correct rather than a limitation to lift later.
pub trait IntoSystem<Marker> {
    fn build(
        self,
        ctx: &mut InitCtx,
    ) -> (alloc::vec::Vec<sys::Term>, sys::SystemEntry, *mut core::ffi::c_void);
}

/// Reconstruct a zero-sized callable from nothing.
///
/// Sound because a ZST has exactly one value and no bytes to be wrong about; the
/// const assertion is what keeps it that way if someone passes a capturing
/// closure.
#[inline(always)]
unsafe fn materialize<T>() -> T {
    const {
        assert!(
            core::mem::size_of::<T>() == 0,
            "a system must be a plain fn or a non-capturing closure — a capturing              closure has state the host cannot own",
        );
    }
    core::mem::MaybeUninit::<T>::uninit().assume_init()
}

/// Marker so the one- and two-parameter impls don't overlap.
pub struct QueryOnly<D, F>(PhantomData<(D, F)>);
pub struct QueryTime<D, F>(PhantomData<(D, F)>);

impl<D, Fl, Func> IntoSystem<QueryOnly<D, Fl>> for Func
where
    D: QueryData,
    Fl: QueryFilter,
    Func: for<'a> Fn(Query<'a, D, Fl>) + 'static,
{
    fn build(
        self,
        ctx: &mut InitCtx,
    ) -> (alloc::vec::Vec<sys::Term>, sys::SystemEntry, *mut core::ffi::c_void) {
        let mut terms = alloc::vec::Vec::new();
        D::terms(ctx, &mut terms);
        Fl::terms(ctx, &mut terms);
        (terms, thunk_q::<D, Fl, Func>, core::ptr::null_mut())
    }
}

impl<D, Fl, Func> IntoSystem<QueryTime<D, Fl>> for Func
where
    D: QueryData,
    Fl: QueryFilter,
    Func: for<'a> Fn(Query<'a, D, Fl>, Res<'a, Time>) + 'static,
{
    fn build(
        self,
        ctx: &mut InitCtx,
    ) -> (alloc::vec::Vec<sys::Term>, sys::SystemEntry, *mut core::ffi::c_void) {
        let mut terms = alloc::vec::Vec::new();
        D::terms(ctx, &mut terms);
        Fl::terms(ctx, &mut terms);
        (terms, thunk_qt::<D, Fl, Func>, core::ptr::null_mut())
    }
}

/// Run a system body, converting a panic into a status the host can act on.
///
/// A panic unwinding out of an `extern "C"` function aborts the process, so
/// without this one bad index in a half-written system takes the editor down.
/// `AssertUnwindSafe` is required because the call carries raw pointers; that is
/// sound here because a panic leaves host memory in whatever state the partial
/// loop wrote, which is data the host owns and re-reads next frame — there are no
/// plugin-side invariants to break.
unsafe fn guard(call: &sys::SystemCall, body: impl FnOnce()) -> sys::SystemStatus {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)) {
        Ok(()) => sys::SystemStatus::Ok,
        Err(e) => {
            let msg = e
                .downcast_ref::<&str>()
                .map(|s| alloc::string::ToString::to_string(s))
                .or_else(|| e.downcast_ref::<alloc::string::String>().cloned())
                .unwrap_or_else(|| alloc::string::String::from("panic"));
            if !call.iface.is_null() {
                ((*call.iface).log)(
                    call.host,
                    sys::LogLevel::Error,
                    sys::StrRef { ptr: msg.as_ptr(), len: msg.len() },
                );
            }
            sys::SystemStatus::Panicked
        }
    }
}

unsafe extern "C" fn thunk_q<D, Fl, Func>(call: *const sys::SystemCall) -> sys::SystemStatus
where
    D: QueryData,
    Fl: QueryFilter,
    Func: for<'a> Fn(Query<'a, D, Fl>) + 'static,
{
    let call = &*call;
    guard(call, || materialize::<Func>()(Query::new(call)))
}

unsafe extern "C" fn thunk_qt<D, Fl, Func>(call: *const sys::SystemCall) -> sys::SystemStatus
where
    D: QueryData,
    Fl: QueryFilter,
    Func: for<'a> Fn(Query<'a, D, Fl>, Res<'a, Time>) + 'static,
{
    let call = &*call;
    guard(call, || {
        materialize::<Func>()(Query::new(call), Res(Time(call.frame), PhantomData))
    })
}

// ── App / Plugin ─────────────────────────────────────────────────────────────

/// Which schedule a system runs in. Mirrors Bevy's schedule labels.
pub use crate::sys::Schedule::{self, First, Last, PostUpdate, PreUpdate, Update};

/// Mirrors `bevy::App` for the surface a plugin can reach.
pub struct App {
    ctx: InitCtx,
}

impl App {
    /// # Safety
    /// `iface` and `host` must be the values the host passed to init.
    pub unsafe fn new(iface: *const sys::Interface, host: *mut sys::Host) -> Self {
        Self {
            ctx: InitCtx {
                iface,
                host,
                cache: alloc::vec::Vec::new(),
                unresolved: None,
            },
        }
    }

    /// The type path of the first component that could not be resolved.
    ///
    /// `add!` turns this into [`sys::InitResult::Failed`]; it is exposed so a
    /// plugin hand-writing its own entry point can do the same.
    pub fn unresolved_component(&self) -> Option<&'static str> {
        self.ctx.unresolved
    }

    pub fn add_systems<M, S: IntoSystem<M>>(&mut self, schedule: Schedule, system: S) -> &mut Self {
        let (terms, entry, user) = system.build(&mut self.ctx);
        // SAFETY: `terms` outlives the call; the host copies it into its own plan.
        unsafe {
            ((*self.ctx.iface).add_system)(
                self.ctx.host,
                schedule,
                entry,
                &sys::QueryDesc { terms: terms.as_ptr(), term_count: terms.len() },
                user,
            );
        }
        self
    }

    /// Register a plugin-owned component ahead of first use. Optional — a
    /// component named in a query is registered automatically.
    pub fn register_component<T: Component>(&mut self) -> &mut Self {
        self.ctx.id_of::<T>();
        self
    }
}

/// Mirrors `bevy::Plugin`.
pub trait Plugin {
    fn build(&self, app: &mut App);
}

extern crate alloc;
