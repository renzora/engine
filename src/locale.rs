use bevy::prelude::*;
use serde::Deserialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

// Built-in locale strings embedded in the binary
const LOCALE_EN: &str = include_str!("../assets/locale/en.toml");
const LOCALE_FR: &str = include_str!("../assets/locale/fr.toml");
const LOCALE_DE: &str = include_str!("../assets/locale/de.toml");
const LOCALE_ES: &str = include_str!("../assets/locale/es.toml");
const LOCALE_JA: &str = include_str!("../assets/locale/ja.toml");

const BUILTIN_LOCALES: &[(&str, &str)] = &[
    ("en", LOCALE_EN),
    ("fr", LOCALE_FR),
    ("de", LOCALE_DE),
    ("es", LOCALE_ES),
    ("ja", LOCALE_JA),
];

// Thread-local active locale strings — set once per frame before UI rendering
thread_local! {
    static ACTIVE_STRINGS: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

/// Translate a key using the active locale. Falls back to the key itself if not found.
/// This reads from the thread-local set by `set_active_locale` each frame.
pub fn t(key: &str) -> String {
    ACTIVE_STRINGS.with(|strings| {
        strings
            .borrow()
            .get(key)
            .cloned()
            .unwrap_or_else(|| key.to_string())
    })
}

/// Update the thread-local translation table. Call this once per frame before rendering UI.
pub fn set_active_locale(strings: &HashMap<String, String>) {
    ACTIVE_STRINGS.with(|active| {
        *active.borrow_mut() = strings.clone();
    });
}

/// Shorthand translation macro — calls `crate::locale::t(key)` and returns a `String`.
/// Usage: `t!("settings.tab.general")` or `&t!("key")` where `&str` is needed.
#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::locale::t($key)
    };
}

// ─── Locale file format ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct LocaleFile {
    meta: LocaleFileMeta,
    strings: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct LocaleFileMeta {
    name: String,
    code: String,
    #[serde(default)]
    author: String,
    #[serde(default = "default_version")]
    version: String,
}

fn default_version() -> String {
    "1.0".to_string()
}

// ─── LocaleInfo ───────────────────────────────────────────────────────────────

/// Metadata about an available locale (built-in or user-installed pack)
#[derive(Debug, Clone)]
pub struct LocaleInfo {
    /// Human-readable display name (e.g. "Français")
    pub name: String,
    /// BCP-47-style code (e.g. "fr")
    pub code: String,
    /// Author credit
    pub author: String,
    /// Pack version string
    pub version: String,
    /// True for locales embedded in the binary
    pub is_builtin: bool,
    /// Disk path for user-installed packs; None for built-ins
    pub path: Option<PathBuf>,
}

// ─── LocaleResource ───────────────────────────────────────────────────────────

/// Bevy resource that manages the active locale and all discovered locale packs.
#[derive(Resource)]
pub struct LocaleResource {
    /// Currently active locale code (e.g. "en", "fr")
    pub current: String,
    /// Translation strings for the active locale
    pub strings: HashMap<String, String>,
    /// All available locales (built-in + user-installed)
    pub available: Vec<LocaleInfo>,
}

impl Default for LocaleResource {
    fn default() -> Self {
        let mut res = Self {
            current: "en".to_string(),
            strings: HashMap::new(),
            available: Vec::new(),
        };
        res.discover_locales();
        res.load_locale("en");
        res
    }
}

