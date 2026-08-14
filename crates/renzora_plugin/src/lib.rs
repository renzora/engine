#![cfg_attr(not(feature = "std"), no_std)]

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
//!
//! ## `no_std`
//!
//! A plugin can drop the standard library and shrink from ~112 KB to ~18 KB:
//!
//! ```toml
//! [dependencies]
//! renzora_plugin = { version = "0.1", default-features = false, features = ["libm"] }
//!
//! [profile.dist]
//! inherits = "release"
//! panic = "abort"   # mandatory: `no_std` on stable cannot unwind
//! ```
//!
//! ```ignore
//! #![no_std]
//! extern crate alloc;
//! renzora_plugin::no_std_runtime!();   // allocator + panic handler
//! ```
//!
//! The cost is the panic firewall — see the `std` feature in this crate's
//! manifest for what exactly is given up, and [`no_std_runtime!`] for what the
//! macro emits. The `script` and `host` features are unavailable without `std`.

// Declared at the root so every module reaches `alloc::` the same way in both
// builds. A plugin allocates — components, query buffers, log strings — so
// dropping `std` means dropping to `alloc`, not to `core`.
extern crate alloc;

// `no_std` without a math source would fail deep inside `Vec3::length` with a
// missing-method error that says nothing about the cause. Say it here instead.
#[cfg(all(not(feature = "std"), not(feature = "libm")))]
compile_error!(
    "renzora_plugin without `std` needs a soft-float math source: \
     features = [\"libm\"]"
);

pub mod ecs;
pub mod static_link;
pub mod sys;

/// Cross-checks a post-process component's `#[repr(C)]` layout against the WGSL
/// uniform block it is uploaded into — the one defect in a near-identical set of
/// ~59 effect plugins that produces a wrong picture rather than an error.
///
/// Not part of the ABI: nothing here appears in [`sys::Interface`], so adding it
/// moves no version and no prefix hash. Compiled unconditionally (rather than
/// behind a `cfg(test)`) because a plugin's test binary is a *different* crate
/// from this one — a `cfg(test)` module here would be invisible to every plugin
/// that wants to call it.
pub mod uniform_check;

/// The byte codec every encoded boundary payload is written with.
///
/// Not behind a feature: it is shared by [`script`] and [`audio`], and the
/// whole point of one codec is that there is one copy of it.
pub mod wire;

/// `f32` math for `no_std` plugins — the `std`-only methods, over `libm`.
/// Exists only in a `no_std` build; see the module docs for why.
#[cfg(not(feature = "std"))]
pub mod float;

/// The allocator and abort behind [`no_std_runtime!`]. Public because the macro
/// expands in the plugin's crate and has to name them; not something a plugin
/// calls directly.
///
/// Compiled in **every** configuration, including `std` where nothing installs
/// it, so that its unsafe pointer arithmetic is reachable from the test suite.
/// A `no_std`-only module would be a module CI never builds and never runs,
/// which is the worst place to put hand-written allocator code.
pub mod no_std_heap;

/// Animation: play clips, drive state machines, read animator state.
///
/// Opt in with `features = ["anim"]`. It is a *user* of the boundary, not part
/// of it — see the module doc for why a domain lives here rather than in [`sys`],
/// and why adding one does not move the ABI version.
#[cfg(feature = "anim")]
pub mod anim;

/// Physics: forces, impulses, velocity, and reading a body's state.
///
/// Opt in with `features = ["physics"]`. Same shape as [`anim`] — a user of the
/// boundary, not part of it.
#[cfg(feature = "physics")]
pub mod physics;

/// HTTP: fire a request, poll for the response.
///
/// Opt in with `features = ["http"]`. Same shape as [`anim`] and [`physics`].
#[cfg(feature = "http")]
pub mod http;

/// File dialogs: ask the host to open a native file or folder picker.
///
/// Opt in with `features = ["dialog"]`. Same shape as [`anim`] and [`http`] —
/// and the first domain that cost no ABI surface at all, because the reply rides
/// the generic channel rather than a source of its own.
#[cfg(feature = "dialog")]
pub mod dialog;

/// Panels: replace a registered panel's contents at run time.
///
/// Not behind a feature, unlike the domains above, because the other half of it
/// — `App::add_panel` — is not either. A plugin that registers a panel should
/// not have to opt into a second name to change what is in it.
pub mod panel;

