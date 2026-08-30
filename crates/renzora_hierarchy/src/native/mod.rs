//! Bevy-native (ember) Hierarchy panel — a full migration of the egui panel.
//!
//! The entity tree (nesting, connector lines, expand/collapse, type icons,
//! selection highlight, click/ctrl/shift select) reads the same
//! `HierarchyTreeCache` + `EditorSelection` the egui panel uses. Layered on
//! (one file each): drag-and-drop reparenting (`drag`), the right-click context
//! menu (`context_menu`), its Attach-an-asset submenu + overlay
//! (`create_asset`), Add Entity (`add_entity`), search + type filter
//! (`filter`), inline rename (`rename`), the empty-scene starter picker
//! (`scene_starter`), and the visibility/lock suffix toggles (`row`/`systems`).

mod add_entity;
mod asset_drop;
mod components;
mod context_menu;
mod create_asset;
mod drag;
mod filter;
mod marquee;
mod pin;
mod rename;
mod row;
mod scene_drop;
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

/// The tree's scroll viewport, sized every frame by [`hier_fit_scroll`].
#[derive(Component)]
pub(crate) struct HierScrollViewport;

/// The panel is too narrow for the header's full "+ Add Entity" label. Set by
/// [`hier_responsive_header`] from the measured panel width; the button collapses
/// to icon-only so the search box keeps a usable width. Before this, the button
/// simply shrank and broke its label onto two lines.
#[derive(Resource, Default)]
pub(crate) struct HierCompact(pub bool);

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
    app.init_resource::<HierCompact>();
    app.init_resource::<HierRevealPending>();
    app.init_resource::<tree::HierFlatCache>();
    app.init_resource::<drag::HierDrag>();
    app.init_resource::<marquee::HierMarquee>();
    app.init_resource::<filter::HierFilter>();
    app.init_resource::<filter::HierSearch>();
    app.init_resource::<rename::HierRename>();
    app.init_resource::<scene_drop::ArmedHierSceneDrop>();
    app.init_resource::<asset_drop::ArmedHierAssetDrop>();
    app.init_resource::<renzora::core::ArrowKeysClaimed>();
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
                // Scene-asset drop target: dropping a `.bsn` / `.ron` here spawns
                // it as a nested instance (handled in `scene_drop`).
                scene_drop::HierRoot,
                bevy::ui::RelativeCursorPosition::default(),
            ))
            .id();

        // Add Entity sits in a pinned footer, centred, rather than in the header
        // beside the search box. It is the panel's one *creating* action among a
        // header of *finding* ones (search, filter), and reads as another of
        // them up there. At the bottom it is where you look after scrolling
        // through what exists to conclude that what you want does not.
        //
        // Accent-filled for the same reason: it was an unfilled icon-label like
        // every other header control, which is the right weight for a filter and
        // the wrong one for the button that puts things in the scene.
        let add = renzora_ember::widgets::icon_label_button_collapsing(
            commands,
            fonts,
            "plus",
            &renzora::lang::t("hierarchy.add_entity"),
            // Never collapses to its icon now: the footer gives it the width the
            // header could not, and a lone `+` centred in a bar is a puzzle.
            |_| false,
        );
        // Left on the default `Button` role. An accent fill was tried here and
        // read as too loud for a button that sits in the tree rather than over
        // it; the placement carries the emphasis on its own.
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
        commands.entity(header).add_children(&[search, funnel]);

        // The row Add Entity sits in: directly under the last entity.
        //
        // It is a sibling of the scroll viewport, not a child of it, and the
        // viewport SHRINK-WRAPS its content (see below). Those two together are
        // what make one row satisfy both halves of the requirement: with a few
        // entities the viewport is only as tall as the tree, so the button sits
        // immediately under the last one; with a few hundred the viewport fills
        // the panel and the button is pinned under it, still on screen.
        //
        // Inside the scroll it was genuinely after the last row and genuinely
        // unreachable — several hundred rows below the fold in any real scene.
        let footer = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(8.0)),
                    flex_shrink: 0.0,
                    ..default()
                },
                Name::new("hierarchy-add-row"),
            ))
            .id();
        commands.entity(footer).add_child(add);

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
        // builds only the visible window. The versioned form skips re-running
        // the snapshot entirely on frames where neither the rows nor the scroll
        // position changed (see `tree::hier_flat_version`).
        renzora_ember::virtual_scroll::virtual_scroll_versioned(
            commands,
            list,
            6,
            tree::hier_flat_version,
            tree::hierarchy_snapshot,
        );
        let scroll = renzora_ember::widgets::scroll_view(commands, list);
        // Hit-test target for the right-click quick-add menu (see `context_menu`).
        // `HierScrollViewport` hands its height to `hier_fit_scroll`, which is
        // what makes Add Entity sit under the last row until the tree overflows.
        commands.entity(scroll).insert((
            context_menu::HierListArea,
            HierScrollViewport,
            bevy::ui::RelativeCursorPosition::default(),
        ));
        // Parent-stacking overlay: pinned ancestor headers over the top of the
        // scroll viewport (toggled by EditorSettings.hierarchy_parent_stacking).
        let stack_container = pin::build_stack_container(commands);
        commands.entity(scroll).add_child(stack_container);
        commands.insert_resource(pin::HierParentStack {
            container: stack_container,
            current: Vec::new(),
        });
        // While the scene has entities, show the tree; when empty, the starter
        // picker takes its place. Add Entity follows the tree — with no scene
        // the picker *is* the way to add the first thing, and a second control
        // for it under an empty panel is noise.
        renzora_ember::reactive::tracked::bind_display(commands, scroll, |w| !scene_starter::scene_is_empty(w));
        renzora_ember::reactive::tracked::bind_display(commands, footer, |w| !scene_starter::scene_is_empty(w));
        let picker = scene_starter::build_picker(commands, fonts);
        renzora_ember::reactive::tracked::bind_display(commands, picker, scene_starter::scene_is_empty);

        commands.entity(root).add_children(&[header, scroll, footer, picker]);
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
            pin::hierarchy_pin_click,
            (systems::hierarchy_caret_click, systems::hierarchy_arrow_keys),
            systems::hierarchy_vis_toggle,
            systems::hierarchy_lock_toggle,
            systems::hierarchy_badge_click,
            (
                drag::hier_drag,
                drag::hier_drag_tooltip,
                marquee::hier_marquee,
                marquee::hier_marquee_overlay,
                marquee::hier_marquee_autoscroll,
                hier_responsive_header,
                hier_fit_scroll,
                row::animate_audio_bars,
            ),
            (
                // Scene drops land at the scene root wherever they hit the
                // panel; script/blueprint/material drops target the row under
                // the cursor. Grouped so the tuple stays under bevy's arity cap.
                scene_drop::arm_hier_scene_drop,
                scene_drop::commit_hier_scene_drop,
                asset_drop::arm_hier_asset_drop,
                asset_drop::commit_hier_asset_drop,
            ),
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
    // Kept ungated by panel visibility: it publishes `ArrowKeysClaimed`, which
    // ember's arrow-key scrolling and the 2D viewport's nudge both read. Frozen
    // while the hierarchy is backgrounded, a stuck "claimed" would swallow both
    // until the tree came back. A handful of resource reads.
    // panel-systems-ungated: publish_arrow_claim feeds ember's scroll_arrow_keys and renzora_gizmo's 2D nudge — other crates read it
    app.add_systems(
        Update,
        systems::publish_arrow_claim.run_if(in_state(SplashState::Editor)),
    );
    scene_starter::register(app);
    create_asset::register(app);
}

