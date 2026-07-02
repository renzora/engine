//! Viewport-toolbar buttons for the modeling feature.
//!
//! Registered in a `ToolSection::Custom("modeling")` section that the header
//! renders after the built-in sections. Two always-relevant mode toggles
//! (Edit / Sculpt) appear whenever a mesh is selected; the rest are
//! context-sensitive — select-mode switches and op buttons in Edit mode,
//! brush pickers in Sculpt mode. Buttons reuse the same funnels as the
//! keyboard: mode writes go to `ViewportSettings`, ops go through
//! [`PendingOps`], loop cut arms the same modal the Ctrl+R shortcut does.

use bevy::prelude::*;
use renzora::core::viewport_types::{ViewportMode, ViewportSettings, ViewportView};
use renzora_editor_framework::{AppEditorExt, ToolEntry, ToolSection};

use crate::sculpt::{BrushKind, SculptBrush};
use crate::selection::{MeshSelection, SelectMode};
use crate::tools::{LoopCutState, ModelingOp, ModelingSettings, PendingOps};

const SECTION: ToolSection = ToolSection::Custom("modeling");

// ── Predicates ─────────────────────────────────────────────────────────────

fn mode(w: &World) -> ViewportMode {
    w.get_resource::<ViewportSettings>()
        .map(|s| s.viewport_mode)
        .unwrap_or(ViewportMode::Scene)
}

fn in_edit(w: &World) -> bool {
    mode(w) == ViewportMode::Edit
}

fn in_sculpt(w: &World) -> bool {
    mode(w) == ViewportMode::Sculpt
}

fn in_edit_or_sculpt(w: &World) -> bool {
    matches!(mode(w), ViewportMode::Edit | ViewportMode::Sculpt)
}

/// The mode toggles show when modeling is relevant: 3D view, not playing,
/// and either already in a modeling mode or a mesh entity is selected.
fn modeling_context(w: &World) -> bool {
    let three_d = w
        .get_resource::<ViewportSettings>()
        .map(|s| s.viewport_view == ViewportView::Three)
        .unwrap_or(false);
    if !three_d {
        return false;
    }
    if w.get_resource::<renzora::PlayModeState>()
        .is_some_and(|p| p.is_in_play_mode())
    {
        return false;
    }
    in_edit_or_sculpt(w)
        || w.get_resource::<renzora::EditorSelection>()
            .and_then(|s| s.get())
            .is_some_and(|e| w.get::<Mesh3d>(e).is_some())
}

// ── Activators ─────────────────────────────────────────────────────────────

fn set_mode(world: &mut World, m: ViewportMode) {
    if let Some(mut s) = world.get_resource_mut::<ViewportSettings>() {
        s.viewport_mode = m;
    }
}

fn push_op(world: &mut World, op: ModelingOp) {
    if let Some(mut p) = world.get_resource_mut::<PendingOps>() {
        p.0.push(op);
    }
}

// ── Registration ───────────────────────────────────────────────────────────

