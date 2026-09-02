//! Images loaded from a path on disk, decoded off the main thread.
//!
//! The counterpart to the markdown widget's URL cache: same shape, but the bytes
//! come from the filesystem rather than the network. It exists because several
//! panels need to show artwork that ships *beside* the thing it depicts — a
//! plugin's `thumbnail.jpg` next to its `Cargo.toml` — which Bevy's
//! `AssetServer` cannot reach. The asset server resolves against the project's
//! assets root, and these files live under the executable's `plugins/`
//! directory, outside it.
//!
//! # Why a cache rather than decoding in the builder
//!
//! A UI builder gets `&mut Commands` and nothing else, and reactive lists
//! rebuild their rows whenever anything they hash changes. Decoding in the
//! builder would re-decode every visible thumbnail on every rebuild — for the
//! plugins panel that is ~76 JPEGs, on the main thread, each time a switch is
//! flipped. Requesting into a cache and binding the handle means a file is read
//! once per session.
//!
//! Failures are remembered too, so a plugin with no thumbnail costs one failed
//! `read` for the whole session rather than one per frame.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use crossbeam_channel::{unbounded, Receiver, Sender};

struct Decoded {
    path: PathBuf,
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

/// Path → `Handle<Image>` cache for images on disk.
///
/// [`FileImages::request`] starts a background read+decode, [`poll_file_images`]
/// registers finished ones, and [`FileImages::get`] returns the handle.
#[derive(Resource)]
pub struct FileImages {
    handles: HashMap<PathBuf, Handle<Image>>,
    in_flight: HashSet<PathBuf>,
    /// Paths that do not exist or would not decode. Kept so a missing file is
    /// not retried every frame forever.
    failed: HashSet<PathBuf>,
    tx: Sender<Result<Decoded, PathBuf>>,
    rx: Receiver<Result<Decoded, PathBuf>>,
}

impl Default for FileImages {
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

impl FileImages {
    /// The loaded handle for `path`, or `None` while it is loading or if it
    /// failed.
    pub fn get(&self, path: &Path) -> Option<Handle<Image>> {
        self.handles.get(path).cloned()
    }

    /// True once we know `path` will never produce an image — missing, or not a
    /// format we decode. Lets a caller commit to its placeholder instead of
    /// leaving an empty box in case something arrives.
    pub fn failed(&self, path: &Path) -> bool {
        self.failed.contains(path)
    }

    /// Read and decode `path` in the background, unless it is already loaded, in
    /// flight, or known bad.
    pub fn request(&mut self, path: &Path) {
        if self.handles.contains_key(path)
            || self.in_flight.contains(path)
            || self.failed.contains(path)
        {
            return;
        }
        self.in_flight.insert(path.to_path_buf());
        start_decode(path.to_path_buf(), self.tx.clone());
    }
}

#[cfg(all(feature = "editor_tools", not(target_arch = "wasm32")))]
fn start_decode(path: PathBuf, tx: Sender<Result<Decoded, PathBuf>>) {
    std::thread::spawn(move || {
        let result = (|| -> Result<Decoded, PathBuf> {
            let bytes = std::fs::read(&path).map_err(|_| path.clone())?;
            let img = image::load_from_memory(&bytes).map_err(|_| path.clone())?;
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            Ok(Decoded { path: path.clone(), rgba: rgba.into_raw(), width, height })
        })();
        let _ = tx.send(result);
    });
}

/// No filesystem to read, so every request fails and callers show placeholders.
#[cfg(not(all(feature = "editor_tools", not(target_arch = "wasm32"))))]
fn start_decode(path: PathBuf, tx: Sender<Result<Decoded, PathBuf>>) {
    let _ = tx.send(Err(path));
}

/// A square thumbnail for `path`, with a glyph standing in until (or unless) the
/// file loads.
///
/// The placeholder is always spawned rather than swapped in on failure: a plugin
/// that ships no `thumbnail.jpg` is the common case, not the error case, and a
/// box that fills in later would make the grid jump. The image is layered over
/// the top and revealed only once decoded.
///
/// `aspect_ratio` rather than a fixed height so the tile stays square while its
/// card flex-grows to fill the row.
pub fn file_image_tile(
    commands: &mut Commands,
    fonts: &crate::font::EmberFonts,
    path: PathBuf,
    placeholder_icon: &str,
    placeholder_color: (u8, u8, u8),
    radius: f32,
) -> Entity {
    use crate::theme::{hover_bg, rgb};

    let frame = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                aspect_ratio: Some(1.0),
                position_type: PositionType::Relative,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(Val::Px(radius)),
                ..default()
            },
            BackgroundColor(rgb(hover_bg())),
            bevy::ui::FocusPolicy::Pass,
            FileImageWanted(path.clone()),
        ))
        .id();

    let ph = crate::font::icon_text(commands, &fonts.phosphor, placeholder_icon, placeholder_color, 30.0);
    commands.entity(ph).insert(bevy::ui::FocusPolicy::Pass);
    commands.entity(frame).add_child(ph);

    let img = commands
        .spawn((
            ImageNode::default(),
            bevy::ui::FocusPolicy::Pass,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::None,
                ..default()
            },
        ))
        .id();
    let want = path;
    crate::reactive::tracked::bind_with(
        commands,
        img,
        move |w| w.get_resource::<FileImages>().and_then(|c| c.get(&want)),
        |w, e, handle: &Option<Handle<Image>>| {
            if let Some(h) = handle {
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
    commands.entity(frame).add_child(img);
    frame
}

/// Marks a tile that wants `path` loaded, so [`request_file_images`] can ask for
/// it without every caller having to reach the cache resource itself.
#[derive(Component)]
pub struct FileImageWanted(pub PathBuf);

/// Request the file behind every on-screen tile. Cheap: [`FileImages::request`]
/// drops anything already loaded, in flight, or known bad.
pub fn request_file_images(mut cache: ResMut<FileImages>, q: Query<&FileImageWanted>) {
    for want in q.iter() {
        cache.request(&want.0);
    }
}

/// Drain finished decodes and register them as `Image` assets.
pub fn poll_file_images(mut cache: ResMut<FileImages>, mut images: ResMut<Assets<Image>>) {
    let mut done = Vec::new();
    while let Ok(res) = cache.rx.try_recv() {
        done.push(res);
    }
    for res in done {
        match res {
            Ok(d) => {
                cache.in_flight.remove(&d.path);
                let image = Image::new(
                    Extent3d { width: d.width, height: d.height, depth_or_array_layers: 1 },
                    TextureDimension::D2,
                    d.rgba,
                    TextureFormat::Rgba8UnormSrgb,
                    default(),
                );
                let handle = images.add(image);
                cache.handles.insert(d.path, handle);
            }
            Err(path) => {
                cache.in_flight.remove(&path);
                cache.failed.insert(path);
            }
        }
    }
}
