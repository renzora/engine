//! Nested submenus for [`screen_menu`](super::popup::screen_menu) — a menu row
//! that opens a panel of further *action* rows on hover.
//!
//! Ember already had hover submenus in [`super::menu`], but those rows are
//! label-only: clicking one just closes the menu. Everything built on
//! `screen_menu` does its work through [`MenuAction`](super::popup::MenuAction)
//! closures instead, so a quick-add menu (hierarchy → category → entity) needs
//! the same nesting for *action* rows. That's what this module adds; the reveal
//! pass here handles nesting to any depth.
//!
//! WHY the panel is a parentless root node rather than a child of its row: this
//! is the same rule the global tooltip follows. bevy_ui clips absolutely
//! positioned children by every scrolling/clipping ancestor, and a `screen_menu`
//! keeps its items inside a height-capped scroll area — so a panel parented to
//! its row would be sliced off at the menu's edge the moment it was taller than
//! the room left below that row (`GlobalZIndex` fixes paint order, not
//! clipping). Parentless, it's positioned in window pixels exactly like the menu
//! root it belongs to, and an `on_remove` hook on the row despawns it with the
//! menu so nothing is left floating.

use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, FocusPolicy, RelativeCursorPosition, UiGlobalTransform};
use bevy::window::{PrimaryWindow, SystemCursorIcon};

use crate::font::{icon_text, ui_font, EmberFonts};
use crate::reactive::bind_bg;
use crate::theme::*;

use super::popup::{OverlaySurface, MENU_GAP, MENU_ICON, MENU_PAD_X, MENU_PAD_Y, MENU_TEXT};

/// Same cap as [`screen_menu`](super::popup::screen_menu): a long submenu (the
/// shape list, say) scrolls instead of running off the screen.
const SUBMENU_MAX_H: f32 = 420.0;

/// How far the submenu slides back *over* its parent menu, in logical px. A
/// small overlap means a cursor moving diagonally toward the submenu never
/// crosses a gap of dead space (which would close it mid-travel).
const OVERLAP: f32 = 3.0;

/// A row that reveals `panel` while it (or anything deeper in its submenu chain)
/// is hovered.
#[derive(Component)]
#[component(on_remove = submenu_row_removed)]
pub(crate) struct SubmenuRow {
    panel: Entity,
}

/// The panel half of a [`SubmenuRow`], tagged with the row that owns it so the
/// reveal pass can walk a nested chain back up to its root row.
#[derive(Component)]
pub(crate) struct SubPanel {
    owner: Entity,
}

/// The panel is parentless, so closing the menu can't take it down the tree —
/// take it down with the row instead.
fn submenu_row_removed(mut world: DeferredWorld, ctx: HookContext) {
    let Some(panel) = world.get::<SubmenuRow>(ctx.entity).map(|r| r.panel) else {
        return;
    };
    world.commands().entity(panel).try_despawn();
}

/// A [`screen_menu`](super::popup::screen_menu) row that opens a nested panel on
/// hover, for building `Category ▸ Item` menus.
///
/// Returns `(row, content)`: add `row` to the parent menu exactly like a
/// [`menu_item`](super::popup::menu_item), and fill `content` with menu items —
/// including further submenus, which nest to any depth.
pub fn menu_submenu(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
) -> (Entity, Entity) {
    menu_submenu_styled(commands, fonts, icon, label, text_muted())
}

/// [`menu_submenu`] with an explicit icon color — for menus that color-code
/// their rows (e.g. one accent per entity category).
pub fn menu_submenu_styled(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
    icon_color: (u8, u8, u8),
) -> (Entity, Entity) {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(MENU_GAP),
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(MENU_PAD_X), Val::Px(MENU_PAD_Y)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("menu-submenu"),
        ))
        .id();
    bind_bg(commands, row, move |w| match w.get::<Interaction>(row) {
        Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(hover_bg()),
        _ => Color::NONE,
    });

    let ic = icon_text(commands, &fonts.phosphor, icon, icon_color, MENU_ICON);
    let text = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, MENU_TEXT),
            TextColor(rgb(text_primary())),
        ))
        .id();
    // Pushes the caret to the row's trailing edge whatever the label's width.
    let spacer = commands
        .spawn(Node {
            flex_grow: 1.0,
            min_width: Val::Px(12.0),
            ..default()
        })
        .id();
    let caret = icon_text(commands, &fonts.phosphor, "caret-right", text_muted(), 10.0);
    commands.entity(row).add_children(&[ic, text, spacer, caret]);

    let content = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Name::new("screen-submenu-items"),
        ))
        .id();
    let scroller = super::scroll_area::scroll_area(commands, content, SUBMENU_MAX_H);
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                min_width: Val::Px(184.0),
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            // Above the 9000 of the menu root it belongs to.
            GlobalZIndex(9100),
            OverlaySurface,
            // Spawn-time (not via an `Added` pass) for the same reason the menu
            // root does it: a menu torn down the same frame would leave the
            // deferred insert operating on a dead entity.
            FocusPolicy::Block,
            RelativeCursorPosition::default(),
            // Editor chrome, not scene content — a menu open when autosave fires
            // must not be serialized into the scene.
            renzora::HideInHierarchy,
            SubPanel { owner: row },
            Name::new("screen-submenu"),
        ))
        .id();
    commands.entity(panel).add_child(scroller);
    commands.entity(row).insert(SubmenuRow { panel });
    (row, content)
}

