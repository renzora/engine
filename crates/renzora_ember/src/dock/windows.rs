//! Floating dock windows: tearing a panel off into its own OS window, the
//! chrome that window wears, dragging it around (with re-dock on release), and
//! tearing it down again.
//!
//! The teardown ordering is the subtle part. A `Camera` whose
//! `RenderTarget::Window` entity is already gone panics `camera_system`, so a
//! window, its camera and its UI root must die in the *same* frame, before
//! bevy's camera update — which is why every close path funnels through
//! [`DockWindowCloseRequests`] instead of despawning directly.

use bevy::prelude::*;

use crate::font::{icon_text, ui_font, EmberFonts};
use crate::theme::{border, divider, header_bg, rgb, text_muted, text_primary};

use crate::dock::components::{DockLeaf, GlobalCursor, RootDropOverlay, TabBarOf};
use crate::dock::drag::{
    insert_action, pick_root_zone, set_root_zone_rect, set_zone_rect, DropAction,
};
use crate::dock::routing::{area_tree_mut, flag_area_dirty, window_contains, window_local};
use crate::dock::tree::{DockTree, DropZone};
use crate::dock::{humanize, Dock, DockArea, DockDirty, FixedDock};

/// One floating OS window hosting its own [`DockTree`] — created by
/// Ctrl+dragging a tab out of a dock (tear-off) or programmatically via
/// [`DockWindowRequests`]. Each window is a full dock: tabs, splits and
/// drag-docking all work inside it, and tabs drag between windows.
pub struct DockWindowState {
    /// The OS window entity.
    pub window: Entity,
    /// The `Camera2d` rendering this window's UI.
    pub camera: Entity,
    /// The root UI node in this window (title bar + dock area + resize grip).
    pub root: Entity,
    /// The [`DockArea`] node the tree reconciles into.
    pub area: Entity,
    /// This window's live dock layout.
    pub tree: DockTree,
    /// Per-window rebuild flag — the floating counterpart of [`DockDirty`].
    pub dirty: bool,
}

/// All live floating dock windows. The editor shell reads this to persist
/// floating layouts; everything else goes through the dock's own systems.
#[derive(Resource, Default)]
pub struct DockWindows(pub Vec<DockWindowState>);

/// Request to open a floating dock window. Push onto [`DockWindowRequests`];
/// the dock spawns the OS window + camera + chrome next frame (or the same
/// frame for the tear-off gesture, which runs before the spawn system).
pub struct DockWindowRequest {
    /// The layout the new window opens with.
    pub tree: DockTree,
    /// Desired client-area origin in physical screen px (`None` = OS default).
    pub position: Option<IVec2>,
    /// Client-area size in physical px.
    pub size: UVec2,
    /// Tear-off: the window follows the held cursor until mouse release.
    pub grab: bool,
}

/// Queue of pending [`DockWindowRequest`]s (public seam — the shell uses this
/// to restore persisted floating windows at startup).
#[derive(Resource, Default)]
pub struct DockWindowRequests(pub Vec<DockWindowRequest>);

/// A floating dock window being dragged: the tear-off follow (until the
/// gesture's button releases) and title-bar drags share this. Title-bar drags
/// also resolve a re-dock drop — release over a tab bar or a dock root
/// edge/corner in the main window and the panel docks back there.
struct FloatDragState {
    window: Entity,
    /// Cursor → window-origin offset in logical px (scaled per frame, so the
    /// grab survives the window crossing monitors with different DPI).
    grab: Vec2,
    /// Whether releasing over a dock target re-docks the panel. `false` for
    /// the tear-off gesture (its release point is still inside the tab bar it
    /// just left — re-docking there would undo the tear-off).
    redock: bool,
    action: Option<DropAction>,
    shown_overlay: Option<Entity>,
}

/// The active floating-window drag, if any.
#[derive(Resource, Default)]
pub(crate) struct FloatDrag(Option<FloatDragState>);

/// Marks a floating window's [`DockArea`] and links it back to its OS window.
/// The primary dock area (the editor shell's) does NOT carry this — that's how
/// dock systems tell "mutate `Dock.tree`" from "mutate this window's tree".
#[derive(Component)]
pub struct FloatingDockArea {
    pub window: Entity,
}

