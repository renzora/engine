//! Dock tree data model — the editor panel layout.
//!
//! This is an **egui-free** port of the layout model that currently lives in
//! `renzora_ui::dock_tree` (which is coupled to egui only in its *renderer*,
//! not its data). The bevy_ui shell reuses the same shape — a binary tree of
//! `Split`s and `Leaf`s — so workspace layouts translate 1:1. The renderer is
//! the only thing that changes (egui immediate-mode → bevy_ui reconcile).
//!
//! Mutation methods, persistence and the full `LayoutManager` will be ported
//! as later phases land (resize, tab drag/drop, ribbon workspace switching).
//! For now we only need the structure + the default Scene layout.

/// Direction of a split in the dock tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    /// Children are side-by-side (left / right).
    Horizontal,
    /// Children are stacked (top / bottom).
    Vertical,
}

/// A node in the dock tree.
#[derive(Debug, Clone)]
pub enum DockTree {
    Split {
        direction: SplitDirection,
        /// Fraction of space given to the first child (0.1–0.9).
        ratio: f32,
        first: Box<DockTree>,
        second: Box<DockTree>,
    },
    Leaf {
        /// Panel IDs shown as tabs.
        tabs: Vec<String>,
        /// Index of the currently visible tab.
        active_tab: usize,
    },
    Empty,
}

impl DockTree {
    /// Single-tab leaf.
    pub fn leaf(id: impl Into<String>) -> Self {
        DockTree::Leaf {
            tabs: vec![id.into()],
            active_tab: 0,
        }
    }

    /// A leaf with several tabbed panels.
    pub fn tabs(tabs: &[&str]) -> Self {
        DockTree::Leaf {
            tabs: tabs.iter().map(|s| s.to_string()).collect(),
            active_tab: 0,
        }
    }

