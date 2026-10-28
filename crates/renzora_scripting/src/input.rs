use bevy::input::gamepad::{Gamepad, GamepadAxis, GamepadButton};
use bevy::prelude::*;
use std::collections::HashMap;

// Re-export ScriptInput from renzora
pub use renzora::ScriptInput;

use renzora::context::{TouchSnapshot, TouchState, TAP_MAX_SECONDS, TAP_MAX_TRAVEL};

/// All buttons mirrored into [`ScriptInput`], in the order scripts index them.
pub const SCRIPT_GAMEPAD_BUTTONS: [GamepadButton; 16] = [
    GamepadButton::South,
    GamepadButton::East,
    GamepadButton::West,
    GamepadButton::North,
    GamepadButton::LeftTrigger,
    GamepadButton::RightTrigger,
    GamepadButton::LeftTrigger2,
    GamepadButton::RightTrigger2,
    GamepadButton::Select,
    GamepadButton::Start,
    GamepadButton::LeftThumb,
    GamepadButton::RightThumb,
    GamepadButton::DPadUp,
    GamepadButton::DPadDown,
    GamepadButton::DPadLeft,
    GamepadButton::DPadRight,
];

/// System to update ScriptInput from Bevy input resources
pub fn update_script_input(
    mut script_input: ResMut<ScriptInput>,
    keyboard_events: Option<MessageReader<bevy::input::keyboard::KeyboardInput>>,
    mouse_buttons: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mouse_motion: Option<MessageReader<bevy::input::mouse::MouseMotion>>,
    scroll: Option<MessageReader<bevy::input::mouse::MouseWheel>>,
    gamepads: Query<(Entity, &Gamepad)>,
    mut gamepad_slots: Local<HashMap<Entity, u32>>,
    touches: Option<Res<bevy::input::touch::Touches>>,
    real_time: Option<Res<Time<Real>>>,
    mut touch_memory: Local<TouchMemory>,
) {
    // Real time, not virtual: a tap is a human-scale duration, and pausing or
    // slowing the game clock must not turn a quick tap into a long press.
    let now = real_time.map(|t| t.elapsed_secs_f64()).unwrap_or(0.0);
    script_input.touch = touches
        .map(|t| collect_touches(&t, now, &mut touch_memory))
        .unwrap_or_default();

    script_input.keys_just_pressed.clear();
    script_input.keys_just_released.clear();
    script_input.mouse_just_pressed.clear();
    script_input.mouse_delta = Vec2::ZERO;
    script_input.scroll_delta = Vec2::ZERO;

    if let Some(mut keyboard_events) = keyboard_events {
        for event in keyboard_events.read() {
            if event.state.is_pressed() {
                if !script_input.keys_pressed.contains_key(&event.key_code) {
                    script_input.keys_just_pressed.insert(event.key_code, true);
                }
                script_input.keys_pressed.insert(event.key_code, true);
            } else {
                script_input.keys_pressed.remove(&event.key_code);
                script_input.keys_just_released.insert(event.key_code, true);
            }
        }
    }

    if let Some(mouse_buttons) = mouse_buttons {
        for button in mouse_buttons.get_pressed() {
            script_input.mouse_pressed.insert(*button, true);
        }
        for button in mouse_buttons.get_just_pressed() {
            script_input.mouse_just_pressed.insert(*button, true);
        }
        for button in mouse_buttons.get_just_released() {
            script_input.mouse_pressed.remove(button);
        }
    }

    if let Ok(window) = windows.single() {
        if let Some(pos) = window.cursor_position() {
            script_input.mouse_position = pos;
        }
    }

    if let Some(mut mouse_motion) = mouse_motion {
        for event in mouse_motion.read() {
            script_input.mouse_delta += event.delta;
        }
    }
    if let Some(mut scroll) = scroll {
        for event in scroll.read() {
            script_input.scroll_delta += Vec2::new(event.x, event.y);
        }
    }

    script_input.gamepad_axes.clear();
    script_input.gamepad_buttons.clear();
    script_input.gamepad_buttons_just_pressed.clear();
    script_input.connected_gamepads.clear();

    // Stable slot assignment: a pad keeps its slot for as long as it stays
    // connected; a new pad takes the lowest free slot. Query iteration order
    // is not stable, so without this pads could swap ids between frames.
    gamepad_slots.retain(|entity, _| gamepads.contains(*entity));
    let mut new_pads: Vec<Entity> = gamepads
        .iter()
        .map(|(entity, _)| entity)
        .filter(|e| !gamepad_slots.contains_key(e))
        .collect();
    new_pads.sort();
    for entity in new_pads {
        let mut slot = 0u32;
        while gamepad_slots.values().any(|&s| s == slot) {
            slot += 1;
        }
        gamepad_slots.insert(entity, slot);
    }

    for (entity, gamepad) in gamepads.iter() {
        let id = gamepad_slots[&entity];
        script_input.connected_gamepads.push(id);

        let mut axes = HashMap::new();
        let ls = gamepad.left_stick();
        let rs = gamepad.right_stick();
        axes.insert(GamepadAxis::LeftStickX, ls.x);
        axes.insert(GamepadAxis::LeftStickY, ls.y);
        axes.insert(GamepadAxis::RightStickX, rs.x);
        axes.insert(GamepadAxis::RightStickY, rs.y);
        // Analog triggers: the Z axes on some controllers, the
        // LeftTrigger2/RightTrigger2 analog buttons on others (e.g. Windows
        // XInput). Take whichever reports a value.
        axes.insert(
            GamepadAxis::LeftZ,
            gamepad
                .get(GamepadAxis::LeftZ)
                .unwrap_or(0.0)
                .max(gamepad.get(GamepadButton::LeftTrigger2).unwrap_or(0.0)),
        );
        axes.insert(
            GamepadAxis::RightZ,
            gamepad
                .get(GamepadAxis::RightZ)
                .unwrap_or(0.0)
                .max(gamepad.get(GamepadButton::RightTrigger2).unwrap_or(0.0)),
        );
        script_input.gamepad_axes.insert(id, axes);

        let mut buttons = HashMap::new();
        let mut just_pressed = HashMap::new();
        for btn in SCRIPT_GAMEPAD_BUTTONS {
            buttons.insert(btn, gamepad.pressed(btn));
            just_pressed.insert(btn, gamepad.just_pressed(btn));
        }
        script_input.gamepad_buttons.insert(id, buttons);
        script_input
            .gamepad_buttons_just_pressed
            .insert(id, just_pressed);
    }
    script_input.connected_gamepads.sort_unstable();
}

