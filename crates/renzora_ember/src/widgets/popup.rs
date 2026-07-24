//! Generic popup behavior: a trigger that toggles a panel, with click-outside
//! dismiss — so every dropdown/menu/color-popup gets consistent open/close for
//! free instead of re-implementing it per panel.
//!
//! Attach [`Popup`] to a trigger entity (the clickable element). Ember drives the
//! `panel`'s `Node.display`: clicking the trigger toggles it, and clicking
//! anywhere outside both the trigger and the panel closes it. The panel must
//! carry a `RelativeCursorPosition` so outside-click can tell when the cursor is
//! over it.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, FocusPolicy, RelativeCursorPosition};
use bevy::window::{PrimaryWindow, SystemCursorIcon};

use crate::font::{icon_text, ui_font, EmberFonts};
use crate::reactive::bind_bg;
use crate::theme::*;


/// A menu item's one-shot action, run with `&mut World` when the item is
/// clicked. The menu closes immediately after.
#[derive(Component)]
pub struct MenuAction(pub Box<dyn Fn(&mut World) + Send + Sync>);

/// Hard cap on a [`screen_menu`]'s height; taller menus (e.g. the node-graph
/// "add node" list) clip + scroll with the wheel instead of overflowing the
/// window.
const SCREEN_MENU_MAX_H: f32 = 420.0;

// Row metrics shared by every screen-menu row ([`menu_item`], [`menu_header`],
// [`menu_submenu`](super::submenu::menu_submenu)). The glyph is deliberately
// larger than the label and the vertical padding deliberately thin — the
// Photoshop layer-menu feel, where you pick a row by its icon and a long list
// still fits on screen. Because the icon sets the row height, growing it and
// trimming the padding together keeps rows the same height as the old, smaller
// icon did.
pub(crate) const MENU_ICON: f32 = 15.0;
pub(crate) const MENU_TEXT: f32 = 12.0;
pub(crate) const MENU_PAD_X: f32 = 8.0;
pub(crate) const MENU_PAD_Y: f32 = 2.0;
pub(crate) const MENU_GAP: f32 = 7.0;

/// Spawn a floating menu at window position `(x, y)`. It's a [`ScreenMenu`] (kept
/// on-screen + pointer-blocking), dismissed by clicking outside it. Fill the
/// returned entity with [`menu_item`] / [`menu_item_styled`] / [`menu_sep`]
/// children, then it runs itself.
///
/// Items live inside a [`scroll_area`](super::scroll_area::scroll_area) capped at
/// [`SCREEN_MENU_MAX_H`], so a long menu scrolls (with a draggable, auto-hiding
/// scrollbar) instead of spilling off-screen. The returned entity is that scroll
/// content — add items to it exactly as before.
pub fn screen_menu(commands: &mut Commands, x: f32, y: f32) -> Entity {
    screen_menu_anchored(commands, x, Val::Px(y), Val::Auto)
}

/// Like [`screen_menu`], but flips the menu to open *upward* from `(x, y)` when
/// the click lands in the lower half of a `win_h`-tall window — so a context menu
/// near the bottom edge grows up from the cursor (the click sits at the menu's
/// bottom) instead of being clamped on top of it. `x` / `y` / `win_h` are window
/// logical pixels.
pub fn screen_menu_flip(commands: &mut Commands, x: f32, y: f32, win_h: f32) -> Entity {
    if y > win_h * 0.5 {
        // Pin the menu's bottom edge at the click; its items stack upward.
        screen_menu_anchored(commands, x, Val::Auto, Val::Px((win_h - y).max(0.0)))
    } else {
        screen_menu_anchored(commands, x, Val::Px(y), Val::Auto)
    }
}

/// Shared body of [`screen_menu`] / [`screen_menu_flip`]: spawns the floating menu
/// at `left = x` with the given vertical anchor (`top` for downward menus,
/// `bottom` for upward ones — the other should be [`Val::Auto`]).
fn screen_menu_anchored(commands: &mut Commands, x: f32, top: Val, bottom: Val) -> Entity {
    // Scroll content column — callers add their menu items here.
    let content = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Name::new("screen-menu-items"),
        ))
        .id();
    // Height-capped, scrollable viewport with an auto-hiding scrollbar thumb.
    let scroller = super::scroll_area::scroll_area(commands, content, SCREEN_MENU_MAX_H);
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x),
                top,
                bottom,
                flex_direction: FlexDirection::Column,
                min_width: Val::Px(184.0),
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            GlobalZIndex(9000),
            ScreenMenu,
            OverlaySurface,
            // Block pass-through directly at spawn — inserting it later (via an
            // `Added` system) races with menus that are despawned the same frame
            // (e.g. hover-switching a menu bar), crashing on the dead entity.
            FocusPolicy::Block,
            RelativeCursorPosition::default(),
            Name::new("screen-menu"),
        ))
        .id();
    commands.entity(root).add_child(scroller);
    content
}

