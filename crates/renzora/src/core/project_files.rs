//! What changed on disk under the open project, as one event every crate reads.
//!
//! # Why this exists
//!
//! Before this, every feature that cared about a file changing grew its own
//! answer, and they disagreed: the Rust script watcher walked the whole project
//! every 0.5s, the inspector's script index walked the same tree every 3s, the
//! material drawer walked it again on its own 3s timer, the asset browser
//! re-listed a directory twice a second, language packs rescanned every 2s, and
//! the shader editor ran a second `notify` backend beside all of it. Nineteen
//! files compared `SystemTime` by hand.
//!
//! Two of those walks were separately measured costing 110ms and 130ms **in a
//! single frame** on the same project, and separately fixed by moving each onto
//! a task pool. That is the same bug found twice and queued to be found again,
//! and the reason it kept recurring is that "did a file change?" had no single
//! owner. This is that owner.
//!
//! # Why events rather than polling
//!
//! Polling scales with the size of the project and answers nothing in between
//! ticks. A project with a few thousand files pays the walk whether or not
//! anything changed, which is the common case by an enormous margin, and still
//! reports a save up to an interval late. An event costs nothing when the disk
//! is quiet and arrives as soon as it is not.
//!
//! # Where the events come from
//!
//! `renzora_project_watch` owns one `notify` watcher over the project root and
//! publishes [`ProjectFileChanged`]. It uses Bevy's own [`FileWatcher`], so there
//! is no new dependency and no second debouncer, and it forwards asset-shaped
//! changes into [`AssetReloadSink`] so `AssetServer` hot-reload works too.
//!
//! Nothing in this module knows how the watching is done. A crate that wants to
//! react to a file changing reads the event and links no watcher at all.
//!
//! [`FileWatcher`]: bevy::asset::io::file::FileWatcher

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use bevy::asset::io::AssetSourceEvent;
use bevy::prelude::*;

/// Coarse classification of a file by extension.
///
/// Variants are deliberately broad: `Texture` covers every image format Bevy
/// can decode rather than one variant per extension, because every consumer so
/// far wants "is this an image" and none wants "is this specifically a TGA".
///
/// This used to live in `renzora_asset_registry`, which is a crate the watcher
/// cannot depend on without dragging the whole registry along. It moved here
/// under the rule that a type crossing a crate boundary gets one definition in
/// the contract crate. `renzora_asset_registry` re-exports it, so every path a
/// caller already wrote still resolves.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum AssetKind {
    /// 3D model: `glb`, `gltf`, `obj`, `fbx`, `usd*`, `dae`, `abc`, `blend`.
    Model,
    /// Image format Bevy can decode at runtime. Includes HDR/EXR and `.rmip`.
    Texture,
    /// Renzora `.material` file consumed by `renzora_shader`.
    Material,
    /// Renzora scene file, which is `.bsn`.
    ///
    /// This said `.scene` until the watcher needed it, which is how long a
    /// classification nobody queried can stay wrong: BSN replaced RON as the
    /// scene format and the table was never updated, so every scene in every
    /// project has been classified `Other` since.
    Scene,
    /// Audio sample.
    Audio,
    /// Video clip.
    Video,
    /// Source-level script. `.rs` is included: it is compiled rather than
    /// interpreted (see `renzora_rust_script`), but it is a script to everything
    /// that asks this question.
    Script,
    /// Hand-authored shader source (WGSL/GLSL/HLSL).
    Shader,
    /// A `.particle` effect definition.
    Particle,
    /// An `.anim` clip, whether skeletal or property.
    Animation,
    /// A `.html` markup template for the game-UI authoring tools.
    UiTemplate,
    /// Anything else: config, docs, unrecognised extensions.
    Other,
}

impl AssetKind {
    /// Classify a path by its lower-cased extension.
    ///
    /// Matches the extension table the asset browser uses for icon picking, so
    /// a file that shows up as "Image" in the browser classifies as `Texture`
    /// here.
    pub fn from_path(path: &Path) -> Self {
        let Some(ext) = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase())
        else {
            return AssetKind::Other;
        };
        match ext.as_str() {
            "glb" | "gltf" | "obj" | "fbx" | "usd" | "usda" | "usdc" | "usdz" | "abc" | "dae"
            | "blend" => AssetKind::Model,
            "png" | "jpg" | "jpeg" | "bmp" | "tga" | "webp" | "hdr" | "exr" | "rmip" => {
                AssetKind::Texture
            }
            "material" | "material_bp" => AssetKind::Material,
            "bsn" => AssetKind::Scene,
            "wav" | "ogg" | "mp3" | "flac" | "opus" => AssetKind::Audio,
            "mp4" | "avi" | "mov" | "webm" => AssetKind::Video,
            "lua" | "rs" | "js" | "ts" => AssetKind::Script,
            "wgsl" | "glsl" | "vert" | "frag" | "hlsl" => AssetKind::Shader,
            "particle" => AssetKind::Particle,
            "anim" => AssetKind::Animation,
            "html" => AssetKind::UiTemplate,
            _ => AssetKind::Other,
        }
    }
}

