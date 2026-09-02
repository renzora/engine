//! Everything a pointer does to a dock that isn't a drag: switching tabs,
//! tracking focus, hovering, closing a tab, collapsing a bottom region, and the
//! two undock affordances (a tab's grip handle and its right-click menu).

use bevy::prelude::*;

use crate::font::EmberFonts;
use crate::theme::{close_red, rgb, text_muted, text_primary};

use crate::dock::components::{
    BottomCollapseBtn, BottomSnap, BottomSnapRequest, DockLeaf, DockTab, FocusPanelRequest,
    FocusedPanel, GlobalCursor, TabClose,
};
use crate::dock::drag::PendingSwitch;
use crate::dock::reconcile::TabGrip;
use crate::dock::routing::{area_tree_mut, area_window, flag_area_dirty};
use crate::dock::tree::DockTree;
use crate::dock::windows::{
    close_empty_dock_window, tear_off_panel, DockWindowCloseRequests, DockWindowRequest,
    DockWindowRequests, DockWindows, FLOAT_TITLEBAR_H,
};
use crate::dock::{Dock, DockDirty, FixedDock};

/// Show each tab's undock handle while its tab (or the handle itself — once
/// shown, it blocks the tab's own hover) is hovered; hide it otherwise so
/// unhovered tabs keep their exact look and hit-testing.
pub(crate) fn tab_grip_hover(
    grips: Query<(Entity, &Interaction, &TabGrip)>,
    tabs: Query<&Interaction, With<DockTab>>,
    mut nodes: Query<&mut Node>,
) {
    for (grip_entity, grip_interaction, grip) in &grips {
        let hovered = matches!(grip_interaction, Interaction::Hovered | Interaction::Pressed)
            || tabs
                .get(grip.tab)
                .is_ok_and(|i| matches!(i, Interaction::Hovered | Interaction::Pressed));
        if let Ok(mut node) = nodes.get_mut(grip_entity) {
            let display = if hovered { Display::Flex } else { Display::None };
            if node.display != display {
                node.display = display;
            }
        }
    }
}

/// Press a tab's undock handle → tear that panel off into a floating window
/// that follows the cursor until release (the same gesture as Ctrl+dragging
/// the tab, without the modifier).
#[allow(clippy::too_many_arguments)]
pub(crate) fn tab_grip_interact(
    grips: Query<(&Interaction, &TabGrip), Changed<Interaction>>,
    leaves: Query<&bevy::ui::ComputedNode, With<DockLeaf>>,
    windows: Query<&Window>,
    primary: Query<Entity, With<bevy::window::PrimaryWindow>>,
    global: Res<GlobalCursor>,
    mut dock: ResMut<Dock>,
    mut fixed: ResMut<FixedDock>,
    mut dirty: ResMut<DockDirty>,
    mut wins: ResMut<DockWindows>,
    mut requests: ResMut<DockWindowRequests>,
    mut close_queue: ResMut<DockWindowCloseRequests>,
) {
    for (i, grip) in &grips {
        if *i != Interaction::Pressed {
            continue;
        }
        let scale = area_window(grip.area, &wins, primary.single().ok())
            .and_then(|w| windows.get(w).ok())
            .map(|w| w.scale_factor())
            .unwrap_or(1.0);
        let leaf_size = leaves.get(grip.leaf).map(|cn| cn.size()).ok();
        tear_off_panel(
            &grip.id.clone(),
            grip.area,
            leaf_size,
            scale,
            global.pos,
            true,
            (&mut dock, &mut fixed, &mut dirty, &mut wins, &mut requests, &mut close_queue),
        );
    }
}