/// Marks a floating UI surface (popup panel, dropdown list, context menu) that
/// should swallow pointer wheel/clicks so they never bleed through to the panels
/// or viewport behind it. Attached automatically to popups/dropdowns/menus.
#[derive(Component)]
pub struct OverlaySurface;

/// True while pointer interaction is **captured by a floating overlay or a
/// modal** — i.e. the cursor is over any *visible* [`OverlaySurface`] (dropdown
/// / menu / popup), OR a [`ModalSurface`](super::overlay::ModalSurface) is open
/// (modals block everything behind them regardless of cursor position).
///
/// Any panel-level wheel / drag / scrub handler (timeline zoom, code-editor
/// scroll, graph zoom, …) should bail when this is set, so an open overlay or
/// modal can't have its wheel *also* drive the panel sitting behind it. Bevy
/// broadcasts `MouseWheel` to every reader, so each such handler must opt out
/// itself — this resource is the shared signal they check.
#[derive(Resource, Default)]
pub struct PointerOverOverlay(pub bool);

/// Recompute [`PointerOverOverlay`] each frame from the visible overlay surfaces
/// and any open modal.
pub(crate) fn update_pointer_over_overlay(
    mut res: ResMut<PointerOverOverlay>,
    surfaces: Query<(&RelativeCursorPosition, &Node), With<OverlaySurface>>,
    modals: Query<(), With<super::overlay::ModalSurface>>,
) {
    let over = !modals.is_empty()
        || surfaces
            .iter()
            .any(|(rcp, node)| node.display != Display::None && rcp.cursor_over);
    if res.0 != over {
        res.0 = over;
    }
}

/// Tag every [`Popup`]'s panel as an [`OverlaySurface`] (and give it a
/// `RelativeCursorPosition` if it lacks one) so it blocks pass-through.
/// `try_insert`: a popup spawned inside a UI subtree that is torn down the
/// same frame (keyed-list/inspector rebuilds) has a dead panel by the time
/// this command applies — same race the `screen_menu` spawn-time tagging
/// avoids.
pub(crate) fn tag_popup_panels(q: Query<&Popup, Added<Popup>>, mut commands: Commands) {
    for p in &q {
        // `HideInHierarchy` marks this as editor chrome so the scene saver never
        // serializes it (a popup open when auto-save fires would otherwise be
        // baked into the scene as leaked UI).
        commands.entity(p.panel).try_insert((
            OverlaySurface,
            RelativeCursorPosition::default(),
            renzora::HideInHierarchy,
        ));
    }
}

/// An icon + label menu row that runs `on_click` (and closes the menu) when
/// pressed. Standard colors.
pub fn menu_item<F>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
    on_click: F,
) -> Entity
where
    F: Fn(&mut World) + Send + Sync + 'static,
{
    menu_item_styled(commands, fonts, icon, label, text_muted(), text_primary(), on_click)
}

/// [`menu_item`] with explicit icon/text colors (e.g. a red Delete).
pub fn menu_item_styled<F>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
    icon_color: (u8, u8, u8),
    text_color: (u8, u8, u8),
    on_click: F,
) -> Entity
where
    F: Fn(&mut World) + Send + Sync + 'static,
{
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
            MenuAction(Box::new(on_click)),
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("menu-item"),
        ))
        .id();
    bind_bg(commands, row, move |w| match w.get::<Interaction>(row) {
        Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(hover_bg()),
        _ => Color::NONE,
    });
    let ic = icon_text(commands, &fonts.phosphor, icon, icon_color, MENU_ICON);
    let t = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, MENU_TEXT),
            TextColor(rgb(text_color)),
        ))
        .id();
    commands.entity(row).add_children(&[ic, t]);
    row
}

