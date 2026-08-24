//! Document tab bar — renders between title bar and dock tree.
//!
//! Each tab represents an open scene. The "+" button creates a new empty scene tab.
//! Workspace layouts are switched independently via the layout/workspace system.

use bevy::prelude::*;

// ── Constants (matching legacy) ──────────────────────────────────────────────

const TAB_HEIGHT: f32 = 28.0;
const TOP_MARGIN: f32 = 4.0;

/// Total height consumed by the document tab bar.
pub const DOC_TAB_BAR_HEIGHT: f32 = TAB_HEIGHT + TOP_MARGIN;

// ── Document tab ─────────────────────────────────────────────────────────────

/// What type of asset a document tab represents. The layout that should be
/// active when the tab is focused, and the icon used in its tab header, both
/// follow from this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DocTabKind {
    #[default]
    Scene,
    Material,
    Particle,
    Blueprint,
    Script,
    Shader,
    Other,
}

impl DocTabKind {
    /// True if this tab represents a single asset file (vs a scene). Asset
    /// tabs put the editor into Asset mode where panels load directly from
    /// the file path and entity selection is irrelevant.
    pub fn is_asset(self) -> bool {
        !matches!(self, DocTabKind::Scene | DocTabKind::Other)
    }

    /// Named workspace layout that should activate when this tab is focused
    /// in **Scene mode** (i.e. only meaningful for `Scene`). `None` means
    /// keep whatever layout is currently active.
    pub fn layout_name(self) -> Option<&'static str> {
        match self {
            DocTabKind::Scene => Some("Scene"),
            DocTabKind::Material => Some("Materials"),
            DocTabKind::Particle => Some("Particles"),
            DocTabKind::Blueprint => Some("Blueprints"),
            DocTabKind::Script => Some("Scripting"),
            DocTabKind::Shader => Some("Shaders"),
            DocTabKind::Other => None,
        }
    }

    /// Hidden workspace layout used for **Asset mode** — when a single asset
    /// file is open. These layouts drop the hierarchy/outline panels so the
    /// editor doesn't nag for an entity selection that doesn't apply.
    pub fn asset_layout_name(self) -> Option<&'static str> {
        match self {
            DocTabKind::Material => Some("Materials-Asset"),
            DocTabKind::Script | DocTabKind::Shader => Some("Scripting-Asset"),
            DocTabKind::Blueprint => Some("Blueprints-Asset"),
            DocTabKind::Particle => Some("Particles-Asset"),
            DocTabKind::Scene | DocTabKind::Other => None,
        }
    }

    /// Stable name used when persisting this kind into `project.toml`
    /// (`editor_open_tabs`). Paired with [`Self::from_persist_name`] — keep
    /// the two in sync when adding a kind.
    pub fn persist_name(self) -> &'static str {
        match self {
            DocTabKind::Scene => "scene",
            DocTabKind::Material => "material",
            DocTabKind::Particle => "particle",
            DocTabKind::Blueprint => "blueprint",
            DocTabKind::Script => "script",
            DocTabKind::Shader => "shader",
            DocTabKind::Other => "other",
        }
    }

    /// Inverse of [`Self::persist_name`]. Unknown names (a config written by
    /// a newer editor with extra kinds, or hand-edited) fall back to `Other`,
    /// which behaves as a plain scene-mode tab instead of guessing an asset
    /// editor that may not fit the file.
    pub fn from_persist_name(name: &str) -> Self {
        match name {
            // Empty covers configs where the `kind` key was omitted by hand.
            "scene" | "" => DocTabKind::Scene,
            "material" => DocTabKind::Material,
            "particle" => DocTabKind::Particle,
            "blueprint" => DocTabKind::Blueprint,
            "script" => DocTabKind::Script,
            "shader" => DocTabKind::Shader,
            _ => DocTabKind::Other,
        }
    }

    /// Phosphor icon *name* (kebab-case) for this tab kind. A name-based
    /// renderer (e.g. `renzora_ember::font::icon_glyph`) resolves it to a glyph.
    pub fn icon(self) -> &'static str {
        match self {
            DocTabKind::Scene => "film-slate",
            DocTabKind::Material => "palette",
            DocTabKind::Particle => "sparkle",
            DocTabKind::Blueprint => "blueprint",
            DocTabKind::Script => "code",
            DocTabKind::Shader => "graphics-card",
            DocTabKind::Other => "file",
        }
    }

    /// The type's accent color, as sRGB bytes. Deliberately the same values the
    /// asset browser gives these extensions in its tile accents and its "create
    /// new" menu, so a material is the same green wherever you meet it — in the
    /// grid, in the menu that made it, and in the tab you opened it into.
    ///
    /// Theme-independent on purpose: these are *type* identities, not chrome, so
    /// they must not shift when the palette does. The tab strip paints the icon
    /// with this in every state, active or not — the accent underline and the
    /// brighter label are what mark which tab is current, which leaves the icon
    /// free to say what kind of thing each tab holds.
    pub fn color(self) -> (u8, u8, u8) {
        match self {
            DocTabKind::Scene => (115, 191, 242),
            DocTabKind::Material => (0, 200, 130),
            DocTabKind::Particle => (230, 160, 90),
            DocTabKind::Blueprint => (100, 180, 255),
            DocTabKind::Script => (120, 170, 255),
            DocTabKind::Shader => (220, 120, 255),
            DocTabKind::Other => (150, 155, 170),
        }
    }
}