/// A floating dock window's title bar — press starts a window drag (which can
/// end in a re-dock, see [`FloatDragState`]).
#[derive(Component)]
pub(crate) struct FloatWindowBar(Entity);

/// A floating dock window's close button (×).
#[derive(Component)]
pub(crate) struct FloatWindowClose(Entity);

/// A floating dock window's edge/corner resize zone — press starts an OS
/// resize toward the given octant.
#[derive(Component)]
pub(crate) struct FloatWindowResize {
    window: Entity,
    octant: bevy::math::CompassOctant,
}

/// Floating dock windows queued to close (window entities). All close paths —
/// the title-bar ×, dragging the last panel out, a re-dock — go through this
/// queue instead of despawning the window directly: [`process_dock_window_closes`]
/// tears the window, its camera and its UI root down **together, before bevy's
/// camera update**. A `Camera` whose `RenderTarget::Window` entity is already
/// gone panics `camera_system` ("RenderTarget::Window missing"), so the camera
/// must never outlive its window across that boundary — not even one frame.
#[derive(Resource, Default)]
pub(crate) struct DockWindowCloseRequests(pub(crate) Vec<Entity>);

/// Height of a floating dock window's title bar, logical px.
pub(crate) const FLOAT_TITLEBAR_H: f32 = 26.0;

/// If `area` belongs to a floating dock window whose tree just emptied, queue
/// that window for close — [`process_dock_window_closes`] tears it down at the
/// end of the frame. An empty floating shell has no way to gain panels, so
/// leaving it open is just clutter.
pub(crate) fn close_empty_dock_window(
    area: Entity,
    wins: &DockWindows,
    closes: &mut DockWindowCloseRequests,
) {
    if let Some(st) = wins.0.iter().find(|s| s.area == area) {
        if st.tree.is_empty() {
            closes.0.push(st.window);
        }
    }
}

/// Undock `id` from `source_area` into a new floating window: remove it from
/// the owning tree, size the window like the leaf it came from (`leaf_size` in
/// physical px, plus the title bar), and queue the window request. `grab` makes
/// the new window follow the held cursor until release (the drag gestures);
/// the context-menu path passes `false` so the window just opens under the
/// cursor. Shared by Ctrl+drag, the header grip, and the right-click menu.
pub(crate) fn tear_off_panel(
    id: &str,
    source_area: Entity,
    leaf_size: Option<Vec2>,
    scale: f32,
    cursor: Option<Vec2>,
    grab: bool,
    (dock, fixed, dirty, wins, requests, closes): (
        &mut Dock,
        &mut FixedDock,
        &mut DockDirty,
        &mut DockWindows,
        &mut DockWindowRequests,
        &mut DockWindowCloseRequests,
    ),
) {
    let size = leaf_size.unwrap_or(Vec2::new(480.0, 360.0) * scale);
    let size = UVec2::new(
        (size.x.clamp(240.0 * scale, 1600.0 * scale)) as u32,
        (size.y + FLOAT_TITLEBAR_H * scale).clamp(160.0 * scale, 1200.0 * scale) as u32,
    );
    area_tree_mut(source_area, dock, fixed, wins).remove_panel(id);
    flag_area_dirty(source_area, dirty, fixed, wins);
    close_empty_dock_window(source_area, wins, closes);
    // Put the title bar under the cursor so the tear-off reads as grabbing the
    // new window by its header.
    let position = cursor
        .map(|p| (p - Vec2::new(60.0, FLOAT_TITLEBAR_H * 0.5) * scale).round().as_ivec2());
    requests.0.push(DockWindowRequest {
        tree: DockTree::leaf(id),
        position,
        size,
        grab,
    });
}

// ── Floating dock window lifecycle ───────────────────────────────────────────

