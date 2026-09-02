//! Borderless-window chrome: the minimize / maximize / close buttons, the
//! drag-to-move handle on the top bar's empty space, and the eight invisible
//! perimeter zones that start an OS edge/corner resize.
//!
//! On the web there is no OS window to move, minimize or close, so the three
//! buttons are replaced by a single fullscreen toggle — the one window state a
//! page is allowed to change.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use renzora_ui::window_chrome::{WindowAction, WindowActionQueue};

use crate::save_prompts::ExitRequest;

/// A window-control button (minimize / maximize / close).
#[derive(Component)]
pub(crate) struct WindowBtn(pub(crate) WindowAction);

/// The web's stand-in for the window controls: a fullscreen toggle.
///
/// Browser fullscreen is the only "window" state a page can actually change,
/// and it is the one worth having — it takes the tab strip and address bar away
/// and gives the editor the whole display, which is much closer to how the
/// desktop build is used.
#[cfg(target_arch = "wasm32")]
#[derive(Component)]
pub(crate) struct WebFullscreenBtn;

/// Toggle browser fullscreen, and report whether the page is now fullscreen.
///
/// Must run from a click: browsers only grant `requestFullscreen` in response
/// to a user gesture, and refuse it silently otherwise.
#[cfg(target_arch = "wasm32")]
fn toggle_web_fullscreen() {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    if doc.fullscreen_element().is_some() {
        doc.exit_fullscreen();
    } else if let Some(el) = doc.document_element() {
        // Fullscreen the whole page rather than the canvas: the canvas is sized
        // from its parent (`fit_canvas_to_parent`), so promoting the root keeps
        // that relationship and lets Bevy resize into the new viewport on its
        // own.
        let _ = el.request_fullscreen();
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn web_fullscreen_click(
    q: Query<&Interaction, (With<WebFullscreenBtn>, Changed<Interaction>)>,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        toggle_web_fullscreen();
    }
}

/// Swap the glyph between "expand" and "collapse" as fullscreen changes.
///
/// Polled rather than driven by the `fullscreenchange` event: the state can
/// also change by Esc or F11, which no click of ours would hear about, and a
/// cheap per-frame read of `document.fullscreenElement` covers every route.
#[cfg(target_arch = "wasm32")]
pub(crate) fn sync_web_fullscreen_icon(
    q: Query<&Children, With<WebFullscreenBtn>>,
    mut text: Query<&mut Text>,
) {
    let is_fs = web_sys::window()
        .and_then(|w| w.document())
        .is_some_and(|d| d.fullscreen_element().is_some());
    let Some(want) =
        renzora_ember::phosphor_map::icon_glyph(if is_fs { "corners-in" } else { "corners-out" })
    else {
        return;
    };
    let want = want.to_string();
    for children in &q {
        for child in children.iter() {
            if let Ok(mut t) = text.get_mut(child) {
                if t.0 != want {
                    t.0 = want.clone();
                }
            }
        }
    }
}

/// An empty top-bar region that initiates an OS window-move on press (and, when
/// maximized, restores first — Windows aero-snap then handles half/maximize).
#[derive(Component)]
pub(crate) struct WindowDragHandle;

/// A perimeter hit zone that initiates an OS edge/corner resize on press.
#[derive(Component)]
pub(crate) struct WindowResizeZone(bevy::math::CompassOctant);

/// The maximize button's icon — swapped between maximize/restore glyphs.
#[derive(Component)]
pub(crate) struct MaximizeIcon;

/// Keep the maximize button's glyph in sync with the window's maximized state.
pub(crate) fn update_maximize_icon(
    queue: Option<Res<WindowActionQueue>>,
    mut q: Query<&mut renzora_ember::icons::Icon, With<MaximizeIcon>>,
) {
    let maximized = queue.is_some_and(|q| q.maximized);
    let want = if maximized { "arrows-in-simple" } else { "square" };
    for mut icon in &mut q {
        if icon.name != want {
            icon.name = want.to_string();
            icon.resolved = false; // force `apply_icons` to re-render the glyph
        }
    }
}

pub(crate) fn window_btn_click(
    q: Query<(&Interaction, &WindowBtn), Changed<Interaction>>,
    queue: Option<ResMut<WindowActionQueue>>,
    mut commands: Commands,
) {
    let Some(mut queue) = queue else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        // Close is routed through the exit flow (which may prompt to save
        // unsaved changes first); everything else applies immediately.
        if matches!(btn.0, WindowAction::Close) {
            commands.insert_resource(ExitRequest);
        } else {
            queue.push(btn.0);
        }
    }
}

