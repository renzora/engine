//! Editor workspace layouts (which panels go where, per ribbon workspace).
//!
//! The dock **model** (`DockTree`, mutations, `DropZone`) now lives in
//! [`renzora_ember::dock`] — it's the reusable, UI-framework half. This module
//! is the editor-specific part: it builds concrete `DockTree`s for the editor's
//! workspaces using that model. Re-exported here so the rest of the shell keeps
//! importing `dock::DockTree` etc. unchanged.

pub use renzora_ember::dock::{DockTree, DropZone, SplitDirection};

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

// ── Collapsible bottom panel ───────────────────────────────────────────────────

/// One workspace's stashed bottom region while its bottom panel is closed: the
/// detached subtree plus the split ratio (top share) that restores its height
/// on reopen. The panels inside a closed bottom panel exist *only* here — not in
/// the workspace's tree — so this must persist with the layout or a
/// save-while-closed would drop them.
#[derive(Clone, Serialize, Deserialize)]
pub struct ClosedBottom {
    pub tree: DockTree,
    pub ratio: f32,
    /// Panel ids of the region the strip sat under when it was detached — the
    /// anchor that lets a non-full-width bottom reopen in its old place rather
    /// than spanning the whole root. Empty for a full-width stash (reopens
    /// full-width) and for legacy/pre-anchor stashes (`#[serde(default)]`).
    #[serde(default)]
    pub anchor: Vec<String>,
}

/// Fallback reopen ratio when a stash has no recorded one (legacy strips that
/// never lived below a vertical divider). The value the default scene layout
/// used back when it still carried a strip of its own.
pub const BOTTOM_PANEL_RATIO: f32 = 0.72;

/// Opening height of the global bottom panel, logical px. Roughly the old
/// 0.72 split on a 1080p window, so an upgraded layout looks unchanged.
pub const BOTTOM_DOCK_HEIGHT: f32 = 280.0;

/// Smallest height the bottom panel can be dragged to before it reads as a
/// tab strip with no room for content. Below this the drag snaps it closed.
pub const BOTTOM_DOCK_MIN_HEIGHT: f32 = 80.0;

/// How much of the dock region [`BottomDockMode::Layout`] has to leave for the
/// workspace above it. In layout mode the panel takes its height *off* the
/// workspace, so a panel taller than this would squeeze every panel above it
/// into a row of tab bars — which is why growing past it hands the panel to
/// [`BottomDockMode::Overlay`] instead of clamping the drag (see
/// [`BottomDockMode::effective`]).
pub const BOTTOM_DOCK_MIN_WORKSPACE: f32 = 120.0;

/// The tallest the panel can be while still docking into the workspace, given
/// `avail` logical px of dock region. Floored at the panel's own minimum so a
/// window too short for both never inverts the range.
pub fn max_layout_height(avail: f32) -> f32 {
    (avail - BOTTOM_DOCK_MIN_WORKSPACE).max(BOTTOM_DOCK_MIN_HEIGHT)
}

/// Fit a bottom-panel height into `avail` logical px of dock region: no shorter
/// than [`BOTTOM_DOCK_MIN_HEIGHT`], no taller than the region itself. Shared by
/// the live drag and the node sync so the height that is persisted and the
/// height that is drawn can't disagree.
pub fn clamp_height(height: f32, avail: f32) -> f32 {
    height.clamp(BOTTOM_DOCK_MIN_HEIGHT, avail.max(BOTTOM_DOCK_MIN_HEIGHT))
}

/// The tabs the one global bottom panel ships with, in tab order.
///
/// These are the panels that are useful in *every* workspace rather than in one
/// of them — a browser for the project's files, the animation timeline, the log,
/// the audio mixer and the shape library. None of them appear in a workspace
/// tree any more (see [`scene_layout`]): the panel is global, so a second copy
/// docked inside a workspace would be an independent instance of the same panel
/// sitting a few pixels above the real one.
pub const DEFAULT_BOTTOM_TABS: [&str; 5] =
    ["assets", "timeline", "console", "mixer", "shape_library"];

/// The default global bottom panel: one leaf tabbing [`DEFAULT_BOTTOM_TABS`].
pub fn default_bottom_tree() -> DockTree {
    DockTree::tabs(&DEFAULT_BOTTOM_TABS)
}

/// The global bottom panel a fresh install starts with — the state
/// "Reset Global Docks" restores, and what is used when there is no
/// `layout.json` at all.
///
/// Distinct from [`migrate_bottom_dock`], which is for the other empty case: a
/// layout file written *before* the panel was global, whose contents have to be
/// recovered out of the workspace trees rather than replaced by a default.
///
/// `sets` is left empty on purpose, matching the migration: the shell wraps a
/// set-less layout in its one default set on load, and naming the set here as
/// well would mean two spellings of the default name.
pub fn default_bottom_dock() -> BottomDockLayout {
    BottomDockLayout {
        tree: default_bottom_tree(),
        height: BOTTOM_DOCK_HEIGHT,
        // Closed at first launch, as every previous version was: the panel
        // covers a quarter of the viewport, and the collapsed strip below the
        // dock area advertises what is in it.
        open: false,
        mode: BottomDockMode::Overlay,
        sets: Vec::new(),
        active: 0,
    }
}

