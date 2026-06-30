//! Engine localization contract — the one translation table every crate reads.
//!
//! ## Why a process-global, not a Bevy resource
//!
//! `t("menu.file")` has to be callable from *anywhere*: ECS systems, but also
//! widget builders, error formatters, and `Display` impls that have no `World`
//! handle. A `Resource` can't serve those call sites. So the active strings live
//! behind a process-global `RwLock`, populated by the `renzora_lang`
//! plugin (embedded built-ins + external `languages/*.toml` packs) and by any
//! plugin contributing its own keys via [`register_translations`] /
//! [`register_pack_str`]. This is the same "shared state in the contract dylib"
//! pattern the runtime-warnings ring buffer uses: one global, one `TypeId`,
//! reachable across the dlopen boundary.
//!
//! ## Resolution + graceful degradation
//!
//! [`t`] resolves **active language → English → the key itself**. A missing
//! translation therefore renders as readable English (or, worst case, the dotted
//! key) instead of blanking the UI. That means the engine is usable the instant
//! the runtime links it, before a single call site is converted — converting a
//! literal to `t("…")` only *adds* the ability to translate it.
//!
//! ## Plugin API (what other `renzora_*` plugins use)
//!
//! A plugin localizes its own UI by registering strings for each language it
//! ships, in its `build()`:
//!
//! ```rust,ignore
//! // From an embedded toml pack (recommended — same `[meta]`/`[strings]`
//! // format as the built-in language files):
//! let _ = renzora::lang::register_pack_str(include_str!("../languages/de.toml"));
//!
//! // …or inline, with no toml file (handy for a handful of keys):
//! renzora::lang::register_translations("en", [
//!     ("myplugin.title", "My Plugin"),
//!     ("myplugin.run",   "Run"),
//! ]);
//! ```
//!
//! Registration order doesn't matter: the store accumulates every contribution,
//! and `t()` resolves against the active language at call time. Later writes to
//! the same `(code, key)` win, so an external pack or a user can override a
//! built-in string.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{OnceLock, RwLock};

use bevy::prelude::*;
use serde::Deserialize;

/// The fallback language. Every built-in key is guaranteed present here, so it
/// is the second link in the resolution chain (after the active language) and
/// the language `t()` returns when nothing else matches a key.
pub const FALLBACK_CODE: &str = "en";

/// Metadata header of a language pack — the `[meta]` table of a `.toml`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct LocaleMeta {
    /// Native display name shown in the picker, e.g. `"Français"`, `"日本語"`.
    #[serde(default)]
    pub name: String,
    /// BCP-47-ish code used everywhere else, e.g. `"fr"`, `"pt-BR"`, `"zh"`.
    #[serde(default)]
    pub code: String,
    /// Optional credit, surfaced in the Language Packs UI.
    #[serde(default)]
    pub author: String,
    /// Optional pack version string.
    #[serde(default)]
    pub version: String,
}

/// A parsed language pack: its metadata plus the `key → translated string` map.
#[derive(Clone, Debug, Default)]
pub struct LocalePack {
    pub meta: LocaleMeta,
    pub strings: HashMap<String, String>,
}

/// Wire format: `[meta]` + `[strings]`. Kept private; callers go through
/// [`parse_pack`] which validates the code is non-empty.
#[derive(Deserialize)]
struct RawPack {
    #[serde(default)]
    meta: LocaleMeta,
    #[serde(default)]
    strings: HashMap<String, String>,
}

/// Parse a `[meta]` + `[strings]` toml pack. The `Err` carries a human-readable
/// reason (the toml error, or that `[meta].code` is missing) so the loader can
/// log *which* pack failed without aborting the others.
pub fn parse_pack(src: &str) -> Result<LocalePack, String> {
    let raw: RawPack = toml::from_str(src).map_err(|e| e.to_string())?;
    if raw.meta.code.trim().is_empty() {
        return Err("language pack is missing [meta].code".into());
    }
    Ok(LocalePack {
        meta: raw.meta,
        strings: raw.strings,
    })
}

// ── Global store ───────────────────────────────────────────────────────────
//
// `HashMap::new()` is not `const`, so the store can't be a bare `static`; a
// `OnceLock` gives us lazy first-touch init with no external crate.

struct Store {
    active: String,
    langs: HashMap<String, LocalePack>,
}

static STORE: OnceLock<RwLock<Store>> = OnceLock::new();

/// Bumped on every change to the table or the active language. Reactive UIs
/// gate their rebuilds on this rather than re-translating every frame.
static REVISION: AtomicU64 = AtomicU64::new(0);

fn store() -> &'static RwLock<Store> {
    STORE.get_or_init(|| {
        RwLock::new(Store {
            active: FALLBACK_CODE.to_string(),
            langs: HashMap::new(),
        })
    })
}

#[inline]
fn bump() {
    REVISION.fetch_add(1, Ordering::Relaxed);
}

/// Monotonic change counter. Increments whenever translations are registered or
/// the active language changes. Compare against a stored value to know when to
/// rebuild localized UI; never decreases.
#[inline]
pub fn revision() -> u64 {
    REVISION.load(Ordering::Relaxed)
}

// ── Registration ───────────────────────────────────────────────────────────