/// Click-timing for the drag handle: distinguishes a single press (window move)
/// from a double-click (toggle maximize).
#[derive(Default)]
pub(crate) struct DragClickState {
    last: f32,
    /// Whether the previous press restored a maximized window (so a double-click
    /// on a maximized window restores rather than re-maximizing).
    restored_on_press: bool,
}

/// Press an empty top-bar area → start an OS window-move; double-click → toggle
/// maximize/restore (the OS then handles aero-snap when you drag to an edge).
pub(crate) fn window_drag(
    bar: Query<&Interaction, (With<WindowDragHandle>, Changed<Interaction>)>,
    others: Query<&Interaction, Without<WindowDragHandle>>,
    queue: Option<ResMut<WindowActionQueue>>,
    time: Res<Time>,
    mut state: Local<DragClickState>,
) {
    let Some(mut queue) = queue else { return };
    if !bar.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    // If any other widget is hovered/pressed, the press landed on a menu/button —
    // not the empty bar — so don't drag (belt-and-braces over focus blocking).
    if others.iter().any(|i| *i != Interaction::None) {
        return;
    }
    let now = time.elapsed_secs();
    if now - state.last < 0.4 {
        // Double-click. If the first press already restored a maximized window
        // (via StartDrag), don't re-maximize — leave it restored.
        state.last = 0.0;
        if !state.restored_on_press {
            queue.push(WindowAction::ToggleMaximize);
        }
    } else {
        state.last = now;
        state.restored_on_press = queue.maximized;
        queue.push(WindowAction::StartDrag);
    }
}

pub(crate) fn window_resize_start(
    q: Query<(&Interaction, &WindowResizeZone), Changed<Interaction>>,
    queue: Option<ResMut<WindowActionQueue>>,
) {
    let Some(mut queue) = queue else { return };
    for (interaction, zone) in &q {
        if *interaction == Interaction::Pressed {
            queue.push(WindowAction::StartResize(zone.0));
        }
    }
}

/// Build the 8 invisible edge/corner resize zones overlaid on the window border.
/// Returns them so the caller parents them under the shell root.
pub(crate) fn build_resize_zones(commands: &mut Commands) -> Vec<Entity> {
    use bevy::math::CompassOctant as O;
    const T: f32 = 5.0; // edge thickness
    const C: f32 = 12.0; // corner size
    let px = Val::Px;
    // (octant, cursor, node)
    // The top edge is the title bar (drag area) — only the corners resize there,
    // so dragging the bar doesn't clash with a top-edge resize.
    let zones: [(O, SystemCursorIcon, Node); 7] = [
        (O::South, SystemCursorIcon::SResize, Node { position_type: PositionType::Absolute, bottom: px(0.0), left: px(C), right: px(C), height: px(T), ..default() }),
        (O::West, SystemCursorIcon::WResize, Node { position_type: PositionType::Absolute, left: px(0.0), top: px(C), bottom: px(C), width: px(T), ..default() }),
        (O::East, SystemCursorIcon::EResize, Node { position_type: PositionType::Absolute, right: px(0.0), top: px(C), bottom: px(C), width: px(T), ..default() }),
        (O::NorthWest, SystemCursorIcon::NwResize, Node { position_type: PositionType::Absolute, top: px(0.0), left: px(0.0), width: px(C), height: px(C), ..default() }),
        (O::NorthEast, SystemCursorIcon::NeResize, Node { position_type: PositionType::Absolute, top: px(0.0), right: px(0.0), width: px(C), height: px(C), ..default() }),
        (O::SouthWest, SystemCursorIcon::SwResize, Node { position_type: PositionType::Absolute, bottom: px(0.0), left: px(0.0), width: px(C), height: px(C), ..default() }),
        (O::SouthEast, SystemCursorIcon::SeResize, Node { position_type: PositionType::Absolute, bottom: px(0.0), right: px(0.0), width: px(C), height: px(C), ..default() }),
    ];
    zones
        .into_iter()
        .map(|(octant, cursor, node)| {
            let id = commands
                .spawn((
                    node,
                    BackgroundColor(Color::NONE),
                    GlobalZIndex(60),
                    Interaction::default(),
                    // Overlaid on the window perimeter, so it covers the edge of
                    // whatever panel is docked against it: the press is this
                    // zone's alone, and panels can see the gesture is in flight
                    // rather than reading it as a press on their content.
                    renzora_ember::resize::ResizeHandle,
                    WindowResizeZone(octant),
                    renzora_ember::cursor_icon::HoverCursor(cursor),
                    Name::new("resize-zone"),
                ))
                .id();
            // Resizing makes no sense while maximized — hide the grips then.
            renzora_ember::reactive::tracked::bind_display(commands, id, |w| {
                !w.get_resource::<WindowActionQueue>().map(|q| q.maximized).unwrap_or(false)
            });
            id
        })
        .collect()
}
