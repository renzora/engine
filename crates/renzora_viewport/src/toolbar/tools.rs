//! Filling the tool strip from `ToolbarRegistry`.
//!
//! Deferred to an exclusive system because the registry is populated by each
//! tool's own plugin at startup, after the toolbar is built, and the
//! visibility/active predicates take `&World`.
//!
//! **This is the one place tools are picked.** The gizmos, the terrain modes and
//! what each mode reveals used to live on a floating shelf down the viewport's
//! left edge, separate from this strip. That shelf sat over the scene and cost
//! the picture a column of it, so its groups now lead the toolbar instead,
//! followed by undo and redo, and the viewport shows nothing but the scene on
//! its left edge.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use renzora_editor_framework::{ToolSection, ToolbarRegistry};
use renzora_ember::font::EmberFonts;

use crate::tool_buttons::{tool_button, tool_separator, ToolSepVis, ToolsPopulated};

/// The toolbar's tool-button strip; filled from `ToolbarRegistry` once it exists.
///
/// `primary` is the first viewport's strip, the only one that carries the shelf
/// groups and undo / redo. They drive the one shared `ActiveTool` and the one
/// session history, so a copy on every slot of a four-way split would be four
/// sets of buttons for one piece of state.
#[derive(Component)]
pub(super) struct ToolContainer {
    pub(super) primary: bool,
}

/// Run condition for [`populate_tools`]: a container still waits to be filled.
/// Once it is, the system has nothing left to do for the session, so the
/// exclusive pass (and its schedule-wide sync point) stops with it.
pub(super) fn tools_unpopulated(q: Query<(), (With<ToolContainer>, Without<ToolsPopulated>)>) -> bool {
    !q.is_empty()
}

/// Fill an empty `ToolContainer` from the registry: Transform, Terrain and the
/// custom sections, then (primary strip only) every shelf group and undo / redo,
/// each group divided from the next by a rule. Exclusive because the
/// visibility/active predicates take `&World`; runs until the registry is
/// populated and the container exists.
pub(super) fn populate_tools(world: &mut World) {
    let Some(registry) = world.get_resource::<ToolbarRegistry>().cloned() else {
        return;
    };
    if registry.entries().is_empty() {
        return; // tools not registered yet
    }
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    let mut cq = world.query_filtered::<(Entity, &ToolContainer), Without<ToolsPopulated>>();
    let Some((container, primary)) = cq.iter(world).next().map(|(e, c)| (e, c.primary)) else {
        return;
    };

    let by_section = |sec| {
        let mut v: Vec<_> = registry
            .entries()
            .iter()
            .filter(|e| e.section == sec)
            .cloned()
            .collect();
        v.sort_by_key(|e| e.order);
        v
    };
    let mut sections: Vec<Vec<renzora_editor_framework::ToolEntry>> = vec![
        by_section(ToolSection::Transform),
        by_section(ToolSection::Terrain),
    ];
    for id in registry.custom_sections() {
        sections.push(by_section(ToolSection::Custom(id)));
    }
    // Shelf groups in their alphabetical group order, which is the order a
    // multi-group feature (terrain) pins by prefixing its ids. Read left to
    // right they come out as the shelf read top to bottom.
    if primary {
        for id in registry.shelf_groups() {
            sections.push(by_section(ToolSection::Shelf(id)));
        }
    }
    sections.retain(|v| !v.is_empty());

    let mut queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        // Buttons first, per section, so each separator can be tagged with the
        // buttons on either side of it (that's what drives its visibility).
        let section_buttons: Vec<Vec<Entity>> = sections
            .iter()
            .map(|section| {
                section
                    .iter()
                    .map(|entry| tool_button(&mut commands, &fonts, entry))
                    .collect()
            })
            .collect();
        let mut children: Vec<Entity> = Vec::new();
        for (si, btns) in section_buttons.iter().enumerate() {
            if si > 0 {
                let sep = tool_separator(&mut commands);
                commands.entity(sep).insert(ToolSepVis {
                    before: section_buttons[..si].concat(),
                    after: btns.clone(),
                });
                children.push(sep);
            }
            children.extend(btns.iter().copied());
        }
        if primary {
            // Undo / redo close the group. They are not registry tools (they act
            // on the session, not the viewport's mode), but they are what you
            // reach for between two gizmo drags. The rule before them shows only
            // while some tool is visible: both sides of `ToolSepVis` are the
            // tools, since undo / redo are always there, so in 2D, where no tool
            // applies, the pair stands alone without a rule capping nothing.
            let tools = section_buttons.concat();
            let sep = tool_separator(&mut commands);
            commands.entity(sep).insert(ToolSepVis {
                before: tools.clone(),
                after: tools,
            });
            children.push(sep);
            children.extend(super::build_history_actions(&mut commands, &fonts));
        }
        commands.entity(container).add_children(&children);
        commands.entity(container).insert(ToolsPopulated);
    }
    queue.apply(world);
}
