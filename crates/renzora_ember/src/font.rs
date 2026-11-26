//! Fonts + text/icon helpers shared by every ember component and the editor
//! chrome. Inter (the default UI font) and Noto Sans are embedded; the Phosphor
//! icon font is reused from `renzora_hui` (folded into ember later).

use bevy::prelude::*;
use bevy::text::{Font, FontSize, FontSource};

use crate::theme::rgb;

/// Inter, the default UI font. Embedded so it's available regardless of the
/// running app's asset-root. SIL Open Font License; see
/// `embedded/Inter-LICENSE.txt`.
///
/// It replaced Noto Sans as the default because it is drawn for dense
/// interfaces: narrower letters and tighter spacing fit more of a name into a
/// tree row or an inspector label at the same pixel size, where Noto's wide,
/// tall lowercase read as large and soft.
const INTER: &[u8] = include_bytes!("../embedded/Inter-Regular.ttf");

/// Noto Sans, the previous default, kept as a choice in Settings.
const NOTO_SANS: &[u8] = include_bytes!("../embedded/NotoSans-Regular.ttf");

/// The embedded UI fonts that are not the default, by name, so the settings
/// can offer them without a reload. The default lives in
/// [`EmberFonts::default_ui`].
#[derive(Resource, Clone)]
pub struct BuiltinUiFonts {
    pub noto_sans: FontSource,
}

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

/// A theme-supplied UI font override. A folder theme can ship its own font (its
/// `[fonts] ui` points at a file inside the theme folder); the shell loads it and
/// sets it here. `apply_font_settings` then prefers this over the user's font
/// setting *while that theme is active*, and reverts when it's cleared (`None`).
/// Runtime-safe (no `World`), like the color palette.
static THEME_UI_FONT: std::sync::LazyLock<std::sync::RwLock<Option<FontSource>>> =
    std::sync::LazyLock::new(|| std::sync::RwLock::new(None));

/// Set (or clear, with `None`) the active theme's UI font override.
pub fn set_theme_ui_font(src: Option<FontSource>) {
    if let Ok(mut g) = THEME_UI_FONT.write() {
        *g = src;
    }
}

/// The active theme's UI font override, if any.
pub fn theme_ui_font() -> Option<FontSource> {
    THEME_UI_FONT.read().ok().and_then(|g| g.clone())
}

/// The fonts ember renders with. `ui` = the proportional UI font (Inter by
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
    let ui = FontSource::Handle(fonts.add(Font::from_bytes(INTER.to_vec())));
    let mono = FontSource::Handle(fonts.add(Font::from_bytes(JETBRAINS_MONO.to_vec())));
    commands.insert_resource(BuiltinUiFonts {
        noto_sans: FontSource::Handle(fonts.add(Font::from_bytes(NOTO_SANS.to_vec()))),
    });
    commands.insert_resource(EmberFonts {
        ui: ui.clone(),
        phosphor: phosphor.0.clone(),
        mono: mono.clone(),
        default_ui: ui,
        default_mono: mono,
    });
}

/// Rendered font sizes are snapped to these values (in final px, after
/// [`TEXT_SCALE`] and [`ui_font_scale`]).
///
/// **Why this exists — an 8-entry LRU cliff in the text stack.** Rasterizing a
/// glyph run builds a hinted `swash` scaler, and swash caches those in a
/// fixed-size table of exactly **8** entries (`MAX_CACHED_HINT_INSTANCES`), keyed
/// on `(font id, size, variation coords)` and shared across every TrueType
/// (`glyf`) font — which all three embedded fonts are. On a miss it re-runs the
/// font's `fpgm` and `prep` bytecode through the TrueType interpreter, tens of µs
/// each. Bevy also builds that scaler *before* checking the glyph atlas, so a
/// fully-warm atlas still pays it.
///
/// Unsnapped, ember spread text across ~59 distinct `(font, size)` pairs — against
/// 8 slots, visited in archetype order, that is a ~100% miss rate. Profiling
/// caught `text_system` at 18.7 ms on a frame that spawned ~1000 text nodes;
/// without the thrash the same burst is a few ms. Each pair also owns a 512×512
/// RGBA atlas (1 MiB), so the spread cost ~59 MiB too.
///
/// The three dominant authored sizes (11.0 → 297 call sites, 10.0 → 142,
/// 12.0 → 120; together ~65% of all text) land on just the 9 and 11 rungs.
///
/// **Tuning:** fewer rungs = fewer cache misses. Fitting *entirely* inside the 8
/// slots is not reachable without a visual redesign — three fonts share the table,
/// so it would mean ~2-3 sizes each — so this is deliberately a large reduction
/// rather than a complete fix. Removing a rung is a design decision about how the
/// editor looks; measure `text_system` before and after if you change it.
const FONT_SIZE_LADDER: [f32; 6] = [9.0, 11.0, 13.0, 16.0, 20.0, 26.0];