/// What happened to a path.
///
/// Deliberately four cases and not Bevy's eleven. [`AssetSourceEvent`] splits
/// assets from folders from metadata sidecars, because the asset server acts
/// differently on each; a consumer here wants to know whether a file it cares
/// about appeared, changed, or went away, and gets `is_dir` alongside for the
/// rare case that the distinction matters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileChange {
    /// The path did not exist and now does.
    Added,
    /// The path existed and its contents changed.
    Modified,
    /// The path existed and no longer does.
    ///
    /// **Treat this as a question, not a fact.** Many editors save by writing a
    /// temporary file and renaming it over the original, so a perfectly ordinary
    /// save can surface as a removal followed by an addition. A consumer that
    /// reacts to this by warning the user should confirm the file is really gone
    /// (see [`ProjectFileChanged::still_missing`]) rather than trusting it.
    Removed,
    /// The path moved. `from` is where it was.
    Renamed { from: PathBuf },
}

/// A file under the open project changed.
///
/// Read it with an ordinary `MessageReader<ProjectFileChanged>`. There is no
/// subscription to register and no filter to install: one is published for
/// every change under the project root that is not inside an ignored directory,
/// and a consumer matches on the parts it cares about.
///
/// A buffered `Message` rather than an observer `Event`, because the bursts are
/// what make this hard. A `git checkout` or a batch export rewrites thousands of
/// paths at once, and an observer would run a consumer once per path,
/// synchronously, inside the drain. Buffering lets the asset browser re-list its
/// directory once for the whole burst instead of a thousand times.
#[derive(Message, Debug, Clone)]
pub struct ProjectFileChanged {
    /// What happened.
    pub change: FileChange,
    /// Absolute path, which is what `std::fs` wants.
    pub path: PathBuf,
    /// The watched directory [`Self::relative`] is relative to.
    ///
    /// Almost always the open project, and a consumer that only cares about
    /// project files should say so: `change.root == project.path`. It is not
    /// guaranteed, because a crate can register another directory through
    /// [`ExtraWatchRoots`] (the engine's own `languages/` folder does), and a
    /// path under one of those is not project-relative at all.
    pub root: PathBuf,
    /// Relative to [`Self::root`], forward-slashed.
    ///
    /// For a project file this is the key every other producer in the engine
    /// uses: `ContentProblems` rows, the asset registry and `AssetServer` paths
    /// are all in this form, so a consumer can look the file up in any of them
    /// without re-deriving it.
    pub relative: String,
    /// Classification of [`Self::path`] by extension.
    pub kind: AssetKind,
    /// Whether the path is (or was) a directory.
    ///
    /// Best-effort on removal: the path is gone by the time the event arrives,
    /// so this comes from what the watcher was told rather than from a stat.
    pub is_dir: bool,
}

impl ProjectFileChanged {
    /// Does this path end in `ext` (given without the dot, any case)?
    pub fn has_extension(&self, ext: &str) -> bool {
        self.path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case(ext))
    }

    /// Did the file's content change, one way or another?
    ///
    /// True for added, modified and renamed-to; false for removed. This is the
    /// condition almost every hot-reload wants, and writing it out at each call
    /// site is how one of them ends up quietly not handling `Added`.
    pub fn is_live(&self) -> bool {
        !matches!(self.change, FileChange::Removed)
    }

    /// Confirm a [`FileChange::Removed`] by asking the filesystem.
    ///
    /// The event is a hint (see [`FileChange::Removed`]). Anything that would
    /// tell the user their file is missing should call this first, because a
    /// save-by-rename briefly looks exactly like a deletion and a warning fired
    /// on that is a false alarm the user cannot act on.
    pub fn still_missing(&self) -> bool {
        !self.path.exists()
    }
}

/// Directories the watcher never reports from.
///
/// Not a performance tweak. `target/` alone can hold hundreds of thousands of
/// files that change constantly during a build, and a script or shader rebuild
/// writing into it would feed its own output straight back in as change events.
/// The same list is used by the project walks (`collect_project_scripts` and
/// friends), and the two must agree or a file will be compiled by one and
/// ignored by the other.
pub const IGNORED_DIRS: &[&str] = &[
    "target",
    ".git",
    ".renzora",
    "node_modules",
    "dist",
    ".svn",
    ".hg",
];