/// Drain [`DockWindowRequests`]: spawn the OS window, a `Camera2d` targeting
/// it, and its chrome (title bar with close ×, the [`DockArea`], a corner
/// resize grip). The window is undecorated to match the editor's borderless
/// look — the title bar drives OS move via `start_drag_move`, the grip OS
/// resize — which also keeps its `position` exactly the client-area origin
/// (no title-bar offset in the screen-space math).
pub(crate) fn spawn_dock_windows(
    mut requests: ResMut<DockWindowRequests>,
    mut wins: ResMut<DockWindows>,
    mut drag: ResMut<FloatDrag>,
    fonts: Option<Res<EmberFonts>>,
    splash: Option<Res<State<renzora::SplashState>>>,
    registry: Option<Res<renzora::core::ShellPanelRegistry>>,
    mut commands: Commands,
) {
    if requests.0.is_empty() {
        return;
    }
    // Keep requests queued until fonts exist (startup restore can race them),
    // and — in the editor — until the splash/loading phases are over, so
    // restored floating windows don't pop up over the splash screen. Games
    // don't register `SplashState`; they spawn immediately.
    let Some(fonts) = fonts else {
        return;
    };
    if splash.is_some_and(|s| *s.get() != renzora::SplashState::Editor) {
        return;
    }
    for req in requests.0.drain(..) {
        let lead = req.tree.first_panel().unwrap_or("panels");
        let title = registry
            .as_ref()
            .and_then(|r| r.panels.get(lead))
            .map(|info| renzora::lang::t_or(&format!("panel.{lead}"), &info.title))
            .unwrap_or_else(|| humanize(lead));

        let window = commands
            .spawn((
                Window {
                    title: format!("Renzora — {title}"),
                    resolution: bevy::window::WindowResolution::new(req.size.x, req.size.y),
                    position: req
                        .position
                        .map(bevy::window::WindowPosition::At)
                        .unwrap_or(bevy::window::WindowPosition::Automatic),
                    decorations: false,
                    resizable: true,
                    // Normal window level (NOT always-on-top): floats layer
                    // like any OS window so they can live on other monitors
                    // without sitting over everything. Clicking the maximized
                    // main window will raise it over a float on the SAME
                    // monitor — that's standard OS behavior; alt-tab or click
                    // the float's taskbar entry to bring it back.
                    ..default()
                },
                // Seed a cursor icon so the undecorated window always shows
                // one; `apply_cursor_icon` retargets it on hover after that.
                bevy::window::CursorIcon::System(bevy::window::SystemCursorIcon::Default),
                // Engine chrome, not scene content: keep it out of the
                // hierarchy panel AND out of the scene-clear sweep (which
                // despawns named entities without this marker).
                renzora::HideInHierarchy,
                Name::new("dock-window"),
            ))
            .id();
        let camera = commands
            .spawn((
                Camera2d,
                Camera::default(),
                bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(window)),
                renzora::HideInHierarchy,
                Name::new("dock-window-camera"),
            ))
            .id();

        // Root column: title bar / dock area, framed by a dark border so the
        // undecorated window reads as a panel against whatever is behind it.
        // `UiTargetCamera` on the root routes the whole subtree onto this
        // window's camera.
        let root = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(rgb(crate::theme::window_bg())),
                BorderColor::all(rgb(border())),
                bevy::ui::UiTargetCamera(camera),
                renzora::HideInHierarchy,
                Name::new("dock-window-root"),
            ))
            .id();

        let bar = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(FLOAT_TITLEBAR_H),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    padding: UiRect::horizontal(Val::Px(8.0)),
                    flex_shrink: 0.0,
                    border: UiRect::bottom(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(rgb(header_bg())),
                BorderColor::all(rgb(divider())),
                Interaction::default(),
                FloatWindowBar(window),
                Name::new("dock-window-titlebar"),
            ))
            .id();
        let bar_title = commands
            .spawn((
                Text::new(title),
                ui_font(&fonts.ui, 12.0),
                TextColor(rgb(text_primary())),
                bevy::text::TextLayout::no_wrap(),
            ))
            .id();
        let bar_fill = commands
            .spawn((Node {
                flex_grow: 1.0,
                ..default()
            },))
            .id();
        let close = icon_text(&mut commands, &fonts.phosphor, "x", text_muted(), 12.0);
        commands.entity(close).insert((
            Interaction::default(),
            crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            // Block so the press closes instead of starting a window drag.
            bevy::ui::FocusPolicy::Block,
            FloatWindowClose(window),
        ));
        commands.entity(bar).add_children(&[bar_title, bar_fill, close]);

        let area = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    min_width: Val::Px(0.0),
                    min_height: Val::Px(0.0),
                    flex_basis: Val::Px(0.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                DockArea,
                FloatingDockArea { window },
                Name::new("dock-window-area"),
            ))
            .id();

        let mut kids = vec![bar, area];
        kids.extend(spawn_float_resize_zones(&mut commands, window));
        commands.entity(root).add_children(&kids);

        bevy::log::info!(
            "[dock] spawned floating window {window} (camera {camera}, root {root}) for '{lead}'"
        );
        wins.0.push(DockWindowState {
            window,
            camera,
            root,
            area,
            tree: req.tree,
            dirty: true,
        });
        if req.grab {
            drag.0 = Some(FloatDragState {
                window,
                grab: Vec2::new(60.0, FLOAT_TITLEBAR_H * 0.5),
                // Tear-off: the release point is still over the tab bar the
                // panel just left — re-docking there would undo the gesture.
                redock: false,
                action: None,
                shown_overlay: None,
            });
        }
    }
}