/// Snap a final pixel size to the nearest [`FONT_SIZE_LADDER`] rung.
///
/// Set `RENZORA_NO_FONT_LADDER=1` to bypass this and get the old
/// one-atlas-per-authored-size behaviour — there to A/B the change against a
/// profile, not because unsnapped is ever preferable.
pub fn snap_font_px(px: f32) -> f32 {
    if std::env::var_os("RENZORA_NO_FONT_LADDER").is_some() {
        return px;
    }
    // Sizes beyond the top rung are rare and usually deliberate (splash/hero
    // text); snapping them down would be visible, so let them through.
    if px > FONT_SIZE_LADDER[FONT_SIZE_LADDER.len() - 1] {
        return px;
    }
    let mut best = FONT_SIZE_LADDER[0];
    let mut best_d = (px - best).abs();
    for rung in FONT_SIZE_LADDER.iter().skip(1) {
        let d = (px - rung).abs();
        if d < best_d {
            best = *rung;
            best_d = d;
        }
    }
    best
}

/// A `TextFont` in the given font source at the given (pre-scale) size.
///
/// Takes a [`FontSource`] so callers pass `&fonts.ui` / `&fonts.mono` (now
/// source-typed) and the UI font can be swapped to any family/generic at
/// runtime. Existing call sites are unchanged — they already passed `&fonts.ui`.
///
/// The resulting size is snapped to [`FONT_SIZE_LADDER`]; see there for why.
pub fn ui_font(font: &FontSource, size: f32) -> TextFont {
    TextFont {
        font: font.clone(),
        font_size: FontSize::Px(snap_font_px(size * TEXT_SCALE * ui_font_scale())),
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
    /// A `.ttf` / `.otf` discovered in the project's `fonts/` folder, at any
    /// depth.
    Project,
}

/// One selectable font.
#[derive(Clone)]
pub struct FontEntry {
    /// Display + lookup name: the file stem for project fonts, with its folder
    /// added when two project fonts share a stem.
    pub name: String,
    /// How to render it.
    pub source: FontSource,
    pub origin: FontOrigin,
    /// A project font's project-relative path (`fonts/isometra/Isometra.ttf`).
    /// This, not [`Self::name`], is what a setting should store: a shipped
    /// game reads its fonts from the packed archive and has no folder to scan,
    /// so a bare name finds nothing there, while a path loads anywhere.
    pub path: Option<String>,
}

/// The live list of fonts available to every UI font picker. Built-ins are
/// always present; every `.ttf`/`.otf` under `fonts/` is loaded by
/// [`scan_project_fonts`] and appended, so dropdowns auto-populate when a font
/// is added to the project. Runtime-safe, so the editor and a shipped game
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
        let entry = |name: &str, source: FontSource, origin: FontOrigin| FontEntry {
            name: name.into(),
            source,
            origin,
            path: None,
        };
        vec![
            entry("Default", default_ui, FontOrigin::Embedded),
            entry("System UI", FontSource::system_ui(), FontOrigin::Generic),
            entry("Sans Serif", FontSource::sans_serif(), FontOrigin::Generic),
            entry("Serif", FontSource::serif(), FontOrigin::Generic),
            entry("Monospace", FontSource::monospace(), FontOrigin::Generic),
        ]
    }

    /// Resolve a font, by name or by a project font's path, to its render
    /// source (`None` if unknown).
    pub fn resolve(&self, name: &str) -> Option<FontSource> {
        self.entries
            .iter()
            .find(|e| e.name == name || e.path.as_deref() == Some(name))
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

    /// The project-folder fonts as `(display name, project-relative path)`,
    /// for a picker whose choice is saved into the project and must load in a
    /// shipped game (see [`FontEntry::path`]).
    pub fn project_fonts(&self) -> Vec<(String, String)> {
        self.entries
            .iter()
            .filter_map(|e| Some((e.name.clone(), e.path.clone()?)))
            .collect()
    }

    /// Changes whenever the set of project fonts does, so a picker that is
    /// already open can tell it has to rebuild to show a font just added.
    pub fn revision(&self) -> u64 {
        self.dir_sig
    }
}