/// Diagnostics: read the host's frame time, FPS, entity count and per-pass GPU
/// times from a system.
///
/// Not behind a feature, for the same reason as [`panel`] and a different one
/// besides: the source it reads is a field of [`sys::SystemCall`] in every
/// build, so gating the reader would gate access to something already there.
pub mod diagnostics;

/// Scripting: implement a language backend the engine can run scripts through.
///
/// Opt in with `features = ["script"]`. The odd one out among the domains —
/// the host calls *into* the plugin here, so this one does touch [`sys`]. See
/// the module docs for the shape of a call.
#[cfg(feature = "script")]
pub mod script;

/// Audio: implement the mixer the engine plays through.
///
/// Opt in with `features = ["audio"]`. Scripting's shape rather than a domain's
/// — the host calls *into* the plugin here — so like [`script`] this one does
/// touch [`sys`]. See the module docs for the shape of a call.
#[cfg(feature = "audio")]
pub mod audio;

/// Networking: implement the HTTP client the engine fetches through.
///
/// Opt in with `features = ["net"]`. Scripting's shape rather than a domain's —
/// the host calls *into* the plugin here — so like [`script`] and [`audio`] this
/// one does touch [`sys`]. Not to be confused with [`http`], which is the same
/// protocol pointed the other way; see the module docs for both.
#[cfg(feature = "net")]
pub mod net;

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
        Action, Added, App, Bundle, Changed, Commands, Entity, EntityCommands, Images, Input,
        Mesh3d, MeshData, Meshes,
        Or, Panel, Plugin, Quat, Query, RemovedComponents, RenderPass, Res, ResMut, Resource,
        Scene, Schedule,
        Color, Str256, Time, Transform, Vec2, Vec3, Visibility, With, Without,
    };
    // `Key::W` and `MouseButton::Left` read like the Bevy names they map to, and a
    // plugin writing input handling wants both without reaching into `sys`.
    pub use crate::sys::{Key, MouseButton};
    // A post-process effect names the phase it runs in; without this every one of
    // them starts with a `use renzora_plugin::sys::RenderPhase`.
    pub use crate::sys::RenderPhase;
    // Both forms: the macros for `info!("x = {x}")`, the functions for
    // `info(&msg)`. Macros and values are separate namespaces, so these coexist.
    pub use crate::ecs::{error, info, warn};
    pub use crate::{error, info, warn};
    pub use crate::ecs::{First, Last, PostUpdate, PreUpdate, Update};

    // The owned containers, from `alloc`.
    //
    // Bevy's prelude does not carry these because the std prelude already has
    // them — but a `#![no_std]` plugin HAS no std prelude, and `Vec` suddenly
    // not existing is a confusing way to meet that. Re-exporting them here means
    // a plugin's source is the same either way, which is the same principle the
    // rest of this module follows.
    //
    // Safe under `std` too: `std::vec::Vec` *is* `alloc::vec::Vec` re-exported,
    // so this shadows the language prelude with the identical item rather than
    // introducing an ambiguity.
    pub use alloc::borrow::ToOwned;
    pub use alloc::boxed::Box;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec::Vec;
    pub use alloc::{format, vec};

    // `f32::sin` and friends, which `core` does not have. Only under `no_std` —
    // with `std` the inherent methods are already there, and exporting a trait
    // carrying the same names would be a second candidate for every call.
    #[cfg(not(feature = "std"))]
    pub use crate::float::FloatExt;
}


/// `format!` for the log macros below, resolved through `$crate` rather than
/// written as `::std::format!` at each call site.
///
/// The distinction matters because the macros expand inside the *plugin's*
/// crate: `::std` there names whatever the plugin has, which under `#![no_std]`
/// is nothing. Routing through `$crate` picks up this crate's `alloc` instead,
/// which exists in both builds.
#[doc(hidden)]
pub use alloc::format as __format;

/// `info!("x = {x}")`, formatting like Bevy's.
///
/// A macro *and* a function, deliberately. Bevy's logging is macros and plugin
/// source is meant to be Bevy source, so `info!(..)` has to work — but macros and
/// values live in separate namespaces, so [`ecs::info`] keeps working too and
/// neither shadows the other.
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => { $crate::ecs::info(&$crate::__format!($($arg)*)) };
}

/// `warn!("…")`. See [`info!`].
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => { $crate::ecs::warn(&$crate::__format!($($arg)*)) };
}

/// `error!("…")`. See [`info!`].
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => { $crate::ecs::error(&$crate::__format!($($arg)*)) };
}

