use bevy::prelude::*;
use std::collections::HashMap;

/// Actions that can be bound to keys
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EditorAction {
    // Camera movement
    CameraMoveForward,
    CameraMoveBackward,
    CameraMoveLeft,
    CameraMoveRight,
    CameraMoveUp,
    CameraMoveDown,
    CameraMoveFaster,
    FocusSelected,

    // Tool modes
    ToolSelect,
    GizmoTranslate,
    GizmoRotate,
    GizmoScale,

    // Modal transforms (Blender-style)
    ModalGrab,
    ModalRotate,
    ModalScale,

    // Selection
    Delete,
    Duplicate,
    DuplicateAndMove,
    Deselect,

    // Edit operations
    Undo,
    Redo,

    // File operations
    SaveScene,
    SaveSceneAs,
    OpenScene,
    NewScene,
    OpenSettings,

    // View operations
    ToggleBottomPanel,
    ToggleWireframe,
    ToggleLighting,
    ToggleGrid,

    // Play mode
    PlayStop,
    PlayScriptsOnly,

    // Camera view angles
    ViewFront,
    ViewBack,
    ViewLeft,
    ViewRight,
    ViewTop,
    ViewBottom,
    ToggleProjection,
}

impl EditorAction {
    pub fn display_name(&self) -> &'static str {
        match self {
            EditorAction::CameraMoveForward => "Move Forward",
            EditorAction::CameraMoveBackward => "Move Backward",
            EditorAction::CameraMoveLeft => "Move Left",
            EditorAction::CameraMoveRight => "Move Right",
            EditorAction::CameraMoveUp => "Move Up",
            EditorAction::CameraMoveDown => "Move Down",
            EditorAction::CameraMoveFaster => "Move Faster (Hold)",
            EditorAction::FocusSelected => "Focus Selected",
            EditorAction::ToolSelect => "Select Mode",
            EditorAction::GizmoTranslate => "Translate Mode",
            EditorAction::GizmoRotate => "Rotate Mode",
            EditorAction::GizmoScale => "Scale Mode",
            EditorAction::ModalGrab => "Grab (Move)",
            EditorAction::ModalRotate => "Rotate",
            EditorAction::ModalScale => "Scale",
            EditorAction::Delete => "Delete",
            EditorAction::Duplicate => "Duplicate",
            EditorAction::DuplicateAndMove => "Duplicate & Move",
            EditorAction::Deselect => "Deselect",
            EditorAction::Undo => "Undo",
            EditorAction::Redo => "Redo",
            EditorAction::SaveScene => "Save Scene",
            EditorAction::SaveSceneAs => "Save Scene As",
            EditorAction::OpenScene => "Open Scene",
            EditorAction::NewScene => "New Scene",
            EditorAction::OpenSettings => "Settings",
            EditorAction::ToggleBottomPanel => "Toggle Bottom Panel",
            EditorAction::ToggleWireframe => "Toggle Wireframe",
            EditorAction::ToggleLighting => "Toggle Lighting",
            EditorAction::ToggleGrid => "Toggle Grid",
            EditorAction::PlayStop => "Play/Stop",
            EditorAction::PlayScriptsOnly => "Run Scripts Only",
            EditorAction::ViewFront => "View Front",
            EditorAction::ViewBack => "View Back",
            EditorAction::ViewLeft => "View Left",
            EditorAction::ViewRight => "View Right",
            EditorAction::ViewTop => "View Top",
            EditorAction::ViewBottom => "View Bottom",
            EditorAction::ToggleProjection => "Toggle Ortho/Persp",
        }
    }

    pub fn category(&self) -> &'static str {
        match self {
            EditorAction::CameraMoveForward
            | EditorAction::CameraMoveBackward
            | EditorAction::CameraMoveLeft
            | EditorAction::CameraMoveRight
            | EditorAction::CameraMoveUp
            | EditorAction::CameraMoveDown
            | EditorAction::CameraMoveFaster
            | EditorAction::FocusSelected => "Camera",

            EditorAction::ToolSelect
            | EditorAction::GizmoTranslate
            | EditorAction::GizmoRotate
            | EditorAction::GizmoScale => "Tools",

            EditorAction::ModalGrab
            | EditorAction::ModalRotate
            | EditorAction::ModalScale => "Transform",

            EditorAction::Delete | EditorAction::Duplicate | EditorAction::DuplicateAndMove | EditorAction::Deselect => "Selection",

            EditorAction::Undo | EditorAction::Redo => "Edit",

            EditorAction::SaveScene
            | EditorAction::SaveSceneAs
            | EditorAction::OpenScene
            | EditorAction::NewScene
            | EditorAction::OpenSettings => "File",

            EditorAction::ToggleBottomPanel
            | EditorAction::ToggleWireframe
            | EditorAction::ToggleLighting
            | EditorAction::ToggleGrid
            | EditorAction::ViewFront
            | EditorAction::ViewBack
            | EditorAction::ViewLeft
            | EditorAction::ViewRight
            | EditorAction::ViewTop
            | EditorAction::ViewBottom
            | EditorAction::ToggleProjection => "View",

            EditorAction::PlayStop | EditorAction::PlayScriptsOnly => "Play",
        }
    }

    /// Get all actions in order for display
    pub fn all() -> Vec<EditorAction> {
        vec![
            // Camera
            EditorAction::CameraMoveForward,
            EditorAction::CameraMoveBackward,
            EditorAction::CameraMoveLeft,
            EditorAction::CameraMoveRight,
            EditorAction::CameraMoveUp,
            EditorAction::CameraMoveDown,
            EditorAction::CameraMoveFaster,
            EditorAction::FocusSelected,
            // Tools
            EditorAction::ToolSelect,
            EditorAction::GizmoTranslate,
            EditorAction::GizmoRotate,
            EditorAction::GizmoScale,
            // Transform (modal)
            EditorAction::ModalGrab,
            EditorAction::ModalRotate,
            EditorAction::ModalScale,
            // Selection
            EditorAction::Delete,
            EditorAction::Duplicate,
            EditorAction::DuplicateAndMove,
            EditorAction::Deselect,
            // Edit
            EditorAction::Undo,
            EditorAction::Redo,
            // File
            EditorAction::SaveScene,
            EditorAction::SaveSceneAs,
            EditorAction::OpenScene,
            EditorAction::NewScene,
            EditorAction::OpenSettings,
            // View
            EditorAction::ToggleBottomPanel,
            EditorAction::ToggleWireframe,
            EditorAction::ToggleLighting,
            EditorAction::ToggleGrid,
            EditorAction::ViewFront,
            EditorAction::ViewBack,
            EditorAction::ViewLeft,
            EditorAction::ViewRight,
            EditorAction::ViewTop,
            EditorAction::ViewBottom,
            EditorAction::ToggleProjection,
            // Play
            EditorAction::PlayStop,
            EditorAction::PlayScriptsOnly,
        ]
    }
}

