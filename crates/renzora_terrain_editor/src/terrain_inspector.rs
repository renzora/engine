//! Terrain inspector state — the tab enum that drives `ActiveTool`, plus the
//! tool-sync system. The egui inspector body was removed in the bevy_ui
//! migration; the native terrain panel (see `native.rs`) is the live UI.

use bevy::prelude::*;

use renzora_editor::{ActiveTool, EditorSelection};
use renzora_terrain::data::TerrainData;

// ── Tab state ───────────────────────────────────────────────────────────────

#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum TerrainInspectorTab {
    #[default]
    Size,
    Sculpt,
    Paint,
    Foliage,
    // Retained as part of the tab model; not yet surfaced by the native panel.
    #[allow(dead_code)]
    Heightmap,
}

impl TerrainInspectorTab {
    /// Which ActiveTool this tab drives when selected and a terrain is picked.
    pub fn active_tool(&self) -> ActiveTool {
        match self {
            Self::Sculpt => ActiveTool::TerrainSculpt,
            Self::Paint => ActiveTool::TerrainPaint,
            Self::Foliage => ActiveTool::FoliagePaint,
            Self::Size | Self::Heightmap => ActiveTool::Select,
        }
    }
}

// ── ActiveTool <-> tab sync ────────────────────────────────────────────────

/// Sync `ActiveTool` to follow the selected terrain's inspector tab.
/// Resets to `Select` when no terrain is selected.
pub fn sync_active_tool_system(
    selection: Res<EditorSelection>,
    tab: Option<Res<TerrainInspectorTab>>,
    terrain_query: Query<&TerrainData>,
    mut active: ResMut<ActiveTool>,
) {
    let terrain_selected = selection
        .get()
        .map(|e| terrain_query.get(e).is_ok())
        .unwrap_or(false);

    let desired = if terrain_selected {
        tab.map(|t| t.active_tool()).unwrap_or(ActiveTool::Select)
    } else {
        // If the user switched away from a terrain while a terrain tool was
        // active, drop back to Select so brush gizmos stop rendering.
        if active.is_terrain() || *active == ActiveTool::FoliagePaint {
            ActiveTool::Select
        } else {
            *active
        }
    };

    if *active != desired {
        *active = desired;
    }
}
