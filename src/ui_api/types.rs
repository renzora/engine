//! Core UI types for the plugin API.

use serde::{Deserialize, Serialize};

/// Unique identifier for UI elements.
/// Used for event routing and widget identification.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
#[repr(C)]
pub struct UiId(pub u64);

impl UiId {
    /// Create a new UiId from a u64
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Create a UiId from a string hash
    pub fn from_str(s: &str) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        Self(hasher.finish())
    }
}

/// Semantic color names - editor applies actual colors from theme.
/// Plugins should use semantic colors instead of hardcoded values.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum SemanticColor {
    #[default]
    Primary,
    Secondary,
    Accent,
    Success,
    Warning,
    Error,
    Background,
    Surface,
    Text,
    TextMuted,
    Border,
}

/// Semantic text styles.
/// The editor applies appropriate fonts and sizes from the current theme.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum TextStyle {
    #[default]
    Body,
    Heading1,
    Heading2,
    Heading3,
    Caption,
    Code,
    Label,
}

/// Layout direction for containers.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum Direction {
    #[default]
    Horizontal,
    Vertical,
}

/// Size specification for widgets and layout.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum Size {
    /// Automatically size based on content
    Auto,
    /// Fixed size in logical pixels
    Fixed(f32),
    /// Percentage of parent container (0.0 - 1.0)
    Percent(f32),
    /// Fill all available space
    Fill,
    /// Fill with a proportional portion (for flex layouts)
    FillPortion(u32),
}

impl Default for Size {
    fn default() -> Self {
        Self::Auto
    }
}

/// Alignment for widgets within containers.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

/// Cross-axis alignment for flex containers.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum CrossAlign {
    #[default]
    Start,
    Center,
    End,
    Stretch,
    Baseline,
}

/// Justify content for flex containers.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum JustifyContent {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// 2D vector for positions and sizes
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[repr(C)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// 3D vector for positions
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[repr(C)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

/// RGBA color
#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(C)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Default for Color {
    fn default() -> Self {
        Self::WHITE
    }
}

impl Color {
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const RED: Self = Self {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const GREEN: Self = Self {
        r: 0.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };
    pub const BLUE: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub fn to_array(&self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    pub fn from_array(arr: [f32; 4]) -> Self {
        Self {
            r: arr[0],
            g: arr[1],
            b: arr[2],
            a: arr[3],
        }
    }
}

/// Keyboard shortcut
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(C)]
pub struct Shortcut {
    pub key: KeyCode,
    pub modifiers: Modifiers,
}

/// Key codes for shortcuts
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum KeyCode {
    #[default]
    None,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Key0,
    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Escape,
    Enter,
    Space,
    Tab,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
}

/// Modifier keys
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(C)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub command: bool, // macOS command key
}

impl Modifiers {
    pub const NONE: Self = Self {
        ctrl: false,
        shift: false,
        alt: false,
        command: false,
    };

    pub const CTRL: Self = Self {
        ctrl: true,
        shift: false,
        alt: false,
        command: false,
    };

    pub const SHIFT: Self = Self {
        ctrl: false,
        shift: true,
        alt: false,
        command: false,
    };

    pub const ALT: Self = Self {
        ctrl: false,
        shift: false,
        alt: true,
        command: false,
    };

    pub const CTRL_SHIFT: Self = Self {
        ctrl: true,
        shift: true,
        alt: false,
        command: false,
    };
}
