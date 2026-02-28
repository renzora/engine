//! Core dock tree data structures
//!
//! The dock tree represents the hierarchical layout of panels using a binary tree structure.
//! Each node is either a Split (dividing space between two children) or a Leaf (containing tabs).

use serde::{Deserialize, Serialize};
use egui_phosphor::regular::{
    TREE_STRUCTURE, SLIDERS_HORIZONTAL, FOLDER_OPEN, TERMINAL,
    MONITOR, FILM_STRIP, CODE, CLOCK_COUNTER_CLOCKWISE, PUZZLE_PIECE,
    GRAPH, LIST_BULLETS, GEAR, CUBE, GAME_CONTROLLER, CHART_LINE, CPU,
    STACK, CHART_BAR, ATOM, VIDEO_CAMERA, TIMER, WAVEFORM, IMAGE,
    SPARKLE, PAINT_BUCKET, SPEAKER_HIGH, VIDEO,
    EYE,
};

/// Direction of a split in the dock tree
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitDirection {
    Horizontal, // Children are side by side (left/right)
    Vertical,   // Children are stacked (top/bottom)
}

/// Identifies a panel type in the editor
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PanelId {
    Hierarchy,
    Inspector,
    Assets,
    Console,
    Viewport,
    Animation,
    Timeline,
    #[serde(alias = "ScriptEditor")]
    CodeEditor,
    ShaderPreview,
    History,
    Blueprint,
    NodeLibrary,
    MaterialPreview,
    Settings,
    Gamepad,
    Performance,
    RenderStats,
    EcsStats,
    MemoryProfiler,
    PhysicsDebug,
    CameraDebug,
    /// Culling debug — frustum/distance culling stats
    CullingDebug,
    SystemProfiler,
    LevelTools,
    /// Isolated 3D preview with studio lighting
    StudioPreview,
    /// Node explorer - shows entity hierarchy tree with components
    NodeExplorer,
    /// Image preview panel for viewing images
    ImagePreview,
    /// Video editor panel
    VideoEditor,
    /// Digital Audio Workstation panel
    DAW,
    /// Particle FX editor panel
    ParticleEditor,
    /// Particle preview panel - isolated viewport for particle effects
    ParticlePreview,
    /// Texture editor panel
    TextureEditor,
    /// Script Variables panel - shows props from active script
    ScriptVariables,
    /// Physics playground — stress-test spawner
    PhysicsPlayground,
    /// Physics properties — global simulation settings
    PhysicsProperties,
    /// Forces & impulses — interactive force application
    PhysicsForces,
    /// Physics metrics — energy, velocity, momentum monitoring
    PhysicsMetrics,
    /// Physics scenario presets — one-click test scene spawning
    PhysicsScenarios,
    /// Collision visualizer — contact points and normals
    CollisionViz,
    /// Movement trails — trajectory visualization
    MovementTrails,
    /// Stress test — automated scaling tests
    StressTest,
    /// State recorder — record/replay physics state
    StateRecorder,
    /// Arena presets — spawn pre-built arena environments
    ArenaPresets,
    /// Render pipeline — node graph visualization of render passes
    RenderPipeline,
    /// Shape Library — visual grid of mesh primitives for quick spawning
    ShapeLibrary,
    /// Custom plugin-provided panel
    Plugin(String),
}

