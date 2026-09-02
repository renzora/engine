//! The workspace ribbon: the centred strip of workspace buttons in the top bar,
//! and everything that edits the workspace list — switch, reorder by drag,
//! rename in place, remove, add, and "drop a docked panel here to give it a
//! workspace of its own".
//!
//! The reorder and the plain click are one gesture, resolved on release: only
//! the code that saw the press *and* the motion knows which of the two it was.
//! The document tabs and the bottom panel's set menu are built the same way.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora_ember::dock::{Dock, DockDirty};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::{accent, rgb, text_muted, text_primary};
use renzora_ember::widgets::{menu_item, screen_menu, text_input, EmberTextInput};

use crate::dock::DockTree;
use crate::ShellLayouts;

/// Width budget for the workspace ribbon before workspaces start folding. Unlike
/// the document tabs there's no container to measure — the ribbon is centered
/// and content-sized, so it grows symmetrically out of the middle of the bar and
/// a constant is what keeps it from meeting the two ends. Sized for the eight
/// built-in workspaces plus a few of your own.
pub(crate) const RIBBON_W: f32 = 700.0;

/// A ribbon workspace button (Scene, Blueprints, …). Carries its layout index;
/// the active highlight comes from the reactive rebuild (see [`ribbon_snapshot`]).
#[derive(Component)]
pub(crate) struct RibbonItem {
    index: usize,
    /// Vertical insertion marker shown at this tab's left/right edge during a
    /// reorder drag (mirrors the dock tab-drag preview). Toggled in
    /// [`ribbon_interact`].
    marker: Entity,
}

/// The ribbon's "+" — adds a new empty workspace.
#[derive(Component)]
pub(crate) struct WorkspaceAddBtn;

/// Tags the ribbon strip + its `+` as a drop target for dock-tab drags: dropping
/// a dragged panel here spawns a new workspace from it (see
/// [`workspace_drop_to_new`]).
#[derive(Component)]
pub(crate) struct WorkspaceDropZone;

/// In-progress ribbon drag (press-latch → reorder on release). `active` flips
/// once the cursor moves past a small threshold so a plain click still switches.
#[derive(Resource, Default)]
pub(crate) struct RibbonDrag(Option<RibbonDragState>);

struct RibbonDragState {
    from: usize,
    start_cursor: Vec2,
    active: bool,
    /// Insertion slot (0..=len) under the live cursor; applied on release.
    target: usize,
}

/// The workspace currently being inline-renamed (`None` = none). Read by
/// [`ribbon_snapshot`] so that tab renders an edit field in place of its label.
#[derive(Resource, Default)]
pub(crate) struct RibbonRename(Option<usize>);

/// Marks the inline rename text field, carrying the workspace index it renames.
#[derive(Component)]
pub(crate) struct RibbonRenameInput(usize);

