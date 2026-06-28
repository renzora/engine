//! Text input — a single-line editable field (click to focus, type to edit).

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::text::TextLayoutInfo;
use bevy::ui::{ComputedNode, RelativeCursorPosition};
use bevy::window::SystemCursorIcon;

use crate::font::ui_font;
use crate::style::{Role, Styled, WidgetState};
use crate::theme::*;

/// Horizontal padding inside the input box (matches the box's `padding` x), so
/// the caret/click math measures from where the text actually starts.
const PAD_X: f32 = 8.0;
/// Caret height + vertical offset (the box is `padding` y = 5 over a ~12px font).
const CARET_H: f32 = 14.0;
const CARET_TOP: f32 = 6.0;
/// Advance (px/char) used before the field has been measured, or while empty.
const FALLBACK_ADVANCE: f32 = 6.0;

/// Marks a single-line [`text_input`] (not the multi-line `textarea`, which
/// shares [`EmberTextInput`]) so the caret-positioning / measure / click systems
/// only touch single-line fields.
#[derive(Component)]
pub(crate) struct SingleLineInput;

/// Shared state for text-input-like widgets (single line + textarea). Public so
/// panels in other crates can read the typed `value` (and clear it on submit).
#[derive(Component)]
pub struct EmberTextInput {
    pub value: String,
    pub focused: bool,
    pub text_entity: Entity,
    pub placeholder: String,
    pub caret: Entity,
    /// When true, the value renders masked (`••••`) — for password fields.
    pub password: bool,
    /// When true, the whole value is "selected" (highlighted): the next typed
    /// character replaces it, and Backspace/Delete clears it. Set on a fresh
    /// focus where the caller wants select-all (e.g. inline rename), and cleared
    /// the moment the user edits.
    pub select_all: bool,
    /// Caret position as a char index into `value` (`0..=len`). Insert/Backspace/
    /// Delete and the arrow keys act here; a click places it (single-line fields).
    pub caret_index: usize,
    /// Measured average advance (logical px per char) of the current value, from
    /// the text node's [`TextLayoutInfo`] — the same probe-style measurement the
    /// code editor uses to map clicks↔columns. `FALLBACK_ADVANCE` until measured.
    pub advance: f32,
}

/// The text + color to display for an input's current value (masked for password
/// fields; the muted placeholder when empty).
fn display_for(value: &str, placeholder: &str, password: bool) -> (String, (u8, u8, u8)) {
    if value.is_empty() {
        (placeholder.to_string(), text_muted())
    } else if password {
        ("\u{2022}".repeat(value.chars().count()), text_primary())
    } else {
        (value.to_string(), text_primary())
    }
}

/// Spawn a blinking-caret bar (hidden until the input is focused).
pub(crate) fn caret(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(2.0),
                height: Val::Px(14.0),
                margin: UiRect::left(Val::Px(1.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(accent())),
            bevy::ui::FocusPolicy::Pass,
            Name::new("caret"),
        ))
        .id()
}

/// A single-line text input. Click to focus, type to edit (basic: character
/// entry + backspace; no cursor/selection yet).
pub fn text_input(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    placeholder: &str,
    value: &str,
) -> Entity {
    build_input(commands, font, placeholder, value, false)
}

/// A [`text_input`] whose value renders masked (`••••`) — for passwords.
pub fn password_input(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    placeholder: &str,
    value: &str,
) -> Entity {
    build_input(commands, font, placeholder, value, true)
}

fn build_input(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    placeholder: &str,
    value: &str,
    password: bool,
) -> Entity {
    let (disp, col) = display_for(value, placeholder, password);
    let box_e = commands
        .spawn((
            Node {
                min_width: Val::Px(180.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            // Needed to map a click's x to a caret index (code-editor technique).
            RelativeCursorPosition::default(),
            SingleLineInput,
            Styled::new(Role::Input),
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Text),
            Name::new("text-input"),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(disp),
            ui_font(font, 12.0),
            TextColor(rgb(col)),
            // Selection highlight (drawn behind the glyphs when `select_all`).
            BackgroundColor(Color::NONE),
        ))
        .id();
    // Absolute caret positioned by `text_input_caret_pos` at `PAD_X + idx*advance`
    // (out of flow, so it floats over the text without affecting its layout).
    let car = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(PAD_X),
                top: Val::Px(CARET_TOP),
                width: Val::Px(2.0),
                height: Val::Px(CARET_H),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::WHITE),
            bevy::ui::FocusPolicy::Pass,
            Name::new("caret"),
        ))
        .id();
    commands.entity(box_e).insert(EmberTextInput {
        value: value.to_string(),
        focused: false,
        text_entity: text,
        placeholder: placeholder.to_string(),
        caret: car,
        password,
        select_all: false,
        caret_index: value.chars().count(),
        advance: FALLBACK_ADVANCE,
    });
    commands.entity(box_e).add_children(&[text, car]);
    box_e
}

