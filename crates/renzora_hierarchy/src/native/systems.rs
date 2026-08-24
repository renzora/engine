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
    BadgeKind, HierAssetBadge, HierCaretToggle, HierLockToggle, HierPinClick, HierRowClick,
    HierVisToggle,
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

    // A cleared selection has nothing to reveal. Bail *without* touching
    // `pending` — otherwise this would stomp a reveal another system armed in the
    // same frame (the sticky-header click deselects the branch it collapses, then
    // arms a scroll back to that branch's row; clobbering it to `None` here would
    // cancel that scroll).
    if current.is_none() {
        return;
    }

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
///
/// A plain click also folds the row's subtree open (or shut, when the click
/// deselects it) — unless `EditorSettings.hierarchy_toggle_on_click` is off, in
/// which case selection and expansion are fully separate gestures and only the
/// caret (or Left/Right — see [`hierarchy_arrow_keys`]) folds a branch.
pub(crate) fn hierarchy_row_click(
    rows: Query<(&Interaction, &HierRowClick), Changed<Interaction>>,
    carets: Query<(&Interaction, &HierCaretToggle)>,
    pins: Query<&Interaction, With<HierPinClick>>,
    selection: Option<Res<EditorSelection>>,
    keys: Res<ButtonInput<KeyCode>>,
    cache: Res<HierarchyTreeCache>,
    settings: Res<renzora_editor_framework::EditorSettings>,
    mut expanded: ResMut<HierExpanded>,
    time: Res<Time>,
    mut rename: ResMut<super::rename::HierRename>,
    mut last_click: Local<Option<(Entity, f64)>>,
) {
    let Some(selection) = selection else {
        return;
    };
    // A click on a sticky parent-stack header also lands on whatever row sits
    // behind it — overlapping UI nodes both register the press here (the same
    // reason the caret guard above exists). When a pin was pressed this frame,
    // let `hierarchy_pin_click` own the click entirely and ignore the row behind
    // it, so the sticky header doesn't select/deselect a random hidden row.
    if pins.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
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
        // The caret sits over the row's click layer, so the same physical click
        // fires both. When it landed on the caret, let the caret own it (toggle
        // the subtree) and leave the selection alone — clicking a selected row's
        // arrow must expand/collapse it, not deselect it.
        if carets
            .iter()
            .any(|(i, c)| c.0 == row.entity && *i == Interaction::Pressed)
        {
            continue;
        }
        // While this row is being inline-renamed, its rename field owns clicks —
        // don't re-select or restart the double-click timer from a click that's
        // really landing in the edit field.
        if rename.0 == Some(row.entity) {
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
        } else if selection.is_selected(row.entity) && !selection.has_multi_selection() {
            // Clicking the row that's already the sole selection deselects it and
            // collapses its subtree — one gesture to both drop the selection and
            // tidy the tree back up.
            selection.set(None);
            if settings.hierarchy_toggle_on_click {
                expanded.0.remove(&row.entity);
            }
        } else {
            // Selecting a row also opens its subtree, so the thing you just
            // picked reveals its children (the mirror of the deselect-collapse
            // above).
            selection.set(Some(row.entity));
            if settings.hierarchy_toggle_on_click {
                expanded.0.insert(row.entity);
            }
        }
    }
}

/// Depth-first search for `target`'s node in the cached tree.
fn find_tree_node(nodes: &[EntityNode], target: Entity) -> Option<&EntityNode> {
    for n in nodes {
        if n.entity == target {
            return Some(n);
        }
        if let Some(found) = find_tree_node(&n.children, target) {
            return Some(found);
        }
    }
    None
}

/// Does the hierarchy own the arrow keys right now? True when it's the focused
/// panel, nothing is typing, and there's a selection to walk from. Shared by the
/// navigation system and the [`renzora::core::ArrowKeysClaimed`] publisher so
/// the claim can never disagree with what the keys actually do.
fn owns_arrow_keys(
    focused: Option<&renzora_ember::dock::FocusedPanel>,
    play_mode: Option<&renzora::core::PlayModeState>,
    input_focus: &renzora::core::InputFocusState,
    rename: &super::rename::HierRename,
    selection: Option<&EditorSelection>,
) -> bool {
    if play_mode.is_some_and(|pm| pm.is_in_play_mode()) {
        return false;
    }
    // The search box and the inline rename field both want these keys for caret
    // motion while they hold the keyboard.
    if input_focus.ui_wants_keyboard || rename.0.is_some() {
        return false;
    }
    if focused.is_none_or(|f| f.0.as_deref() != Some(super::PANEL_ID)) {
        return false;
    }
    selection.is_some_and(|s| s.get().is_some())
}