/// Fold every workspace's bottom strip — and every persisted closed-bottom
/// stash — into the one global bottom panel, mutating the workspace trees to
/// remove what was taken.
///
/// Runs once, when a layout file has no `bottom_dock` (or when there is no
/// layout file at all). It has to be non-lossy in both directions: a panel in
/// a *closed* stash exists nowhere else, and a panel in an *open* strip is
/// about to have its region cut out of the workspace tree. Both sets are
/// merged, deduplicated by panel id, into a single leaf.
///
/// Deduplicating is right even though duplicate panels are otherwise allowed:
/// every workspace's strip holds much the same thing (console, assets), so
/// preserving them verbatim would produce one bottom panel with five `console`
/// tabs. Duplicates the user *creates* later are untouched — this is a
/// one-time fold of layouts that were authored under the old per-workspace
/// model.
pub fn migrate_bottom_dock(
    workspaces: &mut [(String, DockTree)],
    closed: &BTreeMap<String, ClosedBottom>,
) -> BottomDockLayout {
    let mut tabs: Vec<String> = Vec::new();
    let mut push = |tree: &DockTree| {
        let mut ids = Vec::new();
        tree.collect_panels(&mut ids);
        for id in ids {
            if !tabs.contains(&id) {
                tabs.push(id);
            }
        }
    };

    // Closed stashes first: those panels are homeless, so they lead the tab
    // order rather than trailing whatever happened to still be open.
    for stash in closed.values() {
        push(&stash.tree);
    }
    for (_, tree) in workspaces.iter_mut() {
        if let Some(stash) = take_bottom_strip(tree) {
            push(&stash.tree);
        }
    }

    BottomDockLayout {
        tree: if tabs.is_empty() {
            DockTree::Empty
        } else {
            DockTree::Leaf {
                tabs,
                active_tab: 0,
            }
        },
        height: BOTTOM_DOCK_HEIGHT,
        // Matches the old startup behaviour: every launch began with the
        // bottom strip stashed, and Ctrl+Space brought it back.
        open: false,
        mode: BottomDockMode::Overlay,
        // Left empty rather than synthesized here: the shell wraps a set-less
        // layout in its one default set on load, and doing it in both places
        // would mean two spellings of the default name.
        sets: Vec::new(),
        active: 0,
    }
}

/// Does `tree`'s root have a bottom region holding the classic bottom-strip
/// panels (assets/console)? Startup collapses only strips like these; a
/// workspace whose bottom region is something else (Animation's timeline)
/// keeps it open until the user toggles it themselves.
pub fn has_bottom_strip(tree: &DockTree) -> bool {
    matches!(
        tree,
        DockTree::Split { direction: SplitDirection::Vertical, second, .. }
            if second.contains_panel("assets") || second.contains_panel("console")
    )
}

/// Panel ids that used to exist and no longer do.
///
/// A saved `layout.json` outlives the build that wrote it, so a removed panel
/// stays in it forever — the dock renders an id it has no builder for as a
/// placeholder pane rather than failing, which makes the ghost harmless but
/// permanent. Stripping them on load is the only thing that actually clears
/// them. Append here whenever a panel is retired; entries can be removed again
/// once no plausible saved layout still mentions them.
const RETIRED_PANELS: &[&str] = &[
    // Scripts-on-entity, the code outline and script variables. The code editor
    // has its own tab strip and toolbar, and the three panels between them cost
    // a whole column of the Scripting workspace for what they showed.
    "scripts_on_entity",
    "outline",
    "script_variables",
    // The cinematics sequencer: in-editor playback worked, but sequences never
    // persisted and bake-to-video was a stub. Its job — keying a camera and
    // other entities against one playhead, with markers — is what the animation
    // Timeline already does, against clips that do save.
    "sequencer",
];

/// Is `tree`'s leaf holding `console` the classic bottom strip? The strip is
/// recognized by console being tabbed together with another strip panel —
/// mixer/timeline/shape_library, or assets/hub_store in layouts saved before
/// those moved out of the strip. Requiring a companion keeps this from matching
/// a standalone console leaf (Blueprints) or the console+problems pair
/// (Scripting) — those stay open at launch, as before.
///
/// Only saved layouts reach this now. No shipped default carries a strip any
/// more (the panel is global), so this is purely about reading files written by
/// builds that predate that.
fn is_strip_leaf(tree: &mut DockTree) -> bool {
    const COMPANIONS: [&str; 6] = [
        "mixer",
        "timeline",
        "shape_library",
        "record",
        "assets",
        "hub_store",
    ];
    matches!(
        tree.find_leaf_mut("console"),
        Some(DockTree::Leaf { tabs, .. }) if tabs.iter().any(|t| COMPANIONS.contains(&t.as_str()))
    )
}

