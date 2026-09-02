//! The two drag gestures: resizing a split by its divider, and moving a tab (or
//! a whole leaf) to re-dock it.
//!
//! Both work in physical **screen** space rather than window-local coordinates.
//! That is what makes them window-agnostic: the same code drives splits and
//! drops in the primary dock, in the pinned area and inside floating windows,
//! and a drag survives the cursor leaving the window it started in — which it
//! must, because the pressed window holds the mouse capture and no other window
//! receives cursor events until release.

use bevy::prelude::*;

use crate::font::{glyph, ui_font, EmberFonts};
use crate::theme::{rgb, tab_active, text_primary};

use crate::dock::components::{
    BottomSnap, BottomSnapRequest, Divider, DockLeaf, DockTab, GlobalCursor, GrabRootDivider,
    LeafGrip, RootDropOverlay, TabBarOf, TabGhost, BOTTOM_SNAP_PX,
};
use crate::dock::reconcile::TabScrollStrip;
use crate::dock::routing::{area_tree_mut, area_window, flag_area_dirty, window_contains, window_local};
use crate::dock::tree::{DockTree, DropZone, ROOT_DOCK_RATIO};
use crate::dock::windows::{close_empty_dock_window, tear_off_panel, DockWindowCloseRequests};
use crate::dock::{
    tab_meta, Dock, DockArea, DockDirty, DockWindowRequests, DockWindows, FixedDock,
    FloatingDockArea,
};

pub(crate) const TAB_DRAG_THRESHOLD: f32 = 6.0;

#[derive(Resource, Default)]
pub(crate) struct DraggedDivider(Option<DividerDrag>);

struct DividerDrag {
    handle: Entity,
    /// Physical screen-space grab point (from [`GlobalCursor`]) — screen space
    /// keeps the drag alive even when the cursor leaves the window mid-drag.
    start_cursor: Vec2,
    start_ratio: f32,
}

#[derive(Resource, Default)]
pub(crate) struct TabDrag(Option<TabDragState>);

/// Public seam letting an outside consumer (the editor shell) observe an
/// in-flight tab drag and divert its drop somewhere the dock knows nothing
/// about — e.g. dropping a panel on the workspace ribbon to spawn a new
/// workspace from it.
///
/// `dragging` carries the panel id once a drag passes the move threshold and is
/// cleared on release. A consumer that wants to handle the drop itself sets
/// `claim` (typically while the cursor is over its own drop target); on release
/// the dock then skips its own re-dock/tab-switch and leaves the panel to the
/// claimant, which is responsible for removing it from the live [`Dock`] tree.
/// The dock clears both fields on release.
#[derive(Resource, Default)]
pub struct DockDragWatch {
    /// Id of the panel currently being dragged (`None` when no active drag).
    pub dragging: Option<String>,
    /// Set by an external consumer to take ownership of the pending drop.
    pub claim: bool,
}

struct TabDragState {
    id: String,
    leaf: Entity,
    /// True when the drag started on the leaf's [`LeafGrip`]: the whole tab
    /// set moves as one unit instead of just `id` (which is then the leaf's
    /// active tab, used for the ghost and target resolution).
    group: bool,
    /// The dock area the drag started in — the tree the panel is removed from.
    source_area: Entity,
    /// The OS window hosting `source_area`. During the drag this window holds
    /// the mouse capture, so it's the only window whose cursor stays readable.
    source_window: Entity,
    /// Cursor at press, in the source window's logical coords.
    start_cursor: Vec2,
    active: bool,
    action: Option<DropAction>,
    ghost: Option<Entity>,
    shown_overlay: Option<Entity>,
    shown_marker: Option<Entity>,
}

/// Where a drop lands. Every variant carries the **target** dock area, which
/// may differ from the drag's source area (cross-window drops).
pub(crate) enum DropAction {
    Split { area: Entity, rep: String, zone: DropZone },
    /// Full-height / full-width split against the whole dock (edge/corner drop).
    RootSplit { area: Entity, zone: DropZone },
    Tab { area: Entity, rep: String, before: Option<String> },
}

