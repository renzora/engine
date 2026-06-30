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

#[cfg(any(feature = "lua", feature = "rhai"))]
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
        // registry — only when a backend is actually compiled in.
        #[cfg(any(feature = "lua", feature = "rhai"))]
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
    for dir in external_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
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
