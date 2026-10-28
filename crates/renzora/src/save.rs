//! Saved games: named values a game keeps between sessions, written to slots.
//!
//! A save holds whatever the game puts in it (`"level"`, `"coins"`,
//! `"has_key"`) and nothing else. That is deliberate. A snapshot of the whole
//! scene would need no script code, but it would also break the first time a
//! level was edited after a player saved in it, and it would be large. Named
//! values survive both, and are what most shipped games store.
//!
//! # Why a process-wide store
//!
//! Like the translation table in [`crate::lang`]: every language backend and
//! every Rust script must see one store, and this crate is the one image they
//! all share. A resource would need the `World` in hand at the moment a script
//! asks for a value, which a hook halfway through running does not have.
//!
//! # The working set and the slots
//!
//! [`set`] and [`get`] act on the values in memory, the *working set*. Nothing
//! reaches disk until [`write`] copies the working set into a slot, and
//! [`load`] replaces the working set with a slot's contents. So a game sets
//! values as play goes on and writes when it reaches a checkpoint, and a value
//! that was never written is gone when the game quits.
//!
//! # Where slots live
//!
//! One `<slot>.json` per slot, in a `saves` folder under the platform's
//! per-user data folder and the game's name (see [`default_directory`]). The
//! engine points the store there once the project is known. On the web there
//! is no folder; slots are kept in memory and last until the tab closes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};

use crate::component::ScriptValue;

/// The layout version written into every save.
///
/// Bumped when the file layout changes, so a future engine can migrate an old
/// save rather than misread it. A save from a *newer* engine is refused (see
/// [`SaveError::TooNew`]) instead of loaded with fields silently dropped.
pub const SAVE_FORMAT_VERSION: u32 = 1;

/// Longest slot name accepted.
pub const MAX_SLOT_NAME: usize = 64;

/// What goes on disk.
#[derive(Serialize, Deserialize)]
struct SaveFile {
    version: u32,
    /// Seconds since the Unix epoch, for "saved 2 hours ago" in a load menu.
    saved_at: u64,
    values: BTreeMap<String, ScriptValue>,
}

/// One slot, as [`list`] reports it.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotInfo {
    pub name: String,
    /// Seconds since the Unix epoch.
    pub saved_at: u64,
}

/// Why a slot could not be written, read or removed.
#[derive(Debug, Clone, PartialEq)]
pub enum SaveError {
    /// The slot name is empty, too long, or has a character other than a
    /// letter, digit, `-` or `_`. Checked because the name becomes a file name,
    /// and `"../settings"` must not reach outside the saves folder.
    InvalidSlot(String),
    /// No save folder is known yet: the project has not loaded, or the
    /// platform reported no per-user data folder.
    NoLocation,
    NotFound(String),
    Io(String),
    /// The file exists but is not a save this engine can read.
    Corrupt(String),
    /// Written by a newer engine, in a layout this one does not know.
    TooNew { found: u32 },
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSlot(name) => write!(
                f,
                "\"{name}\" is not a valid slot name: use 1 to {MAX_SLOT_NAME} letters, digits, - or _"
            ),
            Self::NoLocation => write!(f, "there is nowhere to save yet"),
            Self::NotFound(name) => write!(f, "there is no save in slot \"{name}\""),
            Self::Io(e) => write!(f, "{e}"),
            Self::Corrupt(e) => write!(f, "the save is damaged: {e}"),
            Self::TooNew { found } => write!(
                f,
                "the save was made by a newer version of the game (format {found}, this one reads {SAVE_FORMAT_VERSION})"
            ),
        }
    }
}

impl std::error::Error for SaveError {}

struct Store {
    values: BTreeMap<String, ScriptValue>,
    directory: Option<PathBuf>,
    /// Slots kept in memory, used when there is no folder: the web, and tests.
    memory: Option<BTreeMap<String, String>>,
}

static STORE: Mutex<Store> = Mutex::new(Store {
    values: BTreeMap::new(),
    directory: None,
    memory: if cfg!(target_arch = "wasm32") {
        Some(BTreeMap::new())
    } else {
        None
    },
});

/// The store, even if a thread panicked while holding it. A save that is
/// half-updated is still better than a game that can never save again.
fn store() -> MutexGuard<'static, Store> {
    STORE.lock().unwrap_or_else(PoisonError::into_inner)
}

