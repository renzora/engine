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
        format!("{}{}", renzora_auth::client::api_base(), url)
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

/// A rectangular thumbnail (rounded corners) that fills in when the URL loads.
/// For marketplace covers and other non-avatar imagery.
pub(crate) fn thumb_image(
    commands: &mut Commands,
    fonts: &renzora_ember::font::EmberFonts,
    url: Option<&str>,
    width: f32,
    height: f32,
    placeholder_icon: &str,
) -> Entity {
    use renzora_ember::theme::{hover_bg, rgb, text_muted};

    let wrap = commands
        .spawn((
            Node {
                width: Val::Px(width),
                height: Val::Px(height),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                flex_shrink: 0.0,
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(hover_bg())),
        ))
        .id();
    let ph = renzora_ember::font::icon_text(commands, &fonts.phosphor, placeholder_icon, text_muted(), height * 0.4);
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
                    ..default()
                },
                // Pass so a clickable wrap (e.g. a photos-grid cell that opens the
                // lightbox) receives the press instead of this image swallowing it.
                bevy::ui::FocusPolicy::Pass,
            ))
            .id();
        renzora_ember::reactive::bind_with(
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

/// Overlay a cover/banner image that fills `wrap` once `url` loads. `wrap` must
/// clip its overflow; the image is absolutely positioned to cover it and stays
/// hidden until the download completes (so the color backdrop shows meanwhile).
/// Used for profile cover photos in the hero banner.
pub(crate) fn fill_image(commands: &mut Commands, wrap: Entity, url: &str) {
    let url = absolute_url(url);
    commands.entity(wrap).insert(AvatarUrl(url.clone()));
    let img = commands
        .spawn((
            ImageNode::default(),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                // Cover by WIDTH: full width; `aspect_ratio` (set from the loaded
                // image's real dimensions below) drives the height, so a wide
                // cover fills the strip and crops top/bottom via the parent's
                // `overflow: clip` — instead of letterboxing (contained + centered)
                // as the default aspect-preserving `ImageNode` does (the
                // "squashed" bug). `min_height: 100%` guards against any gap.
                width: Val::Percent(100.0),
                min_height: Val::Percent(100.0),
                display: Display::None,
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    renzora_ember::reactive::bind_with(
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
                // Read the real pixel size so we can pin the aspect ratio — taffy
                // then computes height = width / aspect reliably (the ImageNode
                // auto-measure doesn't for an absolutely-positioned node).
                let dims = w
                    .get_resource::<Assets<Image>>()
                    .and_then(|a| a.get(h))
                    .map(|img| img.size());
                if let Some(mut node) = w.get_mut::<Node>(e) {
                    if let Some(d) = dims {
                        if d.y > 0 {
                            node.aspect_ratio = Some(d.x as f32 / d.y as f32);
                        }
                    }
                    node.display = Display::Flex;
                }
            }
        },
    );
    commands.entity(wrap).add_child(img);
}

/// An avatar with a presence dot overlaid on its bottom-right corner
/// (old-Facebook-sidebar style). The dot pulses gently while online.
pub(crate) fn avatar_with_presence(
    commands: &mut Commands,
    fonts: &renzora_ember::font::EmberFonts,
    url: Option<&str>,
    size: f32,
    online: bool,
) -> Entity {
    use renzora_ember::theme::{card_bg, rgb};

    let wrap = commands
        .spawn(Node {
            width: Val::Px(size),
            height: Val::Px(size),
            flex_shrink: 0.0,
            ..default()
        })
        .id();
    let av = avatar_image(commands, fonts, url, size);
    let dot_size = (size * 0.34).max(8.0);
    // Ring in the card color so the dot reads as sitting on top of the avatar.
    let ring = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(-1.0),
                bottom: Val::Px(-1.0),
                width: Val::Px(dot_size),
                height: Val::Px(dot_size),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(dot_size / 2.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
        ))
        .id();
    let dot = renzora_ember::widgets::status_dot(commands, (82, 196, 120), dot_size - 3.0, online);
    commands.entity(ring).add_child(dot);
    commands.entity(wrap).add_children(&[av, ring]);
    wrap
}

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
        renzora_ember::reactive::bind_with(
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