/// Size the tree's scroll viewport to its content, capped at the space the panel
/// has left over — which is what puts Add Entity under the last entity in a
/// small scene and pins it to the panel's bottom edge in a large one.
///
/// # Why this is measured rather than declared
///
/// The declarative spelling is `height: Auto` + `flex_shrink: 1` on the
/// viewport: ask for the content's height, let the column shrink it to what is
/// available. That is the whole behaviour in three fields, and it did not work
/// — a `Overflow::scroll_y` box with an auto height does not resolve to its
/// content the way a plain box does, so the viewport kept taking the full panel
/// and the button stayed welded to the bottom no matter how few entities were
/// in the scene.
///
/// So the two numbers are read from the laid-out nodes instead. There is no
/// feedback loop in doing it here: the list's height comes from its rows (the
/// virtual scroller's spacers stand in for the windowed-out ones), and nothing
/// about the list depends on the viewport's height except which rows get built
/// — which does not change the total.
fn hier_fit_scroll(
    viewports: Query<(Entity, &ChildOf), With<HierScrollViewport>>,
    children_q: Query<&Children>,
    computed: Query<&bevy::ui::ComputedNode>,
    lists: Query<(), With<HierScrollContent>>,
    mut nodes: Query<&mut Node>,
) {
    // Resolved through the hierarchy rather than by `single()` on each marker.
    // A panel can exist more than once — a second dock leaf, or a stashed copy
    // the dock keeps alive — and a `single()` that finds two entities returns
    // an error, so one extra instance anywhere in the editor would silently
    // switch this off for the visible one.
    for (viewport, child_of) in &viewports {
        let root = child_of.parent();
        let Ok(root_cn) = computed.get(root) else {
            continue;
        };
        let inv = root_cn.inverse_scale_factor();
        let panel_h = root_cn.size().y * inv;
        // Nothing has been laid out yet (a backgrounded dock tab, or the first
        // frame). Leave the height alone rather than collapsing the panel to
        // zero and rebuilding it when it comes back.
        if panel_h <= 0.0 {
            continue;
        }

        // What the panel column spends on everything that isn't the tree: the
        // search header and the Add Entity row. Measured rather than named, so
        // a fourth thing added to this panel is accounted for without touching
        // this system. A hidden child (the empty-scene picker) measures zero.
        let mut spent = 0.0;
        if let Ok(kids) = children_q.get(root) {
            for kid in kids.iter() {
                if kid == viewport {
                    continue;
                }
                if let Ok(cn) = computed.get(kid) {
                    spent += cn.size().y * inv;
                }
            }
        }

        // The tree's own height, from the list rather than the viewport — the
        // viewport is what we are about to set, and the list is what it should
        // be set from.
        let mut content = 0.0;
        if let Ok(kids) = children_q.get(viewport) {
            for kid in kids.iter() {
                if lists.contains(kid) {
                    if let Ok(cn) = computed.get(kid) {
                        content = cn.size().y * inv;
                    }
                }
            }
        }

        let target = Val::Px(content.min((panel_h - spent).max(0.0)));
        if let Ok(mut node) = nodes.get_mut(viewport) {
            if node.height != target {
                node.height = target;
            }
        }
    }
}

/// Watch the panel width and flip [`HierCompact`] at the point where the header's
/// three controls stop fitting on one line: an "+ Add Entity" pill (~85px), the
/// search box at its usable floor (~56px) and the filter funnel (~24px), plus
/// gaps and padding.
fn hier_responsive_header(
    root: Query<&bevy::ui::ComputedNode, With<scene_drop::HierRoot>>,
    mut compact: ResMut<HierCompact>,
) {
    const COMPACT_WIDTH: f32 = 210.0;
    let Ok(cn) = root.single() else {
        return;
    };
    let width = cn.size().x * cn.inverse_scale_factor();
    if width <= 0.0 {
        return;
    }
    let c = width < COMPACT_WIDTH;
    if compact.0 != c {
        compact.0 = c;
    }
}
