//! The terrain brush palette, on the viewport's left-edge tool shelf.
//!
//! The brushes were always there — seventeen sculpt brushes and four paint
//! brushes, all implemented in `renzora_terrain::sculpt` / `::paint`. What they
//! lacked was a place to *be*. Their only home was the Terrain Tools dock panel,
//! which you reach through the dock's panel picker, so with that panel undocked
//! a terrain read as having no tools at all.
//!
//! Here they sit permanently beside the viewport the moment a terrain tool is
//! on, two to a row, the way an image editor puts its brushes. The dock panel
//! keeps working and stays in sync for free: both surfaces write the same
//! `TerrainSettings.brush_type`, so whichever you click, the other follows.
//!
//! Only the *choice* of brush lives here. Its settings — size, strength,
//! falloff, and whatever that particular brush adds — go in the viewport toolbar
//! (see [`crate::brush_bar`]), because a palette you have to scroll past to
//! reach a slider is a worse palette.

use bevy::prelude::*;

use renzora_editor_framework::{ActiveTool, AppEditorExt, ToolEntry, ToolSection};
use renzora_terrain::data::{TerrainBrushType, TerrainSettings};
use renzora_terrain::paint::{PaintBrushType, SurfacePaintSettings};

/// The terrain toolset's shelf groups, in the order they stack down the shelf.
///
/// Shelf groups sort by their id string *globally*, across every crate that
/// registers one, so the leading `terrain.` and the `a`/`b`/`c` letters are what
/// fix their order. `renzora_foliage_editor` continues the same sequence with
/// `terrain.d-…` / `terrain.e-…`: it is a separate crate, but foliage painting
/// is one of the terrain modes and its palette belongs with the others.
///
/// The mode buttons that turn the palettes on (Sculpt / Paint / Foliage) are
/// *not* here — they stay in the viewport's top strip, in
/// [`ToolSection::Terrain`], so there is always one visible row saying which
/// mode is on above the palette it opens.
///
/// [`REGION`] is the exception, and the reason it is: Resize Terrain opens no
/// palette, so on the strip it was a mode button with nothing under it. Here it
/// sits with the terrain's *extent* controls, which is what it actually is —
/// paired with the numeric size / resolution editor that reaches the same
/// settings by typing instead of by clicking ghost tiles.
///
/// Like every other group here it shows only once a terrain tool is in hand, not
/// merely because a terrain exists somewhere in the scene. The shelf is what a
/// mode opens; a group that appeared before you picked one would sit over the
/// viewport in scenes you were not editing terrain in at all. Resize is still
/// reachable in one hop — click any terrain mode on the top strip and the whole
/// column, this group included, comes up together.
const REGION: ToolSection = ToolSection::Shelf("terrain.a-region");
const SCULPT: ToolSection = ToolSection::Shelf("terrain.b-sculpt");
const PAINT: ToolSection = ToolSection::Shelf("terrain.c-paint");

/// Sculpt brushes, in shelf order, with the icon and tooltip each shows.
///
/// The order is the one the old panel's grid used: the brushes you reach for
/// constantly first (Sculpt, Smooth, Flatten), the shaping ones next, and the
/// specialised or slow ones (Erosion, Hydraulic, Retop) last. Icons are carried
/// over from that grid so the palette stays recognisable.
const SCULPT_BRUSHES: &[(TerrainBrushType, &str, &str)] = &[
    (TerrainBrushType::Sculpt, "mountains", "Sculpt — raise, or lower with Shift"),
    (TerrainBrushType::Raise, "arrows-out-cardinal", "Raise"),
    (TerrainBrushType::Lower, "arrow-fat-line-down", "Lower"),
    (TerrainBrushType::Smooth, "waves", "Smooth"),
    (TerrainBrushType::Flatten, "equals", "Flatten to a target height"),
    (TerrainBrushType::SetHeight, "arrow-fat-line-up", "Set Height"),
    (TerrainBrushType::Erase, "eraser", "Erase back to the base level"),
    (TerrainBrushType::Noise, "waveform", "Noise"),
    (TerrainBrushType::Stamp, "stamp", "Stamp a heightmap image"),
    (TerrainBrushType::Terrace, "stairs", "Terrace"),
    (TerrainBrushType::Cliff, "triangle", "Cliff"),
    (TerrainBrushType::Pinch, "arrows-in-cardinal", "Pinch"),
    (TerrainBrushType::Relax, "activity", "Relax"),
    (TerrainBrushType::Ramp, "flow-arrow", "Ramp"),
    (TerrainBrushType::Erosion, "wind", "Erosion — thermal weathering"),
    (TerrainBrushType::Hydro, "drop", "Hydraulic erosion"),
    (TerrainBrushType::Retop, "grid-four", "Retopologise"),
];

