//! The browser's shared vocabulary: the [`NativeAssets`] resource every system
//! reads, the marker components the widgets carry, the thumbnail-source enum,
//! the sort mode, the creatable asset kinds, and the favorites/recent files on
//! disk.
//!
//! Everything here is `pub(crate)` because the panel is split by *region* (grid,
//! tree, menus, drag) and all of those regions act on one piece of state.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use bevy::prelude::*;

use renzora_editor_framework::{
    MaterialThumbnailRegistry, ModelThumbnailRegistry, SceneThumbnailRegistry,
};
use renzora_ember::reactive::Rx;

use crate::grid::Entry;
use crate::thumbnails::{
    supports_material_thumbnail, supports_model_thumbnail, supports_scene_thumbnail,
    supports_thumbnail, ThumbnailCache,
};

/// Which thumbnail source a file uses.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ThumbKind {
    Image,
    Model,
    Material,
    Scene,
}

pub(crate) fn thumb_kind(name: &str) -> Option<ThumbKind> {
    if supports_thumbnail(name) {
        Some(ThumbKind::Image)
    } else if supports_model_thumbnail(name) {
        Some(ThumbKind::Model)
    } else if supports_material_thumbnail(name) {
        Some(ThumbKind::Material)
    } else if supports_scene_thumbnail(name) {
        Some(ThumbKind::Scene)
    } else {
        None
    }
}

/// The ready thumbnail handle for `path`, from whichever registry owns `kind`.
pub(crate) fn handle_for(w: &Rx, kind: ThumbKind, path: &PathBuf) -> Option<Handle<Image>> {
    match kind {
        ThumbKind::Image => w.get_resource::<ThumbnailCache>().and_then(|c| c.handle(path)),
        ThumbKind::Model => w.get_resource::<ModelThumbnailRegistry>().and_then(|r| r.handle(path)),
        ThumbKind::Material => w.get_resource::<MaterialThumbnailRegistry>().and_then(|r| r.handle(path)),
        ThumbKind::Scene => w.get_resource::<SceneThumbnailRegistry>().and_then(|r| r.handle(path)),
    }
}

pub(crate) const TILE_W: f32 = 96.0;

/// How the current folder's entries are ordered (folders always sort first).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SortMode {
    Name,
    Type,
    Size,
    Modified,
}

impl SortMode {
    pub(crate) const ALL: [SortMode; 4] =
        [SortMode::Name, SortMode::Type, SortMode::Size, SortMode::Modified];
    pub(crate) fn label(self) -> String {
        match self {
            SortMode::Name => renzora::lang::t("common.name"),
            SortMode::Type => renzora::lang::t("common.type"),
            SortMode::Size => renzora::lang::t("common.size"),
            SortMode::Modified => renzora::lang::t("assets.sort.modified"),
        }
    }
}

// Content-area surfaces (Unreal-style: flat, dark).