impl DropAction {
    pub(crate) fn area(&self) -> Entity {
        match self {
            DropAction::Split { area, .. }
            | DropAction::RootSplit { area, .. }
            | DropAction::Tab { area, .. } => *area,
        }
    }
}

/// A pending in-place tab switch: (leaf entity, panel id to activate).
#[derive(Resource, Default)]
pub(crate) struct PendingSwitch(pub(crate) Option<(Entity, String)>);

fn as_ratio(v: Val) -> Option<f32> {
    if let Val::Percent(p) = v {
        Some(p / 100.0)
    } else {
        None
    }
}

/// Drag a divider handle to resize its split. Latches on press (continues off
/// the handle), moves by cursor delta (no snap), resizes live + persists.
///
/// Works in physical screen space ([`GlobalCursor`] vs the container's
/// physical size) so it's window-agnostic — the same code drives splits in the
/// primary dock and in floating dock windows, and the drag survives the cursor
/// briefly leaving the window (mouse capture keeps the move events coming).
pub(crate) fn divider_drag(
    mut dragged: ResMut<DraggedDivider>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Res<GlobalCursor>,
    dividers: Query<(Entity, &Interaction, &Divider)>,
    computed: Query<&bevy::ui::ComputedNode>,
    mut nodes: Query<&mut Node>,
    mut dock: ResMut<Dock>,
    mut fixed: ResMut<FixedDock>,
    mut wins: ResMut<DockWindows>,
    mut snap: ResMut<BottomSnapRequest>,
    mut grab: ResMut<GrabRootDivider>,
) {
    if mouse.just_released(MouseButton::Left) {
        dragged.0 = None;
        grab.0 = None;
    }
    let Some(cursor) = cursor.pos else {
        return;
    };

    // Adopt a consumer-initiated grab (the shell's drag-the-collapsed-strip-
    // open gesture): the mouse is already held, so take the vertical divider
    // at the grab's path — where the bottom region just re-attached — as the
    // live drag the moment the rebuilt tree provides it, no press on the
    // handle needed.
    if dragged.0.is_none() && mouse.pressed(MouseButton::Left) {
        if let Some(path) = grab.0.as_ref() {
            if let Some((entity, _, div)) = dividers
                .iter()
                .find(|(_, _, d)| !d.floating && !d.horizontal && &d.path == path)
            {
                let start_ratio = nodes
                    .get(div.first_wrap)
                    .ok()
                    .and_then(|n| as_ratio(n.height))
                    .unwrap_or(0.5);
                dragged.0 = Some(DividerDrag {
                    handle: entity,
                    start_cursor: cursor,
                    start_ratio,
                });
                grab.0 = None;
            }
        }
    }

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
    // Physical px on both sides of the division, so per-window scale factors
    // (mixed-DPI monitors) cancel out.
    let extent = if div.horizontal {
        cn.size().x
    } else {
        cn.size().y
    };
    if extent <= 0.0 {
        return;
    }
    let moved = if div.horizontal {
        cursor.x - drag.start_cursor.x
    } else {
        cursor.y - drag.start_cursor.y
    };
    let raw = drag.start_ratio + moved / extent;
    // Snap-closed gesture: overshooting a collapsible bottom region's divider
    // (the primary root vertical one, or a nested strip's — `strip`) far
    // enough that the bottom pane would drop under the threshold ends the
    // drag and asks the consumer (the shell's bottom panel) to collapse the
    // region — the clamp below would otherwise just pin the pane at 10%.
    if !div.horizontal
        && !div.floating
        && (div.path.is_empty() || div.strip.is_some())
        && (1.0 - raw) * extent < BOTTOM_SNAP_PX
    {
        snap.0 = Some(BottomSnap {
            restore: Some(drag.start_ratio),
            // The root region collapses content-agnostically; only a nested
            // strip needs the panel id to be found again.
            target: if div.path.is_empty() { None } else { div.strip.clone() },
        });
        dragged.0 = None;
        return;
    }
    let ratio = raw.clamp(0.1, 0.9);

    let first_wrap = div.first_wrap;
    let horizontal = div.horizontal;
    let dpath = div.path.clone();
    let darea = div.area;

    // Resize the first pane; the divider strip is a flex sibling, so flexbox
    // re-places it on the new boundary automatically — no handle to move.
    if let Ok(mut n) = nodes.get_mut(first_wrap) {
        if horizontal {
            n.width = Val::Percent(ratio * 100.0);
        } else {
            n.height = Val::Percent(ratio * 100.0);
        }
    }
    area_tree_mut(darea, &mut dock, &mut fixed, &mut wins).update_ratio(&dpath, ratio);
}

