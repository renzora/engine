//! End-to-end: an inspector-style attribute edit writes back to the
//! source `.html` on disk and the in-memory span cache stays coherent.
//!
//! This test exercises the full writeback path the inspector will hit:
//! - load a real file through `AssetServer`
//! - mark an entity with `MarkupSource`
//! - call `writeback::write_attr_to_markup`
//! - assert the on-disk file mutated and only the targeted bytes changed.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_hui::prelude::{HtmlTemplate, LoaderPlugin};
use renzora_ember::markup::provenance::MarkupSource;
use renzora_ember::markup::writeback::write_attr_to_markup;

/// Drive the app until the asset finishes loading (or we time out). Bevy's
/// AssetServer is async; without ticking the app the load future never
/// completes.
///
/// The budget is wall-clock time, not a frame count. `MinimalPlugins` frames
/// cost microseconds, so 200 of them were over in about 0.2 s, and on a slow CI
/// runner the file read on the IO pool had not finished by then. The short
/// sleep hands that pool the CPU instead of spinning against it.
fn pump_until_loaded(app: &mut App, handle: &Handle<HtmlTemplate>) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        app.update();
        let templates = app.world().resource::<Assets<HtmlTemplate>>();
        if templates.get(handle).is_some() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("HtmlTemplate did not load within 10 s");
}

#[test]
fn inspector_edit_patches_source_file() {
    // Stage a real .html under a tempdir so AssetServer can read it through
    // its standard file source.
    let tmp = tempfile::tempdir().expect("tempdir");
    let asset_root = tmp.path();
    let rel_path = "ui/test_inspector.html";
    let disk_path = asset_root.join(rel_path);
    std::fs::create_dir_all(disk_path.parent().unwrap()).unwrap();

    // Initial markup mirrors what the user pasted into chat: a `<text>` with
    // two style attributes and a literal body. The writeback target is the
    // `font_size` value `"12"`.
    let source = r##"<template>
    <text font_size="12" font_color="#8A93A2">#</text>
</template>"##;
    std::fs::write(&disk_path, source).expect("write fixture");

    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(AssetPlugin {
        file_path: asset_root.to_string_lossy().to_string(),
        ..default()
    });
    app.add_plugins(LoaderPlugin);

    // The writeback helper resolves the disk path via
    // `CurrentProject::path.join(asset_path)`. Pointing project root at the
    // same tempdir matches how the editor wires this up at runtime.
    app.insert_resource(renzora::core::CurrentProject {
        path: PathBuf::from(asset_root),
        config: Default::default(),
    });

    let handle = app
        .world()
        .resource::<AssetServer>()
        .load::<HtmlTemplate>(rel_path);
    pump_until_loaded(&mut app, &handle);

    // Spawn a stand-in entity for "the `<text>` node the user clicked".
    // The real loader would stamp this via `apply_xnode_to`; we shortcut it
    // here so the test stays focused on the writeback path.
    let entity = app
        .world_mut()
        .spawn(MarkupSource {
            template_handle: handle.clone(),
            node_path: Vec::new(),
        })
        .id();

    // Edit: font_size 12 → 14.
    write_attr_to_markup(app.world_mut(), entity, "font_size", "14");

    // On-disk file changed exactly at the patched span; nothing else moved.
    let after = std::fs::read_to_string(&disk_path).expect("read after");
    let expected = source.replacen("font_size=\"12\"", "font_size=\"14\"", 1);
    assert_eq!(after, expected, "writeback did not patch in place");

    // The in-memory cache is also coherent — a second edit must hit the
    // freshly-shifted span (the value now reads "14", which is the same
    // length as "12", so shifts are zero, but the loop still exercises the
    // post-edit span lookup).
    write_attr_to_markup(app.world_mut(), entity, "font_size", "20");
    let after2 = std::fs::read_to_string(&disk_path).expect("read after2");
    assert!(after2.contains("font_size=\"20\""));

    // Brand-new attribute insertion: `border_radius` wasn't in the source.
    write_attr_to_markup(app.world_mut(), entity, "border_radius", "8px");
    let after3 = std::fs::read_to_string(&disk_path).expect("read after3");
    assert!(
        after3.contains("border_radius=\"8px\""),
        "new attribute should be inserted; got:\n{after3}"
    );
}
