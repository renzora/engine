//! Engine localization runtime — loads language packs into the shared
//! [`renzora::lang`] table and keeps the active language in sync.
//!
//! The *contract* (the global table + `t()` lookup + the plugin registration
//! API) lives in the `renzora` dylib so every crate and dlopen'd plugin reads
//! one table. This crate is the *runtime* that fills it:
//!
//! 1. **Embedded built-ins.** The shipped languages are compiled in via
//!    `include_str!` from the repo-root `languages/` directory, so a fresh
//!    binary is fully localized with no external files — important for exported
//!    games, where there is no `languages/` folder unless the dev ships one.
//! 2. **External packs.** Any `languages/*.toml` beside the executable or in the
//!    working directory is loaded too, and *overrides* a built-in of the same
//!    code key-for-key. This is the install path a future marketplace language
//!    pack drops into; for now you place the file by hand. The folder is
//!    re-scanned periodically, so editing a pack updates the UI live.
//! 3. **Plugin contributions.** Any `renzora_*` plugin can register its own
//!    strings from its `build()` — see [`renzora::lang`] for the API.
//!    Those merge into the same table, so a distribution plugin localizes its
//!    own panels without this crate knowing about it.
//!
//! Scope is **Runtime**: localization is needed in the editor viewport *and* the
//! shipped game, and it's core infrastructure, so it links into the binary
//! (self-registers via `inventory`) rather than shipping as an optional dlopen
//! plugin that could be missing.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use bevy::prelude::*;
use bevy::time::common_conditions::on_timer;

#[cfg(feature = "scripting")]
mod script_extension;

/// Built-in language packs, embedded at compile time from the repo-root
/// `languages/` directory. English is the fallback and must stay first so a
/// half-written non-English pack can never starve the resolver of a base.
///
/// As the other 15 target languages are authored, add them here — that's the
/// only edit needed to ship a new built-in language.
const EMBEDDED_PACKS: &[(&str, &str)] = &[
    ("en", include_str!("../../../languages/en.toml")),
    ("de", include_str!("../../../languages/de.toml")),
    ("es", include_str!("../../../languages/es.toml")),
    ("fr", include_str!("../../../languages/fr.toml")),
    ("ja", include_str!("../../../languages/ja.toml")),
    ("zh", include_str!("../../../languages/zh.toml")),
    ("zh-TW", include_str!("../../../languages/zh-TW.toml")),
    ("ko", include_str!("../../../languages/ko.toml")),
    ("it", include_str!("../../../languages/it.toml")),
    ("pt-BR", include_str!("../../../languages/pt-BR.toml")),
    ("ru", include_str!("../../../languages/ru.toml")),
    ("pl", include_str!("../../../languages/pl.toml")),
    ("nl", include_str!("../../../languages/nl.toml")),
    ("tr", include_str!("../../../languages/tr.toml")),
    ("uk", include_str!("../../../languages/uk.toml")),
    ("ar", include_str!("../../../languages/ar.toml")),
    ("hi", include_str!("../../../languages/hi.toml")),
    ("id", include_str!("../../../languages/id.toml")),
    ("vi", include_str!("../../../languages/vi.toml")),
    ("th", include_str!("../../../languages/th.toml")),
];

/// How often the external `languages/` folder is re-scanned for new or edited
/// packs. Cheap (a stat per file); long enough not to matter on a frame budget.
const RESCAN_INTERVAL: Duration = Duration::from_secs(2);

/// Tracks external pack files and their last-seen modified time, so a rescan
/// only re-parses a file that actually changed.
#[derive(Resource, Default)]
struct ExternalPacks {
    /// `path → last modified (secs since epoch)`.
    seen: HashMap<PathBuf, u64>,
}

/// Runtime-scope plugin that installs the localization runtime.
#[derive(Default)]
pub struct LangPlugin;

