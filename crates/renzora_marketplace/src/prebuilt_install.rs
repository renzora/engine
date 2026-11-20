//! Installing a plugin's prebuilt binary instead of compiling its source.
//!
//! An official nightly or release finds, on the marketplace, a binary CI
//! compiled for exactly its build, and installs that: no SDK to unpack, no Rust
//! toolchain, no compile. Everything else (a source build of the engine, a
//! plugin CI does not build, a binary that fails any check) installs from source
//! exactly as before, which is why this returns a reason rather than an error
//! when it does not install.
//!
//! The fetching and checking are `renzora_native_plugin::prebuilt_fetch`, shared
//! with the launch-time pass that refreshes a plugin's binary when the engine
//! moves to a new build. The source is always installed as well: the lean
//! exporter compiles plugins into a game from source, and when no binary exists
//! for a new build the loader rebuilds from it.

use std::path::Path;

/// What happened when a prebuilt was tried.
pub enum Outcome {
    /// The binary is in place and the loader will take it as built.
    Installed { version: String, engine_tag: String },
    /// Not installed, and why, in a phrase that reads after "because".
    Unavailable(String),
}

/// Try to install the prebuilt binary of marketplace asset `asset_id` into
/// `plugin_dir`, the directory its source was just installed into.
pub fn install(
    token: Option<&str>,
    asset_id: &str,
    plugin_dir: &Path,
    on_bytes: &mut dyn FnMut(u64),
) -> Outcome {
    let Some(root) = renzora_native_build::install::root() else {
        return Outcome::Unavailable("the engine's install directory is unknown".into());
    };
    // `None` on a build that recorded no stamp and has no SDK, which no
    // prebuilt can be matched against.
    let Some(stamp) = renzora_native_plugin::expected_stamp(&root) else {
        return Outcome::Unavailable("this engine build has no build stamp".into());
    };
    match renzora_native_plugin::prebuilt_fetch::fetch_and_install(token, asset_id, plugin_dir, &stamp, on_bytes) {
        Ok(fetched) => Outcome::Installed { version: fetched.version, engine_tag: fetched.engine_tag },
        Err(why) => Outcome::Unavailable(why),
    }
}