/// Drag a tab to re-dock; a plain click switches the active tab.
///
/// Multi-window aware:
/// - **Ctrl+drag** tears the panel off into a new floating OS window that
///   follows the cursor until release (see
///   [`crate::dock::DockWindowRequest::grab`]).
/// - A plain drag re-docks **across windows**: while the cursor is inside the
///   source window the usual `RelativeCursorPosition` machinery resolves the
///   drop; once it leaves (the pressed window holds the mouse capture, so no
///   other window receives cursor events) the drop target is resolved manually
///   from [`GlobalCursor`] against each window's leaf rects.
#[allow(clippy::too_many_arguments)]
pub(crate) fn tab_drag(
    mut drag: ResMut<TabDrag>,
    mut pending: ResMut<PendingSwitch>,
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    input: (
        Res<ButtonInput<MouseButton>>,
        Res<ButtonInput<KeyCode>>,
        Res<GlobalCursor>,
    ),
    windows: Query<&Window>,
    primary: Query<Entity, With<bevy::window::PrimaryWindow>>,
    tabs: Query<(Entity, &Interaction, &DockTab, &bevy::ui::RelativeCursorPosition)>,
    grips: Query<(&Interaction, &LeafGrip)>,
    // Bundled into one tuple param so `tab_drag` stays under Bevy's 16-param
    // system cap: `.0` = tab bars (drop-onto-bar targeting), `.1` = tab strips
    // (in-place reorder re-sorts the strip's tab children).
    bars: (
        Query<(Entity, &TabBarOf, &bevy::ui::RelativeCursorPosition)>,
        Query<(Entity, &TabScrollStrip)>,
    ),
    mut leaves: Query<(
        Entity,
        &mut DockLeaf,
        &bevy::ui::RelativeCursorPosition,
        &bevy::ui::ComputedNode,
        &bevy::ui::UiGlobalTransform,
    )>,
    areas: Query<
        (
            Entity,
            &bevy::ui::RelativeCursorPosition,
            &bevy::ui::ComputedNode,
            Option<&FloatingDockArea>,
        ),
        With<DockArea>,
    >,
    root_overlays: Query<(Entity, &RootDropOverlay)>,
    mut nodes: Query<&mut Node>,
    model: (
        ResMut<Dock>,
        ResMut<FixedDock>,
        ResMut<DockDirty>,
        ResMut<DockWindows>,
        ResMut<DockWindowRequests>,
        ResMut<DockWindowCloseRequests>,
    ),
    mut watch: ResMut<DockDragWatch>,
) {
    let (mouse, keys, global) = input;
    let (mut dock, mut fixed, mut dirty, mut wins, mut requests, mut close_queue) = model;

    if drag.0.is_none() && mouse.just_pressed(MouseButton::Left) {
        // A grip press outranks a tab press (the grip sits inside the bar but
        // not inside a tab, so both can't be Pressed at once anyway).
        let pressed = grips
            .iter()
            .find(|(i, _)| **i == Interaction::Pressed)
            .and_then(|(_, grip)| {
                let (_, ld, ..) = leaves.get(grip.leaf).ok()?;
                // Drag the whole tab set; `id` is the active tab (or the
                // first) — it stands in for the leaf in target resolution.
                let id = if ld.tabs.iter().any(|t| t == &ld.active) {
                    ld.active.clone()
                } else {
                    ld.tabs.first()?.clone()
                };
                Some((id, grip.leaf, true))
            })
            .or_else(|| {
                tabs.iter()
                    .find(|(_, i, ..)| **i == Interaction::Pressed)
                    .map(|(_, _, tab, _)| (tab.id.clone(), tab.leaf, false))
            });
        if let Some((id, leaf_e, group)) = pressed {
            if let Ok((_, ld, ..)) = leaves.get(leaf_e) {
                let source_area = ld.area;
                if let Some(source_window) =
                    area_window(source_area, &wins, primary.single().ok())
                {
                    if let Some(cur) = windows
                        .get(source_window)
                        .ok()
                        .and_then(|w| w.cursor_position())
                    {
                        drag.0 = Some(TabDragState {
                            id,
                            leaf: leaf_e,
                            group,
                            source_area,
                            source_window,
                            start_cursor: cur,
                            active: false,
                            action: None,
                            ghost: None,
                            shown_overlay: None,
                            shown_marker: None,
                        });
                    }
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
            // An external consumer (e.g. the workspace ribbon) claimed this drop:
            // it owns moving the panel, so the dock does nothing here — no re-dock,
            // no tab switch. Clear the watch and bail.
            if watch.claim {
                watch.dragging = None;
                watch.claim = false;
                return;
            }
            if state.active {
                if let Some(action) = state.action {
                    let target_area = action.area();
                    let same_area = target_area == state.source_area;
                    // Group drag: take the WHOLE leaf out (tab set + active
                    // index travel together) and insert it per the action.
                    // Target resolution never offers the source leaf itself
                    // (see the `group` skips below), so the take can't
                    // invalidate the drop target.
                    if state.group {
                        let taken = area_tree_mut(state.source_area, &mut dock, &mut fixed, &mut wins)
                            .take_leaf_containing(&state.id);
                        if let Some(leaf_tree) = taken {
                            let target = area_tree_mut(target_area, &mut dock, &mut fixed, &mut wins);
                            insert_leaf_action(target, leaf_tree, &action);
                            flag_area_dirty(target_area, &mut dirty, &mut fixed, &mut wins);
                            if !same_area {
                                flag_area_dirty(state.source_area, &mut dirty, &mut fixed, &mut wins);
                                close_empty_dock_window(
                                    state.source_area,
                                    &wins,
                                    &mut close_queue,
                                );
                            }
                        }
                        watch.dragging = None;
                        watch.claim = false;
                        return;
                    }
                    // A reorder within the same leaf only changes tab order, not
                    // structure — do it in place (move the tab entity) instead of
                    // rebuilding the dock subtree, which avoids the layout flicker.
                    let inplace = match &action {
                        DropAction::Tab { rep, before, .. } if same_area => leaves
                            .get(state.leaf)
                            .ok()
                            .filter(|(_, ld, ..)| ld.tabs.contains(rep))
                            .map(|(_, ld, ..)| reordered(&ld.tabs, &state.id, before.as_deref())),
                        _ => None,
                    };
                    // Move the panel: out of the source tree, into the target's
                    // (the same tree when the drop stayed in-window).
                    area_tree_mut(state.source_area, &mut dock, &mut fixed, &mut wins)
                        .remove_panel(&state.id);
                    insert_action(
                        area_tree_mut(target_area, &mut dock, &mut fixed, &mut wins),
                        &state.id,
                        &action,
                    );
                    match inplace {
                        Some(new_tabs) => {
                            if let Ok((_, mut ld, ..)) = leaves.get_mut(state.leaf) {
                                ld.tabs = new_tabs.clone();
                            }
                            // Re-sort the *strip's* children (the tabs) — not the
                            // tab bar's, which also holds the grip, `+`, filler,
                            // and collapse chevron a wholesale reorder would drop.
                            if let Some((strip, _)) =
                                bars.1.iter().find(|(_, s)| s.leaf == state.leaf)
                            {
                                let ordered: Vec<Entity> = new_tabs
                                    .iter()
                                    .filter_map(|id| {
                                        tabs.iter()
                                            .find(|(_, _, t, _)| &t.id == id)
                                            .map(|(e, _, _, _)| e)
                                    })
                                    .collect();
                                // `replace_children`, not `insert_children(0, …)`:
                                // reordering existing children via Bevy 0.19's
                                // `place` can panic ("insertion index should be
                                // <= len") because it clamps the index before
                                // removing the moved child. `replace_children`
                                // rebuilds the collection wholesale and avoids it.
                                if !ordered.is_empty() {
                                    commands.entity(strip).replace_children(&ordered);
                                }
                            }
                        }
                        None => {
                            flag_area_dirty(target_area, &mut dirty, &mut fixed, &mut wins);
                            if !same_area {
                                flag_area_dirty(state.source_area, &mut dirty, &mut fixed, &mut wins);
                                // Dragging the last panel out of a floating
                                // window leaves an empty shell — close it.
                                close_empty_dock_window(
                                    state.source_area,
                                    &wins,
                                    &mut close_queue,
                                );
                            }
                        }
                    }
                }
            } else if !state.group {
                pending.0 = Some((state.leaf, state.id));
            }
        }
        watch.dragging = None;
        watch.claim = false;
        return;
    }

    let Some(state) = drag.0.as_mut() else {
        return;
    };
    // The source window's cursor — `None` once the cursor leaves its bounds
    // (the drag keeps running on [`GlobalCursor`] then).
    let cursor = windows
        .get(state.source_window)
        .ok()
        .and_then(|w| w.cursor_position());

    if !state.active {
        let Some(cur) = cursor else {
            return;
        };
        if cur.distance(state.start_cursor) <= TAB_DRAG_THRESHOLD {
            return;
        }
        // Ctrl+drag: tear the panel off into a floating OS window that follows
        // the cursor until release, instead of a re-dock drag. Not for group
        // drags — floating windows are chromeless single-panel hosts, so a
        // whole tab set torn off would leave its background tabs unreachable.
        if !state.group
            && (keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight))
        {
            let id = state.id.clone();
            let scale = windows
                .get(state.source_window)
                .map(|w| w.scale_factor())
                .unwrap_or(1.0);
            let leaf_size = leaves.get(state.leaf).map(|(_, _, _, cn, _)| cn.size()).ok();
            tear_off_panel(
                &id,
                state.source_area,
                leaf_size,
                scale,
                global.pos,
                true,
                (&mut dock, &mut fixed, &mut dirty, &mut wins, &mut requests, &mut close_queue),
            );
            drag.0 = None;
            watch.dragging = None;
            watch.claim = false;
            return;
        }
        state.active = true;
        // Publish the dragged panel id so external drop targets (the workspace
        // ribbon) can react while the drag is in flight. Group drags don't
        // publish — the ribbon's claim moves a single panel, which would tear
        // the group apart.
        if !state.group {
            watch.dragging = Some(state.id.clone());
        }
        if let Some(fonts) = &fonts {
            // A drag from a floating window renders its ghost on that window's
            // camera (a root node with no target lands on the primary window).
            let camera = wins
                .0
                .iter()
                .find(|s| s.area == state.source_area)
                .map(|s| s.camera);
            // Group ghosts show how many background tabs ride along.
            let extra = if state.group {
                leaves
                    .get(state.leaf)
                    .map(|(_, ld, ..)| ld.tabs.len().saturating_sub(1))
                    .unwrap_or(0)
            } else {
                0
            };
            state.ghost = Some(spawn_ghost(&mut commands, &fonts.ui, &state.id, extra, cur, camera));
        }
    }

    if let Some(ghost) = state.ghost {
        if let Ok(mut n) = nodes.get_mut(ghost) {
            match cursor {
                Some(cur) => {
                    n.display = Display::Flex;
                    n.left = Val::Px(cur.x + 12.0);
                    n.top = Val::Px(cur.y + 12.0);
                }
                // Cursor outside the source window (cross-window drag) — the
                // ghost would pin to a stale spot, so hide it.
                None => n.display = Display::None,
            }
        }
    }

    let mut action: Option<DropAction> = None;
    // (overlay entity, zone, is_root) — root hits use the area-wide overlay
    // with `set_root_zone_rect`, leaf hits the leaf overlay with `set_zone_rect`.
    let mut new_overlay: Option<(Entity, DropZone, bool)> = None;
    let mut new_marker: Option<(Entity, bool)> = None;

    if cursor.is_some() {
        // ── In-window drop resolution (`RelativeCursorPosition` machinery).
        // Only nodes in the cursor's window report `cursor_over`, so this
        // naturally scopes to the window the drag is currently inside.

        // Root edge/corner hit, in dock-local logical px. Corners outrank tab
        // bars (the top edge is lined with them); plain edge bands don't.
        let root_hit = areas
            .iter()
            .find(|(_, rcp, _, _)| rcp.cursor_over)
            .and_then(|(area_e, rcp, computed, _)| {
                let norm = rcp.normalized?;
                let size = computed.size() * computed.inverse_scale_factor();
                let (zone, corner) =
                    pick_root_zone((norm.x + 0.5) * size.x, (norm.y + 0.5) * size.y, size)?;
                Some((area_e, zone, corner))
            });
        let overlay_for = |area_e: Entity| {
            root_overlays
                .iter()
                .find(|(_, o)| o.area == area_e)
                .map(|(e, _)| e)
        };

        let over_bar = bars
            .0
            .iter()
            .find(|(_, _, rcp)| rcp.cursor_over)
            .map(|(_, bar, _)| bar.0);
        if let Some((area_e, zone, overlay)) = root_hit
            .filter(|(_, _, corner)| *corner)
            .and_then(|(a, z, _)| overlay_for(a).map(|o| (a, z, o)))
        {
            action = Some(DropAction::RootSplit { area: area_e, zone });
            new_overlay = Some((overlay, zone, true));
        } else if let Some(leaf_ent) =
            // A group drag can't drop on its own bar — the "target" is the
            // very leaf being moved.
            over_bar.filter(|e| !(state.group && *e == state.leaf))
        {
            if let Some((_, ld, ..)) = leaves.iter().find(|(e, ..)| *e == leaf_ent) {
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
                    action = Some(DropAction::Tab {
                        area: ld.area,
                        rep,
                        before,
                    });
                    new_marker = marker;
                }
            }
        } else if let Some((area_e, zone, overlay)) =
            root_hit.and_then(|(a, z, _)| overlay_for(a).map(|o| (a, z, o)))
        {
            action = Some(DropAction::RootSplit { area: area_e, zone });
            new_overlay = Some((overlay, zone, true));
        } else {
            for (leaf_e, ld, rcp, ..) in &leaves {
                if rcp.cursor_over {
                    // A group drag over its own leaf has no valid zone —
                    // splitting a leaf against itself is a no-op at best.
                    if state.group && leaf_e == state.leaf {
                        break;
                    }
                    if let Some(norm) = rcp.normalized {
                        let (x, y) = (norm.x + 0.5, norm.y + 0.5);
                        let zone = pick_zone(x, y);
                        if let Some(rep) = ld.tabs.iter().find(|t| **t != state.id).cloned() {
                            action = Some(if matches!(zone, DropZone::Center) {
                                DropAction::Tab {
                                    area: ld.area,
                                    rep,
                                    before: None,
                                }
                            } else {
                                DropAction::Split {
                                    area: ld.area,
                                    rep,
                                    zone,
                                }
                            });
                            new_overlay = Some((ld.overlay, zone, false));
                        }
                    }
                    break;
                }
            }
        }
    } else if let Some(gpos) = global.pos {
        // ── Cross-window drop resolution. The source window holds the mouse
        // capture, so windows under the cursor receive no cursor events and
        // their `RelativeCursorPosition` never updates — hit-test dock leaves
        // manually in physical screen space instead. Only the PRIMARY window
        // is a target: floating windows are chromeless single-panel hosts (no
        // tab bar), so extra tabs dropped into one would be unreachable.
        let mut target: Option<(Entity, Entity)> = None; // (window, area)
        if let Ok(pw) = primary.single() {
            if pw != state.source_window
                && windows.get(pw).is_ok_and(|w| window_contains(w, gpos))
            {
                if let Some((area_e, ..)) = areas.iter().find(|(.., f)| f.is_none()) {
                    target = Some((pw, area_e));
                }
            }
        }
        if let Some((win_e, area_e)) = target {
            if let Some(local) = windows.get(win_e).ok().and_then(|w| window_local(w, gpos)) {
                for (_, ld, _, cn, gt) in &leaves {
                    if ld.area != area_e || !cn.contains_point(*gt, local) {
                        continue;
                    }
                    if let Some(norm) = cn.normalize_point(*gt, local) {
                        let zone = pick_zone(norm.x + 0.5, norm.y + 0.5);
                        if let Some(rep) = ld.tabs.iter().find(|t| **t != state.id).cloned() {
                            action = Some(if matches!(zone, DropZone::Center) {
                                DropAction::Tab {
                                    area: area_e,
                                    rep,
                                    before: None,
                                }
                            } else {
                                DropAction::Split {
                                    area: area_e,
                                    rep,
                                    zone,
                                }
                            });
                            new_overlay = Some((ld.overlay, zone, false));
                        }
                    }
                    break;
                }
            }
        }
    }
    state.action = action;

    let new_overlay_e = new_overlay.map(|(e, _, _)| e);
    if state.shown_overlay != new_overlay_e {
        if let Some(old) = state.shown_overlay {
            if let Ok(mut n) = nodes.get_mut(old) {
                n.display = Display::None;
            }
        }
        state.shown_overlay = new_overlay_e;
    }
    if let Some((e, zone, is_root)) = new_overlay {
        if let Ok(mut n) = nodes.get_mut(e) {
            n.display = Display::Flex;
            if is_root {
                set_root_zone_rect(&mut n, zone);
            } else {
                set_zone_rect(&mut n, zone);
            }
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

fn spawn_ghost(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    id: &str,
    extra: usize,
    cursor: Vec2,
    camera: Option<Entity>,
) -> Entity {
    let (mut title, icon) = tab_meta(id);
    // A whole-leaf drag: show how many background tabs travel with it.
    if extra > 0 {
        title = format!("{title} +{extra}");
    }
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
            BackgroundColor(rgb(tab_active())),
            bevy::ui::GlobalZIndex(1000),
            bevy::ui::FocusPolicy::Pass,
            TabGhost,
            renzora::HideInHierarchy,
            Name::new("tab-ghost"),
        ))
        .id();
    // Ghosts for drags out of a floating dock window render on that window's
    // camera; without a target a root node lands on the primary window.
    if let Some(camera) = camera {
        commands.entity(ghost).insert(bevy::ui::UiTargetCamera(camera));
    }
    let gi = glyph(commands, icon, text_primary(), 13.0);
    let gl = commands
        .spawn((
            Text::new(title),
            ui_font(font, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    commands.entity(ghost).add_children(&[gi, gl]);
    ghost
}

/// Corner squares this big (logical px) at the dock's four corners always
/// root-dock — even over a tab bar, so the gesture stays reachable along the
/// top edge. Inside a corner, the nearer edge wins: closer to the side →
/// full-height column, closer to the top/bottom → full-width row.
const ROOT_CORNER_PX: f32 = 48.0;
/// Thin bands along the dock's left/right edges that also root-dock (lower
/// priority than tab bars). No top band: the topmost tab bars line that edge
/// and tab drops there are far more common — the top corners still give
/// access to a full-width top dock.
const ROOT_EDGE_BAND_PX: f32 = 16.0;
/// The bottom band is much taller than the side bands: "drag it to the bottom"
/// is the gesture that docks a panel (or a torn-off floating window) into the
/// full-width bottom panel, so it must be an easy target — nothing else
/// competes for the dock's bottom edge the way tab bars compete for the top.
const ROOT_BOTTOM_BAND_PX: f32 = 48.0;

/// Root edge/corner hit-test over the whole dock area. `(x, y)` is the cursor
/// in dock-local logical px, `size` the dock's logical size. Returns the root
/// zone and whether it was a corner hit (corners outrank tab bars).
pub(crate) fn pick_root_zone(x: f32, y: f32, size: Vec2) -> Option<(DropZone, bool)> {
    if x < 0.0 || y < 0.0 || x > size.x || y > size.y {
        return None;
    }
    let (dl, dr) = (x, size.x - x);
    let (dt, db) = (y, size.y - y);
    let dx = dl.min(dr);
    let dy = dt.min(db);
    let zone_x = if dl <= dr { DropZone::Left } else { DropZone::Right };
    let zone_y = if dt <= db { DropZone::Top } else { DropZone::Bottom };
    if dx <= ROOT_CORNER_PX && dy <= ROOT_CORNER_PX {
        // Corner: the diagonal decides between the column and the row.
        return Some((if dx <= dy { zone_x } else { zone_y }, true));
    }
    if dx <= ROOT_EDGE_BAND_PX {
        return Some((zone_x, false));
    }
    if dy <= ROOT_BOTTOM_BAND_PX && matches!(zone_y, DropZone::Bottom) {
        return Some((zone_y, false));
    }
    None
}

/// Preview rect for a root edge drop — mirrors the `ROOT_DOCK_RATIO` the split
/// will actually use, so the highlight shows the real resulting area.
pub(crate) fn set_root_zone_rect(n: &mut Node, zone: DropZone) {
    let r = ROOT_DOCK_RATIO * 100.0;
    let (l, t, w, h) = match zone {
        DropZone::Center => (0.0, 0.0, 100.0, 100.0),
        DropZone::Left => (0.0, 0.0, r, 100.0),
        DropZone::Right => (100.0 - r, 0.0, r, 100.0),
        DropZone::Top => (0.0, 0.0, 100.0, r),
        DropZone::Bottom => (0.0, 100.0 - r, 100.0, r),
    };
    n.left = Val::Percent(l);
    n.top = Val::Percent(t);
    n.width = Val::Percent(w);
    n.height = Val::Percent(h);
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

pub(crate) fn set_zone_rect(n: &mut Node, zone: DropZone) {
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

/// Insert `dragged` into `tree` per `action`. The removal from the source tree
/// already happened (possibly in a *different* window's tree — cross-window
/// drops split the old remove+insert into two tree mutations). Total: if the
/// stated sibling can't take the panel, it's adopted somewhere in the tree
/// rather than silently dropped.
pub(crate) fn insert_action(tree: &mut DockTree, dragged: &str, action: &DropAction) {
    match action {
        DropAction::Split { rep, zone, .. } => {
            if rep == dragged || !tree.split_at(rep, dragged.to_string(), *zone) {
                tree.adopt_panel(dragged);
            }
        }
        DropAction::RootSplit { zone, .. } => tree.split_root(dragged.to_string(), *zone),
        DropAction::Tab { rep, before, .. } => {
            if rep == dragged || !tree.add_tab_before(rep, dragged.to_string(), before.as_deref())
            {
                tree.adopt_panel(dragged);
            }
        }
    }
}

/// Insert a whole dragged leaf (`leaf_tree`) into `tree` per `action` — the
/// group-drag counterpart of [`insert_action`]. Total in the same way: if the
/// stated sibling can't take the subtree, every panel in it is adopted
/// somewhere in the tree rather than silently dropped.
fn insert_leaf_action(tree: &mut DockTree, leaf_tree: DockTree, action: &DropAction) {
    let adopt_all = |tree: &mut DockTree, sub: &DockTree| {
        let mut ids = Vec::new();
        sub.collect_panels(&mut ids);
        for id in ids {
            tree.adopt_panel(&id);
        }
    };
    match action {
        DropAction::Split { rep, zone, .. } => {
            if !tree.split_at_with(rep, leaf_tree.clone(), *zone) {
                adopt_all(tree, &leaf_tree);
            }
        }
        DropAction::RootSplit { zone, .. } => tree.split_root_with(leaf_tree, *zone),
        DropAction::Tab { rep, before, .. } => {
            let merged = match &leaf_tree {
                DockTree::Leaf { tabs, .. } => {
                    tree.add_tabs_before(rep, tabs, before.as_deref())
                }
                _ => false,
            };
            if !merged {
                adopt_all(tree, &leaf_tree);
            }
        }
    }
}
