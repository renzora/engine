//! Every file in the open project, kept current, for anyone to read.
//!
//! Panels used to find files by walking the project themselves: the Animation
//! workspace looked for models every four seconds (79 ms on the main thread in
//! a project with a model library), material thumbnails walked for
//! `.material` files, and so on, each with its own folders to skip. A walk is
//! the slow part of all of them, and the answer is the same for everyone.
//!
//! So the project is walked once, off any frame: by the companion process when
//! it is connected (job `"project_index"`), otherwise on a thread of the
//! editor's own. After that the index follows the project watcher's
//! [`ProjectFileChanged`](super::ProjectFileChanged) events, which carry every
//! change already. Reading it costs a map lookup; nobody walks a folder.
//!
//! What counts as a project file is decided by the watcher's rules
//! ([`is_ignored`], [`is_transient`]), so the index and the watcher never
//! disagree about what exists.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::project_files::{is_ignored, is_transient};

/// One file: where, how big, when it last changed.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct IndexedFile {
    /// Project-relative, forward-slashed: the key every other producer in
    /// the engine uses (see `ProjectFileChanged::relative`).
    pub path: String,
    pub size: u64,
    /// Seconds since the Unix epoch.
    pub modified: u64,
}

impl IndexedFile {
    /// The file's extension, lower-case, without the dot.
    pub fn extension(&self) -> String {
        self.path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default()
    }

    /// The file name, without its folder.
    pub fn file_name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    /// Read `absolute`'s size and modification time. `None` for a folder or
    /// a file that is gone.
    pub fn stat(relative: String, absolute: &Path) -> Option<Self> {
        let meta = std::fs::metadata(absolute).ok()?;
        if !meta.is_file() {
            return None;
        }
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());
        Some(Self { path: relative, size: meta.len(), modified })
    }
}

/// The open project's files. Empty and not [`ready`](Self::ready) until the
/// first walk lands.
#[derive(Resource, Default)]
pub struct ProjectIndex {
    root: Option<PathBuf>,
    files: BTreeMap<String, IndexedFile>,
    ready: bool,
    revision: u64,
    /// Changes seen while a walk was out, put back on top of its result: the
    /// walk saw the folder as it was when it read it.
    since_walk: Vec<IndexChange>,
}

/// One change to the index.
#[derive(Clone, Debug, PartialEq)]
pub enum IndexChange {
    Upsert(IndexedFile),
    Remove(String),
}

impl ProjectIndex {
    /// The project this indexes.
    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    /// Whether the first walk has landed. Before it, the index is empty, not
    /// "the project has no files".
    pub fn ready(&self) -> bool {
        self.ready
    }

    /// Bumped on every change: compare it to know whether to look again.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn get(&self, relative: &str) -> Option<&IndexedFile> {
        self.files.get(relative)
    }

    /// Every file, in path order.
    pub fn files(&self) -> impl Iterator<Item = &IndexedFile> {
        self.files.values()
    }

    /// Files whose extension is one of `extensions` (lower-case, no dot).
    pub fn with_extensions<'a>(&'a self, extensions: &'a [&'a str]) -> impl Iterator<Item = &'a IndexedFile> + 'a {
        self.files.values().filter(move |f| extensions.contains(&f.extension().as_str()))
    }

    /// Start over for `root`: empty and not ready until a walk lands.
    pub fn begin(&mut self, root: PathBuf) {
        self.root = Some(root);
        self.files.clear();
        self.ready = false;
        self.since_walk.clear();
        self.revision += 1;
    }

    /// A walk's result. Ignored if the project changed since it was asked for.
    pub fn complete(&mut self, root: &Path, files: Vec<IndexedFile>) {
        if self.root.as_deref() != Some(root) {
            return;
        }
        self.files = files.into_iter().map(|f| (f.path.clone(), f)).collect();
        for change in std::mem::take(&mut self.since_walk) {
            self.apply_one(change);
        }
        self.ready = true;
        self.revision += 1;
    }

    /// A change the watcher reported.
    pub fn apply(&mut self, change: IndexChange) {
        if !self.ready {
            self.since_walk.push(change.clone());
        }
        self.apply_one(change);
        self.revision += 1;
    }

    fn apply_one(&mut self, change: IndexChange) {
        match change {
            IndexChange::Upsert(file) => {
                self.files.insert(file.path.clone(), file);
            }
            IndexChange::Remove(path) => {
                self.files.remove(&path);
                // A folder removed takes everything under it.
                let prefix = format!("{path}/");
                self.files.retain(|k, _| !k.starts_with(&prefix));
            }
        }
    }
}

/// Walk `root` and index every project file, by the watcher's rules. Slow for
/// a big project: run it on a thread, never in a system.
pub fn walk_project(root: &Path) -> Vec<IndexedFile> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(relative) = path.strip_prefix(root) else { continue };
            let relative = relative.to_string_lossy().replace('\\', "/");
            if is_ignored(&relative) {
                continue;
            }
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_dir() {
                stack.push(path);
            } else if !is_transient(&relative) {
                if let Some(file) = IndexedFile::stat(relative, &path) {
                    files.push(file);
                }
            }
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str) -> IndexedFile {
        IndexedFile { path: path.into(), size: 1, modified: 1 }
    }

    #[test]
    fn a_walk_skips_what_the_watcher_ignores() {
        let dir = std::env::temp_dir().join(format!("rz-index-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for p in ["models/car.rvg", "scripts/a.rs", "target/debug/x.rlib", ".renzora/cache.bin", "scenes/main.bsn.tmp"] {
            let path = dir.join(p);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"x").unwrap();
        }
        let files: Vec<String> = walk_project(&dir).into_iter().map(|f| f.path).collect();
        assert_eq!(files, ["models/car.rvg", "scripts/a.rs"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A change that arrives while the walk is out survives the walk landing.
    #[test]
    fn changes_during_a_walk_are_kept() {
        let mut index = ProjectIndex::default();
        let root = PathBuf::from("/p");
        index.begin(root.clone());
        index.apply(IndexChange::Upsert(file("models/new.rvg")));
        index.complete(&root, vec![file("models/old.rvg")]);
        assert!(index.ready());
        let paths: Vec<&str> = index.files().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["models/new.rvg", "models/old.rvg"]);
    }

    #[test]
    fn a_stale_walk_is_ignored_and_a_removed_folder_takes_its_files() {
        let mut index = ProjectIndex::default();
        index.begin(PathBuf::from("/a"));
        index.begin(PathBuf::from("/b"));
        index.complete(Path::new("/a"), vec![file("x.rvg")]);
        assert!(!index.ready(), "a walk of the previous project does not count");
        index.complete(Path::new("/b"), vec![file("models/car/car.rvg"), file("models/car/t.png"), file("m.rvg")]);
        index.apply(IndexChange::Remove("models/car".into()));
        assert_eq!(index.with_extensions(&["rvg"]).map(|f| f.path.as_str()).collect::<Vec<_>>(), ["m.rvg"]);
    }
}