/// Edge + corner resize zones around a floating window's perimeter, each with
/// the matching resize cursor. Thin absolute strips overlaid on the border;
/// corners are spawned last so they win the hit-test where they overlap edges.
fn spawn_float_resize_zones(commands: &mut Commands, window: Entity) -> Vec<Entity> {
    use bevy::math::CompassOctant as O;
    use bevy::window::SystemCursorIcon as C;
    const EDGE: f32 = 5.0;
    const CORNER: f32 = 12.0;
    let full = Val::Percent(100.0);
    let zones: [(O, C, Val, Val, Val, Val, Val, Val); 8] = [
        // (octant, cursor, left, right, top, bottom, width, height)
        (O::North, C::NsResize, Val::Px(CORNER), Val::Auto, Val::Px(0.0), Val::Auto, full, Val::Px(EDGE)),
        (O::South, C::NsResize, Val::Px(CORNER), Val::Auto, Val::Auto, Val::Px(0.0), full, Val::Px(EDGE)),
        (O::West, C::EwResize, Val::Px(0.0), Val::Auto, Val::Px(CORNER), Val::Auto, Val::Px(EDGE), full),
        (O::East, C::EwResize, Val::Auto, Val::Px(0.0), Val::Px(CORNER), Val::Auto, Val::Px(EDGE), full),
        (O::NorthWest, C::NwResize, Val::Px(0.0), Val::Auto, Val::Px(0.0), Val::Auto, Val::Px(CORNER), Val::Px(CORNER)),
        (O::NorthEast, C::NeResize, Val::Auto, Val::Px(0.0), Val::Px(0.0), Val::Auto, Val::Px(CORNER), Val::Px(CORNER)),
        (O::SouthWest, C::SwResize, Val::Px(0.0), Val::Auto, Val::Auto, Val::Px(0.0), Val::Px(CORNER), Val::Px(CORNER)),
        (O::SouthEast, C::SeResize, Val::Auto, Val::Px(0.0), Val::Auto, Val::Px(0.0), Val::Px(CORNER), Val::Px(CORNER)),
    ];
    zones
        .into_iter()
        .map(|(octant, cursor, left, right, top, bottom, width, height)| {
            commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left,
                        right,
                        top,
                        bottom,
                        width,
                        height,
                        ..default()
                    },
                    Interaction::default(),
                    crate::resize::ResizeHandle,
                    crate::cursor_icon::HoverCursor(cursor),
                    FloatWindowResize { window, octant },
                    bevy::ui::GlobalZIndex(200),
                    Name::new("dock-window-resize"),
                ))
                .id()
        })
        .collect()
}