impl LocaleResource {
    /// Directory where users can install custom language pack .toml files.
    /// Platform path: `~/.config/bevy_editor/locale/`
    pub fn user_locale_dir() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("bevy_editor").join("locale"))
    }

    /// Enumerate all user-installed locale files in the packs directory.
    pub fn user_locale_files() -> Vec<PathBuf> {
        Self::user_locale_dir()
            .map(|dir| {
                if dir.exists() {
                    std::fs::read_dir(&dir)
                        .ok()
                        .map(|entries| {
                            entries
                                .filter_map(|e| e.ok())
                                .map(|e| e.path())
                                .filter(|p| {
                                    p.extension().map(|e| e == "toml").unwrap_or(false)
                                })
                                .collect()
                        })
                        .unwrap_or_default()
                } else {
                    Vec::new()
                }
            })
            .unwrap_or_default()
    }

    /// Parse only the [meta] section from a locale TOML string to get info without loading all strings.
    fn parse_info(content: &str, is_builtin: bool, path: Option<PathBuf>) -> Option<LocaleInfo> {
        let file: LocaleFile = toml::from_str(content).ok()?;
        Some(LocaleInfo {
            name: file.meta.name,
            code: file.meta.code,
            author: file.meta.author,
            version: file.meta.version,
            is_builtin,
            path,
        })
    }

    /// Scan built-in and user locale directories to populate `self.available`.
    pub fn discover_locales(&mut self) {
        let mut locales: Vec<LocaleInfo> = Vec::new();

        // Built-in locales (embedded in binary)
        for (_, content) in BUILTIN_LOCALES {
            if let Some(info) = Self::parse_info(content, true, None) {
                locales.push(info);
            }
        }

        // User-installed packs — override built-in if same code, otherwise add
        for path in Self::user_locale_files() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Some(info) = Self::parse_info(&content, false, Some(path)) {
                    if let Some(existing) = locales.iter_mut().find(|l| l.code == info.code) {
                        // User pack overrides the built-in entry
                        *existing = info;
                    } else {
                        locales.push(info);
                    }
                }
            }
        }

        self.available = locales;
    }

    /// Parse all translation strings from a TOML locale string.
    fn parse_strings(content: &str) -> Option<HashMap<String, String>> {
        toml::from_str::<LocaleFile>(content).ok().map(|f| f.strings)
    }

    /// Load a locale by its code. Strings are merged on top of English so any
    /// untranslated key falls back to its English value rather than the raw key.
    pub fn load_locale(&mut self, code: &str) {
        // Always start with English as the base layer
        let base = BUILTIN_LOCALES
            .iter()
            .find(|(c, _)| *c == "en")
            .and_then(|(_, content)| Self::parse_strings(content))
            .unwrap_or_default();

        if code == "en" {
            self.strings = base;
            self.current = "en".to_string();
            return;
        }

        // Try built-in overlay
        if let Some((_, content)) = BUILTIN_LOCALES.iter().find(|(c, _)| *c == code) {
            if let Some(overlay) = Self::parse_strings(content) {
                let mut merged = base;
                merged.extend(overlay);
                self.strings = merged;
                self.current = code.to_string();
                return;
            }
        }

        // Try user-pack overlay
        if let Some(info) = self
            .available
            .iter()
            .find(|l| l.code == code && !l.is_builtin)
            .cloned()
        {
            if let Some(path) = info.path {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Some(overlay) = Self::parse_strings(&content) {
                        let mut merged = base;
                        merged.extend(overlay);
                        self.strings = merged;
                        self.current = code.to_string();
                        return;
                    }
                }
            }
        }

        // Locale not found — stay with English base
        warn!("Locale '{}' not found, falling back to English", code);
        self.strings = base;
        self.current = "en".to_string();
    }

    /// Look up a translation key. Returns the key itself when no translation exists.
    pub fn t<'a>(&'a self, key: &'a str) -> &'a str {
        self.strings.get(key).map(|s| s.as_str()).unwrap_or(key)
    }
}

// ─── Plugin ───────────────────────────────────────────────────────────────────

pub struct LocalePlugin;

impl Plugin for LocalePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LocaleResource>();
        app.add_systems(Startup, init_locale_from_config);
    }
}

/// On startup, load the language saved in AppConfig and prime the thread-local.
fn init_locale_from_config(
    mut locale: ResMut<LocaleResource>,
    app_config: Res<crate::project::AppConfig>,
) {
    let lang = app_config.language.clone();
    if !lang.is_empty() && lang != "en" {
        locale.load_locale(&lang);
    }
    set_active_locale(&locale.strings);
}