/// Merge a parsed pack into the store, creating the language if new or extending
/// it if already present. Later keys override earlier ones, so external packs
/// and plugin contributions can override built-ins.
pub fn register_pack(pack: LocalePack) {
    {
        let mut s = store().write().unwrap();
        let entry = s
            .langs
            .entry(pack.meta.code.clone())
            .or_insert_with(LocalePack::default);
        // First non-empty meta wins for display fields, so a later strings-only
        // contribution (e.g. a plugin adding its keys) can't blank the name.
        if entry.meta.code.is_empty() {
            entry.meta.code = pack.meta.code;
        }
        if entry.meta.name.is_empty() {
            entry.meta.name = pack.meta.name;
        }
        if entry.meta.author.is_empty() {
            entry.meta.author = pack.meta.author;
        }
        if entry.meta.version.is_empty() {
            entry.meta.version = pack.meta.version;
        }
        entry.strings.extend(pack.strings);
    }
    bump();
}

/// Parse and register a toml pack string. Convenience for plugins shipping an
/// embedded pack via `include_str!`. Returns the parse error (logged by the
/// caller) without panicking, so one malformed pack can't take down the engine.
pub fn register_pack_str(src: &str) -> Result<(), String> {
    register_pack(parse_pack(src)?);
    Ok(())
}

/// Register inline `(key, value)` pairs for one language — the no-toml path for
/// plugins that only need a handful of strings. Merges into `code`, creating it
/// if absent.
pub fn register_translations<I, K, V>(code: &str, pairs: I)
where
    I: IntoIterator<Item = (K, V)>,
    K: Into<String>,
    V: Into<String>,
{
    {
        let mut s = store().write().unwrap();
        let entry = s.langs.entry(code.to_string()).or_insert_with(|| LocalePack {
            meta: LocaleMeta {
                code: code.to_string(),
                ..Default::default()
            },
            strings: HashMap::new(),
        });
        for (k, v) in pairs {
            entry.strings.insert(k.into(), v.into());
        }
    }
    bump();
}

// ── Active language ────────────────────────────────────────────────────────

/// Switch the active language. An *unknown* code is still stored, so a pack
/// installed later (e.g. from the marketplace) lights up without a second call;
/// until then `t()` simply falls back. No-op if already active.
pub fn set_active(code: &str) {
    {
        let mut s = store().write().unwrap();
        if s.active == code {
            return;
        }
        s.active = code.to_string();
    }
    bump();
}

/// The active language code (e.g. `"en"`).
pub fn active_code() -> String {
    store().read().unwrap().active.clone()
}

/// Whether a pack for `code` is currently registered.
pub fn has_language(code: &str) -> bool {
    store().read().unwrap().langs.contains_key(code)
}

/// Metadata for every registered language, sorted by native name — what the
/// settings picker and Language Packs UI iterate.
pub fn available() -> Vec<LocaleMeta> {
    let s = store().read().unwrap();
    let mut v: Vec<LocaleMeta> = s.langs.values().map(|p| p.meta.clone()).collect();
    v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    v
}

// ── Lookup ─────────────────────────────────────────────────────────────────

/// Translate `key` in the active language, falling back active → English → the
/// key string itself. Returns an owned `String` because virtually every call
/// site feeds a `Text`/label that owns its content anyway.
pub fn t(key: &str) -> String {
    let s = store().read().unwrap();
    if let Some(p) = s.langs.get(&s.active) {
        if let Some(v) = p.strings.get(key) {
            return v.clone();
        }
    }
    if s.active != FALLBACK_CODE {
        if let Some(p) = s.langs.get(FALLBACK_CODE) {
            if let Some(v) = p.strings.get(key) {
                return v.clone();
            }
        }
    }
    key.to_string()
}

/// Like [`t`], but fall back to a caller-supplied `default` (typically the
/// already-humanized English string) instead of the raw key. For labels derived
/// from reflection — component names, field names, panel titles — where the key
/// (`comp.clouds.coverage`) would be ugly if shown but the default (`Coverage`)
/// is perfectly readable. Resolution: active language → English → `default`.
pub fn t_or(key: &str, default: &str) -> String {
    let s = store().read().unwrap();
    if let Some(p) = s.langs.get(&s.active) {
        if let Some(v) = p.strings.get(key) {
            return v.clone();
        }
    }
    if s.active != FALLBACK_CODE {
        if let Some(p) = s.langs.get(FALLBACK_CODE) {
            if let Some(v) = p.strings.get(key) {
                return v.clone();
            }
        }
    }
    default.to_string()
}

/// Translate `key` in a specific language (for previews or per-player UI),
/// with the same English/key fallback as [`t`].
pub fn t_in(code: &str, key: &str) -> String {
    let s = store().read().unwrap();
    if let Some(p) = s.langs.get(code) {
        if let Some(v) = p.strings.get(key) {
            return v.clone();
        }
    }
    if code != FALLBACK_CODE {
        if let Some(p) = s.langs.get(FALLBACK_CODE) {
            if let Some(v) = p.strings.get(key) {
                return v.clone();
            }
        }
    }
    key.to_string()
}

/// Translate `key`, then substitute `{name}` placeholders from `args`. Lets one
/// localized template carry runtime values, e.g.
/// `t_args("status.saved", &[("file", &name)])` over `"Saved {file}"`.
/// Placeholders with no matching arg are left intact.
pub fn t_args(key: &str, args: &[(&str, &str)]) -> String {
    let mut out = t(key);
    for (name, value) in args {
        let needle = format!("{{{name}}}");
        if out.contains(&needle) {
            out = out.replace(&needle, value);
        }
    }
    out
}

// ── Bevy-facing event ──────────────────────────────────────────────────────

/// Fired by the localization plugin after the active language changes (or packs
/// are (re)loaded), so panels that cache translated text can rebuild. Reactive
/// UIs may instead poll [`revision`]. A buffered `Message` (Bevy 0.19), read
/// with `MessageReader`.
#[derive(Message, Debug, Clone)]
pub struct LanguageChanged {
    pub code: String,
}
