//! Dockable panel layout — a reusable bevy_ui component.
//!
//! The [`DockTree`] model (a binary tree of `Split`s and `Leaf`s) plus the
//! bevy_ui reconciler and interaction systems that render and drive it:
//! draggable split dividers, tab drag-docking with drop zones + insertion
//! markers, in-place tab switching, hover, and a tab-bar secondary resize
//! handle. Used by the editor shell and available to games.
//!
//! ## Using it
//! Add [`DockPlugin`], set [`Dock::tree`], spawn a node tagged [`DockArea`]
//! where the dock should render, and flip [`DockDirty`] to build it. Each leaf's
//! content is left to the consumer — query the public [`DockLeaf`] and fill it
//! (the editor fills it with panels; a game with whatever).

use std::collections::HashMap;

use bevy::prelude::*;

use crate::font::{glyph, icon_text, ui_font, EmberFonts};
use crate::theme::{
    rgb, ACCENT_BLUE, CLOSE_RED, DIVIDER, HEADER_BG, PANEL_BG, TAB_ACTIVE_BG, TAB_HOVER_BG,
    TEXT_MUTED, TEXT_PRIMARY,
};

// ── Model ────────────────────────────────────────────────────────────────────

/// Direction of a split in the dock tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    /// Children are side-by-side (left / right).
    Horizontal,
    /// Children are stacked (top / bottom).
    Vertical,
}

/// A node in the dock tree.
#[derive(Debug, Clone)]
pub enum DockTree {
    Split {
        direction: SplitDirection,
        /// Fraction of space given to the first child (0.1–0.9).
        ratio: f32,
        first: Box<DockTree>,
        second: Box<DockTree>,
    },
    Leaf {
        /// Panel IDs shown as tabs.
        tabs: Vec<String>,
        /// Index of the currently visible tab.
        active_tab: usize,
    },
    Empty,
}

impl DockTree {
    /// Single-tab leaf.
    pub fn leaf(id: impl Into<String>) -> Self {
        DockTree::Leaf {
            tabs: vec![id.into()],
            active_tab: 0,
        }
    }

    /// A leaf with several tabbed panels.
    pub fn tabs(tabs: &[&str]) -> Self {
        DockTree::Leaf {
            tabs: tabs.iter().map(|s| s.to_string()).collect(),
            active_tab: 0,
        }
    }