/// Emit the plugin's exports.
///
/// Wraps the `Plugin::build` call in the version handshake and the pointer
/// plumbing, so a plugin author writes one line instead of an `unsafe extern "C"`
/// function. Exactly one of these per cdylib — the symbols are unmangled and two
/// would collide at link time.
///
/// The exception is a plugin compiled with the `static_link` feature, which
/// deliberately drops the mangling guard so a whole set of plugins can be linked
/// into one binary; see [`crate::static_link`] for who does that and why.
#[macro_export]
macro_rules! add {
    // `add!(MyPlugin, Editor)` — an editor-only plugin. Absent from the shipped
    // runtime binary entirely, rather than present and inactive.
    ($plugin:expr, $scope:ident) => {
        $crate::__plugin_scope_entry!($crate::sys::PluginScope::$scope);
        $crate::__plugin_init_entry!($plugin);
    };
    ($plugin:expr) => {
        // Emitted even though `Runtime` is what the loader assumes when the
        // symbol is missing. Declaring it costs one function and makes the scope
        // readable the same way for every plugin — which is what lets the
        // statically-linked path (`static_link`) call it unconditionally instead
        // of guessing whether an aggregator may name it.
        $crate::__plugin_scope_entry!($crate::sys::PluginScope::Runtime);
        $crate::__plugin_init_entry!($plugin);
    };
}

/// The `renzora_plugin_scope` half of [`add!`], with the export attribute the
/// current link mode needs.
///
/// This exists as its own macro because the choice cannot be made inside
/// `add!`'s expansion: a `#[cfg(feature = ...)]` written there is evaluated when
/// the **plugin** is compiled, against the plugin's own manifest, where
/// `static_link` does not exist and never will. Putting the two variants here
/// evaluates the cfg where the feature actually lives.
#[doc(hidden)]
#[cfg(not(feature = "static_link"))]
#[macro_export]
macro_rules! __plugin_scope_entry {
    ($scope:expr) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn renzora_plugin_scope() -> $crate::sys::PluginScope {
            $scope
        }
    };
}

/// Linked-in variant: no `#[no_mangle]`, so many plugins can coexist in one
/// binary. The aggregator calls it by path instead of by symbol name.
#[doc(hidden)]
#[cfg(feature = "static_link")]
#[macro_export]
macro_rules! __plugin_scope_entry {
    ($scope:expr) => {
        pub extern "C" fn renzora_plugin_scope() -> $crate::sys::PluginScope {
            $scope
        }
    };
}

/// The `renzora_plugin_init` half of [`add!`]. Two definitions, differing only
/// in the export attribute — see [`__plugin_scope_entry`] and the `static_link`
/// feature in this crate's manifest.
#[doc(hidden)]
#[cfg(not(feature = "static_link"))]
#[macro_export]
macro_rules! __plugin_init_entry {
    ($plugin:expr) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn renzora_plugin_init(
            iface: *const $crate::sys::Interface,
            host: *mut $crate::sys::Host,
        ) -> $crate::sys::InitResult {
            $crate::__plugin_init_body!($plugin, iface, host)
        }
    };
}

/// Linked-in variant. See [`__plugin_scope_entry`].
#[doc(hidden)]
#[cfg(feature = "static_link")]
#[macro_export]
macro_rules! __plugin_init_entry {
    ($plugin:expr) => {
        pub unsafe extern "C" fn renzora_plugin_init(
            iface: *const $crate::sys::Interface,
            host: *mut $crate::sys::Host,
        ) -> $crate::sys::InitResult {
            $crate::__plugin_init_body!($plugin, iface, host)
        }
    };
}

