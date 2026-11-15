//! What counts as a project file, the listing of them, and where a file's
//! cached thumbnail lives.
//!
//! The editor's watcher, its project index and the companion all decide "is
//! this a project file" by these rules, and all three must agree or a file is
//! compiled by one and ignored by another. They lived in `renzora`, which links
//! Bevy; the standalone companion walks and watches the project too, so the
//! rules live here and `renzora` re-exports them under their old paths.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Directories the watcher never reports from.
///
/// Not a performance tweak. `target/` alone can hold hundreds of thousands of
/// files that change constantly during a build, and a script or shader rebuild
/// writing into it would feed its own output straight back in as change events.
/// The same list is used by the project walks (`collect_project_scripts` and
/// friends), and the two must agree or a file will be compiled by one and
/// ignored by the other.
pub const IGNORED_DIRS: &[&str] = &["target", ".git", ".renzora", "node_modules", "dist", ".svn", ".hg"];

/// Is this project-relative path inside an ignored directory, or hidden?
///
/// Takes the forward-slashed relative form, which is what `ProjectFileChanged`
/// carries. Leading-dot components are ignored wholesale: editors and tools
/// scribble in them constantly and nothing in a project is authored there.
pub fn is_ignored(relative: &str) -> bool {
    relative
        .split('/')
        .any(|part| IGNORED_DIRS.contains(&part) || (part.starts_with('.') && part.len() > 1))
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
/// are about the real file. `ProjectFileChanged::still_missing` is what stops
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

/// One file: where, how big, when it last changed. The answer to the
/// `"project_index"` job is a list of these, as JSON.
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

/// Walk `root` and index every project file, by the watcher's rules. Slow for
/// a big project: run it on a thread, never in a frame.
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

/// Where the thumbnail cache of the project at `root` keeps one kind
/// (`"models"`, `"materials"`).
pub fn thumbnail_cache_dir(root: &Path, kind: &str) -> PathBuf {
    root.join(".cache").join("thumbnails").join(kind)
}

/// The cached PNG thumbnail of the file at project-relative `relative`.
///
/// `models/audi.glb` of kind `"models"` is
/// `<root>/.cache/thumbnails/models/models/audi.png`. A leading `assets/` is
/// dropped, from when projects kept their files in one. The editor renders to
/// this path and the companion deletes from it when the file goes, so both
/// call this rather than spelling it out.
pub fn cached_thumbnail(root: &Path, relative: &str, kind: &str) -> PathBuf {
    let relative = relative.strip_prefix("assets/").unwrap_or(relative);
    let mut out = thumbnail_cache_dir(root, kind).join(relative);
    out.set_extension("png");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_output_is_ignored() {
        assert!(is_ignored("target/dist/thing.rs"));
        assert!(is_ignored("scripts/.hidden/x.rs"));
        assert!(!is_ignored("scripts/fps.rs"));
    }

    #[test]
    fn a_dotfile_name_is_not_a_hidden_directory() {
        // "." alone is a path component the walker produces, not a hidden dir.
        assert!(!is_ignored("."));
    }

    #[test]
    fn an_atomic_saves_scratch_file_is_transient() {
        // The exact shape observed from a real save.
        assert!(is_transient("particles/watch_test.particle.tmp.20124.8ed8f24b3cfd"));
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

    #[test]
    fn a_thumbnail_sits_under_its_kind_without_the_assets_folder() {
        let root = Path::new("/p");
        assert_eq!(
            cached_thumbnail(root, "assets/models/audi.glb", "models"),
            Path::new("/p/.cache/thumbnails/models/models/audi.png")
        );
        assert_eq!(
            cached_thumbnail(root, "shaders/rock.material", "materials"),
            Path::new("/p/.cache/thumbnails/materials/shaders/rock.png")
        );
    }
}