/// Startup stash: detach the classic console strip wherever it sits so every
/// launch begins with the bottom panel closed.
///
/// - A **root** full-width bottom region (layouts saved before the strip moved
///   under the viewport) detaches with no anchor — it reopens full-width, the
///   shape the user had.
/// - The **nested** strip (the shipped Scene default: the leaf tabbing
///   `console` with the other strip panels, under one column — see
///   [`is_strip_leaf`]) detaches with an anchor, so it reopens under that same
///   column instead of full-width.
/// - A strip not below any vertical divider at all (pre-bottom-region legacy
///   layouts) falls back to taking the leaf, reopening at the default ratio.
pub fn take_bottom_strip(tree: &mut DockTree) -> Option<ClosedBottom> {
    if has_bottom_strip(tree) {
        return tree.detach_bottom().map(|(bottom, ratio)| ClosedBottom {
            tree: bottom,
            ratio,
            anchor: Vec::new(),
        });
    }
    if !is_strip_leaf(tree) {
        return None;
    }
    if let Some((bottom, ratio, anchor)) = tree.detach_bottom_containing("console") {
        return Some(ClosedBottom {
            tree: bottom,
            ratio,
            anchor,
        });
    }
    take_legacy_bottom_strip(tree)
}

/// Detach the classic strip from a layout saved before the strip became a root
/// region (it sat nested under the viewport): the leaf tabbing `console` with
/// another strip panel (see [`is_strip_leaf`] for why a companion is
/// required). Reopens at the default ratio — the nested split's ratio meant
/// something else.
pub fn take_legacy_bottom_strip(tree: &mut DockTree) -> Option<ClosedBottom> {
    is_strip_leaf(tree)
        .then(|| tree.take_leaf_containing("console"))
        .flatten()
        .map(|t| ClosedBottom {
            tree: t,
            ratio: BOTTOM_PANEL_RATIO,
            anchor: Vec::new(),
        })
}

// ── Persistence ────────────────────────────────────────────────────────────────
//
// Dock positions (split ratios, which panels sit where, active tabs) persist
// across sessions in a per-user file, mirroring the `~/.renzora/*.toml`
// convention used for the renderer/UI-scale preferences (see `renzora::core`).
// JSON, not TOML: the layout is a recursive tagged enum tree, which TOML renders
// as an unreadable pile of nested tables — JSON round-trips it cleanly. The set
// of workspaces is machine-local user state, not project state, so it lives next
// to the other per-user prefs rather than in `project.toml`.

/// One persisted workspace: its ribbon name + its dock tree.
#[derive(Serialize, Deserialize)]
struct PersistedWorkspace {
    name: String,
    tree: DockTree,
}

/// One floating dock window's persisted state: its tree + last client-area
/// geometry (physical px), so tear-off windows come back on the same monitor
/// at the same size after a restart.
#[derive(Clone, Serialize, Deserialize)]
pub struct FloatingLayout {
    pub tree: DockTree,
    /// Client-area origin in physical screen px (`None` = let the OS place it).
    pub position: Option<(i32, i32)>,
    /// Client-area size in physical px.
    pub size: (u32, u32),
}

/// How the global bottom panel occupies the bottom of the dock region.
///
/// Both modes put the panel in the same place and give it the same height —
/// the difference is only whether the workspace above knows it is there.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BottomDockMode {
    /// Absolutely positioned over the dock area, covering whatever panels sit
    /// beneath it. Growing the panel never touches the workspace's own split
    /// ratios — the property that made the bottom panel global in the first
    /// place, so it stays the default.
    #[default]
    Overlay,
    /// In-flow at the bottom of the dock column, so the workspace above is
    /// given the remaining height. Resizing the panel therefore reflows every
    /// panel above it instead of hiding their lower edge.
    Layout,
}

impl BottomDockMode {
    /// The mode the panel *renders* in at `height`, given `avail` logical px of
    /// dock region.
    ///
    /// The panel can be dragged the whole way up to the top bar, and past
    /// [`max_layout_height`] there is no longer a workspace for `Layout` to
    /// reflow into. Rather than stopping the drag short, the panel takes over
    /// as an overlay — which is the only reading of "the panel is nearly the
    /// whole window" that leaves the panels above intact instead of crushed to
    /// their tab bars. Drag back down and `Layout` resumes, because this is a
    /// function of the height rather than a mode change that was written down.
    pub fn effective(self, height: f32, avail: f32) -> Self {
        if self == Self::Layout && height > max_layout_height(avail) {
            Self::Overlay
        } else {
            self
        }
    }
}