impl Plugin for LangPlugin {
    fn build(&self, app: &mut App) {
        // 1. Built-ins first, so the table is fully populated before any
        //    external pack or plugin contribution gets a chance to override.
        for (code, src) in EMBEDDED_PACKS {
            if let Err(e) = renzora::lang::register_pack_str(src) {
                error!("[lang] embedded pack '{code}' failed to parse: {e}");
            }
        }

        // 2. External packs (a marketplace install drops a file here). Loaded
        //    after built-ins so they win on conflicting keys.
        let mut external = ExternalPacks::default();
        scan_external_packs(&mut external);

        // 3. Initial active language. `RENZORA_LANG` is an explicit override
        //    (handy for testing / CI screenshots); otherwise the per-user
        //    preference saved in `~/.renzora/editor.toml` (default "en"). The
        //    Settings picker calls `set_active` + `save_language` to update it.
        let initial =
            std::env::var("RENZORA_LANG").unwrap_or_else(|_| renzora::load_language());
        renzora::lang::set_active(&initial);
        info!(
            "[lang] {} language(s) loaded, active = '{}'",
            renzora::lang::available().len(),
            renzora::lang::active_code(),
        );

        app.insert_resource(external)
            .add_message::<renzora::lang::LanguageChanged>()
            .add_systems(
                Update,
                (
                    rescan_external_packs.run_if(on_timer(RESCAN_INTERVAL)),
                    emit_language_changed,
                ),
            );

        // Expose `tr("key")` to Lua/Rhai scripts via the scripting extension
        // registry. Unconditional now that a binding is a declaration rather
        // than a Lua function: which interpreter is present — if any — is
        // decided at runtime by whichever language plugin loaded, so gating
        // this on a compile-time backend feature would drop `tr` from a game
        // scripted in something else.
        #[cfg(feature = "scripting")]
        {
            let mut extensions = app.world_mut().get_resource_or_insert_with(
                renzora_scripting::extension::ScriptExtensions::default,
            );
            extensions.register(script_extension::LangScriptExtension);
        }
    }
}

/// Candidate `languages/` directories: beside the executable and in the working
/// directory. Deduplicated; missing ones are simply skipped.
fn external_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("languages"));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd.join("languages"));
    }
    dirs.sort();
    dirs.dedup();
    dirs
}

/// Read every `*.toml` under the external dirs, registering changed files.
/// Records each file's mtime so a later rescan re-parses only what changed.
fn scan_external_packs(state: &mut ExternalPacks) {
    scan_dirs(state, &external_dirs());
}

/// The body of [`scan_external_packs`], with the directory list passed in.
///
/// Split out purely so it can be tested: [`external_dirs`] reads the process's
/// executable path and working directory, and a test that wanted to exercise the
/// mtime-skip logic through it would have to `set_current_dir` — a process-global
/// mutation that races every other test in the binary.
fn scan_dirs(state: &mut ExternalPacks, dirs: &[PathBuf]) {
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            let mtime = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            // Skip unchanged files so a rescan is cheap and idempotent.
            if state.seen.get(&path) == Some(&mtime) {
                continue;
            }
            let result = std::fs::read_to_string(&path)
                .map_err(|e| format!("unreadable: {e}"))
                .and_then(|src| {
                    renzora::lang::register_pack_str(&src)
                        .map_err(|e| format!("parse error: {e}"))
                });
            match result {
                Ok(()) => info!("[lang] loaded external pack {}", path.display()),
                Err(e) => error!("[lang] {} {e}", path.display()),
            }
            // Record the mtime even on failure: a broken pack is logged ONCE,
            // not re-attempted (and re-logged) every rescan. Editing the file to
            // fix it changes the mtime, which re-triggers a load attempt.
            state.seen.insert(path, mtime);
        }
    }
}

/// Periodic rescan so dropping in or editing a pack updates the engine live.
fn rescan_external_packs(mut state: ResMut<ExternalPacks>) {
    scan_external_packs(&mut state);
}

/// Bridge the lock-free global revision counter to a Bevy event: when anything
/// (active language, a new pack, a plugin contribution) bumps the revision,
/// fire `LanguageChanged` so panels caching translated text can rebuild.
fn emit_language_changed(
    mut last: Local<u64>,
    mut writer: bevy::ecs::message::MessageWriter<renzora::lang::LanguageChanged>,
) {
    let rev = renzora::lang::revision();
    // `Local` starts at 0; the build-time registrations already pushed the
    // revision past 0, so the first frame emits one change — exactly what we
    // want, to localize any UI built before this system first runs.
    if *last != rev {
        *last = rev;
        writer.write(renzora::lang::LanguageChanged {
            code: renzora::lang::active_code(),
        });
    }
}

renzora::add!(LangPlugin, Runtime, priority = -50);