/// A single open document tab. Historically these were always scenes; they
/// now also cover other asset types (materials, particles, blueprints,
/// scripts, shaders) opened via double-click in the asset browser. The
/// `scene_path` field stores the file path regardless of kind — kept under
/// the legacy name so existing scene-tab call sites continue to compile.
#[derive(Debug, Clone)]
pub struct DocumentTab {
    /// Unique id for this tab instance.
    pub id: u64,
    /// Display name.
    pub name: String,
    /// Path to the file on disk, project-relative (None for unsaved tabs).
    pub scene_path: Option<String>,
    /// What kind of asset this tab represents.
    pub kind: DocTabKind,
    /// Whether the document has unsaved changes.
    pub is_modified: bool,
}

// ── Editor context ──────────────────────────────────────────────────────────

/// What the editor is currently focused on. Derived from the active document
/// tab — Scene mode means panels follow `EditorSelection`; Asset mode means
/// panels load directly from the asset file path and ignore entity selection.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub enum EditorContext {
    #[default]
    Scene,
    Asset {
        /// Project-relative path to the file being edited.
        path: String,
        /// What kind of asset this is.
        kind: DocTabKind,
    },
}

impl EditorContext {
    pub fn is_scene(&self) -> bool {
        matches!(self, Self::Scene)
    }

    pub fn is_asset(&self) -> bool {
        matches!(self, Self::Asset { .. })
    }

    pub fn asset_path(&self) -> Option<&str> {
        if let Self::Asset { path, .. } = self {
            Some(path.as_str())
        } else {
            None
        }
    }

    pub fn asset_kind(&self) -> Option<DocTabKind> {
        if let Self::Asset { kind, .. } = self {
            Some(*kind)
        } else {
            None
        }
    }

    /// Build a context from a document tab.
    pub fn from_tab(tab: &DocumentTab) -> Self {
        match (tab.kind, tab.scene_path.as_ref()) {
            (DocTabKind::Scene, _) | (DocTabKind::Other, _) => Self::Scene,
            (kind, Some(path)) => Self::Asset {
                path: path.clone(),
                kind,
            },
            // Asset-kind tab with no path (unsaved): treat as scene-mode so
            // panels don't try to load `None`.
            (_, None) => Self::Scene,
        }
    }
}

// ── State resource ───────────────────────────────────────────────────────────

