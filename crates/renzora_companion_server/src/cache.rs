//! The open project as the companion sees it: its files, its watcher, and the
//! thumbnail cache kept in step with them.
//!
//! The editor asks for a listing of the project when one opens (job
//! `"project_index"`, see `renzora::core::project_index`). The companion
//! answers it here rather than on a worker, because the same request is how it
//! learns which project is open: it keeps the listing, watches the project
//! with a watcher of its own, and from then on:
//!
//! - once the listing lands, every model and material without a current
//!   thumbnail gets one rendered;
//! - a model or material added or changed gets its thumbnail rendered, so the
//!   Assets panel finds it ready instead of asking when the folder opens;
//! - a model or material deleted has its thumbnail deleted with it.
//!
//! Rendering is the render worker's (see `crate::render`), started only while
//! there is something to render. Thumbnails rendered ahead of time wait in a
//! backlog and go to the queue one at a time, only while it is idle: a folder
//! the user opens asks for its thumbnails too, and those must not queue
//! behind a few hundred the user is not looking at. Without a worker (the
//! engine is not beside the companion) nothing is rendered ahead, and the
//! editor renders a thumbnail when a folder asks for it.
//!
//! The editor keeps its own watcher: it feeds the editor's `AssetServer`,
//! which is how hot-reload works, and that has to happen inside the editor.
//! Two watchers over one folder cost two kernel subscriptions, nothing more.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use notify::{RecursiveMode, Watcher};
use renzora_companion_proto::job_args::{MaterialThumbnail, ModelThumbnail, ProjectIndexWalk};
use renzora_companion_proto::project_files::{
    cached_thumbnail, is_ignored, is_transient, walk_project, IndexedFile,
};
use renzora_companion_proto::protocol::{Job, JobId};

use crate::server::Event;

/// Ids of jobs the companion gives itself, kept apart from the editor's
/// (which count up from 1): a report for one of these is not passed on, the
/// editor never asked for it.
pub(crate) const OWN_JOB: JobId = 1 << 62;

/// Files looked at per pass over the backlog: each look is two `stat`s, and a
/// project whose thumbnails are all current empties its backlog in a few
/// passes.
const BACKLOG_STEP: usize = 256;

/// The kind answered here, announced to the editor in the hello.
pub(crate) const KINDS: &[&str] = &["project_index"];

/// Whether a job kind is the project listing, which this module answers.
pub(crate) fn is_listing(kind: &str) -> bool {
    KINDS.contains(&kind)
}

/// How long a burst of changes is collected before acting on it. A save is
/// several events (see `is_transient`), and a folder copied in is hundreds.
pub(crate) const DEBOUNCE: Duration = Duration::from_millis(500);

/// The extensions a model file has.
const MODELS: &[&str] = &["rvg", "glb", "gltf"];

/// A file that has a thumbnail, by the thumbnail cache's folder for it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Thumbnailed {
    Model,
    Material,
}

impl Thumbnailed {
    fn folder(self) -> &'static str {
        match self {
            Thumbnailed::Model => "models",
            Thumbnailed::Material => "materials",
        }
    }
}

#[derive(Default)]
pub(crate) struct ProjectCache {
    /// The open project, as the editor last said.
    pub root: Option<PathBuf>,
    /// Every file, from the walk, kept current from the watcher.
    pub files: BTreeMap<String, IndexedFile>,
    /// Listing requests waiting for the walk.
    asked: Vec<JobId>,
    /// Out on a thread, for this root.
    walking: Option<PathBuf>,
    /// Changes acted on while the walk was out: the walk read the folder as
    /// it was, so these are looked at again when it lands.
    since_walk: BTreeSet<String>,
    watcher: Option<notify::RecommendedWatcher>,
    /// Why the project could not be watched, for the Cache screen.
    pub watch_error: Option<String>,
    /// Project-relative paths the watcher reported, waiting for the burst to
    /// end, and when the last one came.
    changed: BTreeSet<String>,
    last_change: Option<Instant>,
    /// Thumbnail cache: files and bytes, from the last measure.
    pub thumbnails: Option<(usize, u64)>,
    /// Thumbnails this session deleted with their file.
    pub pruned: usize,
    /// Whether thumbnails are rendered ahead of time: only with a render
    /// worker to render them. Set by the server.
    pub render_ahead: bool,
    /// Files still to look at for a missing thumbnail.
    pub backlog: VecDeque<String>,
    /// Thumbnails this session queued ahead of time.
    pub prewarmed: usize,
    next_job: JobId,
    /// Thumbnail jobs for the server to queue.
    to_queue: Vec<(JobId, Job)>,
}