/// Lean native state for the browser (independent of the egui panel's state).
#[derive(Resource)]
pub(crate) struct NativeAssets {
    pub(crate) current: Option<PathBuf>,
    pub(crate) selected: Option<PathBuf>,
    pub(crate) search: String,
    /// Expanded folders in the left tree.
    pub(crate) expanded: HashSet<PathBuf>,
    /// The grid tile the cursor is currently over (for right-click targeting).
    pub(crate) hovered: Option<PathBuf>,
    /// Last tile click (path, time) for double-click detection.
    pub(crate) last_click: Option<(PathBuf, f64)>,
    /// Grid tile zoom (0.5–1.5).
    pub(crate) zoom: f32,
    /// Folder-tree pane width (px).
    pub(crate) tree_width: f32,
    /// Active divider drag: `(start cursor x, start tree width)`. Persists the
    /// drag even when the cursor leaves the thin splitter (bevy_ui drops
    /// `Pressed` off-element).
    pub(crate) divider_drag: Option<(f32, f32)>,
    /// Favorited folders (persisted to `<root>/.editor/favorites`).
    pub(crate) favorites: Vec<PathBuf>,
    /// Recently opened files (persisted to `<root>/.editor/recent`).
    pub(crate) recent: Vec<PathBuf>,
    /// Whether favorites/recent have been loaded from disk this session.
    pub(crate) loaded: bool,
    /// A pending tile press `(path, is_dir, origin)` — promoted to a drag once
    /// the cursor moves >5px (for drag-to-viewport).
    pub(crate) drag_press: Option<(PathBuf, bool, Vec2)>,
    /// A plain press on a tile that's already part of a multi-selection. We must
    /// NOT collapse the selection to it on press — that would drop the other
    /// selected items before a drag can carry them. Instead the collapse is
    /// deferred to mouse-release and only applied if no drag occurred (a click).
    pub(crate) pending_single_select: Option<PathBuf>,
    /// Cached directory listing for the current folder. `read_dir` + a
    /// `metadata()` syscall per file is far too expensive to run every frame, so
    /// `refresh_listing` rescans only when the folder/search/sort changes, after
    /// an edit (`listing_dirty`), or on a slow throttle to catch external
    /// changes. The grid snapshot and `visible_order` both read this — shared in
    /// an `Arc` so neither clones the `Entry` data per frame.
    pub(crate) listing: std::sync::Arc<Vec<Entry>>,
    /// Hash of the inputs the cached `listing` was built from (folder, search,
    /// sort, direction). A mismatch forces an immediate rescan.
    pub(crate) listing_sig: u64,
    /// Seconds since the last rescan — drives the slow external-change throttle.
    pub(crate) listing_timer: f32,
    /// Set by edits (create / rename / delete / move) to force a rescan next
    /// frame without waiting for the throttle.
    pub(crate) listing_dirty: bool,
    /// True while a tile is being dragged out (drives the cursor ghost).
    pub(crate) dragging: bool,
    /// Entry sort order + direction.
    pub(crate) sort: SortMode,
    pub(crate) sort_desc: bool,
    /// List view (rows) instead of the tile grid.
    pub(crate) list_view: bool,
    /// The panel is too narrow for a usable grid alongside the tree: collapse to
    /// a tree-only file browser (grid + splitter hidden, tree shows files). Set
    /// by `responsive_layout` from the panel's measured width.
    pub(crate) narrow: bool,
    /// The panel is wide enough for the grid but too tight for the toolbar's full
    /// action labels: Add / Import / New Folder / Sort collapse to icon-only (the
    /// label lives on in a hover tooltip) and the item count hides, so the
    /// breadcrumb keeps a readable share of the row. Also set by
    /// `responsive_layout`.
    pub(crate) compact: bool,
    /// Which list the narrow browser shows (Project | Recent | Favs tabs).
    pub(crate) tree_tab: TreeTab,
    /// Narrow-mode filename filter (the tree pane's own search box).
    pub(crate) tree_search: String,
    /// Multi-selection (marquee + ctrl/shift click). `selected` stays the
    /// primary/anchor (rename + context-menu target).
    pub(crate) selection: HashSet<PathBuf>,
    pub(crate) selection_anchor: Option<PathBuf>,
    /// Active marquee rubber-band, in window logical px: press origin + current.
    pub(crate) marquee_start: Option<Vec2>,
    pub(crate) marquee_current: Option<Vec2>,
    /// Selection captured when the marquee began (swept tiles add to it).
    pub(crate) pre_marquee: HashSet<PathBuf>,
    /// The current folder's entries in display order — for shift-range select.
    pub(crate) visible_order: Vec<PathBuf>,
    /// The asset being inline-renamed (its grid tile / list row shows a text
    /// field instead of the name label). `None` = no active rename.
    pub(crate) renaming: Option<PathBuf>,
    /// A pending name-click rename `(path, click time)`: set when the name of the
    /// already-sole-selected item is clicked, fired by `rename_arm_fire` after a
    /// short delay — unless a double-click opens the item first (which clears it).
    pub(crate) rename_arm: Option<(PathBuf, f64)>,
}

impl Default for NativeAssets {
    fn default() -> Self {
        Self {
            current: None,
            selected: None,
            search: String::new(),
            expanded: HashSet::new(),
            hovered: None,
            last_click: None,
            zoom: 1.0,
            tree_width: 180.0,
            divider_drag: None,
            favorites: Vec::new(),
            recent: Vec::new(),
            loaded: false,
            drag_press: None,
            pending_single_select: None,
            listing: std::sync::Arc::new(Vec::new()),
            listing_sig: 0,
            listing_timer: 0.0,
            listing_dirty: false,
            dragging: false,
            sort: SortMode::Name,
            sort_desc: false,
            list_view: false,
            narrow: false,
            compact: false,
            tree_tab: TreeTab::Folders,
            tree_search: String::new(),
            selection: HashSet::new(),
            selection_anchor: None,
            marquee_start: None,
            marquee_current: None,
            pre_marquee: HashSet::new(),
            visible_order: Vec::new(),
            renaming: None,
            rename_arm: None,
        }
    }
}