/// A key combination (key + modifiers)
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct KeyBinding {
    pub key: KeyCode,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl KeyBinding {
    pub fn new(key: KeyCode) -> Self {
        Self {
            key,
            ctrl: false,
            shift: false,
            alt: false,
        }
    }

    pub fn ctrl(mut self) -> Self {
        self.ctrl = true;
        self
    }

    pub fn shift(mut self) -> Self {
        self.shift = true;
        self
    }

    pub fn alt(mut self) -> Self {
        self.alt = true;
        self
    }

    pub fn display(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        parts.push(key_name(self.key));
        parts.join(" + ")
    }
}

/// Resource storing all keybindings
#[derive(Resource, Clone)]
pub struct KeyBindings {
    pub bindings: HashMap<EditorAction, KeyBinding>,
    /// Action currently being rebound (if any)
    pub rebinding: Option<EditorAction>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        let mut bindings = HashMap::new();

        // Camera defaults
        bindings.insert(EditorAction::CameraMoveForward, KeyBinding::new(KeyCode::KeyW));
        bindings.insert(EditorAction::CameraMoveBackward, KeyBinding::new(KeyCode::KeyS));
        bindings.insert(EditorAction::CameraMoveLeft, KeyBinding::new(KeyCode::KeyA));
        bindings.insert(EditorAction::CameraMoveRight, KeyBinding::new(KeyCode::KeyD));
        bindings.insert(EditorAction::CameraMoveUp, KeyBinding::new(KeyCode::KeyE));
        bindings.insert(EditorAction::CameraMoveDown, KeyBinding::new(KeyCode::KeyQ));
        bindings.insert(EditorAction::CameraMoveFaster, KeyBinding::new(KeyCode::ShiftLeft));
        bindings.insert(EditorAction::FocusSelected, KeyBinding::new(KeyCode::KeyF));

        // Tool defaults
        bindings.insert(EditorAction::ToolSelect, KeyBinding::new(KeyCode::KeyQ));
        bindings.insert(EditorAction::GizmoTranslate, KeyBinding::new(KeyCode::KeyW));
        bindings.insert(EditorAction::GizmoRotate, KeyBinding::new(KeyCode::KeyE));
        bindings.insert(EditorAction::GizmoScale, KeyBinding::new(KeyCode::KeyR));

        // Modal transform defaults (Blender-style)
        bindings.insert(EditorAction::ModalGrab, KeyBinding::new(KeyCode::KeyG));
        bindings.insert(EditorAction::ModalRotate, KeyBinding::new(KeyCode::KeyR));
        bindings.insert(EditorAction::ModalScale, KeyBinding::new(KeyCode::KeyS));

        // Selection defaults
        bindings.insert(EditorAction::Delete, KeyBinding::new(KeyCode::Delete));
        bindings.insert(EditorAction::Duplicate, KeyBinding::new(KeyCode::KeyD).ctrl());
        bindings.insert(EditorAction::DuplicateAndMove, KeyBinding::new(KeyCode::KeyD).alt());
        bindings.insert(EditorAction::Deselect, KeyBinding::new(KeyCode::Escape));

        // Edit defaults
        bindings.insert(EditorAction::Undo, KeyBinding::new(KeyCode::KeyZ).ctrl());
        bindings.insert(EditorAction::Redo, KeyBinding::new(KeyCode::KeyY).ctrl());

        // File defaults
        bindings.insert(EditorAction::SaveScene, KeyBinding::new(KeyCode::KeyS).ctrl());
        bindings.insert(EditorAction::SaveSceneAs, KeyBinding::new(KeyCode::KeyS).ctrl().shift());
        bindings.insert(EditorAction::OpenScene, KeyBinding::new(KeyCode::KeyO).ctrl());
        bindings.insert(EditorAction::NewScene, KeyBinding::new(KeyCode::KeyN).ctrl());
        bindings.insert(EditorAction::OpenSettings, KeyBinding::new(KeyCode::Comma).ctrl());

        // View defaults
        bindings.insert(EditorAction::ToggleBottomPanel, KeyBinding::new(KeyCode::Backquote).ctrl());
        bindings.insert(EditorAction::ToggleWireframe, KeyBinding::new(KeyCode::KeyZ));
        bindings.insert(EditorAction::ToggleLighting, KeyBinding::new(KeyCode::KeyZ).shift());
        bindings.insert(EditorAction::ToggleGrid, KeyBinding::new(KeyCode::KeyH));

        // Play mode
        bindings.insert(EditorAction::PlayStop, KeyBinding::new(KeyCode::F5));
        bindings.insert(EditorAction::PlayScriptsOnly, KeyBinding::new(KeyCode::F5).shift());

        // View angle defaults (Blender-style numpad)
        bindings.insert(EditorAction::ViewFront, KeyBinding::new(KeyCode::Numpad1));
        bindings.insert(EditorAction::ViewBack, KeyBinding::new(KeyCode::Numpad1).ctrl());
        bindings.insert(EditorAction::ViewRight, KeyBinding::new(KeyCode::Numpad3));
        bindings.insert(EditorAction::ViewLeft, KeyBinding::new(KeyCode::Numpad3).ctrl());
        bindings.insert(EditorAction::ViewTop, KeyBinding::new(KeyCode::Numpad7));
        bindings.insert(EditorAction::ViewBottom, KeyBinding::new(KeyCode::Numpad7).ctrl());
        bindings.insert(EditorAction::ToggleProjection, KeyBinding::new(KeyCode::Numpad5));

        Self {
            bindings,
            rebinding: None,
        }
    }
}