/// Paint brushes. Four, so they fill two rows exactly.
const PAINT_BRUSHES: &[(PaintBrushType, &str, &str)] = &[
    (PaintBrushType::Paint, "paint-brush", "Paint the active layer"),
    (PaintBrushType::Erase, "eraser", "Erase the active layer"),
    (PaintBrushType::Smooth, "waves", "Smooth layer edges"),
    (PaintBrushType::Fill, "paint-bucket", "Fill"),
];

pub fn register(app: &mut App) {
    // Resize by dragging the grid out, or by typing the numbers — same terrain
    // extent, two ways at it.
    app.register_tool(
        ToolEntry::new(
            "builtin.terrain_region",
            "selection-plus",
            "Resize Terrain — click a ghost tile to add, Ctrl+click an edge to remove",
            REGION,
        )
        .order(0)
        .visible_if(any_terrain_tool)
        .active_if(|w| tool_is(w, ActiveTool::TerrainRegion))
        .on_activate(|w| {
            crate::activate_terrain_tool(
                w,
                crate::TerrainInspectorTab::Region,
                ActiveTool::TerrainRegion,
            )
        }),
    );
    app.register_tool(
        ToolEntry::new(
            "terrain.settings",
            "gear",
            "Terrain Size & Resolution — grid size, chunk resolution, height range",
            REGION,
        )
        .order(1)
        .visible_if(any_terrain_tool)
        // An overlay you open, not a mode you are in, so it never highlights.
        .active_if(|_| false)
        .on_activate(|w| {
            // Same entry point as the inspector's "Edit Terrain…" button; the
            // overlay is a deferred-apply draft, so opening it twice is harmless.
            if let Some(entity) = crate::first_terrain_entity(w) {
                crate::settings_overlay::open(w, entity);
            }
        }),
    );

    for (order, (brush, icon, tooltip)) in SCULPT_BRUSHES.iter().enumerate() {
        let brush = *brush;
        app.register_tool(
            ToolEntry::new(brush_id(brush), icon, tooltip, SCULPT)
                .order(order as i32)
                // The sculpt palette is only meaningful while the sculpt tool is
                // the active one; in Paint mode the paint group takes its place.
                .visible_if(|w| tool_is(w, ActiveTool::TerrainSculpt))
                .active_if(move |w| {
                    w.get_resource::<TerrainSettings>()
                        .is_some_and(|s| s.brush_type == brush)
                })
                .on_activate(move |w| {
                    if let Some(mut s) = w.get_resource_mut::<TerrainSettings>() {
                        if s.brush_type != brush {
                            s.brush_type = brush;
                        }
                    }
                }),
        );
    }

    for (order, (brush, icon, tooltip)) in PAINT_BRUSHES.iter().enumerate() {
        let brush = *brush;
        app.register_tool(
            ToolEntry::new(paint_id(brush), icon, tooltip, PAINT)
                .order(order as i32)
                .visible_if(|w| tool_is(w, ActiveTool::TerrainPaint))
                .active_if(move |w| {
                    w.get_resource::<SurfacePaintSettings>()
                        .is_some_and(|s| s.brush_type == brush)
                })
                .on_activate(move |w| {
                    if let Some(mut s) = w.get_resource_mut::<SurfacePaintSettings>() {
                        if s.brush_type != brush {
                            s.brush_type = brush;
                        }
                    }
                }),
        );
    }
}

fn tool_is(w: &World, want: ActiveTool) -> bool {
    w.get_resource::<ActiveTool>().copied() == Some(want)
}

/// True while *any* terrain tool is the active one.
///
/// The sculpt and paint groups each key off their own tool, because each is that
/// tool's palette. The region group is not a palette — it belongs to the terrain
/// as a whole — so it rides along with all four, and stays put while you switch
/// between them instead of blinking out and back.
fn any_terrain_tool(w: &World) -> bool {
    matches!(
        w.get_resource::<ActiveTool>().copied(),
        Some(
            ActiveTool::TerrainSculpt
                | ActiveTool::TerrainPaint
                | ActiveTool::FoliagePaint
                | ActiveTool::TerrainRegion
        )
    )
}