/// Right-click a tab → context menu with **Undock**: tears that panel off into
/// a floating window opened under the cursor (no drag required).
pub(crate) fn tab_context_menu(
    mouse: Res<ButtonInput<MouseButton>>,
    fonts: Option<Res<EmberFonts>>,
    windows: Query<&Window>,
    primary: Query<Entity, With<bevy::window::PrimaryWindow>>,
    wins: Res<DockWindows>,
    tabs: Query<(&DockTab, &bevy::ui::RelativeCursorPosition)>,
    leaves: Query<(&DockLeaf, &bevy::ui::ComputedNode)>,
    global: Res<GlobalCursor>,
    mut commands: Commands,
) {
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    let Some(fonts) = fonts else {
        return;
    };
    for (tab, rcp) in &tabs {
        if !rcp.cursor_over {
            continue;
        }
        let Ok((ld, cn)) = leaves.get(tab.leaf) else {
            break;
        };
        // Menu coordinates are the tab's window's logical cursor (tabs only
        // exist in the primary window — floats are chromeless).
        let win = area_window(ld.area, &wins, primary.single().ok())
            .and_then(|w| windows.get(w).ok());
        let Some(cur) = win.and_then(|w| w.cursor_position()) else {
            break;
        };
        let scale = win.map(|w| w.scale_factor()).unwrap_or(1.0);
        let id = tab.id.clone();
        let area = ld.area;
        let leaf_size = cn.size();
        let gpos = global.pos;

        // Cloned for the second (Close) menu item, since the Undock closure
        // moves `id`. `area` is `Copy`, so it stays available to both.
        let close_id = id.clone();
        let menu = crate::widgets::screen_menu(&mut commands, cur.x, cur.y);
        let undock = crate::widgets::menu_item(
            &mut commands,
            &fonts,
            "arrow-square-out",
            &renzora::lang::t_or("menu.undock", "Undock"),
            move |w: &mut World| {
                // World-side mirror of `tear_off_panel` (menu items run as
                // world closures). Route to the tree owning the leaf's area —
                // in practice the primary dock, since floats have no tabs.
                let floating = w
                    .get_resource::<DockWindows>()
                    .and_then(|ws| ws.0.iter().position(|s| s.area == area));
                match floating {
                    Some(idx) => {
                        if let Some(mut ws) = w.get_resource_mut::<DockWindows>() {
                            ws.0[idx].tree.remove_panel(&id);
                            ws.0[idx].dirty = true;
                        }
                    }
                    None => {
                        if let Some(mut dock) = w.get_resource_mut::<Dock>() {
                            dock.tree.remove_panel(&id);
                        }
                        if let Some(mut d) = w.get_resource_mut::<DockDirty>() {
                            d.0 = true;
                        }
                    }
                }
                let size = UVec2::new(
                    leaf_size.x.clamp(240.0 * scale, 1600.0 * scale) as u32,
                    (leaf_size.y + FLOAT_TITLEBAR_H * scale).clamp(160.0 * scale, 1200.0 * scale)
                        as u32,
                );
                let position = gpos.map(|p| {
                    (p - Vec2::new(60.0, FLOAT_TITLEBAR_H * 0.5) * scale).round().as_ivec2()
                });
                if let Some(mut req) = w.get_resource_mut::<DockWindowRequests>() {
                    req.0.push(DockWindowRequest {
                        tree: DockTree::leaf(id.clone()),
                        position,
                        size,
                        grab: false,
                    });
                }
            },
        );
        // Close: remove the panel from whichever tree owns its area, then close
        // the floating window if that emptied it — the world-side mirror of
        // `tab_close_click` (the per-tab × button).
        let close = crate::widgets::menu_item(
            &mut commands,
            &fonts,
            "x",
            &renzora::lang::t_or("menu.close_panel", "Close panel"),
            move |w: &mut World| {
                let floating = w
                    .get_resource::<DockWindows>()
                    .and_then(|ws| ws.0.iter().position(|s| s.area == area));
                let mut emptied = None;
                match floating {
                    Some(idx) => {
                        if let Some(mut ws) = w.get_resource_mut::<DockWindows>() {
                            if ws.0[idx].tree.remove_panel(&close_id) {
                                ws.0[idx].dirty = true;
                                if ws.0[idx].tree.is_empty() {
                                    emptied = Some(ws.0[idx].window);
                                }
                            }
                        }
                    }
                    None => {
                        if let Some(mut dock) = w.get_resource_mut::<Dock>() {
                            dock.tree.remove_panel(&close_id);
                        }
                        if let Some(mut d) = w.get_resource_mut::<DockDirty>() {
                            d.0 = true;
                        }
                    }
                }
                if let Some(win) = emptied {
                    if let Some(mut closes) = w.get_resource_mut::<DockWindowCloseRequests>() {
                        closes.0.push(win);
                    }
                }
            },
        );
        commands.entity(menu).add_children(&[undock, close]);
        break;
    }
}

/// Track which panel the user last clicked into as [`FocusedPanel`]. A left
/// press over a leaf makes that leaf's visible panel the focused one. Panel
/// content nodes block picking (`FocusPolicy::Block`), so leaf `Interaction`
/// is unreliable here — we test the leaf's own `RelativeCursorPosition`, which
/// is true whenever the cursor is anywhere inside that leaf. Tab switches and
/// programmatic focus update [`FocusedPanel`] separately (see
/// [`apply_tab_switch`] / [`apply_focus_request`]).
pub(crate) fn track_focused_panel(
    mouse: Res<ButtonInput<MouseButton>>,
    mut focused: ResMut<FocusedPanel>,
    leaves: Query<(&DockLeaf, &bevy::ui::RelativeCursorPosition)>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    for (leaf, rcp) in &leaves {
        if rcp.cursor_over {
            if focused.0.as_deref() != Some(leaf.active.as_str()) {
                focused.0 = Some(leaf.active.clone());
            }
            return;
        }
    }
}

/// Turn a [`FocusPanelRequest`] into a pending tab switch: locate the leaf that
/// holds the requested panel and, if it isn't already the active tab, queue the
/// same in-place switch a click would. Runs just before [`apply_tab_switch`] so
/// the switch lands the same frame.
pub(crate) fn apply_focus_request(
    mut req: ResMut<FocusPanelRequest>,
    mut pending: ResMut<PendingSwitch>,
    mut focused: ResMut<FocusedPanel>,
    leaves: Query<(Entity, &DockLeaf)>,
) {
    let Some(id) = req.0.take() else {
        return;
    };
    for (entity, leaf) in &leaves {
        if leaf.tabs.iter().any(|t| t == &id) {
            if leaf.active != id {
                pending.0 = Some((entity, id.clone()));
            }
            // Programmatic focus counts as focusing the panel even when it's
            // already the active tab (no switch queued).
            focused.0 = Some(id);
            return;
        }
    }
}

