//! Images fetched from a URL, decoded off the main thread.
//!
//! The counterpart to [`file_image`](super::file_image): identical shape, but
//! the bytes come from the network rather than the filesystem. Both exist for
//! the same reason — a UI builder holds `&mut Commands` and nothing else, and a
//! reactive list rebuilds its rows whenever anything it hashes changes, so
//! anything decoded in a builder is re-decoded on every rebuild. Requesting into
//! a cache and binding the handle means a URL is fetched once per session.
//!
//! Failures are remembered too, so an avatar that 404s costs one request for the
//! whole session rather than one per frame.
//!
//! This was `MarkdownImages`, private to the markdown widget, because markdown
//! was the only thing that showed a picture from the internet. The About
//! overlay's contributor avatars are the second, and naming a general URL cache
//! after the first widget to want one is how two of them end up existing.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use crossbeam_channel::{unbounded, Receiver, Sender};

pub(crate) struct DownloadedImage {
    url: String,
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

/// Async URL → `Handle<Image>` cache. [`WebImages::request`] starts a background
/// download, [`poll_web_images`] registers what finished, and [`WebImages::get`]
/// returns the handle.
#[derive(Resource)]
pub struct WebImages {
    handles: HashMap<String, Handle<Image>>,
    in_flight: HashSet<String>,
    failed: HashSet<String>,
    tx: Sender<Result<DownloadedImage, String>>,
    rx: Receiver<Result<DownloadedImage, String>>,
}

impl Default for WebImages {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Self {
            handles: HashMap::new(),
            in_flight: HashSet::new(),
            failed: HashSet::new(),
            tx,
            rx,
        }
    }
}

impl WebImages {
    /// The loaded handle for `url`, or `None` if not ready / failed.
    pub fn get(&self, url: &str) -> Option<Handle<Image>> {
        self.handles.get(url).cloned()
    }

    /// Whether `url` failed to download or decode. Lets a caller commit to its
    /// placeholder instead of leaving an empty box in case something arrives.
    pub fn failed(&self, url: &str) -> bool {
        self.failed.contains(url)
    }

    /// Start downloading `url` if not already loaded / in flight / failed.
    pub fn request(&mut self, url: &str) {
        if self.handles.contains_key(url)
            || self.in_flight.contains(url)
            || self.failed.contains(url)
        {
            return;
        }
        self.in_flight.insert(url.to_string());
        start_download(url.to_string(), self.tx.clone());
    }
}

#[cfg(all(feature = "editor_tools", not(target_arch = "wasm32")))]
fn start_download(url: String, tx: Sender<Result<DownloadedImage, String>>) {
    std::thread::spawn(move || {
        let result = (|| -> Result<DownloadedImage, String> {
            // The 10 MiB cap is enforced by the backend as the body arrives,
            // not after: this URL came from a server, and a limit applied once
            // the bytes are already in memory protects nothing.
            let response = renzora_net::Request::get(&url)
                .max_bytes(10 * 1024 * 1024)
                .send()
                .map_err(|_| url.clone())?;
            if !response.is_ok() {
                return Err(url.clone());
            }
            let img = image::load_from_memory(&response.body).map_err(|_| url.clone())?;
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            Ok(DownloadedImage { url: url.clone(), rgba: rgba.into_raw(), width, height })
        })();
        let _ = tx.send(result);
    });
}

/// No network backend here, so every request fails and callers show their
/// placeholder.
#[cfg(not(all(feature = "editor_tools", not(target_arch = "wasm32"))))]
fn start_download(url: String, tx: Sender<Result<DownloadedImage, String>>) {
    let _ = tx.send(Err(url));
}

/// Drain finished downloads and register them as `Image` assets.
pub fn poll_web_images(mut cache: ResMut<WebImages>, mut images: ResMut<Assets<Image>>) {
    let mut done = Vec::new();
    while let Ok(res) = cache.rx.try_recv() {
        done.push(res);
    }
    for res in done {
        match res {
            Ok(d) => {
                cache.in_flight.remove(&d.url);
                let image = Image::new(
                    Extent3d { width: d.width, height: d.height, depth_or_array_layers: 1 },
                    TextureDimension::D2,
                    d.rgba,
                    TextureFormat::Rgba8UnormSrgb,
                    default(),
                );
                let handle = images.add(image);
                cache.handles.insert(d.url, handle);
            }
            Err(url) => {
                cache.in_flight.remove(&url);
                cache.failed.insert(url);
            }
        }
    }
}

/// Marks a node that wants `url` downloaded, so [`request_web_images`] can ask
/// for it without the builder needing the cache resource.
#[derive(Component)]
pub struct WebImageWanted(pub String);

/// Request the image behind every on-screen node. Cheap: [`WebImages::request`]
/// drops anything already loaded, in flight, or known bad.
pub fn request_web_images(mut cache: ResMut<WebImages>, q: Query<&WebImageWanted>) {
    for want in q.iter() {
        cache.request(&want.0);
    }
}
