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
    /// Only a fallback: exact positions come from `offsets` once measured.
    pub advance: f32,
    /// Exact caret x offsets (logical px from the text's left edge), one per
    /// caret slot `0..=chars`, read from the parley layout's per-cluster
    /// advances. An average advance is wrong for proportional fonts (the caret
    /// lands mid-glyph and click rounding skips slots); these are the real
    /// glyph boundaries. Empty until measured — fall back to `advance`.
    pub offsets: Vec<f32>,
    /// Anchor (char index) of a drag selection: the selection spans from here to
    /// `caret_index` (either direction). `None`, or anchor == caret, means no
    /// selection. Set on press by the drag-select system; collapsed by caret
    /// motion and consumed (deleted/replaced) by edits. Single-line fields only.
    pub sel_anchor: Option<usize>,
}

impl EmberTextInput {
    /// The normalized `(start, end)` char range of the drag selection, or
    /// `None` when nothing is selected. `start < end` always.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let a = self.sel_anchor?;
        let b = self.caret_index;
        if a == b {
            return None;
        }
        Some((a.min(b), a.max(b)))
    }

    /// Caret x (logical px from the text's left edge) for char boundary `idx`.
    /// Uses the measured per-glyph `offsets` when they cover the current value,
    /// else estimates from the average advance (pre-measure / layout lag).
    pub fn caret_x(&self, idx: usize) -> f32 {
        self.offsets
            .get(idx)
            .copied()
            .unwrap_or(idx as f32 * self.advance)
    }
}

/// The chars of `s` in the char-index range `[a, b)`.
fn char_slice(s: &str, a: usize, b: usize) -> String {
    let (start, end) = (byte_offset(s, a), byte_offset(s, b));
    s[start..end].to_string()
}

/// Marks the drag-selection highlight bar; points at the input box it belongs
/// to (spawned behind the text so glyphs render over it).
#[derive(Component)]
pub(crate) struct SelectionHighlight(Entity);

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
                width: Val::Px(1.0),
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
                // Single-line input: clip overflowing text instead of letting it
                // wrap the box taller (the caret math assumes one line).
                overflow: Overflow::clip(),
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
            bevy::text::TextLayout::no_wrap(),
            // Selection highlight (drawn behind the glyphs when `select_all`).
            BackgroundColor(Color::NONE),
        ))
        .id();
    // Drag-selection highlight, positioned/sized by `text_input_selection_pos`.
    // Spawned before the text child so the glyphs draw over it.
    let sel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(PAD_X),
                top: Val::Px(CARET_TOP),
                width: Val::Px(0.0),
                height: Val::Px(CARET_H),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(accent()).with_alpha(0.45)),
            bevy::ui::FocusPolicy::Pass,
            SelectionHighlight(box_e),
            Name::new("selection"),
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
                width: Val::Px(1.0),
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
        offsets: Vec::new(),
        sel_anchor: None,
    });
    commands.entity(box_e).add_children(&[sel, text, car]);
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
            if inp.sel_anchor.is_some() {
                inp.sel_anchor = None;
            }
            continue;
        }
        if double_click && !inp.value.is_empty() {
            // Double-click selects the whole value (next type/Backspace wipes it).
            inp.select_all = true;
            inp.sel_anchor = None;
            continue;
        }
        // Single click → drop any selection and drop the caret where it landed.
        inp.select_all = false;
        inp.sel_anchor = None;
        if let Some(nrm) = rcp.and_then(|r| r.normalized) {
            // Click x in the box's local logical space → char index via the
            // measured advance (the code editor's column hit-test, no gutter).
            inp.caret_index = index_at_cursor(&inp, cn, nrm);
        } else {
            inp.caret_index = inp.value.chars().count();
        }
    }
}

/// Byte offset of char index `i` in `s` (`s.len()` when `i` is at/after the end).
fn byte_offset(s: &str, i: usize) -> usize {
    s.char_indices().nth(i).map(|(b, _)| b).unwrap_or(s.len())
}