/// Resource managing all open document tabs.
#[derive(Resource, Clone)]
pub struct DocumentTabState {
    /// All open tabs in display order.
    pub tabs: Vec<DocumentTab>,
    /// Index of the currently active tab.
    pub active_tab: usize,
    /// Auto-incrementing ID counter.
    next_id: u64,
    /// Most-recently-used stack of **scene** tab IDs. Top of stack is the
    /// scene to return to when leaving an asset tab via the title-bar layout
    /// switcher. IDs of closed tabs stay until pruned by `find_mru_scene_tab`.
    scene_mru: Vec<u64>,
}

impl Default for DocumentTabState {
    fn default() -> Self {
        let mut state = Self {
            tabs: Vec::new(),
            active_tab: 0,
            next_id: 1,
            scene_mru: Vec::new(),
        };
        // Start with one default scene tab
        state.add_tab("Untitled Scene".into(), None);
        // Seed MRU with the initial scene tab.
        if let Some(id) = state.tabs.first().map(|t| t.id) {
            state.scene_mru.push(id);
        }
        state
    }
}

impl DocumentTabState {
    /// Add a new scene tab and return its index.
    pub fn add_tab(&mut self, name: String, scene_path: Option<String>) -> usize {
        self.add_tab_of_kind(name, scene_path, DocTabKind::Scene)
    }

    /// Add a tab of the given kind and return its index.
    pub fn add_tab_of_kind(
        &mut self,
        name: String,
        path: Option<String>,
        kind: DocTabKind,
    ) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(DocumentTab {
            id,
            name,
            scene_path: path,
            kind,
            is_modified: false,
        });
        self.tabs.len() - 1
    }

    /// Find an existing tab for the given project-relative path and kind.
    pub fn find_by_path(&self, path: &str, kind: DocTabKind) -> Option<usize> {
        self.tabs
            .iter()
            .position(|t| t.kind == kind && t.scene_path.as_deref() == Some(path))
    }

    /// Close a tab by index. Returns the closed tab's id for buffer cleanup,
    /// or `None` if the close was denied (last tab overall, or last remaining
    /// scene tab — at least one scene must always be open).
    pub fn close_tab(&mut self, index: usize) -> Option<u64> {
        if index >= self.tabs.len() {
            return None;
        }
        // Don't close the last tab
        if self.tabs.len() <= 1 {
            return None;
        }
        // Don't close the last scene tab — Asset mode requires a scene to
        // return to when the user leaves Asset mode via the title bar.
        if self.tabs[index].kind == DocTabKind::Scene {
            let scene_count = self
                .tabs
                .iter()
                .filter(|t| t.kind == DocTabKind::Scene)
                .count();
            if scene_count <= 1 {
                return None;
            }
        }

        let closed_id = self.tabs[index].id;
        self.tabs.remove(index);
        // Drop the closed tab from the MRU.
        self.scene_mru.retain(|id| *id != closed_id);
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        } else if self.active_tab > index {
            self.active_tab -= 1;
        }
        Some(closed_id)
    }

    /// Activate a tab by index. Returns (old_id, new_id) if the tab changed.
    /// Side effect: if the activated tab is a scene, its id is pushed onto
    /// the MRU stack so the title-bar layout switcher knows where to return.
    pub fn activate_tab(&mut self, index: usize) -> Option<(u64, u64)> {
        if index < self.tabs.len() && index != self.active_tab {
            let old_id = self.tabs[self.active_tab].id;
            self.active_tab = index;
            let new_id = self.tabs[index].id;
            self.touch_scene_mru(index);
            Some((old_id, new_id))
        } else {
            None
        }
    }

    /// Push a scene tab to the top of the MRU stack. No-op for non-scene tabs.
    pub fn touch_scene_mru(&mut self, index: usize) {
        let Some(tab) = self.tabs.get(index) else {
            return;
        };
        if tab.kind != DocTabKind::Scene {
            return;
        }
        let id = tab.id;
        self.scene_mru.retain(|other| *other != id);
        self.scene_mru.push(id);
    }

    /// Find the most-recently-used scene tab still open. Walks the MRU
    /// top-down, pruning ids of closed tabs along the way. Falls back to the
    /// first scene tab in display order if MRU is empty.
    pub fn find_mru_scene_tab(&mut self) -> Option<usize> {
        while let Some(&id) = self.scene_mru.last() {
            if let Some(idx) = self
                .tabs
                .iter()
                .position(|t| t.id == id && t.kind == DocTabKind::Scene)
            {
                return Some(idx);
            }
            self.scene_mru.pop();
        }
        // Fallback: the first scene tab in display order.
        self.tabs.iter().position(|t| t.kind == DocTabKind::Scene)
    }

    /// Get the active tab as a reference.
    pub fn active_tab(&self) -> Option<&DocumentTab> {
        self.tabs.get(self.active_tab)
    }

    /// Reorder: move tab from `from` to `to` index.
    pub fn reorder(&mut self, from: usize, to: usize) {
        if from >= self.tabs.len() || to > self.tabs.len() || from == to {
            return;
        }
        let tab = self.tabs.remove(from);
        let insert_at = if to > from { to - 1 } else { to };
        let insert_at = insert_at.min(self.tabs.len());
        self.tabs.insert(insert_at, tab);

        // Update active_tab to follow the moved tab if it was active
        if self.active_tab == from {
            self.active_tab = insert_at;
        } else if from < self.active_tab && self.active_tab <= insert_at {
            self.active_tab -= 1;
        } else if insert_at <= self.active_tab && self.active_tab < from {
            self.active_tab += 1;
        }
    }

    /// Get the active tab's id.
    pub fn active_tab_id(&self) -> Option<u64> {
        self.tabs.get(self.active_tab).map(|t| t.id)
    }
}

