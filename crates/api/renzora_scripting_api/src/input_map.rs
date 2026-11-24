//! The rebindable action map: named actions and the physical inputs bound to
//! them.
//!
//! This is the half of input a game changes while it runs. An in-game controls
//! menu has to read the current bindings and write new ones, and that menu is a
//! script: a Rust script compiled against the SDK, or a Lua one going through
//! the declared `set_setting("input.jump.0", "Key:KeyF")`. While [`InputMap`]
//! lived in `renzora_input` neither could name it, because a script reaches the
//! contract layers and nothing else, so bindings were whatever `input_map.ron`
//! said at startup for the whole session.
//!
//! `renzora_input` still owns the behaviour: it loads the map, and recomputes
//! [`ActionState`](crate::ActionState) from it every frame in `PreUpdate`. That
//! per-frame read is what makes a rebind live: changing the resource is the
//! whole job, and the next frame's actions follow it. `renzora_input`
//! re-exports everything here at the paths it always had.
//!
//! Bindings are stored as strings (`"KeyW"`, `"South"`), not as Bevy's enums,
//! so the map serializes to a readable `input_map.ron` and a script can name an
//! input without a Bevy type in its hand.

use bevy::input::gamepad::{GamepadAxis, GamepadButton};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// What kind of value an action produces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Reflect)]
pub enum ActionKind {
    /// Pressed / not pressed.
    Button,
    /// Single float axis (-1.0 to 1.0).
    Axis1D,
    /// Two-axis vector (e.g. movement stick).
    Axis2D,
}

/// A physical input that can be bound to an action.
///
/// Uses string representations for Bevy types so it can be serialized to RON/JSON.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Reflect)]
pub enum InputBinding {
    /// A keyboard key (stored as Debug string, e.g. "KeyW", "Space").
    Key(String),
    /// A mouse button ("Left", "Right", "Middle").
    MouseButton(String),
    /// A gamepad button ("South", "East", "West", "North", etc.).
    GamepadButton(String),
    /// A single gamepad axis ("LeftStickX", "RightStickY", etc.).
    GamepadAxis(String),
    /// Four keys composited into a 2D axis.
    Composite2D {
        up: String,
        down: String,
        left: String,
        right: String,
    },
}

impl InputBinding {
    pub fn key(key: KeyCode) -> Self {
        Self::Key(format!("{:?}", key))
    }

    pub fn mouse(button: bevy::prelude::MouseButton) -> Self {
        Self::MouseButton(format!("{:?}", button))
    }

    pub fn gamepad_button(button: GamepadButton) -> Self {
        Self::GamepadButton(format!("{:?}", button))
    }

    pub fn gamepad_axis(axis: GamepadAxis) -> Self {
        Self::GamepadAxis(format!("{:?}", axis))
    }

    pub fn composite_2d(up: KeyCode, down: KeyCode, left: KeyCode, right: KeyCode) -> Self {
        Self::Composite2D {
            up: format!("{:?}", up),
            down: format!("{:?}", down),
            left: format!("{:?}", left),
            right: format!("{:?}", right),
        }
    }

    /// The binding as one line of text: `Key:Space`, `Mouse:Left`,
    /// `Button:South`, `Axis:LeftStickX`, `Keys:KeyW,KeyS,KeyA,KeyD` (up, down,
    /// left, right).
    ///
    /// The form a script reads and writes a binding in. A script cannot hand
    /// over a Rust enum, and the RON form (`Key("Space")`) is awkward to build
    /// from a menu's "press a key" prompt, so this is the one string shape a
    /// Lua controls menu and the `input.<action>.<n>` setting paths agree on.
    /// [`Self::from_spec`] reads it back.
    pub fn to_spec(&self) -> String {
        match self {
            Self::Key(k) => format!("Key:{k}"),
            Self::MouseButton(b) => format!("Mouse:{b}"),
            Self::GamepadButton(b) => format!("Button:{b}"),
            Self::GamepadAxis(a) => format!("Axis:{a}"),
            Self::Composite2D {
                up,
                down,
                left,
                right,
            } => format!("Keys:{up},{down},{left},{right}"),
        }
    }