/// Delete whatever is selected (select-all or a drag range) ahead of an edit
/// key acting, leaving `idx` where the removed span began. No-op when nothing
/// is selected. `range` is the pre-computed [`EmberTextInput::selection_range`].
fn wipe_selection(inp: &mut EmberTextInput, range: Option<(usize, usize)>, idx: &mut usize) {
    if inp.select_all {
        inp.value.clear();
        inp.select_all = false;
        *idx = 0;
    } else if let Some((a, b)) = range {
        let (s, e) = (byte_offset(&inp.value, a), byte_offset(&inp.value, b));
        inp.value.replace_range(s..e, "");
        *idx = a;
    }
    inp.sel_anchor = None;
}

/// Char index under the cursor, from the box's [`RelativeCursorPosition`] and
/// the measured advance — the same hit-test `text_input_focus` uses for clicks.
/// Clamped to `0..=len`, including while the cursor is dragged outside the box
/// (normalized keeps extrapolating, which is what lets a drag select to the
/// ends).
fn index_at_cursor(inp: &EmberTextInput, cn: &ComputedNode, nrm: Vec2) -> usize {
    let width = cn.size().x * cn.inverse_scale_factor();
    let local_x = (nrm.x + 0.5) * width - PAD_X;
    let n = inp.value.chars().count();
    if inp.offsets.len() == n + 1 {
        // Nearest measured glyph boundary — exact for proportional fonts.
        inp.offsets
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (local_x - **a).abs().total_cmp(&(local_x - **b).abs())
            })
            .map(|(i, _)| i)
            .unwrap_or(n)
    } else {
        let adv = inp.advance.max(0.1);
        let idx = (local_x / adv).round().max(0.0) as usize;
        idx.min(n)
    }
}

/// Drag selection: press in an input anchors a selection at the caret, moving
/// the mouse while held sweeps `caret_index` (so the selection spans anchor →
/// cursor), release collapses an empty selection back to a plain caret.
///
/// Runs after [`text_input_focus`] so the press frame anchors at the caret
/// that click just placed (focus also cleared any previous selection).
#[allow(clippy::type_complexity)]
pub(crate) fn text_input_drag_select(
    mouse: Res<ButtonInput<MouseButton>>,
    mut inputs: Query<
        (
            Entity,
            &Interaction,
            &mut EmberTextInput,
            &ComputedNode,
            &RelativeCursorPosition,
        ),
        With<SingleLineInput>,
    >,
    // The input a left-drag is currently selecting in, if any.
    mut dragging: Local<Option<Entity>>,
) {
    if mouse.just_pressed(MouseButton::Left) {
        *dragging = inputs
            .iter()
            // A double-click just select-all'd — don't fight it with a drag.
            .find(|(_, i, inp, _, _)| {
                matches!(i, Interaction::Pressed) && inp.focused && !inp.select_all
            })
            .map(|(e, _, _, _, _)| e);
        if let Some(e) = *dragging {
            if let Ok((_, _, mut inp, _, _)) = inputs.get_mut(e) {
                let at = inp.caret_index;
                inp.sel_anchor = Some(at);
            }
        }
        return;
    }
    let Some(e) = *dragging else { return };
    let Ok((_, _, mut inp, cn, rcp)) = inputs.get_mut(e) else {
        *dragging = None;
        return;
    };
    if !mouse.pressed(MouseButton::Left) {
        // Release: a zero-width selection is just a caret.
        if inp.selection_range().is_none() && inp.sel_anchor.is_some() {
            inp.sel_anchor = None;
        }
        *dragging = None;
        return;
    }
    if let Some(nrm) = rcp.normalized {
        let idx = index_at_cursor(&inp, cn, nrm);
        if idx != inp.caret_index {
            inp.caret_index = idx;
        }
    }
}

/// Show/size the selection highlight bar over the selected span (focused
/// single-line fields only); hidden otherwise.
pub(crate) fn text_input_selection_pos(
    inputs: Query<&EmberTextInput, With<SingleLineInput>>,
    mut bars: Query<(&SelectionHighlight, &mut Node)>,
) {
    for (bar, mut n) in &mut bars {
        let Ok(inp) = inputs.get(bar.0) else { continue };
        match (inp.focused, inp.selection_range()) {
            (true, Some((a, b))) => {
                let (xa, xb) = (inp.caret_x(a), inp.caret_x(b));
                let left = Val::Px(PAD_X + xa);
                let width = Val::Px(xb - xa);
                if n.display != Display::Flex || n.left != left || n.width != width {
                    n.display = Display::Flex;
                    n.left = left;
                    n.width = width;
                }
            }
            _ => {
                if n.display != Display::None {
                    n.display = Display::None;
                }
            }
        }
    }
}