    /// Horizontal split (left / right).
    pub fn horizontal(first: DockTree, second: DockTree, ratio: f32) -> Self {
        DockTree::Split {
            direction: SplitDirection::Horizontal,
            ratio: ratio.clamp(0.1, 0.9),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Vertical split (top / bottom).
    pub fn vertical(first: DockTree, second: DockTree, ratio: f32) -> Self {
        DockTree::Split {
            direction: SplitDirection::Vertical,
            ratio: ratio.clamp(0.1, 0.9),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Set the split ratio at `path` (`false`/`true` = first/second child;
    /// empty path targets this node). Persists a divider drag.
    pub fn update_ratio(&mut self, path: &[bool], new_ratio: f32) {
        if let DockTree::Split {
            ratio,
            first,
            second,
            ..
        } = self
        {
            match path.split_first() {
                Some((&true, rest)) => second.update_ratio(rest, new_ratio),
                Some((&false, rest)) => first.update_ratio(rest, new_ratio),
                None => *ratio = new_ratio.clamp(0.1, 0.9),
            }
        }
    }

    /// The leaf that contains `panel`, if any.
    pub fn find_leaf_mut(&mut self, panel: &str) -> Option<&mut DockTree> {
        match self {
            DockTree::Split { first, second, .. } => first
                .find_leaf_mut(panel)
                .or_else(|| second.find_leaf_mut(panel)),
            DockTree::Leaf { tabs, .. } => tabs.iter().any(|t| t == panel).then_some(self),
            DockTree::Empty => None,
        }
    }

    /// Is `panel` the active (visible) tab in its leaf?
    pub fn is_active_tab(&self, panel: &str) -> bool {
        match self {
            DockTree::Split { first, second, .. } => {
                first.is_active_tab(panel) || second.is_active_tab(panel)
            }
            DockTree::Leaf { tabs, active_tab } => {
                tabs.get(*active_tab).is_some_and(|t| t == panel)
            }
            DockTree::Empty => false,
        }
    }

    /// Make `panel` the active tab in its leaf.
    pub fn set_active_tab(&mut self, panel: &str) {
        if let Some(DockTree::Leaf { tabs, active_tab }) = self.find_leaf_mut(panel) {
            if let Some(idx) = tabs.iter().position(|t| t == panel) {
                *active_tab = idx;
            }
        }
    }

    /// Append `new_panel` as a tab in the leaf containing `sibling`.
    pub fn add_tab(&mut self, sibling: &str, new_panel: String) -> bool {
        if let Some(DockTree::Leaf { tabs, active_tab }) = self.find_leaf_mut(sibling) {
            tabs.push(new_panel);
            *active_tab = tabs.len() - 1;
            true
        } else {
            false
        }
    }

    /// Insert `new_panel` into `sibling`'s leaf before `before` (or at the end).
    pub fn add_tab_before(&mut self, sibling: &str, new_panel: String, before: Option<&str>) -> bool {
        if let Some(DockTree::Leaf { tabs, active_tab }) = self.find_leaf_mut(sibling) {
            let idx = before
                .and_then(|b| tabs.iter().position(|t| t == b))
                .unwrap_or(tabs.len())
                .min(tabs.len());
            tabs.insert(idx, new_panel);
            *active_tab = idx;
            true
        } else {
            false
        }
    }

    /// Remove a panel from the tree, collapsing any emptied leaves/splits.
    pub fn remove_panel(&mut self, panel: &str) -> bool {
        let removed = match self {
            DockTree::Split { first, second, .. } => {
                first.remove_panel(panel) || second.remove_panel(panel)
            }
            DockTree::Leaf { tabs, active_tab } => {
                if let Some(idx) = tabs.iter().position(|t| t == panel) {
                    tabs.remove(idx);
                    if *active_tab >= tabs.len() && !tabs.is_empty() {
                        *active_tab = tabs.len() - 1;
                    }
                    true
                } else {
                    false
                }
            }
            DockTree::Empty => false,
        };
        if removed {
            self.cleanup_empty();
        }
        removed
    }

    fn cleanup_empty(&mut self) {
        if let DockTree::Split { first, second, .. } = self {
            first.cleanup_empty();
            second.cleanup_empty();
            let first_empty = first.is_empty();
            let second_empty = second.is_empty();
            if first_empty {
                *self = std::mem::replace(second, DockTree::Empty);
            } else if second_empty {
                *self = std::mem::replace(first, DockTree::Empty);
            }
        } else if let DockTree::Leaf { tabs, .. } = self {
            if tabs.is_empty() {
                *self = DockTree::Empty;
            }
        }
    }

    fn is_empty(&self) -> bool {
        matches!(self, DockTree::Empty)
            || matches!(self, DockTree::Leaf { tabs, .. } if tabs.is_empty())
    }

    /// Split the leaf containing `target`, placing `new_panel` on the given side.
    pub fn split_at(&mut self, target: &str, new_panel: String, zone: DropZone) -> bool {
        if let Some(leaf) = self.find_leaf_mut(target) {
            let old = std::mem::replace(leaf, DockTree::Empty);
            let new_leaf = DockTree::leaf(new_panel);
            *leaf = match zone {
                DropZone::Left => DockTree::horizontal(new_leaf, old, 0.5),
                DropZone::Right => DockTree::horizontal(old, new_leaf, 0.5),
                DropZone::Top => DockTree::vertical(new_leaf, old, 0.5),
                DropZone::Bottom => DockTree::vertical(old, new_leaf, 0.5),
                DropZone::Center => {
                    *leaf = old;
                    return false;
                }
            };
            true
        } else {
            false
        }
    }
}

/// Where a dragged panel will land on a leaf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropZone {
    /// Add as a tab in the target leaf.
    Center,
    Left,
    Right,
    Top,
    Bottom,
}

// ── Plugin + public seam ─────────────────────────────────────────────────────

/// Adds the dock reconciler + interaction systems and the resources they need.
pub struct DockPlugin;

impl Plugin for DockPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dock>()
            .init_resource::<DockDirty>()
            .init_resource::<PendingSwitch>()
            .init_resource::<DraggedDivider>()
            .init_resource::<TabDrag>()
            .add_systems(
                Update,
                (
                    crate::font::load_fonts,
                    divider_drag,
                    tab_drag,
                    apply_tab_switch,
                    tab_hover,
                    tab_close_hover,
                    // Rebuild last, in the same frame the model mutates, so the
                    // dock doesn't show a stale layout for a frame (flicker).
                    rebuild_dock,
                    // Toggle per-tab pane visibility after any rebuild.
                    sync_panes,
                )
                    .chain(),
            )
            .add_systems(Update, add_panel_click);
    }
}

/// The live dock layout. Set [`Dock::tree`]; divider/tab drags mutate it in
/// place; it persists across rebuilds.
#[derive(Resource)]
pub struct Dock {
    pub tree: DockTree,
}

impl Default for Dock {
    fn default() -> Self {
        Self {
            tree: DockTree::Empty,
        }
    }
}

/// Tag the node the dock should render into. Flip [`DockDirty`] to (re)build.
#[derive(Component)]
pub struct DockArea;

/// Set to rebuild the dock subtree (structure changed, or first build). Tab
/// switches do NOT set this — they update in place.
#[derive(Resource, Default)]
pub struct DockDirty(pub bool);

// ── Components ───────────────────────────────────────────────────────────────

/// A panel tab. Click switches the active tab in place; drag re-docks. Holds
/// its leaf + child entities so consumers can restyle (e.g. real titles/icons).
#[derive(Component)]
pub struct DockTab {
    pub id: String,
    pub leaf: Entity,
    pub label: Entity,
    pub icon: Entity,
    /// Vertical insertion marker shown at this tab's edge during a drag.
    marker: Entity,
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
    overlay: Entity,
}

/// A per-tab content pane that lives inside a leaf's `content` container. Panes
/// are built **once** (lazily, when their tab is first activated) and kept; tab
/// switches just toggle their [`Node::display`] via [`sync_panes`] instead of
/// despawning + rebuilding. This is the persistent-content model — it avoids the
/// despawn/rebuild churn (and entity-reuse races) of swapping a single content
/// node on every tab switch.
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

/// Toggle each pane's visibility to match its leaf's active tab, and drop panes
/// whose tab has left the leaf (e.g. moved to another leaf). Runs every frame;
/// writes are guarded so it never churns.
fn sync_panes(
    mut commands: Commands,
    leaves: Query<&DockLeaf>,
    children: Query<&Children>,
    panes: Query<&TabPane>,
    mut nodes: Query<&mut Node>,
) {
    for leaf in &leaves {
        let Ok(kids) = children.get(leaf.content) else {
            continue;
        };
        for child in kids.iter() {
            let Ok(pane) = panes.get(child) else {
                continue;
            };
            if !leaf.tabs.contains(&pane.id) {
                commands.entity(child).despawn();
                continue;
            }
            let display = if pane.id == leaf.active {
                Display::Flex
            } else {
                Display::None
            };
            if let Ok(mut n) = nodes.get_mut(child) {
                if n.display != display {
                    n.display = display;
                }
            }
        }
    }
}

#[derive(Component)]
struct InsertMarker;

#[derive(Component)]
struct TabClose;

#[derive(Component)]
struct DropOverlay;

#[derive(Component)]
struct TabBarOf(Entity);

#[derive(Component)]
struct TabGhost;

/// Tags a split's divider with everything its drag needs.
#[derive(Component)]
struct Divider {
    container: Entity,
    first_wrap: Entity,
    horizontal: bool,
    path: Vec<bool>,
}

/// Info passed to a leaf so its tab-bar empty area can act as a secondary
/// resize handle for the parent split's boundary ("more to grip").
#[derive(Clone)]
struct ParentSplit {
    container: Entity,
    first_wrap: Entity,
    horizontal: bool,
    is_second: bool,
    path: Vec<bool>,
}

impl ParentSplit {
    fn aligned(&self) -> bool {
        (!self.horizontal && self.is_second) || (self.horizontal && !self.is_second)
    }
}

#[derive(Resource, Default)]
struct DraggedDivider(Option<DividerDrag>);

struct DividerDrag {
    handle: Entity,
    start_cursor: Vec2,
    start_ratio: f32,
}

#[derive(Resource, Default)]
struct TabDrag(Option<TabDragState>);

struct TabDragState {
    id: String,
    leaf: Entity,
    start_cursor: Vec2,
    active: bool,
    action: Option<DropAction>,
    ghost: Option<Entity>,
    shown_overlay: Option<Entity>,
    shown_marker: Option<Entity>,
}

enum DropAction {
    Split { rep: String, zone: DropZone },
    Tab { rep: String, before: Option<String> },
}

/// A pending in-place tab switch: (leaf entity, panel id to activate).
#[derive(Resource, Default)]
struct PendingSwitch(Option<(Entity, String)>);

// ── Defaults a consumer overrides ────────────────────────────────────────────

/// Default tab title + icon for a panel id (humanized + a neutral dot).
/// Consumers override with real metadata (e.g. the editor's panel registry).
fn tab_meta(id: &str) -> (String, &'static str) {
    (humanize(id), "circle")
}

/// `render_pipeline` → `Render Pipeline`.
pub fn humanize(id: &str) -> String {
    id.split('_')
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ── Systems ──────────────────────────────────────────────────────────────────

const TAB_DRAG_THRESHOLD: f32 = 6.0;

fn as_ratio(v: Val) -> Option<f32> {
    if let Val::Percent(p) = v {
        Some(p / 100.0)
    } else {
        None
    }
}

/// Drag a divider handle to resize its split. Latches on press (continues off
/// the handle), moves by cursor delta (no snap), resizes live + persists.
fn divider_drag(
    mut dragged: ResMut<DraggedDivider>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    dividers: Query<(Entity, &Interaction, &Divider)>,
    computed: Query<&bevy::ui::ComputedNode>,
    mut nodes: Query<&mut Node>,
    mut dock: ResMut<Dock>,
) {
    if mouse.just_released(MouseButton::Left) {
        dragged.0 = None;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };

    if dragged.0.is_none() {
        if !mouse.just_pressed(MouseButton::Left) {
            return;
        }
        for (entity, interaction, div) in &dividers {
            if *interaction == Interaction::Pressed {
                let start_ratio = nodes
                    .get(div.first_wrap)
                    .ok()
                    .and_then(|n| as_ratio(if div.horizontal { n.width } else { n.height }))
                    .unwrap_or(0.5);
                dragged.0 = Some(DividerDrag {
                    handle: entity,
                    start_cursor: cursor,
                    start_ratio,
                });
                break;
            }
        }
    }

    let Some(drag) = dragged.0.as_ref() else {
        return;
    };
    let Ok((_, _, div)) = dividers.get(drag.handle) else {
        dragged.0 = None;
        return;
    };
    let Ok(cn) = computed.get(div.container) else {
        return;
    };
    let extent = if div.horizontal {
        cn.size().x
    } else {
        cn.size().y
    } * cn.inverse_scale_factor();
    if extent <= 0.0 {
        return;
    }
    let moved = if div.horizontal {
        cursor.x - drag.start_cursor.x
    } else {
        cursor.y - drag.start_cursor.y
    };
    let ratio = (drag.start_ratio + moved / extent).clamp(0.1, 0.9);

    let first_wrap = div.first_wrap;
    let handle = drag.handle;
    let horizontal = div.horizontal;
    let dpath = div.path.clone();

    if let Ok(mut n) = nodes.get_mut(first_wrap) {
        if horizontal {
            n.width = Val::Percent(ratio * 100.0);
        } else {
            n.height = Val::Percent(ratio * 100.0);
        }
    }
    if let Ok(mut n) = nodes.get_mut(handle) {
        if horizontal {
            n.left = Val::Percent(ratio * 100.0);
        } else {
            n.top = Val::Percent(ratio * 100.0);
        }
    }
    dock.tree.update_ratio(&dpath, ratio);
}

/// Drag a tab to re-dock; a plain click switches the active tab.
#[allow(clippy::too_many_arguments)]
fn tab_drag(
    mut drag: ResMut<TabDrag>,
    mut dirty: ResMut<DockDirty>,
    mut pending: ResMut<PendingSwitch>,
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    tabs: Query<(Entity, &Interaction, &DockTab, &bevy::ui::RelativeCursorPosition)>,
    tabbars: Query<(Entity, &TabBarOf, &bevy::ui::RelativeCursorPosition)>,
    mut leaves: Query<(Entity, &mut DockLeaf, &bevy::ui::RelativeCursorPosition)>,
    mut nodes: Query<&mut Node>,
    mut dock: ResMut<Dock>,
) {
    let cursor = windows.single().ok().and_then(|w| w.cursor_position());

    if drag.0.is_none() && mouse.just_pressed(MouseButton::Left) {
        if let Some(cur) = cursor {
            for (_, interaction, tab, _) in &tabs {
                if *interaction == Interaction::Pressed {
                    drag.0 = Some(TabDragState {
                        id: tab.id.clone(),
                        leaf: tab.leaf,
                        start_cursor: cur,
                        active: false,
                        action: None,
                        ghost: None,
                        shown_overlay: None,
                        shown_marker: None,
                    });
                    break;
                }
            }
        }
    }

    if mouse.just_released(MouseButton::Left) {
        if let Some(state) = drag.0.take() {
            if let Some(ghost) = state.ghost {
                commands.entity(ghost).despawn();
            }
            for e in [state.shown_overlay, state.shown_marker].into_iter().flatten() {
                if let Ok(mut n) = nodes.get_mut(e) {
                    n.display = Display::None;
                }
            }
            if state.active {
                if let Some(action) = state.action {
                    // A reorder within the same leaf only changes tab order, not
                    // structure — do it in place (move the tab entity) instead of
                    // rebuilding the dock subtree, which avoids the layout flicker.
                    let inplace = match &action {
                        DropAction::Tab { rep, before } => leaves
                            .get(state.leaf)
                            .ok()
                            .filter(|(_, ld, _)| ld.tabs.contains(rep))
                            .map(|(_, ld, _)| reordered(&ld.tabs, &state.id, before.as_deref())),
                        _ => None,
                    };
                    apply_action(&mut dock.tree, &state.id, action);
                    match inplace {
                        Some(new_tabs) => {
                            if let Ok((_, mut ld, _)) = leaves.get_mut(state.leaf) {
                                ld.tabs = new_tabs.clone();
                            }
                            if let Some((tabbar, _, _)) =
                                tabbars.iter().find(|(_, b, _)| b.0 == state.leaf)
                            {
                                let ordered: Vec<Entity> = new_tabs
                                    .iter()
                                    .filter_map(|id| {
                                        tabs.iter()
                                            .find(|(_, _, t, _)| &t.id == id)
                                            .map(|(e, _, _, _)| e)
                                    })
                                    .collect();
                                if !ordered.is_empty() {
                                    commands.entity(tabbar).insert_children(0, &ordered);
                                }
                            }
                        }
                        None => dirty.0 = true,
                    }
                }
            } else {
                pending.0 = Some((state.leaf, state.id));
            }
        }
        return;
    }

    let Some(cur) = cursor else {
        return;
    };
    let Some(state) = drag.0.as_mut() else {
        return;
    };
    if !state.active {
        if cur.distance(state.start_cursor) > TAB_DRAG_THRESHOLD {
            state.active = true;
            if let Some(fonts) = &fonts {
                state.ghost = Some(spawn_ghost(&mut commands, &fonts.ui, &state.id, cur));
            }
        } else {
            return;
        }
    }

    if let Some(ghost) = state.ghost {
        if let Ok(mut n) = nodes.get_mut(ghost) {
            n.left = Val::Px(cur.x + 12.0);
            n.top = Val::Px(cur.y + 12.0);
        }
    }

    let mut action: Option<DropAction> = None;
    let mut new_overlay: Option<(Entity, DropZone)> = None;
    let mut new_marker: Option<(Entity, bool)> = None;

    let over_bar = tabbars
        .iter()
        .find(|(_, _, rcp)| rcp.cursor_over)
        .map(|(_, bar, _)| bar.0);
    if let Some(leaf_ent) = over_bar {
        if let Some((_, ld, _)) = leaves.iter().find(|(e, _, _)| *e == leaf_ent) {
            let mut before: Option<String> = None;
            let mut marker: Option<(Entity, bool)> = None;
            for id in ld.tabs.iter() {
                if let Some((_, _, tab, rcp)) = tabs.iter().find(|(_, _, t, _)| &t.id == id) {
                    let nx = rcp.normalized.map_or(f32::INFINITY, |n| n.x);
                    if nx < 0.0 {
                        before = Some(id.clone());
                        marker = Some((tab.marker, false));
                        break;
                    }
                }
            }
            if marker.is_none() {
                if let Some(last) = ld.tabs.last() {
                    marker = tabs
                        .iter()
                        .find(|(_, _, t, _)| &t.id == last)
                        .map(|(_, _, t, _)| (t.marker, true));
                }
                before = None;
            }
            if let Some(rep) = ld.tabs.iter().find(|t| **t != state.id).cloned() {
                action = Some(DropAction::Tab { rep, before });
                new_marker = marker;
            }
        }
    } else {
        for (_, ld, rcp) in &leaves {
            if rcp.cursor_over {
                if let Some(norm) = rcp.normalized {
                    let (x, y) = (norm.x + 0.5, norm.y + 0.5);
                    let zone = pick_zone(x, y);
                    if let Some(rep) = ld.tabs.iter().find(|t| **t != state.id).cloned() {
                        action = Some(if matches!(zone, DropZone::Center) {
                            DropAction::Tab { rep, before: None }
                        } else {
                            DropAction::Split { rep, zone }
                        });
                        new_overlay = Some((ld.overlay, zone));
                    }
                }
                break;
            }
        }
    }
    state.action = action;

    let new_overlay_e = new_overlay.map(|(e, _)| e);
    if state.shown_overlay != new_overlay_e {
        if let Some(old) = state.shown_overlay {
            if let Ok(mut n) = nodes.get_mut(old) {
                n.display = Display::None;
            }
        }
        state.shown_overlay = new_overlay_e;
    }
    if let Some((e, zone)) = new_overlay {
        if let Ok(mut n) = nodes.get_mut(e) {
            n.display = Display::Flex;
            set_zone_rect(&mut n, zone);
        }
    }

    let new_marker_e = new_marker.map(|(e, _)| e);
    if state.shown_marker != new_marker_e {
        if let Some(old) = state.shown_marker {
            if let Ok(mut n) = nodes.get_mut(old) {
                n.display = Display::None;
            }
        }
        state.shown_marker = new_marker_e;
    }
    if let Some((e, right)) = new_marker {
        if let Ok(mut n) = nodes.get_mut(e) {
            n.display = Display::Flex;
            if right {
                n.left = Val::Auto;
                n.right = Val::Px(-2.0);
            } else {
                n.left = Val::Px(-2.0);
                n.right = Val::Auto;
            }
        }
    }
}

fn spawn_ghost(commands: &mut Commands, font: &Handle<Font>, id: &str, cursor: Vec2) -> Entity {
    let (title, icon) = tab_meta(id);
    let ghost = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(cursor.x + 12.0),
                top: Val::Px(cursor.y + 12.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(TAB_ACTIVE_BG)),
            bevy::ui::GlobalZIndex(1000),
            bevy::ui::FocusPolicy::Pass,
            TabGhost,
            renzora::HideInHierarchy,
            Name::new("tab-ghost"),
        ))
        .id();
    let gi = glyph(commands, icon, TEXT_PRIMARY, 13.0);
    let gl = commands
        .spawn((
            Text::new(title),
            ui_font(font, 12.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();
    commands.entity(ghost).add_children(&[gi, gl]);
    ghost
}

fn pick_zone(x: f32, y: f32) -> DropZone {
    const EDGE: f32 = 0.25;
    if x < EDGE {
        DropZone::Left
    } else if x > 1.0 - EDGE {
        DropZone::Right
    } else if y < EDGE {
        DropZone::Top
    } else if y > 1.0 - EDGE {
        DropZone::Bottom
    } else {
        DropZone::Center
    }
}

fn set_zone_rect(n: &mut Node, zone: DropZone) {
    let (l, t, w, h) = match zone {
        DropZone::Center => (0.0, 0.0, 100.0, 100.0),
        DropZone::Left => (0.0, 0.0, 50.0, 100.0),
        DropZone::Right => (50.0, 0.0, 50.0, 100.0),
        DropZone::Top => (0.0, 0.0, 100.0, 50.0),
        DropZone::Bottom => (0.0, 50.0, 100.0, 50.0),
    };
    n.left = Val::Percent(l);
    n.top = Val::Percent(t);
    n.width = Val::Percent(w);
    n.height = Val::Percent(h);
}

/// The new tab order for a same-leaf reorder: `dragged` removed and re-inserted
/// before `before` (or appended).
fn reordered(old: &[String], dragged: &str, before: Option<&str>) -> Vec<String> {
    let mut v: Vec<String> = old.iter().filter(|t| t.as_str() != dragged).cloned().collect();
    match before.and_then(|b| v.iter().position(|t| t == b)) {
        Some(idx) => v.insert(idx, dragged.to_string()),
        None => v.push(dragged.to_string()),
    }
    v
}

fn apply_action(tree: &mut DockTree, dragged: &str, action: DropAction) {
    match action {
        DropAction::Split { rep, zone } => {
            if rep == dragged {
                return;
            }
            tree.remove_panel(dragged);
            tree.split_at(&rep, dragged.to_string(), zone);
        }
        DropAction::Tab { rep, before } => {
            tree.remove_panel(dragged);
            tree.add_tab_before(&rep, dragged.to_string(), before.as_deref());
        }
    }
}

/// (Re)build the dock subtree into the [`DockArea`] node when [`DockDirty`].
fn rebuild_dock(
    mut dirty: ResMut<DockDirty>,
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    dock: Res<Dock>,
    area: Query<(Entity, Option<&Children>), With<DockArea>>,
    leaves: Query<&DockLeaf>,
) {
    if !dirty.0 {
        return;
    }
    let Some(fonts) = fonts else {
        return;
    };
    let Ok((area_entity, children)) = area.single() else {
        return;
    };

    // Preserve each leaf's content entity (keyed by its active panel) and detach
    // it from the hierarchy so the despawn below doesn't take it — `build_tree`
    // re-parents it, so reordering/moving tabs keeps the panel (and its state)
    // instead of recreating it.
    let mut preserved: HashMap<String, Entity> = HashMap::new();
    for leaf in &leaves {
        if !leaf.active.is_empty() {
            preserved.insert(leaf.active.clone(), leaf.content);
            commands.entity(leaf.content).remove::<ChildOf>();
        }
    }

    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    let tree_root = build_tree(
        &mut commands,
        &fonts.ui,
        &fonts.phosphor,
        None,
        Vec::new(),
        &dock.tree,
        &mut preserved,
    );
    commands.entity(area_entity).add_child(tree_root);

    // Any preserved content not reused (its panel no longer exists) → despawn.
    for (_, content) in preserved.drain() {
        commands.entity(content).despawn();
    }
    dirty.0 = false;
}

/// Apply a pending tab switch in place: recolor label/icon and update the
/// leaf's `active` id (the consumer's content system reacts to that).
fn apply_tab_switch(
    mut pending: ResMut<PendingSwitch>,
    mut dock: ResMut<Dock>,
    tabs: Query<(Entity, &DockTab)>,
    mut leaves: Query<&mut DockLeaf>,
    mut colors: Query<&mut TextColor>,
) {
    let Some((leaf, id)) = pending.0.take() else {
        return;
    };
    dock.tree.set_active_tab(&id);

    for (_tab_entity, tab) in &tabs {
        if tab.leaf != leaf {
            continue;
        }
        let fg = rgb(if tab.id == id { TEXT_PRIMARY } else { TEXT_MUTED });
        if let Ok(mut c) = colors.get_mut(tab.label) {
            c.0 = fg;
        }
        if let Ok(mut c) = colors.get_mut(tab.icon) {
            c.0 = fg;
        }
    }

    if let Ok(mut ld) = leaves.get_mut(leaf) {
        ld.active = id;
    }
}

/// Tab background follows hover + active state (active wins).
fn tab_hover(dock: Res<Dock>, mut tabs: Query<(&Interaction, &DockTab, &mut BackgroundColor)>) {
    for (interaction, tab, mut bg) in &mut tabs {
        let target = if dock.tree.is_active_tab(&tab.id) {
            rgb(TAB_ACTIVE_BG)
        } else if matches!(interaction, Interaction::Hovered | Interaction::Pressed) {
            rgb(TAB_HOVER_BG)
        } else {
            Color::NONE
        };
        if bg.0 != target {
            bg.0 = target;
        }
    }
}

/// A tab's close × reddens while the cursor is over it.
fn tab_close_hover(
    mut closes: Query<(&bevy::ui::RelativeCursorPosition, &mut TextColor), With<TabClose>>,
) {
    for (rcp, mut color) in &mut closes {
        let target = rgb(if rcp.cursor_over { CLOSE_RED } else { TEXT_MUTED });
        if color.0 != target {
            color.0 = target;
        }
    }
}

// ── Reconciler ───────────────────────────────────────────────────────────────

fn build_tree(
    commands: &mut Commands,
    font: &Handle<Font>,
    phosphor: &Handle<Font>,
    parent: Option<ParentSplit>,
    path: Vec<bool>,
    tree: &DockTree,
    preserved: &mut HashMap<String, Entity>,
) -> Entity {
    match tree {
        DockTree::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            let row = matches!(direction, SplitDirection::Horizontal);
            let container = commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        min_width: Val::Px(0.0),
                        min_height: Val::Px(0.0),
                        flex_direction: if row {
                            FlexDirection::Row
                        } else {
                            FlexDirection::Column
                        },
                        position_type: PositionType::Relative,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    Name::new("split"),
                ))
                .id();

            let pct = ratio.clamp(0.1, 0.9) * 100.0;

            let mut wa = Node {
                overflow: Overflow::clip(),
                flex_shrink: 0.0,
                ..default()
            };
            if row {
                wa.width = Val::Percent(pct);
                wa.height = Val::Percent(100.0);
            } else {
                wa.height = Val::Percent(pct);
                wa.width = Val::Percent(100.0);
            }
            let wrap_a = commands.spawn((wa, Name::new("split-first"))).id();
            let mut path_a = path.clone();
            path_a.push(false);
            let info_a = ParentSplit {
                container,
                first_wrap: wrap_a,
                horizontal: row,
                is_second: false,
                path: path.clone(),
            };
            let child_a = build_tree(commands, font, phosphor, Some(info_a), path_a, first, preserved);
            commands.entity(wrap_a).add_child(child_a);

            let mut dv = Node {
                flex_shrink: 0.0,
                ..default()
            };
            if row {
                dv.width = Val::Px(1.0);
                dv.height = Val::Percent(100.0);
            } else {
                dv.height = Val::Px(1.0);
                dv.width = Val::Percent(100.0);
            }
            let divider = commands
                .spawn((dv, BackgroundColor(rgb(DIVIDER)), Name::new("divider")))
                .id();

            let mut wb = Node {
                overflow: Overflow::clip(),
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                // Without a zero minimum, this flex child's automatic min-size is
                // its (tall) content, so it inflates instead of shrinking to the
                // remaining space — tall panels then clip at the bottom with no
                // scroll. (The first child is capped by its explicit pct size.)
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                ..default()
            };
            if row {
                wb.height = Val::Percent(100.0);
            } else {
                wb.width = Val::Percent(100.0);
            }
            let wrap_b = commands.spawn((wb, Name::new("split-second"))).id();
            let mut path_b = path.clone();
            path_b.push(true);
            let info_b = ParentSplit {
                container,
                first_wrap: wrap_a,
                horizontal: row,
                is_second: true,
                path: path.clone(),
            };
            let child_b = build_tree(commands, font, phosphor, Some(info_b), path_b, second, preserved);
            commands.entity(wrap_b).add_child(child_b);

            const GRAB: f32 = 11.0;
            let mut hit = Node {
                position_type: PositionType::Absolute,
                ..default()
            };
            if row {
                hit.left = Val::Percent(pct);
                hit.margin = UiRect::left(Val::Px(-GRAB / 2.0));
                hit.width = Val::Px(GRAB);
                hit.top = Val::Px(0.0);
                hit.height = Val::Percent(100.0);
            } else {
                hit.top = Val::Percent(pct);
                hit.margin = UiRect::top(Val::Px(-GRAB / 2.0));
                hit.height = Val::Px(GRAB);
                hit.left = Val::Px(0.0);
                hit.width = Val::Percent(100.0);
            }
            let cursor = renzora_hui::cursor_icon::parse_cursor(if row {
                "ew-resize"
            } else {
                "ns-resize"
            })
            .unwrap();
            let handle = commands
                .spawn((
                    hit,
                    Interaction::default(),
                    renzora_hui::cursor_icon::HoverCursor(cursor),
                    Divider {
                        container,
                        first_wrap: wrap_a,
                        horizontal: row,
                        path: path.clone(),
                    },
                    Name::new("divider-handle"),
                ))
                .id();

            commands
                .entity(container)
                .add_children(&[wrap_a, divider, wrap_b, handle]);
            container
        }
        DockTree::Leaf { tabs, active_tab } => {
            build_leaf(commands, font, phosphor, parent, tabs, *active_tab, preserved)
        }
        DockTree::Empty => commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                Name::new("empty"),
            ))
            .id(),
    }
}