/// `ToolEntry::id` is `&'static str` (it's a stable key for debug + keybind
/// lookup), so the ids are matched out rather than formatted.
fn brush_id(brush: TerrainBrushType) -> &'static str {
    match brush {
        TerrainBrushType::Raise => "terrain.brush.raise",
        TerrainBrushType::Lower => "terrain.brush.lower",
        TerrainBrushType::Smooth => "terrain.brush.smooth",
        TerrainBrushType::Flatten => "terrain.brush.flatten",
        TerrainBrushType::SetHeight => "terrain.brush.set_height",
        TerrainBrushType::Sculpt => "terrain.brush.sculpt",
        TerrainBrushType::Erase => "terrain.brush.erase",
        TerrainBrushType::Ramp => "terrain.brush.ramp",
        TerrainBrushType::Erosion => "terrain.brush.erosion",
        TerrainBrushType::Hydro => "terrain.brush.hydro",
        TerrainBrushType::Noise => "terrain.brush.noise",
        TerrainBrushType::Retop => "terrain.brush.retop",
        TerrainBrushType::Terrace => "terrain.brush.terrace",
        TerrainBrushType::Pinch => "terrain.brush.pinch",
        TerrainBrushType::Relax => "terrain.brush.relax",
        TerrainBrushType::Cliff => "terrain.brush.cliff",
        TerrainBrushType::Stamp => "terrain.brush.stamp",
    }
}

fn paint_id(brush: PaintBrushType) -> &'static str {
    match brush {
        PaintBrushType::Paint => "terrain.paint.paint",
        PaintBrushType::Erase => "terrain.paint.erase",
        PaintBrushType::Smooth => "terrain.paint.smooth",
        PaintBrushType::Fill => "terrain.paint.fill",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every brush the engine implements must be reachable from the palette —
    /// the whole point of the shelf is that nothing is hidden. `all()` is the
    /// canonical list, so compare against it rather than a hand-kept count.
    #[test]
    fn every_sculpt_brush_is_on_the_shelf() {
        for brush in TerrainBrushType::all() {
            assert!(
                SCULPT_BRUSHES.iter().any(|(b, ..)| b == brush),
                "{brush:?} has no shelf entry"
            );
        }
        assert_eq!(SCULPT_BRUSHES.len(), TerrainBrushType::all().len());
    }

    /// An unknown Phosphor name doesn't fail — `tool_button` falls back to
    /// rendering the *name itself*, so a typo ships as the literal text
    /// "arrows-out-cardinl" crammed into a 28px shelf button.
    #[test]
    fn every_icon_name_resolves() {
        use renzora_ember::font::icon_glyph;
        for (_, icon, _) in SCULPT_BRUSHES {
            assert!(icon_glyph(icon).is_some(), "unknown sculpt icon {icon:?}");
        }
        for (_, icon, _) in PAINT_BRUSHES {
            assert!(icon_glyph(icon).is_some(), "unknown paint icon {icon:?}");
        }
        // The region group is two hand-written registrations rather than a
        // table, so its icons are listed again here — the point of the test is
        // that a typo ships as the literal name crammed into a 28px button.
        for icon in ["selection-plus", "gear"] {
            assert!(icon_glyph(icon).is_some(), "unknown region icon {icon:?}");
        }
    }

    #[test]
    fn shelf_ids_are_unique() {
        let mut ids: Vec<&str> = SCULPT_BRUSHES
            .iter()
            .map(|(b, ..)| brush_id(*b))
            .chain(PAINT_BRUSHES.iter().map(|(b, ..)| paint_id(*b)))
            .collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total, "duplicate shelf tool id");
    }

    /// The shelf is two buttons wide, and groups start on a fresh row. A group
    /// with an odd count leaves a visible gap at its end, which reads as a
    /// missing button rather than as the end of a group.
    #[test]
    fn paint_group_fills_whole_rows() {
        assert_eq!(PAINT_BRUSHES.len() % 2, 0);
    }

    /// The shelf stacks its groups in **alphabetical order of the group id**, so
    /// the `a-`/`b-`/`c-` letters are the only thing fixing region above sculpt
    /// above paint above foliage. A well-meant rename to something more
    /// descriptive would silently reorder the palette.
    #[test]
    fn group_ids_sort_into_shelf_order() {
        let ids: Vec<&str> = [REGION, SCULPT, PAINT]
            .iter()
            .map(|s| match s {
                ToolSection::Shelf(id) => *id,
                _ => panic!("terrain groups must be shelf sections"),
            })
            .collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted, "terrain shelf groups are out of order");
        // `renzora_foliage_editor` continues the sequence with `terrain.d-…`.
        assert!(*ids.last().unwrap() < "terrain.d-foliage-brush");
    }

}