pub(crate) fn text_input_type(
    mut events: MessageReader<KeyboardInput>,
    keys: Res<ButtonInput<KeyCode>>,
    mut inputs: Query<(&mut EmberTextInput, Option<&SingleLineInput>)>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft)
        || keys.pressed(KeyCode::ControlRight)
        || keys.pressed(KeyCode::SuperLeft)
        || keys.pressed(KeyCode::SuperRight);
    for ev in events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        for (mut inp, single) in &mut inputs {
            if !inp.focused {
                continue;
            }
            // Clipboard shortcuts (Ctrl/Cmd + V/C/X/A). Handled up front so
            // the plain character branch never types a literal "v".
            if ctrl {
                match ev.key_code {
                    KeyCode::KeyV => {
                        if let Some(pasted) = clipboard_get() {
                            let pasted = pasted.replace('\r', "");
                            if inp.select_all {
                                inp.value.clear();
                                inp.select_all = false;
                                inp.caret_index = 0;
                            } else if let Some((a, b)) = inp.selection_range() {
                                // Paste over a drag selection: replace it.
                                let (s, e) = (byte_offset(&inp.value, a), byte_offset(&inp.value, b));
                                inp.value.replace_range(s..e, "");
                                inp.caret_index = a;
                            }
                            inp.sel_anchor = None;
                            if single.is_some() {
                                // Single-line: newlines become spaces, insert at caret.
                                let pasted = pasted.replace('\n', " ");
                                let len = inp.value.chars().count();
                                let idx = inp.caret_index.min(len);
                                let b = byte_offset(&inp.value, idx);
                                inp.value.insert_str(b, &pasted);
                                inp.caret_index = idx + pasted.chars().count();
                            } else {
                                inp.value.push_str(&pasted);
                            }
                        }
                    }
                    KeyCode::KeyC => {
                        // Copy the drag selection when there is one, else the whole value.
                        match inp.selection_range() {
                            Some((a, b)) => clipboard_set(&char_slice(&inp.value, a, b)),
                            None => clipboard_set(&inp.value),
                        }
                    }
                    KeyCode::KeyX => {
                        if let Some((a, b)) = inp.selection_range() {
                            clipboard_set(&char_slice(&inp.value, a, b));
                            let (s, e) = (byte_offset(&inp.value, a), byte_offset(&inp.value, b));
                            inp.value.replace_range(s..e, "");
                            inp.caret_index = a;
                            inp.sel_anchor = None;
                        } else {
                            clipboard_set(&inp.value);
                            inp.value.clear();
                            inp.caret_index = 0;
                            inp.select_all = false;
                        }
                    }
                    KeyCode::KeyA => {
                        inp.select_all = true;
                        inp.sel_anchor = None;
                        continue;
                    }
                    _ => continue,
                }
                let (text_e, val, ph, pw) = (inp.text_entity, inp.value.clone(), inp.placeholder.clone(), inp.password);
                if let Ok((mut t, mut c)) = texts.get_mut(text_e) {
                    let (disp, col) = display_for(&val, &ph, pw);
                    *t = Text::new(disp);
                    c.0 = rgb(col);
                }
                break;
            }
            let selected = inp.select_all;
            if single.is_some() {
                // Single-line: edits + caret moves act at `caret_index`. A drag
                // selection behaves like a partial select-all: typing/paste
                // replaces it, Backspace/Delete removes it, caret motion
                // collapses it.
                let len = inp.value.chars().count();
                let mut idx = inp.caret_index.min(len);
                let range = inp.selection_range();
                match &ev.logical_key {
                    Key::Character(s) => {
                        wipe_selection(&mut inp, range, &mut idx);
                        let b = byte_offset(&inp.value, idx);
                        inp.value.insert_str(b, s);
                        idx += s.chars().count();
                    }
                    Key::Space => {
                        wipe_selection(&mut inp, range, &mut idx);
                        let b = byte_offset(&inp.value, idx);
                        inp.value.insert(b, ' ');
                        idx += 1;
                    }
                    Key::Backspace => {
                        if selected || range.is_some() {
                            wipe_selection(&mut inp, range, &mut idx);
                        } else if idx > 0 {
                            let (start, end) =
                                (byte_offset(&inp.value, idx - 1), byte_offset(&inp.value, idx));
                            inp.value.replace_range(start..end, "");
                            idx -= 1;
                        }
                    }
                    Key::Delete => {
                        if selected || range.is_some() {
                            wipe_selection(&mut inp, range, &mut idx);
                        } else if idx < len {
                            let (start, end) =
                                (byte_offset(&inp.value, idx), byte_offset(&inp.value, idx + 1));
                            inp.value.replace_range(start..end, "");
                        }
                    }
                    Key::ArrowLeft => {
                        inp.select_all = false;
                        inp.sel_anchor = None;
                        idx = idx.saturating_sub(1);
                    }
                    Key::ArrowRight => {
                        inp.select_all = false;
                        inp.sel_anchor = None;
                        idx = (idx + 1).min(len);
                    }
                    Key::Home => {
                        inp.select_all = false;
                        inp.sel_anchor = None;
                        idx = 0;
                    }
                    Key::End => {
                        inp.select_all = false;
                        inp.sel_anchor = None;
                        idx = len;
                    }
                    // Enter is a submit/commit gesture other systems own (see
                    // `form::form_enter_submit`); never a literal newline in a
                    // single-line field.
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

/// Reflect programmatic `value` changes into the displayed text.
///
/// Panels clear/rewrite `EmberTextInput.value` directly on submit (send a
/// message, create a team, …), but the Text child was only redrawn by the
/// typing system — so a cleared composer kept *showing* the sent message until
/// the next keystroke. This syncs the display (back to the placeholder when
/// emptied) and clamps the caret/selection whenever the value changes for any
/// reason; its own writes bypass change detection so it settles instead of
/// re-triggering itself every frame.
pub(crate) fn text_input_sync(
    mut inputs: Query<&mut EmberTextInput, Changed<EmberTextInput>>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
) {
    for mut inp in &mut inputs {
        let inp = inp.bypass_change_detection();
        let n = inp.value.chars().count();
        if inp.caret_index > n {
            inp.caret_index = n;
        }
        if let Some(a) = inp.sel_anchor {
            if a > n {
                inp.sel_anchor = Some(n);
            }
        }
        if inp.value.is_empty() {
            inp.select_all = false;
            inp.sel_anchor = None;
        }
        let Ok((mut t, mut c)) = texts.get_mut(inp.text_entity) else {
            continue;
        };
        let (disp, col) = display_for(&inp.value, &inp.placeholder, inp.password);
        if t.0 != disp {
            t.0 = disp;
        }
        let col = rgb(col);
        if c.0 != col {
            c.0 = col;
        }
    }
}

/// Measure per-char caret boundaries from the text node's shaped parley layout
/// (via [`bevy::text::ComputedTextBlock`]). Each cluster's advance yields the
/// exact x of every caret slot, so the caret sits between the right glyphs and
/// clicks hit every position — an averaged advance drifts on proportional
/// fonts. Also keeps the average `advance` as the pre-measure fallback. Skips
/// empty fields (the layout holds the placeholder, not the value).
pub(crate) fn text_input_measure(
    mut inputs: Query<&mut EmberTextInput, With<SingleLineInput>>,
    layouts: Query<(&TextLayoutInfo, &bevy::text::ComputedTextBlock)>,
) {
    for mut inp in &mut inputs {
        let n = inp.value.chars().count();
        if n == 0 {
            if !inp.offsets.is_empty() {
                inp.offsets.clear();
            }
            continue;
        }
        let Ok((info, block)) = layouts.get(inp.text_entity) else {
            continue;
        };
        let sf = if info.scale_factor > 0.0 { info.scale_factor } else { 1.0 };
        let adv = (info.size.x / sf) / n as f32;
        if adv > 0.5 && (adv - inp.advance).abs() > 0.01 {
            inp.advance = adv;
        }
        // What the layout actually shaped (bullets for password fields — same
        // char count as the value, so slots map 1:1).
        let (disp, _) = display_for(&inp.value, &inp.placeholder, inp.password);
        let mut offsets: Vec<f32> = Vec::with_capacity(n + 1);
        offsets.push(0.0);
        let mut x = 0.0f32;
        // Single line (no_wrap); logical order == visual order for LTR text.
        for line in block.buffer().lines() {
            for run in line.runs() {
                for cluster in run.clusters() {
                    let chars = disp
                        .get(cluster.text_range())
                        .map(|s| s.chars().count())
                        .unwrap_or(1)
                        .max(1);
                    let cadv = cluster.advance() / sf;
                    // A multi-char cluster (ligature) gets evenly split slots.
                    for k in 1..=chars {
                        offsets.push(x + cadv * k as f32 / chars as f32);
                    }
                    x += cadv;
                }
            }
        }
        // Commit only when the layout matches the value — while the shaper is
        // a frame behind a keystroke, stale boundaries would misplace the
        // caret; `caret_x` falls back to the average advance instead.
        if offsets.len() == n + 1 {
            if inp.offsets != offsets {
                inp.offsets = offsets;
            }
        } else if !inp.offsets.is_empty() && inp.offsets.len() != n + 1 {
            inp.offsets.clear();
        }
    }
}

/// Position the absolute caret at `PAD_X +` the measured boundary x of
/// `caret_index` (see [`EmberTextInput::caret_x`]).
pub(crate) fn text_input_caret_pos(
    inputs: Query<&EmberTextInput, With<SingleLineInput>>,
    mut nodes: Query<&mut Node>,
) {
    for inp in &inputs {
        if let Ok(mut n) = nodes.get_mut(inp.caret) {
            let left = Val::Px(PAD_X + inp.caret_x(inp.caret_index));
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


// ── System clipboard (native only; wasm no-ops like the code editor) ─────────

#[cfg(not(target_arch = "wasm32"))]
fn clipboard_get() -> Option<String> {
    arboard::Clipboard::new().ok().and_then(|mut cb| cb.get_text().ok())
}
#[cfg(target_arch = "wasm32")]
fn clipboard_get() -> Option<String> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn clipboard_set(s: &str) {
    if let Ok(mut cb) = arboard::Clipboard::new() {
        let _ = cb.set_text(s.to_string());
    }
}
#[cfg(target_arch = "wasm32")]
fn clipboard_set(_s: &str) {}

// ── Right-click context menu: Copy / Cut / Paste / Select all ────────────────

/// The open input context menu: (menu root, target input box).
#[derive(Resource, Default)]
pub(crate) struct InputContextMenu(Option<(Entity, Entity)>);

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputCtxAction {
    Copy,
    Cut,
    Paste,
    SelectAll,
}

#[derive(Component)]
pub(crate) struct InputCtxBtn(InputCtxAction);
#[derive(Component)]
pub(crate) struct InputCtxBackdrop;

/// Right-click on any text input → spawn the Copy/Cut/Paste menu at the cursor.
pub(crate) fn text_input_context_open(
    mut commands: Commands,
    fonts: Option<Res<crate::font::EmberFonts>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut menu: ResMut<InputContextMenu>,
    inputs: Query<(Entity, &Interaction), With<EmberTextInput>>,
) {
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    let Some(fonts) = fonts else { return };
    if let Some((root, _)) = menu.0.take() {
        commands.entity(root).try_despawn();
    }
    let Some((target, _)) = inputs
        .iter()
        .find(|(_, i)| matches!(i, Interaction::Hovered | Interaction::Pressed))
    else {
        return;
    };
    let cursor = windows
        .iter()
        .next()
        .and_then(|w| w.cursor_position())
        .unwrap_or(Vec2::new(200.0, 200.0));

    let backdrop = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            crate::cursor_icon::NoAutoCursor,
            InputCtxBackdrop,
            GlobalZIndex(980),
            Name::new("input_context_menu"),
        ))
        .id();
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(cursor.x.max(4.0)),
                top: Val::Px(cursor.y.max(4.0)),
                width: Val::Px(130.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(4.0)),
                row_gap: Val::Px(1.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            bevy::ui::FocusPolicy::Block,
        ))
        .id();
    for (label, action) in [
        ("Copy", InputCtxAction::Copy),
        ("Cut", InputCtxAction::Cut),
        ("Paste", InputCtxAction::Paste),
        ("Select all", InputCtxAction::SelectAll),
    ] {
        let row = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                InputCtxBtn(action),
            ))
            .id();
        let t = commands
            .spawn((Text::new(label), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary()))))
            .id();
        commands.entity(row).add_child(t);
        commands.entity(panel).add_child(row);
    }
    commands.entity(backdrop).add_child(panel);
    menu.0 = Some((backdrop, target));
}

