//! Bevy thumbnail cache for the marketplace panels: images from URLs, shown in
//! `ImageNode`s, each with a small blurred copy for the soft backdrop behind
//! cards and hero slides.
//!
//! Each image is fetched once and kept on disk, already shrunk
//! ([`fetch_hub_thumbnail`]): in the companion when one is connected (job kind
//! [`KIND`]), on a thread here otherwise. The editor then only reads two small
//! PNGs per image. Before, every session downloaded every image again,
//! decoded it at full size and blurred it, one thread per image with no limit,
//! and held each at full size in memory and on the GPU for a card a few
//! hundred pixels wide.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;

use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver, Sender};
use renzora_ember::widgets::{add_gpu_image, GpuImageSizes};
use serde::{Deserialize, Serialize};

/// The companion job kind.
pub const KIND: &str = "hub_thumbnail";

/// Images fetched at once. The store opens on a grid of dozens; all of them at
/// once was dozens of threads decoding at full size together.
const CONCURRENT: usize = 6;

/// Long side a cached image is shrunk to. Above the largest hero slide, so it
/// is only ever drawn smaller.
const MAX_SIDE: u32 = 1024;

/// Width of the blurred backdrop copy. A `Stretch` node upscales it into a
/// soft gradient, so it can be tiny.
const BLUR_WIDTH: u32 = 72;

/// How long a cached image is trusted before it is fetched again, in case the
/// listing changed it under the same URL.
const MAX_AGE: std::time::Duration = std::time::Duration::from_secs(7 * 24 * 60 * 60);

/// One image to fetch and cache: the job's arguments.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HubThumbnail {
    pub url: String,
    pub image: PathBuf,
    pub blurred: PathBuf,
}

impl HubThumbnail {
    /// Where `url`'s two files are cached. `None` without a home directory.
    fn for_url(url: &str) -> Option<Self> {
        let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
        let dir = PathBuf::from(home).join(".renzora").join("cache").join("hub");
        // FNV-1a: a stable name for the URL without a hashing dependency.
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for b in url.as_bytes() {
            hash ^= u64::from(*b);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        Some(Self {
            url: url.to_string(),
            image: dir.join(format!("{hash:016x}.png")),
            blurred: dir.join(format!("{hash:016x}-blur.png")),
        })
    }

    /// Both files are there, and recent.
    fn cached(&self) -> bool {
        let fresh = |p: &PathBuf| {
            std::fs::metadata(p)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age < MAX_AGE)
        };
        fresh(&self.image) && self.blurred.is_file()
    }
}

