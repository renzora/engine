//! Keyboard navigation for menus and dropdowns.
//!
//! While a [`screen_menu`](super::popup::screen_menu) or a dropdown is open,
//! **Up** and **Down** move a highlight through its rows, **Right** opens a
//! submenu and moves into it, **Left** steps back out, **Enter** picks the row,
//! and **Escape** closes one level at a time. The pointer and the keys share
//! one highlight: hovering a row moves the keyboard focus there, so either can
//! pick up where the other left off.
//!
//! Enter does not reimplement anything. It sets the row's [`UiInteraction`] to
//! `Pressed`, the documented way to press a widget from the keyboard (see
//! [`crate::interaction`]), so a menu action, a dropdown option and a submenu
//! row each run exactly the code a click runs, and the pointer's state replaces
//! the write the next frame.
//!
//! The keys are taken while a menu is open: they are cleared from
//! `ButtonInput<KeyCode>` here, in `PreUpdate`, so the editor's own bindings
//! (Escape clearing the selection, arrows moving through the hierarchy) do not
//! also act on them.

use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, InteractionDisabled, ScrollPosition, UiGlobalTransform};

use super::dropdown::{EmberDropdown, EmberDropdownOption};
use super::popup::{MenuAction, ScreenMenu};
use super::scroll_area::EmberScroll;
use super::submenu::{SubPanel, SubmenuRow};
use crate::UiInteraction;

/// The menu row the keyboard is on, drawn like a hovered row. A resource, not
/// a component, so a row's highlight binding sees the focus move as one
/// change rather than having to notice a component being removed.
#[derive(Resource, Default)]
pub struct MenuKeyFocus(pub Option<Entity>);

/// Submenu rows opened with the keyboard. `submenu_reveal` keeps these open
/// wherever the pointer is, until Left or Escape closes them.
#[derive(Resource, Default)]
pub(crate) struct KeyOpenSubmenus(pub(crate) HashSet<Entity>);

/// Whether a menu or dropdown is open and so owns the arrow keys this frame.
/// Clearing `just_pressed` is not enough on its own: the scroll area scrolls
/// for as long as an arrow is *held*, so without this a dropdown inside a
/// scrolling page moved the page under it on every press.
#[derive(Resource, Default, PartialEq)]
pub(crate) struct MenuHasKeys(pub(crate) bool);

/// The keys this answers, all taken while a menu is open.
const KEYS: [KeyCode; 6] = [
    KeyCode::ArrowUp,
    KeyCode::ArrowDown,
    KeyCode::ArrowLeft,
    KeyCode::ArrowRight,
    KeyCode::Enter,
    KeyCode::NumpadEnter,
];

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<MenuKeyFocus>()
        .init_resource::<KeyOpenSubmenus>()
        .init_resource::<MenuHasKeys>()
        // After the pointer state is settled for the frame (so a `Pressed`
        // written here is not overwritten until next frame), and before
        // `Update`, where the menu systems act on it and the editor's key
        // bindings would otherwise see the keys too.
        .add_systems(PreUpdate, menu_keys.after(crate::correct_pointer_state));
}

/// Anything a menu can be navigated to.
type RowFilter = Or<(With<MenuAction>, With<SubmenuRow>, With<EmberDropdownOption>)>;
/// A navigable row: its pointer state (written to press it), whether it opens
/// a submenu, and whether it is disabled.
type Row = (Entity, &'static mut UiInteraction, Option<&'static SubmenuRow>, Has<InteractionDisabled>);

