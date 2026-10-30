//! The open project as the companion sees it: its files, its watcher, and the
//! caches the companion keeps in step with them.
//!
//! The editor asks for a listing of the project when one opens (job
//! `"project_index"`, see `renzora::core::project_index`). The companion
//! answers it from here rather than from a worker, because the same request
//! is how it learns which project is open: it keeps the listing, watches the
//! project with a watcher of its own, and from then on:
//!
//! - once the listing lands, every model and material without a current
//!   thumbnail gets one rendered. Loading a project used to render the
//!   materials' thumbnails behind the splash; now the project opens straight
//!   away and they fill in here afterwards;
//! - a model or material added or changed gets its thumbnail rendered
//!   straight away, so the Assets panel finds it ready instead of asking for
//!   it when the folder is opened;
//! - a model or material deleted has its thumbnail deleted with it;
//! - the Cache screen shows what the thumbnail cache holds, and can clear it
//!   or fill it.
//!
//! Thumbnails rendered ahead of time wait in a backlog and go to the queue
//! one at a time, only while it is idle: a folder the user opens asks the
//! companion for its thumbnails too, and those must not queue behind a few
//! hundred the user is not looking at.
//!
//! The editor keeps its own watcher: it feeds the editor's `AssetServer`,
//! which is how hot-reload works, and that has to happen inside the editor.
//! Two watchers over one folder cost two kernel subscriptions, nothing more.
//!
//! Thumbnails rendered here are ordinary rendering jobs in the companion's
//! queue (see `render`), so they wait while a game plays or a profile records
//! like any other.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;
use std::time::Duration;

use bevy::asset::io::AssetSourceEvent;
use bevy::prelude::*;
use renzora::core::project_index::{walk_project, IndexedFile};
use renzora::job_args::{MaterialThumbnail, ModelThumbnail, ProjectIndexWalk};

use super::jobs::JobQueue;
use crate::protocol::{Job, JobId};

/// Ids of jobs the companion gives itself, kept apart from the editor's: a
/// report for one of these is not forwarded (the editor never asked).
pub(crate) const OWN_JOB: JobId = 1 << 62;

/// The extensions a model file has.
const MODELS: &[&str] = &["rvg", "glb", "gltf"];

/// A file that has a thumbnail, by what renders it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Thumbnailed {
    Model,
    Material,
}

/// How long a burst of changes is collected before acting on it.
const DEBOUNCE: Duration = Duration::from_millis(500);

/// A listing request from the editor, waiting for its walk.
struct Asked {
    id: JobId,
    root: PathBuf,
}

#[derive(Resource, Default)]
pub(crate) struct ProjectCache {
    /// The open project, as the editor last said.
    pub root: Option<PathBuf>,
    project: Option<renzora::core::CurrentProject>,
    /// Every file, from the walk, kept current from the watcher.
    pub files: BTreeMap<String, IndexedFile>,
    asked: Vec<Asked>,
    walk: Option<(PathBuf, Mutex<Receiver<Vec<IndexedFile>>>)>,
    _watcher: Option<bevy::asset::io::file::FileWatcher>,
    events: Option<async_channel::Receiver<AssetSourceEvent>>,
    next_job: JobId,
    /// Thumbnail cache: files and bytes, from the last measure.
    pub thumbnails: Option<(usize, u64)>,
    measuring: Option<Mutex<Receiver<(usize, u64)>>>,
    /// Thumbnails this session rendered ahead of time, and deleted with
    /// their file.
    pub prewarmed: usize,
    pub pruned: usize,
    /// Files still to check for a missing thumbnail (see the module doc).
    pub backlog: VecDeque<String>,
}

/// What the Cache screen asks for.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CacheRequest {
    ClearThumbnails,
    RenderMissing,
}

pub(crate) fn build(app: &mut App) {
    app.init_resource::<ProjectCache>()
        .add_message::<CacheRequest>()
        .add_systems(Update, (finish_walk, follow_changes, handle_requests, drain_backlog, finish_measure).chain());
}

/// Whether a job kind is the project listing, which this module answers.
pub(crate) fn is_listing(kind: &str) -> bool {
    kind == "project_index"
}