impl KeyBindings {
    /// Check if an action key is currently pressed (for hold actions like movement)
    pub fn pressed(&self, action: EditorAction, keyboard: &ButtonInput<KeyCode>) -> bool {
        if let Some(binding) = self.bindings.get(&action) {
            keyboard.pressed(binding.key)
        } else {
            false
        }
    }

    /// Check if an action key was just pressed this frame (with exact modifier check)
    pub fn just_pressed(&self, action: EditorAction, keyboard: &ButtonInput<KeyCode>) -> bool {
        if let Some(binding) = self.bindings.get(&action) {
            let key_just_pressed = keyboard.just_pressed(binding.key);
            let ctrl_pressed = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
            let shift_pressed = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
            let alt_pressed = keyboard.pressed(KeyCode::AltLeft) || keyboard.pressed(KeyCode::AltRight);

            // Exact modifier matching: modifiers must match exactly
            // (e.g., Ctrl+Z won't trigger a binding for just Z)
            let ctrl_ok = binding.ctrl == ctrl_pressed;
            let shift_ok = binding.shift == shift_pressed;
            let alt_ok = binding.alt == alt_pressed;

            key_just_pressed && ctrl_ok && shift_ok && alt_ok
        } else {
            false
        }
    }

    /// Get the binding for an action
    pub fn get(&self, action: EditorAction) -> Option<&KeyBinding> {
        self.bindings.get(&action)
    }