impl ProjectCache {
    /// The editor asked for a listing (and so said which project is open).
    /// `Ok(Some)` is the answer now; `Ok(None)` means it comes when the walk
    /// lands, from [`Self::walked`].
    pub(crate) fn ask(&mut self, id: JobId, payload: &str, events: &Sender<Event>) -> Result<Option<String>, String> {
        let args: ProjectIndexWalk =
            serde_json::from_str(payload).map_err(|e| format!("could not read the job's arguments: {e}"))?;
        if self.root.as_ref() != Some(&args.root) {
            self.open(args.root, events);
        } else if self.walking.is_none() {
            // Same project, asked again (an editor reconnect): what is known
            // already is current, since the watcher has kept it so.
            return self.listing().map(Some);
        }
        self.asked.push(id);
        Ok(None)
    }

    fn listing(&self) -> Result<String, String> {
        serde_json::to_string(&self.files.values().collect::<Vec<_>>()).map_err(|e| e.to_string())
    }

    fn open(&mut self, root: PathBuf, events: &Sender<Event>) {
        self.files.clear();
        self.backlog.clear();
        self.changed.clear();
        self.since_walk.clear();
        self.last_change = None;
        self.thumbnails = None;

        let walk_root = root.clone();
        let tx = events.clone();
        let _ = std::thread::Builder::new().name("companion-project-walk".into()).spawn(move || {
            let files = walk_project(&walk_root);
            let _ = tx.send(Event::Walked(walk_root, files));
        });
        self.walking = Some(root.clone());

        // Dropping the old watcher stops it. The callback filters what nobody
        // wants before it reaches the channel: a build writing into `target/`
        // is thousands of events a second.
        self.watcher = None;
        let watch_root = root.clone();
        let tx = events.clone();
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            let Ok(event) = event else { return };
            let changed: Vec<String> = event
                .paths
                .iter()
                .filter_map(|p| p.strip_prefix(&watch_root).ok())
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .filter(|rel| !rel.is_empty() && !is_ignored(rel) && !is_transient(rel))
                .collect();
            if !changed.is_empty() {
                let _ = tx.send(Event::Changed(watch_root.clone(), changed));
            }
        })
        .and_then(|mut w| w.watch(&root, RecursiveMode::Recursive).map(|()| w));
        match watcher {
            Ok(w) => {
                self.watcher = Some(w);
                self.watch_error = None;
                crate::log::info(format!("watching {}", root.display()));
            }
            Err(e) => {
                crate::log::warn(format!("cannot watch {}: {e}", root.display()));
                self.watch_error = Some(e.to_string());
            }
        }
        self.root = Some(root);
        self.measure(events);
    }

    /// A walk landed. Answers every request waiting for it.
    pub(crate) fn walked(&mut self, root: &Path, files: Vec<IndexedFile>) -> Vec<(JobId, Result<String, String>)> {
        if self.walking.as_deref() != Some(root) || self.root.as_deref() != Some(root) {
            return Vec::new();
        }
        self.walking = None;
        self.files = files.into_iter().map(|f| (f.path.clone(), f)).collect();
        for relative in std::mem::take(&mut self.since_walk) {
            self.restat(&relative);
        }
        self.fill_backlog();
        let answer = self.listing();
        std::mem::take(&mut self.asked).into_iter().map(|id| (id, answer.clone())).collect()
    }

    /// The watcher saw these change under `root`.
    pub(crate) fn changed(&mut self, root: &Path, paths: Vec<String>) {
        if self.root.as_deref() != Some(root) {
            return;
        }
        self.changed.extend(paths);
        self.last_change = Some(Instant::now());
    }

    /// When the current burst of changes is due to be acted on, if one is
    /// waiting.
    pub(crate) fn due(&self) -> Option<Instant> {
        self.last_change.filter(|_| !self.changed.is_empty()).map(|t| t + DEBOUNCE)
    }

    /// Act on a burst of changes once it has gone quiet. Each changed path is
    /// looked at as it is now, which is simpler and more reliable than reading
    /// the watcher's account of what happened: a rename arrives as two events
    /// on some systems and one on others, and a folder moved in arrives as one
    /// event for the folder and none for what is inside it.
    pub(crate) fn settle(&mut self, events: &Sender<Event>) {
        if self.due().is_none_or(|due| Instant::now() < due) {
            return;
        }
        self.last_change = None;
        let mut thumbnails_changed = false;
        for relative in std::mem::take(&mut self.changed) {
            if self.walking.is_some() {
                self.since_walk.insert(relative.clone());
            }
            thumbnails_changed |= self.restat(&relative);
        }
        if thumbnails_changed {
            self.measure(events);
        }
    }

    /// Bring one path up to date in the listing. True when a thumbnail was
    /// deleted with its file.
    fn restat(&mut self, relative: &str) -> bool {
        let Some(root) = self.root.clone() else { return false };
        let absolute = root.join(relative);
        if let Some(file) = IndexedFile::stat(relative.to_string(), &absolute) {
            self.files.insert(relative.to_string(), file);
            // Added or changed: its thumbnail, if it has one, is out of date.
            self.prewarm(relative);
            return false;
        }
        if absolute.is_dir() {
            // A folder that arrived whole: list what is in it.
            for mut file in walk_project(&absolute) {
                file.path = format!("{relative}/{}", file.path);
                self.files.insert(file.path.clone(), file);
            }
            return false;
        }
        // Gone: the file, or a folder and everything that was under it.
        let prefix = format!("{relative}/");
        let gone: Vec<String> =
            self.files.keys().filter(|k| *k == relative || k.starts_with(&prefix)).cloned().collect();
        let mut pruned = false;
        for path in gone {
            self.files.remove(&path);
            pruned |= self.prune(&root, &path);
        }
        // Not in the listing (it never was, or the walk has not landed), but
        // its thumbnail may still be on disk.
        pruned || self.prune(&root, relative)
    }

    /// Delete the thumbnail of a file that was removed.
    fn prune(&mut self, root: &Path, relative: &str) -> bool {
        let Some(kind) = thumbnailed(relative) else { return false };
        // A sidecar's thumbnail is its model's: the two share a cache path.
        // Deleting it when the sidecar goes (a model's virtual geometry being
        // rebuilt) would take the model's thumbnail with it.
        if is_sidecar(relative, |sibling| root.join(sibling).exists()) {
            return false;
        }
        let removed = std::fs::remove_file(cached_thumbnail(root, relative, kind.folder())).is_ok();
        if removed {
            self.pruned += 1;
        }
        removed
    }

    /// Queue a thumbnail for `relative` if it has one and its cached one is
    /// missing or older than the file. True when one was queued.
    fn prewarm(&mut self, relative: &str) -> bool {
        if !self.render_ahead {
            return false;
        }
        let Some(kind) = thumbnailed(relative) else { return false };
        let Some(root) = self.root.clone() else { return false };
        if is_sidecar(relative, |sibling| root.join(sibling).exists()) {
            return false;
        }
        let file = root.join(relative);
        let png = cached_thumbnail(&root, relative, kind.folder());
        let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        if modified(&png).zip(modified(&file)).is_some_and(|(png, file)| png >= file) {
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
        self.to_queue.push((OWN_JOB | self.next_job, Job::Run { kind: job_kind.into(), payload, target }));
        self.prewarmed += 1;
        true
    }

    /// Look at every listed file for a missing thumbnail, a few at a time
    /// (see [`Self::drain_backlog`]).
    fn fill_backlog(&mut self) {
        if !self.render_ahead {
            return;
        }
        let files: Vec<String> = self.files.keys().filter(|k| thumbnailed(k).is_some()).cloned().collect();
        self.backlog.extend(files);
    }

    /// The Cache screen's "Render missing thumbnails".
    pub(crate) fn render_missing(&mut self) {
        self.backlog.clear();
        self.fill_backlog();
    }

    /// Feed the backlog to the queue while it has nothing else to do: queue
    /// the next missing thumbnail, skipping current ones on the way.
    pub(crate) fn drain_backlog(&mut self, queue_idle: bool, events: &Sender<Event>) {
        if self.backlog.is_empty() || !queue_idle {
            return;
        }
        for _ in 0..BACKLOG_STEP {
            let Some(relative) = self.backlog.pop_front() else { break };
            if self.prewarm(&relative) {
                break;
            }
        }
        if self.backlog.is_empty() {
            self.measure(events);
        }
    }

    /// Thumbnail jobs to queue, from changes and the backlog.
    pub(crate) fn take_jobs(&mut self) -> Vec<(JobId, Job)> {
        std::mem::take(&mut self.to_queue)
    }

    /// Delete every cached thumbnail. The editor holds those it loaded in
    /// memory and keeps showing them; it renders them again next session or
    /// when a model changes.
    pub(crate) fn clear_thumbnails(&mut self, events: &Sender<Event>) {
        if let Some(root) = &self.root {
            let _ = std::fs::remove_dir_all(thumbnails_dir(root));
        }
        self.measure(events);
    }

    /// Re-measure the thumbnail cache, off this thread.
    pub(crate) fn measure(&mut self, events: &Sender<Event>) {
        let Some(root) = self.root.clone() else { return };
        let tx = events.clone();
        let _ = std::thread::Builder::new().name("companion-cache-measure".into()).spawn(move || {
            let size = folder_size(&thumbnails_dir(&root));
            let _ = tx.send(Event::Measured(root, size));
        });
    }

    pub(crate) fn measured(&mut self, root: &Path, size: (usize, u64)) {
        if self.root.as_deref() == Some(root) {
            self.thumbnails = Some(size);
        }
    }

    /// Whether the first walk of the open project is still out.
    pub(crate) fn is_walking(&self) -> bool {
        self.walking.is_some()
    }
}