impl ProjectCache {
    /// The editor asked for a listing (and so said which project is open).
    pub(crate) fn ask(&mut self, id: JobId, payload: &str) -> Result<(), String> {
        let args: ProjectIndexWalk =
            serde_json::from_str(payload).map_err(|e| format!("could not read the job's arguments: {e}"))?;
        self.asked.push(Asked { id, root: args.root.clone() });
        if self.root.as_ref() != Some(&args.root) {
            self.open(args.root);
        } else if self.walk.is_none() {
            // Same project, asked again (an editor reconnect): answer from
            // what is already known on the next frame.
            let files: Vec<IndexedFile> = self.files.values().cloned().collect();
            let (tx, rx) = channel();
            let _ = tx.send(files);
            self.walk = Some((args.root, Mutex::new(rx)));
        }
        Ok(())
    }

    fn open(&mut self, root: PathBuf) {
        self.project = renzora::core::session::open_project(&root.join("project.toml")).ok();
        self.files.clear();
        self.backlog.clear();
        self.thumbnails = None;
        let (tx, rx) = channel();
        let walk_root = root.clone();
        let _ = std::thread::Builder::new().name("companion-project-walk".into()).spawn(move || {
            let _ = tx.send(walk_project(&walk_root));
        });
        self.walk = Some((root.clone(), Mutex::new(rx)));
        let (sender, receiver) = async_channel::unbounded();
        match bevy::asset::io::file::FileWatcher::new(root.clone(), sender, DEBOUNCE) {
            Ok(w) => {
                self._watcher = Some(w);
                self.events = Some(receiver);
                info!("[companion] watching {}", root.display());
            }
            Err(e) => {
                warn!("[companion] cannot watch {}: {e}", root.display());
                self._watcher = None;
                self.events = None;
            }
        }
        self.root = Some(root);
        self.measure();
    }

    /// Re-measure the thumbnail cache, off this thread.
    fn measure(&mut self) {
        let Some(dir) = self.thumbnail_dir() else { return };
        let (tx, rx) = channel();
        let _ = std::thread::Builder::new().name("companion-cache-measure".into()).spawn(move || {
            let _ = tx.send(folder_size(&dir));
        });
        self.measuring = Some(Mutex::new(rx));
    }

    fn thumbnail_dir(&self) -> Option<PathBuf> {
        self.root.as_ref().map(|r| r.join(".cache").join("thumbnails"))
    }

    /// The cached thumbnail for a file, by the editor's own rule.
    fn thumbnail_of(&self, kind: Thumbnailed, file: &Path) -> Option<PathBuf> {
        self.project.as_ref().map(|p| match kind {
            Thumbnailed::Model => renzora_editor_framework::model_thumb_path(file, p),
            Thumbnailed::Material => renzora_editor_framework::material_thumb_path(file, p),
        })
    }

    /// Queue a thumbnail for `relative` unless it has none or its cached one
    /// is newer than the file.
    fn prewarm(&mut self, queue: &mut JobQueue, relative: &str, force: bool) -> bool {
        let Some(kind) = thumbnailed(relative) else { return false };
        let Some(root) = self.root.clone() else { return false };
        if is_sidecar(relative, |sibling| root.join(sibling).exists()) {
            return false;
        }
        let file = root.join(relative);
        let Some(png) = self.thumbnail_of(kind, &file) else { return false };
        let fresh = !force
            && std::fs::metadata(&png)
                .and_then(|p| p.modified())
                .ok()
                .zip(std::fs::metadata(&file).and_then(|m| m.modified()).ok())
                .is_some_and(|(p, m)| p >= m);
        if fresh {
            return false;
        }
        let (job_kind, payload) = match kind {
            Thumbnailed::Model => ("model_thumbnail", serde_json::to_string(&ModelThumbnail { project: root, model: file })),
            Thumbnailed::Material => {
                ("material_thumbnail", serde_json::to_string(&MaterialThumbnail { project: root, material: file }))
            }
        };
        let Ok(payload) = payload else { return false };
        self.next_job += 1;
        let target = relative.rsplit('/').next().unwrap_or(relative).to_string();
        queue.submit(OWN_JOB | self.next_job, Job::Run { kind: job_kind.into(), payload, target });
        self.prewarmed += 1;
        true
    }

    /// Delete the thumbnail of a file that was removed.
    fn prune(&mut self, relative: &str) -> bool {
        let Some(kind) = thumbnailed(relative) else { return false };
        let Some(png) = self.root.as_ref().and_then(|root| self.thumbnail_of(kind, &root.join(relative))) else {
            return false;
        };
        let removed = std::fs::remove_file(&png).is_ok();
        if removed {
            self.pruned += 1;
        }
        removed
    }

