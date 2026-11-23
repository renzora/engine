//! The legacy plugin declaration.
//!
//! Engine crates declare their plugins in their own `Cargo.toml`:
//!
//! ```toml
//! [[package.metadata.renzora.plugin]]
//! type = "MyPlugin"      # path from the crate root
//! scope = "Editor"       # optional, Runtime by default
//! priority = -100        # optional, 0 by default
//! ```
//!
//! `cargo renzora sync` reads those tables and generates the dependency edge
//! that links each crate and the lists that install its plugins (see
//! `xtask/src/sync.rs`).
//!
//! The older form, a one-line `renzora::add!(MyPlugin, Editor);` in the source,
//! is still read by the generator, because the `renzora add` scaffolder writes
//! one. This macro exists only so that line compiles. It emits no registration
//! code: it checks that the named type is a `Plugin` with a `Default` impl, and
//! that is all. Native plugins installed into `plugins/` use `plugin!` instead,
//! which lives in `renzora_platform_api`.

/// Declare a Bevy plugin in source, the legacy way; prefer a
/// `[[package.metadata.renzora.plugin]]` table in the crate's `Cargo.toml`.
///
/// See the [module docs](self). The line must be on its own, at the top level
/// of the file, for the generator to read it.
#[macro_export]
macro_rules! add {
    ($plugin_type:ty) => {
        $crate::add!($plugin_type, Runtime, priority = 0);
    };
    ($plugin_type:ty, $scope:ident) => {
        $crate::add!($plugin_type, $scope, priority = 0);
    };
    ($plugin_type:ty, $scope:ident, priority = $priority:expr) => {
        // Type check only: the generated list is what installs the plugin.
        const _: fn() = || {
            fn assert_declarable<T: $crate::bevy::app::Plugin + ::std::default::Default>() {}
            assert_declarable::<$plugin_type>();
        };
    };
}