impl NativeAssets {
    /// Apply a single-tile click with modifiers: ctrl toggles, shift selects the
    /// range from the anchor (using the grid's `visible_order`), plain replaces.
    pub(crate) fn click_select(&mut self, path: &Path, ctrl: bool, shift: bool) {
        let order = self.visible_order.clone();
        self.click_select_in(path, ctrl, shift, &order);
    }

    /// Same as [`NativeAssets::click_select`] but with an explicit visible order
    /// for shift-range — lets the folder tree multi-select using the tree's
    /// flattened order while the grid uses its listing order. `selected` tracks
    /// the primary item for rename / context-menu targeting.
    pub(crate) fn click_select_in(
        &mut self,
        path: &Path,
        ctrl: bool,
        shift: bool,
        order: &[PathBuf],
    ) {
        let p = path.to_path_buf();
        if ctrl {
            if self.selection.contains(&p) {
                self.selection.remove(&p);
                self.selected = self.selection.iter().next().cloned();
            } else {
                self.selection.insert(p.clone());
                self.selected = Some(p.clone());
                self.selection_anchor = Some(p);
            }
        } else if shift && self.selection_anchor.is_some() {
            let anchor = self.selection_anchor.clone().unwrap();
            let ai = order.iter().position(|q| *q == anchor);
            let ci = order.iter().position(|q| *q == p);
            if let (Some(a), Some(c)) = (ai, ci) {
                let (s, e) = if a <= c { (a, c) } else { (c, a) };
                self.selection.clear();
                for q in &order[s..=e] {
                    self.selection.insert(q.clone());
                }
                self.selected = Some(p);
            } else {
                // Anchor/target not in view — fall back to a plain select.
                self.selection.clear();
                self.selection.insert(p.clone());
                self.selected = Some(p.clone());
                self.selection_anchor = Some(p);
            }
        } else {
            self.selection.clear();
            self.selection.insert(p.clone());
            self.selected = Some(p.clone());
            self.selection_anchor = Some(p);
        }
    }

    /// True if `path` is part of the current (multi-)selection.
    pub(crate) fn is_selected(&self, path: &Path) -> bool {
        self.selection.contains(path) || self.selected.as_deref() == Some(path)
    }
}

pub(crate) fn file_name_of(p: &Path) -> String {
    p.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string()
}

/// Order-independent hash of a set/list of paths — XOR-folded so a `HashSet`'s
/// iteration order doesn't change the result frame to frame — mixed with count.
pub(crate) fn hash_path_set<'a>(paths: impl IntoIterator<Item = &'a PathBuf>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut acc: u64 = 0;
    let mut count: u64 = 0;
    for p in paths {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        p.hash(&mut h);
        acc ^= h.finish();
        count += 1;
    }
    acc.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(count)
}

// ── Favorites / recent persistence (<root>/.editor/{favorites,recent}) ─────────

