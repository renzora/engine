//! Bevy thumbnail cache for the bevy_ui marketplace panels: downloads images
//! from URLs on background threads (ureq + image, like the egui `ImageCache`)
//! and registers them as bevy `Image` assets for display in `ImageNode`s.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use crossbeam_channel::{unbounded, Receiver, Sender};

struct Downloaded {
    url: String,
    rgba: Vec<u8>,
    width: u32,
    height: u32,
    /// A small, gaussian-blurred copy of the same image, used as the soft
    /// backdrop behind cards / hero slides (upscaled by a `Stretch` `ImageNode`).
    blur_rgba: Vec<u8>,
    blur_width: u32,
    blur_height: u32,
}

/// Async URL → `Handle<Image>` cache. `request` starts a background download;
/// `poll_thumbs` registers finished images each frame; `get` returns the handle.
#[derive(Resource)]
pub(crate) struct HubThumbs {
    handles: HashMap<String, Handle<Image>>,
    /// Blurred backdrop variant of each loaded image, keyed by the same URL.
    blurred: HashMap<String, Handle<Image>>,
    in_flight: HashSet<String>,
    failed: HashSet<String>,
    tx: Sender<Result<Downloaded, String>>,
    rx: Receiver<Result<Downloaded, String>>,
}

impl Default for HubThumbs {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Self {
            handles: HashMap::new(),
            blurred: HashMap::new(),
            in_flight: HashSet::new(),
            failed: HashSet::new(),
            tx,
            rx,
        }
    }
}

impl HubThumbs {
    /// The loaded handle for `url`, or `None` if not ready / failed.
    pub fn get(&self, url: &str) -> Option<Handle<Image>> {
        self.handles.get(url).cloned()
    }

    /// The blurred backdrop handle for `url`, or `None` if not ready / failed.
    pub fn get_blurred(&self, url: &str) -> Option<Handle<Image>> {
        self.blurred.get(url).cloned()
    }

    /// Start downloading `url` if not already loaded / in flight / failed.
    pub fn request(&mut self, url: &str) {
        if self.handles.contains_key(url) || self.in_flight.contains(url) || self.failed.contains(url) {
            return;
        }
        self.in_flight.insert(url.to_string());
        start_download(url.to_string(), self.tx.clone());
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn start_download(url: String, tx: Sender<Result<Downloaded, String>>) {
    use std::io::Read;
    std::thread::spawn(move || {
        let result = (|| -> Result<Downloaded, String> {
            let response = ureq::get(&url).call().map_err(|_| url.clone())?;
            let mut bytes = Vec::new();
            response
                .into_body()
                .into_reader()
                .take(10 * 1024 * 1024)
                .read_to_end(&mut bytes)
                .map_err(|_| url.clone())?;
            let img = image::load_from_memory(&bytes).map_err(|_| url.clone())?;
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            // Blurred backdrop: downscale (cheap) then gaussian-blur, so the
            // `Stretch` ImageNode upscales it into a soft gradient. Small source
            // dimensions keep the blur fast regardless of thumbnail size.
            let bw = 72u32.min(width).max(1);
            let bh = ((bw as f32 * height as f32 / width.max(1) as f32).round() as u32).max(1);
            let small = image::imageops::resize(&rgba, bw, bh, image::imageops::FilterType::Triangle);
            let blurred = image::imageops::blur(&small, 4.0);
            let (blur_width, blur_height) = blurred.dimensions();
            Ok(Downloaded {
                url: url.clone(),
                rgba: rgba.into_raw(),
                width,
                height,
                blur_rgba: blurred.into_raw(),
                blur_width,
                blur_height,
            })
        })();
        let _ = tx.send(result);
    });
}

#[cfg(target_arch = "wasm32")]
fn start_download(_url: String, _tx: Sender<Result<Downloaded, String>>) {}

/// Drain finished downloads, register them as `Image` assets.
pub(crate) fn poll_thumbs(mut thumbs: ResMut<HubThumbs>, mut images: ResMut<Assets<Image>>) {
    let mut done = Vec::new();
    while let Ok(res) = thumbs.rx.try_recv() {
        done.push(res);
    }
    for res in done {
        match res {
            Ok(d) => {
                thumbs.in_flight.remove(&d.url);
                let image = Image::new(
                    Extent3d { width: d.width, height: d.height, depth_or_array_layers: 1 },
                    TextureDimension::D2,
                    d.rgba,
                    TextureFormat::Rgba8UnormSrgb,
                    default(),
                );
                let handle = images.add(image);
                let blur = Image::new(
                    Extent3d { width: d.blur_width, height: d.blur_height, depth_or_array_layers: 1 },
                    TextureDimension::D2,
                    d.blur_rgba,
                    TextureFormat::Rgba8UnormSrgb,
                    default(),
                );
                let blur_handle = images.add(blur);
                thumbs.handles.insert(d.url.clone(), handle);
                thumbs.blurred.insert(d.url, blur_handle);
            }
            Err(url) => {
                thumbs.in_flight.remove(&url);
                thumbs.failed.insert(url);
            }
        }
    }
}
