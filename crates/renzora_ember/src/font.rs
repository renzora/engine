//! Fonts + text/icon helpers shared by every ember component and the editor
//! chrome. Noto Sans (proportional) is embedded; the Phosphor icon font is
//! reused from `renzora_hui` (folded into ember later).

use bevy::prelude::*;
use bevy::text::{Font, FontSize, FontSource};

use crate::theme::rgb;

/// Noto Sans, embedded so it's available regardless of the running app's
/// asset-root.
const NOTO_SANS: &[u8] = include_bytes!("../embedded/NotoSans-Regular.ttf");

/// JetBrains Mono — the monospace font for the code editor.
const JETBRAINS_MONO: &[u8] = include_bytes!("../embedded/JetBrainsMono-Regular.ttf");

/// Slight global down-scale so text matches the editor's size.
const TEXT_SCALE: f32 = 0.92;

/// Global UI font-size multiplier, driven by the editor's "Font Size" setting
/// (relative to the 14px default). Stored as an `f32`'s bits in an atomic so the
/// free-function [`ui_font`] can read it without a `World`. `1.0` = default.
static UI_FONT_SCALE: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0x3f80_0000); // 1.0_f32

/// Current global UI font-size multiplier (see [`UI_FONT_SCALE`]).
pub fn ui_font_scale() -> f32 {
    f32::from_bits(UI_FONT_SCALE.load(std::sync::atomic::Ordering::Relaxed))
}