impl PanelId {
    /// Get the display title for this panel
    pub fn title(&self) -> &str {
        match self {
            PanelId::Hierarchy => "Hierarchy",
            PanelId::Inspector => "Inspector",
            PanelId::Assets => "Assets",
            PanelId::Console => "Console",
            PanelId::Viewport => "Viewport",
            PanelId::Animation => "Animation",
            PanelId::Timeline => "Timeline",
            PanelId::CodeEditor => "Code Editor",
            PanelId::ShaderPreview => "Shader Preview",
            PanelId::History => "History",
            PanelId::Blueprint => "Blueprint",
            PanelId::NodeLibrary => "Node Library",
            PanelId::MaterialPreview => "Material Preview",
            PanelId::Settings => "Settings",
            PanelId::Gamepad => "Gamepad",
            PanelId::Performance => "Performance",
            PanelId::RenderStats => "Render Stats",
            PanelId::EcsStats => "ECS Stats",
            PanelId::MemoryProfiler => "Memory",
            PanelId::PhysicsDebug => "Physics Debug",
            PanelId::CameraDebug => "Camera Debug",
            PanelId::CullingDebug => "Culling Debug",
            PanelId::SystemProfiler => "System Profiler",
            PanelId::LevelTools => "Level Tools",
            PanelId::StudioPreview => "Studio Preview",
            PanelId::NodeExplorer => "Node Explorer",
            PanelId::ImagePreview => "Image Preview",
            PanelId::VideoEditor => "Video Editor",
            PanelId::DAW => "Audio",
            PanelId::ParticleEditor => "Particles",
            PanelId::ParticlePreview => "Particle Preview",
            PanelId::TextureEditor => "Textures",
            PanelId::ScriptVariables => "Script Variables",
            PanelId::PhysicsPlayground => "Physics Playground",
            PanelId::PhysicsProperties => "Physics Properties",
            PanelId::PhysicsForces => "Forces",
            PanelId::PhysicsMetrics => "Physics Metrics",
            PanelId::PhysicsScenarios => "Scenarios",
            PanelId::CollisionViz => "Collisions",
            PanelId::MovementTrails => "Trails",
            PanelId::StressTest => "Stress Test",
            PanelId::StateRecorder => "Recorder",
            PanelId::ArenaPresets => "Arena Presets",
            PanelId::RenderPipeline => "Render Pipeline",
            PanelId::ShapeLibrary => "Shape Library",
            PanelId::Plugin(name) => name,
        }
    }