// ── The working set ─────────────────────────────────────────────────────────

pub fn get(key: &str) -> Option<ScriptValue> {
    store().values.get(key).cloned()
}

pub fn set(key: impl Into<String>, value: ScriptValue) {
    store().values.insert(key.into(), value);
}

/// Returns whether the key was there.
pub fn remove(key: &str) -> bool {
    store().values.remove(key).is_some()
}

/// Empty the working set, as for a new game. Slots on disk are untouched.
pub fn clear() {
    store().values.clear();
}

/// Every key in the working set, sorted.
pub fn keys() -> Vec<String> {
    store().values.keys().cloned().collect()
}

// ── Slots ───────────────────────────────────────────────────────────────────

/// Copy the working set into `slot`, replacing what was there.
///
/// Written to a temporary file and renamed into place, so a crash or a full
/// disk part-way through leaves the previous save intact rather than a
/// truncated one. Losing the last save is the failure players remember.
pub fn write(slot: &str) -> Result<(), SaveError> {
    check_slot(slot)?;
    let mut store = store();
    let file = SaveFile {
        version: SAVE_FORMAT_VERSION,
        saved_at: now(),
        values: store.values.clone(),
    };
    let text = serde_json::to_string_pretty(&file).map_err(|e| SaveError::Io(e.to_string()))?;
    if let Some(memory) = &mut store.memory {
        memory.insert(slot.to_string(), text);
        return Ok(());
    }
    let dir = store.directory.clone().ok_or(SaveError::NoLocation)?;
    drop(store);

    std::fs::create_dir_all(&dir).map_err(io)?;
    let path = slot_path(&dir, slot);
    let partial = path.with_extension("json.partial");
    std::fs::write(&partial, text).map_err(io)?;
    std::fs::rename(&partial, &path).map_err(io)
}

/// Replace the working set with what `slot` holds.
///
/// All or nothing: a slot that cannot be read leaves the working set as it was,
/// so a damaged save does not also wipe the game in progress.
pub fn load(slot: &str) -> Result<(), SaveError> {
    let file = read(slot)?;
    store().values = file.values;
    Ok(())
}

pub fn exists(slot: &str) -> bool {
    if check_slot(slot).is_err() {
        return false;
    }
    let store = store();
    if let Some(memory) = &store.memory {
        return memory.contains_key(slot);
    }
    store
        .directory
        .as_ref()
        .is_some_and(|dir| slot_path(dir, slot).is_file())
}

/// Remove `slot`. Removing one that does not exist is not an error.
pub fn delete(slot: &str) -> Result<(), SaveError> {
    check_slot(slot)?;
    let mut store = store();
    if let Some(memory) = &mut store.memory {
        memory.remove(slot);
        return Ok(());
    }
    let dir = store.directory.clone().ok_or(SaveError::NoLocation)?;
    match std::fs::remove_file(slot_path(&dir, slot)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(io(e)),
        _ => Ok(()),
    }
}

/// Every slot, most recently saved first.
///
/// A file in the folder that is not a readable save is left out rather than
/// failing the list: one damaged slot should not hide the others from a load
/// menu.
pub fn list() -> Vec<SlotInfo> {
    let names: Vec<String> = {
        let store = store();
        if let Some(memory) = &store.memory {
            memory.keys().cloned().collect()
        } else if let Some(dir) = &store.directory {
            std::fs::read_dir(dir)
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|entry| {
                    let path = entry.path();
                    (path.extension()? == "json")
                        .then(|| path.file_stem()?.to_str().map(str::to_string))
                        .flatten()
                })
                .filter(|name| check_slot(name).is_ok())
                .collect()
        } else {
            Vec::new()
        }
    };
    let mut slots: Vec<SlotInfo> = names
        .into_iter()
        .filter_map(|name| {
            let saved_at = read(&name).ok()?.saved_at;
            Some(SlotInfo { name, saved_at })
        })
        .collect();
    slots.sort_by(|a, b| b.saved_at.cmp(&a.saved_at).then_with(|| a.name.cmp(&b.name)));
    slots
}

// ── Location ────────────────────────────────────────────────────────────────

/// Point the store at a folder. `None` forgets it, after which writes fail with
/// [`SaveError::NoLocation`] until another is set.
pub fn set_directory(dir: Option<PathBuf>) {
    store().directory = dir;
}