/// Floating window chrome: title-bar press starts a window drag (with re-dock
/// on release, see [`float_window_drag`]), a resize zone starts an OS resize,
/// the × queues the window's close (its panel returns to the main dock via
/// [`process_dock_window_closes`]).
pub(crate) fn float_window_controls(
    bars: Query<(&Interaction, &FloatWindowBar), Changed<Interaction>>,
    resizes: Query<(&Interaction, &FloatWindowResize), Changed<Interaction>>,
    closes: Query<(&Interaction, &FloatWindowClose), Changed<Interaction>>,
    mut windows: Query<&mut Window>,
    mut drag: ResMut<FloatDrag>,
    mut close_queue: ResMut<DockWindowCloseRequests>,
) {
    for (i, bar) in &bars {
        if *i == Interaction::Pressed && drag.0.is_none() {
            if let Ok(w) = windows.get_mut(bar.0) {
                // Grab offset = where inside the window the cursor pressed, so
                // the window doesn't jump under the cursor.
                let grab = w.cursor_position().unwrap_or(Vec2::new(60.0, FLOAT_TITLEBAR_H * 0.5));
                drag.0 = Some(FloatDragState {
                    window: bar.0,
                    grab,
                    redock: true,
                    action: None,
                    shown_overlay: None,
                });
            }
        }
    }
    for (i, rz) in &resizes {
        if *i == Interaction::Pressed {
            if let Ok(mut w) = windows.get_mut(rz.window) {
                w.start_drag_resize(rz.octant);
            }
        }
    }
    for (i, close) in &closes {
        if *i == Interaction::Pressed {
            close_queue.0.push(close.0);
        }
    }
}