    /// Get the locale key for this panel's title
    pub fn locale_key(&self) -> Option<&'static str> {
        match self {
            PanelId::Hierarchy => Some("panel.hierarchy"),
            PanelId::Inspector => Some("panel.inspector"),
            PanelId::Assets => Some("panel.assets"),
            PanelId::Console => Some("panel.console"),
            PanelId::Viewport => Some("panel.viewport"),
            PanelId::Animation => Some("panel.animation"),
            PanelId::Timeline => Some("panel.timeline"),
            PanelId::CodeEditor => Some("panel.code_editor"),
            PanelId::ShaderPreview => Some("panel.shader_preview"),
            PanelId::History => Some("panel.history"),
            PanelId::Blueprint => Some("panel.blueprint"),
            PanelId::NodeLibrary => Some("panel.node_library"),
            PanelId::MaterialPreview => Some("panel.material_preview"),
            PanelId::Settings => Some("panel.settings"),
            PanelId::Gamepad => Some("panel.gamepad"),
            PanelId::Performance => Some("panel.performance"),
            PanelId::RenderStats => Some("panel.render_stats"),
            PanelId::EcsStats => Some("panel.ecs_stats"),
            PanelId::MemoryProfiler => Some("panel.memory"),
            PanelId::PhysicsDebug => Some("panel.physics_debug"),
            PanelId::CameraDebug => Some("panel.camera_debug"),
            PanelId::CullingDebug => Some("panel.culling_debug"),
            PanelId::SystemProfiler => Some("panel.system_profiler"),
            PanelId::LevelTools => Some("panel.level_tools"),
            PanelId::StudioPreview => Some("panel.studio_preview"),
            PanelId::NodeExplorer => Some("panel.node_explorer"),
            PanelId::ImagePreview => Some("panel.image_preview"),
            PanelId::VideoEditor => Some("panel.video_editor"),
            PanelId::DAW => Some("panel.daw"),
            PanelId::ParticleEditor => Some("panel.particles"),
            PanelId::ParticlePreview => Some("panel.particle_preview"),
            PanelId::TextureEditor => Some("panel.textures"),
            PanelId::ScriptVariables => Some("panel.script_variables"),
            PanelId::PhysicsPlayground => Some("panel.physics_playground"),
            PanelId::PhysicsProperties => Some("panel.physics_properties"),
            PanelId::PhysicsForces => Some("panel.forces"),
            PanelId::PhysicsMetrics => Some("panel.physics_metrics"),
            PanelId::PhysicsScenarios => Some("panel.scenarios"),
            PanelId::CollisionViz => Some("panel.collisions"),
            PanelId::MovementTrails => Some("panel.trails"),
            PanelId::StressTest => Some("panel.stress_test"),
            PanelId::StateRecorder => Some("panel.recorder"),
            PanelId::ArenaPresets => Some("panel.arena_presets"),
            PanelId::RenderPipeline => Some("panel.render_pipeline"),
            PanelId::ShapeLibrary => Some("panel.shape_library"),
            PanelId::Plugin(_) => None,
        }
    }

    /// Get the localized display title for this panel.
    /// Falls back to the static English title if no locale is active.
    pub fn localized_title(&self) -> String {
        if let Some(key) = self.locale_key() {
            crate::locale::t(key)
        } else {
            self.title().to_string()
        }
    }

    /// Get the icon for this panel (Phosphor icons)
    pub fn icon(&self) -> &'static str {
        match self {
            PanelId::Hierarchy => TREE_STRUCTURE,
            PanelId::Inspector => SLIDERS_HORIZONTAL,
            PanelId::Assets => FOLDER_OPEN,
            PanelId::Console => TERMINAL,
            PanelId::Viewport => MONITOR,
            PanelId::Animation => FILM_STRIP,
            PanelId::Timeline => WAVEFORM,
            PanelId::CodeEditor => CODE,
            PanelId::ShaderPreview => MONITOR,
            PanelId::History => CLOCK_COUNTER_CLOCKWISE,
            PanelId::Blueprint => GRAPH,
            PanelId::NodeLibrary => LIST_BULLETS,
            PanelId::MaterialPreview => CUBE,
            PanelId::Settings => GEAR,
            PanelId::Gamepad => GAME_CONTROLLER,
            PanelId::Performance => CHART_LINE,
            PanelId::RenderStats => CPU,
            PanelId::EcsStats => STACK,
            PanelId::MemoryProfiler => CHART_BAR,
            PanelId::PhysicsDebug => ATOM,
            PanelId::CameraDebug => VIDEO_CAMERA,
            PanelId::CullingDebug => EYE,
            PanelId::SystemProfiler => TIMER,
            PanelId::LevelTools => CUBE,
            PanelId::StudioPreview => VIDEO_CAMERA,
            PanelId::NodeExplorer => TREE_STRUCTURE,
            PanelId::ImagePreview => IMAGE,
            PanelId::VideoEditor => VIDEO,
            PanelId::DAW => SPEAKER_HIGH,
            PanelId::ParticleEditor => SPARKLE,
            PanelId::ParticlePreview => SPARKLE,
            PanelId::TextureEditor => PAINT_BUCKET,
            PanelId::ScriptVariables => SLIDERS_HORIZONTAL,
            PanelId::PhysicsPlayground => CUBE,
            PanelId::PhysicsProperties => GEAR,
            PanelId::PhysicsForces => ATOM,
            PanelId::PhysicsMetrics => CHART_LINE,
            PanelId::PhysicsScenarios => CUBE,
            PanelId::CollisionViz => ATOM,
            PanelId::MovementTrails => CHART_LINE,
            PanelId::StressTest => CPU,
            PanelId::StateRecorder => TIMER,
            PanelId::ArenaPresets => CUBE,
            PanelId::RenderPipeline => STACK,
            PanelId::ShapeLibrary => CUBE,
            PanelId::Plugin(_) => PUZZLE_PIECE,
        }
    }

    /// Check if this panel can be closed (some panels like Viewport shouldn't be closeable)
    pub fn can_close(&self) -> bool {
        !matches!(self, PanelId::Viewport)
    }
}

/// A node in the dock tree - either a split or a leaf containing tabs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DockTree {
    /// A split node dividing space between two children
    Split {
        direction: SplitDirection,
        /// Ratio of first child's size (0.0 to 1.0)
        ratio: f32,
        /// First child (left or top)
        first: Box<DockTree>,
        /// Second child (right or bottom)
        second: Box<DockTree>,
    },
    /// A leaf node containing tabbed panels
    Leaf {
        /// List of panels in this tab group
        tabs: Vec<PanelId>,
        /// Index of the currently active tab
        active_tab: usize,
    },
    /// An empty node (placeholder during drag operations)
    Empty,
}