/// True while the cursor sits over a *visible* submenu panel — the signal
/// [`screen_menu_dismiss`](super::popup::screen_menu_dismiss) uses to leave a
/// menu alone when the click landed in one of its submenus (which floats outside
/// the root's own rect, so the root reads as "not hovered").
pub(crate) fn cursor_over_submenu(
    panels: &Query<(&RelativeCursorPosition, &Node), With<SubPanel>>,
) -> bool {
    panels
        .iter()
        .any(|(rcp, node)| node.display != Display::None && rcp.cursor_over)
}

/// Show/hide + position every submenu panel each frame.
pub(crate) fn submenu_reveal(
    windows: Query<&Window, With<PrimaryWindow>>,
    rows: Query<(Entity, &Interaction, &SubmenuRow)>,
    panels: Query<(&RelativeCursorPosition, &ComputedNode, &SubPanel)>,
    parents: Query<&ChildOf>,
    // `UiGlobalTransform`, not `GlobalTransform`: bevy 0.19's layout writes the
    // node's placement here, and a UI node's `GlobalTransform` just sits at the
    // origin — reading that put every submenu in the window's top-left corner.
    geom: Query<(&UiGlobalTransform, &ComputedNode)>,
    mut nodes: Query<&mut Node>,
) {
    if rows.is_empty() {
        return;
    }

    // Rows kept open by the cursor being on them or on their own panel.
    let mut open: HashSet<Entity> = HashSet::default();
    for (row, interaction, sub) in &rows {
        let over_panel = panels
            .get(sub.panel)
            .map(|(rcp, _, _)| rcp.cursor_over)
            .unwrap_or(false);
        if *interaction != Interaction::None || over_panel {
            open.insert(row);
        }
    }
    // A cursor inside a *nested* panel is outside every one of its ancestor rows
    // and panels, so without this the whole chain behind it would collapse the
    // moment the pointer reached the deepest level. Walk each open row back up
    // its owner chain and keep the path open.
    for seed in open.iter().copied().collect::<Vec<_>>() {
        let mut cur = seed;
        while let Some(owner) = owner_row(cur, &parents, &panels) {
            if !open.insert(owner) {
                break;
            }
            cur = owner;
        }
    }

    let (win_w, win_h) = windows
        .iter()
        .next()
        .map(|w| (w.width(), w.height()))
        .unwrap_or((f32::MAX, f32::MAX));

    for (row, _, sub) in &rows {
        let show = open.contains(&row);
        let Ok(mut node) = nodes.get_mut(sub.panel) else {
            continue;
        };
        let want = if show { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
        if !show {
            continue;
        }
        let (Ok((row_ugt, row_cn)), Ok((_, panel_cn, _))) = (geom.get(row), panels.get(sub.panel))
        else {
            continue;
        };

        // UI transforms and computed sizes are physical pixels (and the
        // transform is the node's *centre*); a root node's `left`/`top` are
        // logical window pixels, so convert once here.
        let inv = row_cn.inverse_scale_factor();
        let half = row_cn.size() * 0.5 * inv;
        let mid = row_ugt.translation * inv;
        let panel_size = panel_cn.size() * inv;

        // Open to the right of the row; flip to its left when that would run off
        // the window (a menu opened against the right edge of the screen).
        // `panel_size` is zero until the panel has been laid out once, which
        // just means no flip on that first frame.
        let mut x = mid.x + half.x - OVERLAP;
        if panel_size.x > 0.0 && x + panel_size.x > win_w {
            x = (mid.x - half.x - panel_size.x + OVERLAP).max(0.0);
        }
        // Line the panel's first item up with its row, then slide up as needed
        // to keep the bottom on-screen.
        let mut y = mid.y - half.y - 4.0;
        if panel_size.y > 0.0 {
            y = y.min((win_h - panel_size.y).max(0.0));
        }
        let (left, top) = (Val::Px(x.max(0.0)), Val::Px(y.max(0.0)));
        if node.left != left {
            node.left = left;
        }
        if node.top != top {
            node.top = top;
        }
    }
}

/// The row owning the submenu panel `e` sits inside, if any.
fn owner_row(
    mut e: Entity,
    parents: &Query<&ChildOf>,
    panels: &Query<(&RelativeCursorPosition, &ComputedNode, &SubPanel)>,
) -> Option<Entity> {
    while let Ok(child_of) = parents.get(e) {
        let parent = child_of.parent();
        if let Ok((_, _, sub)) = panels.get(parent) {
            return Some(sub.owner);
        }
        e = parent;
    }
    None
}
