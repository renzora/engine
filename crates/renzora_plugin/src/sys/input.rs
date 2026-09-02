//! This frame's keyboard, mouse and cursor, as a snapshot.
//!
//! A whole snapshot rather than `is_pressed(key)` calls because input is read in
//! bursts: a movement system asks about four keys and a mouse button, and five
//! FFI calls per frame per system to answer questions the host already has in a
//! bitset is the wrong trade.
//!
//! [`Key`]'s numbering **is** the bit index into [`InputState::keys_down`], so
//! nothing here may be renumbered — a contract no layout test can see.

/// One frame of input, as the host saw it.
///
/// Bitsets rather than arrays of bools: 256 keys fit in 32 bytes, the whole struct
/// copies in one go, and "is this key down" is a shift and a mask on the plugin's
/// own side of the boundary.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct InputState {
    /// Currently held, indexed by [`Key`].
    pub keys_down: [u64; 4],
    /// Went down THIS frame.
    pub keys_just_pressed: [u64; 4],
    /// Came up this frame.
    pub keys_just_released: [u64; 4],
    /// Currently held, indexed by [`MouseButton`].
    pub mouse_down: u32,
    pub mouse_just_pressed: u32,
    pub mouse_just_released: u32,
    /// Cursor position in the primary window, in logical pixels, with the origin
    /// top-left. `NaN` when the cursor is outside the window — a plugin that
    /// forgets to check gets an obviously wrong number rather than a plausible
    /// `0, 0` in the corner.
    pub cursor_x: f32,
    pub cursor_y: f32,
    /// Movement since the previous frame. Zero when the cursor did not move, and
    /// still meaningful while the cursor is locked, which is why it is separate
    /// from the position rather than derived from it.
    pub cursor_delta_x: f32,
    pub cursor_delta_y: f32,
    /// Wheel movement this frame, in lines.
    pub scroll_x: f32,
    pub scroll_y: f32,
}

/// A keyboard key, as a stable number.
///
/// Deliberately NOT a copy of `bevy::KeyCode`'s discriminants. That enum is
/// `#[non_exhaustive]` and its values are an implementation detail, so a Bevy
/// upgrade that inserted a variant would silently remap every plugin's key
/// handling. These values are frozen here and the host maps `KeyCode` onto them,
/// which puts the cost of a Bevy change in one match statement instead of in
/// everybody's plugin.
///
/// Append-only, like every other newtype in this file. The value IS the bit index
/// into [`InputState::keys_down`], so nothing may be renumbered.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Key(pub u16);

#[allow(non_upper_case_globals)]
impl Key {
    // Letters, 0..25.
    pub const A: Self = Self(0);
    pub const B: Self = Self(1);
    pub const C: Self = Self(2);
    pub const D: Self = Self(3);
    pub const E: Self = Self(4);
    pub const F: Self = Self(5);
    pub const G: Self = Self(6);
    pub const H: Self = Self(7);
    pub const I: Self = Self(8);
    pub const J: Self = Self(9);
    pub const K: Self = Self(10);
    pub const L: Self = Self(11);
    pub const M: Self = Self(12);
    pub const N: Self = Self(13);
    pub const O: Self = Self(14);
    pub const P: Self = Self(15);
    pub const Q: Self = Self(16);
    pub const R: Self = Self(17);
    pub const S: Self = Self(18);
    pub const T: Self = Self(19);
    pub const U: Self = Self(20);
    pub const V: Self = Self(21);
    pub const W: Self = Self(22);
    pub const X: Self = Self(23);
    pub const Y: Self = Self(24);
    pub const Z: Self = Self(25);

    // Digit row, 26..35.
    pub const Digit0: Self = Self(26);
    pub const Digit1: Self = Self(27);
    pub const Digit2: Self = Self(28);
    pub const Digit3: Self = Self(29);
    pub const Digit4: Self = Self(30);
    pub const Digit5: Self = Self(31);
    pub const Digit6: Self = Self(32);
    pub const Digit7: Self = Self(33);
    pub const Digit8: Self = Self(34);
    pub const Digit9: Self = Self(35);

    // Editing and navigation, 36..51.
    pub const Space: Self = Self(36);
    pub const Enter: Self = Self(37);
    pub const Escape: Self = Self(38);
    pub const Tab: Self = Self(39);
    pub const Backspace: Self = Self(40);
    pub const Delete: Self = Self(41);
    pub const Insert: Self = Self(42);
    pub const Home: Self = Self(43);
    pub const End: Self = Self(44);
    pub const PageUp: Self = Self(45);
    pub const PageDown: Self = Self(46);
    pub const ArrowUp: Self = Self(47);
    pub const ArrowDown: Self = Self(48);
    pub const ArrowLeft: Self = Self(49);
    pub const ArrowRight: Self = Self(50);

