//! One frame of input, flattened into something a plugin can read across the
//! boundary.
//!
//! Bevy's input is `ButtonInput<KeyCode>`, `ButtonInput<MouseButton>` and a window
//! query. None of that shape crosses a C ABI: `ButtonInput<T>` is generic, so
//! there is nothing to instantiate on the other side, and a plugin has no
//! `KeyCode` because it never linked Bevy.
//!
//! So the host flattens it once per frame into [`sys::InputState`] — three
//! 256-bit keyboard sets, three mouse bitmasks and the cursor — and sends a
//! pointer to it with every system call. A plugin asking "is W down" does a shift
//! and a mask in its own address space; nothing crosses the boundary per question.
//!
//! The alternative was `is_pressed(key)` as a host function. A movement system asks
//! about four keys and a button, so that is five FFI calls per system per frame to
//! answer questions the host already holds in a bitset.
//!
//! ## Why `sys::Key` is not `KeyCode`
//!
//! `KeyCode` is `#[non_exhaustive]` and its discriminants are an implementation
//! detail. If the wire format used them, a Bevy upgrade that inserted a variant
//! would silently remap every plugin's key handling — W becomes E, and nothing
//! fails to compile. [`sys::Key`]'s values are frozen, and the cost of a Bevy
//! change lands in [`map_key`] instead of in everybody's plugin.

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::sys;

/// This frame's input, in the form the boundary carries.
///
/// A resource rather than something built per call: several plugin systems run per
/// frame and they all read the same snapshot, so building it once in `PreUpdate` is
/// both cheaper and consistent — two systems in the same frame cannot disagree
/// about whether a key was just pressed.
#[derive(Resource, Default)]
pub struct PluginInput(pub sys::InputState);

/// Bevy `KeyCode` to the frozen wire value, or `None` for a key the ABI has no
/// number for.
///
/// The unmapped ones are deliberate rather than forgotten: numpad, media keys, IME
/// and the long tail of international keys have no consts yet. Returning `None`
/// drops them, which is correct — the alternative is inventing a number now and
/// being unable to change it later.
fn map_key(code: KeyCode) -> Option<sys::Key> {
    use sys::Key as K;
    Some(match code {
        KeyCode::KeyA => K::A,
        KeyCode::KeyB => K::B,
        KeyCode::KeyC => K::C,
        KeyCode::KeyD => K::D,
        KeyCode::KeyE => K::E,
        KeyCode::KeyF => K::F,
        KeyCode::KeyG => K::G,
        KeyCode::KeyH => K::H,
        KeyCode::KeyI => K::I,
        KeyCode::KeyJ => K::J,
        KeyCode::KeyK => K::K,
        KeyCode::KeyL => K::L,
        KeyCode::KeyM => K::M,
        KeyCode::KeyN => K::N,
        KeyCode::KeyO => K::O,
        KeyCode::KeyP => K::P,
        KeyCode::KeyQ => K::Q,
        KeyCode::KeyR => K::R,
        KeyCode::KeyS => K::S,
        KeyCode::KeyT => K::T,
        KeyCode::KeyU => K::U,
        KeyCode::KeyV => K::V,
        KeyCode::KeyW => K::W,
        KeyCode::KeyX => K::X,
        KeyCode::KeyY => K::Y,
        KeyCode::KeyZ => K::Z,

        KeyCode::Digit0 => K::Digit0,
        KeyCode::Digit1 => K::Digit1,
        KeyCode::Digit2 => K::Digit2,
        KeyCode::Digit3 => K::Digit3,
        KeyCode::Digit4 => K::Digit4,
        KeyCode::Digit5 => K::Digit5,
        KeyCode::Digit6 => K::Digit6,
        KeyCode::Digit7 => K::Digit7,
        KeyCode::Digit8 => K::Digit8,
        KeyCode::Digit9 => K::Digit9,

        KeyCode::Space => K::Space,
        KeyCode::Enter => K::Enter,
        KeyCode::Escape => K::Escape,
        KeyCode::Tab => K::Tab,
        KeyCode::Backspace => K::Backspace,
        KeyCode::Delete => K::Delete,
        KeyCode::Insert => K::Insert,
        KeyCode::Home => K::Home,
        KeyCode::End => K::End,
        KeyCode::PageUp => K::PageUp,
        KeyCode::PageDown => K::PageDown,
        KeyCode::ArrowUp => K::ArrowUp,
        KeyCode::ArrowDown => K::ArrowDown,
        KeyCode::ArrowLeft => K::ArrowLeft,
        KeyCode::ArrowRight => K::ArrowRight,

        KeyCode::ShiftLeft => K::ShiftLeft,
        KeyCode::ShiftRight => K::ShiftRight,
        KeyCode::ControlLeft => K::ControlLeft,
        KeyCode::ControlRight => K::ControlRight,
        KeyCode::AltLeft => K::AltLeft,
        KeyCode::AltRight => K::AltRight,
        KeyCode::SuperLeft => K::SuperLeft,
        KeyCode::SuperRight => K::SuperRight,

        KeyCode::F1 => K::F1,
        KeyCode::F2 => K::F2,
        KeyCode::F3 => K::F3,
        KeyCode::F4 => K::F4,
        KeyCode::F5 => K::F5,
        KeyCode::F6 => K::F6,
        KeyCode::F7 => K::F7,
        KeyCode::F8 => K::F8,
        KeyCode::F9 => K::F9,
        KeyCode::F10 => K::F10,
        KeyCode::F11 => K::F11,
        KeyCode::F12 => K::F12,

        KeyCode::Minus => K::Minus,
        KeyCode::Equal => K::Equal,
        KeyCode::BracketLeft => K::BracketLeft,
        KeyCode::BracketRight => K::BracketRight,
        KeyCode::Backslash => K::Backslash,
        KeyCode::Semicolon => K::Semicolon,
        KeyCode::Quote => K::Quote,
        KeyCode::Comma => K::Comma,
        KeyCode::Period => K::Period,
        KeyCode::Slash => K::Slash,
        KeyCode::Backquote => K::Backquote,
        KeyCode::CapsLock => K::CapsLock,

        _ => return None,
    })
}

