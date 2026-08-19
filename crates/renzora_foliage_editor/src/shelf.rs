//! The foliage palette, on the viewport's left-edge tool shelf.
//!
//! Terrain's sculpt and paint brushes got a shelf because a palette buried in a
//! dock panel is a palette nobody finds (see `renzora_terrain_editor::shelf`).
//! Foliage painting had exactly the same problem and worse: the *brush* is only
//! half of it, since which foliage type you are painting is the choice you make
//! most often, and it lived in a list inside the Foliage Painting panel.
//!
//! So the shelf carries two groups. Paint/Erase first, matching the terrain
//! paint group, then one button per foliage type. Both write the same
//! [`FoliagePaintSettings`] the dock panel writes, so the two surfaces stay in
//! sync for free whichever one you click.
//!
//! # Why fixed slots
//!
//! Foliage types are created and deleted at runtime, but a [`ToolEntry`] is
//! `&'static str` all the way down and the shelf is populated once, when the
//! viewport first builds. So all [`MAX_FOLIAGE_TYPES`] slots are registered up
//! front and each hides itself while the config is shorter than its index —
//! which is also why the buttons are numbered rather than iconised: the number
//! matches the row number in the panel's list. The tooltip *does* follow the
//! type's name, via [`sync_type_tooltips`], because that is the one part a
//! per-frame system can reach.

use bevy::prelude::*;

use renzora_editor_framework::{ActiveTool, AppEditorExt, ToolEntry, ToolSection};
use renzora_ember::widgets::HoverTooltip;
use renzora_terrain::foliage::{FoliageBrushType, FoliageConfig, FoliagePaintSettings};

// Shelf groups sort by id string across *every* crate that registers one, so
// these carry the `terrain.` prefix rather than a `foliage.` one: foliage
// painting is one of the terrain modes, and its palette has to stack with the
// sculpt and paint palettes it is a sibling of. A `foliage.` prefix would sort
// ahead of `terrain.` and float the foliage types above them instead.
const BRUSH: ToolSection = ToolSection::Shelf("terrain.d-foliage-brush");
const TYPES: ToolSection = ToolSection::Shelf("terrain.e-foliage-types");

/// Brush modes. Two, so they fill one shelf row exactly.
const BRUSHES: &[(FoliageBrushType, &str, &str)] = &[
    (
        FoliageBrushType::Paint,
        "paint-brush",
        "Paint the active foliage type",
    ),
    (
        FoliageBrushType::Erase,
        "eraser",
        "Erase the active foliage type",
    ),
];

/// Per-slot ids, icons and fallback tooltips. Indexed by foliage type slot, so
/// the array length is checked against [`MAX_FOLIAGE_TYPES`] in tests.
const TYPE_SLOTS: &[(&str, &str, &str)] = &[
    ("foliage.type.0", "number-one", "Foliage type 1"),
    ("foliage.type.1", "number-two", "Foliage type 2"),
    ("foliage.type.2", "number-three", "Foliage type 3"),
    ("foliage.type.3", "number-four", "Foliage type 4"),
    ("foliage.type.4", "number-five", "Foliage type 5"),
    ("foliage.type.5", "number-six", "Foliage type 6"),
    ("foliage.type.6", "number-seven", "Foliage type 7"),
    ("foliage.type.7", "number-eight", "Foliage type 8"),
];

pub fn register(app: &mut App) {
    for (order, (brush, icon, tooltip)) in BRUSHES.iter().enumerate() {
        let brush = *brush;
        app.register_tool(
            ToolEntry::new(brush_id(brush), icon, tooltip, BRUSH)
                .order(order as i32)
                .visible_if(foliage_tool_active)
                .active_if(move |w| {
                    w.get_resource::<FoliagePaintSettings>()
                        .is_some_and(|s| s.brush_type == brush)
                })
                .on_activate(move |w| {
                    if let Some(mut s) = w.get_resource_mut::<FoliagePaintSettings>() {
                        if s.brush_type != brush {
                            s.brush_type = brush;
                        }
                    }
                }),
        );
    }

    for (index, (id, icon, tooltip)) in TYPE_SLOTS.iter().enumerate() {
        app.register_tool(
            ToolEntry::new(id, icon, tooltip, TYPES)
                .order(index as i32)
                // A slot exists only while the config actually has that many
                // types — an empty numbered button would read as a type you
                // could select and then paint nothing with.
                .visible_if(move |w| {
                    foliage_tool_active(w)
                        && w.get_resource::<FoliageConfig>()
                            .is_some_and(|c| index < c.types.len())
                })
                .active_if(move |w| {
                    w.get_resource::<FoliagePaintSettings>()
                        .is_some_and(|s| s.active_type == index)
                })
                .on_activate(move |w| {
                    if let Some(mut s) = w.get_resource_mut::<FoliagePaintSettings>() {
                        if s.active_type != index {
                            s.active_type = index;
                        }
                    }
                }),
        );
    }

    app.add_systems(Update, sync_type_tooltips);
}