/// What [`collect_touches`] carries from one frame to the next, per finger.
#[derive(Default)]
pub struct TouchMemory {
    /// When each finger went down, because a `Touch` knows where it started
    /// but not when, and a tap is defined by both.
    started: HashMap<u64, f64>,
    /// Where each finger was last frame. `Touch::delta` cannot stand in for
    /// this: it is measured between the last two move events, so a finger
    /// that stops keeps reporting its last movement every frame until it
    /// moves again, and a pinch-zoom would keep zooming under a still hand.
    last: HashMap<u64, Vec2>,
}

/// This frame's fingers and gestures, from Bevy's [`Touches`].
///
/// [`Touches`]: bevy::input::touch::Touches
fn collect_touches(
    touches: &bevy::input::touch::Touches,
    now: f64,
    memory: &mut TouchMemory,
) -> TouchState {
    fn snapshot(
        touches: &bevy::input::touch::Touches,
        last: &mut HashMap<u64, Vec2>,
        t: &bevy::input::touch::Touch,
        just_released: bool,
    ) -> TouchSnapshot {
        let id = t.id();
        // A finger seen for the first time has not moved yet as far as a
        // script is concerned, including one that came and went in one frame.
        let previous = last.get(&id).copied().unwrap_or(t.position());
        if just_released {
            last.remove(&id);
        } else {
            last.insert(id, t.position());
        }
        TouchSnapshot {
            id,
            position: t.position(),
            start_position: t.start_position(),
            delta: t.position() - previous,
            just_pressed: touches.just_pressed(id),
            just_released,
        }
    }

    let mut state = TouchState::default();
    for touch in touches.iter() {
        if touches.just_pressed(touch.id()) {
            memory.started.insert(touch.id(), now);
        }
        state.touches.push(snapshot(touches, &mut memory.last, touch, false));
    }
    for touch in touches.iter_just_released() {
        // A finger that went down and up inside one frame was never seen
        // pressed, so it has no recorded start: it was as quick as a tap gets.
        let held_for = now - memory.started.remove(&touch.id()).unwrap_or(now);
        if held_for <= TAP_MAX_SECONDS && touch.distance().length() <= TAP_MAX_TRAVEL {
            state.taps.push(touch.position());
        }
        state.touches.push(snapshot(touches, &mut memory.last, touch, true));
    }
    // A cancelled touch (an incoming call, the OS taking the gesture) is not a
    // tap and must not leave anything behind for a reused id.
    for touch in touches.iter_just_canceled() {
        memory.started.remove(&touch.id());
        memory.last.remove(&touch.id());
    }
    state.touches.sort_by_key(|t| t.id);

    // Only a steady pair: a third finger makes "the" pinch ambiguous, and a
    // finger that just landed has no previous position to measure from.
    let held: Vec<&TouchSnapshot> = state.touches.iter().filter(|t| !t.just_released).collect();
    if let [a, b] = held[..] {
        if !a.just_pressed && !b.just_pressed {
            (state.pinch, state.rotation) =
                two_finger_gesture(a.position - a.delta, a.position, b.position - b.delta, b.position);
        }
    }
    state
}