// ── Actions ──────────────────────────────────────────────────────────────────

/// Actions returned from the document tab bar.
pub enum DocTabAction {
    None,
    /// Activate tab at index.
    Activate(usize),
    /// Close tab at index.
    Close(usize),
    /// Reorder tab from index to index.
    Reorder(usize, usize),
    /// Add a new scene tab.
    AddNew,
}

#[cfg(test)]
mod doc_tab_kind_tests {
    use super::*;

    /// Every kind, so adding one without extending the tables below fails here
    /// rather than silently in the editor.
    const ALL: &[DocTabKind] = &[
        DocTabKind::Scene,
        DocTabKind::Material,
        DocTabKind::Particle,
        DocTabKind::Blueprint,
        DocTabKind::Script,
        DocTabKind::Shader,
        DocTabKind::Other,
    ];

    /// These names go into `project.toml`. A change to one silently reopens the
    /// user's saved tabs as the wrong kind — a material tab comes back as a
    /// plain scene tab — so the round trip is the contract.
    #[test]
    fn every_kind_round_trips_through_its_persisted_name() {
        for kind in ALL {
            let name = kind.persist_name();
            assert_eq!(
                DocTabKind::from_persist_name(name),
                *kind,
                "{name:?} did not round-trip"
            );
        }
    }