pub fn directory() -> Option<PathBuf> {
    store().directory.clone()
}

/// Keep slots in memory instead of on disk, for tests. On the web this is
/// already the case.
pub fn use_memory_slots() {
    store().memory = Some(BTreeMap::new());
}

/// Where `game`'s saves go on this platform:
///
/// | Platform | Folder |
/// |---|---|
/// | Windows | `%APPDATA%\<game>\saves` |
/// | macOS, iOS | `~/Library/Application Support/<game>/saves` |
/// | Linux | `$XDG_DATA_HOME/<game>/saves`, else `~/.local/share/<game>/saves` |
/// | Android | the app's internal data folder, `/saves` |
/// | Web | none |
///
/// Per user and outside the install folder, because an installed game's own
/// folder is often read-only and is replaced by an update.
pub fn default_directory(game: &str) -> Option<PathBuf> {
    #[cfg(target_os = "android")]
    {
        // The app's sandbox is already per game, so no name folder.
        let _ = game;
        return bevy::android::ANDROID_APP
            .get()
            .and_then(|app| app.internal_data_path())
            .map(|dir| dir.join("saves"));
    }
    #[cfg(not(target_os = "android"))]
    platform_data_dir().map(|dir| dir.join(folder_name(game)).join("saves"))
}

#[cfg(not(target_os = "android"))]
fn platform_data_dir() -> Option<PathBuf> {
    let var = |name| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(target_arch = "wasm32") {
        None
    } else if cfg!(windows) {
        var("APPDATA")
    } else if cfg!(any(target_os = "macos", target_os = "ios")) {
        var("HOME").map(|home| home.join("Library").join("Application Support"))
    } else {
        var("XDG_DATA_HOME").or_else(|| var("HOME").map(|home| home.join(".local").join("share")))
    }
}