    /// Parse the text form [`Self::to_spec`] writes.
    ///
    /// Every name must resolve to a real input, so a typo is refused here
    /// rather than stored as a binding that silently never fires: the action
    /// map resolves names each frame and skips the ones it does not know, which
    /// would make a mistyped rebind look like a dead controller.
    pub fn from_spec(spec: &str) -> Option<Self> {
        let (kind, name) = spec.split_once(':')?;
        let name = name.trim();
        match kind.trim() {
            "Key" => Self::resolve_key(name).map(|_| Self::Key(name.to_string())),
            "Mouse" => Self::resolve_mouse(name).map(|_| Self::MouseButton(name.to_string())),
            "Button" => {
                Self::resolve_gamepad_button(name).map(|_| Self::GamepadButton(name.to_string()))
            }
            "Axis" => Self::resolve_gamepad_axis(name).map(|_| Self::GamepadAxis(name.to_string())),
            "Keys" => {
                let keys: Vec<&str> = name.split(',').map(str::trim).collect();
                let [up, down, left, right] = keys.as_slice() else {
                    return None;
                };
                if [up, down, left, right]
                    .iter()
                    .any(|k| Self::resolve_key(k).is_none())
                {
                    return None;
                }
                Some(Self::Composite2D {
                    up: up.to_string(),
                    down: down.to_string(),
                    left: left.to_string(),
                    right: right.to_string(),
                })
            }
            _ => None,
        }
    }

    /// Resolve the key string back to a KeyCode.
    pub fn resolve_key(s: &str) -> Option<KeyCode> {
        key_from_str(s)
    }

    /// Resolve the mouse button string back to a MouseButton.
    pub fn resolve_mouse(s: &str) -> Option<bevy::prelude::MouseButton> {
        match s {
            "Left" => Some(bevy::prelude::MouseButton::Left),
            "Right" => Some(bevy::prelude::MouseButton::Right),
            "Middle" => Some(bevy::prelude::MouseButton::Middle),
            _ => None,
        }
    }

    /// Resolve the gamepad button string back to a GamepadButton.
    pub fn resolve_gamepad_button(s: &str) -> Option<GamepadButton> {
        match s {
            "South" => Some(GamepadButton::South),
            "East" => Some(GamepadButton::East),
            "West" => Some(GamepadButton::West),
            "North" => Some(GamepadButton::North),
            "LeftTrigger" => Some(GamepadButton::LeftTrigger),
            "RightTrigger" => Some(GamepadButton::RightTrigger),
            "LeftTrigger2" => Some(GamepadButton::LeftTrigger2),
            "RightTrigger2" => Some(GamepadButton::RightTrigger2),
            "Select" => Some(GamepadButton::Select),
            "Start" => Some(GamepadButton::Start),
            "LeftThumb" => Some(GamepadButton::LeftThumb),
            "RightThumb" => Some(GamepadButton::RightThumb),
            "DPadUp" => Some(GamepadButton::DPadUp),
            "DPadDown" => Some(GamepadButton::DPadDown),
            "DPadLeft" => Some(GamepadButton::DPadLeft),
            "DPadRight" => Some(GamepadButton::DPadRight),
            _ => None,
        }
    }

    /// Resolve the gamepad axis string back to a GamepadAxis.
    pub fn resolve_gamepad_axis(s: &str) -> Option<GamepadAxis> {
        match s {
            "LeftStickX" => Some(GamepadAxis::LeftStickX),
            "LeftStickY" => Some(GamepadAxis::LeftStickY),
            "RightStickX" => Some(GamepadAxis::RightStickX),
            "RightStickY" => Some(GamepadAxis::RightStickY),
            "LeftZ" => Some(GamepadAxis::LeftZ),
            "RightZ" => Some(GamepadAxis::RightZ),
            _ => None,
        }
    }
}

