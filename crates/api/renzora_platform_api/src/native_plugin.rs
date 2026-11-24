//! Native plugins: what a plugin installed into `<exe dir>/plugins/` emits so
//! the engine can find it, and what the loader reads back off the file.
//!
//! Moved here from `renzora`'s `plugin_meta`, beside the plugin inventory and
//! updates that describe the same plugins on disk. Plugins still write
//! `renzora::plugin!`, through `renzora_sdk`.
//!
//! Engine crates compiled INTO the binary are not native plugins and use none
//! of this: they declare themselves in their own `Cargo.toml` with a
//! `[[package.metadata.renzora.plugin]]` table, which `cargo renzora sync`
//! reads (see `xtask/src/sync.rs`).

/// Declare a **native plugin**: a Bevy plugin shipped as Rust source and
/// compiled on the machine that installs it.
///
/// It is installed into `<exe dir>/plugins/` and rebuilt whenever the engine
/// moves under it. Like the crates compiled into the engine, it is an ordinary
/// Bevy plugin with full `&mut World` access; the difference is only in how it
/// is built and where it can run.
///
/// ```ignore
/// use bevy::prelude::*;
///
/// pub struct SpinThing;
/// impl Plugin for SpinThing {
///     fn build(&self, app: &mut App) { app.add_systems(Update, spin); }
/// }
///
/// renzora::plugin!(SpinThing);
/// ```
///
/// Takes an expression, not just a type, so a plugin needing configuration can
/// write `renzora::plugin!(SpinThing::new(4))` rather than contorting itself
/// into a `Default` impl.
///
/// # Why this exists rather than the four lines it expands to
///
/// The loader finds a plugin by asking the OS for one symbol *by name*. Every
/// way of writing that by hand fails identically: the library loads, the
/// symbol is absent, and it is **skipped in silence**, because a library
/// without the entry point is not an error, it is simply not a plugin:
///
/// * a typo in `renzora_native_plugin_ctor`
/// * a missing `#[unsafe(no_mangle)]`, which leaves the symbol mangled with an
///   unpredictable hash
/// * a wrong return type, which is worse than silent: the loader calls it as
///   `fn() -> Box<dyn Plugin>` regardless and reads whatever is in the return
///   register as a boxed trait object
///
/// The macro fixes the name and the signature, so none of the three is reachable.
///
/// # Scope
///
/// `renzora::plugin!(MyThing, Runtime)` declares a plugin that also belongs in a
/// shipped game; `Editor` (the default) keeps it to the editor.
///
/// A runtime native plugin works for the same reason the editor's does: an
/// exported game built by the copy-based modes ships the very `bevy_dylib` and
/// `renzora_dylib` the plugin was compiled against, so the `World` on both sides
/// of the boundary is one type. It is the *lean* export that cannot host one:
/// that binary links Bevy statically and shares no image, which is the same
/// reason a Rust script has to be compiled into it instead.
///
/// `Editor` is the default because that is what every native plugin written so
/// far is, and because the cost of guessing wrong points the safe way: an
/// editor-scoped plugin merely does not appear in a game, while a
/// runtime-scoped one that should not have shipped is in the player's hands.
///
/// # The editor half
///
/// Editor surface (an inspector section, a preset, a panel, a tool) lives in
/// **`src/editor.rs`** or **`src/editor/mod.rs`**, with as many modules below it
/// as it needs, and one entry point, `pub(crate) fn register(app: &mut App)`.
/// The plugin does not declare that module and says nothing about it here:
///
/// ```ignore
/// renzora::plugin!(VignettePlugin, Runtime);
/// ```
///
/// The macro owns it. Where the plugin is loaded from a library (the editor, a
/// copy-based export) it declares `mod editor;` and runs `editor::register`
/// right after the plugin's own `build`. Where the plugin is compiled INTO a
/// game (a lean export) it declares nothing, so the folder is never read and
/// the game carries none of it: a lean export has no editor contract, and code
/// naming `AppEditorExt` or `InspectorEntry` would not even compile there.
///
/// Whether the folder exists is decided by the plugin build, which can look at
/// the disk where a macro cannot: the driver passes `--cfg renzora_no_editor`
/// when there is neither file. The cfg is the "no" so that tools passing none
/// of our flags, rust-analyzer above all, see the editor module and analyse
/// it; a plugin with no editor folder then shows a missing-module diagnostic
/// there, beside the `renzora_build` one, and builds normally.
///
/// A cargo feature could not do this: a native plugin is compiled with no
/// features of its own, so `#[cfg(feature = "editor")]` in it is always false.
#[macro_export]
macro_rules! plugin {
    ($plugin:expr) => {
        $crate::plugin!($plugin, Editor);
    };
    ($plugin:expr, $scope:ident) => {
        $crate::__native_plugin_entry!($plugin, $scope);
    };
}

