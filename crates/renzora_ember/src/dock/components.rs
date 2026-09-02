//! The components and resources the dock's systems share: what a tab, a leaf
//! and a divider carry, the per-tab content pane, and the small resources that
//! form the dock's seams to an outside consumer (focus requests, bottom-strip
//! collapse intents, the screen-space cursor).

use bevy::prelude::*;

use crate::dock::DockDirty;

/// Last known cursor position in physical **screen** coordinates, tracked from
/// [`bevy::window::CursorMoved`] messages (window client origin + event
/// position). Unlike `Window::cursor_position()`, this keeps updating during a
/// mouse-capture drag even once the cursor leaves the source window's bounds —
/// bevy clamps the readable position to `None` outside the window, but the
/// underlying move events keep flowing to the captured window with
/// out-of-bounds coordinates. Cross-window tab drops and the tear-off window
/// follow both depend on that.
#[derive(Resource, Default)]
pub struct GlobalCursor {
    pub pos: Option<Vec2>,
}

/// External request to focus (make active) a panel by id. Other crates set this
/// to programmatically bring a tab to the foreground — e.g. the viewport crate
/// focusing the **Game** tab when play starts — and `apply_focus_request`
/// routes it through the same in-place tab-switch a click performs (so labels
/// recolor and the right pane shows). Consumed (reset to `None`) each frame.
#[derive(Resource, Default)]
pub struct FocusPanelRequest(pub Option<String>);

/// Id of the dock panel the user last interacted with (clicked into, switched
/// to, or programmatically focused). Distinct from "visible" — several leaves
/// are visible at once, but only one has focus. Consumers use this to route
/// focus-sensitive behaviour (e.g. undo/redo picking the active document's
/// stack). `None` until the first interaction, letting consumers fall back to
/// another signal (like the active document tab).
#[derive(Resource, Default)]
pub struct FocusedPanel(pub Option<String>);

/// A panel tab. Click switches the active tab in place; drag re-docks. Holds
/// its leaf + child entities so consumers can restyle (e.g. real titles/icons).
#[derive(Component)]
pub struct DockTab {
    pub id: String,
    pub leaf: Entity,
    pub label: Entity,
    pub icon: Entity,
    /// Vertical insertion marker shown at this tab's edge during a drag.
    pub(crate) marker: Entity,
}

/// A dock leaf — the drop target for tab drags, and where consumers put panel
/// content. Fill the `content` node based on `active` (the visible panel id);
/// ember leaves `content` empty so the consumer owns it.
#[derive(Component)]
pub struct DockLeaf {
    pub tabs: Vec<String>,
    /// The container to put the active panel's content into.
    pub content: Entity,
    /// Id of the currently-visible tab — drives what content to show.
    pub active: String,
    /// The [`crate::dock::DockArea`] this leaf was built into — routes tree
    /// mutations to the primary dock or the owning floating window.
    pub area: Entity,
    pub(crate) overlay: Entity,
}

/// A per-tab content pane that lives inside a leaf's `content` container.
///
/// Built lazily when its tab is activated (`build_active_panels`) and despawned
/// when it stops being the active tab ([`sync_panes`]) — so at most one pane per
/// leaf exists at a time, and a workspace you are not looking at holds none.
///
/// This replaced a persistent-content model that kept every pane alive and hid it
/// with `Display::None`. That was cheaper per switch but accumulated without
/// bound, and hidden UI is *not* free: see [`sync_panes`] for the measurements and
/// the tradeoff taken.
#[derive(Component)]
pub struct TabPane {
    pub id: String,
}