/// Two-way bind a [`text_input`] to a `String` piece of state. While the input
/// is focused, the user's edits flow to state; while unfocused, external changes
/// flow back into the input (without clobbering typing).
pub fn bind_text_input(
    commands: &mut Commands,
    input: Entity,
    get: impl Fn(&World) -> String + Send + Sync + 'static,
    set: impl Fn(&mut World, String) + Send + Sync + 'static,
) {
    crate::reactive::react(commands, move |world: &mut World| {
        if world.get_entity(input).is_err() {
            return false;
        }
        let Some((focused, widget_val, text_e, ph, password)) = world
            .get::<EmberTextInput>(input)
            .map(|i| (i.focused, i.value.clone(), i.text_entity, i.placeholder.clone(), i.password))
        else {
            return true;
        };
        let state_val = get(world);
        if focused {
            // User is editing → push to state.
            if widget_val != state_val {
                set(world, widget_val);
            }
        } else if widget_val != state_val {
            // External change → reflect into the input + its displayed text.
            if let Some(mut i) = world.get_mut::<EmberTextInput>(input) {
                i.value = state_val.clone();
                // Keep the caret within the new value.
                let n = state_val.chars().count();
                if i.caret_index > n {
                    i.caret_index = n;
                }
            }
            let (disp, col) = display_for(&state_val, &ph, password);
            if let Some(mut t) = world.get_mut::<Text>(text_e) {
                t.0 = disp;
            }
            if let Some(mut c) = world.get_mut::<TextColor>(text_e) {
                c.0 = rgb(col);
            }
        }
        true
    });
}

/// Max gap (seconds) between two presses on the same input to count as a
/// double-click (which selects the whole value).
const DOUBLE_CLICK_SECS: f32 = 0.4;

#[allow(clippy::type_complexity)]
pub(crate) fn text_input_focus(
    mouse: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    mut inputs: Query<(
        Entity,
        &Interaction,
        &mut EmberTextInput,
        &mut Styled,
        &ComputedNode,
        Option<&RelativeCursorPosition>,
    )>,
    // Last press: which input it landed on (if any) + when, for double-click detection.
    mut last_click: Local<Option<(Entity, f32)>>,
) {
    // Only react to the press itself. The input under the press (if any) takes
    // focus; a press anywhere else — empty space or another widget — blurs every
    // input (off-click to dismiss).
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let clicked = inputs
        .iter()
        .find(|(_, i, _, _, _, _)| matches!(i, Interaction::Pressed))
        .map(|(e, _, _, _, _, _)| e);

    // A second press on the same input within the threshold → double-click.
    let now = time.elapsed_secs();
    let double_click = matches!(
        (clicked, *last_click),
        (Some(c), Some((last_e, last_t))) if c == last_e && now - last_t < DOUBLE_CLICK_SECS
    );
    *last_click = clicked.map(|e| (e, now));

    for (e, _, mut inp, mut styled, cn, rcp) in &mut inputs {
        let focus = Some(e) == clicked;
        if inp.focused != focus {
            inp.focused = focus;
            styled.state = if focus {
                WidgetState::Active
            } else {
                WidgetState::Normal
            };
        }
        if !focus {
            continue;
        }
        if double_click && !inp.value.is_empty() {
            // Double-click selects the whole value (next type/Backspace wipes it).
            inp.select_all = true;
            continue;
        }
        // Single click → drop any selection and drop the caret where it landed.
        inp.select_all = false;
        let len = inp.value.chars().count();
        if let Some(nrm) = rcp.and_then(|r| r.normalized) {
            // Click x in the box's local logical space → char index via the
            // measured advance (the code editor's column hit-test, no gutter).
            let width = cn.size().x * cn.inverse_scale_factor();
            let local_x = (nrm.x + 0.5) * width;
            let adv = inp.advance.max(0.1);
            let idx = ((local_x - PAD_X) / adv).round().max(0.0) as usize;
            inp.caret_index = idx.min(len);
        } else {
            inp.caret_index = len;
        }
    }
}

/// Byte offset of char index `i` in `s` (`s.len()` when `i` is at/after the end).
fn byte_offset(s: &str, i: usize) -> usize {
    s.char_indices().nth(i).map(|(b, _)| b).unwrap_or(s.len())
}

