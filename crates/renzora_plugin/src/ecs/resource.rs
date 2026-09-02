//! Resources, and the two pseudo-resources that never touch the world.
//!
//! [`ResourceParam`] is separate from [`Resource`] because the two kinds resolve
//! differently: a plugin's own resource lives in the host world and arrives as a
//! pointer in [`sys::SystemCall::resources`], while [`Time`] and [`Input`] are
//! read straight out of the call and declare no access at all — so two systems
//! reading input can never conflict.

use core::marker::PhantomData;

use crate::sys;

use super::init::InitCtx;

/// A single global value, one per world. Mirrors `bevy::Resource`.
///
/// Emitted by `#[derive(Resource)]`. Registration and layout work exactly like a
/// component's, because in Bevy a resource *is* a component on a hidden entity —
/// the same descriptor, the same field schema, the same default constructor.
pub trait Resource: Sized + 'static {
    const TYPE_PATH: &'static str;
    fn display_name() -> &'static str;
    fn id_cell() -> &'static core::sync::atomic::AtomicU32;
    fn fields() -> &'static [sys::FieldDesc];
    /// See [`super::Component::field_ranges`].
    fn field_ranges() -> &'static [(usize, sys::FieldRange)] {
        &[]
    }
    fn descriptor() -> sys::ComponentDesc;
}

/// The id the host assigned `T`, or `INVALID` if it was never registered.
pub fn resource_id_of<T: Resource>() -> sys::ComponentId {
    sys::ComponentId(T::id_cell().load(core::sync::atomic::Ordering::Relaxed))
}

/// What can sit inside [`Res`] / [`ResMut`].
///
/// Two kinds satisfy this and they resolve differently, which is why it is its
/// own trait rather than folded into [`Resource`]: a plugin's own resource lives
/// in the host world and arrives as a pointer in
/// [`sys::SystemCall::resources`], while [`Time`] is read straight out of the
/// call's frame context and never round-trips through the world at all.
///
/// # Safety
/// `res_ptr` must return a pointer valid for the duration of the call, or null.
pub unsafe trait ResourceParam: Sized {
    fn res_term(ctx: &mut InitCtx, out: &mut alloc::vec::Vec<sys::Term>, access: sys::Access);
    /// # Safety
    /// `call` must be live.
    unsafe fn res_ptr(call: *const sys::SystemCall) -> *mut Self;
}

unsafe impl<T: Resource> ResourceParam for T {
    fn res_term(ctx: &mut InitCtx, out: &mut alloc::vec::Vec<sys::Term>, access: sys::Access) {
        out.push(sys::Term { component: ctx.resource_id_of::<T>(), access });
    }
    unsafe fn res_ptr(call: *const sys::SystemCall) -> *mut T {
        let want = resource_id_of::<T>();
        let slots = core::slice::from_raw_parts((*call).resources, (*call).resource_count);
        slots
            .iter()
            .find(|s| s.id == want)
            .map_or(core::ptr::null_mut(), |s| s.ptr.cast())
    }
}

/// The frame clock. Mirrors `bevy::Time` for the parts a plugin can reach.
///
/// `#[repr(transparent)]` over the frame context so [`ResourceParam::res_ptr`]
/// can hand back a pointer to the live call's own field rather than to a
/// temporary — a `Res<Time>` borrows the call, exactly like every other param.
#[repr(transparent)]
pub struct Time(sys::FrameCtx);

impl Time {
    pub fn delta_secs(&self) -> f32 {
        self.0.delta_secs
    }
    pub fn elapsed_secs(&self) -> f32 {
        self.0.elapsed_secs
    }
}

unsafe impl ResourceParam for Time {
    // Declares nothing: the host sends the frame context with every call, so
    // there is no access to negotiate and no scheduling conflict to create.
    fn res_term(_: &mut InitCtx, _: &mut alloc::vec::Vec<sys::Term>, _: sys::Access) {}
    unsafe fn res_ptr(call: *const sys::SystemCall) -> *mut Time {
        core::ptr::addr_of!((*call).frame) as *mut Time
    }
}

/// This frame's keyboard, mouse and cursor.
///
/// ```rust,ignore
/// fn walk(input: Res<Input>, mut q: Query<&mut Transform>) {
///     for t in &mut q {
///         if input.pressed(Key::W) { t.translation.z -= 0.1; }
///         if input.just_pressed(Key::Space) { info("jump"); }
///     }
/// }
/// ```
///
/// One param covering keyboard, mouse and cursor, where Bevy has
/// `Res<ButtonInput<KeyCode>>`, `Res<ButtonInput<MouseButton>>` and a window
/// query. The generic form cannot cross the boundary — there is no way to
/// instantiate `ButtonInput<T>` at runtime from the other side — so this is the
/// one place the surface reads deliberately unlike Bevy rather than accidentally.
///
/// Reading it costs nothing: the host already has the state as a bitset and sends
/// it with the call, so `pressed` is a shift and a mask inside the plugin.
#[repr(transparent)]
pub struct Input(sys::InputState);