    /// Set the binding for an action
    pub fn set(&mut self, action: EditorAction, binding: KeyBinding) {
        self.bindings.insert(action, binding);
    }
}

/// Convert KeyCode to display name
pub fn key_name(key: KeyCode) -> &'static str {
    match key {
        KeyCode::KeyA => "A",
        KeyCode::KeyB => "B",
        KeyCode::KeyC => "C",
        KeyCode::KeyD => "D",
        KeyCode::KeyE => "E",
        KeyCode::KeyF => "F",
        KeyCode::KeyG => "G",
        KeyCode::KeyH => "H",
        KeyCode::KeyI => "I",
        KeyCode::KeyJ => "J",
        KeyCode::KeyK => "K",
        KeyCode::KeyL => "L",
        KeyCode::KeyM => "M",
        KeyCode::KeyN => "N",
        KeyCode::KeyO => "O",
        KeyCode::KeyP => "P",
        KeyCode::KeyQ => "Q",
        KeyCode::KeyR => "R",
        KeyCode::KeyS => "S",
        KeyCode::KeyT => "T",
        KeyCode::KeyU => "U",
        KeyCode::KeyV => "V",
        KeyCode::KeyW => "W",
        KeyCode::KeyX => "X",
        KeyCode::KeyY => "Y",
        KeyCode::KeyZ => "Z",
        KeyCode::Digit0 => "0",
        KeyCode::Digit1 => "1",
        KeyCode::Digit2 => "2",
        KeyCode::Digit3 => "3",
        KeyCode::Digit4 => "4",
        KeyCode::Digit5 => "5",
        KeyCode::Digit6 => "6",
        KeyCode::Digit7 => "7",
        KeyCode::Digit8 => "8",
        KeyCode::Digit9 => "9",
        KeyCode::Escape => "Esc",
        KeyCode::F1 => "F1",
        KeyCode::F2 => "F2",
        KeyCode::F3 => "F3",
        KeyCode::F4 => "F4",
        KeyCode::F5 => "F5",
        KeyCode::F6 => "F6",
        KeyCode::F7 => "F7",
        KeyCode::F8 => "F8",
        KeyCode::F9 => "F9",
        KeyCode::F10 => "F10",
        KeyCode::F11 => "F11",
        KeyCode::F12 => "F12",
        KeyCode::Space => "Space",
        KeyCode::Tab => "Tab",
        KeyCode::Enter => "Enter",
        KeyCode::Backspace => "Backspace",
        KeyCode::Delete => "Delete",
        KeyCode::Insert => "Insert",
        KeyCode::Home => "Home",
        KeyCode::End => "End",
        KeyCode::PageUp => "PgUp",
        KeyCode::PageDown => "PgDn",
        KeyCode::ArrowUp => "Up",
        KeyCode::ArrowDown => "Down",
        KeyCode::ArrowLeft => "Left",
        KeyCode::ArrowRight => "Right",
        KeyCode::ShiftLeft | KeyCode::ShiftRight => "Shift",
        KeyCode::ControlLeft | KeyCode::ControlRight => "Ctrl",
        KeyCode::AltLeft | KeyCode::AltRight => "Alt",
        KeyCode::Comma => ",",
        KeyCode::Period => ".",
        KeyCode::Slash => "/",
        KeyCode::Backslash => "\\",
        KeyCode::BracketLeft => "[",
        KeyCode::BracketRight => "]",
        KeyCode::Semicolon => ";",
        KeyCode::Quote => "Quote",
        KeyCode::Backquote => "`",
        KeyCode::Minus => "-",
        KeyCode::Equal => "=",
        KeyCode::Numpad0 => "Num0",
        KeyCode::Numpad1 => "Num1",
        KeyCode::Numpad2 => "Num2",
        KeyCode::Numpad3 => "Num3",
        KeyCode::Numpad4 => "Num4",
        KeyCode::Numpad5 => "Num5",
        KeyCode::Numpad6 => "Num6",
        KeyCode::Numpad7 => "Num7",
        KeyCode::Numpad8 => "Num8",
        KeyCode::Numpad9 => "Num9",
        _ => "?",
    }
}