fn build_leaf(
    commands: &mut Commands,
    font: &Handle<Font>,
    phosphor: &Handle<Font>,
    parent: Option<ParentSplit>,
    tabs: &[String],
    active: usize,
    preserved: &mut HashMap<String, Entity>,
) -> Entity {
    let leaf = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                // Force a zero minimum so tall panel content can't push the
                // leaf's content-based min-size up and disturb sibling splits.
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(PANEL_BG)),
            bevy::ui::RelativeCursorPosition::default(),
            Name::new("leaf"),
        ))
        .id();
    populate_leaf(commands, font, phosphor, parent, leaf, tabs, active, preserved);
    leaf
}

/// The tab bar's `+` button — opens the Add-Panel picker for its leaf.
#[derive(Component)]
struct AddPanelButton {
    leaf: Entity,
}

/// Click the tab bar `+` → open the shared search overlay of panels not already
/// in that leaf; selecting one adds it as a tab (mirrors the egui panel picker).
fn add_panel_click(
    q: Query<(&Interaction, &AddPanelButton), Changed<Interaction>>,
    leaves: Query<&DockLeaf>,
    fonts: Option<Res<EmberFonts>>,
    registry: Option<Res<renzora::core::ShellPanelRegistry>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else {
        return;
    };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let leaf = btn.leaf;
        let existing: std::collections::HashSet<String> = leaves
            .get(leaf)
            .map(|l| l.tabs.iter().cloned().collect())
            .unwrap_or_default();
        let mut entries: Vec<crate::widgets::SearchEntry> = Vec::new();
        if let Some(reg) = &registry {
            let mut items: Vec<(&String, &renzora::core::ShellPanelInfo)> =
                reg.panels.iter().collect();
            items.sort_by(|a, b| a.1.title.cmp(&b.1.title));
            for (id, info) in items {
                if existing.contains(id.as_str()) {
                    continue;
                }
                let id = id.clone();
                let icon = if info.icon.is_empty() {
                    "circle".to_string()
                } else {
                    info.icon.clone()
                };
                let category = if info.category.is_empty() {
                    "General".to_string()
                } else {
                    info.category.clone()
                };
                entries.push(crate::widgets::SearchEntry::new(
                    icon,
                    info.title.clone(),
                    category,
                    move |w: &mut World| {
                        let sibling = w.get::<DockLeaf>(leaf).and_then(|l| l.tabs.first().cloned());
                        if let Some(mut dock) = w.get_resource_mut::<Dock>() {
                            match sibling {
                                Some(sib) => {
                                    dock.tree.add_tab(&sib, id.clone());
                                }
                                None => dock.tree = DockTree::leaf(id.clone()),
                            }
                        }
                        if let Some(mut d) = w.get_resource_mut::<DockDirty>() {
                            d.0 = true;
                        }
                    },
                ));
            }
        }
        crate::widgets::grid_overlay(&mut commands, &fonts, "Add Panel", entries);
    }
}