/// Editor-only code written inline, wherever it sits in a plugin.
///
/// The small-scale form of a plugin's editor folder (see [`plugin!`]): what is
/// inside is compiled where the plugin is loaded beside the editor and left out
/// of a lean game build, as if it were in `src/editor.rs`. Items or statements:
///
/// ```ignore
/// renzora::editor! {
///     use renzora::{AppEditorExt, InspectorEntry};
///
///     fn inspector_entry() -> InspectorEntry { /* ... */ }
/// }
///
/// impl Plugin for VignettePlugin {
///     fn build(&self, app: &mut App) {
///         app.add_systems(Update, sync_vignette);
///         renzora::editor! { app.register_inspector(inspector_entry()); }
///     }
/// }
/// ```
///
/// The folder is the better home for anything larger than a few lines; this is
/// for the line or two that reads better where it is. Imports used only inside
/// a block go inside one too, since a game build has no editor contract for a
/// `use` outside to name.
///
/// Switched by the same thing as `plugin!`'s editor half, and for the same
/// reason a `cfg(feature)` in the plugin could not be: `static_plugins`, on
/// this crate, is on exactly in a build that compiles plugins into a game.
#[cfg(not(feature = "static_plugins"))]
#[macro_export]
macro_rules! editor {
    ($($code:tt)*) => { $($code)* };
}

/// [`editor!`] in a build that compiles plugins into a game: nothing at all.
#[doc(hidden)]
#[cfg(feature = "static_plugins")]
#[macro_export]
macro_rules! editor {
    ($($code:tt)*) => {};
}

/// A native plugin together with its editor half, installed as one plugin.
///
/// What [`plugin!`] hands the loader when the plugin has an editor folder: `build` runs the plugin's
/// own `build`, then the `register` function from its `src/editor.rs`. Every
/// other hook forwards untouched, and `name` is the inner plugin's, so Bevy's
/// duplicate check and the logs see the plugin the author wrote, not a wrapper.
pub struct WithEditor<P> {
    plugin: P,
    register: fn(&mut crate::bevy::app::App),
}

impl<P> WithEditor<P> {
    pub fn new(plugin: P, register: fn(&mut crate::bevy::app::App)) -> Self {
        Self { plugin, register }
    }
}

impl<P: crate::bevy::app::Plugin> crate::bevy::app::Plugin for WithEditor<P> {
    fn build(&self, app: &mut crate::bevy::app::App) {
        self.plugin.build(app);
        (self.register)(app);
    }
    fn ready(&self, app: &crate::bevy::app::App) -> bool {
        self.plugin.ready(app)
    }
    fn finish(&self, app: &mut crate::bevy::app::App) {
        self.plugin.finish(app);
    }
    fn cleanup(&self, app: &mut crate::bevy::app::App) {
        self.plugin.cleanup(app);
    }
    fn name(&self) -> &str {
        self.plugin.name()
    }
    fn is_unique(&self) -> bool {
        self.plugin.is_unique()
    }
}