/// Clicking a ribbon workspace button switches the dock layout: save the current
/// dock back into its slot, load the chosen layout into the ember [`Dock`],
/// flag a rebuild, and restyle the ribbon.
/// Press-latch ribbon interaction: a plain click switches workspace; a drag past
/// a small threshold reorders on release (mirrors the egui title-bar tabs).
#[allow(clippy::too_many_arguments)]
pub(crate) fn ribbon_interact(
    mut drag: ResMut<RibbonDrag>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    rename: Res<RibbonRename>,
    pressed: Query<(&RibbonItem, &Interaction)>,
    items: Query<(&RibbonItem, &RelativeCursorPosition)>,
    mut nodes: Query<&mut Node>,
    mut layouts: ResMut<ShellLayouts>,
    mut dock: ResMut<Dock>,
    mut dirty: ResMut<DockDirty>,
) {
    // Hide every insertion marker (no drag live, or before re-showing one slot).
    let hide_markers = |items: &Query<(&RibbonItem, &RelativeCursorPosition)>, nodes: &mut Query<&mut Node>| {
        for (it, _) in items {
            if let Ok(mut n) = nodes.get_mut(it.marker) {
                if n.display != Display::None {
                    n.display = Display::None;
                }
            }
        }
    };

    // Don't drag/switch while a tab is being renamed.
    if rename.0.is_some() {
        drag.0 = None;
        hide_markers(&items, &mut nodes);
        return;
    }
    let cursor = windows.iter().next().and_then(|w| w.cursor_position());

    if drag.0.is_none() && mouse.just_pressed(MouseButton::Left) {
        if let Some(cur) = cursor {
            for (item, interaction) in &pressed {
                if *interaction == Interaction::Pressed {
                    drag.0 = Some(RibbonDragState { from: item.index, start_cursor: cur, active: false, target: item.index });
                    break;
                }
            }
        }
    }

    if let (Some(state), Some(cur)) = (drag.0.as_mut(), cursor) {
        if (cur - state.start_cursor).length() > 5.0 {
            state.active = true;
        }
    }

    // While actively dragging, track the insertion slot under the cursor and show
    // the matching edge marker. Using each tab's RelativeCursorPosition (not a
    // GlobalTransform center compared against the cursor, which drifts under UI
    // scaling) keeps the hit-test in the cursor's own space — fixing both the
    // missing divider and the wrong drop position.
    if let Some(state) = drag.0.as_mut() {
        if state.active {
            // (marker, right-edge): cursor in a tab's left half inserts before it,
            // right half after it.
            let mut shown: Option<(Entity, bool)> = None;
            for (it, rcp) in &items {
                if rcp.cursor_over {
                    let before = rcp.normalized.is_none_or(|n| n.x < 0.0);
                    state.target = if before { it.index } else { it.index + 1 };
                    shown = Some((it.marker, !before));
                    break;
                }
            }
            hide_markers(&items, &mut nodes);
            if let Some((marker, right)) = shown {
                if let Ok(mut n) = nodes.get_mut(marker) {
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
    } else {
        hide_markers(&items, &mut nodes);
    }

    if mouse.just_released(MouseButton::Left) {
        hide_markers(&items, &mut nodes);
        if let Some(state) = drag.0.take() {
            if !state.active {
                apply_workspace(state.from, &mut layouts, &mut dock, &mut dirty);
            } else {
                let from = state.from;
                let target = state.target.min(layouts.layouts.len());
                // Removing `from` first shifts later slots left by one.
                let post_to = if from < target { target.saturating_sub(1) } else { target };
                if post_to != from {
                    move_workspace(&mut layouts, &dock, from, post_to);
                }
            }
        }
    }
}

/// Move workspace `from` → `to` (remove-then-insert), saving the live dock tree
/// into the active slot first and remapping the active index to follow.
fn move_workspace(layouts: &mut ShellLayouts, dock: &Dock, from: usize, to: usize) {
    let len = layouts.layouts.len();
    if from >= len || to >= len || from == to {
        return;
    }
    let active = layouts.active;
    if let Some(slot) = layouts.layouts.get_mut(active) {
        slot.1 = dock.tree.clone();
    }
    let item = layouts.layouts.remove(from);
    layouts.layouts.insert(to, item);
    layouts.active = if active == from {
        to
    } else {
        let mut a = active;
        if from < a {
            a -= 1;
        }
        if to <= a {
            a += 1;
        }
        a
    };
}

/// Right-click a ribbon tab → context menu (Rename / Remove).
pub(crate) fn ribbon_context_menu(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    fonts: Option<Res<EmberFonts>>,
    items: Query<(&RibbonItem, &RelativeCursorPosition)>,
    layouts: Res<ShellLayouts>,
    mut commands: Commands,
) {
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    let Some(fonts) = fonts else { return };
    let Some(cur) = windows.iter().next().and_then(|w| w.cursor_position()) else {
        return;
    };
    for (item, rcp) in &items {
        if !rcp.cursor_over {
            continue;
        }
        let index = item.index;
        let can_delete = layouts.layouts.len() > 1;
        let menu = screen_menu(&mut commands, cur.x, cur.y);
        let rename = menu_item(&mut commands, &fonts, "pencil-simple", "Rename", move |w| {
            if let Some(mut r) = w.get_resource_mut::<RibbonRename>() {
                r.0 = Some(index);
            }
        });
        let mut kids = vec![rename];
        if can_delete {
            let remove = menu_item(&mut commands, &fonts, "trash", "Remove", move |w| remove_workspace(w, index));
            kids.push(remove);
        }
        commands.entity(menu).add_children(&kids);
        break;
    }
}

/// Remove workspace `index`, remapping the active index (and switching the live
/// dock to the new active's tree when the active workspace itself is removed).
fn remove_workspace(world: &mut World, index: usize) {
    let (len, active) = {
        let Some(l) = world.get_resource::<ShellLayouts>() else { return };
        (l.layouts.len(), l.active)
    };
    if len <= 1 || index >= len {
        return;
    }
    let removing_active = index == active;
    {
        let mut l = world.resource_mut::<ShellLayouts>();
        l.layouts.remove(index);
        let new_len = l.layouts.len();
        l.active = if active == index {
            active.min(new_len - 1)
        } else if active > index {
            active - 1
        } else {
            active
        };
    }
    // The bottom panel is no longer part of any workspace, so removing one
    // leaves it alone — there is nothing keyed by `removed_name` to clean up.
    // This used to drop that workspace's stash, which also meant deleting a
    // workspace silently deleted whatever panels were sitting in its closed
    // bottom strip.
    if removing_active {
        let new_tree = {
            let l = world.resource::<ShellLayouts>();
            l.layouts[l.active].1.clone()
        };
        world.resource_mut::<Dock>().tree = new_tree;
        world.resource_mut::<DockDirty>().0 = true;
    }
}

/// Auto-focus the rename field the frame it spawns.
pub(crate) fn ribbon_focus_rename(mut q: Query<&mut EmberTextInput, Added<RibbonRenameInput>>) {
    for mut inp in &mut q {
        inp.focused = true;
    }
}

/// Commit (Enter / blur) or cancel (Escape) the active ribbon rename.
pub(crate) fn ribbon_rename_commit(
    mut rename: ResMut<RibbonRename>,
    keys: Res<ButtonInput<KeyCode>>,
    inputs: Query<(&EmberTextInput, &RibbonRenameInput)>,
    mut layouts: ResMut<ShellLayouts>,
    mut had_focus: Local<bool>,
) {
    let Some(index) = rename.0 else {
        *had_focus = false;
        return;
    };
    if keys.just_pressed(KeyCode::Escape) {
        rename.0 = None;
        *had_focus = false;
        return;
    }
    let Some((inp, _)) = inputs.iter().find(|(_, r)| r.0 == index) else {
        return;
    };
    if inp.focused {
        *had_focus = true;
    }
    let enter = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter);
    let blurred = *had_focus && !inp.focused;
    if !enter && !blurred {
        return;
    }
    let new: String = inp.value.replace('\n', "").trim().to_string();
    rename.0 = None;
    *had_focus = false;
    if new.is_empty() {
        return;
    }
    if let Some(slot) = layouts.layouts.get_mut(index) {
        slot.0 = new;
        // Renaming used to have to re-key the bottom-panel stash, which was
        // keyed by workspace name and held the only copy of its panels — miss
        // the re-key and the rename orphaned them. The bottom panel is global
        // now and knows nothing about workspace names, so a rename is just a
        // rename.
    }
}

/// `+` → add a new empty workspace and switch to it.
pub(crate) fn workspace_add_click(
    q: Query<&Interaction, (With<WorkspaceAddBtn>, Changed<Interaction>)>,
    mut layouts: ResMut<ShellLayouts>,
    mut dock: ResMut<Dock>,
    mut dirty: ResMut<DockDirty>,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    // Save the current layout, then append + focus a fresh empty workspace.
    let active = layouts.active;
    if let Some(slot) = layouts.layouts.get_mut(active) {
        slot.1 = dock.tree.clone();
    }
    let name = format!("Workspace {}", layouts.layouts.len() + 1);
    // A genuinely empty workspace (not a tab literally named "empty"), so the
    // dock shows its "Add Panel" button.
    layouts.layouts.push((name, DockTree::Empty));
    let idx = layouts.layouts.len() - 1;
    dock.tree = layouts.layouts[idx].1.clone();
    layouts.active = idx;
    dirty.0 = true;
}

/// Drag any dock tab onto the workspace ribbon (the strip or its `+`) and drop it
/// to spawn a NEW workspace containing only that panel — the panel is *moved* out
/// of the workspace it came from.
///
/// The ember dock publishes the in-flight drag through [`renzora_ember::dock::DockDragWatch`]:
/// `dragging` is the panel id, and setting `claim` tells the dock to leave the
/// drop to us (so it neither re-docks nor tab-switches). We claim while the cursor
/// is over the ribbon, then build the workspace on release.
///
/// The release is handled from `Local` state captured on earlier frames because
/// the dock clears its own watch on release and may run before us that frame — so
/// `watch.dragging` can already be `None` by the time we see the mouse-up.
pub(crate) fn workspace_drop_to_new(
    watch: Option<ResMut<renzora_ember::dock::DockDragWatch>>,
    mouse: Res<ButtonInput<MouseButton>>,
    zones: Query<&RelativeCursorPosition, With<WorkspaceDropZone>>,
    mut add_bg: Query<&mut BackgroundColor, With<WorkspaceAddBtn>>,
    mut layouts: ResMut<ShellLayouts>,
    mut dock: ResMut<Dock>,
    mut dirty: ResMut<DockDirty>,
    mut dragged_id: Local<Option<String>>,
    mut over_zone: Local<bool>,
) {
    let Some(mut watch) = watch else {
        return;
    };

    // Resolve the drop using the prior frames' captured state, then reset.
    if mouse.just_released(MouseButton::Left) {
        if *over_zone {
            if let Some(id) = dragged_id.take() {
                make_workspace_from_panel(&id, &mut layouts, &mut dock, &mut dirty);
            }
        }
        *over_zone = false;
        *dragged_id = None;
        if let Ok(mut bg) = add_bg.single_mut() {
            bg.0 = Color::NONE;
        }
        // Deliberately leave `watch.claim`/`watch.dragging` for the dock to clear:
        // if the dock's `tab_drag` runs after us this frame it must still see the
        // claim so it skips its own re-dock. It clears both on release regardless.
        return;
    }

    // Track the in-flight drag and claim the drop while over the ribbon.
    if let Some(id) = &watch.dragging {
        if dragged_id.as_deref() != Some(id.as_str()) {
            *dragged_id = Some(id.clone());
        }
    }
    let hovering = watch.dragging.is_some() && zones.iter().any(|rcp| rcp.cursor_over);
    if watch.claim != hovering {
        watch.claim = hovering;
    }
    if *over_zone != hovering {
        *over_zone = hovering;
        if let Ok(mut bg) = add_bg.single_mut() {
            bg.0 = if hovering { rgb(accent()) } else { Color::NONE };
        }
    }
}

/// Move panel `id` into a brand-new workspace of its own and switch to it. The
/// panel is removed from the current (active) tree first so this is a move, not a
/// copy; the emptied current workspace is saved back into its slot.
fn make_workspace_from_panel(
    id: &str,
    layouts: &mut ShellLayouts,
    dock: &mut Dock,
    dirty: &mut DockDirty,
) {
    dock.tree.remove_panel(id);
    let active = layouts.active;
    if let Some(slot) = layouts.layouts.get_mut(active) {
        slot.1 = dock.tree.clone();
    }
    let name = renzora_ember::dock::humanize(id);
    layouts.layouts.push((name, DockTree::leaf(id.to_string())));
    let idx = layouts.layouts.len() - 1;
    dock.tree = layouts.layouts[idx].1.clone();
    layouts.active = idx;
    dirty.0 = true;
}

/// A top-bar ribbon entry (workspace switcher). Full height so the active
/// item's blue underline pins to the bottom edge. Clicking switches workspace
/// `index`; dragging reorders, right-click renames/removes (see [`ribbon_interact`]).
fn ribbon_item(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    label: &str,
    index: usize,
    active: bool,
) -> Entity {
    let item = commands
        .spawn((
            Node {
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            Interaction::default(),
            RelativeCursorPosition::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new(format!("ribbon:{label}")),
        ))
        .id();
    // Localize the *display* of built-in workspace names (Scene, Scripting, …)
    // via `layout.<slug>`; the stored `label` stays the workspace's identity (it's
    // the persisted key + the entity Name). A user-renamed/added workspace has no
    // matching key, so `t_or` falls back to its raw name.
    let display = renzora::lang::t_or(&format!("layout.{}", label.to_lowercase()), label);
    let text = commands
        .spawn((
            Text::new(display),
            ui_font(font, 12.0),
            TextColor(rgb(if active { text_primary() } else { text_muted() })),
        ))
        .id();
    let text_wrap = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::horizontal(Val::Px(7.0)),
                ..default()
            },
            Name::new("ribbon-label"),
        ))
        .id();
    commands.entity(text_wrap).add_child(text);
    let underline = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(if active { rgb(accent()) } else { Color::NONE }),
            Name::new("ribbon-underline"),
        ))
        .id();
    // Insertion marker: a thin accent bar pinned to the item's edge, hidden until
    // a reorder drag points at this slot (see [`ribbon_interact`]). Absolutely
    // positioned so it never affects the ribbon's layout.
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
            BackgroundColor(rgb(accent())),
            bevy::ui::FocusPolicy::Pass,
            Name::new("ribbon-insert-marker"),
        ))
        .id();
    commands.entity(item).insert(RibbonItem { index, marker });
    // What this workspace looks like in the ribbon's `»` menu once it folds —
    // and, while it's the active one, the guarantee that it never folds at all.
    commands.entity(item).insert(renzora_ember::widgets::OverflowEntry::new(
        "browsers",
        &renzora::lang::t_or(&format!("layout.{}", label.to_lowercase()), label),
        move |w| select_workspace(w, index),
    ));
    if active {
        commands.entity(item).insert(renzora_ember::widgets::OverflowKeep);
    }
    commands.entity(item).add_children(&[text_wrap, underline, marker]);
    item
}