fn foliage_tool_active(w: &World) -> bool {
    w.get_resource::<ActiveTool>().copied() == Some(ActiveTool::FoliagePaint)
}

/// `ToolEntry::id` is `&'static str` (a stable key for debug + keybind lookup),
/// so the ids are matched out rather than formatted.
fn brush_id(brush: FoliageBrushType) -> &'static str {
    match brush {
        FoliageBrushType::Paint => "foliage.brush.paint",
        FoliageBrushType::Erase => "foliage.brush.erase",
    }
}

/// Keep each numbered slot's tooltip showing the type's current name.
///
/// The button itself is built once from a `&'static str`, but its
/// [`HoverTooltip`] is a plain `String` component — so renaming "Grass" to
/// "Wildflowers" in the panel can still reach the shelf, which is the only place
/// the number on the button gets its meaning. Runs on config changes only.
fn sync_type_tooltips(
    config: Option<Res<FoliageConfig>>,
    mut buttons: Query<(&Name, &mut HoverTooltip)>,
) {
    let Some(config) = config.filter(|c| c.is_changed()) else {
        return;
    };
    for (name, mut tooltip) in &mut buttons {
        let Some(slot) = name.as_str().strip_prefix("vp-tool:foliage.type.") else {
            continue;
        };
        let Ok(index) = slot.parse::<usize>() else {
            continue;
        };
        let Some((_, _, fallback)) = TYPE_SLOTS.get(index) else {
            continue;
        };
        let want = config
            .types
            .get(index)
            .map(|t| format!("{}. {}", index + 1, t.name))
            .unwrap_or_else(|| (*fallback).to_string());
        if tooltip.0 != want {
            tooltip.0 = want;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_terrain::foliage::MAX_FOLIAGE_TYPES;

    /// One slot per weight the density map carries. Fewer and a type would be
    /// unreachable from the shelf; more and a button could select an index the
    /// paint system silently drops.
    #[test]
    fn one_slot_per_density_map_channel() {
        assert_eq!(TYPE_SLOTS.len(), MAX_FOLIAGE_TYPES);
    }

    /// An unknown Phosphor name doesn't fail — `tool_button` falls back to
    /// rendering the *name itself*, so a typo ships as the literal text
    /// "number-eigth" crammed into a 28px shelf button.
    #[test]
    fn every_icon_name_resolves() {
        use renzora_ember::font::icon_glyph;
        for (_, icon, _) in BRUSHES {
            assert!(icon_glyph(icon).is_some(), "unknown brush icon {icon:?}");
        }
        for (_, icon, _) in TYPE_SLOTS {
            assert!(icon_glyph(icon).is_some(), "unknown type icon {icon:?}");
        }
    }

    #[test]
    fn shelf_ids_are_unique() {
        let mut ids: Vec<&str> = BRUSHES
            .iter()
            .map(|(b, ..)| brush_id(*b))
            .chain(TYPE_SLOTS.iter().map(|(id, ..)| *id))
            .collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total, "duplicate shelf tool id");
    }

    /// The shelf is two buttons wide and a group starts on a fresh row, so an
    /// odd-sized group ends on a visible gap that reads as a missing button.
    #[test]
    fn groups_fill_whole_rows() {
        assert_eq!(BRUSHES.len() % 2, 0);
        assert_eq!(TYPE_SLOTS.len() % 2, 0);
    }

    /// The tooltip sync keys off the button's `Name`, which `tool_button` builds
    /// as `vp-tool:{id}`. If the id scheme drifts the sync goes quietly dead.
    #[test]
    fn type_slot_ids_match_the_parsed_prefix() {
        for (index, (id, ..)) in TYPE_SLOTS.iter().enumerate() {
            assert_eq!(*id, format!("foliage.type.{index}"));
        }
    }
}
