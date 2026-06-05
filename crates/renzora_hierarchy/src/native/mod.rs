//! Bevy-native (ember) Hierarchy panel — staged migration of the egui panel.
//!
//! **Stage 1 (this):** the entity tree — nesting, connector lines, expand/
//! collapse, type icons, selection highlight, click-to-select. Reads the same
//! `HierarchyTreeCache` + `EditorSelection` the egui panel uses.
//!
//! Later stages layer on (one file each): rename, drag-and-drop, the right-click
//! context menu, the search box, the scene-starter picker, and the
//! visibility/lock suffix toggles.

mod add_entity;
mod components;
mod context_menu;
mod drag;
mod filter;
mod rename;
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
    app.init_resource::<filter::HierFilter>();
    app.init_resource::<filter::HierSearch>();
    app.init_resource::<rename::HierRename>();
    // A pinned header (Add Entity) over the scrollable, reactive tree list.
    app.register_panel_content(PANEL_ID, false, |commands, fonts| {
        let root = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    min_height: Val::Px(0.0),
                    ..default()
                },
                Name::new("hierarchy-root"),
            ))
            .id();

        let add = renzora_ember::widgets::icon_label_button(commands, fonts, "plus", "Add Entity");
        commands.entity(add).insert(add_entity::HierAddEntity);
        let search = filter::build_search_box(commands, fonts);
        let funnel = filter::build_filter_funnel(commands, fonts);
        let header = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(5.0)),
                    flex_shrink: 0.0,
                    ..default()
                },
                Name::new("hierarchy-header"),
            ))
            .id();
        commands.entity(header).add_children(&[add, search, funnel]);

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
        let scroll = renzora_ember::widgets::scroll_view(commands, list);

        commands.entity(root).add_children(&[header, scroll]);
        root
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
            add_entity::hier_add_entity_open,
            filter::hier_filter_toggle,
            filter::hier_filter_clear,
            filter::hier_search_sync,
            rename::focus_rename_field,
            rename::rename_commit,
        )
            .run_if(in_state(SplashState::Editor)),
    );
}