#[allow(clippy::too_many_arguments)]
fn menu_keys(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut focus: ResMut<MenuKeyFocus>,
    mut key_open: ResMut<KeyOpenSubmenus>,
    mut has_keys: ResMut<MenuHasKeys>,
    menus: Query<Entity, With<ScreenMenu>>,
    mut dropdowns: Query<&mut EmberDropdown>,
    mut rows: Query<Row, RowFilter>,
    panels: Query<&SubPanel>,
    children: Query<&Children>,
    parents: Query<&ChildOf>,
    mut nodes: Query<&mut Node>,
    geom: Query<(&ComputedNode, &UiGlobalTransform)>,
    mut viewports: Query<(&mut EmberScroll, &ScrollPosition, &ComputedNode, &UiGlobalTransform)>,
    mut commands: Commands,
) {
    // The surface everything starts from: an open menu, else an open dropdown.
    let base = menus
        .iter()
        .last()
        .or_else(|| dropdowns.iter().find_map(|d| d.open_menu()));
    has_keys.set_if_neq(MenuHasKeys(base.is_some()));
    let Some(base) = base else {
        if focus.0.is_some() {
            focus.0 = None;
        }
        if !key_open.0.is_empty() {
            key_open.0.clear();
        }
        return;
    };

    // The pointer moved onto a row: the keyboard carries on from there.
    let hovered = rows
        .iter_mut()
        .find(|(_, i, _, _)| i.is_changed() && **i == UiInteraction::Hovered)
        .map(|(e, ..)| e);
    if let Some(row) = hovered {
        if focus.0 != Some(row) {
            focus.0 = Some(row);
        }
        key_open.0.clear();
    }
    // A focused row that has gone (its menu closed) is no focus at all.
    if focus.0.is_some_and(|f| !rows.contains(f)) {
        focus.0 = None;
    }

    if !KEYS.iter().any(|k| keys.just_pressed(*k)) && !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    let pressed = |k: KeyCode| keys.just_pressed(k);
    let (up, down, left, right) = (
        pressed(KeyCode::ArrowUp),
        pressed(KeyCode::ArrowDown),
        pressed(KeyCode::ArrowLeft),
        pressed(KeyCode::ArrowRight),
    );
    let enter = pressed(KeyCode::Enter) || pressed(KeyCode::NumpadEnter);
    let escape = pressed(KeyCode::Escape);
    for k in KEYS.iter().chain([KeyCode::Escape].iter()) {
        keys.clear_just_pressed(*k);
    }

    // The surface the focus is in: a submenu panel when it has moved into one.
    let row_ok = |e: Entity| rows.get(e).is_ok_and(|(_, _, _, disabled)| !disabled);
    let surface = focus
        .0
        .and_then(|f| parents.iter_ancestors(f).find(|a| panels.contains(*a)))
        .unwrap_or(base);
    let list = rows_in(surface, &children, &nodes, &row_ok);
    let at = focus.0.and_then(|f| list.iter().position(|r| *r == f));
    let focused_submenu = focus.0.and_then(|f| rows.get(f).ok().and_then(|(_, _, s, _)| s.map(|s| s.panel())));

    let mut moved_to = None;
    if down || up {
        let n = list.len();
        if n > 0 {
            let i = match (at, down) {
                (Some(i), true) => (i + 1) % n,
                (Some(i), false) => (i + n - 1) % n,
                (None, true) => 0,
                (None, false) => n - 1,
            };
            moved_to = Some(list[i]);
        }
    } else if (right || enter) && focused_submenu.is_some() {
        // Into a submenu: open it and move to its first row.
        let (Some(row), Some(panel)) = (focus.0, focused_submenu) else { return };
        key_open.0.insert(row);
        moved_to = rows_in(panel, &children, &nodes, &row_ok).first().copied();
    } else if left || escape {
        if let Some(owner) = panels.get(surface).ok().map(SubPanel::owner) {
            // Back out of a submenu to the row that opened it.
            key_open.0.remove(&owner);
            moved_to = Some(owner);
        } else if escape {
            for m in &menus {
                commands.entity(m).despawn();
            }
            for mut d in &mut dropdowns {
                if d.open_menu().is_some() {
                    d.close(&mut nodes);
                }
            }
            focus.0 = None;
            key_open.0.clear();
            return;
        }
    } else if enter {
        // The click path, untouched: see the module docs.
        if let Some((_, mut interaction, _, _)) = focus.0.and_then(|f| rows.get_mut(f).ok()) {
            *interaction = UiInteraction::Pressed;
        }
    }

    if let Some(row) = moved_to {
        if focus.0 != Some(row) {
            focus.0 = Some(row);
        }
        reveal(row, &parents, &geom, &mut viewports);
    }
}

/// The navigable rows under `surface`, top to bottom. Hidden rows (a
/// `Display::None` row or ancestor) are skipped, and so is anything inside a
/// row, which is its icon and label. The surface's own display is not checked:
/// a submenu panel opened this frame is still hidden until `submenu_reveal`
/// runs.
fn rows_in(
    surface: Entity,
    children: &Query<&Children>,
    nodes: &Query<&mut Node>,
    is_row: &impl Fn(Entity) -> bool,
) -> Vec<Entity> {
    let mut out = Vec::new();
    let mut stack: Vec<Entity> = children.get(surface).map(|c| c.iter().collect()).unwrap_or_default();
    stack.reverse();
    while let Some(e) = stack.pop() {
        if nodes.get(e).is_ok_and(|n| n.display == Display::None) {
            continue;
        }
        if is_row(e) {
            out.push(e);
            continue;
        }
        if let Ok(c) = children.get(e) {
            let start = stack.len();
            stack.extend(c.iter());
            stack[start..].reverse();
        }
    }
    out
}