/// Apply a pending tab switch in place: recolor label/icon and update the
/// leaf's `active` id (the consumer's content system reacts to that).
pub(crate) fn apply_tab_switch(
    mut pending: ResMut<PendingSwitch>,
    mut dock: ResMut<Dock>,
    mut fixed: ResMut<FixedDock>,
    mut wins: ResMut<DockWindows>,
    mut focused: ResMut<FocusedPanel>,
    tabs: Query<(Entity, &DockTab)>,
    mut leaves: Query<&mut DockLeaf>,
    mut colors: Query<&mut TextColor>,
) {
    let Some((leaf, id)) = pending.0.take() else {
        return;
    };
    // Switching to a tab focuses it.
    focused.0 = Some(id.clone());
    if let Ok(ld) = leaves.get(leaf) {
        area_tree_mut(ld.area, &mut dock, &mut fixed, &mut wins).set_active_tab(&id);
    }

    for (_tab_entity, tab) in &tabs {
        if tab.leaf != leaf {
            continue;
        }
        let fg = rgb(if tab.id == id { text_primary() } else { text_muted() });
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
pub(crate) fn tab_hover(
    leaves: Query<&DockLeaf>,
    theme: Res<crate::style::Theme>,
    mut tabs: Query<(&Interaction, &DockTab, &mut BackgroundColor)>,
    mut texts: Query<&mut TextColor>,
) {
    let d = &theme.dock;
    for (interaction, tab, mut bg) in &mut tabs {
        // Active per the tab's own leaf (not a tree lookup): with floating dock
        // windows the same panel id can exist in several trees at once.
        let active = leaves.get(tab.leaf).is_ok_and(|l| l.active == tab.id);
        let hovered = matches!(interaction, Interaction::Hovered | Interaction::Pressed);
        let target = if active {
            d.tab_active.color()
        } else if hovered {
            d.tab_hover.color()
        } else {
            d.tab_inactive.color()
        };
        if bg.0 != target {
            bg.0 = target;
        }
        let tc = if active {
            d.tab_text_active.color()
        } else {
            d.tab_text.color()
        };
        for ent in [tab.label, tab.icon] {
            if let Ok(mut t) = texts.get_mut(ent) {
                if t.0 != tc {
                    t.0 = tc;
                }
            }
        }
    }
}

/// A tab's close × reddens while the cursor is over it.
pub(crate) fn tab_close_hover(
    mut closes: Query<(&bevy::ui::RelativeCursorPosition, &mut TextColor), With<TabClose>>,
) {
    for (rcp, mut color) in &mut closes {
        let target = rgb(if rcp.cursor_over { close_red() } else { text_muted() });
        if color.0 != target {
            color.0 = target;
        }
    }
}

/// Click a tab's × → remove that panel from its dock tree (primary or a
/// floating window's; closing a floating window's last tab closes the window).
pub(crate) fn tab_close_click(
    mouse: Res<ButtonInput<MouseButton>>,
    closes: Query<(&bevy::ui::RelativeCursorPosition, &ChildOf), With<TabClose>>,
    tabs: Query<&DockTab>,
    leaves: Query<&DockLeaf>,
    mut dock: ResMut<Dock>,
    mut fixed: ResMut<FixedDock>,
    mut dirty: ResMut<DockDirty>,
    mut wins: ResMut<DockWindows>,
    mut close_queue: ResMut<DockWindowCloseRequests>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    for (rcp, parent) in &closes {
        if !rcp.cursor_over {
            continue;
        }
        if let Ok(tab) = tabs.get(parent.parent()) {
            let area = leaves.get(tab.leaf).map(|l| l.area).ok();
            let Some(area) = area else { break };
            if area_tree_mut(area, &mut dock, &mut fixed, &mut wins).remove_panel(&tab.id) {
                flag_area_dirty(area, &mut dirty, &mut fixed, &mut wins);
                close_empty_dock_window(area, &wins, &mut close_queue);
            }
        }
        break;
    }
}

/// Click a bottom region's collapse chevron → publish [`BottomSnapRequest`]
/// so the consumer (the shell's bottom panel) collapses that region. No
/// ratio in the payload: unlike a divider snap, a click leaves the tree's
/// ratio accurate, so the consumer records whatever the detach returns.
pub(crate) fn bottom_collapse_click(
    q: Query<(&Interaction, &BottomCollapseBtn), Changed<Interaction>>,
    mut snap: ResMut<BottomSnapRequest>,
) {
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        snap.0 = Some(BottomSnap {
            restore: None,
            target: btn.target.clone(),
        });
    }
}
