//! Query data, filters, and the `Query` view itself.
//!
//! The safety contract that holds the whole thing together: a `QueryData`'s
//! `CELLS` must equal the number of terms its `terms` pushes, and `fetch` must
//! read exactly that many cells. The host lays cells out row-major on that
//! promise, so a type that miscounts reads a neighbouring component's bytes as
//! its own.
//!
//! `ReadOnly` exists for the reason Bevy's does: without it, `iter(&self)` on a
//! `Query<&mut T>` yielded `&mut T` from a shared borrow, so nesting `q.iter()`
//! inside `for x in &mut q` produced two live `&mut` to the same bytes with no
//! `unsafe` written anywhere.

use core::marker::PhantomData;

use crate::sys;

use super::component::Component;
use super::init::InitCtx;

/// What a query reads or writes. Implemented for `&T`, `&mut T`, and tuples.
///
/// # Safety
/// `CELLS` must equal the number of terms `terms` pushes, and `fetch` must read
/// exactly that many cells. The host lays cells out row-major on that promise.
///
/// [`Self::ReadOnly`] must have the same `CELLS` and read the same cells in the
/// same order, because iteration reuses one host layout for both projections —
/// the terms were declared once, from `Self`. A `ReadOnly` that disagreed would
/// read a neighbouring component's bytes as its own.
pub unsafe trait QueryData {
    type Item<'a>;
    /// This query's read-only projection: `&mut T` becomes `&T`, `Option<&mut T>`
    /// becomes `Option<&T>`, tuples map elementwise, everything else is itself.
    ///
    /// Exists so `iter(&self)` can hand out shared items while `iter_mut(&mut self)`
    /// hands out mutable ones — which is the only thing making the borrow checker
    /// able to see that two simultaneous iterations of one query would alias.
    /// Without it, `iter(&self)` on a `Query<&mut T>` yielded `&mut T` from a shared
    /// borrow, so nesting `q.iter()` inside `for x in &mut q` produced two live
    /// `&mut` to the same bytes with no `unsafe` written anywhere.
    ///
    /// Mirrors Bevy's `QueryData::ReadOnly`, which exists for the same reason.
    ///
    /// Its `CELLS` must equal this type's, and its `fetch` must read the same
    /// cells in the same order — see the trait's safety contract. That cannot be
    /// written as a bound (`associated_const_equality` is unstable), so it is an
    /// obligation on the implementor.
    type ReadOnly: QueryData;
    /// Cells this contributes per row. Zero for terms that read something other
    /// than component data, like [`sys::Entity`].
    const CELLS: usize;
    fn terms(ctx: &mut InitCtx, out: &mut alloc::vec::Vec<sys::Term>);
    /// # Safety
    /// `cells` points at this item's first cell in the row `row`, and `view` is
    /// live. `'a` is unbounded: the item borrows the host's staging buffers,
    /// which outlive the call but are not reachable from any argument, so
    /// nothing in the types can express the real bound. The caller is
    /// responsible for not letting an item escape the system body.
    unsafe fn fetch<'a>(
        view: *const sys::QueryView,
        row: usize,
        cells: *mut *mut u8,
    ) -> Self::Item<'a>;
}

/// Yields the entity being iterated. Mirrors Bevy's `Entity` query term.
///
/// Contributes no cell and no term — the host already sends an entity id per
/// row alongside the component data, so this costs nothing beyond reading it.
/// Without it a system cannot act on the entity it is looking at, which makes
/// "do this once, then mark it done" unwritable.
unsafe impl QueryData for sys::Entity {
    type Item<'a> = sys::Entity;
    type ReadOnly = Self;
    const CELLS: usize = 0;
    fn terms(_: &mut InitCtx, _: &mut alloc::vec::Vec<sys::Term>) {}
    unsafe fn fetch<'a>(
        view: *const sys::QueryView,
        row: usize,
        _cells: *mut *mut u8,
    ) -> Self::Item<'a> {
        if (*view).entities.is_null() {
            return sys::Entity(u64::MAX);
        }
        *(*view).entities.add(row)
    }
}

unsafe impl<T: Component> QueryData for &T {
    type Item<'a> = &'a T;
    type ReadOnly = Self;
    const CELLS: usize = 1;
    fn terms(ctx: &mut InitCtx, out: &mut alloc::vec::Vec<sys::Term>) {
        out.push(sys::Term { component: ctx.id_of::<T>(), access: sys::Access::Read });
    }
    unsafe fn fetch<'a>(_: *const sys::QueryView, _: usize, cells: *mut *mut u8) -> &'a T {
        &*(*cells as *const T)
    }
}