pub(crate) fn load_list(root: &Path, file: &str) -> Vec<PathBuf> {
    let path = root.join(".editor").join(file);
    std::fs::read_to_string(&path)
        .ok()
        .map(|c| {
            c.lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| root.join(l.trim()))
                .filter(|p| p.exists())
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn save_list(root: &Path, file: &str, list: &[PathBuf]) {
    let dir = root.join(".editor");
    let _ = std::fs::create_dir_all(&dir);
    let content: String = list
        .iter()
        .filter_map(|p| p.strip_prefix(root).ok().map(|r| r.to_string_lossy().replace('\\', "/")))
        .collect::<Vec<_>>()
        .join("\n");
    let _ = std::fs::write(dir.join(file), content);
}

/// Load favorites + recent from disk once the project is known.
pub(crate) fn load_persisted(
    mut state: ResMut<NativeAssets>,
    project: Option<Res<renzora::core::CurrentProject>>,
) {
    if state.loaded {
        return;
    }
    let Some(root) = project.map(|p| p.path.clone()) else {
        return;
    };
    state.favorites = load_list(&root, "favorites");
    state.recent = load_list(&root, "recent");
    state.loaded = true;
}

#[derive(Component)]
pub(crate) struct AssetRoot;
#[derive(Component)]
pub(crate) struct DragGhost;
/// The toolbar "Add" button — clicking it opens the new-asset menu.
#[derive(Component)]
pub(crate) struct AddMenuBtn;
/// Marks the tree strip's "+" key (an [`AddMenuBtn`] too). Its menu also carries
/// New Folder and Import, because the tree-only layout hides the toolbar that
/// would otherwise offer them.
#[derive(Component)]
pub(crate) struct TreeAddBtn;
#[derive(Component)]
pub(crate) struct Splitter;

#[derive(Component)]
pub(crate) struct AssetTile {
    pub(crate) path: PathBuf,
    pub(crate) is_dir: bool,
}
/// Marks the inline rename text field in a grid tile / list row, carrying the
/// asset path it renames. The keyed-list rebuild spawns it when `renaming` is set.
#[derive(Component)]
pub(crate) struct AssetRenameInput(pub(crate) PathBuf);
/// The clickable name label of a tile/row. Clicking it while its asset is already
/// the sole selection arms an inline rename (OS-explorer "slow second click").
/// Uses `FocusPolicy::Pass` so the click still reaches the tile beneath it
/// (selection / double-click-open keep working).
#[derive(Component)]
pub(crate) struct AssetNameLabel(pub(crate) PathBuf);
#[derive(Component)]
pub(crate) struct AssetBack;
#[derive(Component)]
pub(crate) struct AssetSearch;

/// Which list the tree pane shows. Folders / Recent / Favorites are tabs (in
/// both the wide sidebar and the narrow tree-only browser) so each list gets
/// the full pane height, replacing the old collapsible FAVORITES / RECENT
/// sections stacked above the folder tree.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum TreeTab {
    Folders,
    Recent,
    Favorites,
}

#[derive(Component)]
pub(crate) struct TreeTabBtn(pub(crate) TreeTab);
/// The narrow-mode search field. Separate marker (and state field) from the
/// toolbar's [`AssetSearch`]: both inputs coexist as entities, and a shared
/// field would make the hidden one fight the visible one every frame.
#[derive(Component)]
pub(crate) struct TreeSearch;

#[derive(Component)]
pub(crate) struct TreeToggle(pub(crate) PathBuf);
#[derive(Component)]
pub(crate) struct TreeNav(pub(crate) PathBuf);
#[derive(Component)]
pub(crate) struct NewAssetBtn(pub(crate) NewAsset);
#[derive(Component)]
pub(crate) struct ImportBtn;
#[derive(Component)]
pub(crate) struct CrumbNav(pub(crate) PathBuf);
#[derive(Component)]
pub(crate) struct ShortcutClick {
    pub(crate) path: PathBuf,
    pub(crate) is_dir: bool,
}

#[derive(Component)]
pub(crate) struct SortMenuBtn;
#[derive(Component)]
pub(crate) struct ViewToggleBtn;
#[derive(Component)]
pub(crate) struct AssetGrid;
/// The grid viewport — a marquee starts on an empty press here.
#[derive(Component)]
pub(crate) struct GridArea;
/// The rubber-band selection rectangle overlay (top-level, unclipped).
#[derive(Component)]
pub(crate) struct MarqueeRect;

/// What the New-Folder / Add menu creates.
#[derive(Clone, Copy)]
pub(crate) enum NewAsset {
    Folder,
    Material,
    Blueprint,
    Lua,
    Particle,
    Template,
    Bsn,
}

impl NewAsset {
    /// The creatable file types offered by the Add button + right-click menu,
    /// in display order. `Folder` is excluded — it has its own toolbar button.
    pub(crate) const MENU: [NewAsset; 6] = [
        NewAsset::Material,
        NewAsset::Blueprint,
        NewAsset::Lua,
        NewAsset::Particle,
        NewAsset::Template,
        NewAsset::Bsn,
    ];

    pub(crate) fn filename(self) -> &'static str {
        match self {
            NewAsset::Folder => "New Folder",
            NewAsset::Material => "NewMaterial.material",
            NewAsset::Blueprint => "NewBlueprint.blueprint",
            NewAsset::Lua => "new_script.lua",
            NewAsset::Particle => "NewParticle.particle",
            NewAsset::Template => "NewTemplate.html",
            NewAsset::Bsn => "NewScene.bsn",
        }
    }
    /// `boilerplate` comes from `EditorSettings::new_file_boilerplate`; the
    /// starters themselves are owned by the crates that consume the formats, so
    /// a file made here is byte-identical to one made from the hierarchy's
    /// Attach menu.
    pub(crate) fn content(self, boilerplate: bool) -> String {
        match self {
            NewAsset::Folder => String::new(),
            NewAsset::Material => "{}".to_string(),
            // Not `{}`: nothing in a blueprint runs unless it hangs off an
            // event, so a new file starts with On Ready + On Update placed —
            // see `renzora_blueprint::starter`.
            NewAsset::Blueprint => renzora_blueprint::starter_blueprint_json(),
            NewAsset::Lua => renzora_scripting::starter_lua(boilerplate),
            NewAsset::Particle => "(name: \"New Particle\")".to_string(),
            NewAsset::Template => renzora_ember::markup::starter_template(boilerplate),
            // An empty scene = just the interim-BSN header the parser expects.
            NewAsset::Bsn => "// renzora interim bsn v1\n".to_string(),
        }
    }
    /// Menu label.
    pub(crate) fn label(self) -> String {
        match self {
            NewAsset::Folder => renzora::lang::t("assets.new.folder"),
            NewAsset::Material => renzora::lang::t("assets.new.material"),
            NewAsset::Blueprint => renzora::lang::t("assets.new.blueprint"),
            NewAsset::Lua => renzora::lang::t("assets.new.lua"),
            NewAsset::Particle => renzora::lang::t("assets.new.particle"),
            NewAsset::Template => renzora::lang::t("assets.new.template"),
            NewAsset::Bsn => renzora::lang::t("assets.new.bsn"),
        }
    }
    /// Small uppercase type subtitle shown under the title on the menu card
    /// (Unreal's "STATIC MESH" / "BASIC SHAPE" second line).
    pub(crate) fn subtitle(self) -> String {
        match self {
            NewAsset::Folder => renzora::lang::t("assets.new.folder"),
            NewAsset::Material => renzora::lang::t("assets.new.material_sub"),
            NewAsset::Blueprint => renzora::lang::t("assets.new.blueprint_sub"),
            NewAsset::Lua => renzora::lang::t("assets.new.lua"),
            NewAsset::Particle => renzora::lang::t("assets.new.particle_sub"),
            NewAsset::Template => renzora::lang::t("assets.new.template_sub"),
            NewAsset::Bsn => renzora::lang::t("assets.new.scene_sub"),
        }
    }
    /// Phosphor icon — mirrors each type's editor opener in `open_action`.
    pub(crate) fn icon(self) -> &'static str {
        match self {
            NewAsset::Folder => "folder-plus",
            NewAsset::Material => "palette",
            NewAsset::Blueprint => "blueprint",
            NewAsset::Lua | NewAsset::Template => "code",
            NewAsset::Particle => "sparkle",
            NewAsset::Bsn => "film-slate",
        }
    }
    /// Accent color — matches the tile's type accent in
    /// [`crate::ops::asset_type_info`] so the menu, the tile strip and
    /// the subtitle all read as one color language.
    pub(crate) fn color(self) -> (u8, u8, u8) {
        match self {
            NewAsset::Folder => (235, 200, 120),
            NewAsset::Material => (0, 200, 130),
            NewAsset::Blueprint => (100, 180, 255),
            NewAsset::Lua => (120, 170, 255),
            NewAsset::Particle => (230, 160, 90),
            NewAsset::Template => (230, 120, 90),
            NewAsset::Bsn => (115, 191, 242),
        }
    }
    pub(crate) fn is_folder(self) -> bool {
        matches!(self, NewAsset::Folder)
    }
}

/// A free path in `folder` for `filename`, suffixing " 2", " 3"… on collision.
pub(crate) fn unique_path(folder: &Path, filename: &str, is_folder: bool) -> PathBuf {
    let candidate = folder.join(filename);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, ext) = if is_folder {
        (filename.to_string(), String::new())
    } else {
        let p = Path::new(filename);
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or(filename).to_string();
        let ext = p.extension().and_then(|e| e.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
        (stem, ext)
    };
    for n in 2..1000 {
        let cand = folder.join(format!("{stem} {n}{ext}"));
        if !cand.exists() {
            return cand;
        }
    }
    candidate
}