fn map_mouse(button: MouseButton) -> Option<sys::MouseButton> {
    Some(match button {
        MouseButton::Left => sys::MouseButton::Left,
        MouseButton::Right => sys::MouseButton::Right,
        MouseButton::Middle => sys::MouseButton::Middle,
        MouseButton::Back => sys::MouseButton::Back,
        MouseButton::Forward => sys::MouseButton::Forward,
        // `MouseButton::Other(u16)` — no wire number, and inventing one would
        // collide with whatever the named buttons grow into.
        _ => return None,
    })
}

/// Flatten Bevy's input into the wire snapshot.
///
/// Every parameter is optional so this survives a host with no input plugins at
/// all — a dedicated server builds no `ButtonInput` and no window, and a plugin
/// there simply sees nothing pressed rather than the system failing to run.
pub fn collect_input(
    mut out: ResMut<PluginInput>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    buttons: Option<Res<ButtonInput<MouseButton>>>,
    motion: Option<Res<AccumulatedMouseMotion>>,
    scroll: Option<Res<AccumulatedMouseScroll>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    // Rebuilt from scratch each frame rather than mutated: `just_pressed` is only
    // true for one frame, and clearing bits individually is how a stale one
    // survives into the next frame.
    let mut state = sys::InputState::default();

    if let Some(keys) = keys {
        for code in keys.get_pressed() {
            if let Some(k) = map_key(*code) {
                sys::InputState::set_key(&mut state.keys_down, k);
            }
        }
        for code in keys.get_just_pressed() {
            if let Some(k) = map_key(*code) {
                sys::InputState::set_key(&mut state.keys_just_pressed, k);
            }
        }
        for code in keys.get_just_released() {
            if let Some(k) = map_key(*code) {
                sys::InputState::set_key(&mut state.keys_just_released, k);
            }
        }
    }

    if let Some(buttons) = buttons {
        for b in buttons.get_pressed() {
            if let Some(m) = map_mouse(*b) {
                state.mouse_down |= 1 << m.0;
            }
        }
        for b in buttons.get_just_pressed() {
            if let Some(m) = map_mouse(*b) {
                state.mouse_just_pressed |= 1 << m.0;
            }
        }
        for b in buttons.get_just_released() {
            if let Some(m) = map_mouse(*b) {
                state.mouse_just_released |= 1 << m.0;
            }
        }
    }

    if let Some(motion) = motion {
        state.cursor_delta_x = motion.delta.x;
        state.cursor_delta_y = motion.delta.y;
    }
    if let Some(scroll) = scroll {
        state.scroll_x = scroll.delta.x;
        state.scroll_y = scroll.delta.y;
    }

    // NaN for "no cursor", so a plugin that forgets to check gets an obviously
    // wrong number rather than a plausible `0, 0` in the window corner.
    let cursor = windows.iter().next().and_then(|w| w.cursor_position());
    let (x, y) = cursor.map_or((f32::NAN, f32::NAN), |p| (p.x, p.y));
    state.cursor_x = x;
    state.cursor_y = y;

    out.0 = state;
}