unsafe impl<T: Component> QueryData for &mut T {
    type Item<'a> = &'a mut T;
    /// The whole point of the projection: shared iteration of a `&mut T` query
    /// yields `&T`, exactly as it does in Bevy.
    type ReadOnly = &'static T;
    const CELLS: usize = 1;
    fn terms(ctx: &mut InitCtx, out: &mut alloc::vec::Vec<sys::Term>) {
        out.push(sys::Term { component: ctx.id_of::<T>(), access: sys::Access::Write });
    }
    unsafe fn fetch<'a>(_: *const sys::QueryView, _: usize, cells: *mut *mut u8) -> &'a mut T {
        &mut *(*cells as *mut T)
    }
}

/// Reads `T` when the entity has it. Mirrors Bevy's `Option<&T>`.
///
/// The term does not filter, so the entity matches either way and the cell is
/// null when the component is absent — which is what makes "every entity, plus
/// this extra data if it happens to be there" a single query instead of two.
unsafe impl<T: Component> QueryData for Option<&T> {
    type Item<'a> = Option<&'a T>;
    type ReadOnly = Self;
    const CELLS: usize = 1;
    fn terms(ctx: &mut InitCtx, out: &mut alloc::vec::Vec<sys::Term>) {
        out.push(sys::Term { component: ctx.id_of::<T>(), access: sys::Access::ReadOptional });
    }
    unsafe fn fetch<'a>(_: *const sys::QueryView, _: usize, cells: *mut *mut u8) -> Option<&'a T> {
        (*cells).cast::<T>().as_ref()
    }
}

unsafe impl<T: Component> QueryData for Option<&mut T> {
    type Item<'a> = Option<&'a mut T>;
    type ReadOnly = Option<&'static T>;
    const CELLS: usize = 1;
    fn terms(ctx: &mut InitCtx, out: &mut alloc::vec::Vec<sys::Term>) {
        out.push(sys::Term { component: ctx.id_of::<T>(), access: sys::Access::WriteOptional });
    }
    unsafe fn fetch<'a>(
        _: *const sys::QueryView,
        _: usize,
        cells: *mut *mut u8,
    ) -> Option<&'a mut T> {
        (*cells).cast::<T>().as_mut()
    }
}

/// Generates the tuple [`QueryData`] impls.
///
/// The cells offset has to accumulate across the tuple — element *n* reads from
/// `cells + sum(CELLS of 0..n)` — which a macro cannot express as a const
/// expression per element without quadratic repetition. A running counter in
/// `fetch` does it in one line instead, and is exact because Rust evaluates
/// tuple elements left to right.
macro_rules! query_data_tuples {
    ($(($($t:ident),+))+) => {
        $(
            #[allow(non_snake_case, unused_assignments)]
            unsafe impl<$($t: QueryData),+> QueryData for ($($t,)+) {
                type Item<'a> = ($($t::Item<'a>,)+);
                type ReadOnly = ($($t::ReadOnly,)+);
                const CELLS: usize = 0 $( + $t::CELLS )+;
                fn terms(ctx: &mut InitCtx, out: &mut alloc::vec::Vec<sys::Term>) {
                    $( $t::terms(ctx, out); )+
                }
                unsafe fn fetch<'a>(
                    view: *const sys::QueryView,
                    row: usize,
                    cells: *mut *mut u8,
                ) -> Self::Item<'a> {
                    let mut offset = 0usize;
                    ($({
                        let item = $t::fetch(view, row, cells.add(offset));
                        offset += $t::CELLS;
                        item
                    },)+)
                }
            }
        )+
    };
}

