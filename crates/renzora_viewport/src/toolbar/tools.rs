//! Filling the tool strip from `ToolbarRegistry`.
//!
//! Deferred to an exclusive system because the registry is populated by each
//! tool's own plugin at startup, after the toolbar is built, and the
//! visibility/active predicates take `&World`.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use renzora_editor_framework::{ToolSection, ToolbarRegistry};
use renzora_ember::font::EmberFonts;

use crate::tool_buttons::{tool_button, tool_separator, ToolSepVis, ToolsPopulated};

/// The toolbar's tool-button strip; filled from `ToolbarRegistry` once it exists.
#[derive(Component)]
pub(super) struct ToolContainer;

/// Fill an empty `ToolContainer` from the registry (Transform / Terrain / custom
/// sections with separators). Exclusive because the visibility/active predicates
/// take `&World`; runs until the registry is populated and the container exists.
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
    let mut cq = world.query_filtered::<Entity, (With<ToolContainer>, Without<ToolsPopulated>)>();
    let Some(container) = cq.iter(world).next() else {
        return;
    };

    // Build the ordered section list: Transform, Terrain, then custom sections.
    // These are the *mode* buttons — the ones that say what the viewport is set
    // to do. What each mode opens (brushes, select modes, ops) renders on the
    // shelf instead; see `native_tool_shelf`.
    let mut sections: Vec<Vec<renzora_editor_framework::ToolEntry>> = Vec::new();
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
    let transform = by_section(ToolSection::Transform);
    if !transform.is_empty() {
        sections.push(transform);
    }
    let terrain = by_section(ToolSection::Terrain);
    if !terrain.is_empty() {
        sections.push(terrain);
    }
    for id in registry.custom_sections() {
        let v = by_section(ToolSection::Custom(id));
        if !v.is_empty() {
            sections.push(v);
        }
    }

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
        commands.entity(container).add_children(&children);
        commands.entity(container).insert(ToolsPopulated);
    }
    queue.apply(world);
}