/// What kind of thumbnail `relative` has, if it has one.
pub(crate) fn thumbnailed(relative: &str) -> Option<Thumbnailed> {
    let lower = relative.to_ascii_lowercase();
    if lower.ends_with(".material") {
        return Some(Thumbnailed::Material);
    }
    let model = MODELS.iter().any(|ext| lower.ends_with(&format!(".{ext}")))
        // The original a rig was built from is kept beside the model; it is
        // not shown anywhere and has no thumbnail.
        && !lower.contains(".unrigged.")
        && !lower.contains(".unshaped.");
    model.then_some(Thumbnailed::Model)
}

/// An `.rvg` kept beside a `.glb` (or `.gltf`) of the same name: that model's
/// virtual geometry, with no scene of its own. The `.glb` is the model and has
/// the thumbnail. The Assets panel hides the same files by the same rule.
/// `sibling_exists` takes a project-relative path.
pub(crate) fn is_sidecar(relative: &str, sibling_exists: impl Fn(&str) -> bool) -> bool {
    let lower = relative.to_ascii_lowercase().replace('\\', "/");
    if !lower.ends_with(".rvg") {
        return false;
    }
    // A terrain chunk's clusters (`renzora_terrain::clusters`): no scene, so
    // no thumbnail.
    if lower.starts_with("terrain_clusters/") {
        return true;
    }
    let stem = &relative[..relative.len() - ".rvg".len()];
    sibling_exists(&format!("{stem}.glb")) || sibling_exists(&format!("{stem}.gltf"))
}