    /// Check every listed file for a missing thumbnail, a few at a time.
    fn fill_backlog(&mut self) {
        let files: Vec<String> = self.files.keys().filter(|k| thumbnailed(k).is_some()).cloned().collect();
        self.backlog.extend(files);
    }
}

/// What renders the thumbnail of `relative`, if it has one.
pub(crate) fn thumbnailed(relative: &str) -> Option<Thumbnailed> {
    let lower = relative.to_ascii_lowercase();
    if lower.ends_with(".material") {
        return Some(Thumbnailed::Material);
    }
    let model = MODELS.iter().any(|ext| lower.ends_with(&format!(".{ext}")))
        // The original a rig was built from is kept beside the model; it is
        // not shown anywhere and needs no thumbnail.
        && !lower.contains(".unrigged.")
        && !lower.contains(".unshaped.");
    model.then_some(Thumbnailed::Model)
}

/// An `.rvg` kept beside a `.glb` (or `.gltf`) of the same name: that model's
/// virtual geometry, with no scene of its own, which the virtual geometry
/// converter writes for every `.glb` in a project. The `.glb` is the model and
/// gets the thumbnail; rendering the sidecar found an empty model and warned
/// that it "has no scenes", once for every `.glb` added. The Assets panel
/// hides the same files by the same rule. `sibling_exists` takes a
/// project-relative path.
pub(crate) fn is_sidecar(relative: &str, sibling_exists: impl Fn(&str) -> bool) -> bool {
    let lower = relative.to_ascii_lowercase().replace('\\', "/");
    if !lower.ends_with(".rvg") {
        return false;
    }
    // A terrain chunk's clusters (`renzora_terrain::clusters`): no scene, so
    // nothing to draw a thumbnail of.
    if lower.starts_with("terrain_clusters/") {
        return true;
    }
    let stem = &relative[..relative.len() - ".rvg".len()];
    sibling_exists(&format!("{stem}.glb")) || sibling_exists(&format!("{stem}.gltf"))
}

/// Answer every listing request once its walk lands.
fn finish_walk(mut cache: ResMut<ProjectCache>, mut queue: ResMut<JobQueue>) {
    let landed = cache
        .walk
        .as_ref()
        .and_then(|(root, rx)| rx.lock().ok().and_then(|rx| rx.try_recv().ok()).map(|files| (root.clone(), files)));
    let Some((root, files)) = landed else { return };
    cache.walk = None;
    if cache.root.as_ref() != Some(&root) {
        return;
    }
    cache.files = files.iter().map(|f| (f.path.clone(), f.clone())).collect();
    // Asked again after a reconnect: the backlog holds what is left of the
    // first fill, and a full one would only check fresh files again.
    if cache.backlog.is_empty() {
        cache.fill_backlog();
    }
    let answer = serde_json::to_string(&files).map_err(|e| e.to_string());
    for asked in std::mem::take(&mut cache.asked) {
        if asked.root == root {
            queue.answer(asked.id, answer.clone());
        } else {
            cache.asked.push(asked);
        }
    }
}

/// Keep the listing and the thumbnails in step with the project.
fn follow_changes(mut cache: ResMut<ProjectCache>, mut queue: ResMut<JobQueue>) {
    let Some(events) = cache.events.as_ref() else { return };
    let arrived: Vec<AssetSourceEvent> = std::iter::from_fn(|| events.try_recv().ok()).collect();
    if arrived.is_empty() {
        return;
    }
    let Some(root) = cache.root.clone() else { return };
    let rel = |p: &Path| p.to_string_lossy().replace('\\', "/");
    let mut thumbnails_changed = false;
    for event in arrived {
        match event {
            AssetSourceEvent::AddedAsset(p) | AssetSourceEvent::ModifiedAsset(p) => {
                let relative = rel(&p);
                if renzora::core::is_ignored(&relative) {
                    continue;
                }
                if let Some(file) = IndexedFile::stat(relative.clone(), &root.join(&p)) {
                    cache.files.insert(relative.clone(), file);
                }
                if cache.prewarm(&mut queue, &relative, false) {
                    thumbnails_changed = true;
                }
            }
            AssetSourceEvent::RemovedAsset(p) => {
                let relative = rel(&p);
                cache.files.remove(&relative);
                if cache.prune(&relative) {
                    thumbnails_changed = true;
                }
            }
            AssetSourceEvent::RenamedAsset { old, new } => {
                let (old, new) = (rel(&old), rel(&new));
                cache.files.remove(&old);
                if let Some(file) = IndexedFile::stat(new.clone(), &root.join(&new)) {
                    cache.files.insert(new.clone(), file);
                }
                if cache.prewarm(&mut queue, &new, false) {
                    thumbnails_changed = true;
                }
            }
            _ => {}
        }
    }
    if thumbnails_changed {
        cache.measure();
    }
}