impl Default for DockTree {
    fn default() -> Self {
        // Default layout: Hierarchy | Viewport+Assets | Inspector
        DockTree::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.15,
            first: Box::new(DockTree::Leaf {
                tabs: vec![PanelId::Hierarchy],
                active_tab: 0,
            }),
            second: Box::new(DockTree::Split {
                direction: SplitDirection::Horizontal,
                ratio: 0.75, // Center takes 75% of remaining space
                first: Box::new(DockTree::Split {
                    direction: SplitDirection::Vertical,
                    ratio: 0.7,
                    first: Box::new(DockTree::Leaf {
                        tabs: vec![PanelId::Viewport],
                        active_tab: 0,
                    }),
                    second: Box::new(DockTree::Leaf {
                        tabs: vec![PanelId::Assets, PanelId::Console, PanelId::Animation],
                        active_tab: 0,
                    }),
                }),
                second: Box::new(DockTree::Leaf {
                    tabs: vec![PanelId::Inspector, PanelId::History],
                    active_tab: 0,
                }),
            }),
        }
    }
}

impl DockTree {
    /// Create a leaf with a single panel
    pub fn leaf(panel: PanelId) -> Self {
        DockTree::Leaf {
            tabs: vec![panel],
            active_tab: 0,
        }
    }