/// The body both `__plugin_init_entry!` variants share — the version handshake,
/// the shape check, and the guarded `Plugin::build` call. Factored out so the
/// two differ in nothing but their attribute; a copy-paste pair would drift, and
/// the half that drifted would be the one nobody builds day to day.
#[doc(hidden)]
#[macro_export]
macro_rules! __plugin_init_body {
    ($plugin:expr, $iface:expr, $host:expr) => {{
        let iface = $iface;
        let host = $host;
        let i = &*iface;
        // A newer host is always fine — the table is append-only. An older
        // one is not, because we may call a function it lacks.
        if i.version_major != $crate::sys::VERSION_MAJOR
            || i.version_minor < $crate::sys::VERSION_MINOR
        {
            return $crate::sys::InitResult::VersionTooOld;
        }
        // The version numbers are two integers a human types, so they say
        // nothing about whether the table is actually shaped the way this
        // plugin was compiled to read it. That gap is not theoretical: two
        // functions were once inserted mid-struct and released as MINOR
        // bumps, which sends an older plugin's call into a different
        // function — passing, say, a mesh descriptor to something that reads
        // it as an image descriptor. A segfault, and the panic guard around
        // plugin calls catches panics rather than those.
        //
        // So compare the host's hash of its first N fields against ours,
        // where N is how many fields this plugin knows. Appending leaves that
        // prefix untouched, which is precisely the promise the append-only
        // rule makes; anything else moves it and the load is refused.
        if i.prefix_count <= $crate::sys::INTERFACE_FIELDS
            || *i.prefix_hashes.add($crate::sys::INTERFACE_FIELDS)
                != $crate::sys::INTERFACE_PREFIX_HASHES[$crate::sys::INTERFACE_FIELDS]
        {
            return $crate::sys::InitResult::AbiMismatch;
        }
        let mut app = $crate::ecs::App::new(iface, host);
        // A panic in `build` would unwind out of an `extern "C"` fn and abort
        // the editor. Refusing to load is the correct outcome instead.
        //
        // The catch lives behind a function in THIS crate rather than inline
        // here, because whether it can be caught at all depends on this crate's
        // `std` feature — and a `#[cfg]` written in a macro body is evaluated
        // against the *plugin's* manifest, where `std` is not a feature that
        // exists. Same reason `__plugin_scope_entry!` is its own macro.
        if !$crate::ecs::guarded_build(&$plugin, &mut app) {
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
    }};
}

/// Emit the two lang items a `#![no_std]` plugin must define itself: a global
/// allocator and a panic handler. Call it once, beside [`add!`].
///
/// ```ignore
/// #![no_std]
/// extern crate alloc;
/// renzora_plugin::no_std_runtime!();
/// ```
///
/// **The allocator is the host process's own `malloc`/`free`.** That is the
/// right choice specifically because a plugin is `dlopen`'d *into* a running
/// engine: the C runtime is already mapped and already initialised, so the
/// plugin shares one heap with everything else in the process instead of
/// carrying a second allocator. It is also what makes a `Vec` the plugin
/// allocates safe to free after the host hands it back.
///
/// **The panic handler aborts**, because there is nothing better available:
/// `no_std` on stable cannot unwind (defining `eh_personality` is nightly-only),
/// which is why such a plugin must also set `panic = "abort"` in its profile.
/// This is the panic firewall being given up — see the `std` feature in this
/// crate's manifest.
///
/// Expands to nothing when it would be wrong to emit: under `std` the standard
/// library supplies both items, and under `static_link` the plugin is compiled
/// into the host binary, which supplies them — defining them again is a
/// duplicate-lang-item error in both cases. So the macro is safe to leave in
/// place unconditionally, which is the point: a plugin's source should not have
/// to know how it is being linked.
#[macro_export]
#[cfg(any(feature = "std", feature = "static_link"))]
macro_rules! no_std_runtime {
    () => {};
}

/// The variant that actually emits. See the `std`-side stub above.
#[macro_export]
#[cfg(not(any(feature = "std", feature = "static_link")))]
macro_rules! no_std_runtime {
    () => {
        // Both items are suppressed under `cfg(test)`, and that is what makes a
        // `no_std` plugin testable at all.
        //
        // `cargo test` compiles the plugin crate with `--test`, which links the
        // harness and therefore `std`. `std` already defines `panic_impl` and
        // installs a global allocator, so emitting ours produced
        // `E0152: found duplicate lang item` and the crate would not build —
        // meaning `cargo test` failed for all 59 `no_std` plugins whether or not
        // they had a single test in them. Gating on `test` hands those
        // responsibilities back to `std` for the test binary only; the cdylib
        // that actually ships is never built with `--test` and is unaffected.
        #[cfg(not(test))]
        #[global_allocator]
        static __RENZORA_HEAP: $crate::no_std_heap::HostHeap = $crate::no_std_heap::HostHeap;

        /// Aborts. A `no_std` plugin cannot unwind, so there is no way to turn
        /// this into the `SystemStatus::Panicked` a `std` plugin would report.
        #[cfg(not(test))]
        #[panic_handler]
        fn __renzora_panic(_: &::core::panic::PanicInfo) -> ! {
            $crate::no_std_heap::abort()
        }
    };
}