/// Scroll the nearest scroll area above `row` just far enough to show it, so a
/// long menu or dropdown follows the keyboard. Reads last frame's layout; a row
/// that has never been laid out (a submenu opened this frame) is left alone.
fn reveal(
    row: Entity,
    parents: &Query<&ChildOf>,
    geom: &Query<(&ComputedNode, &UiGlobalTransform)>,
    viewports: &mut Query<(&mut EmberScroll, &ScrollPosition, &ComputedNode, &UiGlobalTransform)>,
) {
    let Ok((row_node, row_xf)) = geom.get(row) else { return };
    let inv = row_node.inverse_scale_factor();
    let row_h = row_node.size().y * inv;
    if row_h <= 0.0 {
        return;
    }
    let row_top = row_xf.translation.y * inv - row_h * 0.5;
    for ancestor in parents.iter_ancestors(row) {
        let Ok((mut scroll, pos, vp_node, vp_xf)) = viewports.get_mut(ancestor) else { continue };
        let vp_h = vp_node.size().y * inv;
        if vp_h <= 0.0 {
            return;
        }
        let vp_top = vp_xf.translation.y * inv - vp_h * 0.5;
        let delta = reveal_delta(row_top, row_h, vp_top, vp_h);
        if delta.abs() > 0.5 {
            scroll.scroll_to(pos.y + delta);
        }
        return;
    }
}

/// How far to scroll (positive is down) to show a row at `top` of height `h`
/// in a viewport at `vp_top` of height `vp_h`: nothing when it already shows,
/// otherwise just enough to bring the nearer edge in.
fn reveal_delta(top: f32, h: f32, vp_top: f32, vp_h: f32) -> f32 {
    if top < vp_top {
        top - vp_top
    } else if top + h > vp_top + vp_h {
        top + h - (vp_top + vp_h)
    } else {
        0.0
    }
}

#[cfg(test)]
mod nav_tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;

    /// A screen menu holding `n` action rows, the middle one hidden when
    /// `hide_middle` is set.
    fn menu(n: usize, hide_middle: bool) -> (World, Entity, Vec<Entity>) {
        let mut world = World::new();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<MenuKeyFocus>();
        world.init_resource::<KeyOpenSubmenus>();
        world.init_resource::<MenuHasKeys>();
        let root = world.spawn((Node::default(), ScreenMenu)).id();
        let rows: Vec<Entity> = (0..n)
            .map(|i| {
                let display = if hide_middle && i == n / 2 { Display::None } else { Display::Flex };
                world
                    .spawn((
                        Node { display, ..default() },
                        UiInteraction::default(),
                        MenuAction(Box::new(|_| {})),
                        ChildOf(root),
                    ))
                    .id()
            })
            .collect();
        (world, root, rows)
    }

    fn press(world: &mut World, key: KeyCode) {
        let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
        keys.release_all();
        keys.clear();
        keys.press(key);
        world.run_system_once(menu_keys).unwrap();
    }

    fn focused(world: &World) -> Option<Entity> {
        world.resource::<MenuKeyFocus>().0
    }

    #[test]
    fn down_starts_at_the_top_and_moves_on() {
        let (mut world, _, rows) = menu(3, false);
        press(&mut world, KeyCode::ArrowDown);
        assert_eq!(focused(&world), Some(rows[0]));
        press(&mut world, KeyCode::ArrowDown);
        assert_eq!(focused(&world), Some(rows[1]));
    }

    #[test]
    fn up_from_nothing_wraps_to_the_bottom() {
        let (mut world, _, rows) = menu(3, false);
        press(&mut world, KeyCode::ArrowUp);
        assert_eq!(focused(&world), Some(rows[2]));
    }

    #[test]
    fn a_hidden_row_is_skipped() {
        let (mut world, _, rows) = menu(3, true);
        press(&mut world, KeyCode::ArrowDown);
        press(&mut world, KeyCode::ArrowDown);
        assert_eq!(focused(&world), Some(rows[2]));
    }

    #[test]
    fn enter_presses_the_focused_row() {
        let (mut world, _, rows) = menu(3, false);
        press(&mut world, KeyCode::ArrowDown);
        press(&mut world, KeyCode::Enter);
        assert_eq!(*world.get::<UiInteraction>(rows[0]).unwrap(), UiInteraction::Pressed);
        assert_eq!(*world.get::<UiInteraction>(rows[1]).unwrap(), UiInteraction::None);
    }

    #[test]
    fn escape_closes_the_menu_and_is_not_seen_behind_it() {
        let (mut world, root, _) = menu(3, false);
        press(&mut world, KeyCode::Escape);
        world.flush();
        assert!(world.get_entity(root).is_err(), "the menu should be gone");
        assert!(!world.resource::<ButtonInput<KeyCode>>().just_pressed(KeyCode::Escape));
    }
}

#[cfg(test)]
mod tests {
    use super::reveal_delta;

    #[test]
    fn a_visible_row_needs_no_scroll() {
        assert_eq!(reveal_delta(110.0, 20.0, 100.0, 200.0), 0.0);
    }

    #[test]
    fn a_row_below_scrolls_down_to_its_bottom_edge() {
        assert_eq!(reveal_delta(290.0, 20.0, 100.0, 200.0), 10.0);
    }

    #[test]
    fn a_row_above_scrolls_up_to_its_top_edge() {
        assert_eq!(reveal_delta(80.0, 20.0, 100.0, 200.0), -20.0);
    }
}
