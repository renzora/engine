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

// The file record and the walk, shared with the standalone companion, which
// answers the walk when it is connected and links no Bevy.
pub use renzora_companion_proto::project_files::{walk_project, IndexedFile};

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

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str) -> IndexedFile {
        IndexedFile { path: path.into(), size: 1, modified: 1 }
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