#[cfg(test)]
mod tests {
    use super::*;

    /// Serializes the tests that touch `renzora::lang`'s **process-global**
    /// table and revision counter.
    ///
    /// The store is a single `RwLock` shared by the whole binary, and cargo runs
    /// tests in parallel threads within one process — so a test asserting "the
    /// revision did not move" races any other test that registers a pack. That
    /// is not hypothetical: it is what made the first version of
    /// `the_first_run_emits_a_language_changed` fail depending on scheduling.
    ///
    /// **Every test that WRITES to the store must take this lock, not just the
    /// ones that assert on it.** A half-applied lock is worse than none, because
    /// it looks correct and fails rarely: the first version of this module left
    /// `every_embedded_pack_parses` unlocked, and its twenty registrations raced
    /// `a_quiet_frame_emits_nothing_further` — which passed locally and in CI's
    /// `test` job, then failed in the coverage job, where the slower
    /// instrumented build shifted the interleaving.
    ///
    /// Tests that only read their own `ExternalPacks` state need no lock.
    static LANG: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Take the lock, tolerating a previous test having panicked while holding
    /// it — a poisoned mutex would otherwise cascade one failure into several.
    fn lang_lock() -> std::sync::MutexGuard<'static, ()> {
        LANG.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Every embedded pack must parse. A typo in a hand-authored `.toml` is
    /// otherwise a runtime `error!` nobody reads, and that language silently
    /// falls back to English in a shipped game.
    #[test]
    fn every_embedded_pack_parses() {
        // Registers twenty packs into the global store — twenty revision bumps
        // that would race any test asserting the revision holds still.
        let _guard = lang_lock();
        for (code, src) in EMBEDDED_PACKS {
            assert!(
                renzora::lang::register_pack_str(src).is_ok(),
                "embedded pack '{code}' does not parse"
            );
        }
    }

    /// English is the resolver's base. If it stops being first, a half-written
    /// pack registered ahead of it can leave keys with no fallback.
    #[test]
    fn english_is_the_first_embedded_pack() {
        assert_eq!(EMBEDDED_PACKS[0].0, "en");
    }