/// The global bottom panel's persisted state — one per editor, not one per
/// workspace, which is the whole point of it (see [`renzora_ember::dock::FixedDock`]).
///
/// `height` is logical px, not a ratio: the panel is sized in absolute terms in
/// both modes, so there is no sibling to be a fraction of, and a ratio would
/// silently rescale the panel when the window resizes.
#[derive(Clone, Serialize, Deserialize)]
pub struct BottomDockLayout {
    /// The *active* set's tree, duplicated out of [`Self::sets`].
    ///
    /// Written even though `sets` already holds it, because it is the only
    /// field a build that predates panel sets knows how to read: downgrading
    /// then finds its bottom panel as the user left it instead of an empty one.
    pub tree: DockTree,
    pub height: f32,
    pub open: bool,
    /// `#[serde(default)]` so a layout file written before the mode toggle
    /// existed loads as `Overlay` — the behaviour it was saved with.
    #[serde(default)]
    pub mode: BottomDockMode,
    /// The panel's named tab-sets, active one included. Empty in a layout file
    /// written before they existed, which the shell reads as "one set holding
    /// `tree`" — so there is exactly one place a set can come from and no way
    /// for the two fields to disagree about the live one.
    #[serde(default)]
    pub sets: Vec<BottomPanelSet>,
    /// Index into [`Self::sets`]. Clamped on load, so a hand-edited or
    /// truncated file can't point past the end.
    #[serde(default)]
    pub active: usize,
}

/// One named tab-set of the global bottom panel — a "panel workspace".
///
/// The panel is global (one per editor, shared by every dock workspace), so
/// keeping several sets is how a project ends up with, say, a debugging set
/// (console + profiler) and an authoring one (assets + mixer) without either
/// having to be rebuilt tab by tab.
#[derive(Clone, Serialize, Deserialize)]
pub struct BottomPanelSet {
    pub name: String,
    pub tree: DockTree,
}

/// The on-disk dock layout file: every workspace plus the active index, plus
/// any floating dock windows (`#[serde(default)]` keeps pre-floating layout
/// files loading).
#[derive(Serialize, Deserialize)]
struct PersistedLayout {
    active: usize,
    workspaces: Vec<PersistedWorkspace>,
    #[serde(default)]
    floating: Vec<FloatingLayout>,
    /// Bottom-panel stashes for workspaces whose bottom panel is closed, keyed
    /// by workspace name (see [`ClosedBottom`] for why these must persist).
    ///
    /// Retained only so a layout file written before the global bottom panel
    /// can still be read and migrated — the panels inside a closed stash exist
    /// nowhere else, so dropping the field would delete them. Nothing writes it
    /// any more.
    #[serde(default)]
    closed_bottoms: BTreeMap<String, ClosedBottom>,
    /// `None` in a layout file that predates the global bottom panel, which is
    /// the signal to migrate: fold every workspace's bottom strip (and every
    /// `closed_bottoms` stash) into one shared tree.
    #[serde(default)]
    bottom_dock: Option<BottomDockLayout>,
}

/// Path to the persisted dock layout: `~/.renzora/layout.json`. Resolves the
/// home dir via env vars (matching `renzora::core`'s pref paths) so this stays
/// dependency-light. `None` on wasm / when no home dir is resolvable.
#[cfg(not(target_arch = "wasm32"))]
fn layout_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)?;
    Some(home.join(".renzora").join("layout.json"))
}

/// Load the persisted workspaces + active index + floating windows + closed
/// bottom-panel stashes, or `None` when the file is absent / unreadable /
/// malformed (callers then fall back to the built-in [`workspace_layouts`]).
#[allow(clippy::type_complexity)]
pub fn load_dock_layouts() -> Option<(
    Vec<(String, DockTree)>,
    usize,
    Vec<FloatingLayout>,
    BTreeMap<String, ClosedBottom>,
    Option<BottomDockLayout>,
)> {
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let text = std::fs::read_to_string(layout_path()?).ok()?;
        let data: PersistedLayout = serde_json::from_str(&text).ok()?;
        if data.workspaces.is_empty() {
            return None;
        }
        let mut workspaces = data
            .workspaces
            .into_iter()
            .map(|w| (w.name, w.tree))
            .collect::<Vec<_>>();
        for (_, tree) in &mut workspaces {
            tree.retire_panels(RETIRED_PANELS);
        }
        let active = data.active.min(workspaces.len() - 1);
        let mut bottom_dock = data.bottom_dock;
        if let Some(b) = bottom_dock.as_mut() {
            b.tree.retire_panels(RETIRED_PANELS);
            // Every set, not just the live one: a retired panel sitting in a
            // set the user hasn't switched to yet would come back the moment
            // they did.
            for set in &mut b.sets {
                set.tree.retire_panels(RETIRED_PANELS);
            }
            b.active = b.active.min(b.sets.len().saturating_sub(1));
        }
        Some((
            workspaces,
            active,
            data.floating,
            data.closed_bottoms,
            bottom_dock,
        ))
    }
}