/// Set the global UI font-size multiplier. New text built via [`ui_font`] picks
/// it up immediately; existing text is rescaled by `apply_font_settings`.
pub fn set_ui_font_scale(scale: f32) {
    UI_FONT_SCALE.store(scale.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

/// The fonts ember renders with. `ui` = the proportional UI font (Noto by
/// default, user-changeable via the theme); `mono` = the monospace font;
/// `phosphor` = the icon font.
///
/// `ui`/`mono` are [`FontSource`]s (not raw handles) so the UI font can be a
/// loaded asset (`Handle`), a system/loaded family by name (`Family`), or a
/// generic category (`SansSerif`, `SystemUi`, …) — Bevy 0.19's Parley backend
/// resolves all three. The default embedded Noto/JetBrains handles are kept in
/// `default_ui`/`default_mono` so "reset to default" never needs to reload them
/// and so the live font-swap can tell UI text apart from icon/mono text.
#[derive(Resource, Clone)]
pub struct EmberFonts {
    pub ui: FontSource,
    pub phosphor: Handle<Font>,
    pub mono: FontSource,
    pub default_ui: FontSource,
    pub default_mono: FontSource,
}

/// Build [`EmberFonts`] once the Phosphor font (loaded by HUI) is available.
pub(crate) fn load_fonts(
    mut commands: Commands,
    existing: Option<Res<EmberFonts>>,
    mut fonts: ResMut<Assets<Font>>,
    phosphor: Option<Res<crate::icons::PhosphorFont>>,
) {
    if existing.is_some() {
        return;
    }
    let Some(phosphor) = phosphor else {
        return;
    };
    // 0.19/Parley: `Font::from_bytes` (Result) → `Font::from_bytes` (infallible).
    let ui = FontSource::Handle(fonts.add(Font::from_bytes(NOTO_SANS.to_vec())));
    let mono = FontSource::Handle(fonts.add(Font::from_bytes(JETBRAINS_MONO.to_vec())));
    commands.insert_resource(EmberFonts {
        ui: ui.clone(),
        phosphor: phosphor.0.clone(),
        mono: mono.clone(),
        default_ui: ui,
        default_mono: mono,
    });
}

/// A `TextFont` in the given font source at the given (pre-scale) size.
///
/// Takes a [`FontSource`] so callers pass `&fonts.ui` / `&fonts.mono` (now
/// source-typed) and the UI font can be swapped to any family/generic at
/// runtime. Existing call sites are unchanged — they already passed `&fonts.ui`.
pub fn ui_font(font: &FontSource, size: f32) -> TextFont {
    TextFont {
        font: font.clone(),
        font_size: FontSize::Px(size * TEXT_SCALE * ui_font_scale()),
        ..default()
    }
}

// ── Font registry (built-ins + project `fonts/` folder) ─────────────────────

/// Where a registry font comes from — for grouping / icons in a picker.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FontOrigin {
    /// Embedded in the binary (the default Noto).
    Embedded,
    /// A generic family resolved by the OS via Parley (System UI, Sans, …).
    Generic,
    /// A `.ttf` / `.otf` discovered in the project's `fonts/` folder.
    Project,
}

/// One selectable font.
#[derive(Clone)]
pub struct FontEntry {
    /// Display + lookup name (file stem for project fonts).
    pub name: String,
    /// How to render it.
    pub source: FontSource,
    pub origin: FontOrigin,
}

/// The live list of fonts available to every UI font picker. Built-ins are
/// always present; project `fonts/*.{ttf,otf}` are scanned + loaded by
/// [`scan_project_fonts`] and appended, so dropdowns auto-populate when a font
/// is dropped into the folder. Runtime-safe, so the editor and a shipped game
/// share one registry.
#[derive(Resource, Default, Clone)]
pub struct FontRegistry {
    pub entries: Vec<FontEntry>,
    /// Project `fonts/` dir at last scan (change detection).
    scanned_dir: Option<std::path::PathBuf>,
    /// Signature (file set + mtimes) of that dir at last scan.
    dir_sig: u64,
}

impl FontRegistry {
    /// The always-present built-in entries: the embedded default + generic
    /// families. Shared by the editor (disk) and game (VFS) scanners so the
    /// option set is identical. `default_ui` is the embedded UI font source.
    pub fn builtin_entries(default_ui: FontSource) -> Vec<FontEntry> {
        vec![
            FontEntry {
                name: "Default".into(),
                source: default_ui,
                origin: FontOrigin::Embedded,
            },
            FontEntry {
                name: "System UI".into(),
                source: FontSource::SystemUi,
                origin: FontOrigin::Generic,
            },
            FontEntry {
                name: "Sans Serif".into(),
                source: FontSource::SansSerif,
                origin: FontOrigin::Generic,
            },
            FontEntry {
                name: "Serif".into(),
                source: FontSource::Serif,
                origin: FontOrigin::Generic,
            },
            FontEntry {
                name: "Monospace".into(),
                source: FontSource::Monospace,
                origin: FontOrigin::Generic,
            },
        ]
    }

    /// Resolve a font name to its render source (`None` if unknown).
    pub fn resolve(&self, name: &str) -> Option<FontSource> {
        self.entries
            .iter()
            .find(|e| e.name == name)
            .map(|e| e.source.clone())
    }

    /// Display names of just the project-folder fonts (the "custom" section of
    /// a picker).
    pub fn project_names(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|e| e.origin == FontOrigin::Project)
            .map(|e| e.name.clone())
            .collect()
    }
}

/// A cheap hash of the `fonts/` folder (`.ttf`/`.otf` filenames + mtimes) so the
/// scanner can skip work unless something actually changed.
fn font_dir_signature(dir: &std::path::Path) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        let mut items: Vec<(String, u64)> = rd
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                let ext = p
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_ascii_lowercase());
                if !matches!(ext.as_deref(), Some("ttf") | Some("otf")) {
                    return None;
                }
                let name = p.file_name()?.to_str()?.to_string();
                let mtime = e
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                Some((name, mtime))
            })
            .collect();
        items.sort();
        items.hash(&mut h);
    }
    h.finish()
}