    #[test]
    fn embedded_language_codes_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for (code, _) in EMBEDDED_PACKS {
            assert!(seen.insert(*code), "'{code}' is listed twice");
        }
    }

    #[test]
    fn external_dirs_are_deduplicated() {
        let dirs = external_dirs();
        let unique: std::collections::HashSet<_> = dirs.iter().collect();
        assert_eq!(dirs.len(), unique.len(), "external_dirs returned a duplicate");
        assert!(dirs.iter().all(|d| d.ends_with("languages")));
    }

    fn pack_dir(files: &[(&str, &str)]) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        for (name, body) in files {
            std::fs::write(tmp.path().join(name), body).unwrap();
        }
        tmp
    }

    #[test]
    fn a_toml_pack_is_registered_and_its_mtime_recorded() {
        // Writes to the global store (and reads it back via `has_language`).
        let _guard = lang_lock();
        let tmp = pack_dir(&[(
            "test-pack.toml",
            "[meta]\nname = \"Scan Test\"\ncode = \"zz-scan\"\n[strings]\nscan_key = \"scanned\"\n",
        )]);
        let mut state = ExternalPacks::default();
        scan_dirs(&mut state, &[tmp.path().to_path_buf()]);

        assert_eq!(state.seen.len(), 1, "the pack's mtime should be recorded");
        assert!(renzora::lang::has_language("zz-scan"));
    }

    /// The mtime table is what keeps the 2-second rescan cheap. If an unchanged
    /// file were re-parsed every tick, every pack would be re-registered twice a
    /// second forever.
    #[test]
    fn an_unchanged_pack_is_skipped_on_rescan() {
        let _guard = lang_lock();
        let tmp = pack_dir(&[(
            "stable.toml",
            "[meta]\nname = \"Stable\"\ncode = \"zz-stable\"\n[strings]\nk = \"v\"\n",
        )]);
        let dirs = [tmp.path().to_path_buf()];
        let mut state = ExternalPacks::default();

        scan_dirs(&mut state, &dirs);
        let after_first = renzora::lang::revision();

        scan_dirs(&mut state, &dirs);
        assert_eq!(
            renzora::lang::revision(),
            after_first,
            "an unchanged pack was re-registered, bumping the revision"
        );
    }

    #[test]
    fn non_toml_files_are_ignored() {
        let tmp = pack_dir(&[("readme.md", "not a pack"), ("pack.toml.bak", "nor this")]);
        let mut state = ExternalPacks::default();
        scan_dirs(&mut state, &[tmp.path().to_path_buf()]);
        assert!(state.seen.is_empty());
    }

    /// A broken pack must still record its mtime, or it is re-parsed and
    /// re-logged on every rescan — twice a second, forever.
    #[test]
    fn a_malformed_pack_is_recorded_so_it_is_not_retried() {
        let tmp = pack_dir(&[("broken.toml", "this is not = valid toml [[[")]);
        let dirs = [tmp.path().to_path_buf()];
        let mut state = ExternalPacks::default();
        scan_dirs(&mut state, &dirs);
        assert_eq!(state.seen.len(), 1, "a failed parse must still be recorded");

        let before = state.seen.clone();
        scan_dirs(&mut state, &dirs);
        assert_eq!(state.seen, before, "the broken pack was retried");
    }

    #[test]
    fn a_missing_directory_is_skipped_rather_than_fatal() {
        let mut state = ExternalPacks::default();
        scan_dirs(&mut state, &[PathBuf::from("no/such/languages/dir")]);
        assert!(state.seen.is_empty());
    }

    /// Counts `LanguageChanged` messages so a multi-frame app can be asserted
    /// on; `run_system_once` cannot, because it hands the system a fresh
    /// `Local<u64>` every call and the whole behaviour under test is that
    /// `Local`'s persistence.
    #[derive(Resource, Default)]
    struct Seen(usize);

    fn emit_app() -> App {
        let mut app = App::new();
        app.init_resource::<Seen>()
            .add_message::<renzora::lang::LanguageChanged>()
            .add_systems(
                Update,
                (
                    emit_language_changed,
                    |mut reader: bevy::ecs::message::MessageReader<
                        renzora::lang::LanguageChanged,
                    >,
                     mut seen: ResMut<Seen>| {
                        seen.0 += reader.read().count();
                    },
                )
                    .chain(),
            );
        app
    }

    /// The `Local` starts at 0 and the revision is already past it by the time
    /// any real app runs, so the first run must emit — that is what localizes UI
    /// built before this system first ran.
    #[test]
    fn the_first_run_emits_a_language_changed() {
        let _guard = lang_lock();
        // Guarantee the global revision is non-zero without depending on which
        // other tests have already run.
        renzora::lang::register_pack_str(EMBEDDED_PACKS[0].1).unwrap();

        let mut app = emit_app();
        app.update();
        assert_eq!(
            app.world().resource::<Seen>().0,
            1,
            "the first run should announce the current language"
        );
    }

    /// The counter is the point: panels rebuild their translated text on this
    /// message, so re-emitting every frame would rebuild every localized panel
    /// every frame.
    #[test]
    fn a_quiet_frame_emits_nothing_further() {
        let _guard = lang_lock();
        renzora::lang::register_pack_str(EMBEDDED_PACKS[0].1).unwrap();

        let mut app = emit_app();
        app.update();
        let after_first = app.world().resource::<Seen>().0;

        app.update();
        app.update();
        assert_eq!(
            app.world().resource::<Seen>().0,
            after_first,
            "the message repeated on a frame where nothing changed"
        );
    }

    #[test]
    fn switching_language_emits_again() {
        let _guard = lang_lock();
        renzora::lang::register_pack_str(EMBEDDED_PACKS[0].1).unwrap();
        renzora::lang::register_pack_str(EMBEDDED_PACKS[1].1).unwrap();
        renzora::lang::set_active("en");

        let mut app = emit_app();
        app.update();
        let after_first = app.world().resource::<Seen>().0;

        renzora::lang::set_active("de");
        app.update();

        assert_eq!(app.world().resource::<Seen>().0, after_first + 1);
        renzora::lang::set_active("en");
    }

    #[test]
    fn set_active_switches_the_reported_code() {
        let _guard = lang_lock();
        renzora::lang::register_pack_str(EMBEDDED_PACKS[0].1).unwrap();
        renzora::lang::set_active("en");
        assert_eq!(renzora::lang::active_code(), "en");
    }
}