/// Wrap built panel `content` in a tab pane for tab `id`. `scroll` adds a
/// wheel-scrollable viewport + scrollbar (use `false` for panels that manage
/// their own scrolling, e.g. the console or a viewport). Panes are built only
/// when their tab is active, so they start visible; [`sync_panes`] toggles
/// visibility thereafter. Add the returned pane to the leaf's `content`.
pub fn tab_pane(commands: &mut Commands, id: &str, content: Entity, scroll: bool) -> Entity {
    let outer = if scroll {
        crate::widgets::scroll_view(commands, content)
    } else {
        let p = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    min_width: Val::Px(0.0),
                    min_height: Val::Px(0.0),
                    flex_direction: FlexDirection::Column,
                    overflow: Overflow::clip(),
                    ..default()
                },
                Name::new(format!("pane:{id}")),
            ))
            .id();
        commands.entity(p).add_child(content);
        p
    };
    commands.entity(outer).insert(TabPane { id: id.to_string() });
    outer
}

/// Keep exactly one pane alive per leaf — the active tab's — and despawn the
/// rest. Runs every frame; `build_active_panels` rebuilds a pane when its tab is
/// activated again.
///
/// **This used to hide inactive panes with `Display::None` and keep them.** That
/// persistent-content model was cheaper per tab switch but unbounded over a
/// session: `ui_layout_system` performs three *unconditional* full-tree walks per
/// frame and does **not** skip `Display::None` subtrees, and taffy is worse still
/// — `compute_hidden_layout` clears the cache and recurses, so a hidden subtree is
/// re-walked from scratch every frame and never cached. So every panel ever
/// opened, in every workspace ever visited, kept costing layout forever. Lazy
/// building only deferred that: visit five workspaces and you have permanently
/// accumulated five workspaces of hidden panes. Measured, editor chrome layout was
/// 1.397 ms/frame on a *completely empty scene*.
///
/// The trade, taken deliberately: a tab switch now costs a rebuild instead of a
/// `display` flip, and transient in-panel state (scroll offset, search text,
/// expanded rows, in-progress typing) is lost with the entities. Dock *layout*
/// still persists via `layout.json`. State that should survive belongs in a
/// resource keyed by panel id — the pattern `ScriptSectionsOpen` and
/// `InspectorSectionsOpen` already use — not on the pane entities.
pub(crate) fn sync_panes(
    mut commands: Commands,
    dirty: Option<Res<DockDirty>>,
    leaves: Query<&DockLeaf>,
    children: Query<&Children>,
    panes: Query<&TabPane>,
) {
    // Stand down while a dock rebuild is armed. The rebuild owns pane lifetimes
    // on that frame: it detaches the panes the new tree will reuse to the root and
    // lets the rest die attached to their old leaf — deliberately, because a
    // content node detached *and then despawned* in the same frame frees its
    // taffy slotmap key while the old leaf still lists it as a child, which panics
    // with `invalid SlotMap key` (see `DockTree::active_tab_ids`).
    //
    // Before this system despawned inactive panes it only fired on the rare
    // "tab left the leaf" case and the overlap was unlikely; now it would fire on
    // essentially every multi-tab frame, so the collision is no longer a corner.
    if dirty.is_some_and(|d| d.0) {
        return;
    }
    for leaf in &leaves {
        let Ok(kids) = children.get(leaf.content) else {
            continue;
        };
        for child in kids.iter() {
            let Ok(pane) = panes.get(child) else {
                continue;
            };
            // One rule now covers both cases the old code split: a tab that left
            // the leaf entirely, and a tab that is merely not the active one.
            if pane.id != leaf.active {
                commands.entity(child).despawn();
            }
        }
    }
}

#[derive(Component)]
pub(crate) struct InsertMarker;

#[derive(Component)]
pub(crate) struct TabClose;

/// The empty stretch of a tab bar in a non-movable area (see `movable` on
/// `rebuild_area`) — the filler between the tabs and the bar's right-hand
/// controls. Carries an `Interaction` so the consumer can use it as a drag
/// surface of its own.
///
/// Public because the consumer, not the dock, decides what dragging a pinned
/// area's header means; the dock doesn't know which way such an area resizes,
/// or whether it resizes at all.
///
/// On the *filler* deliberately, not on the tab bar. `apply_cursor_icon` takes
/// the first hovered entity carrying a `HoverCursor` and does no topmost
/// resolution, so a cursor on the bar competes with every tab nested inside it
/// — which showed the resize cursor while hovering tabs. Scoping the marker to
/// the filler means it can only ever be hovered where there is nothing else.
#[derive(Component)]
pub struct FixedAreaHeader;

