//! Bevy-native (ember) Hierarchy panel — a full migration of the egui panel.
//!
//! The entity tree (nesting, connector lines, expand/collapse, type icons,
//! selection highlight, click/ctrl/shift select) reads the same
//! `HierarchyTreeCache` + `EditorSelection` the egui panel uses. Layered on
//! (one file each): drag-and-drop reparenting (`drag`), the right-click context
//! menu (`context_menu`), Add Entity (`add_entity`), search + type filter
//! (`filter`), inline rename (`rename`), the empty-scene starter picker
//! (`scene_starter`), and the visibility/lock suffix toggles (`row`/`systems`).

mod add_entity;
mod components;
mod context_menu;
mod drag;
mod filter;
mod pin;
mod rename;
mod row;
mod scene_starter;
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

/// Marks the hierarchy's keyed-list content node so the reveal logic can locate
/// *this* panel's scroll viewport (the content's parent) without colliding with
/// any other [`renzora_ember::widgets::EmberScroll`] in the editor.
#[derive(Component)]
pub(crate) struct HierScrollContent;

/// A selection waiting to be revealed (ancestors expanded + scrolled into view).
/// Armed *only* when the primary selection changes — never on cache rebuilds, so
/// it can't fight the user scrolling. Persists a few frames because newly
/// expanded rows take a frame or two to lay out and grow the content height the
/// scroll position clamps against.
#[derive(Resource, Default)]
pub(crate) struct HierRevealPending {
    pub entity: Option<Entity>,
    pub frames: u32,
    /// Whether the in-view decision has been made yet (on the first frame the
    /// target row resolves).
    pub decided: bool,
    /// Outcome of that decision: the row was off-screen, so we scroll-centre it.
    /// When the row was already visible we don't move the scroll at all.
    pub scroll: bool,
}

pub fn register_native_hierarchy(app: &mut App) {
    use renzora_editor_framework::SplashState;
    app.init_resource::<HierExpanded>();
    app.init_resource::<HierRevealPending>();
    app.init_resource::<tree::HierFlatCache>();
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
        commands
            .entity(add)
            .insert((add_entity::HierAddEntity, Name::new("add-entity")));
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
                HierScrollContent,
            ))
            .id();
        // Virtualized via the shared ember primitive (the hierarchy's own
        // windowing used to live here; it's now one implementation for every
        // panel). `hierarchy_snapshot` returns the full row list; the helper
        // builds only the visible window.
        renzora_ember::virtual_scroll::virtual_scroll(commands, list, 6, tree::hierarchy_snapshot);
        let scroll = renzora_ember::widgets::scroll_view(commands, list);
        // Parent-stacking overlay: pinned ancestor headers over the top of the
        // scroll viewport (toggled by EditorSettings.hierarchy_parent_stacking).
        let stack_container = pin::build_stack_container(commands);
        commands.entity(scroll).add_child(stack_container);
        commands.insert_resource(pin::HierParentStack {
            container: stack_container,
            current: Vec::new(),
        });
        // While the scene has entities, show the tree; when empty, the starter
        // picker takes its place.
        renzora_ember::reactive::bind_display(commands, scroll, |w| !scene_starter::scene_is_empty(w));
        let picker = scene_starter::build_picker(commands, fonts);
        renzora_ember::reactive::bind_display(commands, picker, scene_starter::scene_is_empty);

        commands.entity(root).add_children(&[header, scroll, picker]);
        root
    });
    app.add_systems(
        Update,
        (
            tree::update_flatten_cache,
            systems::hierarchy_row_click,
            systems::hierarchy_reveal_selection,
            systems::hierarchy_scroll_to_selection,
            pin::hierarchy_parent_stack,
            systems::hierarchy_caret_click,
            systems::hierarchy_vis_toggle,
            systems::hierarchy_lock_toggle,
            systems::hierarchy_badge_click,
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
            .run_if(in_state(SplashState::Editor))
            .run_if(renzora_ember::dock::panel_active(PANEL_ID)),
    );
    scene_starter::register(app);
}