query_data_tuples! {
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

/// Matches entities whose `T` was added since this system last ran.
///
/// Like Bevy's, and with Bevy's implications: it also requires `T` to be
/// present, and takes a read borrow on it, so the system will not run in
/// parallel with anything writing `T`.
///
/// **A hot reload makes this fire for the whole scene.** Reloading keeps your
/// components but registers new systems, and a system that has never run treats
/// everything already in the world as freshly added. Reload is the inner loop of
/// plugin development, so do not use `Added<T>` for one-time setup.
///
/// Cannot appear inside [`Or`] — the host refuses the system at load and says so.
pub struct Added<T>(PhantomData<T>);

impl<T: Component> QueryFilter for Added<T> {
    fn terms(ctx: &mut InitCtx, out: &mut alloc::vec::Vec<sys::Term>) {
        out.push(sys::Term { component: ctx.id_of::<T>(), access: sys::Access::Added });
    }
}

/// Matches entities whose `T` changed since this system last ran.
///
/// Like Bevy's, with one semantic difference worth knowing before you rely on
/// it: **a plugin's `&mut` write only marks a component changed when the bytes
/// actually change.** Bevy marks it the moment you take `&mut`, whether or not
/// you wrote anything different. The host compares your staged bytes against a
/// snapshot taken before the system ran and skips the write when they match, and
/// skipping the write is what skips the tick.
///
/// So `Changed<T>` fires *less* often here than the same code would in-tree, and
/// there is no `set_changed()` to force it — the Bevy idiom `let _ = &mut *foo;`
/// compiles and does nothing. The comparison is bitwise over the whole
/// component, padding included, so it can report a change that did not really
/// happen but never misses one that did.
///
/// Cannot appear inside [`Or`] — the host refuses the system at load and says so.
pub struct Changed<T>(PhantomData<T>);

impl<T: Component> QueryFilter for Changed<T> {
    fn terms(ctx: &mut InitCtx, out: &mut alloc::vec::Vec<sys::Term>) {
        out.push(sys::Term { component: ctx.id_of::<T>(), access: sys::Access::Changed });
    }
}

/// Generates the tuple [`QueryFilter`] impls. A filter tuple means "and", and
/// contributes no cells, so this is just concatenation.
macro_rules! query_filter_tuples {
    ($(($($t:ident),+))+) => {
        $(
            impl<$($t: QueryFilter),+> QueryFilter for ($($t,)+) {
                fn terms(ctx: &mut InitCtx, out: &mut alloc::vec::Vec<sys::Term>) {
                    $( $t::terms(ctx, out); )+
                }
            }
        )+
    };
}

query_filter_tuples! {
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
}

/// Matches entities satisfying any branch. Mirrors `bevy::Or<(..)>`.
///
/// Encoded as a bracketed run of terms — [`sys::Access::OrBegin`], one branch
/// per [`sys::Access::OrNext`] separator, then [`sys::Access::OrEnd`] — rather
/// than as a nested descriptor, so a query stays one flat term list at the
/// boundary no matter how the filter is spelled.
pub struct Or<T>(PhantomData<T>);

macro_rules! or_filters {
    ($(($first:ident $(, $rest:ident)*))+) => {
        $(
            impl<$first: QueryFilter $(, $rest: QueryFilter)*> QueryFilter
                for Or<($first, $($rest,)*)>
            {
                fn terms(ctx: &mut InitCtx, out: &mut alloc::vec::Vec<sys::Term>) {
                    out.push(sys::Term::marker(sys::Access::OrBegin));
                    $first::terms(ctx, out);
                    $(
                        out.push(sys::Term::marker(sys::Access::OrNext));
                        $rest::terms(ctx, out);
                    )*
                    out.push(sys::Term::marker(sys::Access::OrEnd));
                }
            }
        )+
    };
}

or_filters! {
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
}

// ── Query ────────────────────────────────────────────────────────────────────

/// A view over the entities this system matched. Mirrors `bevy_ecs::Query`.
pub struct Query<'a, D: QueryData, F: QueryFilter = ()> {
    view: sys::QueryView,
    _p: PhantomData<(&'a (), D, F)>,
}

impl<'a, D: QueryData, F: QueryFilter> Query<'a, D, F> {
    /// # Safety
    /// `view` must index a live [`sys::QueryView`] whose terms were declared by
    /// this exact `D`/`F` pair.
    pub(crate) unsafe fn new(call: *const sys::SystemCall, view: usize) -> Self {
        // Copied, not borrowed. A `QueryView` is four words and `Copy`, so this
        // is free — and it keeps the whole lifetime and `Sync` question out of
        // the type, which a `&'static` fallback for the out-of-range case would
        // otherwise drag in.
        let view = if view < (*call).view_count {
            *(*call).views.add(view)
        } else {
            // Unreachable if the host honoured the descriptor. A query that
            // yields nothing still beats one that reads a wild pointer.
            EMPTY_VIEW
        };
        Self {
            view,
            _p: PhantomData,
        }
    }

    /// Shared iteration. Yields `D`'s read-only projection, so a `Query<&mut T>`
    /// gives you `&T` here and `&mut T` from [`Self::iter_mut`] — same split as
    /// Bevy, and the reason two simultaneous iterations no longer alias.
    pub fn iter(&self) -> QueryIter<'_, D::ReadOnly, F> {
        QueryIter {
            view: self.view,
            row: 0,
            _p: PhantomData,
        }
    }

    /// Exclusive iteration. Yields `D` itself, mutable terms included.
    pub fn iter_mut(&mut self) -> QueryIter<'_, D, F> {
        QueryIter {
            view: self.view,
            row: 0,
            _p: PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        self.view.entity_count
    }

    /// The one matching item, or `None` if there is not exactly one.
    ///
    /// Bevy returns a `Result` and panics on `.single().unwrap()`; here it is an
    /// `Option`, because a plugin that panics loses its system for the session
    /// and `if let Some(x)` is the shape that avoids it.
    pub fn single(&self) -> Option<<D::ReadOnly as QueryData>::Item<'_>> {
        (self.view.entity_count == 1).then(|| self.iter().next()).flatten()
    }

    /// The one matching item, mutably.
    pub fn single_mut(&mut self) -> Option<D::Item<'_>> {
        (self.view.entity_count == 1).then(|| self.iter_mut().next()).flatten()
    }

    /// The item for `entity`, if this query matched it.
    ///
    /// Linear in the number of matched rows: the host hands over a flat array,
    /// not a map. Fine for the "did the thing I spawned match?" case; do not put
    /// it inside a loop over another query.
    pub fn get(&self, entity: sys::Entity) -> Option<<D::ReadOnly as QueryData>::Item<'_>> {
        let row = self.row_of(entity)?;
        self.iter().nth(row)
    }

    /// The item for `entity`, mutably.
    pub fn get_mut(&mut self, entity: sys::Entity) -> Option<D::Item<'_>> {
        let row = self.row_of(entity)?;
        self.iter_mut().nth(row)
    }

    /// Whether this query matched `entity`.
    pub fn contains(&self, entity: sys::Entity) -> bool {
        self.row_of(entity).is_some()
    }

    fn row_of(&self, entity: sys::Entity) -> Option<usize> {
        if self.view.entities.is_null() {
            return None;
        }
        // SAFETY: the host wrote `entity_count` ids at this pointer.
        let ids = unsafe {
            core::slice::from_raw_parts(self.view.entities, self.view.entity_count)
        };
        ids.iter().position(|e| e.0 == entity.0)
    }

    pub fn is_empty(&self) -> bool {
        self.view.entity_count == 0
    }
}