/// Keep [`FontRegistry`] in sync with the project's `fonts/` folder: load new
/// `.ttf`/`.otf` files into `Assets<Font>` and drop removed ones. Polls the
/// folder a few times a second (throttled) so dropping a font in is picked up
/// live without a per-frame `read_dir`.
pub(crate) fn scan_project_fonts(
    mut tick: Local<u32>,
    mut registry: ResMut<FontRegistry>,
    asset_server: Res<AssetServer>,
    ember: Option<Res<EmberFonts>>,
    project: Option<Res<renzora::core::CurrentProject>>,
) {
    *tick = tick.wrapping_add(1);
    // First population runs immediately; afterwards poll ~twice a second.
    if !registry.entries.is_empty() && *tick % 30 != 0 {
        return;
    }
    let Some(ember) = ember else {
        return; // fonts not ready yet
    };

    let dir = project.as_ref().map(|p| p.path.join("fonts"));
    let sig = dir.as_deref().map(font_dir_signature).unwrap_or(0);
    if !registry.entries.is_empty()
        && registry.scanned_dir.as_deref() == dir.as_deref()
        && registry.dir_sig == sig
    {
        return; // nothing changed
    }

    // Rebuild the list: built-ins first, then project fonts. We discover the
    // file names on disk (for the picker list) but LOAD each through the asset
    // server by its project-relative path — so the handle is path-backed. That
    // matters downstream: the path is what the scene `.bsn` records for a
    // `TextFont`, what the exporter's reference trace packs, and what a shipped
    // game loads from the rpak (no game-side font scanner needed).
    let mut entries = FontRegistry::builtin_entries(ember.default_ui.clone());
    if let Some(dir) = &dir {
        if let Ok(rd) = std::fs::read_dir(dir) {
            let mut files: Vec<std::path::PathBuf> = rd
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    matches!(
                        p.extension()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_ascii_lowercase())
                            .as_deref(),
                        Some("ttf") | Some("otf")
                    )
                })
                .collect();
            files.sort();
            for path in files {
                let (Some(stem), Some(file)) = (
                    path.file_stem().and_then(|s| s.to_str()),
                    path.file_name().and_then(|s| s.to_str()),
                ) else {
                    continue;
                };
                // Project-relative asset path (asset_server dedupes by path).
                let handle = asset_server.load::<Font>(format!("fonts/{file}"));
                entries.push(FontEntry {
                    name: stem.to_string(),
                    source: FontSource::Handle(handle),
                    origin: FontOrigin::Project,
                });
            }
        }
    }

    registry.entries = entries;
    registry.scanned_dir = dir;
    registry.dir_sig = sig;
}

/// Resolve a Phosphor icon name (e.g. `"caret-down"`) to its glyph char, for
/// binding an icon that changes at runtime. Returns `None` for unknown names.
pub use crate::phosphor_map::icon_glyph;

/// An inline Phosphor glyph resolved immediately (real glyph + Phosphor font),
/// so rebuilding the entity doesn't flash a blank frame like the deferred
/// `Icon` component would.
pub fn icon_text(
    commands: &mut Commands,
    phosphor: &Handle<Font>,
    name: &str,
    color: (u8, u8, u8),
    size: f32,
) -> Entity {
    let ch = crate::phosphor_map::icon_glyph(name).unwrap_or('\u{E4C6}');
    commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                ..default()
            },
            Text::new(ch.to_string()),
            TextFont {
                font: bevy::text::FontSource::Handle(phosphor.clone()),
                font_size: bevy::text::FontSize::Px(size),
                ..default()
            },
            TextColor(rgb(color)),
            Name::new(format!("icon:{name}")),
        ))
        .id()
}

/// A deferred Phosphor glyph (resolved a frame later by HUI's `apply_icons`) —
/// fine for chrome built once.
pub fn glyph(commands: &mut Commands, name: &str, color: (u8, u8, u8), size: f32) -> Entity {
    commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                ..default()
            },
            crate::icons::Icon::new(name.to_string(), size, Some(rgb(color))),
            Name::new(format!("glyph:{name}")),
        ))
        .id()
}

/// A padded deferred Phosphor icon button (e.g. chrome action buttons).
pub fn icon_item(commands: &mut Commands, name: &str, color: (u8, u8, u8), size: f32) -> Entity {
    commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                align_items: AlignItems::Center,
                ..default()
            },
            crate::icons::Icon::new(name.to_string(), size, Some(rgb(color))),
            Name::new(format!("icon:{name}")),
        ))
        .id()
}