    /// Create a horizontal split (left/right)
    pub fn horizontal(first: DockTree, second: DockTree, ratio: f32) -> Self {
        DockTree::Split {
            direction: SplitDirection::Horizontal,
            ratio: ratio.clamp(0.1, 0.9),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Create a vertical split (top/bottom)
    pub fn vertical(first: DockTree, second: DockTree, ratio: f32) -> Self {
        DockTree::Split {
            direction: SplitDirection::Vertical,
            ratio: ratio.clamp(0.1, 0.9),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Find a leaf containing the given panel and return a mutable reference
    pub fn find_leaf_mut(&mut self, panel: &PanelId) -> Option<&mut DockTree> {
        match self {
            DockTree::Split { first, second, .. } => {
                first.find_leaf_mut(panel).or_else(|| second.find_leaf_mut(panel))
            }
            DockTree::Leaf { tabs, .. } => {
                if tabs.contains(panel) {
                    Some(self)
                } else {
                    None
                }
            }
            DockTree::Empty => None,
        }
    }

    /// Find a leaf containing the given panel
    #[allow(dead_code)]
    pub fn find_leaf(&self, panel: &PanelId) -> Option<&DockTree> {
        match self {
            DockTree::Split { first, second, .. } => {
                first.find_leaf(panel).or_else(|| second.find_leaf(panel))
            }
            DockTree::Leaf { tabs, .. } => {
                if tabs.contains(panel) {
                    Some(self)
                } else {
                    None
                }
            }
            DockTree::Empty => None,
        }
    }

    /// Remove a panel from the tree, cleaning up empty leaves
    pub fn remove_panel(&mut self, panel: &PanelId) -> bool {
        match self {
            DockTree::Split { first, second, .. } => {
                // Try to remove from children
                if first.remove_panel(panel) || second.remove_panel(panel) {
                    // Clean up empty nodes
                    self.cleanup_empty();
                    true
                } else {
                    false
                }
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
        }
    }

    /// Add a panel as a tab to the leaf containing target_panel
    pub fn add_tab(&mut self, target_panel: &PanelId, new_panel: PanelId) -> bool {
        if let Some(leaf) = self.find_leaf_mut(target_panel) {
            if let DockTree::Leaf { tabs, active_tab } = leaf {
                tabs.push(new_panel);
                *active_tab = tabs.len() - 1;
                return true;
            }
        }
        false
    }

    /// Split a leaf containing target_panel and add new_panel in the specified direction
    pub fn split_at(&mut self, target_panel: &PanelId, new_panel: PanelId, direction: SplitDirection, insert_first: bool) -> bool {
        self.split_at_recursive(target_panel, new_panel, direction, insert_first)
    }

    fn split_at_recursive(&mut self, target_panel: &PanelId, new_panel: PanelId, direction: SplitDirection, insert_first: bool) -> bool {
        match self {
            DockTree::Split { first, second, .. } => {
                // Check if target is directly in first or second child
                let in_first = first.contains_panel(target_panel);
                let in_second = second.contains_panel(target_panel);

                if in_first {
                    if let DockTree::Leaf { .. } = first.as_ref() {
                        // Replace first with a split
                        let old_first = std::mem::replace(first.as_mut(), DockTree::Empty);
                        let new_leaf = DockTree::leaf(new_panel);
                        *first = Box::new(if insert_first {
                            DockTree::Split {
                                direction,
                                ratio: 0.5,
                                first: Box::new(new_leaf),
                                second: Box::new(old_first),
                            }
                        } else {
                            DockTree::Split {
                                direction,
                                ratio: 0.5,
                                first: Box::new(old_first),
                                second: Box::new(new_leaf),
                            }
                        });
                        return true;
                    } else {
                        return first.split_at_recursive(target_panel, new_panel, direction, insert_first);
                    }
                }

                if in_second {
                    if let DockTree::Leaf { .. } = second.as_ref() {
                        // Replace second with a split
                        let old_second = std::mem::replace(second.as_mut(), DockTree::Empty);
                        let new_leaf = DockTree::leaf(new_panel);
                        *second = Box::new(if insert_first {
                            DockTree::Split {
                                direction,
                                ratio: 0.5,
                                first: Box::new(new_leaf),
                                second: Box::new(old_second),
                            }
                        } else {
                            DockTree::Split {
                                direction,
                                ratio: 0.5,
                                first: Box::new(old_second),
                                second: Box::new(new_leaf),
                            }
                        });
                        return true;
                    } else {
                        return second.split_at_recursive(target_panel, new_panel, direction, insert_first);
                    }
                }

                false
            }
            DockTree::Leaf { .. } => {
                // This is the target leaf - replace self with a split
                let old_self = std::mem::replace(self, DockTree::Empty);
                let new_leaf = DockTree::leaf(new_panel);
                *self = if insert_first {
                    DockTree::Split {
                        direction,
                        ratio: 0.5,
                        first: Box::new(new_leaf),
                        second: Box::new(old_self),
                    }
                } else {
                    DockTree::Split {
                        direction,
                        ratio: 0.5,
                        first: Box::new(old_self),
                        second: Box::new(new_leaf),
                    }
                };
                true
            }
            DockTree::Empty => false,
        }
    }

    /// Check if this tree contains the given panel
    pub fn contains_panel(&self, panel: &PanelId) -> bool {
        match self {
            DockTree::Split { first, second, .. } => {
                first.contains_panel(panel) || second.contains_panel(panel)
            }
            DockTree::Leaf { tabs, .. } => tabs.contains(panel),
            DockTree::Empty => false,
        }
    }

    /// Get all panels in the tree
    #[allow(dead_code)]
    pub fn all_panels(&self) -> Vec<PanelId> {
        let mut panels = Vec::new();
        self.collect_panels(&mut panels);
        panels
    }

    #[allow(dead_code)]
    fn collect_panels(&self, panels: &mut Vec<PanelId>) {
        match self {
            DockTree::Split { first, second, .. } => {
                first.collect_panels(panels);
                second.collect_panels(panels);
            }
            DockTree::Leaf { tabs, .. } => {
                panels.extend(tabs.iter().cloned());
            }
            DockTree::Empty => {}
        }
    }

    /// Clean up empty leaves and collapse single-child splits
    fn cleanup_empty(&mut self) {
        match self {
            DockTree::Split { first, second, .. } => {
                // Recursively clean children
                first.cleanup_empty();
                second.cleanup_empty();

                // If first is empty, replace self with second
                if matches!(first.as_ref(), DockTree::Empty) ||
                   matches!(first.as_ref(), DockTree::Leaf { tabs, .. } if tabs.is_empty()) {
                    let second_val = std::mem::replace(second.as_mut(), DockTree::Empty);
                    *self = second_val;
                }
                // If second is empty, replace self with first
                else if matches!(second.as_ref(), DockTree::Empty) ||
                        matches!(second.as_ref(), DockTree::Leaf { tabs, .. } if tabs.is_empty()) {
                    let first_val = std::mem::replace(first.as_mut(), DockTree::Empty);
                    *self = first_val;
                }
            }
            DockTree::Leaf { tabs, .. } => {
                if tabs.is_empty() {
                    *self = DockTree::Empty;
                }
            }
            DockTree::Empty => {}
        }
    }

    /// Update the split ratio for a split at the given path
    pub fn update_ratio(&mut self, path: &[bool], new_ratio: f32) {
        if path.is_empty() {
            if let DockTree::Split { ratio, .. } = self {
                *ratio = new_ratio.clamp(0.1, 0.9);
            }
            return;
        }

        if let DockTree::Split { first, second, .. } = self {
            if path[0] {
                second.update_ratio(&path[1..], new_ratio);
            } else {
                first.update_ratio(&path[1..], new_ratio);
            }
        }
    }

    /// Set the active tab for a leaf containing the given panel
    pub fn set_active_tab(&mut self, panel: &PanelId) {
        if let Some(leaf) = self.find_leaf_mut(panel) {
            if let DockTree::Leaf { tabs, active_tab } = leaf {
                if let Some(idx) = tabs.iter().position(|t| t == panel) {
                    *active_tab = idx;
                }
            }
        }
    }

    /// Count total number of leaves (tab groups) in the tree
    #[allow(dead_code)]
    pub fn leaf_count(&self) -> usize {
        match self {
            DockTree::Split { first, second, .. } => first.leaf_count() + second.leaf_count(),
            DockTree::Leaf { .. } => 1,
            DockTree::Empty => 0,
        }
    }

    /// Wrap the entire tree in a new root split, placing `panel` on the specified dock edge.
    /// The new panel takes ~30% of space on left/right or ~25% on top/bottom.
    pub fn wrap_with_panel(&mut self, panel: PanelId, zone: DropZone) {
        let old_tree = std::mem::replace(self, DockTree::Empty);
        let new_leaf = Box::new(DockTree::Leaf { tabs: vec![panel], active_tab: 0 });
        let old_box = Box::new(old_tree);
        *self = match zone {
            DropZone::EdgeLeft => DockTree::Split {
                direction: SplitDirection::Horizontal,
                ratio: 0.3,
                first: new_leaf,
                second: old_box,
            },
            DropZone::EdgeRight => DockTree::Split {
                direction: SplitDirection::Horizontal,
                ratio: 0.7,
                first: old_box,
                second: new_leaf,
            },
            DropZone::EdgeTop => DockTree::Split {
                direction: SplitDirection::Vertical,
                ratio: 0.25,
                first: new_leaf,
                second: old_box,
            },
            DropZone::EdgeBottom => DockTree::Split {
                direction: SplitDirection::Vertical,
                ratio: 0.75,
                first: old_box,
                second: new_leaf,
            },
            _ => *old_box, // shouldn't happen
        };
    }
}

/// Represents where a drop will occur
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropZone {
    /// Add as a new tab to existing leaf
    Tab,
    /// Split and place on the left of a panel
    Left,
    /// Split and place on the right of a panel
    Right,
    /// Split and place on top of a panel
    Top,
    /// Split and place on bottom of a panel
    Bottom,
    /// Full-height panel on the left edge of the dock
    EdgeLeft,
    /// Full-height panel on the right edge of the dock
    EdgeRight,
    /// Full-width panel on the top edge of the dock
    EdgeTop,
    /// Full-width panel on the bottom edge of the dock
    EdgeBottom,
}

impl DropZone {
    /// Convert to split direction and whether to insert first
    #[allow(dead_code)]
    pub fn to_split_params(self) -> Option<(SplitDirection, bool)> {
        match self {
            DropZone::Tab => None,
            DropZone::Left => Some((SplitDirection::Horizontal, true)),
            DropZone::Right => Some((SplitDirection::Horizontal, false)),
            DropZone::Top => Some((SplitDirection::Vertical, true)),
            DropZone::Bottom => Some((SplitDirection::Vertical, false)),
            DropZone::EdgeLeft => Some((SplitDirection::Horizontal, true)),
            DropZone::EdgeRight => Some((SplitDirection::Horizontal, false)),
            DropZone::EdgeTop => Some((SplitDirection::Vertical, true)),
            DropZone::EdgeBottom => Some((SplitDirection::Vertical, false)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_layout() {
        let tree = DockTree::default();
        assert!(tree.contains_panel(&PanelId::Hierarchy));
        assert!(tree.contains_panel(&PanelId::Viewport));
        assert!(tree.contains_panel(&PanelId::Inspector));
        assert!(tree.contains_panel(&PanelId::Assets));
    }

    #[test]
    fn test_remove_panel() {
        let mut tree = DockTree::default();
        assert!(tree.contains_panel(&PanelId::Console));
        tree.remove_panel(&PanelId::Console);
        assert!(!tree.contains_panel(&PanelId::Console));
    }

    #[test]
    fn test_add_tab() {
        let mut tree = DockTree::leaf(PanelId::Viewport);
        tree.add_tab(&PanelId::Viewport, PanelId::Assets);

        if let DockTree::Leaf { tabs, active_tab } = tree {
            assert_eq!(tabs.len(), 2);
            assert_eq!(active_tab, 1); // New tab is active
        } else {
            panic!("Expected leaf");
        }
    }

    #[test]
    fn test_split_at_horizontal() {
        let mut tree = DockTree::leaf(PanelId::Viewport);
        let result = tree.split_at(&PanelId::Viewport, PanelId::Inspector, SplitDirection::Horizontal, false);
        assert!(result);
        assert!(tree.contains_panel(&PanelId::Viewport));
        assert!(tree.contains_panel(&PanelId::Inspector));
        if let DockTree::Split { direction, .. } = &tree {
            assert_eq!(*direction, SplitDirection::Horizontal);
        } else {
            panic!("Expected split");
        }
    }

    #[test]
    fn test_split_at_vertical() {
        let mut tree = DockTree::leaf(PanelId::Viewport);
        let result = tree.split_at(&PanelId::Viewport, PanelId::Console, SplitDirection::Vertical, false);
        assert!(result);
        if let DockTree::Split { direction, .. } = &tree {
            assert_eq!(*direction, SplitDirection::Vertical);
        } else {
            panic!("Expected split");
        }
    }

    #[test]
    fn test_split_at_insert_first() {
        let mut tree = DockTree::leaf(PanelId::Viewport);
        tree.split_at(&PanelId::Viewport, PanelId::Hierarchy, SplitDirection::Horizontal, true);
        if let DockTree::Split { first, .. } = &tree {
            assert!(first.contains_panel(&PanelId::Hierarchy));
        } else {
            panic!("Expected split");
        }
    }

    #[test]
    fn test_remove_last_tab_becomes_empty() {
        let mut tree = DockTree::leaf(PanelId::Console);
        tree.remove_panel(&PanelId::Console);
        // After removing the only tab, should become Empty
        assert!(!tree.contains_panel(&PanelId::Console));
    }

    #[test]
    fn test_panel_id_title_non_empty() {
        let panels = [
            PanelId::Hierarchy, PanelId::Inspector, PanelId::Assets,
            PanelId::Console, PanelId::Viewport, PanelId::Animation,
            PanelId::CodeEditor, PanelId::Blueprint, PanelId::Settings,
            PanelId::Performance, PanelId::ParticleEditor,
        ];
        for panel in &panels {
            assert!(!panel.title().is_empty(), "{:?} should have a title", panel);
        }
    }

    #[test]
    fn test_panel_id_icon_non_empty() {
        let panels = [
            PanelId::Hierarchy, PanelId::Inspector, PanelId::Assets,
            PanelId::Console, PanelId::Viewport, PanelId::Blueprint,
        ];
        for panel in &panels {
            assert!(!panel.icon().is_empty(), "{:?} should have an icon", panel);
        }
    }

    #[test]
    fn test_viewport_cannot_close() {
        assert!(!PanelId::Viewport.can_close());
    }

    #[test]
    fn test_other_panels_can_close() {
        let closeable = [
            PanelId::Console, PanelId::Inspector, PanelId::Assets,
            PanelId::Hierarchy, PanelId::CodeEditor,
        ];
        for panel in &closeable {
            assert!(panel.can_close(), "{:?} should be closeable", panel);
        }
    }

    #[test]
    fn test_find_leaf_in_nested_tree() {
        let tree = DockTree::horizontal(
            DockTree::leaf(PanelId::Hierarchy),
            DockTree::vertical(
                DockTree::leaf(PanelId::Viewport),
                DockTree::horizontal(
                    DockTree::leaf(PanelId::Console),
                    DockTree::leaf(PanelId::Inspector),
                    0.5,
                ),
                0.7,
            ),
            0.2,
        );
        assert!(tree.find_leaf(&PanelId::Console).is_some());
        assert!(tree.find_leaf(&PanelId::Inspector).is_some());
        assert!(tree.find_leaf(&PanelId::Settings).is_none());
    }

    #[test]
    fn test_contains_panel_absent() {
        let tree = DockTree::leaf(PanelId::Viewport);
        assert!(!tree.contains_panel(&PanelId::Settings));
    }

    #[test]
    fn test_remove_tab_returns_false_for_absent() {
        let mut tree = DockTree::leaf(PanelId::Viewport);
        let result = tree.remove_panel(&PanelId::Settings);
        assert!(!result);
    }

    #[test]
    fn test_add_tab_returns_false_for_absent_target() {
        let mut tree = DockTree::leaf(PanelId::Viewport);
        let result = tree.add_tab(&PanelId::Settings, PanelId::Console);
        assert!(!result);
    }

    #[test]
    fn test_empty_tree_operations() {
        let mut tree = DockTree::Empty;
        assert!(!tree.contains_panel(&PanelId::Viewport));
        assert!(!tree.remove_panel(&PanelId::Viewport));
        assert!(tree.find_leaf(&PanelId::Viewport).is_none());
    }

    #[test]
    fn test_ratio_clamping() {
        let tree = DockTree::horizontal(
            DockTree::leaf(PanelId::Hierarchy),
            DockTree::leaf(PanelId::Viewport),
            0.05, // should be clamped to 0.1
        );
        if let DockTree::Split { ratio, .. } = tree {
            assert!(ratio >= 0.1, "Ratio {} should be >= 0.1", ratio);
        }

        let tree2 = DockTree::horizontal(
            DockTree::leaf(PanelId::Hierarchy),
            DockTree::leaf(PanelId::Viewport),
            0.95, // should be clamped to 0.9
        );
        if let DockTree::Split { ratio, .. } = tree2 {
            assert!(ratio <= 0.9, "Ratio {} should be <= 0.9", ratio);
        }
    }

    #[test]
    fn test_leaf_count() {
        let tree = DockTree::horizontal(
            DockTree::leaf(PanelId::Hierarchy),
            DockTree::vertical(
                DockTree::leaf(PanelId::Viewport),
                DockTree::leaf(PanelId::Console),
                0.5,
            ),
            0.3,
        );
        assert_eq!(tree.leaf_count(), 3);
    }

    #[test]
    fn test_all_panels() {
        let tree = DockTree::horizontal(
            DockTree::leaf(PanelId::Hierarchy),
            DockTree::Leaf {
                tabs: vec![PanelId::Viewport, PanelId::Console],
                active_tab: 0,
            },
            0.3,
        );
        let panels = tree.all_panels();
        assert_eq!(panels.len(), 3);
        assert!(panels.contains(&PanelId::Hierarchy));
        assert!(panels.contains(&PanelId::Viewport));
        assert!(panels.contains(&PanelId::Console));
    }

    #[test]
    fn test_set_active_tab() {
        let mut tree = DockTree::Leaf {
            tabs: vec![PanelId::Viewport, PanelId::Console, PanelId::Assets],
            active_tab: 0,
        };
        tree.set_active_tab(&PanelId::Console);
        if let DockTree::Leaf { active_tab, .. } = tree {
            assert_eq!(active_tab, 1);
        }
    }

    #[test]
    fn test_plugin_panel_id() {
        let panel = PanelId::Plugin("My Plugin".into());
        assert_eq!(panel.title(), "My Plugin");
        assert!(panel.can_close());
    }

    #[test]
    fn test_drop_zone_to_split_params() {
        assert!(DropZone::Tab.to_split_params().is_none());
        assert_eq!(DropZone::Left.to_split_params(), Some((SplitDirection::Horizontal, true)));
        assert_eq!(DropZone::Right.to_split_params(), Some((SplitDirection::Horizontal, false)));
        assert_eq!(DropZone::Top.to_split_params(), Some((SplitDirection::Vertical, true)));
        assert_eq!(DropZone::Bottom.to_split_params(), Some((SplitDirection::Vertical, false)));
    }
}