fn is_font_file(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".ttf") || lower.ends_with(".otf")
}

/// The fonts under `fonts/`, as `(project-relative path, mtime)`, sorted.
///
/// In the editor this reads `ProjectIndex`, which was walked once on a thread
/// and is kept current from the watcher's events, so a font added anywhere
/// under `fonts/` (they usually come as a family folder,
/// `fonts/barlow/Barlow-Bold.ttf`) is found without this system touching the
/// disk. Main-thread folder walks are what the index exists to end; see
/// `renzora_platform_api::project_index`.
///
/// Without an index (a game: the watcher is editor-only) it lists the top
/// level of `fonts/` once, as it always has, so a font name saved by an
/// earlier version still resolves. Nothing in a game needs more: settings
/// store a project font by its path, which loads without being listed.
fn project_font_files(
    index: Option<&renzora_platform_api::project_index::ProjectIndex>,
    dir: Option<&std::path::Path>,
) -> Vec<(String, u64)> {
    let mut found: Vec<(String, u64)> = match index {
        Some(index) => index
            .files()
            .filter(|f| f.path.starts_with("fonts/") && is_font_file(&f.path))
            .map(|f| (f.path.clone(), f.modified))
            .collect(),
        None => dir
            .and_then(|d| std::fs::read_dir(d).ok())
            .map(|rd| {
                rd.flatten()
                    .filter_map(|e| {
                        let name = e.file_name().to_str()?.to_string();
                        is_font_file(&name).then(|| (format!("fonts/{name}"), 0))
                    })
                    .collect()
            })
            .unwrap_or_default(),
    };
    found.sort();
    found
}

/// A cheap hash of the font set (paths + mtimes), so the registry is rebuilt
/// only when it actually changed: rebuilding re-issues every handle.
fn font_dir_signature(files: &[(String, u64)]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    files.hash(&mut h);
    h.finish()
}

