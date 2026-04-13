//! Thumbnail image cache — downloads images from URLs on background threads
//! and registers them as egui textures for display in marketplace panels.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc;

use bevy_egui::egui;

struct DownloadedImage {
    url: String,
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

/// Asynchronous image cache that downloads thumbnails and converts them to
/// egui textures. Call [`poll`] each frame before [`get`].
pub struct ImageCache {
    textures: HashMap<String, egui::TextureHandle>,
    in_flight: HashSet<String>,
    failed: HashSet<String>,
    sender: mpsc::Sender<Result<DownloadedImage, String>>,
    receiver: mpsc::Receiver<Result<DownloadedImage, String>>,
}

impl Default for ImageCache {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            textures: HashMap::new(),
            in_flight: HashSet::new(),
            failed: HashSet::new(),
            sender,
            receiver,
        }
    }
}

impl ImageCache {
    /// Poll for completed downloads and register textures with egui.
    pub fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(result) = self.receiver.try_recv() {
            match result {
                Ok(img) => {
                    self.in_flight.remove(&img.url);
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                        [img.width as usize, img.height as usize],
                        &img.rgba,
                    );
                    let handle = ctx.load_texture(
                        &img.url,
                        color_image,
                        egui::TextureOptions::LINEAR,
                    );
                    self.textures.insert(img.url, handle);
                }
                Err(url) => {
                    self.in_flight.remove(&url);
                    self.failed.insert(url);
                }
            }
        }
    }

    /// Get the texture for a URL, starting a background download if needed.
    /// Returns `None` if not yet loaded or failed.
    pub fn get(&mut self, url: &str) -> Option<&egui::TextureHandle> {
        if self.textures.contains_key(url) {
            return self.textures.get(url);
        }
        if !self.in_flight.contains(url) && !self.failed.contains(url) {
            self.start_download(url);
        }
        None
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn start_download(&mut self, url: &str) {
        use std::io::Read;

        self.in_flight.insert(url.to_string());
        let url_owned = url.to_string();
        let sender = self.sender.clone();

        std::thread::spawn(move || {
            let result = (|| -> Result<DownloadedImage, String> {
                let response = ureq::get(&url_owned)
                    .call()
                    .map_err(|_| url_owned.clone())?;

                let mut bytes = Vec::new();
                response
                    .into_body()
                    .into_reader()
                    .take(10 * 1024 * 1024) // 10 MB limit for thumbnails
                    .read_to_end(&mut bytes)
                    .map_err(|_| url_owned.clone())?;

                let img = image::load_from_memory(&bytes).map_err(|_| url_owned.clone())?;
                let rgba = img.to_rgba8();
                let (width, height) = rgba.dimensions();

                Ok(DownloadedImage {
                    url: url_owned,
                    rgba: rgba.into_raw(),
                    width,
                    height,
                })
            })();
            let _ = sender.send(result);
        });
    }

    #[cfg(target_arch = "wasm32")]
    fn start_download(&mut self, _url: &str) {}
}