/// Get all available keys for binding
pub fn bindable_keys() -> Vec<KeyCode> {
    vec![
        KeyCode::KeyA, KeyCode::KeyB, KeyCode::KeyC, KeyCode::KeyD,
        KeyCode::KeyE, KeyCode::KeyF, KeyCode::KeyG, KeyCode::KeyH,
        KeyCode::KeyI, KeyCode::KeyJ, KeyCode::KeyK, KeyCode::KeyL,
        KeyCode::KeyM, KeyCode::KeyN, KeyCode::KeyO, KeyCode::KeyP,
        KeyCode::KeyQ, KeyCode::KeyR, KeyCode::KeyS, KeyCode::KeyT,
        KeyCode::KeyU, KeyCode::KeyV, KeyCode::KeyW, KeyCode::KeyX,
        KeyCode::KeyY, KeyCode::KeyZ,
        KeyCode::Digit0, KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3,
        KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7,
        KeyCode::Digit8, KeyCode::Digit9,
        KeyCode::F1, KeyCode::F2, KeyCode::F3, KeyCode::F4,
        KeyCode::F5, KeyCode::F6, KeyCode::F7, KeyCode::F8,
        KeyCode::F9, KeyCode::F10, KeyCode::F11, KeyCode::F12,
        KeyCode::Space, KeyCode::Tab, KeyCode::Enter, KeyCode::Escape,
        KeyCode::Backspace, KeyCode::Delete, KeyCode::Insert,
        KeyCode::Home, KeyCode::End, KeyCode::PageUp, KeyCode::PageDown,
        KeyCode::ArrowUp, KeyCode::ArrowDown, KeyCode::ArrowLeft, KeyCode::ArrowRight,
        KeyCode::Comma, KeyCode::Period, KeyCode::Slash, KeyCode::Backslash,
        KeyCode::BracketLeft, KeyCode::BracketRight, KeyCode::Semicolon,
        KeyCode::Quote, KeyCode::Backquote, KeyCode::Minus, KeyCode::Equal,
        KeyCode::Numpad0, KeyCode::Numpad1, KeyCode::Numpad2, KeyCode::Numpad3,
        KeyCode::Numpad4, KeyCode::Numpad5, KeyCode::Numpad6, KeyCode::Numpad7,
        KeyCode::Numpad8, KeyCode::Numpad9,
    ]
}
