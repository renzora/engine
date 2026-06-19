//! Text input — a single-line editable field (click to focus, type to edit).

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::ui_font;
use crate::style::{Role, Styled, WidgetState};
use crate::theme::*;

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
    /// the moment the user edits. There's no partial selection (no caret index).
    pub select_all: bool,
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
    font: &Handle<Font>,
    placeholder: &str,
    value: &str,
) -> Entity {
    build_input(commands, font, placeholder, value, false)
}

/// A [`text_input`] whose value renders masked (`••••`) — for passwords.
pub fn password_input(
    commands: &mut Commands,
    font: &Handle<Font>,
    placeholder: &str,
    value: &str,
) -> Entity {
    build_input(commands, font, placeholder, value, true)
}

fn build_input(
    commands: &mut Commands,
    font: &Handle<Font>,
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
    let car = caret(commands);
    commands.entity(box_e).insert(EmberTextInput {
        value: value.to_string(),
        focused: false,
        text_entity: text,
        placeholder: placeholder.to_string(),
        caret: car,
        password,
        select_all: false,
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

pub(crate) fn text_input_focus(
    mouse: Res<ButtonInput<MouseButton>>,
    mut inputs: Query<(Entity, &Interaction, &mut EmberTextInput, &mut Styled)>,
) {
    // Only react to the press itself. The input under the press (if any) takes
    // focus; a press anywhere else — empty space or another widget — blurs every
    // input (off-click to dismiss).
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let clicked = inputs
        .iter()
        .find(|(_, i, _, _)| matches!(i, Interaction::Pressed))
        .map(|(e, _, _, _)| e);
    for (e, _, mut inp, mut styled) in &mut inputs {
        let focus = Some(e) == clicked;
        if inp.focused != focus {
            inp.focused = focus;
            styled.state = if focus {
                WidgetState::Active
            } else {
                WidgetState::Normal
            };
        }
    }
}

pub(crate) fn text_input_type(
    mut events: MessageReader<KeyboardInput>,
    mut inputs: Query<&mut EmberTextInput>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
) {
    for ev in events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        for mut inp in &mut inputs {
            if !inp.focused {
                continue;
            }
            // With the whole value selected, the first edit replaces it: typing
            // overwrites, Backspace/Delete clears. Cleared as soon as it applies.
            let selected = inp.select_all;
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
                        // Delete/Backspace over a full selection wipes everything.
                        inp.value.clear();
                        inp.select_all = false;
                    } else if matches!(ev.logical_key, Key::Backspace) {
                        // No selection + caret pinned at the end: Backspace removes
                        // the last char; forward-Delete is a no-op.
                        inp.value.pop();
                    }
                }
                _ => {}
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