/// Is this project-relative path inside an ignored directory, or hidden?
///
/// Takes the forward-slashed relative form, which is what [`ProjectFileChanged`]
/// carries. Leading-dot components are ignored wholesale: editors and tools
/// scribble in them constantly and nothing in a project is authored there.
pub fn is_ignored(relative: &str) -> bool {
    relative.split('/').any(|part| {
        IGNORED_DIRS.contains(&part) || (part.starts_with('.') && part.len() > 1)
    })
}

/// Directories to watch besides the open project.
///
/// Insert a path here during plugin `build` and the watcher picks it up; events
/// from it carry that directory as their [`ProjectFileChanged::root`].
///
/// The bar for adding one is high, and "my feature reads files from there" is
/// not it. This exists for directories that belong to the *engine install*
/// rather than the user's project, where there is genuinely no project-relative
/// path to speak of. `renzora_lang`'s `languages/` folder, which sits beside the
/// executable, is the case it was built for.
///
/// Anything under the project is already covered. That includes several things
/// that look like engine directories and are not: `themes/` and `fonts/` are
/// both `<project>/...`, and registering them here would watch them twice.
#[derive(Resource, Default)]
pub struct ExtraWatchRoots(Vec<PathBuf>);

impl ExtraWatchRoots {
    /// Watch `dir` as well. Ignored if it is already registered.
    pub fn add(&mut self, dir: PathBuf) {
        if !self.0.contains(&dir) {
            self.0.push(dir);
        }
    }

    pub fn paths(&self) -> &[PathBuf] {
        &self.0
    }
}

/// Is this file name an editor's scratch file rather than a real one?
///
/// Almost nothing writes a file in place. The safe way to save is to write a
/// temporary file beside the target and rename it over the top, so the reader
/// either sees the old contents or the new ones and never a half-written file.
/// That is what an editor, `git`, and most tooling actually does, and the
/// watcher sees every step of it.
///
/// A real save observed through the watcher looks like this:
///
/// ```text
/// added    particles/x.particle.tmp.20124.8ed8f24b3cfd
/// modified particles/x.particle.tmp.20124.8ed8f24b3cfd
/// removed  particles/x.particle
/// renamed  ...tmp... -> particles/x.particle
/// ```
///
/// Publishing the first two helps nobody and actively hurts: the scratch file
/// exists for a few milliseconds, it is half-written for most of them, and a
/// consumer that reacted by parsing it would be reading a truncated file. Every
/// consumer would otherwise need this same filter, and the one that forgot would
/// fail rarely and confusingly.
///
/// The `removed` / `renamed` pair still comes through, which is correct: those
/// are about the real file. [`ProjectFileChanged::still_missing`] is what stops
/// the removal half being mistaken for a deletion.
pub fn is_transient(relative: &str) -> bool {
    let name = relative.rsplit('/').next().unwrap_or(relative);
    // `.tmp.` rather than a suffix test: the atomic-write convention puts the
    // real extension first and its own junk last (`x.particle.tmp.<pid>.<hash>`),
    // so the marker is in the middle and the file classifies as whatever
    // trailing garbage it happens to end with.
    name.contains(".tmp.")
        || name.ends_with(".tmp")
        || name.ends_with(".temp")
        // Emacs and gedit backups; vim swap files; VS Code's crash-recovery
        // scratch; partial downloads.
        || name.ends_with('~')
        || name.ends_with(".swp")
        || name.ends_with(".swo")
        || name.ends_with(".swx")
        || name.ends_with(".crswap")
        || name.ends_with(".part")
        || name.ends_with(".crdownload")
        // Emacs auto-save, `#file#`. Its lock file is `.#file`, already dropped
        // by `is_ignored` along with every other dot-prefixed name.
        || (name.starts_with('#') && name.ends_with('#'))
}

/// What the editor itself last wrote to or read from a file.
///
/// The watcher cannot tell "the user edited this in another program" from "we
/// just saved it": both are a write, and both arrive as the same event. Anything
/// that reacts to a change by reloading therefore needs a way to recognise the
/// echo of its own write, or saving a scene makes the editor reload the scene it
/// just saved, throwing away selection and undo for no reason. Worse, if the
/// reload writes anything back, it never stops.
///
/// A content hash rather than a timestamp, and recorded on read as well as
/// write. Timestamps put this at the mercy of clock resolution and of how long a
/// save takes, which is the kind of race that works on one machine and not
/// another. A hash answers the question directly: these are the bytes we believe
/// are on disk, so a file matching them has nothing new in it, whoever wrote it.
#[derive(Resource, Default)]
pub struct SelfWrites {
    hashes: std::collections::HashMap<PathBuf, u64>,
}

