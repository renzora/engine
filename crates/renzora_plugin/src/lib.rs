//! Write Renzora plugins in Rust.
//!
//! A plugin is a `cdylib` whose entire manifest is:
//!
//! ```toml
//! [lib]
//! crate-type = ["cdylib"]
//!
//! [dependencies]
//! renzora_plugin = "0.1"
//! ```
//!
//! No engine checkout, no Bevy, no workspace, no pinned toolchain, no Docker.
//! That works because a plugin exports one symbol and imports nothing from the
//! host — see [`sys`] for the mechanism and why it is safe.
//!
//! ## What this crate is
//!
//! [`sys`] is the raw C ABI: `#[repr(C)]` types and a function-pointer table.
//! The crate root (this module) is the ergonomic layer over it, and its job is
//! to make plugin source **identical to Bevy source**:
//!
//! ```ignore
//! use renzora_plugin::prelude::*;
//!
//! #[derive(Component)]
//! struct Spinner { speed: f32 }
//!
//! fn spin(mut q: Query<(&mut Transform, &Spinner)>, time: Res<Time>) {
//!     for (mut t, s) in &mut q {
//!         t.rotate_y(s.speed * time.delta_secs());
//!     }
//! }
//!
//! pub struct SpinPlugin;
//! impl Plugin for SpinPlugin {
//!     fn build(&self, app: &mut App) {
//!         app.add_systems(Update, spin);
//!     }
//! }
//!
//! renzora_plugin::add!(SpinPlugin);
//! ```
//!
//! `Query`, `Res`, `App`, `Plugin` and `Transform` above are *this crate's*
//! types, not Bevy's. Mirroring Bevy's names and signatures exactly is a hard
//! design constraint, not a nicety: it means existing Bevy knowledge transfers
//! untouched and porting a plugin is a change to the `use` line. Resist the
//! temptation to improve on an API here — a "better" name is a name nobody
//! already knows.
//!
//! ## What a plugin cannot do
//!
//! Reach anything the table does not expose. Most importantly it cannot use
//! ecosystem crates that take real Bevy types (`bevy_mod_outline`, `bevy_hanabi`
//! …), because the types above are shims. Anything needing that is an in-tree
//! engine crate instead, compiled against real Bevy — which is the deliberate
//! split between the two tiers, not a gap to be closed.

pub mod ecs;
pub mod sys;

/// Animation: play clips, drive state machines, read animator state.
///
/// Opt in with `features = ["anim"]`. It is a *user* of the boundary, not part
/// of it — see the module doc for why a domain lives here rather than in [`sys`],
/// and why adding one does not move the ABI version.
#[cfg(feature = "anim")]
pub mod anim;

#[cfg(feature = "host")]
pub mod host;

/// `#[derive(Component)]`. Re-exported so a plugin depends on exactly one crate
/// — the proc-macro crate is an implementation detail (a proc macro must live in
/// its own crate; that is a rustc rule, not a structural choice).
pub use renzora_plugin_derive::{bsn, bsn_list, Component, Resource};

/// Everything a plugin needs. Mirrors `bevy::prelude`.
pub mod prelude {
    pub use renzora_plugin_derive::{bsn, bsn_list, Component, Resource};
    pub use crate::ecs::{
        Action, App, Bundle, Commands, Entity, EntityCommands, Input, Mesh3d, Or, Panel, Plugin,
        Quat, Query, RenderPass, Res, ResMut, Resource, Scene, Schedule, Time, Transform, Vec3,
        Visibility,
        With, Without,
    };
    // `Key::W` and `MouseButton::Left` read like the Bevy names they map to, and a
    // plugin writing input handling wants both without reaching into `sys`.
    pub use crate::sys::{Key, MouseButton};
    // A post-process effect names the phase it runs in; without this every one of
    // them starts with a `use renzora_plugin::sys::RenderPhase`.
    pub use crate::sys::RenderPhase;
    pub use crate::ecs::{error, info, warn};
    pub use crate::ecs::{First, Last, PostUpdate, PreUpdate, Update};
}

/// Emit the plugin's sole export.
///
/// Wraps the `Plugin::build` call in the version handshake and the pointer
/// plumbing, so a plugin author writes one line instead of an `unsafe extern "C"`
/// function. Exactly one of these per cdylib — the symbol is unmangled and two
/// would collide at link time.
#[macro_export]
macro_rules! add {
    // `add!(MyPlugin, Editor)` — an editor-only plugin. Absent from the shipped
    // runtime binary entirely, rather than present and inactive.
    ($plugin:expr, $scope:ident) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn renzora_plugin_scope() -> $crate::sys::PluginScope {
            $crate::sys::PluginScope::$scope
        }
        $crate::add!($plugin);
    };
    ($plugin:expr) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn renzora_plugin_init(
            iface: *const $crate::sys::Interface,
            host: *mut $crate::sys::Host,
        ) -> $crate::sys::InitResult {
            let i = &*iface;
            // A newer host is always fine — the table is append-only. An older
            // one is not, because we may call a function it lacks.
            if i.version_major != $crate::sys::VERSION_MAJOR
                || i.version_minor < $crate::sys::VERSION_MINOR
            {
                return $crate::sys::InitResult::VersionTooOld;
            }
            let mut app = $crate::ecs::App::new(iface, host);
            // A panic in `build` would unwind out of an `extern "C"` fn and abort
            // the editor. Refusing to load is the correct outcome instead.
            let built = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                $crate::ecs::Plugin::build(&$plugin, &mut app);
            }));
            if built.is_err() {
                return $crate::sys::InitResult::Failed;
            }
            // Refuse rather than install systems whose queries can never match.
            // The host logs which component was missing.
            if app.unresolved_component().is_some() {
                return $crate::sys::InitResult::Failed;
            }
            // Likewise for a system the host declined — an access conflict, or a
            // term it could not resolve. Loading anyway would give a plugin that
            // reports success and then quietly does less than it says.
            if app.rejected_system().is_some() {
                return $crate::sys::InitResult::Failed;
            }
            $crate::sys::InitResult::Ok
        }
    };
}