/// Switch to workspace `index` from a `&mut World` context (the ribbon's
/// overflow menu, which has no system params of its own). The three resources
/// [`apply_workspace`] mutates can't be borrowed at once, hence the nesting.
pub(crate) fn select_workspace(w: &mut World, index: usize) {
    w.resource_scope(|w, mut layouts: Mut<ShellLayouts>| {
        w.resource_scope(|w, mut dock: Mut<Dock>| {
            let mut dirty = w.resource_mut::<DockDirty>();
            apply_workspace(index, &mut layouts, &mut dock, &mut dirty);
        });
    });
}

/// Keyed snapshot of the workspace ribbon (one button per `ShellLayouts` entry;
/// the content hash carries the active flag so switching repaints just the two
/// affected buttons).
pub(crate) fn ribbon_snapshot(world: &Rx) -> renzora_ember::reactive::KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    let empty = || renzora_ember::reactive::KeyedSnapshot {
        items: Vec::new(),
        build: Box::new(|c, _, _| c.spawn(Node::default()).id()),
    };
    let Some(layouts) = world.get_resource::<ShellLayouts>() else {
        return empty();
    };
    let active = layouts.active;
    let renaming = world.get_resource::<RibbonRename>().and_then(|r| r.0);
    let names: Vec<(usize, String)> = layouts
        .layouts
        .iter()
        .enumerate()
        .map(|(i, (n, _))| (i, n.clone()))
        .collect();
    let items: Vec<(u64, u64)> = names
        .iter()
        .map(|(i, name)| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            i.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (name, *i == active, renaming == Some(*i)).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    renzora_ember::reactive::KeyedSnapshot {
        items,
        build: Box::new(move |c, f, idx| {
            let (i, name) = &names[idx];
            if renaming == Some(*i) {
                build_ribbon_rename_field(c, &f.ui, *i, name)
            } else {
                ribbon_item(c, &f.ui, name, *i, *i == active)
            }
        }),
    }
}

/// Inline rename field for a ribbon tab (mirrors the native hierarchy's). Seeded
/// with the current name; committed by [`ribbon_rename_commit`].
fn build_ribbon_rename_field(commands: &mut Commands, font: &bevy::text::FontSource, index: usize, name: &str) -> Entity {
    let input = text_input(commands, font, "Name", name);
    commands.entity(input).insert((
        RibbonRenameInput(index),
        Node {
            width: Val::Px(96.0),
            height: Val::Px(22.0),
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(4.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        },
    ));
    input
}

/// Swap the dock to workspace `index`, saving the current layout into the active
/// slot first. The ribbon highlight follows via the reactive rebuild (the
/// snapshot keys on `layouts.active`). Shared by the ribbon + doc-tab clicks.
pub(crate) fn apply_workspace(index: usize, layouts: &mut ShellLayouts, dock: &mut Dock, dirty: &mut DockDirty) {
    if index == layouts.active || index >= layouts.layouts.len() {
        return;
    }
    let active = layouts.active;
    if let Some(slot) = layouts.layouts.get_mut(active) {
        slot.1 = dock.tree.clone();
    }
    dock.tree = layouts.layouts[index].1.clone();
    layouts.active = index;
    dirty.0 = true;
}