/// The Cache screen's buttons.
fn handle_requests(mut requests: MessageReader<CacheRequest>, mut cache: ResMut<ProjectCache>) {
    for request in requests.read() {
        match request {
            CacheRequest::ClearThumbnails => {
                if let Some(dir) = cache.thumbnail_dir() {
                    // The editor holds thumbnails it loaded in memory and
                    // keeps showing them; it asks again, and gets them
                    // rendered again, next session or when a model changes.
                    let _ = std::fs::remove_dir_all(&dir);
                }
                cache.measure();
            }
            CacheRequest::RenderMissing => {
                cache.backlog.clear();
                cache.fill_backlog();
            }
        }
    }
}

/// Feed the backlog to the queue while the queue has nothing else to do.
/// Fresh files are skipped on the spot, so a project whose thumbnails are all
/// current empties its backlog within a frame or two.
fn drain_backlog(mut cache: ResMut<ProjectCache>, mut queue: ResMut<JobQueue>) {
    if cache.backlog.is_empty() || !queue.idle() {
        return;
    }
    // A bound on the files checked per frame: each check is two `stat`s.
    for _ in 0..256 {
        let Some(relative) = cache.backlog.pop_front() else { break };
        if cache.prewarm(&mut queue, &relative, false) {
            break;
        }
    }
    if cache.backlog.is_empty() {
        cache.measure();
    }
}

fn finish_measure(mut cache: ResMut<ProjectCache>) {
    let measured = cache.measuring.as_ref().and_then(|rx| rx.lock().ok().and_then(|rx| rx.try_recv().ok()));
    if let Some(size) = measured {
        cache.thumbnails = Some(size);
        cache.measuring = None;
    }
}

/// Files and bytes under `dir`.
fn folder_size(dir: &Path) -> (usize, u64) {
    let (mut count, mut bytes) = (0, 0);
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for entry in entries.flatten() {
            match entry.metadata() {
                Ok(m) if m.is_dir() => stack.push(entry.path()),
                Ok(m) => {
                    count += 1;
                    bytes += m.len();
                }
                Err(_) => {}
            }
        }
    }
    (count, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbnailed_files_are_known_by_extension_and_rig_originals_are_not() {
        assert_eq!(thumbnailed("models/car/car.rvg"), Some(Thumbnailed::Model));
        assert_eq!(thumbnailed("a/B.GLB"), Some(Thumbnailed::Model));
        assert_eq!(thumbnailed("materials/rock.material"), Some(Thumbnailed::Material));
        assert_eq!(thumbnailed("models/car/textures/paint.png"), None);
        assert_eq!(thumbnailed("models/hero/hero.unrigged.rvg"), None);
    }

    #[test]
    fn only_an_rvg_beside_its_glb_is_a_sidecar() {
        let folder = ["models/boulder_01.glb", "models/boulder_01.rvg", "models/statue.rvg", "models/scene.gltf"];
        let exists = |p: &str| folder.contains(&p);
        assert!(is_sidecar("models/boulder_01.rvg", exists));
        assert!(is_sidecar("models/scene.RVG", exists));
        assert!(!is_sidecar("models/statue.rvg", exists), "a model stored as .rvg");
        assert!(!is_sidecar("models/boulder_01.glb", exists), "the model itself");
    }

    #[test]
    fn the_backlog_holds_every_thumbnailed_file_and_nothing_else() {
        let mut cache = ProjectCache::default();
        for path in ["a.rvg", "b.material", "c.png", "d.unrigged.glb"] {
            cache.files.insert(path.into(), IndexedFile { path: path.into(), size: 0, modified: 0 });
        }
        cache.fill_backlog();
        assert_eq!(cache.backlog, ["a.rvg", "b.material"]);
    }

    #[test]
    fn a_folder_is_measured_through_its_subfolders() {
        let dir = std::env::temp_dir().join(format!("rz-cache-size-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("models/car")).unwrap();
        std::fs::write(dir.join("a.png"), [0u8; 10]).unwrap();
        std::fs::write(dir.join("models/car/car.png"), [0u8; 32]).unwrap();
        assert_eq!(folder_size(&dir), (2, 42));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