/// A named input action with one or more bindings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Reflect)]
pub struct InputAction {
    /// Unique name (e.g. "move", "jump", "sprint").
    pub name: String,
    /// What kind of value this action produces.
    pub kind: ActionKind,
    /// Physical inputs bound to this action.
    pub bindings: Vec<InputBinding>,
    /// Dead zone for analog inputs (0.0 to 1.0).
    pub dead_zone: f32,
}

impl InputAction {
    /// Create a new button action.
    pub fn button(name: impl Into<String>, bindings: Vec<InputBinding>) -> Self {
        Self {
            name: name.into(),
            kind: ActionKind::Button,
            bindings,
            dead_zone: 0.0,
        }
    }

    /// Create a new 2D axis action.
    pub fn axis_2d(name: impl Into<String>, bindings: Vec<InputBinding>, dead_zone: f32) -> Self {
        Self {
            name: name.into(),
            kind: ActionKind::Axis2D,
            bindings,
            dead_zone,
        }
    }

    /// Create a new 1D axis action.
    pub fn axis_1d(name: impl Into<String>, bindings: Vec<InputBinding>, dead_zone: f32) -> Self {
        Self {
            name: name.into(),
            kind: ActionKind::Axis1D,
            bindings,
            dead_zone,
        }
    }
}

/// The project's input action definitions.
///
/// A resource, loaded from `input_map.ron` by `renzora_input` at startup and
/// read by it every frame, so a script that changes it changes the game's
/// controls from the next frame on. Changing it does not write the file: a
/// game that wants a player's bindings to survive a restart saves them itself
/// (see `renzora::save`).
#[derive(Resource, Clone, Debug, PartialEq, Serialize, Deserialize, Reflect)]
pub struct InputMap {
    pub actions: Vec<InputAction>,
}

impl Default for InputMap {
    fn default() -> Self {
        Self {
            actions: default_actions(),
        }
    }
}

/// Why [`InputMap::rebind`] refused a change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RebindError {
    /// No action has this name.
    UnknownAction(String),
    /// The slot is past the end of the action's bindings (one past the end is
    /// allowed: it appends).
    SlotOutOfRange { action: String, slot: usize, len: usize },
}

impl std::fmt::Display for RebindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownAction(a) => write!(f, "no input action named `{a}`"),
            Self::SlotOutOfRange { action, slot, len } => write!(
                f,
                "`{action}` has {len} binding(s), so slot {slot} is out of range"
            ),
        }
    }
}

impl InputMap {
    /// Look up an action by name.
    pub fn get(&self, name: &str) -> Option<&InputAction> {
        self.actions.iter().find(|a| a.name == name)
    }

    /// Look up an action by name, to change it.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut InputAction> {
        self.actions.iter_mut().find(|a| a.name == name)
    }

    /// Add an action. Returns false if the name already exists.
    pub fn add(&mut self, action: InputAction) -> bool {
        if self.actions.iter().any(|a| a.name == action.name) {
            return false;
        }
        self.actions.push(action);
        true
    }

    /// Remove an action by name.
    pub fn remove(&mut self, name: &str) -> bool {
        let len = self.actions.len();
        self.actions.retain(|a| a.name != name);
        self.actions.len() < len
    }

    /// Put `binding` in slot `slot` of `action`, or clear that slot with `None`.
    ///
    /// Slots are positions in the action's binding list, which is how a
    /// controls menu presents them ("primary", "alternate"). Writing one past
    /// the end appends, so a menu can add a second key without knowing how
    /// many there were; clearing removes the slot and shifts the rest down.
    /// Anything further out is an error rather than a silent append, because a
    /// script asking for slot 5 of a two-binding action has its numbering
    /// wrong and should hear about it.
    pub fn rebind(
        &mut self,
        action: &str,
        slot: usize,
        binding: Option<InputBinding>,
    ) -> Result<(), RebindError> {
        let entry = self
            .get_mut(action)
            .ok_or_else(|| RebindError::UnknownAction(action.to_string()))?;
        let len = entry.bindings.len();
        match binding {
            Some(b) if slot < len => entry.bindings[slot] = b,
            Some(b) if slot == len => entry.bindings.push(b),
            None if slot < len => {
                entry.bindings.remove(slot);
            }
            _ => {
                return Err(RebindError::SlotOutOfRange {
                    action: action.to_string(),
                    slot,
                    len,
                })
            }
        }
        Ok(())
    }
}