pub(crate) fn text_input_type(
    mut events: MessageReader<KeyboardInput>,
    mut inputs: Query<(&mut EmberTextInput, Option<&SingleLineInput>)>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
) {
    for ev in events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        for (mut inp, single) in &mut inputs {
            if !inp.focused {
                continue;
            }
            let selected = inp.select_all;
            if single.is_some() {
                // Single-line: edits + caret moves act at `caret_index`.
                let len = inp.value.chars().count();
                let mut idx = inp.caret_index.min(len);
                match &ev.logical_key {
                    Key::Character(s) => {
                        if selected {
                            inp.value.clear();
                            inp.select_all = false;
                            idx = 0;
                        }
                        let b = byte_offset(&inp.value, idx);
                        inp.value.insert_str(b, s);
                        idx += s.chars().count();
                    }
                    Key::Space => {
                        if selected {
                            inp.value.clear();
                            inp.select_all = false;
                            idx = 0;
                        }
                        let b = byte_offset(&inp.value, idx);
                        inp.value.insert(b, ' ');
                        idx += 1;
                    }
                    Key::Backspace => {
                        if selected {
                            inp.value.clear();
                            inp.select_all = false;
                            idx = 0;
                        } else if idx > 0 {
                            let (start, end) =
                                (byte_offset(&inp.value, idx - 1), byte_offset(&inp.value, idx));
                            inp.value.replace_range(start..end, "");
                            idx -= 1;
                        }
                    }
                    Key::Delete => {
                        if selected {
                            inp.value.clear();
                            inp.select_all = false;
                            idx = 0;
                        } else if idx < len {
                            let (start, end) =
                                (byte_offset(&inp.value, idx), byte_offset(&inp.value, idx + 1));
                            inp.value.replace_range(start..end, "");
                        }
                    }
                    Key::ArrowLeft => {
                        inp.select_all = false;
                        idx = idx.saturating_sub(1);
                    }
                    Key::ArrowRight => {
                        inp.select_all = false;
                        idx = (idx + 1).min(len);
                    }
                    Key::Home => {
                        inp.select_all = false;
                        idx = 0;
                    }
                    Key::End => {
                        inp.select_all = false;
                        idx = len;
                    }
                    // Enter is a submit/commit gesture other systems own; never a
                    // literal newline in a single-line field.
                    _ => continue,
                }
                inp.caret_index = idx;
            } else {
                // Multi-line textarea: append/pop at the end (its pre-caret behavior).
                match &ev.logical_key {
                    Key::Character(s) => {
                        if selected {
                            inp.value.clear();
                            inp.select_all = false;
                        }
                        inp.value.push_str(s);
                    }
                    Key::Space => {
                        if selected {
                            inp.value.clear();
                            inp.select_all = false;
                        }
                        inp.value.push(' ');
                    }
                    Key::Enter => inp.value.push('\n'),
                    Key::Backspace | Key::Delete => {
                        if selected {
                            inp.value.clear();
                            inp.select_all = false;
                        } else if matches!(ev.logical_key, Key::Backspace) {
                            inp.value.pop();
                        }
                    }
                    _ => continue,
                }
            }
            let (text_e, val, ph, pw) = (inp.text_entity, inp.value.clone(), inp.placeholder.clone(), inp.password);
            if let Ok((mut t, mut c)) = texts.get_mut(text_e) {
                let (disp, col) = display_for(&val, &ph, pw);
                *t = Text::new(disp);
                c.0 = rgb(col);
            }
            break;
        }
    }
}

/// Measure the field's average advance (px/char) from its text node's laid-out
/// width — the code editor's probe technique, here on the live text. Drives the
/// caret position + click hit-testing. Skips empty fields (placeholder ≠ value).
pub(crate) fn text_input_measure(
    mut inputs: Query<&mut EmberTextInput, With<SingleLineInput>>,
    layouts: Query<&TextLayoutInfo>,
) {
    for mut inp in &mut inputs {
        let n = inp.value.chars().count();
        if n == 0 {
            continue;
        }
        let Ok(info) = layouts.get(inp.text_entity) else {
            continue;
        };
        let sf = if info.scale_factor > 0.0 { info.scale_factor } else { 1.0 };
        let adv = (info.size.x / sf) / n as f32;
        if adv > 0.5 && (adv - inp.advance).abs() > 0.01 {
            inp.advance = adv;
        }
    }
}

/// Position the absolute caret at `PAD_X + caret_index * advance`.
pub(crate) fn text_input_caret_pos(
    inputs: Query<&EmberTextInput, With<SingleLineInput>>,
    mut nodes: Query<&mut Node>,
) {
    for inp in &inputs {
        if let Ok(mut n) = nodes.get_mut(inp.caret) {
            let left = Val::Px(PAD_X + inp.caret_index as f32 * inp.advance);
            if n.left != left {
                n.left = left;
            }
        }
    }
}

/// Tint the value's background while it's fully selected (`select_all`), so the
/// user sees the whole field highlighted — mirrors an OS text field's select-all.
pub(crate) fn text_input_highlight(
    inputs: Query<&EmberTextInput>,
    mut backgrounds: Query<&mut BackgroundColor>,
) {
    for inp in &inputs {
        let Ok(mut bg) = backgrounds.get_mut(inp.text_entity) else {
            continue;
        };
        let target = if inp.select_all && inp.focused && !inp.value.is_empty() {
            rgb(accent()).with_alpha(0.45)
        } else {
            Color::NONE
        };
        if bg.0 != target {
            bg.0 = target;
        }
    }
}

pub(crate) fn caret_blink(
    time: Res<Time>,
    inputs: Query<&EmberTextInput>,
    mut nodes: Query<&mut Node>,
) {
    let on = (time.elapsed_secs() * 1.6).fract() < 0.5;
    for inp in &inputs {
        if let Ok(mut n) = nodes.get_mut(inp.caret) {
            let display = if inp.focused && on {
                Display::Flex
            } else {
                Display::None
            };
            if n.display != display {
                n.display = display;
            }
        }
    }
}