impl SelfWrites {
    /// Remember the bytes now believed to be at `path`. Call after writing a
    /// file, and after reading one.
    pub fn record(&mut self, path: &Path, bytes: &[u8]) {
        self.hashes.insert(path.to_path_buf(), hash_bytes(bytes));
    }

    /// Are these the bytes we already know about?
    ///
    /// `false` for a path never recorded, which is the safe answer: an unknown
    /// file has something in it we have not seen.
    pub fn matches(&self, path: &Path, bytes: &[u8]) -> bool {
        self.hashes.get(path) == Some(&hash_bytes(bytes))
    }

    /// Forget a path, so the next change to it counts as external.
    pub fn forget(&mut self, path: &Path) {
        self.hashes.remove(path);
    }
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut h);
    h.finish()
}

/// Where to push a file change so `AssetServer` reloads it.
///
/// # Why this is a closure and not a channel
///
/// Bevy hands a source's watcher an `async_channel::Sender<AssetSourceEvent>`,
/// and that channel is single-consumer: `handle_internal_asset_events` drains it
/// with `try_recv` in a loop, so a second reader would steal events rather than
/// observe them. The only way to both watch the project ourselves and keep asset
/// hot-reload working is to own the watcher and push into Bevy's sender.
///
/// Naming that sender's type here would mean adding `async-channel` to a crate
/// whose whole point is to depend on nothing but Bevy and serde, so the sender
/// is wrapped in a closure at the one place it is available and stored as one.
///
/// # Why it starts empty
///
/// `AssetSourceBuilder::with_watcher` is called once, while `AssetPlugin`
/// builds, and at that moment no project is open and the root is unknown. So the
/// engine's asset source uses that callback only to hand the sender over, and
/// the watcher that actually uses it is created later and rebuilt whenever the
/// project changes.
#[derive(Resource, Clone, Default)]
pub struct AssetReloadSink(Arc<RwLock<Option<Arc<dyn Fn(AssetSourceEvent) + Send + Sync>>>>);

impl AssetReloadSink {
    /// Record how to reach `AssetServer`. Called once, by the asset source.
    pub fn set(&self, send: impl Fn(AssetSourceEvent) + Send + Sync + 'static) {
        if let Ok(mut slot) = self.0.write() {
            *slot = Some(Arc::new(send));
        }
    }

    /// Forward one event, if the sink has been wired up.
    ///
    /// Silent when it has not: a runtime built without a watching asset source
    /// is a valid configuration (a shipped game reads from a `.rpak` and has
    /// nothing to hot-reload), and it should cost nothing rather than warn.
    pub fn send(&self, event: AssetSourceEvent) {
        // Cloned out of the lock before calling: the send itself touches an
        // unbounded channel and must not be holding a lock that the asset
        // source's own thread might want.
        let sink = self.0.read().ok().and_then(|slot| slot.clone());
        if let Some(sink) = sink {
            sink(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenes_are_bsn() {
        assert_eq!(AssetKind::from_path(Path::new("scenes/main.bsn")), AssetKind::Scene);
    }

    #[test]
    fn rust_counts_as_a_script() {
        assert_eq!(AssetKind::from_path(Path::new("scripts/fps.rs")), AssetKind::Script);
    }

    #[test]
    fn extensions_are_case_insensitive() {
        assert_eq!(AssetKind::from_path(Path::new("T.PNG")), AssetKind::Texture);
    }

    #[test]
    fn no_extension_is_other() {
        assert_eq!(AssetKind::from_path(Path::new("LICENSE")), AssetKind::Other);
    }

    #[test]
    fn build_output_is_ignored() {
        assert!(is_ignored("target/dist/thing.rs"));
        assert!(is_ignored("scripts/.hidden/x.rs"));
        assert!(!is_ignored("scripts/fps.rs"));
    }

    #[test]
    fn an_atomic_saves_scratch_file_is_transient() {
        // The exact shape observed from a real save.
        assert!(is_transient(
            "particles/watch_test.particle.tmp.20124.8ed8f24b3cfd"
        ));
        assert!(is_transient("scenes/a.bsn~"));
        assert!(is_transient("x.swp"));
        assert!(is_transient("#notes.txt#"));
    }

    #[test]
    fn a_real_file_is_not_transient() {
        assert!(!is_transient("particles/watch_test.particle"));
        assert!(!is_transient("scripts/spinner.rs"));
        // A directory legitimately called `temp` must not make its contents
        // invisible: the check is on the file name, not the path.
        assert!(!is_transient("temp/real.bsn"));
    }

    #[test]
    fn a_dotfile_name_is_not_a_hidden_directory() {
        // "." alone is a path component the walker produces, not a hidden dir.
        assert!(!is_ignored("."));
    }
}
