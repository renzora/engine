//! Which tool is driving the viewport, and the deferred-command queue.
//!
//! These sat in `renzora_editor_framework` while [`ToolbarRegistry`] and
//! [`ToolEntry`] — the things that *set* them — were already here, which left
//! the contract crate describing a button whose effect it could not name.
//! `ToolEntry::on_activate`'s doc even says it "runs as a deferred
//! EditorCommand", naming a type that lived somewhere a plugin could not reach.
//!
//! An editing tool is one of the more obvious plugins to write, and every one of
//! them needs both: [`ActiveTool::None`] to tell the built-in picking and gizmo
//! systems to disengage while it drives the mouse, and [`EditorCommands`] to
//! write to the world from a panel that renders with `&World`.
//!
//! [`ToolbarRegistry`]: super::ToolbarRegistry
//! [`ToolEntry`]: super::ToolEntry

use std::sync::Mutex;

use bevy::prelude::*;

/// Gizmo transform mode — shared so both the gizmo and viewport toolbar can access it.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum GizmoMode {
    /// Select mode — click to select, drag for box/marquee selection.
    #[default]
    Select,
    Translate,
    Rotate,
    Scale,
    /// A plugin tool is driving viewport input. Built-in picking + box
    /// selection skip themselves when they see this.
    None,
}

/// Unified active tool — replaces scattered `GizmoMode`, `TerrainToolState`, `FoliageToolState`.
///
/// Only one tool is active at a time. The viewport toolbar sets this directly.
/// Downstream crates read this to decide whether their systems should run.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ActiveTool {
    #[default]
    Select,
    Translate,
    Rotate,
    Scale,
    TerrainSculpt,
    TerrainPaint,
    FoliagePaint,
    /// Grow/shrink the terrain's chunk grid by clicking ghost tiles in the
    /// scene. Deliberately *not* one of [`ActiveTool::is_terrain`]'s brush
    /// tools: it edits the grid, never the heightmap, so the undo-stroke
    /// systems that record height edits must stay disengaged while it's on.
    TerrainRegion,
    /// Place a rectangle on the terrain and fill it with procedural mountains.
    /// Like [`ActiveTool::TerrainRegion`] it is deliberately *not* one of
    /// [`ActiveTool::is_terrain`]'s brush tools — it commits one edit on a
    /// button press rather than accumulating a stroke, so it records its own
    /// undo entry and must not arm the per-stroke capture.
    TerrainGenerate,
    /// No built-in tool active. Plugins that own their own input mode (mesh
    /// draw, brush tools, etc.) set this so the gizmo + select-click systems
    /// disengage while the plugin is driving.
    None,
}

impl ActiveTool {
    /// Returns the equivalent `GizmoMode` if this is a gizmo tool, `None` for terrain/foliage tools.
    pub fn gizmo_mode(&self) -> Option<GizmoMode> {
        match self {
            Self::Select => Some(GizmoMode::Select),
            Self::Translate => Some(GizmoMode::Translate),
            Self::Rotate => Some(GizmoMode::Rotate),
            Self::Scale => Some(GizmoMode::Scale),
            _ => None,
        }
    }

    /// A terrain *brush* is active — the tools that write heights or splat
    /// weights. Gates the undo-stroke capture, so [`ActiveTool::TerrainRegion`]
    /// is excluded on purpose (it resizes the grid, it doesn't paint).
    pub fn is_terrain(&self) -> bool {
        matches!(self, Self::TerrainSculpt | Self::TerrainPaint)
    }

    /// A brush that paints onto a terrain — heights, splat weights or foliage.
    ///
    /// These are the tools that take over the scroll wheel to size their brush,
    /// so the camera checks this before treating a scroll as dolly-zoom. Keep
    /// [`ActiveTool::TerrainRegion`] out of it: it has no radius to size, and
    /// including it would leave the wheel doing nothing at all while that tool
    /// is active.
    pub fn is_terrain_or_foliage(&self) -> bool {
        matches!(
            self,
            Self::TerrainSculpt | Self::TerrainPaint | Self::FoliagePaint
        )
    }

    /// Any tool that is meaningless without a terrain selected — the brushes
    /// plus the region tool. Used to decide when to fall back to `Select`
    /// because the selection moved off the terrain.
    pub fn needs_terrain_selection(&self) -> bool {
        self.is_terrain_or_foliage()
            || matches!(self, Self::TerrainRegion | Self::TerrainGenerate)
    }
}

/// A queue of deferred world-mutation closures.
///
/// Panels render with `&World` but sometimes need to write (e.g. drag a float →
/// update Transform). They push closures here; `drain_editor_commands_native`
/// drains and executes them each frame.
#[derive(Resource)]
pub struct EditorCommands {
    queue: Mutex<Vec<Box<dyn FnOnce(&mut World) + Send>>>,
}

impl Default for EditorCommands {
    fn default() -> Self {
        Self {
            queue: Mutex::new(Vec::new()),
        }
    }
}

impl EditorCommands {
    /// Push a deferred command to be executed after panel rendering.
    pub fn push(&self, cmd: impl FnOnce(&mut World) + Send + 'static) {
        self.queue.lock().unwrap().push(Box::new(cmd));
    }

    /// Drain all queued commands. Called by `drain_editor_commands_native`.
    pub fn drain(&self) -> Vec<Box<dyn FnOnce(&mut World) + Send>> {
        std::mem::take(&mut *self.queue.lock().unwrap())
    }
}