/// The symbols [`plugin!`] emits, or nothing when the plugin is being compiled
/// INTO a binary rather than loaded from one.
///
/// Split out for the same reason `__script_entry!` is: a
/// `#[cfg(feature = ...)]` written inside `plugin!`'s expansion would be
/// evaluated when the **plugin** is compiled, against the plugin's own manifest,
/// where `static_plugins` does not exist and never will. Defining the two
/// variants here evaluates the cfg where the feature lives, on this crate,
/// which cargo compiles once per build and whose features are unified across it.
///
/// Both symbols are `#[no_mangle]`, so linking fifty plugins into one binary
/// defines each of them fifty times and the link fails. The lean exporter turns
/// this feature on and calls `add_plugins` on the plugin type directly, which
/// needs no symbol at all.
#[doc(hidden)]
#[cfg(not(feature = "static_plugins"))]
#[macro_export]
macro_rules! __native_plugin_entry {
    ($plugin:expr, $scope:ident) => {
        // A native plugin is compiled by the Renzora driver against the SDK,
        // which passes `--cfg renzora_build`. Anything else compiling this
        // variant is a `cargo build` (or `cargo run`, `cargo test`) in the
        // plugin's own folder: cargo would resolve the manifest's `bevy` and
        // build a second Bevy, whose `World` is a different type from the
        // engine's, so it is refused here with the reason instead. The lean
        // export compiles the other variant (`static_plugins`), which needs no
        // flag. rust-analyzer shows this one diagnostic and otherwise works.
        #[cfg(not(renzora_build))]
        ::core::compile_error!(
            "this plugin is built by the Renzora editor against its own Bevy; \
             `cargo build` here would build a second Bevy. Open the project in the \
             editor, which compiles installed plugins itself (`cargo renzora` does \
             too, from an engine checkout). The `bevy`/`renzora` entries in \
             Cargo.toml are for your IDE only."
        );

        // The editor half, when the plugin has one (see `plugin!`). Loaded from
        // a library, so the editor contract is in the SDK and it compiles.
        #[cfg(not(renzora_no_editor))]
        mod editor;

        /// The one symbol the loader looks up. Unmangled so it can be found by
        /// string; see `plugin!` for why hand-writing this is a trap.
        #[cfg(not(renzora_no_editor))]
        #[unsafe(no_mangle)]
        pub fn renzora_native_plugin_ctor() -> ::std::boxed::Box<dyn $crate::bevy::app::Plugin> {
            ::std::boxed::Box::new($crate::WithEditor::new($plugin, editor::register))
        }

        /// The same symbol for a plugin with no editor folder.
        #[cfg(renzora_no_editor)]
        #[unsafe(no_mangle)]
        pub fn renzora_native_plugin_ctor() -> ::std::boxed::Box<dyn $crate::bevy::app::Plugin> {
            ::std::boxed::Box::new($plugin)
        }

        /// Where this plugin may load, as a byte the loader reads by symbol.
        ///
        /// A plain `u8` rather than an enum, because this crosses a `dlopen`
        /// boundary: a `#[repr(Rust)]` enum has no guaranteed layout, and the
        /// loader would be reading whichever bytes the compiler happened to
        /// choose. The absence of the symbol reads as `Editor`, so a plugin
        /// built before this existed keeps its old behaviour rather than
        /// silently appearing in someone's game.
        #[unsafe(no_mangle)]
        pub extern "C" fn renzora_native_plugin_scope() -> u8 {
            $crate::NativePluginScope::$scope as u8
        }

        /// Which engine build this plugin binds to, written into the image so
        /// the loader can read it off the file before mapping it.
        ///
        /// `RENZORA_PLUGIN_ABI` is set by the compiler driver to the SDK stamp
        /// the plugin is being compiled against; it is evaluated here, in the
        /// plugin's own compilation, which is the only place that knows the
        /// answer. A plugin built by a bare `rustc` with the variable unset
        /// records an empty stamp, which no engine build matches, so it is
        /// refused rather than guessed at.
        ///
        /// A `static` of bytes rather than a function: the loader must not map
        /// an image to find out whether mapping it is sound (an incompatible
        /// image runs its initialisers against the wrong contract crate the
        /// moment it is opened), so the record is found by a byte search over
        /// the file, the same way the constructor symbol is. `#[used]` keeps it
        /// in the object and `no_mangle` keeps the linker from discarding it.
        #[used]
        #[unsafe(no_mangle)]
        // Named like the exported symbol beside it, not like a Rust constant.
        #[allow(non_upper_case_globals)]
        pub static renzora_native_plugin_abi: [u8; $crate::NATIVE_PLUGIN_ABI_RECORD_LEN] =
            $crate::native_plugin_abi_record(match ::core::option_env!("RENZORA_PLUGIN_ABI") {
                ::core::option::Option::Some(stamp) => stamp,
                ::core::option::Option::None => "",
            });
    };
}

/// [`__native_plugin_entry!`] for a build that links its plugins in.
///
/// Emits nothing. The plugin's type is defined by the author's own code and is
/// all the generated aggregator needs; the scope was decided at export time, by
/// reading it from the built library rather than from this declaration.
///
/// No `mod editor;` either, and that is the point of it: a game compiled with
/// this plugin in it never reads its editor folder, and the aggregator installs
/// the plugin type alone. No `renzora_build` guard: the lean exporter is the
/// only thing that turns `static_plugins` on.
#[doc(hidden)]
#[cfg(feature = "static_plugins")]
#[macro_export]
macro_rules! __native_plugin_entry {
    ($plugin:expr, $scope:ident) => {};
}