/// Download `args.url`, shrink it to [`MAX_SIDE`], blur a tiny copy, and write
/// both as PNGs. Runs in the companion, or on a thread of the editor's when
/// there is none.
#[cfg(not(target_arch = "wasm32"))]
pub fn fetch_hub_thumbnail(args: &HubThumbnail) -> Result<(), String> {
    // The 10 MiB cap is enforced by the backend as the body arrives, not after:
    // this URL came from a server, and a limit applied once the bytes are
    // already in memory protects nothing.
    let response = renzora::net::Request::get(&args.url)
        .max_bytes(10 * 1024 * 1024)
        .send()
        .map_err(|e| format!("could not fetch {}: {e}", args.url))?;
    if !response.is_ok() {
        return Err(format!("{} answered {}", args.url, response.status));
    }
    let img = image::load_from_memory(&response.body).map_err(|e| format!("could not read {}: {e}", args.url))?;
    let img = if img.width().max(img.height()) > MAX_SIDE {
        img.resize(MAX_SIDE, MAX_SIDE, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let bw = BLUR_WIDTH.min(width).max(1);
    let bh = ((bw as f32 * height as f32 / width.max(1) as f32).round() as u32).max(1);
    let small = image::imageops::resize(&rgba, bw, bh, image::imageops::FilterType::Triangle);
    let blurred = image::imageops::blur(&small, 4.0);

    if let Some(dir) = args.image.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    }
    // Each written beside and renamed into place, so a reader never meets half
    // a file. The blurred one goes first: the image is what says "cached".
    for (pixels, path) in [(&blurred, &args.blurred), (&rgba, &args.image)] {
        let partial = path.with_extension("partial.png");
        pixels.save(&partial).map_err(|e| format!("could not write {}: {e}", partial.display()))?;
        std::fs::rename(&partial, path).map_err(|e| format!("could not write {}: {e}", path.display()))?;
    }
    Ok(())
}

struct Downloaded {
    url: String,
    rgba: Vec<u8>,
    width: u32,
    height: u32,
    blur_rgba: Vec<u8>,
    blur_width: u32,
    blur_height: u32,
}

/// Async URL → `Handle<Image>` cache. `request` queues an image; `poll_thumbs`
/// starts fetches and registers finished images each frame; `get` returns the
/// handle.
#[derive(Resource)]
pub(crate) struct HubThumbs {
    handles: HashMap<String, Handle<Image>>,
    /// Blurred backdrop variant of each loaded image, keyed by the same URL.
    blurred: HashMap<String, Handle<Image>>,
    in_flight: HashSet<String>,
    failed: HashSet<String>,
    /// Requested and not started yet, oldest first.
    queue: VecDeque<String>,
    running: usize,
    remote: Option<renzora::CompanionRemote>,
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
            queue: VecDeque::new(),
            running: 0,
            remote: None,
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

    /// Queue `url` if not already loaded / in flight / failed.
    pub fn request(&mut self, url: &str) {
        if self.handles.contains_key(url) || self.in_flight.contains(url) || self.failed.contains(url) {
            return;
        }
        self.in_flight.insert(url.to_string());
        self.queue.push_back(url.to_string());
    }
}

/// Fetch (unless cached) and read back one image, on a thread of its own.
#[cfg(not(target_arch = "wasm32"))]
fn start_download(url: String, remote: Option<renzora::CompanionRemote>, tx: Sender<Result<Downloaded, String>>) {
    std::thread::spawn(move || {
        let result = (|| -> Result<Downloaded, String> {
            let args = HubThumbnail::for_url(&url).ok_or_else(|| url.clone())?;
            if !args.cached() {
                // The companion fetches it when it is there; this thread waits.
                let in_companion = remote.as_ref().and_then(|r| r.run(KIND, url.clone(), &args, &mut |_, _| {}));
                match in_companion {
                    Some(Ok(_)) => {}
                    Some(Err(why)) => {
                        debug!("[hub] {why}");
                        return Err(url.clone());
                    }
                    None => fetch_hub_thumbnail(&args).map_err(|why| {
                        debug!("[hub] {why}");
                        url.clone()
                    })?,
                }
            }
            let read = |p: &PathBuf| image::open(p).map(|i| i.to_rgba8()).map_err(|_| url.clone());
            let (image, blurred) = (read(&args.image)?, read(&args.blurred)?);
            let ((width, height), (blur_width, blur_height)) = (image.dimensions(), blurred.dimensions());
            Ok(Downloaded {
                url: url.clone(),
                rgba: image.into_raw(),
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
fn start_download(_url: String, _remote: Option<renzora::CompanionRemote>, _tx: Sender<Result<Downloaded, String>>) {}

/// Start queued fetches within [`CONCURRENT`], and register finished images as
/// `Image` assets.
pub(crate) fn poll_thumbs(
    mut thumbs: ResMut<HubThumbs>,
    mut images: ResMut<Assets<Image>>,
    mut sizes: ResMut<GpuImageSizes>,
    companion: Option<Res<renzora::CompanionJobs>>,
) {
    if thumbs.remote.is_none() {
        thumbs.remote = companion.map(|c| c.remote());
    }
    while thumbs.running < CONCURRENT {
        let Some(url) = thumbs.queue.pop_front() else { break };
        thumbs.running += 1;
        start_download(url, thumbs.remote.clone(), thumbs.tx.clone());
    }

    let mut done = Vec::new();
    while let Ok(res) = thumbs.rx.try_recv() {
        done.push(res);
    }
    for res in done {
        thumbs.running = thumbs.running.saturating_sub(1);
        match res {
            Ok(d) => {
                thumbs.in_flight.remove(&d.url);
                let handle = add_gpu_image(&mut images, &mut sizes, d.rgba, d.width, d.height);
                let blur_handle =
                    add_gpu_image(&mut images, &mut sizes, d.blur_rgba, d.blur_width, d.blur_height);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_url_has_its_own_cache_files() {
        let a = HubThumbnail::for_url("https://renzora.com/a.png").unwrap();
        let b = HubThumbnail::for_url("https://renzora.com/b.png").unwrap();
        assert_ne!(a.image, b.image);
        assert_eq!(a.image, HubThumbnail::for_url("https://renzora.com/a.png").unwrap().image);
        assert_ne!(a.image, a.blurred);
    }
}