    /// Horizontal split (left / right).
    pub fn horizontal(first: DockTree, second: DockTree, ratio: f32) -> Self {
        DockTree::Split {
            direction: SplitDirection::Horizontal,
            ratio: ratio.clamp(0.1, 0.9),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Vertical split (top / bottom).
    pub fn vertical(first: DockTree, second: DockTree, ratio: f32) -> Self {
        DockTree::Split {
            direction: SplitDirection::Vertical,
            ratio: ratio.clamp(0.1, 0.9),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Set the split ratio at `path` — a chain of branch choices from the root
    /// (`false` = descend into the first child, `true` = the second). An empty
    /// path targets this node. Used to persist a divider drag.
    pub fn update_ratio(&mut self, path: &[bool], new_ratio: f32) {
        if let DockTree::Split {
            ratio,
            first,
            second,
            ..
        } = self
        {
            match path.split_first() {
                Some((&true, rest)) => second.update_ratio(rest, new_ratio),
                Some((&false, rest)) => first.update_ratio(rest, new_ratio),
                None => *ratio = new_ratio.clamp(0.1, 0.9),
            }
        }
    }

    /// The leaf that contains `panel`, if any.
    pub fn find_leaf_mut(&mut self, panel: &str) -> Option<&mut DockTree> {
        match self {
            DockTree::Split { first, second, .. } => first
                .find_leaf_mut(panel)
                .or_else(|| second.find_leaf_mut(panel)),
            DockTree::Leaf { tabs, .. } => tabs.iter().any(|t| t == panel).then_some(self),
            DockTree::Empty => None,
        }
    }

    /// Is `panel` the active (visible) tab in its leaf?
    pub fn is_active_tab(&self, panel: &str) -> bool {
        match self {
            DockTree::Split { first, second, .. } => {
                first.is_active_tab(panel) || second.is_active_tab(panel)
            }
            DockTree::Leaf { tabs, active_tab } => {
                tabs.get(*active_tab).is_some_and(|t| t == panel)
            }
            DockTree::Empty => false,
        }
    }

    /// Make `panel` the active tab in its leaf.
    pub fn set_active_tab(&mut self, panel: &str) {
        if let Some(DockTree::Leaf { tabs, active_tab }) = self.find_leaf_mut(panel) {
            if let Some(idx) = tabs.iter().position(|t| t == panel) {
                *active_tab = idx;
            }
        }
    }

    /// Append `new_panel` as a tab in the leaf containing `sibling`.
    pub fn add_tab(&mut self, sibling: &str, new_panel: String) -> bool {
        if let Some(DockTree::Leaf { tabs, active_tab }) = self.find_leaf_mut(sibling) {
            tabs.push(new_panel);
            *active_tab = tabs.len() - 1;
            true
        } else {
            false
        }
    }

    /// Insert `new_panel` into `sibling`'s leaf immediately before `before`
    /// (or at the end if `before` is `None` / not present). Used for precise
    /// tab-bar drop insertion.
    pub fn add_tab_before(&mut self, sibling: &str, new_panel: String, before: Option<&str>) -> bool {
        if let Some(DockTree::Leaf { tabs, active_tab }) = self.find_leaf_mut(sibling) {
            let idx = before
                .and_then(|b| tabs.iter().position(|t| t == b))
                .unwrap_or(tabs.len())
                .min(tabs.len());
            tabs.insert(idx, new_panel);
            *active_tab = idx;
            true
        } else {
            false
        }
    }

    /// Remove a panel from the tree, collapsing any emptied leaves/splits.
    pub fn remove_panel(&mut self, panel: &str) -> bool {
        let removed = match self {
            DockTree::Split { first, second, .. } => {
                first.remove_panel(panel) || second.remove_panel(panel)
            }
            DockTree::Leaf { tabs, active_tab } => {
                if let Some(idx) = tabs.iter().position(|t| t == panel) {
                    tabs.remove(idx);
                    if *active_tab >= tabs.len() && !tabs.is_empty() {
                        *active_tab = tabs.len() - 1;
                    }
                    true
                } else {
                    false
                }
            }
            DockTree::Empty => false,
        };
        if removed {
            self.cleanup_empty();
        }
        removed
    }

    /// Collapse empty leaves and splits with an empty side.
    fn cleanup_empty(&mut self) {
        if let DockTree::Split { first, second, .. } = self {
            first.cleanup_empty();
            second.cleanup_empty();
            let first_empty = first.is_empty();
            let second_empty = second.is_empty();
            if first_empty {
                *self = std::mem::replace(second, DockTree::Empty);
            } else if second_empty {
                *self = std::mem::replace(first, DockTree::Empty);
            }
        } else if let DockTree::Leaf { tabs, .. } = self {
            if tabs.is_empty() {
                *self = DockTree::Empty;
            }
        }
    }

    fn is_empty(&self) -> bool {
        matches!(self, DockTree::Empty)
            || matches!(self, DockTree::Leaf { tabs, .. } if tabs.is_empty())
    }

    /// Split the leaf containing `target`, placing `new_panel` on the given
    /// side. (Use [`add_tab`](Self::add_tab) for the center/tab case.)
    pub fn split_at(&mut self, target: &str, new_panel: String, zone: DropZone) -> bool {
        if let Some(leaf) = self.find_leaf_mut(target) {
            let old = std::mem::replace(leaf, DockTree::Empty);
            let new_leaf = DockTree::leaf(new_panel);
            *leaf = match zone {
                DropZone::Left => DockTree::horizontal(new_leaf, old, 0.5),
                DropZone::Right => DockTree::horizontal(old, new_leaf, 0.5),
                DropZone::Top => DockTree::vertical(new_leaf, old, 0.5),
                DropZone::Bottom => DockTree::vertical(old, new_leaf, 0.5),
                DropZone::Center => {
                    *leaf = old;
                    return false;
                }
            };
            true
        } else {
            false
        }
    }
}

/// Where a dragged panel will land on a leaf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropZone {
    /// Add as a tab in the target leaf.
    Center,
    Left,
    Right,
    Top,
    Bottom,
}

/// The ribbon workspace layouts, in ribbon order (Scene … Debug). Ports
/// `renzora_ui::layouts` (the visible, non-asset layouts) into the shell's
/// egui-free dock model.
pub fn workspace_layouts() -> Vec<(String, DockTree)> {
    vec![
        ("Scene".into(), scene_layout()),
        ("Blueprints".into(), layout_blueprints()),
        ("Scripting".into(), layout_scripting()),
        ("Animation".into(), layout_animation()),
        ("Materials".into(), layout_materials()),
        ("Particles".into(), layout_particles()),
        ("Video".into(), layout_video()),
        ("Audio".into(), layout_audio()),
        ("Debug".into(), layout_debug()),
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

/// Scripting: Hierarchy/Scripts/Assets | CodeEditor+Console | Viewport/Outline/Vars
fn layout_scripting() -> DockTree {
    DockTree::horizontal(
        DockTree::vertical(
            DockTree::leaf("hierarchy"),
            DockTree::vertical(
                DockTree::leaf("scripts_on_entity"),
                DockTree::leaf("assets"),
                0.4,
            ),
            0.4,
        ),
        DockTree::horizontal(
            DockTree::vertical(
                DockTree::leaf("code_editor"),
                DockTree::tabs(&["console", "problems"]),
                0.7,
            ),
            DockTree::vertical(
                DockTree::leaf("viewport"),
                DockTree::vertical(
                    DockTree::leaf("outline"),
                    DockTree::leaf("script_variables"),
                    0.4,
                ),
                0.6,
            ),
            0.7,
        ),
        0.16,
    )
}

/// Animation: Hierarchy | (StudioPreview/StateMachine) | (Properties/Params) | Timeline
fn layout_animation() -> DockTree {
    DockTree::vertical(
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
        ),
        DockTree::leaf("timeline"),
        0.60,
    )
}

/// Materials: Preview + Properties | MaterialGraph
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

/// Video: Hierarchy | Viewport | Inspector  /  Sequencer+Mixer+Assets
fn layout_video() -> DockTree {
    DockTree::vertical(
        DockTree::horizontal(
            DockTree::leaf("hierarchy"),
            DockTree::horizontal(DockTree::leaf("viewport"), DockTree::leaf("inspector"), 0.78),
            0.15,
        ),
        DockTree::tabs(&["sequencer", "mixer", "assets"]),
        0.55,
    )
}

/// Audio: Hierarchy | DAW | Inspector  /  Mixer | Assets+Console
fn layout_audio() -> DockTree {
    DockTree::vertical(
        DockTree::horizontal(
            DockTree::leaf("hierarchy"),
            DockTree::horizontal(DockTree::leaf("daw"), DockTree::leaf("inspector"), 0.78),
            0.15,
        ),
        DockTree::horizontal(
            DockTree::leaf("mixer"),
            DockTree::tabs(&["assets", "console"]),
            0.6,
        ),
        0.6,
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
                        DockTree::tabs(&["render_stats", "render_pipeline"]),
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

/// Scene workspace: main area (viewport + bottom tabs) on the left, a right
/// column stacking hierarchy/scenes/shapes over inspector/gamepad/history.
///
/// Mirrors `renzora_ui::layouts::scene_layout` so the bevy_ui shell renders the
/// same default the egui editor ships.
pub fn scene_layout() -> DockTree {
    DockTree::horizontal(
        // Main area: viewport on top, assets/console/etc tabbed below.
        DockTree::vertical(
            DockTree::tabs(&["viewport", "render_pipeline", "code_editor"]),
            DockTree::tabs(&[
                "assets",
                "hub_store",
                "console",
                "mixer",
                "sequencer",
                "timeline",
                "record",
            ]),
            0.72,
        ),
        // Right column: hierarchy/scenes/shapes on top, inspector/gamepad/history below.
        DockTree::vertical(
            DockTree::tabs(&["hierarchy", "scenes", "shape_library"]),
            DockTree::tabs(&["inspector", "gamepad", "history"]),
            0.4,
        ),
        0.82,
    )
}
