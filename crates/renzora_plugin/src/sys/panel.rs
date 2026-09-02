//! Editor panels, removal tracking, and the plugin's sole export.

use core::ffi::c_void;

use super::{CommandSink, ComponentId, Entity, Host, Interface, StrRef, SystemStatus};

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
    ///
    /// [`Str256`]: super::Str256
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
///
/// [`MeshRead`]: super::MeshRead
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
///
/// [`MeshSource`]: super::MeshSource
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
///
/// [`INIT_SYMBOL`]: super::INIT_SYMBOL
pub type ExtensionInit =
    unsafe extern "C" fn(iface: *const Interface, host: *mut Host) -> InitResult;
