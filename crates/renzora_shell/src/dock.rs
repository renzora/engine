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
/// never lived below a vertical divider). Matches the default scene layout.
pub const BOTTOM_PANEL_RATIO: f32 = 0.72;

/// Opening height of the global bottom panel, logical px. Roughly the old
/// 0.72 split on a 1080p window, so an upgraded layout looks unchanged.
pub const BOTTOM_DOCK_HEIGHT: f32 = 280.0;

/// Smallest height the bottom panel can be dragged to before it reads as a
/// tab strip with no room for content. Below this the drag snaps it closed.
pub const BOTTOM_DOCK_MIN_HEIGHT: f32 = 80.0;

/// Pull `tree`'s bottom strip out and fold its panels into `bottom`, adopting
/// only ids the bottom panel doesn't already hold.
///
/// For restoring a pristine default tree: [`workspace_layouts`] still describes
/// each workspace with its bottom strip in-tree (that is what
/// [`migrate_bottom_dock`] reads on a first run), so a reset that used those
/// trees verbatim would put `console`/`assets` in the workspace *and* leave
/// them in the global bottom panel. Duplicate panels are allowed when a user
/// makes them deliberately; a reset producing them by itself is just a bug.
pub fn absorb_bottom_strip(tree: &mut DockTree, bottom: &mut DockTree) {
    let Some(stash) = take_bottom_strip(tree) else {
        return;
    };
    let mut ids = Vec::new();
    stash.tree.collect_panels(&mut ids);
    for id in ids {
        if !bottom.contains_panel(&id) {
            bottom.adopt_panel(&id);
        }
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
    // The Material panel: name/domain plus the selected node's pin values. The
    // graph now draws those editors on the nodes themselves, under the pin they
    // belong to, for every node at once — so the panel was a second copy of the
    // same controls, one node at a time, a screen away from the node.
    "material_inspector",
];

/// Is `tree`'s leaf holding `console` the classic bottom strip? The strip is
/// recognized by console being tabbed together with another strip panel —
/// mixer/timeline/shape_library in the shipped default, or
/// assets/hub_store in layouts saved before those moved out of the strip.
/// Requiring a companion keeps this from matching a standalone console leaf
/// (Blueprints) or the console+problems pair (Scripting) — those stay open at
/// launch, as before.
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
#[derive(Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
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

/// The global bottom panel's persisted state — one per editor, not one per
/// workspace, which is the whole point of it (see [`renzora_ember::dock::FixedDock`]).
///
/// `height` is logical px, not a ratio: the panel is sized in absolute terms in
/// both modes, so there is no sibling to be a fraction of, and a ratio would
/// silently rescale the panel when the window resizes.
#[derive(Clone, Serialize, Deserialize)]
pub struct BottomDockLayout {
    pub tree: DockTree,
    pub height: f32,
    pub open: bool,
    /// `#[serde(default)]` so a layout file written before the mode toggle
    /// existed loads as `Overlay` — the behaviour it was saved with.
    #[serde(default)]
    pub mode: BottomDockMode,
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
        ("Debug".into(), layout_debug()),
        ("Hub".into(), layout_hub()),
    ]
}

/// Blueprints: Hierarchy+NodeProperties | BlueprintGraph+Console | Inspector
fn layout_blueprints() -> DockTree {
    DockTree::horizontal(
        DockTree::vertical(
            DockTree::leaf("hierarchy"),
            DockTree::leaf("blueprint_properties"),
            0.5,
        ),
        DockTree::horizontal(
            DockTree::vertical(
                DockTree::leaf("blueprint_graph"),
                DockTree::leaf("console"),
                0.75,
            ),
            DockTree::leaf("inspector"),
            0.78,
        ),
        0.18,
    )
}

