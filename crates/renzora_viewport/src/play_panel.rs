//! The play panel: the running game, inside the editor.
//!
//! The game is a separate process with its own window and its own swapchain.
//! This panel does not draw it. It reserves a rectangle, and the game's window
//! is made a *child* of the editor's window and moved to sit exactly over that
//! rectangle, so the window manager composites it there. The game renders at
//! full speed, nothing is copied, and input needs no forwarding because the
//! child is a real window the OS routes events to.
//!
//! See `renzora::core::window_embed` for the mechanism and what it costs.
//!
//! # The thing to understand before changing anything here
//!
//! **The editor cannot draw over the game.** A child window is painted by the
//! compositor after the parent has finished, so no `GlobalZIndex` reaches it. A
//! dropdown that overlaps this panel, a tooltip, a modal, the dock's drag
//! preview: every one of them would be hidden behind the game.
//!
//! So the game is hidden whenever something needs that rectangle, by
//! [`place_embedded_game`]. That is the entire mitigation, and the list of
//! things that need the rectangle is the part that has to be kept honest.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;

use crate::external_runtime::{self, ExternalRuntime};

pub const PANEL_ID: &str = "play";

/// The node whose rectangle the game's window is placed over.
#[derive(Component)]
struct PlaySurface;

pub(crate) fn register(app: &mut App) {
    app.register_panel_content(PANEL_ID, false, build_play_panel);
    app.add_systems(Update, place_embedded_game);
}

/// The panel's own contents: a ground colour and, when nothing is running, a
/// line saying so.
///
/// Deliberately almost empty. Anything drawn here is invisible the moment a
/// game is attached, because the game's window covers the whole rectangle, so
/// this is what the panel looks like when it is *not* playing and nothing more.
fn build_play_panel(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            // Near black: the game appears over this, and a light ground would
            // flash pale in the gap between the panel laying out and the game
            // being placed over it.
            BackgroundColor(Color::srgb(0.02, 0.02, 0.03)),
            PlaySurface,
            Name::new("play-panel-surface"),
        ))
        .id();

    let hint = commands
        .spawn((
            Text::new(renzora::lang::t_or(
                "play_panel.idle",
                "Press Play to run the game here.",
            )),
            ui_font(&fonts.ui, 12.5),
            TextColor(Color::srgb(0.45, 0.47, 0.55)),
        ))
        .id();
    commands.entity(root).add_child(hint);
    root
}

/// The editor's own window handle, to hand a game process as its parent.
///
/// `None` on a platform without child windows, which is the signal to launch the
/// game in its own window instead of pretending it can be embedded.
pub fn editor_window(world: &mut World) -> Option<renzora::core::window_embed::NativeWindow> {
    let mut q = world.query_filtered::<&bevy::window::RawHandleWrapper, With<bevy::window::PrimaryWindow>>();
    let handle = q.iter(world).next()?.get_window_handle();
    native_window_of(handle)
}

/// The match `renzora::core::window_embed` deliberately does not carry, because
/// naming these variants needs `raw_window_handle` and the contract crate is
/// Bevy and serde only.
fn native_window_of(
    handle: raw_window_handle::RawWindowHandle,
) -> Option<renzora::core::window_embed::NativeWindow> {
    match handle {
        #[cfg(target_os = "windows")]
        raw_window_handle::RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
        #[cfg(all(unix, not(target_os = "macos")))]
        raw_window_handle::RawWindowHandle::Xlib(h) => Some(h.window as isize),
        _ => None,
    }
}

