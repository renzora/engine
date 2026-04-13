use crate::action::{InputAction, InputBinding};

/// Default input actions that ship with the engine.
pub fn default_actions() -> Vec<InputAction> {
    vec![
        // Movement — WASD + arrows + left stick
        InputAction::axis_2d(
            "move",
            vec![
                InputBinding::Composite2D {
                    up: "KeyW".into(),
                    down: "KeyS".into(),
                    left: "KeyA".into(),
                    right: "KeyD".into(),
                },
                InputBinding::Composite2D {
                    up: "ArrowUp".into(),
                    down: "ArrowDown".into(),
                    left: "ArrowLeft".into(),
                    right: "ArrowRight".into(),
                },
                InputBinding::GamepadAxis("LeftStickX".into()),
            ],
            0.15,
        ),
        // Look — right stick (mouse look handled separately via delta)
        InputAction::axis_2d(
            "look",
            vec![
                InputBinding::GamepadAxis("RightStickX".into()),
            ],
            0.15,
        ),
        // Jump
        InputAction::button(
            "jump",
            vec![
                InputBinding::Key("Space".into()),
                InputBinding::GamepadButton("South".into()),
            ],
        ),
        // Sprint
        InputAction::button(
            "sprint",
            vec![
                InputBinding::Key("ShiftLeft".into()),
                InputBinding::GamepadButton("West".into()),
            ],
        ),
        // Interact
        InputAction::button(
            "interact",
            vec![
                InputBinding::Key("KeyE".into()),
                InputBinding::GamepadButton("East".into()),
            ],
        ),
        // Primary action (attack / use)
        InputAction::button(
            "primary",
            vec![
                InputBinding::MouseButton("Left".into()),
                InputBinding::GamepadButton("RightTrigger2".into()),
            ],
        ),
        // Secondary action (aim / block)
        InputAction::button(
            "secondary",
            vec![
                InputBinding::MouseButton("Right".into()),
                InputBinding::GamepadButton("LeftTrigger2".into()),
            ],
        ),
    ]
}