pub fn register(app: &mut App) {
    // Mode toggles.
    app.register_tool(
        ToolEntry::new("modeling.edit_mode", "cube", "Edit Mode (Tab)", SECTION)
            .order(0)
            .visible_if(modeling_context)
            .active_if(in_edit)
            .on_activate(|w| {
                let next = if in_edit(w) {
                    ViewportMode::Scene
                } else {
                    ViewportMode::Edit
                };
                set_mode(w, next);
            }),
    );
    app.register_tool(
        ToolEntry::new("modeling.sculpt_mode", "hand", "Sculpt Mode", SECTION)
            .order(1)
            .visible_if(modeling_context)
            .active_if(in_sculpt)
            .on_activate(|w| {
                let next = if in_sculpt(w) {
                    ViewportMode::Scene
                } else {
                    ViewportMode::Sculpt
                };
                set_mode(w, next);
            }),
    );

    // Select-mode switches (Edit mode).
    for (id, icon, tooltip, order, sel_mode) in [
        (
            "modeling.select_vertex",
            "dot-outline",
            "Vertex Select (1)",
            10,
            SelectMode::Vertex,
        ),
        (
            "modeling.select_edge",
            "line-segment",
            "Edge Select (2)",
            11,
            SelectMode::Edge,
        ),
        (
            "modeling.select_face",
            "square",
            "Face Select (3)",
            12,
            SelectMode::Face,
        ),
    ] {
        app.register_tool(
            ToolEntry::new(id, icon, tooltip, SECTION)
                .order(order)
                .visible_if(in_edit)
                .active_if(move |w| {
                    w.get_resource::<MeshSelection>()
                        .map(|s| s.mode == sel_mode)
                        .unwrap_or(false)
                })
                .on_activate(move |w| crate::systems::set_select_mode(w, sel_mode)),
        );
    }

    // X-symmetry switch (Edit + Sculpt).
    app.register_tool(
        ToolEntry::new(
            "modeling.symmetry_x",
            "arrows-left-right",
            "X Symmetry",
            SECTION,
        )
        .order(20)
        .visible_if(in_edit_or_sculpt)
        .active_if(|w| {
            w.get_resource::<ModelingSettings>()
                .map(|s| s.symmetry_x)
                .unwrap_or(false)
        })
        .on_activate(|w| {
            if let Some(mut s) = w.get_resource_mut::<ModelingSettings>() {
                s.symmetry_x = !s.symmetry_x;
            }
        }),
    );

    // Loop cut arms the same modal as Ctrl+R.
    app.register_tool(
        ToolEntry::new("modeling.loop_cut", "knife", "Loop Cut (Ctrl+R)", SECTION)
            .order(21)
            .visible_if(in_edit)
            .active_if(|w| {
                matches!(
                    w.get_resource::<LoopCutState>(),
                    Some(LoopCutState::Preview { .. })
                )
            })
            .on_activate(|w| {
                if let Some(mut s) = w.get_resource_mut::<LoopCutState>() {
                    *s = match *s {
                        LoopCutState::Preview { .. } => LoopCutState::Idle,
                        _ => LoopCutState::Preview {
                            edge: None,
                            cuts: 1,
                        },
                    };
                }
            }),
    );

    // One-shot ops (Edit mode) — same PendingOps funnel as the panel/keys.
    for (id, icon, tooltip, order, op) in [
        (
            "modeling.subdivide",
            "squares-four",
            "Subdivide Selected Faces",
            22,
            ModelingOp::Subdivide,
        ),
        (
            "modeling.inset",
            "arrows-in-simple",
            "Inset Faces (I)",
            23,
            ModelingOp::Inset,
        ),
        (
            "modeling.merge",
            "arrows-merge",
            "Merge at Center (M)",
            24,
            ModelingOp::MergeAtCenter,
        ),
        (
            "modeling.delete",
            "trash",
            "Delete Selected (X)",
            25,
            ModelingOp::Delete,
        ),
    ] {
        app.register_tool(
            ToolEntry::new(id, icon, tooltip, SECTION)
                .order(order)
                .visible_if(in_edit)
                .on_activate(move |w| push_op(w, op)),
        );
    }

    // Sculpt brushes.
    for (id, icon, tooltip, order, kind) in [
        (
            "modeling.brush_draw",
            "pencil",
            "Draw Brush",
            30,
            BrushKind::Draw,
        ),
        (
            "modeling.brush_smooth",
            "drop",
            "Smooth Brush (Shift)",
            31,
            BrushKind::Smooth,
        ),
        (
            "modeling.brush_grab",
            "hand-grabbing",
            "Grab Brush",
            32,
            BrushKind::Grab,
        ),
        (
            "modeling.brush_inflate",
            "circle-dashed",
            "Inflate Brush",
            33,
            BrushKind::Inflate,
        ),
        (
            "modeling.brush_flatten",
            "stack",
            "Flatten Brush",
            34,
            BrushKind::Flatten,
        ),
        (
            "modeling.brush_pinch",
            "magnet",
            "Pinch Brush",
            35,
            BrushKind::Pinch,
        ),
    ] {
        app.register_tool(
            ToolEntry::new(id, icon, tooltip, SECTION)
                .order(order)
                .visible_if(in_sculpt)
                .active_if(move |w| {
                    w.get_resource::<SculptBrush>()
                        .map(|b| b.kind == kind)
                        .unwrap_or(false)
                })
                .on_activate(move |w| {
                    if let Some(mut b) = w.get_resource_mut::<SculptBrush>() {
                        b.kind = kind;
                    }
                }),
        );
    }
}