/// Publish [`renzora::core::ArrowKeysClaimed`] so the hover-driven arrow-key
/// behaviours — ember's keyboard scrolling, the 2D viewport's nudge — stand down
/// while the tree is walking its selection with them.
///
/// **Deliberately not gated on `panel_active`**, unlike the rest of the panel's
/// systems: a backgrounded hierarchy would freeze the flag at whatever it last
/// wrote, and a stuck `true` swallows arrow-key scrolling in every panel until
/// you bring the tree back. It's a handful of resource reads.
pub(crate) fn publish_arrow_claim(
    focused: Option<Res<renzora_ember::dock::FocusedPanel>>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    input_focus: Res<renzora::core::InputFocusState>,
    rename: Res<super::rename::HierRename>,
    selection: Option<Res<EditorSelection>>,
    mut claimed: ResMut<renzora::core::ArrowKeysClaimed>,
) {
    let owns = owns_arrow_keys(
        focused.as_deref(),
        play_mode.as_deref(),
        &input_focus,
        &rename,
        selection.as_deref(),
    );
    if claimed.0 != owns {
        claimed.0 = owns;
    }
}

/// The arrow keys walk the tree, the way every tree view does it.
///
/// `↑`/`↓` move the selection to the previous/next **visible** row — the same
/// flattened order shift-click ranges over, so a collapsed branch is stepped
/// past rather than through. `→` opens a closed branch and, when it's already
/// open, steps into its first child; `←` shuts an open branch and, when there's
/// nothing left to shut, steps out to the parent. Together they walk a deep
/// imported model without touching the mouse, and `←`/`→` are the only fold
/// gesture besides the caret once `hierarchy_toggle_on_click` is off.
///
/// Gated on the hierarchy being the *focused* panel rather than merely visible,
/// because all four keys are already spoken for elsewhere — nudging the
/// selection in the 2D viewport, stepping frames in the animation timeline, and
/// (for `↑`/`↓`) scrolling whichever panel the cursor rests over. The panel earns
/// focus the moment you click a row, which is also the only way to have a
/// selection to walk from. See [`publish_arrow_claim`] for how the scroll
/// conflict is refereed.
///
/// `Shift`+`↑`/`↓` deliberately does *not* extend the selection: a range needs a
/// fixed anchor and a moving cursor, and `EditorSelection` stores neither — it
/// keeps a flat list whose first entry is the primary, so an extend would drag
/// the "current row" to the top of the range and walk off in the wrong
/// direction. Shift-click ranges work because they're one-shot.
pub(crate) fn hierarchy_arrow_keys(
    keys: Res<ButtonInput<KeyCode>>,
    focused: Option<Res<renzora_ember::dock::FocusedPanel>>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    input_focus: Res<renzora::core::InputFocusState>,
    rename: Res<super::rename::HierRename>,
    selection: Option<Res<EditorSelection>>,
    cache: Res<HierarchyTreeCache>,
    mut expanded: ResMut<HierExpanded>,
) {
    let right = keys.just_pressed(KeyCode::ArrowRight);
    let left = keys.just_pressed(KeyCode::ArrowLeft);
    let down = keys.just_pressed(KeyCode::ArrowDown);
    let up = keys.just_pressed(KeyCode::ArrowUp);
    if !right && !left && !down && !up {
        return;
    }
    if !owns_arrow_keys(
        focused.as_deref(),
        play_mode.as_deref(),
        &input_focus,
        &rename,
        selection.as_deref(),
    ) {
        return;
    }
    // `owns_arrow_keys` already established both of these.
    let Some(selection) = selection else {
        return;
    };
    let Some(entity) = selection.get() else {
        return;
    };

    if up || down {
        // Walk the flattened visible order, so `↓` from a collapsed parent lands
        // on its next sibling rather than on a child nobody can see. Clamped at
        // both ends — wrapping around a 2000-row tree is a jump, not a step.
        let order = visible_order(&cache, &expanded.0);
        let Some(idx) = order.iter().position(|e| *e == entity) else {
            return;
        };
        let next = if down {
            idx.saturating_add(1).min(order.len().saturating_sub(1))
        } else {
            idx.saturating_sub(1)
        };
        if let Some(target) = order.get(next) {
            if *target != entity {
                selection.set(Some(*target));
            }
        }
        return;
    }

    let Some(node) = find_tree_node(&cache.nodes, entity) else {
        return;
    };
    let has_children = !node.children.is_empty();
    let is_open = expanded.0.contains(&entity);

    if right {
        if has_children && !is_open {
            expanded.0.insert(entity);
        } else if has_children {
            selection.set(Some(node.children[0].entity));
        }
    } else if has_children && is_open {
        expanded.0.remove(&entity);
    } else {
        // Step out to the *displayed* parent, which is the tree's parent rather
        // than the raw `ChildOf` one — the cache re-parents through hidden GLTF
        // wrappers, and jumping to a row that isn't on screen would be a dead end.
        let mut path = Vec::new();
        if find_tree_path(&cache.nodes, entity, &mut path) && path.len() >= 2 {
            selection.set(Some(path[path.len() - 2]));
        }
    }
}

/// Caret click → toggle the row's expansion.
pub(crate) fn hierarchy_caret_click(
    q: Query<(&Interaction, &HierCaretToggle), Changed<Interaction>>,
    pins: Query<&Interaction, With<HierPinClick>>,
    mut expanded: ResMut<HierExpanded>,
) {
    // A pin click overlaps the row (and its caret) behind the sticky header;
    // let `hierarchy_pin_click` own it so a hidden row's caret doesn't toggle.
    if pins.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
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