#[derive(Component)]
pub(crate) struct DropOverlay;

/// The dock-wide drop preview for root edge/corner docking — one overlay child
/// per [`crate::dock::DockArea`] node, shown while a drag targets a full-height
/// / full-width root split of that area. Recreated on every dock rebuild.
#[derive(Component)]
pub(crate) struct RootDropOverlay {
    /// The dock area this overlay previews for.
    pub(crate) area: Entity,
}

#[derive(Component)]
pub(crate) struct TabBarOf(pub(crate) Entity);

#[derive(Component)]
pub(crate) struct TabGhost;

/// Tags a split's divider with everything its drag needs.
#[derive(Component)]
pub(crate) struct Divider {
    pub(crate) container: Entity,
    pub(crate) first_wrap: Entity,
    pub(crate) horizontal: bool,
    pub(crate) path: Vec<bool>,
    /// The dock area this divider's split belongs to — routes the ratio
    /// persist to the right tree (primary vs a floating window's).
    pub(crate) area: Entity,
    /// True in a floating dock window — those never publish
    /// [`BottomSnapRequest`] (the snap-closed gesture is a primary-dock
    /// bottom-panel affair).
    pub(crate) floating: bool,
    /// `Some(panel)` when this is a vertical divider whose bottom pane is a
    /// leaf holding one of the [`BottomStripMarkers`] panels — i.e. the
    /// collapsible bottom strip even when it isn't the root region. Overshoot
    /// this divider downward and the strip snap-closes, same as the root one;
    /// the panel id tells the consumer which region to collapse.
    pub(crate) strip: Option<String>,
}

/// Panel ids that identify the consumer's collapsible bottom strip by
/// content (the editor shell registers `assets`/`console`). The dock is
/// otherwise content-agnostic — the *root* bottom region always collapses —
/// but a strip nested under one column (a full-height panel beside it) can
/// only be recognized by what it holds. A leaf tabbing one of these as the
/// bottom child of a vertical split gets the collapse chevron and the
/// divider snap-closed gesture, wherever it sits in the tree.
#[derive(Resource, Default)]
pub struct BottomStripMarkers(pub Vec<String>);

/// Published when the user drags a collapsible bottom region's divider hard
/// down — past the ratio clamp, squeezing the bottom pane under
/// [`BOTTOM_SNAP_PX`] — or clicks the region's collapse chevron. The dock
/// itself can't collapse anything (the bottom-panel stash is editor-shell
/// state), so it hands the intent to the consumer via this seam. Mirrors
/// [`crate::dock::DockDragWatch`]'s outside-consumer pattern.
#[derive(Resource, Default)]
pub struct BottomSnapRequest(pub Option<BottomSnap>);

/// One collapse intent (see [`BottomSnapRequest`]).
#[derive(Clone)]
pub struct BottomSnap {
    /// Ratio (top share) to record for reopening. Divider snaps pass the
    /// **pre-drag** ratio — by the time the consumer detaches, the live tree
    /// holds the squished mid-drag one. `None` (chevron clicks) means the
    /// ratio in the tree is accurate; use whatever the detach returns.
    pub restore: Option<f32>,
    /// `Some(panel)`: collapse the vertical region containing this panel (a
    /// nested strip, see [`BottomStripMarkers`]). `None`: the root bottom
    /// region.
    pub target: Option<String>,
}