/// Scripting: Hierarchy/Assets | CodeEditor + Console/Problems | Viewport
///
/// The Scripts, Outline and Variables panels are gone, so the right-hand column
/// is the viewport alone and the code editor gets the width they were using. The
/// editor carries its own tab strip for open files and its own toolbar, which is
/// what those panels were mostly duplicating.
fn layout_scripting() -> DockTree {
    DockTree::horizontal(
        DockTree::vertical(
            DockTree::leaf("hierarchy"),
            DockTree::leaf("assets"),
            0.4,
        ),
        DockTree::horizontal(
            DockTree::vertical(
                DockTree::leaf("code_editor"),
                // No `console` here: it lives in the global bottom panel, which
                // is present in every workspace. A copy in the workspace would
                // be a second, independent instance sitting right above it.
                DockTree::leaf("problems"),
                0.72,
            ),
            DockTree::leaf("viewport"),
            0.68,
        ),
        0.16,
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

/// Materials: Preview | MaterialGraph. Pin values are edited on the nodes, so
/// the graph gets the whole width beside the preview.
fn layout_materials() -> DockTree {
    DockTree::horizontal(
        DockTree::leaf("material_preview"),
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

/// Scene workspace: a full-height left column (hierarchy/scenes over the
/// asset browser, split 50/50) | a viewport column (viewport over the console
/// strip) | a full-height right column (inspector/gamepad/history). The strip
/// sits **under the viewport, not full-width at the root** — both side
/// columns keep their full height. It is still the collapsible bottom panel:
/// startup stashes it closed ([`take_bottom_strip`]), Ctrl+Space toggles it,
/// and its tab bar keeps the collapse chevron wherever the strip is docked,
/// because the shell registers `console` as a `BottomStripMarkers` panel (see
/// the shell's `toggle_bottom_panel`).
pub fn scene_layout() -> DockTree {
    DockTree::horizontal(
        // Left column: hierarchy/scenes tabs over the asset browser.
        DockTree::vertical(
            DockTree::tabs(&["hierarchy", "scenes"]),
            DockTree::leaf("assets"),
            0.5,
        ),
        DockTree::horizontal(
            DockTree::vertical(
                DockTree::tabs(&["viewport", "code_editor", "social_learn"]),
                DockTree::tabs(&["console", "timeline", "mixer", "shape_library"]),
                BOTTOM_PANEL_RATIO,
            ),
            DockTree::tabs(&["inspector", "gamepad", "history"]),
            0.78,
        ),
        0.16,
    )
}

/// Hub workspace: the community home. A left friends column | the main content
/// tabs (feed, messages, docs, marketplace, become a creator, publish) | a right
/// column with the wallet over the asset library. Notifications live in the
/// top-bar bell dropdown, teams in the Friends panel's Teams tab, and profiles in
/// a shared overlay, so none of them are panels here anymore; the forum was
/// replaced by the feed.
fn layout_hub() -> DockTree {
    DockTree::horizontal(
        // Left: friends.
        DockTree::leaf("social_friends"),
        DockTree::horizontal(
            // Center: the big content surfaces, in reading order.
            DockTree::tabs(&[
                "social_feed",
                "social_chat",
                "social_learn",
                "hub_store",
                "social_onboarding",
                "asset_uploader",
            ]),
            // Right: wallet over the asset library.
            DockTree::tabs(&["social_wallet", "hub_library"]),
            0.62,
        ),
        0.18,
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
    /// empty leaf — `sync_bottom_dock_node` keys "is there anything to show" off
    /// `is_empty`, and an empty leaf would render a bare bordered slab.
    #[test]
    fn migrate_with_no_strips_yields_an_empty_tree() {
        let mut workspaces = vec![(
            "Bare".to_string(),
            DockTree::horizontal(DockTree::leaf("viewport"), DockTree::leaf("inspector"), 0.5),
        )];

        let bottom = migrate_bottom_dock(&mut workspaces, &BTreeMap::new());

        assert!(bottom.tree.is_empty());
    }

    /// Restoring a pristine default must lift its in-tree strip back out rather
    /// than restoring a second copy beside the global panel's.
    #[test]
    fn absorb_moves_a_default_strip_into_the_existing_bottom_panel() {
        let mut tree = DockTree::vertical(
            DockTree::leaf("viewport"),
            DockTree::tabs(&["console", "assets"]),
            0.7,
        );
        let mut bottom = DockTree::leaf("console");

        absorb_bottom_strip(&mut tree, &mut bottom);

        let mut ids = Vec::new();
        bottom.collect_panels(&mut ids);
        assert_eq!(ids.iter().filter(|i| *i == "console").count(), 1);
        assert!(ids.contains(&"assets".to_string()));
        assert!(!tree.contains_panel("console"));
    }
}