/// Default input actions that ship with the engine.
pub fn default_actions() -> Vec<InputAction> {
    vec![
        // Movement: WASD + arrows + left stick
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
        // Look: right stick (mouse look handled separately via delta)
        InputAction::axis_2d(
            "look",
            vec![InputBinding::GamepadAxis("RightStickX".into())],
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
        // Driving a vehicle with a gamepad: the triggers. The keyboard drives
        // with `move` (W and S), which a vehicle reads as well.
        InputAction::button("accelerate", vec![InputBinding::GamepadButton("RightTrigger2".into())]),
        InputAction::button("brake", vec![InputBinding::GamepadButton("LeftTrigger2".into())]),
    ]
}

/// Parse a KeyCode from its Debug string representation.
fn key_from_str(s: &str) -> Option<KeyCode> {
    Some(match s {
        "KeyA" => KeyCode::KeyA,
        "KeyB" => KeyCode::KeyB,
        "KeyC" => KeyCode::KeyC,
        "KeyD" => KeyCode::KeyD,
        "KeyE" => KeyCode::KeyE,
        "KeyF" => KeyCode::KeyF,
        "KeyG" => KeyCode::KeyG,
        "KeyH" => KeyCode::KeyH,
        "KeyI" => KeyCode::KeyI,
        "KeyJ" => KeyCode::KeyJ,
        "KeyK" => KeyCode::KeyK,
        "KeyL" => KeyCode::KeyL,
        "KeyM" => KeyCode::KeyM,
        "KeyN" => KeyCode::KeyN,
        "KeyO" => KeyCode::KeyO,
        "KeyP" => KeyCode::KeyP,
        "KeyQ" => KeyCode::KeyQ,
        "KeyR" => KeyCode::KeyR,
        "KeyS" => KeyCode::KeyS,
        "KeyT" => KeyCode::KeyT,
        "KeyU" => KeyCode::KeyU,
        "KeyV" => KeyCode::KeyV,
        "KeyW" => KeyCode::KeyW,
        "KeyX" => KeyCode::KeyX,
        "KeyY" => KeyCode::KeyY,
        "KeyZ" => KeyCode::KeyZ,
        "Digit0" => KeyCode::Digit0,
        "Digit1" => KeyCode::Digit1,
        "Digit2" => KeyCode::Digit2,
        "Digit3" => KeyCode::Digit3,
        "Digit4" => KeyCode::Digit4,
        "Digit5" => KeyCode::Digit5,
        "Digit6" => KeyCode::Digit6,
        "Digit7" => KeyCode::Digit7,
        "Digit8" => KeyCode::Digit8,
        "Digit9" => KeyCode::Digit9,
        "Space" => KeyCode::Space,
        "Enter" => KeyCode::Enter,
        "Escape" => KeyCode::Escape,
        "Tab" => KeyCode::Tab,
        "Backspace" => KeyCode::Backspace,
        "ShiftLeft" => KeyCode::ShiftLeft,
        "ShiftRight" => KeyCode::ShiftRight,
        "ControlLeft" => KeyCode::ControlLeft,
        "ControlRight" => KeyCode::ControlRight,
        "AltLeft" => KeyCode::AltLeft,
        "AltRight" => KeyCode::AltRight,
        "ArrowUp" => KeyCode::ArrowUp,
        "ArrowDown" => KeyCode::ArrowDown,
        "ArrowLeft" => KeyCode::ArrowLeft,
        "ArrowRight" => KeyCode::ArrowRight,
        "F1" => KeyCode::F1,
        "F2" => KeyCode::F2,
        "F3" => KeyCode::F3,
        "F4" => KeyCode::F4,
        "F5" => KeyCode::F5,
        "F6" => KeyCode::F6,
        "F7" => KeyCode::F7,
        "F8" => KeyCode::F8,
        "F9" => KeyCode::F9,
        "F10" => KeyCode::F10,
        "F11" => KeyCode::F11,
        "F12" => KeyCode::F12,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jump_action() -> InputAction {
        InputAction::button("jump", vec![InputBinding::key(KeyCode::Space)])
    }

    #[test]
    fn key_constructor_round_trips() {
        // The Debug format of `KeyCode::KeyA` is `"KeyA"`. If Bevy ever
        // changed that, our serialized input maps would silently stop
        // resolving and every binding would go dead.
        let binding = InputBinding::key(KeyCode::KeyA);
        let InputBinding::Key(s) = binding else {
            panic!("wrong variant")
        };
        assert_eq!(s, "KeyA");
        assert_eq!(InputBinding::resolve_key(&s), Some(KeyCode::KeyA));
    }

    #[test]
    fn key_round_trip_covers_letters_digits_and_specials() {
        for kc in [
            KeyCode::KeyW,
            KeyCode::Space,
            KeyCode::Escape,
            KeyCode::Enter,
            KeyCode::Tab,
            KeyCode::Backspace,
            KeyCode::ShiftLeft,
            KeyCode::ControlLeft,
            KeyCode::ArrowUp,
            KeyCode::F5,
            KeyCode::Digit0,
            KeyCode::Digit9,
        ] {
            let binding = InputBinding::key(kc);
            let InputBinding::Key(s) = binding else {
                panic!("wrong variant")
            };
            assert_eq!(
                InputBinding::resolve_key(&s),
                Some(kc),
                "round trip failed for {:?}",
                kc
            );
        }
    }

    #[test]
    fn resolve_key_unknown_returns_none() {
        assert_eq!(InputBinding::resolve_key("ImaginaryKey"), None);
        assert_eq!(InputBinding::resolve_key(""), None);
    }

    #[test]
    fn mouse_constructor_round_trips() {
        for btn in [
            bevy::prelude::MouseButton::Left,
            bevy::prelude::MouseButton::Right,
            bevy::prelude::MouseButton::Middle,
        ] {
            let binding = InputBinding::mouse(btn);
            let InputBinding::MouseButton(s) = binding else {
                panic!()
            };
            assert_eq!(InputBinding::resolve_mouse(&s), Some(btn));
        }
    }

    #[test]
    fn resolve_mouse_unknown_returns_none() {
        assert_eq!(InputBinding::resolve_mouse("Wheel"), None);
        assert_eq!(InputBinding::resolve_mouse(""), None);
    }

    #[test]
    fn gamepad_button_round_trips() {
        for btn in [
            GamepadButton::South,
            GamepadButton::DPadLeft,
            GamepadButton::Start,
        ] {
            let binding = InputBinding::gamepad_button(btn);
            let InputBinding::GamepadButton(s) = binding else {
                panic!()
            };
            assert_eq!(InputBinding::resolve_gamepad_button(&s), Some(btn));
        }
    }

    #[test]
    fn gamepad_axis_round_trips() {
        for axis in [
            GamepadAxis::LeftStickX,
            GamepadAxis::LeftStickY,
            GamepadAxis::RightStickX,
            GamepadAxis::RightStickY,
        ] {
            let binding = InputBinding::gamepad_axis(axis);
            let InputBinding::GamepadAxis(s) = binding else {
                panic!()
            };
            assert_eq!(InputBinding::resolve_gamepad_axis(&s), Some(axis));
        }
    }

    #[test]
    fn composite_2d_stores_debug_strings_per_direction() {
        let binding =
            InputBinding::composite_2d(KeyCode::KeyW, KeyCode::KeyS, KeyCode::KeyA, KeyCode::KeyD);
        let InputBinding::Composite2D {
            up,
            down,
            left,
            right,
        } = binding
        else {
            panic!()
        };
        assert_eq!(up, "KeyW");
        assert_eq!(down, "KeyS");
        assert_eq!(left, "KeyA");
        assert_eq!(right, "KeyD");
    }

    #[test]
    fn every_binding_kind_round_trips_through_its_text_form() {
        // The text form is what a script reads with `get_setting` and writes
        // back with `set_setting`, so reading and writing must agree exactly.
        for b in [
            InputBinding::key(KeyCode::Space),
            InputBinding::mouse(bevy::prelude::MouseButton::Right),
            InputBinding::gamepad_button(GamepadButton::South),
            InputBinding::gamepad_axis(GamepadAxis::LeftStickX),
            InputBinding::composite_2d(KeyCode::KeyW, KeyCode::KeyS, KeyCode::KeyA, KeyCode::KeyD),
        ] {
            let spec = b.to_spec();
            assert_eq!(InputBinding::from_spec(&spec), Some(b), "spec {spec}");
        }
        assert_eq!(InputBinding::key(KeyCode::KeyF).to_spec(), "Key:KeyF");
    }

    #[test]
    fn a_binding_naming_no_real_input_is_refused() {
        // Stored, these would be bindings that never fire: a dead control
        // with nothing logged.
        assert_eq!(InputBinding::from_spec("Key:F"), None);
        assert_eq!(InputBinding::from_spec("Mouse:Wheel"), None);
        assert_eq!(InputBinding::from_spec("Keys:KeyW,KeyS,KeyA"), None);
        assert_eq!(InputBinding::from_spec("Space"), None);
        assert_eq!(InputBinding::from_spec("Joystick:South"), None);
    }

    #[test]
    fn action_constructors_set_kind_and_dead_zone() {
        let a = InputAction::button("jump", vec![InputBinding::key(KeyCode::Space)]);
        assert_eq!(a.kind, ActionKind::Button);
        assert_eq!(a.dead_zone, 0.0);
        let a = InputAction::axis_2d("move", vec![], 0.15);
        assert_eq!(a.kind, ActionKind::Axis2D);
        assert!((a.dead_zone - 0.15).abs() < 1e-6);
        let a = InputAction::axis_1d("look_x", vec![], 0.05);
        assert_eq!(a.kind, ActionKind::Axis1D);
    }

    #[test]
    fn default_input_map_has_actions() {
        // A freshly created project is not bound to nothing.
        assert!(!InputMap::default().actions.is_empty());
    }

    #[test]
    fn add_get_and_remove_by_name() {
        let mut map = InputMap { actions: Vec::new() };
        assert!(map.add(jump_action()));
        // Names are unique: a second `add` must report failure rather than
        // silently double up.
        assert!(!map.add(jump_action()));
        assert_eq!(map.get("jump").map(|a| a.kind), Some(ActionKind::Button));
        assert!(map.get("nope").is_none());
        assert!(map.remove("jump"));
        assert!(!map.remove("jump"));
    }

    #[test]
    fn rebinding_replaces_appends_and_clears_slots() {
        let mut map = InputMap {
            actions: vec![jump_action()],
        };
        let f = InputBinding::key(KeyCode::KeyF);
        let south = InputBinding::gamepad_button(GamepadButton::South);

        map.rebind("jump", 0, Some(f.clone())).unwrap();
        assert_eq!(map.get("jump").unwrap().bindings, vec![f.clone()]);

        map.rebind("jump", 1, Some(south.clone())).unwrap();
        assert_eq!(map.get("jump").unwrap().bindings, vec![f, south.clone()]);

        map.rebind("jump", 0, None).unwrap();
        assert_eq!(map.get("jump").unwrap().bindings, vec![south]);
    }

    #[test]
    fn rebinding_refuses_unknown_actions_and_far_slots() {
        let mut map = InputMap {
            actions: vec![jump_action()],
        };
        let f = InputBinding::key(KeyCode::KeyF);
        assert_eq!(
            map.rebind("fly", 0, Some(f.clone())),
            Err(RebindError::UnknownAction("fly".into()))
        );
        assert!(matches!(
            map.rebind("jump", 5, Some(f)),
            Err(RebindError::SlotOutOfRange { slot: 5, len: 1, .. })
        ));
        assert!(map.rebind("jump", 1, None).is_err());
        assert_eq!(map.get("jump").unwrap().bindings.len(), 1);
    }
}