/// Hover feedback + action execution + dismissal for the input context menu.
pub(crate) fn text_input_context_clicks(
    mut commands: Commands,
    mut menu: ResMut<InputContextMenu>,
    mut rows: Query<(&Interaction, &InputCtxBtn, &mut BackgroundColor), Changed<Interaction>>,
    backdrops: Query<&Interaction, (With<InputCtxBackdrop>, Changed<Interaction>)>,
    mut inputs: Query<&mut EmberTextInput>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
) {
    let Some((root, target)) = menu.0 else { return };
    let mut close = false;
    for (i, btn, mut bg) in &mut rows {
        match i {
            Interaction::Hovered => bg.0 = rgb(hover_bg()),
            Interaction::None => bg.0 = Color::NONE,
            Interaction::Pressed => {
                if let Ok(mut inp) = inputs.get_mut(target) {
                    match btn.0 {
                        InputCtxAction::Copy => match inp.selection_range() {
                            Some((a, b)) => clipboard_set(&char_slice(&inp.value, a, b)),
                            None => clipboard_set(&inp.value),
                        },
                        InputCtxAction::Cut => {
                            if let Some((a, b)) = inp.selection_range() {
                                clipboard_set(&char_slice(&inp.value, a, b));
                                let (s, e) =
                                    (byte_offset(&inp.value, a), byte_offset(&inp.value, b));
                                inp.value.replace_range(s..e, "");
                                inp.caret_index = a;
                                inp.sel_anchor = None;
                            } else {
                                clipboard_set(&inp.value);
                                inp.value.clear();
                                inp.caret_index = 0;
                                inp.select_all = false;
                            }
                        }
                        InputCtxAction::Paste => {
                            if let Some(pasted) = clipboard_get() {
                                let pasted = pasted.replace('\r', "");
                                if inp.select_all {
                                    inp.value.clear();
                                    inp.select_all = false;
                                    inp.caret_index = 0;
                                } else if let Some((a, b)) = inp.selection_range() {
                                    let (s, e) =
                                        (byte_offset(&inp.value, a), byte_offset(&inp.value, b));
                                    inp.value.replace_range(s..e, "");
                                    inp.caret_index = a;
                                }
                                inp.sel_anchor = None;
                                let len = inp.value.chars().count();
                                let idx = inp.caret_index.min(len);
                                let b = byte_offset(&inp.value, idx);
                                inp.value.insert_str(b, &pasted);
                                inp.caret_index = idx + pasted.chars().count();
                            }
                        }
                        InputCtxAction::SelectAll => {
                            inp.select_all = true;
                            inp.sel_anchor = None;
                        }
                    }
                    let (text_e, val, ph, pw) =
                        (inp.text_entity, inp.value.clone(), inp.placeholder.clone(), inp.password);
                    if let Ok((mut t, mut c)) = texts.get_mut(text_e) {
                        let (disp, col) = display_for(&val, &ph, pw);
                        *t = Text::new(disp);
                        c.0 = rgb(col);
                    }
                }
                close = true;
            }
        }
    }
    for i in &backdrops {
        if *i == Interaction::Pressed {
            close = true;
        }
    }
    if close {
        commands.entity(root).try_despawn();
        menu.0 = None;
    }
}