/// Stands in when a view index is out of range. Empty, so iterating it is a
/// no-op rather than a fault.
const EMPTY_VIEW: sys::QueryView = sys::QueryView {
    cells: core::ptr::null_mut(),
    entities: core::ptr::null(),
    entity_count: 0,
    cell_count: 0,
};

pub struct QueryIter<'a, D: QueryData, F: QueryFilter> {
    view: sys::QueryView,
    row: usize,
    _p: PhantomData<(&'a (), D, F)>,
}

impl<'a, D: QueryData, F: QueryFilter> Iterator for QueryIter<'a, D, F> {
    type Item = D::Item<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.row >= self.view.entity_count {
            return None;
        }
        // SAFETY: row < entity_count, and the host laid out `cell_count` cells
        // per row in declaration order.
        let item = unsafe {
            D::fetch(
                &self.view as *const sys::QueryView,
                self.row,
                self.view.cells.add(self.row * self.view.cell_count),
            )
        };
        self.row += 1;
        Some(item)
    }
}

/// `for x in &q` — shared, so `x` is `D`'s read-only projection.
impl<'a, D: QueryData, F: QueryFilter> IntoIterator for &'a Query<'_, D, F> {
    type Item = <D::ReadOnly as QueryData>::Item<'a>;
    type IntoIter = QueryIter<'a, D::ReadOnly, F>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// `for x in &mut q` — exclusive, so `x` is `D` itself.
impl<'a, D: QueryData, F: QueryFilter> IntoIterator for &'a mut Query<'_, D, F> {
    type Item = D::Item<'a>;
    type IntoIter = QueryIter<'a, D, F>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
