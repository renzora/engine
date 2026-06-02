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
mod row;
mod systems;
mod tree;

use bevy::platform::collections::HashSet;
use bevy::prelude::*;

pub(crate) use components::*;

use renzora_ember::dock::{tab_pane, DockLeaf, TabPane};
use renzora_ember::font::EmberFonts;

const PANEL_ID: &str = "hierarchy";

/// The native panel's expand/collapse state (independent of the egui panel's
/// `HierarchyState.expanded`, which lives in a private RwLock).
#[derive(Resource, Default)]
pub(crate) struct HierExpanded(pub HashSet<Entity>);

pub fn register_native_hierarchy(app: &mut App) {
    use renzora::NativePanelExt;
    use renzora_editor::SplashState;
    app.init_resource::<HierExpanded>();
    app.register_native_panel(PANEL_ID);
    app.add_systems(
        Update,
        (
            (hierarchy_content_system, tree::hierarchy_refresh).chain(),
            systems::hierarchy_row_click,
            systems::hierarchy_vis_toggle,
            systems::hierarchy_lock_toggle,
            systems::hierarchy_row_visual,
        )
            .run_if(in_state(SplashState::Editor)),
    );
}

/// Build the hierarchy list pane once (lazily) when its tab is first activated.
pub(crate) fn hierarchy_content_system(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    leaves: Query<&DockLeaf>,
    children: Query<&Children>,
    panes: Query<&TabPane>,
) {
    let Some(_fonts) = fonts else {
        return;
    };
    for leaf in &leaves {
        if leaf.active != PANEL_ID {
            continue;
        }
        let exists = children.get(leaf.content).is_ok_and(|kids| {
            kids.iter()
                .any(|c| panes.get(c).is_ok_and(|p| p.id == PANEL_ID))
        });
        if exists {
            continue;
        }
        let list = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    flex_shrink: 0.0,
                    ..default()
                },
                HierarchyView {
                    content_hash: u64::MAX,
                },
                Name::new("hierarchy-list"),
            ))
            .id();
        let pane = tab_pane(&mut commands, PANEL_ID, list, true);
        commands.entity(leaf.content).add_child(pane);
    }
}