impl Input {
    pub fn pressed(&self, key: sys::Key) -> bool {
        self.0.pressed(key)
    }
    pub fn just_pressed(&self, key: sys::Key) -> bool {
        self.0.just_pressed(key)
    }
    pub fn just_released(&self, key: sys::Key) -> bool {
        self.0.just_released(key)
    }
    pub fn mouse_pressed(&self, button: sys::MouseButton) -> bool {
        self.0.mouse_pressed(button)
    }
    pub fn mouse_just_pressed(&self, button: sys::MouseButton) -> bool {
        self.0.mouse_just_pressed(button)
    }
    pub fn mouse_just_released(&self, button: sys::MouseButton) -> bool {
        self.0.mouse_just_released(button)
    }
    /// Cursor position in the primary window, in logical pixels from the top left.
    /// `None` while the cursor is outside the window.
    pub fn cursor(&self) -> Option<(f32, f32)> {
        (!self.0.cursor_x.is_nan() && !self.0.cursor_y.is_nan())
            .then_some((self.0.cursor_x, self.0.cursor_y))
    }
    /// Cursor movement since the previous frame. Still reported while the cursor
    /// is locked, which is what a first-person camera needs.
    pub fn cursor_delta(&self) -> (f32, f32) {
        (self.0.cursor_delta_x, self.0.cursor_delta_y)
    }
    /// Wheel movement this frame, in lines.
    pub fn scroll(&self) -> (f32, f32) {
        (self.0.scroll_x, self.0.scroll_y)
    }
}

/// Zeroed input, used when the host sent none — a headless server has no
/// keyboard, and every query against this reads as "not pressed".
static NO_INPUT: sys::InputState = sys::InputState {
    keys_down: [0; 4],
    keys_just_pressed: [0; 4],
    keys_just_released: [0; 4],
    mouse_down: 0,
    mouse_just_pressed: 0,
    mouse_just_released: 0,
    // Not NaN, because a `const` cannot call `f32::NAN`'s constructor in a static
    // initialiser on every toolchain this must build with. `cursor()` treats a
    // zeroed cursor as present-at-origin, which for a host with no cursor at all
    // is a distinction without a difference.
    cursor_x: 0.0,
    cursor_y: 0.0,
    cursor_delta_x: 0.0,
    cursor_delta_y: 0.0,
    scroll_x: 0.0,
    scroll_y: 0.0,
};

unsafe impl ResourceParam for Input {
    // Declares nothing, like `Time`: the host sends input with every call, so
    // there is no access to negotiate and two systems reading input never
    // conflict.
    fn res_term(_: &mut InitCtx, _: &mut alloc::vec::Vec<sys::Term>, _: sys::Access) {}
    unsafe fn res_ptr(call: *const sys::SystemCall) -> *mut Input {
        let ptr = (*call).input;
        if ptr.is_null() {
            // A `Res<Input>` must never be absent — a plugin checking
            // `is_present()` before reading a key would be absurd — so a host with
            // no input gets the zeroed static instead of a null.
            return core::ptr::addr_of!(NO_INPUT) as *mut Input;
        }
        ptr as *mut Input
    }
}

/// Shared access to a resource. Mirrors `bevy::Res`.
pub struct Res<'a, T>(pub(crate) *mut T, pub(crate) PhantomData<&'a T>);

impl<T> Res<'_, T> {
    /// Whether the resource exists. Bevy would have skipped the whole system;
    /// here it runs and the plugin decides, which keeps a missing resource a
    /// local question rather than a silent scheduling puzzle.
    pub fn is_present(&self) -> bool {
        !self.0.is_null()
    }
    pub fn get(&self) -> Option<&T> {
        unsafe { self.0.as_ref() }
    }
}

impl<T> core::ops::Deref for Res<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.get()
            .expect("resource not present — check `is_present()` first")
    }
}

/// Mutable access to a resource. Mirrors `bevy::ResMut`.
pub struct ResMut<'a, T>(pub(crate) *mut T, pub(crate) PhantomData<&'a mut T>);

impl<T> ResMut<'_, T> {
    pub fn is_present(&self) -> bool {
        !self.0.is_null()
    }
    pub fn get(&self) -> Option<&T> {
        unsafe { self.0.as_ref() }
    }
    pub fn get_mut(&mut self) -> Option<&mut T> {
        unsafe { self.0.as_mut() }
    }
}

impl<T> core::ops::Deref for ResMut<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.get()
            .expect("resource not present — check `is_present()` first")
    }
}

impl<T> core::ops::DerefMut for ResMut<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.get_mut()
            .expect("resource not present — check `is_present()` first")
    }
}