/// Where a native plugin is allowed to load.
///
/// `#[repr(u8)]` because it crosses a boundary: the loader reads it from the
/// byte a built library's `renzora_native_plugin_scope` symbol returns, so the
/// discriminants are part of what a compiled plugin agrees with the engine on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum NativePluginScope {
    /// Editor only. The default, and what every native plugin was before scopes
    /// existed.
    #[default]
    Editor = 0,
    /// Also loaded by a shipped game; see [`plugin!`] for what makes that sound.
    Runtime = 1,
}

impl NativePluginScope {
    /// Decode the byte the `renzora_native_plugin_scope` symbol returns.
    ///
    /// Anything unrecognised is `Editor`: a plugin built against a newer engine
    /// that grew a third scope should stay out of a game this one ships, rather
    /// than being admitted on the strength of a byte this build cannot read.
    pub fn from_byte(b: u8) -> Self {
        match b {
            1 => Self::Runtime,
            _ => Self::Editor,
        }
    }
}

/// The bytes that open a native plugin's ABI record. The loader searches a
/// library's bytes for them and reads the stamp that follows, up to the first
/// NUL.
///
/// Distinctive on purpose: the search runs over the whole file, and the only
/// thing stopping it from matching some other string is that nothing else
/// looks like this.
pub const NATIVE_PLUGIN_ABI_MARKER: &[u8] = b"RENZORA_NATIVE_PLUGIN_ABI=";

/// Size of the record [`plugin!`] embeds: the marker, the stamp, and at least
/// one NUL.
///
/// Fixed because the record is a `static` array whose length has to be known
/// where the macro expands, and a `const fn` cannot return a slice of a length
/// it computes. A stamp is `<16 hex>+rustc-<version>`, under 40 bytes, and the
/// older filename form is under 70, so this leaves room without wasting much.
/// A stamp that does not fit is a compile error in the plugin, not a truncated
/// record that the loader would then read as a different build.
pub const NATIVE_PLUGIN_ABI_RECORD_LEN: usize = 128;

/// Build the ABI record [`plugin!`] embeds: [`NATIVE_PLUGIN_ABI_MARKER`], then
/// `stamp`, then NUL padding to [`NATIVE_PLUGIN_ABI_RECORD_LEN`].
///
/// `const` so it runs in the plugin's compilation and lands in its read-only
/// data as one contiguous array. Two adjacent string literals would not do:
/// Rust string data carries no terminator, so a marker literal followed by a
/// stamp literal is only adjacent in the file if the compiler happens to lay
/// them out that way, and whatever string came next would read as part of the
/// stamp. Writing the bytes into one array settles both the adjacency and the
/// terminator.
///
/// Panics at compile time if the stamp does not fit; see the length constant.
pub const fn native_plugin_abi_record(stamp: &str) -> [u8; NATIVE_PLUGIN_ABI_RECORD_LEN] {
    let mut out = [0u8; NATIVE_PLUGIN_ABI_RECORD_LEN];
    let marker = NATIVE_PLUGIN_ABI_MARKER;
    let stamp = stamp.as_bytes();
    assert!(
        marker.len() + stamp.len() < NATIVE_PLUGIN_ABI_RECORD_LEN,
        "RENZORA_PLUGIN_ABI is too long for the native plugin ABI record"
    );
    let mut i = 0;
    while i < marker.len() {
        out[i] = marker[i];
        i += 1;
    }
    let mut j = 0;
    while j < stamp.len() {
        out[marker.len() + j] = stamp[j];
        j += 1;
    }
    out
}

#[cfg(test)]
mod abi_record_tests {
    use super::*;

    /// The record is marker, stamp, NUL, in that order, contiguous, and
    /// terminated. The loader's parser depends on each of those.
    #[test]
    fn record_is_marker_then_stamp_then_nul() {
        let record = native_plugin_abi_record("0123456789abcdef+rustc-1.97.1");
        let mut expected = NATIVE_PLUGIN_ABI_MARKER.to_vec();
        expected.extend_from_slice(b"0123456789abcdef+rustc-1.97.1");
        assert_eq!(&record[..expected.len()], &expected[..]);
        assert!(
            record[expected.len()..].iter().all(|&b| b == 0),
            "NUL padded to the end"
        );
    }

    /// An empty stamp still writes the marker, so the loader can tell "built
    /// without a stamp" from "not a plugin at all" and say which.
    #[test]
    fn an_empty_stamp_keeps_the_marker() {
        let record = native_plugin_abi_record("");
        assert_eq!(
            &record[..NATIVE_PLUGIN_ABI_MARKER.len()],
            NATIVE_PLUGIN_ABI_MARKER
        );
        assert_eq!(record[NATIVE_PLUGIN_ABI_MARKER.len()], 0);
    }
}