    #[test]
    fn persisted_names_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for kind in ALL {
            assert!(
                seen.insert(kind.persist_name()),
                "duplicate persist name {:?}",
                kind.persist_name()
            );
        }
    }

    /// A config written by a newer editor carries kinds this build has never
    /// heard of. Falling back to `Other` opens them as plain scene-mode tabs
    /// instead of guessing an asset editor that may not fit the file.
    #[test]
    fn an_unknown_persisted_name_falls_back_to_other() {
        assert_eq!(DocTabKind::from_persist_name("hologram"), DocTabKind::Other);
        assert_eq!(DocTabKind::from_persist_name("SCENE"), DocTabKind::Other);
    }

    /// A hand-edited config can omit the key entirely; that has to mean the
    /// default rather than `Other`, or every such tab loses its scene layout.
    #[test]
    fn a_missing_persisted_name_reads_as_a_scene() {
        assert_eq!(DocTabKind::from_persist_name(""), DocTabKind::Scene);
        assert_eq!(DocTabKind::from_persist_name(""), DocTabKind::default());
    }

    /// Asset mode is what drops the hierarchy/outline panels. `Scene` and
    /// `Other` must stay out of it — `Other` in particular is the fallback for
    /// unknown kinds, and putting an unknown file into asset mode would hide the
    /// panels with nothing to replace them.
    #[test]
    fn only_real_asset_kinds_enter_asset_mode() {
        assert!(!DocTabKind::Scene.is_asset());
        assert!(!DocTabKind::Other.is_asset());
        for kind in [
            DocTabKind::Material,
            DocTabKind::Particle,
            DocTabKind::Blueprint,
            DocTabKind::Script,
            DocTabKind::Shader,
        ] {
            assert!(kind.is_asset(), "{kind:?} should be an asset kind");
        }
    }

    /// The two layout tables and `is_asset` have to agree: an asset kind that
    /// has no asset layout would enter asset mode and then find no layout to
    /// switch to, leaving whatever was on screen before.
    #[test]
    fn every_asset_kind_has_an_asset_layout() {
        for kind in ALL {
            assert_eq!(
                kind.asset_layout_name().is_some(),
                kind.is_asset(),
                "{kind:?}: is_asset={} but asset_layout_name={:?}",
                kind.is_asset(),
                kind.asset_layout_name()
            );
        }
    }

    /// `Other` deliberately has no scene layout — "keep whatever is active" is
    /// the right answer for a file the editor does not recognise.
    #[test]
    fn every_kind_but_other_names_a_scene_layout() {
        assert_eq!(DocTabKind::Other.layout_name(), None);
        for kind in ALL.iter().filter(|k| **k != DocTabKind::Other) {
            assert!(kind.layout_name().is_some(), "{kind:?} has no layout");
        }
    }

    /// Two kinds sharing a scene layout would make focusing one tab switch to
    /// another's workspace.
    #[test]
    fn scene_layout_names_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for kind in ALL {
            if let Some(name) = kind.layout_name() {
                assert!(seen.insert(name), "duplicate layout {name:?}");
            }
        }
    }

    #[test]
    fn every_kind_has_an_icon() {
        for kind in ALL {
            assert!(!kind.icon().is_empty(), "{kind:?} has no icon");
        }
    }

    // ── the editor-context projection ────────────────────────────────────────

    fn tab(kind: DocTabKind, path: Option<&str>) -> DocumentTab {
        DocumentTab {
            id: 1,
            name: "test".to_string(),
            scene_path: path.map(|p| p.to_string()),
            kind,
            is_modified: false,
        }
    }

    /// Scene and `Other` tabs stay in scene mode whatever their path — this is
    /// the switch that decides whether panels read from a file or from the
    /// selected entity.
    #[test]
    fn scene_and_unknown_tabs_project_to_scene_context() {
        for kind in [DocTabKind::Scene, DocTabKind::Other] {
            let ctx = EditorContext::from_tab(&tab(kind, Some("a/b.scene")));
            assert!(ctx.is_scene(), "{kind:?} should be scene context");
            assert!(!ctx.is_asset());
            assert_eq!(ctx.asset_path(), None);
        }
    }

    #[test]
    fn an_asset_tab_carries_its_path_and_kind_into_the_context() {
        let ctx = EditorContext::from_tab(&tab(DocTabKind::Material, Some("mats/stone.material")));
        assert!(ctx.is_asset());
        assert!(!ctx.is_scene());
        assert_eq!(ctx.asset_path(), Some("mats/stone.material"));
        assert_eq!(ctx.asset_kind(), Some(DocTabKind::Material));
    }

    /// An asset tab with no path yet — a freshly created, never-saved material —
    /// must not claim to be an asset context, because every panel in asset mode
    /// loads from that path.
    #[test]
    fn an_asset_tab_without_a_path_is_not_an_asset_context() {
        let ctx = EditorContext::from_tab(&tab(DocTabKind::Material, None));
        assert!(ctx.is_scene(), "a pathless asset tab must fall back to scene mode");
        assert_eq!(ctx.asset_path(), None);
    }
}
