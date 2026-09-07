//! The tool shelf — a single column of tool buttons down the viewport's left
//! edge, in the shape image editors have settled on.
//!
//! **Everything you can set the viewport to do is here, in one list.** The
//! gizmos (select / move / rotate / scale) lead it, the terrain modes
//! (sculpt / paint / foliage) follow when the scene has a terrain, and the
//! palettes a mode opens — its brushes, its select modes, its ops — appear
//! underneath it. Reading top to bottom you get the tools you always have, then
//! the modes available here, then what the current mode gave you.
//!
//! It used to be split across two surfaces: mode buttons on the strip across the
//! viewport's top edge, and what they revealed down here. The argument was that
//! there should always be one visible row saying what the viewport is set to do.
//! In practice it meant picking a tool was two different gestures in two places
//! depending on which tool, and the strip's modes sat among the view controls and
//! the snapping fields, which are not tools at all. One list is the simpler
//! answer to the same question.
//!
//! **One column**, now that it is short. It was two while the terrain brush
//! palettes were here — seventeen sculpt brushes in a single column runs past
//! the bottom of a short viewport — but those are the Terrain component's, and
//! what is left is a list rather than a grid. The left edge is genuinely free
//! for it: the nav cluster, the axis gizmo and the height ruler are on the
//! right.
//!
//! Entries come from the [`ToolbarRegistry`], tagged [`ToolSection::Shelf`].
//! Nothing here is feature-specific: groups render top to bottom in alphabetical
//! order of their group string, which is a *global* sort across every crate that
//! registers one — see [`ToolSection::Shelf`] for how a multi-group feature
//! (terrain, whose foliage groups come from a different crate) pins its own
//! order. The buttons, their show/hide/highlight driver and their click handler
//! are shared with the top strip; see [`crate::tool_buttons`].

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use renzora_editor_framework::{ToolSection, ToolbarRegistry};
use renzora_ember::font::EmberFonts;
use renzora_ember::theme::{panel_bg, rgb};
use renzora_ember::widgets::OverlaySurface;

use crate::tool_buttons::{
    shelf_separator, tool_button, ShelfRoot, ToolSepVis, ToolsPopulated, SIDE_BTN,
};

/// Gap between buttons, in logical px.
const GAP: f32 = 2.0;
/// Padding inside the shelf's panel.
const PAD: f32 = 3.0;
/// One button wide, plus the padding either side.
const SHELF_W: f32 = SIDE_BTN + PAD * 2.0;

/// Inset from the viewport's left edge. Everything on the shelf is 3D-only, and
/// in 3D that edge is empty — the nav cluster, the axis gizmo and the height
/// ruler are all on the right. A shelf group that could show in **2D** would
/// need to dodge the vertical ruler bar, which owns the first 18px there.
const INSET: f32 = 8.0;

/// Build the shelf for a viewport's content node. Absolutely positioned on the
/// left edge, below the toolbar strip.
///
/// **One panel.** It was briefly a stack of one panel per group, from when the
/// shelf carried the terrain brush palettes as well and ran fifteen rows down
/// the viewport, and briefly two when the view controls arrived — but a gap of
/// empty viewport between four buttons and two is a lot of ceremony for a
/// distinction the rule between the groups already makes. Those moved into the
/// Terrain component, so what is left is one block.
pub(crate) fn build(commands: &mut Commands, _fonts: &EmberFonts) -> Entity {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(INSET),
                top: Val::Px(8.0),
                width: Val::Px(SHELF_W),
                // One column. It was two, from when the shelf carried the
                // terrain brush palettes and seventeen of them had to fit above
                // the bottom of a short viewport. Those live in the Terrain
                // component now, so what is left is the gizmos — a short list,
                // and a list reads better as a column than as a grid.
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(PAD)),
                row_gap: Val::Px(GAP),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                // Starts collapsed: `update_tool_buttons` opens it the moment a
                // shelf entry's `visible` predicate says yes. Without this, a
                // scene with no terrain in it would show an empty box.
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            // The shelf floats over the rendered scene, so a click on a button
            // must not also reach the picker underneath — that would select
            // whatever object happened to be behind it.
            OverlaySurface,
            bevy::ui::RelativeCursorPosition::default(),
            Interaction::default(),
            ShelfContainer,
            Name::new("vp-tool-shelf"),
        ))
        .id()
}

/// The shelf's button container; filled from `ToolbarRegistry` once it exists.
#[derive(Component)]
struct ShelfContainer;

/// Fill an empty `ShelfContainer` from the registry's [`ToolSection::Shelf`]
/// entries, one group at a time separated by a rule. Exclusive because the
/// visibility/active predicates take `&World`; runs until the registry is
/// populated and the container exists.
pub(crate) fn populate_shelf(world: &mut World) {
    let Some(registry) = world.get_resource::<ToolbarRegistry>().cloned() else {
        return;
    };
    let groups = registry.shelf_groups();
    if groups.is_empty() {
        return; // no shelf tools registered (yet)
    }
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    let mut cq = world.query_filtered::<Entity, (With<ShelfContainer>, Without<ToolsPopulated>)>();
    let Some(container) = cq.iter(world).next() else {
        return;
    };

    let sections: Vec<Vec<renzora_editor_framework::ToolEntry>> = groups
        .iter()
        .map(|id| {
            let mut v: Vec<_> = registry
                .entries()
                .iter()
                .filter(|e| e.section == ToolSection::Shelf(id))
                .cloned()
                .collect();
            v.sort_by_key(|e| e.order);
            v
        })
        .filter(|v| !v.is_empty())
        .collect();

    let mut queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        // Buttons first, per group, so each separator can be tagged with the
        // buttons on either side of it (that's what drives its visibility).
        let group_buttons: Vec<Vec<Entity>> = sections
            .iter()
            .map(|section| {
                section
                    .iter()
                    .map(|entry| tool_button(&mut commands, &fonts, entry))
                    .collect()
            })
            .collect();
        // Groups within a panel are divided by a full-width rule. The rule also
        // forces the next group onto a fresh row, so two groups never share a
        // line half-and-half, and it hides itself when either side empties.
        let mut children: Vec<Entity> = Vec::new();
        for (gi, btns) in group_buttons.iter().enumerate() {
            if gi > 0 {
                let sep = shelf_separator(&mut commands, SHELF_W - PAD * 2.0);
                commands.entity(sep).insert(ToolSepVis {
                    before: group_buttons[..gi].concat(),
                    after: btns.clone(),
                });
                children.push(sep);
            }
            children.extend(btns.iter().copied());
        }
        commands.entity(container).add_children(&children);
        commands.entity(container).insert(ToolsPopulated);
        // The root *is* the container (the buttons are its children), so the
        // whole-shelf collapse and the per-panel one are the same entity.
        commands.entity(container).insert(ShelfRoot {
            buttons: group_buttons.concat(),
        });
    }
    queue.apply(world);
}