fn populate_leaf(
    commands: &mut Commands,
    font: &Handle<Font>,
    phosphor: &Handle<Font>,
    parent: Option<ParentSplit>,
    leaf: Entity,
    tabs: &[String],
    active: usize,
    preserved: &mut HashMap<String, Entity>,
) {
    let tabbar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(28.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(2.0),
                // Snug: tabs sit flush to the leaf edges (no outer padding).
                flex_shrink: 0.0,
                // Hairline separator under the header.
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(rgb(HEADER_BG)),
            BorderColor::all(rgb(DIVIDER)),
            // Subtle drop shadow so the header reads as lifted above the content.
            BoxShadow::new(
                Color::srgba(0.0, 0.0, 0.0, 0.30),
                Val::Px(0.0),
                Val::Px(2.0),
                Val::Px(0.0),
                Val::Px(4.0),
            ),
            bevy::ui::RelativeCursorPosition::default(),
            TabBarOf(leaf),
            Name::new("tabbar"),
        ))
        .id();

    let mut bar_kids: Vec<Entity> = Vec::new();
    for (i, id) in tabs.iter().enumerate() {
        let is_active = i == active;
        let fg = if is_active { TEXT_PRIMARY } else { TEXT_MUTED };
        let (title, icon) = tab_meta(id);
        let tab = commands
            .spawn((
                Node {
                    // Fill the full bar height so the active tab is snug to the
                    // top (content stays vertically centered via align_items).
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(5.0),
                    padding: UiRect::horizontal(Val::Px(9.0)),
                    position_type: PositionType::Relative,
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(if is_active {
                    rgb(TAB_ACTIVE_BG)
                } else {
                    Color::NONE
                }),
                Interaction::default(),
                bevy::ui::RelativeCursorPosition::default(),
                renzora_hui::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                Name::new(format!("tab:{id}")),
            ))
            .id();
        let tab_icon = icon_text(commands, phosphor, icon, fg, 13.0);
        let tab_label = commands
            .spawn((
                Text::new(title),
                ui_font(font, 12.0),
                TextColor(rgb(fg)),
                bevy::text::TextLayout::new_with_no_wrap(),
            ))
            .id();
        let close = icon_text(commands, phosphor, "x", TEXT_MUTED, 11.0);
        commands.entity(close).insert((
            TabClose,
            bevy::ui::RelativeCursorPosition::default(),
            bevy::ui::FocusPolicy::Pass,
        ));
        let marker = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(-2.0),
                    top: Val::Px(0.0),
                    height: Val::Percent(100.0),
                    width: Val::Px(2.0),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(rgb(ACCENT_BLUE)),
                bevy::ui::FocusPolicy::Pass,
                InsertMarker,
                Name::new("insert-marker"),
            ))
            .id();
        commands.entity(tab).insert(DockTab {
            id: id.clone(),
            leaf,
            label: tab_label,
            icon: tab_icon,
            marker,
        });
        commands
            .entity(tab)
            .add_children(&[tab_icon, tab_label, close, marker]);
        bar_kids.push(tab);
    }
    let add_btn = icon_text(commands, phosphor, "plus", TEXT_MUTED, 13.0);
    commands.entity(add_btn).insert((
        Interaction::default(),
        renzora_hui::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        AddPanelButton { leaf },
        Name::new("dock-add-panel"),
    ));
    bar_kids.push(add_btn);
    let filler = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: Val::Percent(100.0),
                ..default()
            },
            Name::new("tabbar-filler"),
        ))
        .id();
    if let Some(p) = parent.filter(|p| p.aligned()) {
        let cursor = renzora_hui::cursor_icon::parse_cursor(if p.horizontal {
            "ew-resize"
        } else {
            "ns-resize"
        })
        .unwrap();
        commands.entity(filler).insert((
            Interaction::default(),
            renzora_hui::cursor_icon::HoverCursor(cursor),
            Divider {
                container: p.container,
                first_wrap: p.first_wrap,
                horizontal: p.horizontal,
                path: p.path,
            },
        ));
    }
    bar_kids.push(filler);
    commands.entity(tabbar).add_children(&bar_kids);

    // Content region. Reuse the active panel's preserved content entity (kept
    // across this rebuild) so reordering/moving tabs doesn't recreate the panel;
    // otherwise spawn an empty node for the consumer to fill.
    let active_id = tabs.get(active).cloned().unwrap_or_default();
    let content = preserved.remove(&active_id).unwrap_or_else(|| {
        commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    // Zero minimum so the active panel's content size never
                    // reserves space in the leaf's column layout.
                    min_width: Val::Px(0.0),
                    min_height: Val::Px(0.0),
                    flex_basis: Val::Px(0.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                Name::new("content"),
            ))
            .id()
    });

    let overlay = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                border: UiRect::all(Val::Px(2.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(rgb(ACCENT_BLUE)),
            bevy::ui::FocusPolicy::Pass,
            DropOverlay,
            Name::new("drop-overlay"),
        ))
        .id();

    commands.entity(leaf).insert(DockLeaf {
        tabs: tabs.to_vec(),
        content,
        active: active_id,
        overlay,
    });
    commands
        .entity(leaf)
        .add_children(&[tabbar, content, overlay]);
}