/// Drive an in-flight floating-window drag: keep the window under the cursor
/// (physical screen space, so it crosses monitors), and — for title-bar drags —
/// resolve a re-dock target in the main window. Only *small, deliberate*
/// targets re-dock: a leaf's tab bar (dock as a tab there) or the dock's root
/// edge/corner bands (full-height/width split). Leaf centers don't — the main
/// window is usually maximized, so a greedy target would re-dock every drop.
#[allow(clippy::too_many_arguments)]
pub(crate) fn float_window_drag(
    mut drag: ResMut<FloatDrag>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Res<GlobalCursor>,
    mut windows: Query<&mut Window>,
    primary: Query<Entity, With<bevy::window::PrimaryWindow>>,
    areas: Query<
        (
            Entity,
            &bevy::ui::ComputedNode,
            &bevy::ui::UiGlobalTransform,
            Option<&FloatingDockArea>,
        ),
        With<DockArea>,
    >,
    tabbars: Query<(&TabBarOf, &bevy::ui::ComputedNode, &bevy::ui::UiGlobalTransform)>,
    leaves: Query<&DockLeaf>,
    root_overlays: Query<(Entity, &RootDropOverlay)>,
    mut nodes: Query<&mut Node>,
    mut dock: ResMut<Dock>,
    mut fixed: ResMut<FixedDock>,
    mut dirty: ResMut<DockDirty>,
    mut wins: ResMut<DockWindows>,
    mut close_queue: ResMut<DockWindowCloseRequests>,
) {
    if drag.0.is_none() {
        return;
    }

    // ── Release: apply the re-dock (if any) and end the drag. ──
    if !mouse.pressed(MouseButton::Left) {
        let Some(state) = drag.0.take() else {
            return;
        };
        if let Some(e) = state.shown_overlay {
            if let Ok(mut n) = nodes.get_mut(e) {
                n.display = Display::None;
            }
        }
        if let Some(action) = state.action {
            // Move the float's panel (floats host exactly one) into the target
            // tree, then close the emptied window.
            let panel = wins
                .0
                .iter()
                .find(|s| s.window == state.window)
                .and_then(|s| s.tree.first_panel().map(str::to_string));
            if let Some(panel) = panel {
                if let Some(st) = wins.0.iter_mut().find(|s| s.window == state.window) {
                    st.tree.remove_panel(&panel);
                }
                insert_action(
                    area_tree_mut(action.area(), &mut dock, &mut fixed, &mut wins),
                    &panel,
                    &action,
                );
                flag_area_dirty(action.area(), &mut dirty, &mut fixed, &mut wins);
                close_queue.0.push(state.window);
            }
        }
        return;
    }

    let Some(state) = drag.0.as_mut() else {
        return;
    };

    // ── Move the window under the cursor. ──
    let Some(pos) = cursor.pos else {
        return;
    };
    let Ok(mut window) = windows.get_mut(state.window) else {
        return;
    };
    let scale = window.scale_factor();
    let target = (pos - state.grab * scale).round().as_ivec2();
    if window.position != bevy::window::WindowPosition::At(target) {
        window.position = bevy::window::WindowPosition::At(target);
    }

    // ── Re-dock targeting (title-bar drags only), in the primary window. ──
    let mut action: Option<DropAction> = None;
    let mut overlay: Option<(Entity, DropZone, bool)> = None;
    if state.redock {
        let primary_local = primary.single().ok().and_then(|pw| {
            windows
                .get(pw)
                .ok()
                .filter(|w| window_contains(w, pos))
                .and_then(|w| window_local(w, pos))
        });
        if let Some(local) = primary_local {
            let primary_area = areas.iter().find(|(.., f)| f.is_none());
            // Tab bars first: dock as a tab in that leaf.
            let bar_hit = tabbars.iter().find_map(|(bar, cn, gt)| {
                cn.contains_point(*gt, local).then_some(bar.0)
            });
            if let Some(leaf_ent) = bar_hit {
                if let Ok(ld) = leaves.get(leaf_ent) {
                    // Only main-window leaves are targets.
                    if primary_area.is_some_and(|(a, ..)| a == ld.area) {
                        if let Some(rep) = ld.tabs.first().cloned() {
                            action = Some(DropAction::Tab {
                                area: ld.area,
                                rep,
                                before: None,
                            });
                            overlay = Some((ld.overlay, DropZone::Center, false));
                        }
                    }
                }
            } else if let Some((area_e, cn, gt, _)) = primary_area {
                // Root edge/corner bands of the main dock.
                if let Some(norm) = cn.normalize_point(*gt, local) {
                    let size = cn.size() * cn.inverse_scale_factor();
                    let (x, y) = ((norm.x + 0.5) * size.x, (norm.y + 0.5) * size.y);
                    if let Some((zone, _)) = pick_root_zone(x, y, size) {
                        action = Some(DropAction::RootSplit { area: area_e, zone });
                        overlay = root_overlays
                            .iter()
                            .find(|(_, o)| o.area == area_e)
                            .map(|(e, _)| (e, zone, true));
                    }
                }
            }
        }
    }
    state.action = action;

    // Show/hide the drop preview (same mechanics as the tab drag).
    let overlay_e = overlay.map(|(e, _, _)| e);
    if state.shown_overlay != overlay_e {
        if let Some(old) = state.shown_overlay {
            if let Ok(mut n) = nodes.get_mut(old) {
                n.display = Display::None;
            }
        }
        state.shown_overlay = overlay_e;
    }
    if let Some((e, zone, is_root)) = overlay {
        if let Ok(mut n) = nodes.get_mut(e) {
            n.display = Display::Flex;
            if is_root {
                set_root_zone_rect(&mut n, zone);
            } else {
                set_zone_rect(&mut n, zone);
            }
        }
    }
}