/// Keep [`FontRegistry`] in step with the project's fonts: every `.ttf`/`.otf`
/// under `fonts/`, at any depth, loaded into `Assets<Font>`, so a font added to
/// the project shows in every picker without a restart.
///
/// This used to re-`read_dir` the folder every thirty frames, roughly twice a
/// second, forever, and later on every watcher event under `fonts/`. In the
/// editor it now follows `ProjectIndex`'s revision and reads the index, which
/// costs a map scan and no disk access (see [`project_font_files`]).
pub(crate) fn scan_project_fonts(
    mut registry: ResMut<FontRegistry>,
    asset_server: Res<AssetServer>,
    ember: Option<Res<EmberFonts>>,
    project: Option<Res<renzora_project_api::CurrentProject>>,
    index: Option<Res<renzora_platform_api::project_index::ProjectIndex>>,
    mut seen_revision: Local<Option<u64>>,
) {
    let Some(ember) = ember else {
        return; // fonts not ready yet
    };
    let dir = project.as_ref().map(|p| p.path.join("fonts"));
    match index.as_deref() {
        Some(index) => {
            // Until the first walk lands the index is empty, not "no fonts":
            // the built-ins go up now so a picker has something to show.
            if !index.ready() {
                if registry.entries.is_empty() {
                    registry.entries = FontRegistry::builtin_entries(ember.default_ui.clone());
                }
                return;
            }
            if *seen_revision == Some(index.revision()) {
                return;
            }
            *seen_revision = Some(index.revision());
        }
        // A game lists its fonts once; nothing changes them under it.
        None if !registry.entries.is_empty() && registry.scanned_dir == dir => return,
        None => {}
    }

    // The index moves on every project file change; only a change to the
    // font set rebuilds the registry.
    let files = project_font_files(index.as_deref(), dir.as_deref());
    let sig = font_dir_signature(&files);
    if !registry.entries.is_empty() && registry.scanned_dir == dir && registry.dir_sig == sig {
        return; // nothing changed
    }

    // Rebuild the list: built-ins first, then project fonts, each LOADED
    // through the asset server by its project-relative path so the handle is
    // path-backed. That matters downstream: the path is what the scene `.bsn`
    // records for a `TextFont`, what the exporter's reference trace packs, and
    // what a shipped game loads from the rpak.
    let mut entries = FontRegistry::builtin_entries(ember.default_ui.clone());
    {
        // (stem, project-relative path, folder under fonts/)
        let found: Vec<(String, String, String)> = files
            .iter()
            .filter_map(|(rel, _)| {
                let file = rel.rsplit('/').next()?;
                let stem = file.rsplit_once('.').map(|(s, _)| s).unwrap_or(file).to_string();
                let inner = rel.strip_prefix("fonts/")?;
                let folder = inner.rsplit_once('/').map(|(f, _)| f.to_string()).unwrap_or_default();
                Some((stem, rel.clone(), folder))
            })
            .collect();
        for (stem, rel, folder) in &found {
            // Two families can both ship a `Regular.ttf`; the folder tells them
            // apart in a picker. A unique stem stays as it was, so a name saved
            // by an earlier version still resolves.
            let shared = found.iter().filter(|(s, ..)| s == stem).count() > 1;
            let name = if shared && !folder.is_empty() { format!("{stem} ({folder})") } else { stem.clone() };
            // Project-relative asset path (asset_server dedupes by path).
            let handle = asset_server.load::<Font>(rel.clone());
            entries.push(FontEntry {
                name,
                source: FontSource::Handle(handle),
                origin: FontOrigin::Project,
                path: Some(rel.clone()),
            });
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
                // Snapped like `ui_font` — icons were the single biggest source of
                // size spread (~24 distinct sizes on the Phosphor font alone), and
                // they share swash's 8-entry hinting table with the text fonts.
                // Note icons deliberately skip `TEXT_SCALE`/`ui_font_scale`, so the
                // authored size IS the final px here.
                font_size: bevy::text::FontSize::Px(snap_font_px(size)),
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

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_platform_api::project_index::{IndexedFile, ProjectIndex};

    fn file(path: &str) -> IndexedFile {
        IndexedFile { path: path.into(), size: 1, modified: 7 }
    }

    /// Fonts in family folders count, at any depth, and nothing else does:
    /// not a font outside `fonts/`, not a licence beside one.
    #[test]
    fn project_fonts_come_from_the_index_at_any_depth() {
        let root = std::path::PathBuf::from("/p");
        let mut index = ProjectIndex::default();
        index.begin(root.clone());
        index.complete(
            &root,
            vec![
                file("fonts/Top.ttf"),
                file("fonts/isometra/Isometra-Regular.TTF"),
                file("fonts/barlow/static/Barlow-Bold.otf"),
                file("fonts/barlow/OFL.txt"),
                file("ui/Stray.ttf"),
            ],
        );
        let found = project_font_files(Some(&index), None);
        let paths: Vec<&str> = found.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(
            paths,
            ["fonts/Top.ttf", "fonts/barlow/static/Barlow-Bold.otf", "fonts/isometra/Isometra-Regular.TTF"]
        );
        assert!(found.iter().all(|(_, m)| *m == 7), "the index's mtime is what the signature hashes");
    }
}