/// A rich, Unreal "Place Actors"-style menu row: a thumbnail icon box, a colored
/// accent bar, and a two-line label (title + small uppercase subtitle). Runs
/// `on_click` (and closes the menu) when pressed. `accent` tints the icon and
/// the bar so each row reads as its own type at a glance.
#[allow(clippy::too_many_arguments)]
pub fn menu_card<F>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    title: &str,
    subtitle: &str,
    accent: (u8, u8, u8),
    on_click: F,
) -> Entity
where
    F: Fn(&mut World) + Send + Sync + 'static,
{
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            MenuAction(Box::new(on_click)),
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("menu-card"),
        ))
        .id();
    bind_bg(commands, row, move |w| match w.get::<Interaction>(row) {
        Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(hover_bg()),
        _ => Color::NONE,
    });

    // Thumbnail icon box — a dark inset square holding the accent-tinted glyph.
    let glyph = icon_text(commands, &fonts.phosphor, icon, accent, 15.0);
    let icon_box = commands
        .spawn((
            Node {
                width: Val::Px(26.0),
                height: Val::Px(26.0),
                flex_shrink: 0.0,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.22)),
        ))
        .id();
    commands.entity(icon_box).add_children(&[glyph]);

    // Unreal's left-edge accent stripe between the thumbnail and the label.
    let bar = commands
        .spawn((
            Node {
                width: Val::Px(3.0),
                height: Val::Px(20.0),
                flex_shrink: 0.0,
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(rgb(accent)),
        ))
        .id();

    // Title (prominent) over a small, muted, uppercase type subtitle.
    let title_t = commands
        .spawn((
            Text::new(title),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let sub_t = commands
        .spawn((
            Text::new(subtitle.to_uppercase()),
            ui_font(&fonts.ui, 8.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let text_col = commands
        .spawn((Node {
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(1.0),
            ..default()
        },))
        .id();
    commands.entity(text_col).add_children(&[title_t, sub_t]);

    commands.entity(row).add_children(&[icon_box, bar, text_col]);
    row
}

/// A non-interactive section header for a [`screen_menu`] — a small, muted,
/// uppercase label (Unreal's content-browser "CREATE BASIC ASSET" style) that
/// titles the group of items beneath it. Not clickable; carries no
/// [`MenuAction`].
pub fn menu_header(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::new(
                    Val::Px(MENU_PAD_X),
                    Val::Px(MENU_PAD_X),
                    Val::Px(5.0),
                    Val::Px(3.0),
                ),
                ..default()
            },
            Name::new("menu-header"),
        ))
        .id();
    let t = commands
        .spawn((
            Text::new(label.to_uppercase()),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    commands.entity(row).add_children(&[t]);
    row
}

/// A thin divider for a [`screen_menu`].
pub fn menu_sep(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(1.0),
                margin: UiRect::vertical(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(rgb(divider())),
        ))
        .id()
}

/// Click a [`MenuAction`] item → run its action (deferred, with `&mut World`) and
/// close every open [`ScreenMenu`].
pub(crate) fn menu_action_run(
    q: Query<(Entity, &Interaction), (Changed<Interaction>, With<MenuAction>)>,
    menus: Query<Entity, With<ScreenMenu>>,
    mut commands: Commands,
) {
    let mut any = false;
    for (e, interaction) in &q {
        if *interaction == Interaction::Pressed {
            any = true;
            commands.queue(move |world: &mut World| {
                let action = world
                    .get_entity_mut(e)
                    .ok()
                    .and_then(|mut em| em.take::<MenuAction>());
                if let Some(action) = action {
                    (action.0)(world);
                }
            });
        }
    }
    if any {
        for m in &menus {
            commands.entity(m).despawn();
        }
    }
}

/// Press outside an open [`ScreenMenu`] → close it.
pub(crate) fn screen_menu_dismiss(
    mouse: Res<ButtonInput<MouseButton>>,
    menus: Query<(Entity, &RelativeCursorPosition, Ref<ScreenMenu>)>,
    submenus: Query<(&RelativeCursorPosition, &Node), With<super::submenu::SubPanel>>,
    mut commands: Commands,
) {
    if !mouse.just_pressed(MouseButton::Left) && !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    // A submenu panel hangs *outside* its menu root's rect, so a click inside one
    // reads as "outside the menu" here. Clicking a submenu item is handled by
    // `menu_action_run` (which closes the menu itself); anything else in there
    // must leave the menu standing.
    if super::submenu::cursor_over_submenu(&submenus) {
        return;
    }
    for (e, rcp, menu) in &menus {
        // A menu spawned this frame is the one this very press just opened (a
        // sync point can make it visible here before the press is over), and it
        // has no cursor-over state yet — dismissing it would close menus on the
        // click that asked for them.
        if menu.is_added() {
            continue;
        }
        if !rcp.cursor_over {
            commands.entity(e).despawn();
        }
    }
}

/// Marks a floating menu/overlay positioned in **window coordinates** (absolute
/// `left`/`top`, e.g. a context menu opened at the cursor). Ember gives it two
/// behaviors for free: it absorbs pointer events so hover/click never leaks to
/// nodes behind it, and it's clamped to stay fully on-screen (an overlay opened
/// near an edge is pulled back into view instead of being cut off). Add it to
/// the overlay's root node — that's all.
#[derive(Component)]
pub struct ScreenMenu;

/// Clamp each [`ScreenMenu`]'s absolute top-left so its measured box fits inside
/// the window.
pub(crate) fn screen_menu_clamp(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut menus: Query<(&mut Node, &ComputedNode), With<ScreenMenu>>,
) {
    let Some(win) = windows.iter().next() else {
        return;
    };
    let (ww, wh) = (win.width(), win.height());
    for (mut node, cn) in &mut menus {
        let size = cn.size() * cn.inverse_scale_factor();
        if let Val::Px(x) = node.left {
            let nx = x.min((ww - size.x).max(0.0)).max(0.0);
            if (nx - x).abs() > 0.5 {
                node.left = Val::Px(nx);
            }
        }
        if let Val::Px(y) = node.top {
            let ny = y.min((wh - size.y).max(0.0)).max(0.0);
            if (ny - y).abs() > 0.5 {
                node.top = Val::Px(ny);
            }
        }
        // Upward-anchored menus (see `screen_menu_flip`) position by `bottom`;
        // keep their top edge (`wh - bottom - size.y`) on-screen.
        if let Val::Px(b) = node.bottom {
            let nb = b.min((wh - size.y).max(0.0)).max(0.0);
            if (nb - b).abs() > 0.5 {
                node.bottom = Val::Px(nb);
            }
        }
    }
}

/// Marks a trigger that opens/closes `panel`. Build the panel with
/// `display: Display::None` + a `RelativeCursorPosition`.
#[derive(Component)]
pub struct Popup {
    pub panel: Entity,
    pub open: bool,
}

impl Popup {
    pub fn new(panel: Entity) -> Self {
        Self { panel, open: false }
    }
}

/// Close a popup by its trigger entity (e.g. after picking an option).
pub fn close_popup(commands: &mut Commands, trigger: Entity) {
    commands.queue(move |world: &mut World| {
        let panel = world.get::<Popup>(trigger).map(|p| p.panel);
        if let Some(mut p) = world.get_mut::<Popup>(trigger) {
            p.open = false;
        }
        if let Some(panel) = panel {
            if let Some(mut n) = world.get_mut::<Node>(panel) {
                n.display = Display::None;
            }
        }
    });
}

fn set_panel_display(nodes: &mut Query<&mut Node>, panel: Entity, open: bool) {
    if let Ok(mut n) = nodes.get_mut(panel) {
        let want = if open { Display::Flex } else { Display::None };
        if n.display != want {
            n.display = want;
        }
    }
}

/// Click the trigger → toggle its panel.
pub(crate) fn popup_toggle(
    mut triggers: Query<(&Interaction, &mut Popup), Changed<Interaction>>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, mut p) in &mut triggers {
        if *interaction != Interaction::Pressed {
            continue;
        }
        p.open = !p.open;
        let (panel, open) = (p.panel, p.open);
        set_panel_display(&mut nodes, panel, open);
    }
}

/// Flip an open popup above its trigger when opening below would overflow the
/// bottom of the window (and there's room above) — so popups near the bottom of
/// a scroll area aren't cut off.
pub(crate) fn popup_position(
    triggers: Query<(&Popup, &ComputedNode, &GlobalTransform)>,
    panels: Query<&ComputedNode>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut nodes: Query<&mut Node>,
) {
    let Some(win_h) = windows.iter().next().map(|w| w.physical_height() as f32) else {
        return;
    };
    for (p, trigger_cn, gt) in &triggers {
        if !p.open {
            continue;
        }
        let Ok(panel_cn) = panels.get(p.panel) else {
            continue;
        };
        let panel_h = panel_cn.size().y;
        let half = trigger_cn.size().y * 0.5;
        let center = gt.translation().y;
        // Flip up only when opening below overflows AND there's room above.
        let flip = (center + half) + panel_h > win_h && (center - half) - panel_h > 0.0;
        if let Ok(mut n) = nodes.get_mut(p.panel) {
            let (top, bottom) = if flip {
                (Val::Auto, Val::Percent(100.0))
            } else {
                (Val::Percent(100.0), Val::Auto)
            };
            if n.top != top || n.bottom != bottom {
                n.top = top;
                n.bottom = bottom;
                n.margin.top = if flip { Val::Px(0.0) } else { Val::Px(2.0) };
                n.margin.bottom = if flip { Val::Px(2.0) } else { Val::Px(0.0) };
            }
        }
    }
}

/// Press anywhere outside an open popup's trigger + panel → close it.
pub(crate) fn popup_dismiss(
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Query<&RelativeCursorPosition>,
    mut triggers: Query<(&Interaction, &mut Popup)>,
    mut nodes: Query<&mut Node>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    for (interaction, mut p) in &mut triggers {
        if !p.open {
            continue;
        }
        let over_panel = cursor.get(p.panel).map(|r| r.cursor_over).unwrap_or(false);
        // Trigger is `None` only when the cursor isn't over it.
        if *interaction == Interaction::None && !over_panel {
            p.open = false;
            let panel = p.panel;
            set_panel_display(&mut nodes, panel, false);
        }
    }
}
