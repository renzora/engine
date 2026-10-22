//! The loader through a real `AssetServer`, over files on disk.
//!
//! The unit tests in `raster` cover rasterizing from bytes; these cover what
//! only the asset pipeline can show: that `<image href>` files and the sidecar
//! are read through the asset reader (not `std::fs` relative to the process),
//! that a refused path never reaches the reader, that a broken file marks the
//! handle failed instead of panicking the task, and that editing an embedded
//! PNG re-rasterizes the SVG, which is what proves it was registered as a
//! loader dependency.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bevy::asset::{AssetPlugin, LoadState};
use bevy::prelude::*;
use resvg::tiny_skia;

/// A fresh, empty folder per test: tests run in parallel in one process.
fn temp_root(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("renzora_svg_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(root: &Path, rel: &str, bytes: &[u8]) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn png(rgba: [u8; 4]) -> Vec<u8> {
    let mut p = tiny_skia::Pixmap::new(4, 4).unwrap();
    p.fill(tiny_skia::Color::from_rgba8(rgba[0], rgba[1], rgba[2], rgba[3]));
    p.encode_png().unwrap()
}

/// `MinimalPlugins` + assets + the SVG loader. `Image` is registered by hand
/// rather than through `ImagePlugin`, which would pull renderer resources a
/// headless test has no use for.
fn app(root: &Path, watch: bool) -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin {
            file_path: root.to_string_lossy().into_owned(),
            watch_for_changes_override: Some(watch),
            ..default()
        },
    ));
    app.init_asset::<Image>();
    app.add_plugins(renzora_svg::SvgPlugin);
    app.finish();
    app
}

/// Pump frames until `cond` holds or `timeout` passes.
fn pump_until(app: &mut App, timeout: Duration, cond: impl Fn(&App) -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        app.update();
        if cond(app) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

fn state(app: &App, h: &Handle<Image>) -> Option<LoadState> {
    app.world().resource::<AssetServer>().get_load_state(h)
}

fn pixel(app: &App, h: &Handle<Image>, x: u32, y: u32) -> Option<[u8; 4]> {
    let image = app.world().resource::<Assets<Image>>().get(h)?;
    let w = image.texture_descriptor.size.width;
    let i = ((y * w + x) * 4) as usize;
    let d = image.data.as_ref()?;
    Some([d[i], d[i + 1], d[i + 2], d[i + 3]])
}

const WAIT: Duration = Duration::from_secs(20);

const EMBEDS_RELATIVE_PNG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4">
    <image href="img/c.png" x="0" y="0" width="4" height="4"/></svg>"#;

#[test]
fn relative_png_is_read_through_the_asset_reader() {
    let root = temp_root("relative");
    write(&root, "ui/icon.svg", EMBEDS_RELATIVE_PNG.as_bytes());
    write(&root, "ui/img/c.png", &png([0, 255, 0, 255]));

    let mut app = app(&root, false);
    let h: Handle<Image> = app.world().resource::<AssetServer>().load("ui/icon.svg");
    assert!(pump_until(&mut app, WAIT, |a| matches!(state(a, &h), Some(LoadState::Loaded))));
    assert_eq!(pixel(&app, &h, 1, 1), Some([0, 255, 0, 255]));
}

#[test]
fn sidecar_changes_the_size() {
    let root = temp_root("sidecar");
    write(&root, "icon.svg", EMBEDS_RELATIVE_PNG.as_bytes());
    write(&root, "img/c.png", &png([0, 255, 0, 255]));
    write(&root, "icon.svg.toml", b"width = 32\n");

    let mut app = app(&root, false);
    let h: Handle<Image> = app.world().resource::<AssetServer>().load("icon.svg");
    assert!(pump_until(&mut app, WAIT, |a| matches!(state(a, &h), Some(LoadState::Loaded))));
    let images = app.world().resource::<Assets<Image>>();
    let size = images.get(&h).unwrap().texture_descriptor.size;
    assert_eq!((size.width, size.height), (32, 32));
}

#[test]
fn a_path_escaping_the_root_is_refused() {
    // The "secret" sits beside the asset root, so `../` from a top-level SVG
    // would reach it if the check were missing.
    let outer = temp_root("escape");
    let root = outer.join("project");
    std::fs::create_dir_all(&root).unwrap();
    write(&outer, "secret.png", &png([255, 0, 0, 255]));
    write(
        &root,
        "evil.svg",
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4">
            <image href="../secret.png" x="0" y="0" width="4" height="4"/></svg>"#,
    );

    let mut app = app(&root, false);
    let h: Handle<Image> = app.world().resource::<AssetServer>().load("evil.svg");
    assert!(pump_until(&mut app, WAIT, |a| matches!(state(a, &h), Some(LoadState::Loaded))));
    // Loaded, but without the refused image: the pixel is transparent.
    assert_eq!(pixel(&app, &h, 1, 1).map(|p| p[3]), Some(0));
}

#[test]
fn a_malformed_file_fails_the_load() {
    let root = temp_root("malformed");
    write(&root, "broken.svg", b"<svg xmlns=\"http://www.w3.org/2000/svg\"><g></svg>");

    let mut app = app(&root, false);
    let h: Handle<Image> = app.world().resource::<AssetServer>().load("broken.svg");
    assert!(pump_until(&mut app, WAIT, |a| matches!(state(a, &h), Some(LoadState::Failed(_)))));
}

#[test]
fn editing_an_embedded_png_re_rasterizes_the_svg() {
    let root = temp_root("hot_reload");
    write(&root, "icon.svg", EMBEDS_RELATIVE_PNG.as_bytes());
    write(&root, "img/c.png", &png([0, 255, 0, 255]));

    let mut app = app(&root, true);
    let h: Handle<Image> = app.world().resource::<AssetServer>().load("icon.svg");
    assert!(pump_until(&mut app, WAIT, |a| pixel(a, &h, 1, 1) == Some([0, 255, 0, 255])));

    // The watcher needs a moment to be armed before the write it should see.
    std::thread::sleep(Duration::from_millis(500));
    write(&root, "img/c.png", &png([0, 0, 255, 255]));
    assert!(
        pump_until(&mut app, WAIT, |a| pixel(a, &h, 1, 1) == Some([0, 0, 255, 255])),
        "the SVG was not reloaded after its embedded PNG changed"
    );
}
