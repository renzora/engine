//! Gamepad debug state resource.

use bevy::prelude::*;

/// Cached gamepad state for the debug panel.
#[derive(Resource, Default)]
pub struct GamepadDebugState {
    pub gamepads: Vec<GamepadInfo>,
}

/// Information about a single gamepad.
#[derive(Clone, Default)]
pub struct GamepadInfo {
    pub left_stick: Vec2,
    pub right_stick: Vec2,
    pub left_trigger: f32,
    pub right_trigger: f32,
    pub buttons: GamepadButtonState,
    pub raw_axes: Vec<(String, f32)>,
}

/// State of all gamepad buttons.
#[derive(Clone, Default)]
pub struct GamepadButtonState {
    pub south: bool,
    pub east: bool,
    pub west: bool,
    pub north: bool,
    pub left_trigger: bool,
    pub right_trigger: bool,
    pub left_trigger2: bool,
    pub right_trigger2: bool,
    pub dpad_up: bool,
    pub dpad_down: bool,
    pub dpad_left: bool,
    pub dpad_right: bool,
    pub start: bool,
    pub select: bool,
    pub left_thumb: bool,
    pub right_thumb: bool,
}

/// System that reads Bevy gamepad input and updates [`GamepadDebugState`].
pub fn update_gamepad_debug_state(
    mut debug_state: ResMut<GamepadDebugState>,
    gamepads: Query<&Gamepad>,
) {
    debug_state.gamepads.clear();

    for gamepad in gamepads.iter() {
        let mut raw_axes = Vec::new();

        let axes_to_check = [
            (GamepadAxis::LeftStickX, "LeftStickX"),
            (GamepadAxis::LeftStickY, "LeftStickY"),
            (GamepadAxis::RightStickX, "RightStickX"),
            (GamepadAxis::RightStickY, "RightStickY"),
            (GamepadAxis::LeftZ, "LeftZ"),
            (GamepadAxis::RightZ, "RightZ"),
        ];

        for (axis, name) in axes_to_check {
            if let Some(value) = gamepad.get(axis) {
                if value.abs() > 0.001 {
                    raw_axes.push((name.to_string(), value));
                }
            }
        }

        for i in 0..10 {
            if let Some(value) = gamepad.get(GamepadAxis::Other(i)) {
                if value.abs() > 0.001 {
                    raw_axes.push((format!("Other({})", i), value));
                }
            }
        }

        let info = GamepadInfo {
            left_stick: Vec2::new(
                gamepad.get(GamepadAxis::LeftStickX).unwrap_or(0.0),
                gamepad.get(GamepadAxis::LeftStickY).unwrap_or(0.0),
            ),
            right_stick: Vec2::new(
                gamepad.get(GamepadAxis::RightStickX).unwrap_or(0.0),
                gamepad.get(GamepadAxis::RightStickY).unwrap_or(0.0),
            ),
            // Analog triggers: the Z axes on some controllers, the
            // LeftTrigger2/RightTrigger2 analog buttons on others (e.g. Windows
            // XInput). Take whichever reports a value.
            left_trigger: gamepad
                .get(GamepadAxis::LeftZ)
                .unwrap_or(0.0)
                .max(gamepad.get(GamepadButton::LeftTrigger2).unwrap_or(0.0)),
            right_trigger: gamepad
                .get(GamepadAxis::RightZ)
                .unwrap_or(0.0)
                .max(gamepad.get(GamepadButton::RightTrigger2).unwrap_or(0.0)),
            buttons: GamepadButtonState {
                south: gamepad.pressed(GamepadButton::South),
                east: gamepad.pressed(GamepadButton::East),
                west: gamepad.pressed(GamepadButton::West),
                north: gamepad.pressed(GamepadButton::North),
                left_trigger: gamepad.pressed(GamepadButton::LeftTrigger),
                right_trigger: gamepad.pressed(GamepadButton::RightTrigger),
                left_trigger2: gamepad.pressed(GamepadButton::LeftTrigger2),
                right_trigger2: gamepad.pressed(GamepadButton::RightTrigger2),
                dpad_up: gamepad.pressed(GamepadButton::DPadUp),
                dpad_down: gamepad.pressed(GamepadButton::DPadDown),
                dpad_left: gamepad.pressed(GamepadButton::DPadLeft),
                dpad_right: gamepad.pressed(GamepadButton::DPadRight),
                start: gamepad.pressed(GamepadButton::Start),
                select: gamepad.pressed(GamepadButton::Select),
                left_thumb: gamepad.pressed(GamepadButton::LeftThumb),
                right_thumb: gamepad.pressed(GamepadButton::RightThumb),
            },
            raw_axes,
        };
        debug_state.gamepads.push(info);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// Build a world holding one synthetic gamepad, run the reader, and hand
    /// back what the debug panel would see.
    fn read(setup: impl FnOnce(&mut Gamepad)) -> GamepadInfo {
        let mut world = World::new();
        world.init_resource::<GamepadDebugState>();
        let mut pad = Gamepad::default();
        setup(&mut pad);
        world.spawn(pad);
        world.run_system_once(update_gamepad_debug_state).unwrap();
        world.resource::<GamepadDebugState>().gamepads[0].clone()
    }

    fn set_axis(pad: &mut Gamepad, axis: GamepadAxis, value: f32) {
        pad.analog_mut().set(axis, value);
    }

    #[test]
    fn with_no_gamepad_connected_the_panel_shows_nothing() {
        let mut world = World::new();
        world.init_resource::<GamepadDebugState>();
        world.run_system_once(update_gamepad_debug_state).unwrap();
        assert!(world.resource::<GamepadDebugState>().gamepads.is_empty());
    }

    #[test]
    fn stick_axes_are_mirrored_into_the_panel() {
        let info = read(|pad| {
            set_axis(pad, GamepadAxis::LeftStickX, 0.5);
            set_axis(pad, GamepadAxis::LeftStickY, -0.25);
            set_axis(pad, GamepadAxis::RightStickX, -1.0);
            set_axis(pad, GamepadAxis::RightStickY, 1.0);
        });
        assert_eq!(info.left_stick, Vec2::new(0.5, -0.25));
        assert_eq!(info.right_stick, Vec2::new(-1.0, 1.0));
    }

    /// A controller that does not report an axis at all must read as centred,
    /// not drop the pad or leave the previous frame's value behind.
    #[test]
    fn an_unreported_axis_reads_as_centred() {
        let info = read(|_| {});
        assert_eq!(info.left_stick, Vec2::ZERO);
        assert_eq!(info.right_stick, Vec2::ZERO);
        assert_eq!(info.left_trigger, 0.0);
        assert_eq!(info.right_trigger, 0.0);
    }

    // ── the analog-trigger fallback ──────────────────────────────────────────

    /// Analog triggers arrive on the Z axes for some controllers and as the
    /// `*Trigger2` analog buttons for others (Windows XInput). Taking whichever
    /// reports a value is the whole reason for the `.max()`, and it is the kind
    /// of platform quirk that works on the developer's pad and silently reads
    /// zero on half the users'.
    #[test]
    fn a_trigger_reported_on_the_z_axis_is_picked_up() {
        let info = read(|pad| {
            set_axis(pad, GamepadAxis::LeftZ, 0.7);
            set_axis(pad, GamepadAxis::RightZ, 0.3);
        });
        assert!((info.left_trigger - 0.7).abs() < 1e-5);
        assert!((info.right_trigger - 0.3).abs() < 1e-5);
    }

    #[test]
    fn a_trigger_reported_as_an_analog_button_is_picked_up() {
        let info = read(|pad| {
            pad.analog_mut().set(GamepadButton::LeftTrigger2, 0.6);
            pad.analog_mut().set(GamepadButton::RightTrigger2, 0.9);
        });
        assert!((info.left_trigger - 0.6).abs() < 1e-5);
        assert!((info.right_trigger - 0.9).abs() < 1e-5);
    }

    /// A controller reporting both must not halve or sum them — the larger is
    /// the real pull.
    #[test]
    fn a_trigger_reported_twice_takes_the_larger_reading() {
        let info = read(|pad| {
            set_axis(pad, GamepadAxis::LeftZ, 0.2);
            pad.analog_mut().set(GamepadButton::LeftTrigger2, 0.8);
        });
        assert!((info.left_trigger - 0.8).abs() < 1e-5);
    }

    // ── the raw-axis list ────────────────────────────────────────────────────

    /// The deadzone is what keeps the raw list readable. A resting stick reports
    /// a trickle of noise, and without the filter the panel would list every
    /// axis every frame and churn constantly.
    #[test]
    fn resting_noise_is_kept_out_of_the_raw_axis_list() {
        let info = read(|pad| {
            set_axis(pad, GamepadAxis::LeftStickX, 0.0005);
            set_axis(pad, GamepadAxis::LeftStickY, -0.0009);
        });
        assert!(info.raw_axes.is_empty(), "noise leaked through: {:?}", info.raw_axes);
    }

    #[test]
    fn a_deflected_axis_is_listed_by_name() {
        let info = read(|pad| set_axis(pad, GamepadAxis::RightStickX, -0.42));
        assert_eq!(info.raw_axes.len(), 1);
        assert_eq!(info.raw_axes[0].0, "RightStickX");
        assert!((info.raw_axes[0].1 + 0.42).abs() < 1e-5);
    }

    /// Unrecognised axes are what a non-standard controller reports, and seeing
    /// them is the entire point of the raw list — it is the tool for working out
    /// what an unmapped pad is sending.
    #[test]
    fn unrecognised_axes_are_listed_by_index() {
        let info = read(|pad| set_axis(pad, GamepadAxis::Other(3), 0.5));
        assert!(
            info.raw_axes.iter().any(|(n, _)| n == "Other(3)"),
            "got {:?}",
            info.raw_axes
        );
    }

    // ── buttons ──────────────────────────────────────────────────────────────

    #[test]
    fn pressed_buttons_are_reported_and_others_are_not() {
        let info = read(|pad| {
            pad.digital_mut().press(GamepadButton::South);
            pad.digital_mut().press(GamepadButton::DPadLeft);
            pad.digital_mut().press(GamepadButton::Start);
        });
        assert!(info.buttons.south);
        assert!(info.buttons.dpad_left);
        assert!(info.buttons.start);

        assert!(!info.buttons.north);
        assert!(!info.buttons.east);
        assert!(!info.buttons.west);
        assert!(!info.buttons.dpad_right);
        assert!(!info.buttons.select);
        assert!(!info.buttons.left_thumb);
    }

    // ── multiple pads and disconnection ──────────────────────────────────────

    #[test]
    fn every_connected_pad_gets_its_own_row() {
        let mut world = World::new();
        world.init_resource::<GamepadDebugState>();
        for x in [0.25f32, 0.75] {
            let mut pad = Gamepad::default();
            pad.analog_mut().set(GamepadAxis::LeftStickX, x);
            world.spawn(pad);
        }
        world.run_system_once(update_gamepad_debug_state).unwrap();

        let pads = &world.resource::<GamepadDebugState>().gamepads;
        assert_eq!(pads.len(), 2);
        let mut xs: Vec<f32> = pads.iter().map(|p| p.left_stick.x).collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(xs, vec![0.25, 0.75]);
    }

    /// The list is rebuilt from scratch each run. Without the clear, a pad that
    /// was unplugged would stay on the panel forever — and, worse, would keep
    /// showing its last stick position as though it were live.
    #[test]
    fn a_disconnected_pad_disappears_from_the_panel() {
        let mut world = World::new();
        world.init_resource::<GamepadDebugState>();
        let pad = world.spawn(Gamepad::default()).id();
        world.run_system_once(update_gamepad_debug_state).unwrap();
        assert_eq!(world.resource::<GamepadDebugState>().gamepads.len(), 1);

        world.despawn(pad);
        world.run_system_once(update_gamepad_debug_state).unwrap();
        assert!(world.resource::<GamepadDebugState>().gamepads.is_empty());
    }
}