/// Serialize the workspaces + active index + floating windows to the JSON we'd
/// persist. Returns the string so the caller can skip a redundant disk write
/// when nothing changed (the save system compares it against the last-written
/// snapshot).
pub fn layout_json(
    workspaces: &[(String, DockTree)],
    active: usize,
    floating: &[FloatingLayout],
    bottom_dock: &BottomDockLayout,
) -> Option<String> {
    let data = PersistedLayout {
        active,
        workspaces: workspaces
            .iter()
            .map(|(name, tree)| PersistedWorkspace {
                name: name.clone(),
                tree: tree.clone(),
            })
            .collect(),
        floating: floating.to_vec(),
        // Always empty now: the per-workspace stashes were folded into
        // `bottom_dock` on load and nothing recreates them. Writing the field
        // (rather than skipping it) keeps a downgrade to an older build from
        // tripping over a missing key.
        closed_bottoms: BTreeMap::new(),
        bottom_dock: Some(bottom_dock.clone()),
    };
    serde_json::to_string_pretty(&data).ok()
}

/// Write a pre-serialized layout JSON (from [`layout_json`]) to disk, creating
/// `~/.renzora/` if needed. No-op `Ok` on wasm.
#[allow(unused_variables)]
pub fn write_layout(json: &str) -> std::io::Result<()> {
    #[cfg(target_arch = "wasm32")]
    {
        Ok(())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = layout_path().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "could not resolve home directory for dock layout",
            )
        })?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, json)
    }
}

/// The ribbon workspace layouts, in ribbon order (Scene … Debug). Ports
/// `renzora_ui::layouts` (the visible, non-asset layouts) into the shell's
/// egui-free dock model.
pub fn workspace_layouts() -> Vec<(String, DockTree)> {
    vec![
        ("Scene".into(), scene_layout()),
        ("Scripting".into(), layout_scripting()),
        ("Blueprints".into(), layout_blueprints()),
        ("Animation".into(), layout_animation()),
        ("Materials".into(), layout_materials()),
        ("Particles".into(), layout_particles()),
        ("UI".into(), layout_ui()),
        ("Debug".into(), layout_debug()),
        ("Marketplace".into(), layout_marketplace()),
    ]
}

/// UI: UI Hierarchy | UI Editor + Code | Inspector.
///
/// The same three columns as Scene, and the same loop — select in the tree,
/// drag on the canvas, set properties in the inspector (all ~24 of the `ui_*`
/// inspector entries are already registered).
///
/// The tree is `ui_hierarchy`, **not** the scene hierarchy. A UI is a `.html`
/// document whose nodes are rebuilt from the file on every load and which the
/// scene deliberately does not serialise; the scene tree lists entities you can
/// transform, parent and save. They answer different questions, and a mesh in
/// the list while you are laying out a menu is something you cannot do anything
/// UI-shaped to. Filtering the scene panel by workspace was the alternative and
/// is worse: the same panel showing different contents depending on where you
/// stand is state the user cannot see.
///
/// The canvas is the **`ui_canvas` panel**, not the viewport in a special mode.
/// It used to be the latter: mounted inside the viewport panel's slot 0 and
/// revealed by a `ViewportView::Ui` that hid the rendered scene underneath it.
/// Opening a UI therefore took the 3D view away, and gave it back only when you
/// next selected something 3D — one surface doing two jobs, with whichever you
/// were not doing hidden. Two panels means you can dock them side by side and
/// watch a HUD against the scene it sits over.
///
/// `code_editor` is tabbed with the canvas because a `.html` template has a
/// text form worth reaching; `scenes` is not tabbed beside the hierarchy the way
/// Scene does it, since the document here is a template rather than a scene.
fn layout_ui() -> DockTree {
    DockTree::horizontal(
        DockTree::leaf("ui_hierarchy"),
        DockTree::horizontal(
            DockTree::tabs(&["ui_canvas", "code_editor"]),
            DockTree::leaf("inspector"),
            0.76,
        ),
        0.15,
    )
}

/// Blueprints: NodeProperties | BlueprintGraph over Console.
///
/// Neither the hierarchy nor the inspector is here. Both are about the *scene* —
/// which entity is selected and what components it carries — and this workspace
/// is about a graph, whose selection is a node and whose editor is the Node
/// Properties panel beside it. They cost the graph two columns to show what
/// Scene already shows better.
///
/// `console` stays, docked under the graph rather than left to the global bottom
/// panel: a blueprint is debugged by its print output, and watching that scroll
/// beneath the nodes that produce it is the whole loop. It is a second, separate
/// console instance — deliberately, so collapsing the global one doesn't take
/// this one with it.
fn layout_blueprints() -> DockTree {
    DockTree::horizontal(
        DockTree::leaf("blueprint_properties"),
        DockTree::vertical(
            DockTree::leaf("blueprint_graph"),
            DockTree::leaf("console"),
            0.75,
        ),
        0.18,
    )
}