/// The game's name made safe as one folder name. Characters a file system may
/// refuse become `_`; a name with nothing left falls back to a fixed one.
fn folder_name(game: &str) -> String {
    let name: String = game
        .trim()
        .chars()
        .map(|c| if c.is_alphanumeric() || " -_.".contains(c) { c } else { '_' })
        .collect();
    let name = name.trim_matches(|c| c == '.' || c == ' ');
    if name.is_empty() {
        "Renzora Game".to_string()
    } else {
        name.to_string()
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn check_slot(slot: &str) -> Result<(), SaveError> {
    let ok = (1..=MAX_SLOT_NAME).contains(&slot.len())
        && slot.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(SaveError::InvalidSlot(slot.to_string()))
    }
}

fn slot_path(dir: &Path, slot: &str) -> PathBuf {
    dir.join(format!("{slot}.json"))
}

fn read(slot: &str) -> Result<SaveFile, SaveError> {
    check_slot(slot)?;
    let text = {
        let store = store();
        if let Some(memory) = &store.memory {
            memory.get(slot).cloned()
        } else {
            let dir = store.directory.clone().ok_or(SaveError::NoLocation)?;
            drop(store);
            match std::fs::read_to_string(slot_path(&dir, slot)) {
                Ok(text) => Some(text),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(io(e)),
            }
        }
    }
    .ok_or_else(|| SaveError::NotFound(slot.to_string()))?;

    // Read the version on its own first, so a newer layout is reported as
    // such rather than as whatever parse error its new fields cause.
    #[derive(Deserialize)]
    struct Version {
        version: u32,
    }
    let version = serde_json::from_str::<Version>(&text)
        .map_err(|e| SaveError::Corrupt(e.to_string()))?
        .version;
    if version > SAVE_FORMAT_VERSION {
        return Err(SaveError::TooNew { found: version });
    }
    serde_json::from_str(&text).map_err(|e| SaveError::Corrupt(e.to_string()))
}

fn io(e: std::io::Error) -> SaveError {
    SaveError::Io(e.to_string())
}

/// Seconds since the Unix epoch.
///
/// 0 on the web: std's `SystemTime::now` panics there and this crate carries no
/// browser bindings to ask the page's clock instead. Web slots last only for
/// the session, so the time a load menu would show matters least there, and
/// [`list`] falls back to ordering them by name.
fn now() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
    #[cfg(target_arch = "wasm32")]
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The store is process-wide, so tests that use it run one at a time and
    /// each starts from an empty working set on its own folder.
    static SERIAL: Mutex<()> = Mutex::new(());

    fn fresh() -> (MutexGuard<'static, ()>, PathBuf) {
        let guard = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
        let dir = std::env::temp_dir().join(format!(
            "renzora-save-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let mut store = store();
        store.values.clear();
        store.memory = None;
        store.directory = Some(dir.clone());
        (guard, dir)
    }

    #[test]
    fn a_written_slot_loads_back() {
        let (_guard, _dir) = fresh();
        set("coins", ScriptValue::Int(42));
        set("player", ScriptValue::String("Ada".into()));
        write("slot1").unwrap();

        clear();
        assert!(get("coins").is_none());
        load("slot1").unwrap();
        assert!(matches!(get("coins"), Some(ScriptValue::Int(42))));
        assert!(matches!(get("player"), Some(ScriptValue::String(s)) if s == "Ada"));
    }

    #[test]
    fn loading_replaces_the_working_set_rather_than_merging() {
        let (_guard, _dir) = fresh();
        set("level", ScriptValue::Int(1));
        write("a").unwrap();
        set("unsaved", ScriptValue::Bool(true));
        load("a").unwrap();
        assert_eq!(keys(), vec!["level".to_string()]);
    }

    #[test]
    fn a_failed_load_keeps_the_game_in_progress() {
        let (_guard, dir) = fresh();
        set("level", ScriptValue::Int(3));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("broken.json"), "{ not json").unwrap();

        assert!(matches!(load("broken"), Err(SaveError::Corrupt(_))));
        assert!(matches!(load("missing"), Err(SaveError::NotFound(_))));
        assert!(matches!(get("level"), Some(ScriptValue::Int(3))));
    }

    #[test]
    fn a_save_from_a_newer_engine_is_refused_by_name() {
        let (_guard, dir) = fresh();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("future.json"), r#"{"version": 99, "shiny": []}"#).unwrap();
        assert_eq!(load("future"), Err(SaveError::TooNew { found: 99 }));
    }

    #[test]
    fn slot_names_cannot_leave_the_saves_folder() {
        let (_guard, _dir) = fresh();
        for bad in ["", "../settings", "a/b", "a\\b", "C:", "slot.json", &"x".repeat(65)] {
            assert!(matches!(write(bad), Err(SaveError::InvalidSlot(_))), "{bad:?} was accepted");
            assert!(!exists(bad));
        }
        assert!(write("Quick_save-2").is_ok());
    }

    #[test]
    fn list_is_newest_first_and_skips_damaged_files() {
        let (_guard, dir) = fresh();
        write("older").unwrap();
        write("newer").unwrap();
        // Same second on a fast machine; back-date one so the order is real.
        let older = dir.join("older.json");
        let text = std::fs::read_to_string(&older).unwrap();
        let mut file: serde_json::Value = serde_json::from_str(&text).unwrap();
        file["saved_at"] = serde_json::json!(1);
        std::fs::write(&older, file.to_string()).unwrap();
        std::fs::write(dir.join("junk.json"), "nope").unwrap();

        let names: Vec<String> = list().into_iter().map(|s| s.name).collect();
        assert_eq!(names, vec!["newer".to_string(), "older".to_string()]);
    }

    #[test]
    fn delete_removes_the_slot_and_tolerates_a_missing_one() {
        let (_guard, _dir) = fresh();
        write("gone").unwrap();
        assert!(exists("gone"));
        delete("gone").unwrap();
        assert!(!exists("gone"));
        delete("gone").unwrap();
    }

    #[test]
    fn without_a_folder_writing_says_so() {
        let (_guard, _dir) = fresh();
        set_directory(None);
        assert_eq!(write("a"), Err(SaveError::NoLocation));
    }

    #[test]
    fn memory_slots_work_without_a_folder() {
        let (_guard, _dir) = fresh();
        set_directory(None);
        use_memory_slots();
        set("x", ScriptValue::Float(1.5));
        write("m").unwrap();
        clear();
        load("m").unwrap();
        assert!(matches!(get("x"), Some(ScriptValue::Float(v)) if v == 1.5));
        store().memory = None;
    }

    #[test]
    fn game_names_become_safe_folder_names() {
        assert_eq!(folder_name("Space Pirates: Origins"), "Space Pirates_ Origins");
        assert_eq!(folder_name("  ..  "), "Renzora Game");
        assert_eq!(folder_name("a/b\\c"), "a_b_c");
    }
}