/// Keep the game's window over the panel's rectangle, and out of the way when
/// the editor needs to draw there.
///
/// Runs every frame rather than on change. The rectangle moves for a dock drag,
/// a window resize, a panel collapse, a tab switch and a splitter drag, and
/// tracking which of those happened is the same work as just asking, with a
/// chance of being wrong. The call is a `SetWindowPos`.
fn place_embedded_game(
    runtime: Option<Res<ExternalRuntime>>,
    surfaces: Query<(&ComputedNode, &UiGlobalTransform, &InheritedVisibility), With<PlaySurface>>,
    // Everything that would otherwise be painted over. Modals and floating
    // overlays announce themselves, which is what makes this one query rather
    // than a list of special cases.
    modals: Query<(), With<renzora_ember::widgets::ModalSurface>>,
    overlays: Query<
        (&ComputedNode, &UiGlobalTransform),
        (
            With<renzora_ember::widgets::OverlaySurface>,
            Without<PlaySurface>,
        ),
    >,
    // The dock's drag preview is a plain `GlobalZIndex(1000)` node rather than
    // an `OverlaySurface`, so it does not announce itself like the rest and has
    // to be asked about directly. Getting this wrong means the drag ghost and
    // the drop-zone highlights vanish exactly where you are aiming.
    dragging: Option<Res<renzora_ember::dock::DockDragWatch>>,
    mut placed: Local<bool>,
) {
    use renzora::core::window_embed;

    let Some(child) = external_runtime::embedded_game_window() else {
        *placed = false;
        return;
    };
    // A runtime that is alive but not yet `Running` is still coming up; placing
    // it is harmless, so the phase is not consulted here. What matters is that
    // the handle exists, which it only does after the game reported it.
    let _ = &runtime;

    let Ok((node, transform, visible)) = surfaces.single() else {
        // The panel is closed or in a background tab, so there is nowhere to
        // put the game. Hidden rather than left where it was, which would leave
        // it floating over whatever panel took its place.
        window_embed::set_visible(child, false);
        *placed = false;
        return;
    };

    let size = node.size();
    // A panel mid-collapse, or one laid out while hidden, measures zero. Placing
    // a zero-sized window and then showing it flickers; leaving it hidden until
    // there is somewhere to put it does not.
    let on_screen = visible.get() && size.x >= 1.0 && size.y >= 1.0;

    // `UiGlobalTransform` is the node's centre in physical pixels, and
    // `ComputedNode::size` is physical too, which is the space a child window's
    // position is measured in. No scale-factor conversion belongs here: doing
    // one would make the panel correct at 100% and wrong everywhere else.
    let half = size * 0.5;
    let x = (transform.translation.x - half.x).round() as i32;
    let y = (transform.translation.y - half.y).round() as i32;

    let covered = !modals.is_empty()
        || dragging.is_some_and(|d| d.dragging.is_some())
        || overlays.iter().any(|(overlay_node, overlay_xf)| {
            overlaps(
                (x, y, size.x, size.y),
                (overlay_node.size(), overlay_xf.translation),
            )
        });

    if on_screen {
        window_embed::place(child, x, y, size.x as u32, size.y as u32);
        *placed = true;
    }
    // Shown only once it has been placed, so the first frame does not flash the
    // game at the wrong size in the wrong corner.
    window_embed::set_visible(child, on_screen && *placed && !covered);
}

/// Do a placed rectangle and a centred one overlap?
///
/// Both in physical pixels. The panel's is given as its top-left plus size
/// because that is what was just computed for the window; the overlay's is a
/// centre plus size because that is what `UiGlobalTransform` holds.
fn overlaps(panel: (i32, i32, f32, f32), overlay: (Vec2, Vec2)) -> bool {
    let (px, py, pw, ph) = panel;
    let (size, centre) = overlay;
    if size.x < 1.0 || size.y < 1.0 {
        return false;
    }
    let (ox, oy) = (centre.x - size.x * 0.5, centre.y - size.y * 0.5);
    let (px, py) = (px as f32, py as f32);
    px < ox + size.x && ox < px + pw && py < oy + size.y && oy < py + ph
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The overlap test decides whether the game is hidden, so both mistakes
    /// are visible: a false positive blinks the game out for no reason, and a
    /// false negative leaves a dropdown painted over by it.
    #[test]
    fn an_overlay_over_the_panel_counts_as_covering_it() {
        let panel = (100, 100, 400.0, 300.0);
        // Centred inside the panel.
        assert!(overlaps(panel, (Vec2::new(50.0, 50.0), Vec2::new(300.0, 250.0))));
        // Overlapping one corner.
        assert!(overlaps(panel, (Vec2::new(50.0, 50.0), Vec2::new(110.0, 110.0))));
    }

    #[test]
    fn an_overlay_beside_the_panel_does_not() {
        let panel = (100, 100, 400.0, 300.0);
        // Entirely to the left.
        assert!(!overlaps(panel, (Vec2::new(50.0, 50.0), Vec2::new(60.0, 250.0))));
        // Entirely below.
        assert!(!overlaps(panel, (Vec2::new(50.0, 50.0), Vec2::new(300.0, 440.0))));
        // Touching edge-on is not overlapping: a menu that ends exactly where
        // the panel starts obscures none of it.
        assert!(!overlaps(panel, (Vec2::new(50.0, 50.0), Vec2::new(75.0, 250.0))));
    }

    /// A surface laid out to nothing is not covering anything. Hidden panels
    /// measure zero and there are always several of them.
    #[test]
    fn a_zero_sized_overlay_covers_nothing() {
        let panel = (100, 100, 400.0, 300.0);
        assert!(!overlaps(panel, (Vec2::ZERO, Vec2::new(300.0, 250.0))));
    }
}