/// Pinch and twist between two fingers moving from `a0`/`b0` to `a1`/`b1`.
///
/// Returns `(pinch, rotation)`: the change in their distance as a fraction of
/// the old one, and the change in the angle of the line between them, in
/// radians, positive counterclockwise as seen on screen.
pub fn two_finger_gesture(a0: Vec2, a1: Vec2, b0: Vec2, b1: Vec2) -> (f32, f32) {
    let before = b0 - a0;
    let after = b1 - a1;
    // Fingers this close are effectively one point, and dividing by the
    // distance between them would turn sensor noise into a huge pinch.
    if before.length() < 1.0 || after.length() < 1.0 {
        return (0.0, 0.0);
    }
    let pinch = after.length() / before.length() - 1.0;
    // Window coordinates point y down, which mirrors every angle: a twist that
    // looks counterclockwise measures as clockwise here, hence the negation.
    let rotation = -before.angle_to(after);
    (pinch, rotation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spreading_fingers_is_a_positive_pinch() {
        let (pinch, rotation) = two_finger_gesture(
            Vec2::new(100.0, 100.0),
            Vec2::new(90.0, 100.0),
            Vec2::new(200.0, 100.0),
            Vec2::new(210.0, 100.0),
        );
        assert!((pinch - 0.2).abs() < 1e-5, "100px apart to 120px is +20%, got {pinch}");
        assert_eq!(rotation, 0.0);
    }

    #[test]
    fn closing_fingers_is_a_negative_pinch() {
        let (pinch, _) = two_finger_gesture(
            Vec2::new(0.0, 0.0),
            Vec2::new(25.0, 0.0),
            Vec2::new(100.0, 0.0),
            Vec2::new(75.0, 0.0),
        );
        assert!((pinch + 0.5).abs() < 1e-5, "100px apart to 50px is -50%, got {pinch}");
    }

    /// On screen, y grows downward. Moving the right-hand finger up (smaller y)
    /// turns the pair counterclockwise as the player sees it.
    #[test]
    fn twisting_counterclockwise_on_screen_is_positive() {
        let (_, rotation) = two_finger_gesture(
            Vec2::new(0.0, 0.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 0.0),
            Vec2::new(0.0, -100.0),
        );
        assert!((rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-5, "got {rotation}");
    }

    /// Drive Bevy's own touch tracking, so the test sees the same `Touches` a
    /// real device produces rather than a hand-built copy of it.
    fn touches_after(app: &mut App, events: &[(u64, bevy::input::touch::TouchPhase, Vec2)]) {
        for &(id, phase, position) in events {
            app.world_mut().write_message(bevy::input::touch::TouchInput {
                phase,
                position,
                window: Entity::PLACEHOLDER,
                force: None,
                id,
            });
        }
        app.update();
    }

    fn input_app() -> App {
        let mut app = App::new();
        app.add_plugins(bevy::input::InputPlugin);
        app
    }

    #[test]
    fn a_quick_still_press_is_a_tap_and_is_listed_as_released() {
        use bevy::input::touch::TouchPhase::*;
        let mut app = input_app();
        let mut memory = TouchMemory::default();
        let at = Vec2::new(40.0, 60.0);

        touches_after(&mut app, &[(7, Started, at)]);
        let down = collect_touches(app.world().resource(), 1.0, &mut memory);
        assert_eq!(down.touches.len(), 1);
        assert!(down.touches[0].just_pressed);
        assert!(down.taps.is_empty());

        // A device reports any last wobble as `Moved` before `Ended`, and Bevy
        // keeps the position from that, not from the `Ended` event itself.
        let lifted = at + Vec2::new(3.0, 0.0);
        touches_after(&mut app, &[(7, Moved, lifted), (7, Ended, lifted)]);
        let up = collect_touches(app.world().resource(), 1.1, &mut memory);
        assert_eq!(up.taps, vec![lifted]);
        assert!(up.touches[0].just_released, "the lifted finger is listed for its last frame");
        assert!(memory.started.is_empty() && memory.last.is_empty(), "nothing is kept once the finger lifts");
    }

    #[test]
    fn a_long_press_or_a_drag_is_not_a_tap() {
        use bevy::input::touch::TouchPhase::*;
        let mut app = input_app();
        let mut memory = TouchMemory::default();

        touches_after(&mut app, &[(1, Started, Vec2::ZERO), (2, Started, Vec2::ZERO)]);
        collect_touches(app.world().resource(), 0.0, &mut memory);
        touches_after(
            &mut app,
            &[(1, Ended, Vec2::ZERO), (2, Ended, Vec2::new(TAP_MAX_TRAVEL + 1.0, 0.0))],
        );
        // Finger 1 stayed still but was held too long; finger 2 was quick but moved.
        let state = collect_touches(app.world().resource(), TAP_MAX_SECONDS + 0.1, &mut memory);
        assert!(state.taps.is_empty(), "got taps {:?}", state.taps);
    }

    #[test]
    fn two_held_fingers_report_their_pinch() {
        use bevy::input::touch::TouchPhase::*;
        let mut app = input_app();
        let mut memory = TouchMemory::default();

        touches_after(
            &mut app,
            &[(1, Started, Vec2::new(100.0, 0.0)), (2, Started, Vec2::new(200.0, 0.0))],
        );
        let landing = collect_touches(app.world().resource(), 0.0, &mut memory);
        assert_eq!(landing.pinch, 0.0, "fingers that just landed have nothing to measure from");

        touches_after(
            &mut app,
            &[(1, Moved, Vec2::new(90.0, 0.0)), (2, Moved, Vec2::new(210.0, 0.0))],
        );
        let spread = collect_touches(app.world().resource(), 0.1, &mut memory);
        assert!((spread.pinch - 0.2).abs() < 1e-5, "got {}", spread.pinch);
    }

    /// Bevy's own `Touch::delta` keeps the last move until the next one, which
    /// in the editor showed up as a pinch of 1.0 every frame under a still hand.
    #[test]
    fn fingers_that_stop_report_no_movement() {
        use bevy::input::touch::TouchPhase::*;
        let mut app = input_app();
        let mut memory = TouchMemory::default();

        touches_after(
            &mut app,
            &[(1, Started, Vec2::new(100.0, 0.0)), (2, Started, Vec2::new(200.0, 0.0))],
        );
        collect_touches(app.world().resource(), 0.0, &mut memory);
        touches_after(&mut app, &[(2, Moved, Vec2::new(300.0, 0.0))]);
        let moved = collect_touches(app.world().resource(), 0.1, &mut memory);
        assert_eq!(moved.touches[1].delta, Vec2::new(100.0, 0.0));
        assert!((moved.pinch - 1.0).abs() < 1e-5, "got {}", moved.pinch);

        touches_after(&mut app, &[]);
        let still = collect_touches(app.world().resource(), 0.2, &mut memory);
        assert_eq!(still.touches[1].delta, Vec2::ZERO);
        assert_eq!(still.pinch, 0.0);
    }

    #[test]
    fn coincident_fingers_report_no_gesture() {
        let p = Vec2::new(50.0, 50.0);
        assert_eq!(two_finger_gesture(p, p, p, p + Vec2::new(40.0, 0.0)), (0.0, 0.0));
    }
}