/// Tear down closing floating dock windows: everything queued in
/// [`DockWindowCloseRequests`] (the ×, last-panel-out, re-docks), plus
/// `WindowCloseRequested` messages for our windows (Alt+F4 — handled here so
/// the teardown is atomic; bevy's `close_when_requested` marker dance would
/// despawn the window a frame before we'd notice), plus any window despawned
/// externally (caught via `RemovedComponents` the same frame).
///
/// The window's panels return to the primary dock so they're never lost, and
/// the window, its camera and its UI root despawn in ONE command batch. This
/// system runs in `PostUpdate` **before** `CameraUpdateSystems`: a camera whose
/// `RenderTarget::Window` entity is already gone panics `camera_system`, so the
/// camera must never survive its window into that set — not even one frame.
///
/// Also despawns all remaining floating windows when the PRIMARY window is
/// closed, so the app's `ExitCondition::OnAllClosed` fires instead of the
/// process lingering with orphaned tool windows.
pub(crate) fn process_dock_window_closes(
    mut queue: ResMut<DockWindowCloseRequests>,
    mut close_requested: MessageReader<bevy::window::WindowCloseRequested>,
    mut removed: RemovedComponents<Window>,
    primary: Query<Entity, With<bevy::window::PrimaryWindow>>,
    windows: Query<(), With<Window>>,
    mut last_primary: Local<Option<Entity>>,
    mut wins: ResMut<DockWindows>,
    mut dock: ResMut<Dock>,
    mut dirty: ResMut<DockDirty>,
    mut commands: Commands,
) {
    // Remember the primary window entity while it exists — once it's despawned
    // it can't be queried, and the removal event alone doesn't say whose it
    // was. (Our own float despawns surface here as removals a frame later, so
    // "any removed window that isn't ours" would misread them as the primary.)
    if let Ok(pw) = primary.single() {
        *last_primary = Some(pw);
    }

    let mut to_close: Vec<(Entity, &'static str)> =
        queue.0.drain(..).map(|e| (e, "requested")).collect();
    // Alt+F4 / OS close on one of OUR windows only — the primary window's
    // close is the shell chrome's (and bevy's) business.
    for ev in close_requested.read() {
        if wins.0.iter().any(|s| s.window == ev.window) {
            to_close.push((ev.window, "os-close"));
        }
    }
    let externally_removed: Vec<Entity> = removed.read().collect();
    to_close.extend(externally_removed.iter().map(|e| (*e, "despawned-externally")));
    // Belt-and-braces: a state whose window entity no longer exists but never
    // surfaced through the paths above (missed events). Whatever despawned it,
    // the camera + root must not linger.
    for st in &wins.0 {
        if !windows.contains(st.window) {
            to_close.push((st.window, "window-vanished"));
        }
    }

    let primary_gone = last_primary
        .is_some_and(|pw| externally_removed.contains(&pw))
        && !wins.0.is_empty();

    for (e, why) in to_close {
        let Some(idx) = wins.0.iter().position(|s| s.window == e) else {
            continue;
        };
        let st = wins.0.swap_remove(idx);
        bevy::log::info!(
            "[dock] closing floating window {e} ({why}); camera {}, root {}",
            st.camera,
            st.root
        );
        let mut panels = Vec::new();
        st.tree.collect_panels(&mut panels);
        for p in panels {
            if !dock.tree.contains_panel(&p) {
                dock.tree.adopt_panel(&p);
                dirty.0 = true;
            }
        }
        commands.entity(st.window).try_despawn();
        commands.entity(st.camera).try_despawn();
        commands.entity(st.root).try_despawn();
    }

    // A non-dock window was despawned — that's the primary closing. Take every
    // floating window down with it (their layouts are already persisted by the
    // shell, and lingering windows would keep the app alive).
    if primary_gone {
        for st in wins.0.drain(..) {
            bevy::log::info!("[dock] primary window closed; closing floating window {}", st.window);
            commands.entity(st.window).try_despawn();
            commands.entity(st.camera).try_despawn();
            commands.entity(st.root).try_despawn();
        }
    }
}

/// Last line of defense against the `camera_system` panic ("RenderTarget::
/// Window missing"): if ANY camera still targets a window entity that no
/// longer exists — whatever despawned it, through whatever path — despawn the
/// camera before bevy's camera update evaluates it. This never false-positives
/// (a camera and its window spawn in the same command batch) and the warning
/// makes an otherwise-invisible teardown bug diagnosable from the console log
/// instead of a crash report.
pub(crate) fn guard_dock_target_cameras(
    cameras: Query<(Entity, &bevy::camera::RenderTarget), With<Camera>>,
    windows: Query<(), With<Window>>,
    mut commands: Commands,
) {
    for (cam, rt) in &cameras {
        if let bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(w)) = rt {
            if !windows.contains(*w) {
                bevy::log::warn!(
                    "[dock] camera {cam} targets missing window {w} — despawning the camera \
                     to avoid a render panic (something despawned the window out from under it)"
                );
                commands.entity(cam).try_despawn();
            }
        }
    }
}
