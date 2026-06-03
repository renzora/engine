//! Bevy-native (ember) Hierarchy panel — staged migration of the egui panel.
//!
//! **Stage 1 (this):** the entity tree — nesting, connector lines, expand/
//! collapse, type icons, selection highlight, click-to-select. Reads the same
//! `HierarchyTreeCache` + `EditorSelection` the egui panel uses.
//!
//! Later stages layer on (one file each): rename, drag-and-drop, the right-click
//! context menu, the search box, the scene-starter picker, and the
//! visibility/lock suffix toggles.

mod components;
mod context_menu;
mod drag;
mod row;
mod systems;
mod tree;

use bevy::platform::collections::HashSet;
use bevy::prelude::*;

use renzora_ember::panel::RegisterPanelContent;

const PANEL_ID: &str = "hierarchy";

/// The native panel's expand/collapse state (independent of the egui panel's
/// `HierarchyState.expanded`, which lives in a private RwLock).
#[derive(Resource, Default)]
pub(crate) struct HierExpanded(pub HashSet<Entity>);

pub fn register_native_hierarchy(app: &mut App) {
    use renzora_editor::SplashState;
    app.init_resource::<HierExpanded>();
    app.init_resource::<drag::HierDrag>();
    // Build once; the reactive keyed list drives the rows from here on.
    app.register_panel_content(PANEL_ID, true, |commands, _fonts| {
        let list = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    flex_shrink: 0.0,
                    ..default()
                },
                Name::new("hierarchy-list"),
            ))
            .id();
        renzora_ember::reactive::keyed_list(commands, list, tree::hierarchy_snapshot);
        list
    });
    app.add_systems(
        Update,
        (
            systems::hierarchy_row_click,
            systems::hierarchy_caret_click,
            systems::hierarchy_vis_toggle,
            systems::hierarchy_lock_toggle,
            drag::hier_drag,
            drag::hier_drag_tooltip,
            context_menu::hier_context_menu,
        )
            .run_if(in_state(SplashState::Editor)),
    );
}
