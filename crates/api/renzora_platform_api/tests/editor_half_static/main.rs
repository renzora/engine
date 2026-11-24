//! `plugin!(.., Runtime)` with an editor folder, in a build that compiles
//! plugins INTO the binary, as a lean export does: the editor half must not be
//! compiled at all.
//!
//! `editor.rs` beside this file is a `compile_error!`, so this test passing is
//! the proof that a game never reads it. Run with
//! `cargo test -p renzora_platform_api --features static_plugins --test editor_half_static`.
#![cfg(feature = "static_plugins")]

use bevy::prelude::*;

#[derive(Default)]
pub struct SamplePlugin;

// Inline editor code is gone in this build, items and statements alike: both
// would fail to compile if they were not.
renzora_platform_api::editor! {
    compile_error!("inline editor! item compiled into a game build");
}

impl Plugin for SamplePlugin {
    fn build(&self, _app: &mut App) {
        renzora_platform_api::editor! { compile_error!("inline editor! statement compiled into a game build"); }
    }
}

renzora_platform_api::plugin!(SamplePlugin, Runtime);

/// The plugin type is all a compiled-in build installs, and it still builds.
#[test]
fn the_plugin_builds_without_its_editor_half() {
    let mut app = App::new();
    app.add_plugins(SamplePlugin);
}