/// Scripting: Hierarchy over Problems | CodeEditor.
///
/// The code editor gets the whole width beside one narrow column. It carries
/// its own tab strip for open files and its own toolbar, so the Scripts, Outline
/// and Variables panels were mostly duplicating it and are gone.
///
/// No viewport: this workspace is for reading and writing code, and a viewport
/// sharing the row with the editor left neither one wide enough to be worth
/// having. Scene is a ribbon click away when you want to see what the script
/// does, and it already renders the same live scene.
///
/// Problems sits under the hierarchy rather than under the editor — the diagnostics
/// list is short and narrow, so a side column costs the editor nothing, where a
/// row beneath it cost the editor the height it most needs.
///
/// Neither `console` nor `assets` is here: both are [`DEFAULT_BOTTOM_TABS`],
/// present in every workspace via the global bottom panel. A copy in the
/// workspace would be a second, independent instance of the same panel sitting
/// right above it.
fn layout_scripting() -> DockTree {
    DockTree::horizontal(
        DockTree::vertical(
            DockTree::leaf("hierarchy"),
            DockTree::leaf("problems"),
            0.6,
        ),
        DockTree::leaf("code_editor"),
        0.18,
    )
}

/// Animation: Hierarchy | (StudioPreview/StateMachine) | (Properties/Params)
///
/// No `timeline` region: it lives in the global bottom panel now, spanning
/// every workspace, so a copy here would be a second independent instance
/// directly above it.
fn layout_animation() -> DockTree {
    DockTree::horizontal(
        DockTree::leaf("hierarchy"),
        DockTree::horizontal(
            DockTree::vertical(
                DockTree::leaf("studio_preview"),
                DockTree::leaf("animator_state_machine"),
                0.55,
            ),
            DockTree::vertical(
                DockTree::leaf("animation"),
                DockTree::leaf("animator_params"),
                0.55,
            ),
            0.72,
        ),
        0.15,
    )
}

/// Materials: Preview over Material | MaterialGraph.
///
/// The Material panel sits under the preview rather than tabbing with it, so
/// the node you clicked and the sphere it shades are both on screen at once —
/// tabbed, reading a pin meant hiding the thing the pin changes.
fn layout_materials() -> DockTree {
    DockTree::horizontal(
        DockTree::vertical(
            DockTree::leaf("material_preview"),
            DockTree::leaf("material_inspector"),
            0.5,
        ),
        DockTree::leaf("material_graph"),
        0.25,
    )
}

/// Particles: ParticlePreview | ParticleEditor
fn layout_particles() -> DockTree {
    DockTree::horizontal(
        DockTree::leaf("particle_preview"),
        DockTree::leaf("particle_editor"),
        0.8,
    )
}

/// Debug: Hierarchy/Performance | Viewport+diag panels | Inspector/ECS + diagnostics
fn layout_debug() -> DockTree {
    DockTree::horizontal(
        DockTree::vertical(
            DockTree::leaf("hierarchy"),
            DockTree::leaf("performance"),
            0.6,
        ),
        DockTree::horizontal(
            DockTree::vertical(
                DockTree::leaf("viewport"),
                DockTree::horizontal(
                    DockTree::horizontal(
                        DockTree::leaf("system_profiler"),
                        DockTree::leaf("render_stats"),
                        0.5,
                    ),
                    DockTree::horizontal(
                        DockTree::leaf("memory_profiler"),
                        DockTree::horizontal(
                            DockTree::leaf("physics_debug"),
                            DockTree::leaf("camera_debug"),
                            0.5,
                        ),
                        0.33,
                    ),
                    0.4,
                ),
                0.65,
            ),
            DockTree::vertical(
                DockTree::tabs(&["inspector", "gamepad", "ecs_stats"]),
                DockTree::tabs(&[
                    "scene_diagnostics",
                    "material_resolver_diag",
                    "lumen_diag",
                    "scripting_diag",
                ]),
                0.5,
            ),
            0.75,
        ),
        0.15,
    )
}

/// Scene workspace: hierarchy/scenes | the viewport | the inspector.
///
/// **The tree gets the left column, not a shelf above the inspector.** For a
/// while these two shared one right-hand column, tree stacked over inspector, on
/// the reasoning that you pick an entity and then edit it right below. In use
/// that mostly meant both panels were short: a scene tree is a *tall* list, and
/// halving its height costs it more than the narrow column ever gave back, while
/// the inspector's own content grows with the selection and wants the same
/// height for itself. They're back to a column each, which is also the shape
/// every other workspace here already uses — Scripting, Animation and Debug all
/// lead with a hierarchy column on the left.
///
/// Both side columns stay narrow. Neither a list of names nor a stack of
/// labelled fields needs width, and every pixel they give up goes to the
/// viewport, which is the panel that can always spend it.
///
/// **No bottom strip, and no `assets`.** Both used to live here: assets as the
/// lower half of the left column, and console/timeline/mixer/shape_library as a
/// strip under the viewport. All five are [`DEFAULT_BOTTOM_TABS`] now — the one
/// global bottom panel, shared by every workspace and owned by none of them.
/// Leaving a copy in this tree would mean a fresh install opened two `assets`
/// panels, each with its own independent state, one directly above the other;
/// and it would put the global panel's contents inside a workspace, where
/// "Reset Workspace" would be entitled to overwrite them.
///
/// **The inspector is alone in its column.** `gamepad` and `history` used to be
/// tabbed beside it. Neither is something you keep an eye on while editing — you
/// open the undo history when you want to step back through it, and the gamepad
/// panel when you are wiring up input — so as defaults they only cost the
/// inspector two tab widths and gave a fresh install two tabs it would never
/// click. Both are still one click away in the panel menu.
pub fn scene_layout() -> DockTree {
    DockTree::horizontal(
        DockTree::tabs(&["hierarchy", "scenes"]),
        DockTree::horizontal(
            DockTree::tabs(&["viewport", "code_editor"]),
            DockTree::leaf("inspector"),
            0.78,
        ),
        0.15,
    )
}

