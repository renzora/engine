//! Terrain inspector state: the four modes of the Terrain component (Manage,
//! Sculpt, Paint, Foliage), which tool each one arms, and the system that keeps
//! `ActiveTool` and the mode in step.
//!
//! Two resources, because "which mode is armed" and "which mode is on screen"
//! are different questions. [`TerrainInspectorTab`] is the armed one: it drives
//! `ActiveTool` every frame while a terrain is selected. [`ShownTerrainTab`] is
//! the pane the component shows. Pressing the move gizmo with a terrain selected
//! disarms the brush (the armed tab parks on Manage, which drives Select) but
//! must not yank the panel off the Sculpt tools the user was looking at, so
//! only a mode tab click or a terrain brush arming moves the shown pane.

use bevy::prelude::*;

use renzora_editor_framework::{ActiveTool, EditorSelection};
use renzora_terrain::data::TerrainData;

// ── Modes ───────────────────────────────────────────────────────────────────

/// The Terrain component's modes, in the order the mode tabs show them.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum TerrainInspectorTab {
    /// The whole terrain: chunk map, size, streaming, generator, heightmaps.
    /// Arms Select, since nothing here is a brush.
    #[default]
    Manage,
    Sculpt,
    Paint,
    Foliage,
}

impl TerrainInspectorTab {
    pub const ALL: [TerrainInspectorTab; 4] = [Self::Manage, Self::Sculpt, Self::Paint, Self::Foliage];

    pub fn label(self) -> &'static str {
        match self {
            Self::Manage => "Manage",
            Self::Sculpt => "Sculpt",
            Self::Paint => "Paint",
            Self::Foliage => "Foliage",
        }
    }

    /// The tab that goes with `tool`: the inverse of [`Self::active_tool`], for
    /// when something outside this module sets the tool and the tab has to
    /// follow rather than fight it.
    ///
    /// Anything that is not a terrain brush lands on `Manage`, which is the tab
    /// that drives `Select`: leaving the tab on Sculpt while the user has picked
    /// the move gizmo is what would re-arm sculpting a frame later.
    pub fn for_tool(tool: ActiveTool) -> Self {
        match tool {
            ActiveTool::TerrainSculpt => Self::Sculpt,
            ActiveTool::TerrainPaint => Self::Paint,
            ActiveTool::FoliagePaint => Self::Foliage,
            _ => Self::Manage,
        }
    }

    /// Which `ActiveTool` this tab drives while it is armed and a terrain is
    /// selected.
    pub fn active_tool(&self) -> ActiveTool {
        match self {
            Self::Sculpt => ActiveTool::TerrainSculpt,
            Self::Paint => ActiveTool::TerrainPaint,
            Self::Foliage => ActiveTool::FoliagePaint,
            Self::Manage => ActiveTool::Select,
        }
    }
}

/// The mode pane the Terrain component shows. See the module docs for why it
/// is not the same resource as [`TerrainInspectorTab`].
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct ShownTerrainTab(pub TerrainInspectorTab);

/// The pane to show once `tool` is armed, or `None` to leave the pane alone.
///
/// Only the terrain brushes move the pane. A gizmo or Select arming keeps
/// whatever the user was looking at, so dropping out of a brush to nudge the
/// terrain with the move gizmo does not lose their place in the Sculpt tools.
pub fn shown_tab_for_tool(tool: ActiveTool) -> Option<TerrainInspectorTab> {
    match tool {
        ActiveTool::TerrainSculpt | ActiveTool::TerrainPaint | ActiveTool::FoliagePaint => {
            Some(TerrainInspectorTab::for_tool(tool))
        }
        _ => None,
    }
}

// ── ActiveTool <-> tab sync ────────────────────────────────────────────────

/// Sync `ActiveTool` to follow the selected terrain's armed tab. Resets to
/// `Select` when no terrain is selected.
///
/// **Someone else setting `ActiveTool` to a non-terrain tool parks the tab.**
/// This system re-arms the tab's tool every frame while a terrain is selected,
/// so without that check, pressing Move on the shelf set `ActiveTool::Translate`
/// and this put `TerrainSculpt` straight back on the next frame: with a terrain
/// selected you could not get out of a terrain mode by picking a gizmo, which is
/// the obvious way to try.
pub fn sync_active_tool_system(
    selection: Res<EditorSelection>,
    tab: Option<ResMut<TerrainInspectorTab>>,
    terrain_query: Query<&TerrainData>,
    mut active: ResMut<ActiveTool>,
) {
    // Somebody else moved `ActiveTool`: let them, and bring the tab into line
    // behind it. The sync runs the other way the rest of the time (tab drives
    // tool), so without this the tab would put its own tool straight back on the
    // next frame.
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

/// Bring the shown pane to the mode a terrain brush just armed, from wherever
/// it was armed (a brush button, a mode tab, Make Terrain, a script).
pub fn follow_armed_tool_system(active: Option<Res<ActiveTool>>, mut shown: ResMut<ShownTerrainTab>) {
    let Some(active) = active.filter(|a| a.is_changed()) else { return };
    if let Some(want) = shown_tab_for_tool(*active) {
        if shown.0 != want {
            shown.0 = want;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every mode arms a tool, and that tool maps straight back to the mode:
    /// a round trip that drifted would flip the tab every other frame.
    #[test]
    fn every_tab_round_trips_through_its_tool() {
        for tab in TerrainInspectorTab::ALL {
            assert_eq!(TerrainInspectorTab::for_tool(tab.active_tool()), tab, "{tab:?}");
        }
    }

    #[test]
    fn manage_arms_select_and_the_brush_tabs_arm_their_brush() {
        assert_eq!(TerrainInspectorTab::Manage.active_tool(), ActiveTool::Select);
        assert_eq!(TerrainInspectorTab::Sculpt.active_tool(), ActiveTool::TerrainSculpt);
        assert_eq!(TerrainInspectorTab::Paint.active_tool(), ActiveTool::TerrainPaint);
        assert_eq!(TerrainInspectorTab::Foliage.active_tool(), ActiveTool::FoliagePaint);
    }

    /// A gizmo arming parks the armed tab on Manage but leaves the shown pane
    /// where it was; a brush arming moves the pane to its mode.
    #[test]
    fn only_a_brush_moves_the_shown_pane() {
        assert_eq!(TerrainInspectorTab::for_tool(ActiveTool::Translate), TerrainInspectorTab::Manage);
        assert_eq!(shown_tab_for_tool(ActiveTool::Translate), None);
        assert_eq!(shown_tab_for_tool(ActiveTool::Select), None);
        assert_eq!(shown_tab_for_tool(ActiveTool::TerrainSculpt), Some(TerrainInspectorTab::Sculpt));
        assert_eq!(shown_tab_for_tool(ActiveTool::TerrainPaint), Some(TerrainInspectorTab::Paint));
        assert_eq!(shown_tab_for_tool(ActiveTool::FoliagePaint), Some(TerrainInspectorTab::Foliage));
    }

    /// The tools the component no longer has a mode for (the old Generate and
    /// Resize scene tools) land on Manage, never on a brush.
    #[test]
    fn retired_whole_terrain_tools_land_on_manage() {
        assert_eq!(TerrainInspectorTab::for_tool(ActiveTool::TerrainGenerate), TerrainInspectorTab::Manage);
        assert_eq!(TerrainInspectorTab::for_tool(ActiveTool::TerrainRegion), TerrainInspectorTab::Manage);
    }
}
