//! Viewport buttons for the modeling feature, split across the two surfaces.
//!
//! **Edit Mode** and **X Symmetry** stay in the strip across the viewport's top
//! edge, in `ToolSection::Custom("modeling")`: they say what the viewport is set
//! to do, and they appear whenever a mesh is selected. What Edit and Sculpt
//! *open* goes on the left-edge **shelf** — select modes and ops in Edit, brush
//! pickers in Sculpt.
//!
//! There is deliberately **no Sculpt Mode button**: the viewport's Mode dropdown
//! already lists Scene / Edit / Sculpt, so a second control for the same thing
//! is just a second thing to keep in sync. Edit keeps its button because it is
//! the one you flip constantly and it carries the Tab shortcut.
//!
//! Splitting the surfaces at all is a shape argument: all of these together on
//! the strip is well past what a horizontal row holds before it wraps and shoves
//! Play and the view menus onto a second line. On the shelf they are stacked
//! groups, each with its own rule and each an even number of buttons, so no
//! group ends on a half-empty row. Shelf group sorting is by id string
//! *globally* across every crate that registers one, hence the `a`/`b`/`c`
//! letters — they are load-bearing, not decoration, and `modeling.a-draw` (box
//! and polyline, from `renzora_mesh_draw`) sorts in ahead of these.
//!
//! Buttons reuse the same funnels as the keyboard: mode writes go to
//! `ViewportSettings`, ops go through [`PendingOps`], loop cut arms the same
//! modal the Ctrl+R shortcut does.

use bevy::prelude::*;
use renzora::core::viewport_types::{ViewportMode, ViewportSettings, ViewportView};
use renzora_editor_framework::{AppEditorExt, ToolEntry, ToolSection};

use crate::sculpt::{BrushKind, SculptBrush};
use crate::selection::{MeshSelection, SelectMode};
use crate::tools::{LoopCutState, ModelingOp, ModelingSettings, PendingOps};

/// Edit Mode and X-symmetry: the switches that decide what the shelf below
/// shows, so they stay on the top strip. Symmetry rides with them because it
/// applies in Edit *and* Sculpt — on the shelf it would need a rule and a row
/// all to itself in both.
const MODE: ToolSection = ToolSection::Custom("modeling");
/// Vertex / Edge / Face, plus loop cut — Edit mode only.
///
/// Loop cut sits with the select modes rather than with the ops below because it
/// is *modal* like they are: it arms, previews, and reads as active until you
/// commit or cancel, where the four ops fire the moment you click. It also makes
/// both groups four buttons — two clean 2×2 blocks instead of a 3 and a 5, each
/// of which would end on a row with one button and a gap.
const SELECT: ToolSection = ToolSection::Shelf("modeling.b-select");
/// The one-shot mesh ops — Edit mode only.
const OPS: ToolSection = ToolSection::Shelf("modeling.c-ops");
/// The sculpt brush palette — Sculpt mode only.
const BRUSHES: ToolSection = ToolSection::Shelf("modeling.d-sculpt");

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
    // Edit Mode. Sculpt has no button — see the module docs.
    app.register_tool(
        ToolEntry::new("modeling.edit_mode", "cube", "Edit Mode (Tab)", MODE)
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
            ToolEntry::new(id, icon, tooltip, SELECT)
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

    // X-symmetry switch (Edit + Sculpt) — rides with the mode toggles, being
    // the one switch that means something in both.
    app.register_tool(
        ToolEntry::new(
            "modeling.symmetry_x",
            "arrows-left-right",
            "X Symmetry",
            MODE,
        )
        .order(1)
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

    // Loop cut closes the select group: it arms the same modal as Ctrl+R, so
    // unlike the one-shot ops below it it can read as active.
    app.register_tool(
        ToolEntry::new("modeling.loop_cut", "knife", "Loop Cut (Ctrl+R)", SELECT)
            .order(13)
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
            ToolEntry::new(id, icon, tooltip, OPS)
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
            ToolEntry::new(id, icon, tooltip, BRUSHES)
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
