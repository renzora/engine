//! The tool shelf — a two-column grid of tool buttons floating down the
//! viewport's left edge, in the shape image editors have settled on.
//!
//! The two surfaces split by **depth**. A tool that *opens* other tools stays on
//! the strip across the viewport's top edge — the gizmo modes, terrain
//! sculpt/paint/foliage/resize, mesh Edit/Sculpt — so there is always one
//! visible row saying what the viewport is set to do. Everything those modes
//! reveal comes here: the brushes, the select modes, the ops.
//!
//! That is a shape argument, not a taste one. Terrain alone has 17 sculpt
//! brushes and mesh Edit mode wants nine more buttons; laid out horizontally
//! each of those wraps the strip into a second row and pushes everything else —
//! Play included — down with it, and a row of identical squares is a poor thing
//! to hunt through. Down the left edge there is nothing competing for the space.
//! Two columns keeps a palette a compact block rather than a 17-tall ribbon that
//! would run past the bottom of a short viewport, and it puts roughly a
//! screenful of buttons within one glance. The left edge is genuinely free: the
//! nav cluster, the axis gizmo and the height ruler all live on the right.
//!
//! Mesh draw (box / polyline / join) is the one group with no mode above it — it
//! needs no selection and no mode, so it is simply always there in 3D, and being
//! first in the sort it gives the shelf a row that doesn't move while the
//! contextual palettes come and go beneath it.
//!
//! Entries come from the same [`ToolbarRegistry`] the top strip reads, tagged
//! [`ToolSection::Shelf`]. Nothing here is feature-specific: groups render top
//! to bottom in alphabetical order of their group string, which is a *global*
//! sort across every crate that registers one — see [`ToolSection::Shelf`] for
//! how a multi-group feature (terrain, whose foliage groups come from a
//! different crate) pins its own order. The buttons, their show/hide/highlight
//! driver and their click handler are shared with the top strip; see
//! [`crate::tool_buttons`].

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
/// Two columns of buttons plus the gap between them and the padding either side.
const SHELF_W: f32 = SIDE_BTN * 2.0 + GAP + PAD * 2.0;

/// Inset from the viewport's left edge. Everything on the shelf is 3D-only, and
/// in 3D that edge is empty — the nav cluster, the axis gizmo and the height
/// ruler are all on the right. A shelf group that could show in **2D** would
/// need to dodge the vertical ruler bar, which owns the first 18px there.
const INSET: f32 = 8.0;

/// The shelf's button container; filled from `ToolbarRegistry` once it exists.
#[derive(Component)]
struct ShelfContainer;

/// Build the shelf for a viewport's content node. Absolutely positioned on the
/// left edge, below the toolbar strip, and vertically centred on the taller
/// palettes' behalf.
pub(crate) fn build(commands: &mut Commands, _fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(INSET),
                top: Val::Px(8.0),
                width: Val::Px(SHELF_W),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(PAD)),
                row_gap: Val::Px(GAP),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                // Starts collapsed: `update_tool_buttons` opens it the moment a
                // shelf entry's `visible` predicate says yes. Without this, a
                // scene with no terrain in it would show an empty tinted box.
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            // The shelf floats over the rendered scene, so a click on a brush
            // must not also reach the picker underneath — that would select
            // whatever object happened to be behind the button.
            OverlaySurface,
            bevy::ui::RelativeCursorPosition::default(),
            Interaction::default(),
            Name::new("vp-tool-shelf"),
        ))
        .id();

    let container = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                // Two columns: the row wraps at the shelf's fixed width, which is
                // exactly two buttons wide.
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(GAP),
                row_gap: Val::Px(GAP),
                ..default()
            },
            ShelfContainer,
            Name::new("vp-shelf-tools"),
        ))
        .id();
    commands.entity(root).add_child(container);
    root
}

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
    // The shelf's root is its parent — needed so the whole overlay can collapse
    // when every entry in it is hidden.
    let root = world.get::<ChildOf>(container).map(|c| c.parent());

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
        let mut children: Vec<Entity> = Vec::new();
        for (gi, btns) in group_buttons.iter().enumerate() {
            if gi > 0 {
                // A full-width separator also forces the next group onto a fresh
                // row, so two groups never share a line half-and-half.
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
        if let Some(root) = root {
            commands.entity(root).insert(ShelfRoot {
                buttons: group_buttons.concat(),
            });
        }
    }
    queue.apply(world);
}