/// Marketplace workspace: what you sell on the left, what you own on the right.
///
/// This was the "Hub" — a community home with friends down one side and feed,
/// messages and docs across the middle. All of that is gone: the account exists
/// to publish and purchase assets and nothing else.
///
/// The **store itself is not here** any more. Browsing is a place you go rather
/// than a panel you keep, so it opens as an overlay from the storefront icon in
/// the top bar (see `renzora_marketplace::store_overlay`). What is left is the
/// half of an account you come back to: uploading, what you own, and what you
/// can spend. `asset_uploader` carries both halves of selling — becoming a
/// creator and uploading — so there is no separate onboarding tab to place.
fn layout_marketplace() -> DockTree {
    DockTree::horizontal(
        DockTree::leaf("asset_uploader"),
        DockTree::tabs(&["hub_library", "social_wallet"]),
        0.6,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one-time fold to the global bottom panel must not lose a panel.
    /// A closed stash is the only place its panels exist, and an open strip is
    /// about to be cut out of the workspace tree — drop either and the panel is
    /// gone from the user's layout with no way to get it back.
    #[test]
    fn migrate_keeps_panels_from_both_open_strips_and_closed_stashes() {
        let mut workspaces = vec![(
            "Scene".to_string(),
            DockTree::vertical(
                DockTree::leaf("viewport"),
                DockTree::tabs(&["console", "assets"]),
                0.7,
            ),
        )];
        let mut closed = BTreeMap::new();
        closed.insert(
            "Animation".to_string(),
            ClosedBottom {
                tree: DockTree::tabs(&["timeline", "mixer"]),
                ratio: 0.7,
                anchor: Vec::new(),
            },
        );

        let bottom = migrate_bottom_dock(&mut workspaces, &closed);

        let mut ids = Vec::new();
        bottom.tree.collect_panels(&mut ids);
        for want in ["console", "assets", "timeline", "mixer"] {
            assert!(ids.contains(&want.to_string()), "lost {want}: {ids:?}");
        }
        // The strip is cut out of the workspace, not copied — leaving it would
        // put console/assets in two places on the very first launch.
        assert!(
            !workspaces[0].1.contains_panel("console"),
            "strip should be removed from the workspace tree"
        );
        assert!(workspaces[0].1.contains_panel("viewport"));
    }

    /// Every workspace ships much the same strip, so a verbatim fold would
    /// produce a bottom panel tabbing `console` once per workspace.
    #[test]
    fn migrate_deduplicates_the_same_panel_across_workspaces() {
        let strip = || {
            DockTree::vertical(
                DockTree::leaf("viewport"),
                DockTree::tabs(&["console", "assets"]),
                0.7,
            )
        };
        let mut workspaces = vec![
            ("Scene".to_string(), strip()),
            ("Scripting".to_string(), strip()),
            ("Debug".to_string(), strip()),
        ];

        let bottom = migrate_bottom_dock(&mut workspaces, &BTreeMap::new());

        let mut ids = Vec::new();
        bottom.tree.collect_panels(&mut ids);
        assert_eq!(
            ids.iter().filter(|i| *i == "console").count(),
            1,
            "console tabbed once per workspace: {ids:?}"
        );
        assert_eq!(ids.iter().filter(|i| *i == "assets").count(), 1);
    }

    /// A layout with nothing to fold must not leave the bottom panel holding an
    /// empty *leaf*: ember renders an empty tree as its "Add Panel" button, but
    /// an empty leaf as a tab bar with no tabs in it.
    #[test]
    fn migrate_with_no_strips_yields_an_empty_tree() {
        let mut workspaces = vec![(
            "Bare".to_string(),
            DockTree::horizontal(DockTree::leaf("viewport"), DockTree::leaf("inspector"), 0.5),
        )];

        let bottom = migrate_bottom_dock(&mut workspaces, &BTreeMap::new());

        assert!(bottom.tree.is_empty());
    }

    /// The default global panel is what a fresh install gets and what
    /// "Reset Global Docks" restores, so its tab set is worth pinning.
    #[test]
    fn the_default_bottom_dock_holds_every_default_tab() {
        let bottom = default_bottom_dock();

        let mut ids = Vec::new();
        bottom.tree.collect_panels(&mut ids);
        let want: Vec<String> = DEFAULT_BOTTOM_TABS.iter().map(|t| t.to_string()).collect();
        assert_eq!(ids, want);
        // Left for the shell to wrap in its one default set — see
        // `default_bottom_dock` for why the name isn't spelled twice.
        assert!(bottom.sets.is_empty());
    }

    /// The panel resizes the whole way up, and layout mode hands over rather
    /// than crushing the workspace into a stack of tab bars.
    #[test]
    fn layout_mode_gives_way_to_overlay_near_the_top() {
        let avail = 800.0;
        // Room for a workspace above: layout mode is honoured.
        assert_eq!(
            BottomDockMode::Layout.effective(400.0, avail),
            BottomDockMode::Layout
        );
        // Dragged to the top bar: nothing left to reflow, so it overlays.
        assert_eq!(
            BottomDockMode::Layout.effective(avail, avail),
            BottomDockMode::Overlay
        );
        // Overlay never becomes layout by being short — the switch is one-way.
        assert_eq!(
            BottomDockMode::Overlay.effective(100.0, avail),
            BottomDockMode::Overlay
        );
        // And it is a function of the height, not a latch: coming back down
        // restores layout mode with nothing to reset.
        assert_eq!(
            BottomDockMode::Layout.effective(max_layout_height(avail), avail),
            BottomDockMode::Layout
        );
    }

    /// A window too short for the panel *and* its minimum workspace must not
    /// invert the range and clamp every height to zero.
    #[test]
    fn a_short_window_still_leaves_a_usable_range() {
        let avail = 100.0;
        assert_eq!(max_layout_height(avail), BOTTOM_DOCK_MIN_HEIGHT);
        assert_eq!(clamp_height(10.0, avail), BOTTOM_DOCK_MIN_HEIGHT);
        assert_eq!(clamp_height(500.0, avail), avail);
        // Before the dock region has been laid out there is no ceiling yet.
        assert_eq!(clamp_height(500.0, f32::INFINITY), 500.0);
    }

    /// No default workspace may carry a panel the global bottom dock owns: a
    /// fresh install would open two of it, and "Reset Workspace" would then be
    /// resetting global state. `console` is the one exception — Blueprints docks
    /// it beside its graph deliberately.
    #[test]
    fn no_default_workspace_duplicates_a_global_bottom_tab() {
        for (name, tree) in workspace_layouts() {
            for tab in DEFAULT_BOTTOM_TABS {
                if tab == "console" {
                    continue;
                }
                assert!(
                    !tree.contains_panel(tab),
                    "workspace {name} docks {tab}, which lives in the global bottom panel"
                );
            }
        }
    }

    /// A layout file written before panel sets existed has no `sets` key at
    /// all. It must still load — as a panel with no sets, which the shell reads
    /// as "one set, holding `tree`". Losing this default would greet every
    /// existing user with an empty bottom panel.
    #[test]
    fn a_layout_without_panel_sets_still_loads() {
        let json = r#"{
            "tree": {"Leaf": {"tabs": ["console", "assets"], "active_tab": 0}},
            "height": 220.0,
            "open": true
        }"#;
        let bottom: BottomDockLayout = serde_json::from_str(json).expect("pre-sets layout");
        assert!(bottom.sets.is_empty());
        assert_eq!(bottom.active, 0);
        assert!(bottom.tree.contains_panel("console"));
        assert!(bottom.mode == BottomDockMode::Overlay);
    }

    /// And a file that *does* carry sets round-trips them, active index
    /// included — the panel's whole point is that the set you left it on is the
    /// set you come back to.
    #[test]
    fn panel_sets_round_trip_through_the_layout_file() {
        let bottom = BottomDockLayout {
            tree: DockTree::leaf("mixer"),
            height: 240.0,
            open: true,
            mode: BottomDockMode::Layout,
            sets: vec![
                BottomPanelSet {
                    name: "Panels".to_string(),
                    tree: DockTree::tabs(&["console", "assets"]),
                },
                BottomPanelSet {
                    name: "Panels 2".to_string(),
                    tree: DockTree::leaf("mixer"),
                },
            ],
            active: 1,
        };

        let text = serde_json::to_string(&bottom).expect("serialize");
        let back: BottomDockLayout = serde_json::from_str(&text).expect("deserialize");

        assert_eq!(back.active, 1);
        assert_eq!(back.sets.len(), 2);
        assert_eq!(back.sets[1].name, "Panels 2");
        assert!(back.sets[0].tree.contains_panel("console"));
        assert!(back.sets[1].tree.contains_panel("mixer"));
    }
}
