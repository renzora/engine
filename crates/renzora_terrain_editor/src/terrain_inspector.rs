//! Terrain inspector state — the tab enum that drives `ActiveTool`, plus the
//! tool-sync system. The egui inspector body was removed in the bevy_ui
//! migration; the native terrain panel (see `native.rs`) is the live UI.

use bevy::prelude::*;

use renzora_editor_framework::{ActiveTool, EditorSelection};
use renzora_terrain::data::TerrainData;

// ── Tab state ───────────────────────────────────────────────────────────────

#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum TerrainInspectorTab {
    #[default]
    Size,
    Sculpt,
    Paint,
    Foliage,
    /// Grid resizing by clicking ghost tiles in the scene.
    Region,
    /// Procedural generation over a region gizmo in the scene.
    Generate,
    // Retained as part of the tab model; not yet surfaced by the native panel.
    #[allow(dead_code)]
    Heightmap,
}

impl TerrainInspectorTab {
    /// The tab that goes with `tool` — the inverse of [`Self::active_tool`], for
    /// when something outside this module sets the tool and the tab has to
    /// follow rather than fight it.
    ///
    /// Anything that is not a terrain tool lands on `Size`, which is the tab
    /// that drives `Select`: leaving the tab on Sculpt while the user has picked
    /// the move gizmo is what would re-arm sculpting a frame later.
    pub fn for_tool(tool: ActiveTool) -> Self {
        match tool {
            ActiveTool::TerrainSculpt => Self::Sculpt,
            ActiveTool::TerrainPaint => Self::Paint,
            ActiveTool::FoliagePaint => Self::Foliage,
            ActiveTool::TerrainRegion => Self::Region,
            ActiveTool::TerrainGenerate => Self::Generate,
            _ => Self::Size,
        }
    }

    /// Which ActiveTool this tab drives when selected and a terrain is picked.
    pub fn active_tool(&self) -> ActiveTool {
        match self {
            Self::Sculpt => ActiveTool::TerrainSculpt,
            Self::Paint => ActiveTool::TerrainPaint,
            Self::Foliage => ActiveTool::FoliagePaint,
            Self::Region => ActiveTool::TerrainRegion,
            Self::Generate => ActiveTool::TerrainGenerate,
            Self::Size | Self::Heightmap => ActiveTool::Select,
        }
    }
}

// ── ActiveTool <-> tab sync ────────────────────────────────────────────────

/// Sync `ActiveTool` to follow the selected terrain's inspector tab.
/// Resets to `Select` when no terrain is selected.
///
/// **Someone else setting `ActiveTool` to a non-terrain tool parks the tab.**
/// This system re-arms the tab's tool every frame while a terrain is selected,
/// so without that check, pressing Move on the shelf set `ActiveTool::Translate`
/// and this put `TerrainSculpt` straight back on the next frame: with a terrain
/// selected you could not get out of a terrain mode by picking a gizmo, which is
/// the obvious way to try. `activate_terrain_tool`'s toggle-off already had to
/// park the tab by hand for the same reason; this generalises it to any external
/// change rather than to the one path that remembered.
pub fn sync_active_tool_system(
    selection: Res<EditorSelection>,
    tab: Option<ResMut<TerrainInspectorTab>>,
    terrain_query: Query<&TerrainData>,
    mut active: ResMut<ActiveTool>,
) {
    // Somebody else moved `ActiveTool`: let them, and bring the tab into line
    // behind it. The sync runs the other way the rest of the time (tab drives
    // tool), so without this the tab would put its own tool straight back on the
    // next frame — which is what stopped a gizmo press from leaving a terrain
    // mode, and what would stop a foliage brush from arming foliage painting.
    //
    // `is_changed` does not fire on this system's own writes below: the change
    // tick they set is behind the tick this system is compared against on its
    // next run. So this really is "somebody else moved it".
    if active.is_changed() {
        if let Some(mut tab) = tab {
            let want = TerrainInspectorTab::for_tool(*active);
            if *tab != want {
                *tab = want;
            }
        }
        return;
    }

    let terrain_selected = selection
        .get()
        .map(|e| terrain_query.get(e).is_ok())
        .unwrap_or(false);

    let desired = if terrain_selected {
        tab.map(|t| t.active_tool()).unwrap_or(ActiveTool::Select)
    } else {
        // If the user switched away from a terrain while a terrain tool was
        // active, drop back to Select so brush gizmos stop rendering.
        if active.needs_terrain_selection() {
            ActiveTool::Select
        } else {
            *active
        }
    };

    if *active != desired {
        *active = desired;
    }
}
