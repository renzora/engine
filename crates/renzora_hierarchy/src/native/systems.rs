//! Hierarchy interaction: row click selects (plain / ctrl-toggle / shift-range);
//! the caret toggles expansion; eye/lock push undoable visibility/lock commands.
//! (Selection/hover visuals are reactive bindings declared in `row.rs`.)

use bevy::platform::collections::HashSet;
use bevy::prelude::*;

use renzora_editor_framework::{EditorCommands, EditorSelection};
use renzora_undo::{execute, LockToggleCmd, UndoContext, VisibilityToggleCmd};

use bevy::ui::{ComputedNode, ScrollPosition};
use renzora_ember::widgets::EmberScroll;

use crate::cache::HierarchyTreeCache;
use crate::state::EntityNode;

use super::components::{
    BadgeKind, HierAssetBadge, HierCaretToggle, HierLockToggle, HierRowClick, HierVisToggle,
};
use super::row::ROW_H;
use super::{HierExpanded, HierRevealPending, HierScrollContent};

/// Visible (flattened, respecting expansion) entity order — the anchor list for
/// shift-range selection (and the row-index basis for scroll-to + parent stacking).
pub(crate) fn visible_order(cache: &HierarchyTreeCache, expanded: &HashSet<Entity>) -> Vec<Entity> {
    fn walk(nodes: &[EntityNode], expanded: &HashSet<Entity>, out: &mut Vec<Entity>) {
        for n in nodes {
            out.push(n.entity);
            if !n.children.is_empty() && expanded.contains(&n.entity) {
                walk(&n.children, expanded, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(&cache.nodes, expanded, &mut out);
    out
}

/// Depth-first search for `target` in the cached tree, recording the path of
/// ancestors (root → … → target) into `path`. Returns `true` once found.
fn find_tree_path(nodes: &[EntityNode], target: Entity, path: &mut Vec<Entity>) -> bool {
    for n in nodes {
        path.push(n.entity);
        if n.entity == target || find_tree_path(&n.children, target, path) {
            return true;
        }
        path.pop();
    }
    false
}

/// Reveal the primary selection in the tree. When selection changes (typically
/// from a viewport click) the selected entity may live under a collapsed model
/// root, so no row exists to carry the selection-highlight binding — the user
/// sees nothing selected in the hierarchy. Expand every displayed ancestor of
/// the selection so its row materialises and highlights.
///
/// `EditorSelection` uses interior mutability (`set` takes `&self`), so its
/// `Res` change-tick doesn't fire on selection writes — we track the last
/// revealed `(selection, cache version)` in `Local`s instead. Re-running on a
/// cache-version bump also reveals the current selection once a freshly loaded
/// model's subtree appears in the tree.
pub(crate) fn hierarchy_reveal_selection(
    selection: Option<Res<EditorSelection>>,
    marquee: Res<super::marquee::HierMarquee>,
    mut pending: ResMut<HierRevealPending>,
    mut last_sel: Local<Option<Entity>>,
) {
    let Some(selection) = selection else {
        return;
    };
    // A live rubber-band changes the primary selection every frame as it sweeps;
    // revealing (and scroll-centring) that moving target would fight the drag.
    if marquee.active() {
        return;
    }
    let current = selection.get();
    if current == *last_sel {
        return;
    }
    *last_sel = current;

    // Arm the reveal for the new primary selection. The work (expand ancestors,
    // centre the row) happens in `hierarchy_scroll_to_selection`. This is driven
    // SOLELY by selection change — never by cache rebuilds — so spawning the
    // parent-stack/row entities (which dirties the tree cache) can't re-arm it
    // and trap the user's scrolling on the selected row.
    pending.entity = current;
    pending.frames = 0;
    pending.decided = false;
    pending.scroll = false;
}

/// Reveal the pending (just-selected) row: expand its ancestors so the row
/// exists, and — only if it's off-screen — snap-scroll it to the vertical centre
/// of the panel (no easing). If the row is already visible the scroll is left
/// untouched (so clicking a visible row never yanks the view).
///
/// Every row is a fixed `ROW_H`, so the content height and the target's band
/// derive straight from the visible-order index — exact even before the freshly
/// expanded rows have laid out. It re-snaps for a few frames so a transient short
/// content height (rows still building, which would otherwise let `scroll_update`
/// clamp the position down) can't leave the row off-centre. A frame budget guards
/// a target that never resolves (e.g. a model still spawning, or filtered out).
pub(crate) fn hierarchy_scroll_to_selection(
    mut pending: ResMut<HierRevealPending>,
    cache: Res<HierarchyTreeCache>,
    mut expanded: ResMut<HierExpanded>,
    content: Query<Entity, With<HierScrollContent>>,
    parents: Query<&ChildOf>,
    mut viewports: Query<(&mut EmberScroll, &mut ScrollPosition, &ComputedNode)>,
) {
    let Some(target) = pending.entity else {
        return;
    };
    if pending.frames > 40 {
        pending.entity = None;
        return;
    }

    // Expand the target's ancestors so its row materialises. Use the cached tree
    // (which re-parents through hidden wrappers) rather than raw `ChildOf`, so
    // the expansion matches what the panel actually displays. If the target
    // isn't in the tree yet (e.g. a model still spawning), wait and retry.
    let mut path = Vec::new();
    if !find_tree_path(&cache.nodes, target, &mut path) {
        pending.frames += 1;
        return;
    }
    for ancestor in path.iter().rev().skip(1) {
        expanded.0.insert(*ancestor);
    }

    // Index in the (now-expanded) visible order → its pixel band. `order.len()`
    // is the row count, so content height is exact even before rows lay out.
    let order = visible_order(&cache, &expanded.0);
    let Some(idx) = order.iter().position(|e| *e == target) else {
        pending.frames += 1;
        return;
    };

    // The panel's scroll viewport is the parent of the marked content node.
    let Some(list) = content.iter().next() else {
        return;
    };
    let Ok(vp) = parents.get(list).map(|c| c.parent()) else {
        return;
    };
    let Ok((mut scroll, mut sp, cn)) = viewports.get_mut(vp) else {
        return;
    };
    let vh = cn.size().y * cn.inverse_scale_factor();
    if vh <= 0.0 {
        return; // not laid out yet — retry next frame (don't burn the budget)
    }

    let row_top = idx as f32 * ROW_H;
    let row_bottom = row_top + ROW_H;

    // First time the row resolves: decide whether to scroll at all. A row that's
    // already fully visible (e.g. you clicked it in the tree) stays put.
    if !pending.decided {
        pending.decided = true;
        pending.scroll = !(row_top >= sp.y && row_bottom <= sp.y + vh);
        if !pending.scroll {
            pending.entity = None;
            return;
        }
        pending.frames = 0; // start the snap window fresh
    }

    // Centre the row's band in the viewport, clamped to the scrollable range.
    let content_h = order.len() as f32 * ROW_H;
    let max = (content_h - vh).max(0.0);
    let centered = (row_top + ROW_H / 2.0 - vh / 2.0).clamp(0.0, max);

    // Snap: set both the smooth-scroll target and the live position so
    // `scroll_update` has nothing to ease toward.
    scroll.scroll_to(centered);
    sp.y = centered;

    pending.frames += 1;
    if pending.frames >= 4 {
        pending.entity = None;
    }
}

/// Row click → select the entity. Ctrl toggles it in the selection; Shift selects
/// the range from the current anchor; a plain click replaces the selection.
pub(crate) fn hierarchy_row_click(
    rows: Query<(&Interaction, &HierRowClick), Changed<Interaction>>,
    selection: Option<Res<EditorSelection>>,
    keys: Res<ButtonInput<KeyCode>>,
    cache: Res<HierarchyTreeCache>,
    expanded: Res<HierExpanded>,
    time: Res<Time>,
    mut rename: ResMut<super::rename::HierRename>,
    mut last_click: Local<Option<(Entity, f64)>>,
) {
    let Some(selection) = selection else {
        return;
    };
    let ctrl = keys.any_pressed([
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]);
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    for (interaction, row) in &rows {
        if *interaction != Interaction::Pressed {
            continue;
        }
        // Double-click (no modifiers) → inline rename.
        if !ctrl && !shift {
            let now = time.elapsed_secs_f64();
            if last_click.is_some_and(|(e, t)| e == row.entity && now - t < 0.4) {
                *last_click = None;
                rename.0 = Some(row.entity);
                continue;
            }
            *last_click = Some((row.entity, now));
        }
        if ctrl {
            selection.toggle(row.entity);
        } else if shift {
            match selection.get() {
                Some(anchor) => {
                    let order = visible_order(&cache, &expanded.0);
                    selection.select_range(&order, anchor, row.entity);
                }
                None => selection.set(Some(row.entity)),
            }
        } else {
            selection.set(Some(row.entity));
        }
    }
}

/// Caret click → toggle the row's expansion.
pub(crate) fn hierarchy_caret_click(
    q: Query<(&Interaction, &HierCaretToggle), Changed<Interaction>>,
    mut expanded: ResMut<HierExpanded>,
) {
    for (interaction, caret) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if expanded.0.contains(&caret.0) {
            expanded.0.remove(&caret.0);
        } else {
            expanded.0.insert(caret.0);
        }
    }
}

/// Eye toggle click → push a visibility-toggle command (undoable).
pub(crate) fn hierarchy_vis_toggle(
    q: Query<(&Interaction, &HierVisToggle), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else {
        return;
    };
    for (interaction, t) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (entity, was_visible) = (t.entity, t.visible);
        cmds.push(move |world: &mut World| {
            execute(
                world,
                UndoContext::Scene,
                Box::new(VisibilityToggleCmd {
                    entity,
                    was_visible,
                }),
            );
        });
    }
}

/// Asset badge click → open the entity's script / blueprint / material in its
/// editor. The actual path resolution + tab routing runs as a deferred command
/// (needs `&mut World`), via [`open_entity_asset`].
pub(crate) fn hierarchy_badge_click(
    q: Query<(&Interaction, &HierAssetBadge), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else {
        return;
    };
    for (interaction, badge) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (entity, kind) = (badge.entity, badge.kind);
        cmds.push(move |world: &mut World| open_entity_asset(world, entity, kind));
    }
}

/// Resolve the asset path off `entity`'s components and open it in the matching
/// editor. `open_asset_tab` opens (or focuses) the document tab and switches to
/// that asset kind's layout — for scripts it also hands the file to the code
/// editor. The stored paths are project-relative, so we resolve to absolute
/// first (`open_asset_tab` re-derives the relative form itself).
fn open_entity_asset(world: &mut World, entity: Entity, kind: BadgeKind) {
    use renzora_editor_framework::open_asset_tab;
    use renzora_ui::DocTabKind;

    let resolve = |rel: &str, world: &World| -> std::path::PathBuf {
        world
            .get_resource::<renzora::core::CurrentProject>()
            .map(|p| p.resolve_path(rel))
            .unwrap_or_else(|| std::path::PathBuf::from(rel))
    };

    match kind {
        BadgeKind::Material => {
            let rel = world
                .get::<renzora::core::MaterialRef>(entity)
                .map(|m| m.0.clone())
                .filter(|s| !s.is_empty());
            let Some(rel) = rel else {
                return;
            };
            let abs = resolve(&rel, world);
            open_asset_tab(world, &abs, DocTabKind::Material);
        }
        BadgeKind::Script | BadgeKind::Blueprint => {
            let want_blueprint = matches!(kind, BadgeKind::Blueprint);
            // First entry whose blueprint-ness matches the badge and that has a
            // backing file (a registered `script_id` has no file to open).
            let rel = {
                let Some(sc) = world.get::<renzora_scripting::ScriptComponent>(entity) else {
                    return;
                };
                sc.scripts.iter().find_map(|e| {
                    let p = e.script_path.as_ref()?;
                    let is_bp = p
                        .extension()
                        .is_some_and(|x| x.eq_ignore_ascii_case("blueprint"));
                    (is_bp == want_blueprint).then(|| p.to_string_lossy().replace('\\', "/"))
                })
            };
            let Some(rel) = rel else {
                return;
            };
            let abs = resolve(&rel, world);
            let doc_kind = if want_blueprint {
                DocTabKind::Blueprint
            } else {
                DocTabKind::Script
            };
            open_asset_tab(world, &abs, doc_kind);
        }
    }
}

/// Lock toggle click → push a lock-toggle command (undoable).
pub(crate) fn hierarchy_lock_toggle(
    q: Query<(&Interaction, &HierLockToggle), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else {
        return;
    };
    for (interaction, t) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (entity, was_locked) = (t.entity, t.locked);
        cmds.push(move |world: &mut World| {
            execute(
                world,
                UndoContext::Scene,
                Box::new(LockToggleCmd { entity, was_locked }),
            );
        });
    }
}