/// The whole thumbnail cache: the folder every kind's folder
/// (`thumbnail_cache_dir`) is in. Spelled out rather than derived from a
/// kind's folder, because Clear deletes it: a derivation that went wrong
/// could name the project itself.
fn thumbnails_dir(root: &Path) -> PathBuf {
    root.join(".cache").join("thumbnails")
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

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rz-server-cache-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(path: &Path, bytes: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

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

    /// Clear deletes this folder, so it must be the one the thumbnails are in
    /// and nothing above it.
    #[test]
    fn the_cache_folder_holds_every_thumbnail_and_is_inside_the_project() {
        let root = Path::new("/p");
        let dir = thumbnails_dir(root);
        assert!(cached_thumbnail(root, "models/car.glb", "models").starts_with(&dir));
        assert!(cached_thumbnail(root, "rock.material", "materials").starts_with(&dir));
        assert_eq!(dir, Path::new("/p/.cache/thumbnails"));
    }

    #[test]
    fn a_folder_is_measured_through_its_subfolders() {
        let dir = temp("size");
        write(&dir.join("a.png"), &[0u8; 10]);
        write(&dir.join("models/car/car.png"), &[0u8; 32]);
        assert_eq!(folder_size(&dir), (2, 42));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The listing answers once the walk lands, and a second ask of the same
    /// project is answered at once from what is known.
    #[test]
    fn a_listing_is_answered_from_the_walk_then_from_memory() {
        let root = temp("listing");
        write(&root.join("models/car.rvg"), b"x");
        let (events, rx) = std::sync::mpsc::channel();
        let mut cache = ProjectCache::default();
        let payload = serde_json::to_string(&ProjectIndexWalk { root: root.clone() }).unwrap();
        assert_eq!(cache.ask(1, &payload, &events), Ok(None));
        let answers = loop {
            match rx.recv_timeout(Duration::from_secs(5)).expect("the walk landed") {
                Event::Walked(r, files) => break cache.walked(&r, files),
                _ => continue,
            }
        };
        assert_eq!(answers.len(), 1);
        let files: Vec<IndexedFile> = serde_json::from_str(answers[0].1.as_ref().unwrap()).unwrap();
        assert_eq!(files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["models/car.rvg"]);
        assert!(cache.ask(2, &payload, &events).unwrap().is_some(), "answered from memory");
        drop(cache);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A deleted model takes its thumbnail with it, but a deleted sidecar
    /// leaves its model's alone, and a folder deleted takes everything in it
    /// out of the listing.
    #[test]
    fn a_removed_model_takes_its_thumbnail_and_a_sidecar_does_not() {
        let root = temp("prune");
        let (events, _rx) = std::sync::mpsc::channel();
        let mut cache = ProjectCache { root: Some(root.clone()), ..Default::default() };
        write(&root.join("models/rock.glb"), b"x");
        let rock = cached_thumbnail(&root, "models/rock.glb", "models");
        let car = cached_thumbnail(&root, "models/car/car.rvg", "models");
        write(&rock, b"png");
        write(&car, b"png");
        for path in ["models/rock.glb", "models/rock.rvg", "models/car/car.rvg", "models/car/paint.png"] {
            cache.files.insert(path.into(), IndexedFile { path: path.into(), size: 1, modified: 1 });
        }

        // The sidecar went; the model is still there.
        cache.changed(&root, vec!["models/rock.rvg".into()]);
        cache.last_change = Some(Instant::now() - DEBOUNCE);
        cache.settle(&events);
        assert!(rock.exists(), "the model's thumbnail stays");
        assert!(!cache.files.contains_key("models/rock.rvg"));

        // A whole folder went.
        cache.changed(&root, vec!["models/car".into()]);
        cache.last_change = Some(Instant::now() - DEBOUNCE);
        cache.settle(&events);
        assert!(!car.exists(), "the model's thumbnail went with it");
        assert_eq!(cache.files.keys().collect::<Vec<_>>(), ["models/rock.glb"]);
        assert_eq!(cache.pruned, 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The backlog queues a thumbnail for a model without a current one, one
    /// at a time and only while the queue is idle; a current thumbnail, a
    /// sidecar, and a companion with no render worker queue nothing.
    #[test]
    fn missing_thumbnails_are_queued_one_at_a_time_while_idle() {
        let root = temp("backlog");
        let (events, _rx) = std::sync::mpsc::channel();
        for path in ["models/a.glb", "models/a.rvg", "models/b.glb", "models/c.glb"] {
            write(&root.join(path), b"x");
        }
        // c.glb's thumbnail is newer than the model.
        std::thread::sleep(Duration::from_millis(20));
        write(&cached_thumbnail(&root, "models/c.glb", "models"), b"png");
        let mut cache = ProjectCache { root: Some(root.clone()), render_ahead: true, ..Default::default() };
        for path in ["models/a.glb", "models/a.rvg", "models/b.glb", "models/c.glb"] {
            cache.files.insert(path.into(), IndexedFile { path: path.into(), size: 1, modified: 1 });
        }
        cache.fill_backlog();

        cache.drain_backlog(false, &events);
        assert!(cache.take_jobs().is_empty(), "the queue is busy");
        let mut targets = Vec::new();
        for _ in 0..5 {
            cache.drain_backlog(true, &events);
            for (id, job) in cache.take_jobs() {
                assert_ne!(id & OWN_JOB, 0);
                targets.push(job.target());
            }
        }
        assert_eq!(targets, ["a.glb", "b.glb"]);

        let mut without_worker = ProjectCache { root: Some(root.clone()), ..Default::default() };
        without_worker.files = cache.files.clone();
        without_worker.render_missing();
        without_worker.drain_backlog(true, &events);
        assert!(without_worker.take_jobs().is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A burst is held until it goes quiet.
    #[test]
    fn changes_wait_for_the_burst_to_end() {
        let root = temp("debounce");
        let (events, _rx) = std::sync::mpsc::channel();
        let mut cache = ProjectCache { root: Some(root.clone()), ..Default::default() };
        write(&root.join("a.png"), b"x");
        cache.changed(&root, vec!["a.png".into()]);
        cache.settle(&events);
        assert!(cache.files.is_empty(), "not yet");
        cache.last_change = Some(Instant::now() - DEBOUNCE);
        cache.settle(&events);
        assert!(cache.files.contains_key("a.png"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
