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
//! 3. **Project packs.** A project's own `languages/*.toml` hold the game's
//!    strings (and may add a language). Read from the project folder, or from
//!    the rpak in a packed game, before the starting language is chosen, and
//!    re-read in the editor when one changes. They merge over the built-ins.
//! 4. **Plugin contributions.** Any `renzora_*` plugin can register its own
//!    strings from its `build()` — see [`renzora::lang`] for the API.
//!    Those merge into the same table, so an installed plugin localizes its
//!    own panels without this crate knowing about it.
//!
//! Scope is **Runtime**: localization is needed in the editor viewport *and* the
//! shipped game, and it's core infrastructure, so it links into the binary
//! (self-registers via `inventory`) rather than shipping as an optional dlopen
//! plugin that could be missing.

use std::collections::HashMap;
use std::path::PathBuf;

use bevy::prelude::*;

#[cfg(feature = "scripting")]
mod script_extension;

/// Built-in language packs, embedded at compile time from the repo-root
/// `languages/` directory. English is the fallback and must stay first so a
/// half-written non-English pack can never starve the resolver of a base.
///
/// Adding a line here is the only edit needed to ship a new built-in language;
/// the pickers list whatever packs are registered.
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
    ("sv", include_str!("../../../languages/sv.toml")),
    ("da", include_str!("../../../languages/da.toml")),
    ("nb", include_str!("../../../languages/nb.toml")),
    ("fi", include_str!("../../../languages/fi.toml")),
    ("cs", include_str!("../../../languages/cs.toml")),
    ("hu", include_str!("../../../languages/hu.toml")),
    ("ro", include_str!("../../../languages/ro.toml")),
    ("el", include_str!("../../../languages/el.toml")),
    ("he", include_str!("../../../languages/he.toml")),
    ("ms", include_str!("../../../languages/ms.toml")),
];

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
            if let Err(e) = renzora_lang_api::register_pack_str(src) {
                error!("[lang] embedded pack '{code}' failed to parse: {e}");
            }
        }

        // 2. External packs (a marketplace install drops a file here). Loaded
        //    after built-ins so they win on conflicting keys.
        let mut external = ExternalPacks::default();
        scan_external_packs(&mut external);

        // 2b. The project's own packs: the game's strings, and possibly a
        //     language only the game provides. Read before the starting language
        //     is chosen, since the project's default may be one of them. A
        //     packed game's come from the archive via `ProjectLanguagePacks`; an
        //     on-disk project's are read from its folder, and in the editor
        //     `load_project_packs` reads them when a project opens.
        if let Some(packs) = app.world().get_resource::<renzora_lang_api::ProjectLanguagePacks>() {
            for (key, src) in &packs.sources {
                match renzora_lang_api::register_pack_str(src) {
                    Ok(()) => info!("[lang] loaded project pack {key}"),
                    Err(e) => error!("[lang] {key} parse error: {e}"),
                }
            }
        }
        if let Some(dir) = app
            .world()
            .get_resource::<renzora_project_api::CurrentProject>()
            .and_then(project_languages_dir)
        {
            scan_dirs(&mut external, &[dir]);
        }

        // 3. Initial active language: the editor user's preference in an editor
        //    session, the game's own in a game. See `initial_language`.
        //
        //    Both resources this reads are already in the world: `EditorSession`
        //    is inserted by `add_engine_plugins` before any engine plugin builds,
        //    and a game's `CurrentProject` is inserted by `RuntimePlugin` (the
        //    first foundation plugin) from the rpak or `project.toml`, while this
        //    plugin builds later, from the generated list.
        let is_editor = app
            .world()
            .get_resource::<renzora_project_api::EditorSession>()
            .is_some_and(|s| s.is_editor());
        let project_default = app
            .world()
            .get_resource::<renzora_project_api::CurrentProject>()
            .and_then(|p| p.config.default_language.clone());
        let available: Vec<String> = renzora_lang_api::available()
            .into_iter()
            .map(|meta| meta.code)
            .collect();
        let initial = initial_language(
            std::env::var("RENZORA_LANG").ok().as_deref(),
            is_editor,
            renzora_project_api::load_language,
            project_default.as_deref(),
            sys_locale::get_locale,
            &available,
        );
        renzora_lang_api::set_active(&initial);
        info!(
            "[lang] {} language(s) loaded, active = '{}'",
            renzora_lang_api::available().len(),
            renzora_lang_api::active_code(),
        );

        // The `languages/` folders sit beside the executable and in the working
        // directory, NOT under the project, so the project watcher does not see
        // them unless it is told to. This is the case `ExtraWatchRoots` exists
        // for: registering a directory that belongs to the engine install rather
        // than to the user's project.
        //
        // Registered even when the watcher plugin is absent (a shipped game
        // links this crate and not the watcher). The resource is then a list
        // nobody reads, which costs a `Vec` of two paths.
        {
            let mut roots = app
                .world_mut()
                .get_resource_or_insert_with(renzora_project_api::project_files::ExtraWatchRoots::default);
            for dir in external_dirs() {
                roots.add(dir);
            }
        }

        app.insert_resource(external)
            .add_message::<renzora_lang_api::LanguageChanged>()
            // Same idempotent registration as the other consumers: this crate is
            // in every build and the watcher is editor-only, so the message has
            // to exist whether or not anything writes to it.
            .add_message::<renzora_project_api::project_files::ProjectFileChanged>()
            .add_systems(
                Update,
                (
                    load_project_packs
                        .run_if(resource_exists_and_changed::<renzora_project_api::CurrentProject>),
                    rescan_external_packs,
                    emit_language_changed,
                )
                    .chain(),
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

/// The language a session starts in.
///
/// `RENZORA_LANG` wins everywhere: it is an explicit override for testing a
/// translation or taking screenshots. Otherwise:
///
/// - **Editor:** the editing user's interface language from
///   `~/.renzora/settings.toml` (`[app] language`). The Settings picker writes
///   it with `save_language`.
/// - **Game:** the project's `default_language`, else the player's system
///   language if a pack for it is loaded, else English.
///
/// A game used to take the editor branch too, so an exported game started in
/// whatever language the machine's editor user had picked, or in English on a
/// player's machine with no `settings.toml`, whatever the player spoke. The
/// language a game starts in is the game's setting, so it comes from the
/// project and the player, never from an editor preference file.
///
/// The editor preference and the system locale are passed as functions so
/// each is only read on the branch that uses it: the editor never asks the OS,
/// and a game never opens `settings.toml`.
fn initial_language(
    env_override: Option<&str>,
    is_editor: bool,
    editor_preference: impl FnOnce() -> String,
    project_default: Option<&str>,
    system_locale: impl FnOnce() -> Option<String>,
    available: &[String],
) -> String {
    if let Some(code) = env_override.map(str::trim).filter(|c| !c.is_empty()) {
        return code.to_string();
    }
    if is_editor {
        return editor_preference();
    }
    // The project's choice is taken as written, even without a loaded pack for
    // it: `set_active` accepts an unknown code and `t()` falls back to English
    // key by key, which is the same answer a typo would get from any check here,
    // and a pack installed beside the game later then lights up on its own.
    if let Some(code) = project_default.map(str::trim).filter(|c| !c.is_empty()) {
        return code.to_string();
    }
    system_locale()
        .and_then(|tag| match_locale(&tag, available))
        .unwrap_or_else(|| renzora_lang_api::FALLBACK_CODE.to_string())
}

/// Find the loaded pack that best fits an OS locale tag, or `None`.
///
/// The OS speaks full tags (`de-DE`, `pt_BR.UTF-8`, `zh-Hant-TW`) and the packs
/// are keyed by a bare language or a language and region (`de`, `pt-BR`,
/// `zh-TW`), so an exact lookup would miss almost every player. Candidates are
/// tried most specific first: the tag as given, then language and region, then
/// a Chinese script mapped to the pack that writes it, then the bare language.
/// Compared case-insensitively, because `pt-br` and `pt-BR` are the same tag.
fn match_locale(tag: &str, available: &[String]) -> Option<String> {
    // POSIX locales carry an encoding and a modifier (`de_DE.UTF-8@euro`) and
    // use `_`; neither is part of the language.
    let tag = tag.split(['.', '@']).next().unwrap_or("").replace('_', "-");
    let mut parts = tag.split('-').filter(|p| !p.is_empty());
    let language = parts.next()?.to_ascii_lowercase();
    let mut script = None;
    let mut region = None;
    for part in parts {
        if part.len() == 4 && part.chars().all(|c| c.is_ascii_alphabetic()) {
            script.get_or_insert(part.to_ascii_lowercase());
        } else if (part.len() == 2 && part.chars().all(|c| c.is_ascii_alphabetic()))
            || (part.len() == 3 && part.chars().all(|c| c.is_ascii_digit()))
        {
            region.get_or_insert(part.to_ascii_uppercase());
        }
    }

    let mut candidates = vec![tag.clone()];
    if let Some(region) = &region {
        candidates.push(format!("{language}-{region}"));
    }
    // Traditional Chinese is its own pack (`zh-TW`) and Simplified is `zh`; a
    // locale such as `zh-Hant-HK` names the script and a region no pack has.
    if language == "zh" {
        match script.as_deref() {
            Some("hant") => candidates.push("zh-TW".to_string()),
            Some("hans") => candidates.push("zh".to_string()),
            _ => {}
        }
    }
    // Some languages reach us under a code no pack is keyed by. Norway's OS
    // locales say `no` (or `nn` for Nynorsk) where the pack is Bokmål, `nb`, the
    // written form most players read; and older Java-derived locales still use
    // the withdrawn ISO codes `iw` for Hebrew and `in` for Indonesian.
    let alias = match language.as_str() {
        "no" | "nn" => Some("nb"),
        "iw" => Some("he"),
        "in" => Some("id"),
        _ => None,
    };
    candidates.push(language);
    if let Some(alias) = alias {
        candidates.push(alias.to_string());
    }

    candidates.iter().find_map(|candidate| {
        available
            .iter()
            .find(|code| code.eq_ignore_ascii_case(candidate))
            .cloned()
    })
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
                    renzora_lang_api::register_pack_str(&src)
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

/// Re-read the external packs when one of them changes on disk.
///
/// This ran on a two-second timer and re-`read_dir`'d both `languages/` folders
/// whether or not anything had happened. The watcher now says when one did, and
/// [`ExtraWatchRoots`] is how it came to be watching a directory outside the
/// project at all.
///
/// Still a directory scan rather than loading the one file named by the event:
/// [`scan_dirs`] already skips a pack whose mtime has not moved, so the rescan
/// re-parses exactly the file that changed, and a pack landing as a rename or a
/// batch of writes needs no special case.
///
/// [`ExtraWatchRoots`]: renzora_project_api::project_files::ExtraWatchRoots
fn rescan_external_packs(
    mut state: ResMut<ExternalPacks>,
    project: Option<Res<renzora_project_api::CurrentProject>>,
    mut changes: MessageReader<renzora_project_api::project_files::ProjectFileChanged>,
) {
    if changes.is_empty() {
        return;
    }
    let dirs = external_dirs();
    let project_dir = project.as_deref().and_then(project_languages_dir);
    let mut external_touched = false;
    let mut project_touched = false;
    for change in changes.read().filter(|c| c.has_extension("toml")) {
        // `root` rather than the path prefix for the external folders: these
        // events come from a watcher that was pointed at one of them, so the
        // root IS the answer. Project files arrive under the project root, so
        // the pack folder is matched by prefix.
        external_touched |= dirs.contains(&change.root);
        project_touched |= project_dir.as_ref().is_some_and(|d| change.path.starts_with(d));
    }
    if external_touched {
        scan_dirs(&mut state, &dirs);
    }
    if project_touched {
        if let Some(dir) = project_dir {
            scan_dirs(&mut state, &[dir]);
        }
    }
}

/// Load the open project's packs when the project changes: the editor opens a
/// project after this plugin built, so the build-time read never saw it.
/// [`scan_dirs`] skips files it has already read, so a game, whose packs were
/// read at build, re-parses nothing here.
fn load_project_packs(
    mut state: ResMut<ExternalPacks>,
    project: Res<renzora_project_api::CurrentProject>,
) {
    if let Some(dir) = project_languages_dir(&project) {
        scan_dirs(&mut state, &[dir]);
    }
}

/// Where a project keeps its language packs on disk. `None` for a packed game,
/// whose project path is the `"."` sentinel and whose packs come from the
/// archive through [`renzora_lang_api::ProjectLanguagePacks`] instead.
fn project_languages_dir(project: &renzora_project_api::CurrentProject) -> Option<PathBuf> {
    (project.path != std::path::Path::new(".")).then(|| project.path.join("languages"))
}

/// Bridge the lock-free global revision counter to a Bevy event: when anything
/// (active language, a new pack, a plugin contribution) bumps the revision,
/// fire `LanguageChanged` so panels caching translated text can rebuild.
fn emit_language_changed(
    mut last: Local<u64>,
    mut writer: bevy::ecs::message::MessageWriter<renzora_lang_api::LanguageChanged>,
) {
    let rev = renzora_lang_api::revision();
    // `Local` starts at 0; the build-time registrations already pushed the
    // revision past 0, so the first frame emits one change — exactly what we
    // want, to localize any UI built before this system first runs.
    if *last != rev {
        *last = rev;
        writer.write(renzora_lang_api::LanguageChanged {
            code: renzora_lang_api::active_code(),
        });
    }
}


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
                renzora_lang_api::register_pack_str(src).is_ok(),
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
        assert!(renzora_lang_api::has_language("zz-scan"));
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
        let after_first = renzora_lang_api::revision();

        scan_dirs(&mut state, &dirs);
        assert_eq!(
            renzora_lang_api::revision(),
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
            .add_message::<renzora_lang_api::LanguageChanged>()
            .add_systems(
                Update,
                (
                    emit_language_changed,
                    |mut reader: bevy::ecs::message::MessageReader<
                        renzora_lang_api::LanguageChanged,
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
        renzora_lang_api::register_pack_str(EMBEDDED_PACKS[0].1).unwrap();

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
        renzora_lang_api::register_pack_str(EMBEDDED_PACKS[0].1).unwrap();

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
        renzora_lang_api::register_pack_str(EMBEDDED_PACKS[0].1).unwrap();
        renzora_lang_api::register_pack_str(EMBEDDED_PACKS[1].1).unwrap();
        renzora_lang_api::set_active("en");

        let mut app = emit_app();
        app.update();
        let after_first = app.world().resource::<Seen>().0;

        renzora_lang_api::set_active("de");
        app.update();

        assert_eq!(app.world().resource::<Seen>().0, after_first + 1);
        renzora_lang_api::set_active("en");
    }

    fn codes(list: &[&str]) -> Vec<String> {
        list.iter().map(|c| c.to_string()).collect()
    }

    /// The pure decision, with an editor preference and system locale that fail
    /// the test if the branch under test should not have read them.
    fn game_start(project: Option<&str>, system: Option<&str>, available: &[&str]) -> String {
        initial_language(
            None,
            false,
            || panic!("a game read the editor's language preference"),
            project,
            || system.map(str::to_string),
            &codes(available),
        )
    }

    #[test]
    fn a_game_starts_in_the_projects_default_language() {
        assert_eq!(game_start(Some("de"), Some("fr-FR"), &["en", "de", "fr"]), "de");
    }

    #[test]
    fn a_game_without_a_default_follows_the_system_language() {
        assert_eq!(game_start(None, Some("fr-FR"), &["en", "de", "fr"]), "fr");
    }

    #[test]
    fn a_game_falls_back_to_english_when_the_system_language_has_no_pack() {
        assert_eq!(game_start(None, Some("sv-SE"), &["en", "de"]), "en");
        assert_eq!(game_start(None, None, &["en", "de"]), "en");
    }

    #[test]
    fn the_editor_keeps_its_own_preference() {
        let picked = initial_language(
            None,
            true,
            || "ja".to_string(),
            Some("de"),
            || panic!("the editor asked the OS for a language"),
            &codes(&["en", "de", "ja"]),
        );
        assert_eq!(picked, "ja", "the editor took the project's game language");
    }

    #[test]
    fn the_environment_override_wins_in_both_sessions() {
        for editor in [false, true] {
            let picked = initial_language(
                Some("ko"),
                editor,
                || "ja".to_string(),
                Some("de"),
                || Some("fr".to_string()),
                &codes(&["en"]),
            );
            assert_eq!(picked, "ko");
        }
    }

    #[test]
    fn system_locales_map_onto_the_packs_that_exist() {
        let packs = codes(&["en", "de", "pt-BR", "zh", "zh-TW"]);
        for (tag, expected) in [
            ("de-DE", Some("de")),
            ("de_AT.UTF-8@euro", Some("de")),
            ("pt-BR", Some("pt-BR")),
            ("pt_br", Some("pt-BR")),
            ("zh-Hant-TW", Some("zh-TW")),
            ("zh-Hant-HK", Some("zh-TW")),
            ("zh-Hans-CN", Some("zh")),
            ("zh-CN", Some("zh")),
            ("sv-SE", None),
            ("", None),
        ] {
            assert_eq!(match_locale(tag, &packs).as_deref(), expected, "{tag}");
        }
    }

    #[test]
    fn locale_codes_without_a_pack_of_their_own_fall_back_to_an_alias() {
        let packs = codes(&["en", "nb", "he", "id"]);
        for (tag, expected) in [
            ("nb-NO", Some("nb")),
            ("no_NO.UTF-8", Some("nb")),
            ("nn-NO", Some("nb")),
            ("iw-IL", Some("he")),
            ("he-IL", Some("he")),
            ("in-ID", Some("id")),
        ] {
            assert_eq!(match_locale(tag, &packs).as_deref(), expected, "{tag}");
        }
    }

    /// The plugin end to end: a game session reads `CurrentProject`, which
    /// `RuntimePlugin` inserts before this plugin builds.
    #[test]
    fn the_plugin_starts_a_game_in_the_projects_language() {
        let _guard = lang_lock();
        let mut config = renzora_project_api::ProjectConfig::default();
        config.default_language = Some("de".to_string());
        let mut app = App::new();
        app.insert_resource(renzora_project_api::EditorSession(false));
        app.insert_resource(renzora_project_api::CurrentProject {
            path: Default::default(),
            config,
        });
        // An environment override is a developer's explicit choice and wins by
        // design; with one set, this test has nothing to observe.
        if std::env::var_os("RENZORA_LANG").is_some() {
            return;
        }
        app.add_plugins(LangPlugin);
        assert_eq!(renzora_lang_api::active_code(), "de");
        renzora_lang_api::set_active("en");
    }

    /// The other half: the project's language is the game's, and an editor
    /// session opened on that project keeps the editing user's own.
    #[test]
    fn the_plugin_ignores_the_projects_language_in_the_editor() {
        let _guard = lang_lock();
        let mut config = renzora_project_api::ProjectConfig::default();
        config.default_language = Some("zz-game-only".to_string());
        let mut app = App::new();
        app.insert_resource(renzora_project_api::EditorSession(true));
        app.insert_resource(renzora_project_api::CurrentProject {
            path: Default::default(),
            config,
        });
        app.add_plugins(LangPlugin);
        assert_ne!(renzora_lang_api::active_code(), "zz-game-only");
        renzora_lang_api::set_active("en");
    }

    #[test]
    fn set_active_switches_the_reported_code() {
        let _guard = lang_lock();
        renzora_lang_api::register_pack_str(EMBEDDED_PACKS[0].1).unwrap();
        renzora_lang_api::set_active("en");
        assert_eq!(renzora_lang_api::active_code(), "en");
    }
}