/// Set by an outside consumer right after it re-attaches the bottom region
/// while the mouse button is still held (the shell's "drag the collapsed
/// strip open" gesture): once the rebuilt tree's vertical divider at this
/// path exists (empty path = the primary root divider; a non-empty one is
/// where a nested strip re-attached — [`crate::dock::DockTree::attach_bottom_at`]
/// returns it), `divider_drag` adopts it as the live drag, so the just-reopened
/// panel keeps sizing under the held cursor. Cleared on adoption or when the
/// button is released.
#[derive(Resource, Default)]
pub struct GrabRootDivider(pub Option<Vec<bool>>);

/// The collapse chevron at the right end of a collapsible bottom region's
/// tab bar: clicking publishes [`BottomSnapRequest`] so the consumer
/// collapses the region (reopening restores its height).
#[derive(Component)]
pub(crate) struct BottomCollapseBtn {
    /// Mirrors [`BottomSnap::target`]: `None` for the root bottom region,
    /// `Some(panel)` for a nested strip.
    pub(crate) target: Option<String>,
}

/// The whole-leaf drag handle at the far left of a tab bar: dragging it
/// moves the leaf's entire tab set as one unit — same drop zones as a
/// single-tab drag, but the drop inserts the whole leaf (see the `group`
/// paths in `tab_drag`).
#[derive(Component)]
pub(crate) struct LeafGrip {
    pub(crate) leaf: Entity,
}

/// Snap threshold for [`BottomSnapRequest`]: the second pane's would-be
/// height in physical px below which a root-divider drag reads as "close it".
/// The ratio clamp already floors the pane at 10% of the area, so reaching
/// this takes a deliberate overshoot well past where the divider stops.
pub(crate) const BOTTOM_SNAP_PX: f32 = 48.0;

/// Info passed to a leaf so its tab-bar empty area can act as a secondary
/// resize handle for the parent split's boundary ("more to grip").
#[derive(Clone)]
pub(crate) struct ParentSplit {
    pub(crate) container: Entity,
    pub(crate) first_wrap: Entity,
    pub(crate) horizontal: bool,
    pub(crate) is_second: bool,
    pub(crate) path: Vec<bool>,
}

impl ParentSplit {
    /// Does the parent's divider run *along* this leaf's tab bar, so the bar's
    /// empty area is a natural extension of it?
    ///
    /// Only for a vertical split's lower child: its divider is a horizontal
    /// line directly above the bar, spanning the same width — which is what
    /// makes "drag the empty space in the header" mean the same gesture as
    /// "drag the edge above it" (the global bottom panel is the case people
    /// actually use).
    ///
    /// A *horizontal* split's left child used to qualify too, and that was
    /// wrong in a way that showed up as a cursor bug: the divider is a thin
    /// vertical line at the leaf's right edge, but the filler is the whole
    /// remaining width of the tab bar, so every column but the last showed an
    /// ew-resize cursor across its entire header — the width of a panel away
    /// from the boundary it would drag. The vertical dividers are still there
    /// and still draggable; they're just not secretly the size of a tab bar.
    pub(crate) fn aligned(&self) -> bool {
        !self.horizontal && self.is_second
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only a bar with its parent's divider lying *along* it may double as that
    /// divider's handle.
    ///
    /// Regression guard: a horizontal split's left child qualified too, which
    /// gave every column but the last an ew-resize cursor across the whole
    /// width of its tab bar — a panel's width away from the vertical line it
    /// would actually have dragged.
    #[test]
    fn only_a_bar_under_its_divider_doubles_as_a_handle() {
        let split = |horizontal, is_second| ParentSplit {
            container: Entity::PLACEHOLDER,
            first_wrap: Entity::PLACEHOLDER,
            horizontal,
            is_second,
            path: Vec::new(),
        };
        // Vertical split, lower child: the divider is the line right above this
        // bar — the global bottom panel's "drag the header" gesture.
        assert!(split(false, true).aligned());
        // Vertical split, upper child: its bar is at the top, nowhere near.
        assert!(!split(false, false).aligned());
        // Horizontal split, either side: the divider is a vertical line at one
        // edge, not something the bar runs along.
        assert!(!split(true, false).aligned());
        assert!(!split(true, true).aligned());
    }
}
