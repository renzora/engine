//! `plugin!(.., Runtime)` in a plugin that has an editor folder: the macro
//! declares `mod editor;` (this directory's `editor.rs`) and the constructor
//! installs the plugin with its editor half after it. The driver would pass no
//! `renzora_no_editor` here, because the file exists.
//!
//! A directory test, `tests/editor_half/main.rs`, so that `mod editor;` resolves
//! to a sibling file exactly as it does from a plugin's `src/lib.rs`.
//!
//! The compiled-in form is `editor_half_static`.
#![cfg(not(feature = "static_plugins"))]

use bevy::prelude::*;

#[derive(Resource)]
pub struct RuntimeHalfRan;

#[derive(Resource)]
pub struct InlineEditorRan;

// An item through the inline form.
renzora_platform_api::editor! {
    fn mark_inline(app: &mut App) {
        app.insert_resource(InlineEditorRan);
    }
}

#[derive(Default)]
pub struct SamplePlugin;

impl Plugin for SamplePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(RuntimeHalfRan);
        // A statement through the inline form, using a local of the caller.
        renzora_platform_api::editor! { mark_inline(app); }
    }
}

renzora_platform_api::plugin!(SamplePlugin, Runtime);

/// Both halves run, the runtime half first (the editor half reads what it
/// registered), and the plugin keeps its own name rather than the wrapper's.
#[test]
fn the_editor_half_is_installed_after_the_plugin() {
    let plugin = renzora_native_plugin_ctor();
    assert_eq!(plugin.name(), SamplePlugin.name());

    let mut app = App::new();
    plugin.build(&mut app);
    assert!(app.world().contains_resource::<RuntimeHalfRan>());
    assert!(app.world().contains_resource::<InlineEditorRan>(), "inline editor! code did not run");
    let editor = app.world().resource::<editor::EditorHalfRan>();
    assert!(editor.saw_runtime_half, "editor half ran before the plugin's build");
}

/// The scope byte is still the declared one.
#[test]
fn the_scope_survives_the_editor_form() {
    assert_eq!(
        renzora_native_plugin_scope(),
        renzora_platform_api::NativePluginScope::Runtime as u8
    );
}