    // Modifiers, 52..59. Left and right are distinct, because a game that binds
    // one of them cannot express that if they are merged.
    pub const ShiftLeft: Self = Self(52);
    pub const ShiftRight: Self = Self(53);
    pub const ControlLeft: Self = Self(54);
    pub const ControlRight: Self = Self(55);
    pub const AltLeft: Self = Self(56);
    pub const AltRight: Self = Self(57);
    pub const SuperLeft: Self = Self(58);
    pub const SuperRight: Self = Self(59);

    // Function row, 60..71.
    pub const F1: Self = Self(60);
    pub const F2: Self = Self(61);
    pub const F3: Self = Self(62);
    pub const F4: Self = Self(63);
    pub const F5: Self = Self(64);
    pub const F6: Self = Self(65);
    pub const F7: Self = Self(66);
    pub const F8: Self = Self(67);
    pub const F9: Self = Self(68);
    pub const F10: Self = Self(69);
    pub const F11: Self = Self(70);
    pub const F12: Self = Self(71);

    // Punctuation and numpad basics, 72..83.
    pub const Minus: Self = Self(72);
    pub const Equal: Self = Self(73);
    pub const BracketLeft: Self = Self(74);
    pub const BracketRight: Self = Self(75);
    pub const Backslash: Self = Self(76);
    pub const Semicolon: Self = Self(77);
    pub const Quote: Self = Self(78);
    pub const Comma: Self = Self(79);
    pub const Period: Self = Self(80);
    pub const Slash: Self = Self(81);
    pub const Backquote: Self = Self(82);
    pub const CapsLock: Self = Self(83);

    /// One past the highest assigned value. The bitset is `[u64; 4]` = 256 bits,
    /// so there is room to append without changing the wire format.
    pub const COUNT: u16 = 84;

    /// Whether this build knows the key. A value from a newer ABI is out of range
    /// of the bitset, and testing it would read a bit belonging to another key.
    pub const fn is_known(self) -> bool {
        self.0 < Self::COUNT
    }
}

/// A mouse button, indexed into [`InputState::mouse_down`].
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct MouseButton(pub u32);

#[allow(non_upper_case_globals)]
impl MouseButton {
    pub const Left: Self = Self(0);
    pub const Right: Self = Self(1);
    pub const Middle: Self = Self(2);
    pub const Back: Self = Self(3);
    pub const Forward: Self = Self(4);

    pub const COUNT: u32 = 5;

    pub const fn is_known(self) -> bool {
        self.0 < Self::COUNT
    }
}

impl InputState {
    /// Test a bit in one of the keyboard sets.
    ///
    /// Out-of-range keys read `false` rather than wrapping into another key's bit,
    /// which is what an unchecked `1 << (k % 64)` would do with a value from a
    /// newer ABI.
    #[inline]
    fn key_bit(set: &[u64; 4], key: Key) -> bool {
        if !key.is_known() {
            return false;
        }
        let i = key.0 as usize;
        set[i / 64] & (1u64 << (i % 64)) != 0
    }

    #[inline]
    pub fn pressed(&self, key: Key) -> bool {
        Self::key_bit(&self.keys_down, key)
    }
    #[inline]
    pub fn just_pressed(&self, key: Key) -> bool {
        Self::key_bit(&self.keys_just_pressed, key)
    }
    #[inline]
    pub fn just_released(&self, key: Key) -> bool {
        Self::key_bit(&self.keys_just_released, key)
    }

    #[inline]
    pub fn mouse_pressed(&self, button: MouseButton) -> bool {
        button.is_known() && self.mouse_down & (1 << button.0) != 0
    }
    #[inline]
    pub fn mouse_just_pressed(&self, button: MouseButton) -> bool {
        button.is_known() && self.mouse_just_pressed & (1 << button.0) != 0
    }
    #[inline]
    pub fn mouse_just_released(&self, button: MouseButton) -> bool {
        button.is_known() && self.mouse_just_released & (1 << button.0) != 0
    }

    /// Set a key bit. Host-side helper; ignores anything out of range.
    pub fn set_key(set: &mut [u64; 4], key: Key) {
        if key.is_known() {
            let i = key.0 as usize;
            set[i / 64] |= 1u64 << (i % 64);
        }
    }
}
