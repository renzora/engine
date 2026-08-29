//! Shared avatar/image cache for the social panels: downloads images from URLs
//! on background threads and registers them as bevy `Image` assets for display
//! in `ImageNode`s. Same pattern as the marketplace thumbnail cache.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use crossbeam_channel::{unbounded, Receiver, Sender};

struct Downloaded {
    url: String,
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

/// Async URL → `Handle<Image>` cache. `request` starts a background download;
/// `poll_avatars` registers finished images each frame; `get` returns the handle.
#[derive(Resource)]
pub(crate) struct AvatarCache {
    handles: HashMap<String, Handle<Image>>,
    in_flight: HashSet<String>,
    failed: HashSet<String>,
    tx: Sender<Result<Downloaded, String>>,
    rx: Receiver<Result<Downloaded, String>>,
}

impl Default for AvatarCache {
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

impl AvatarCache {
    /// The loaded handle for `url`, or `None` if not ready / failed.
    pub fn get(&self, url: &str) -> Option<Handle<Image>> {
        self.handles.get(url).cloned()
    }

    /// Start downloading `url` if not already loaded / in flight / failed.
    /// Relative URLs (e.g. `/uploads/...`) are resolved against the API base.
    pub fn request(&mut self, url: &str) {
        let url = absolute_url(url);
        if self.handles.contains_key(&url)
            || self.in_flight.contains(&url)
            || self.failed.contains(&url)
        {
            return;
        }
        self.in_flight.insert(url.clone());
        start_download(url, self.tx.clone());
    }
}

/// Resolve site-relative URLs against the API base.
pub(crate) fn absolute_url(url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        format!("{}{}", crate::auth::client::api_base(), url)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn start_download(url: String, tx: Sender<Result<Downloaded, String>>) {
    std::thread::spawn(move || {
        let result = (|| -> Result<Downloaded, String> {
            // The 10 MiB cap is enforced by the backend as the body arrives,
            // not after: this URL came from a server, and a limit applied once
            // the bytes are already in memory protects nothing.
            let response = renzora::net::Request::get(&url)
                .max_bytes(10 * 1024 * 1024)
                .send()
                .map_err(|_| url.clone())?;
            if !response.is_ok() {
                return Err(url.clone());
            }
            let img = image::load_from_memory(&response.body).map_err(|_| url.clone())?;
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            Ok(Downloaded { url: url.clone(), rgba: rgba.into_raw(), width, height })
        })();
        let _ = tx.send(result);
    });
}

#[cfg(target_arch = "wasm32")]
fn start_download(_url: String, _tx: Sender<Result<Downloaded, String>>) {}

/// Drain finished downloads, register them as `Image` assets.
pub(crate) fn poll_avatars(mut cache: ResMut<AvatarCache>, mut images: ResMut<Assets<Image>>) {
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

/// Marks an entity as wanting `url` downloaded into the [`AvatarCache`].
/// A single global system requests all of them each frame (cheap no-op once
/// cached), so builders never need cache access.
#[derive(Component)]
pub(crate) struct AvatarUrl(pub String);

/// Request every on-screen avatar URL through the cache.
pub(crate) fn request_avatars(mut cache: ResMut<AvatarCache>, q: Query<&AvatarUrl>) {
    for a in &q {
        cache.request(&a.0);
    }
}

// Three more builders lived here and served the deleted panels: `thumb_image`
// (rectangular covers), `fill_image` (profile banner photos) and
// `avatar_with_presence` (an avatar with an online dot). The store and library
// have their own thumbnail cache in `thumbs.rs`, and presence is a social idea
// with nothing left to report, so all three went. Only the round avatar below
// is still used — by the wallet and the account settings section.

/// Spawn a fixed-size round avatar: a placeholder circle that swaps to the
/// image once the URL loads (requested through the shared [`AvatarCache`]).
pub(crate) fn avatar_image(
    commands: &mut Commands,
    fonts: &renzora_ember::font::EmberFonts,
    url: Option<&str>,
    size: f32,
) -> Entity {
    use renzora_ember::theme::{hover_bg, rgb, text_muted};

    let wrap = commands
        .spawn((
            Node {
                width: Val::Px(size),
                height: Val::Px(size),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                flex_shrink: 0.0,
                border_radius: BorderRadius::all(Val::Px(size / 2.0)),
                ..default()
            },
            BackgroundColor(rgb(hover_bg())),
        ))
        .id();
    let ph = renzora_ember::font::icon_text(commands, &fonts.phosphor, "user", text_muted(), size * 0.55);
    commands.entity(wrap).add_child(ph);

    if let Some(url) = url {
        let url = absolute_url(url);
        commands.entity(wrap).insert(AvatarUrl(url.clone()));
        let img = commands
            .spawn((
                ImageNode::default(),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    display: Display::None,
                    border_radius: BorderRadius::all(Val::Px(size / 2.0)),
                    ..default()
                },
            ))
            .id();
        renzora_ember::reactive::tracked::bind_with(
            commands,
            img,
            move |w| w.get_resource::<AvatarCache>().and_then(|c| c.get(&url)),
            |w, e, h: &Option<Handle<Image>>| {
                if let Some(h) = h {
                    if let Some(mut n) = w.get_mut::<ImageNode>(e) {
                        if n.image != *h {
                            n.image = h.clone();
                        }
                    }
                    if let Some(mut node) = w.get_mut::<Node>(e) {
                        node.display = Display::Flex;
                    }
                }
            },
        );
        commands.entity(wrap).add_child(img);
    }
    wrap
}
