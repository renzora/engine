//! Text input — a single-line editable field (click to focus, type to edit).

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::ui_font;
use crate::style::{Role, Styled, WidgetState};
use crate::theme::{rgb, ACCENT_BLUE, TEXT_MUTED, TEXT_PRIMARY};

/// Shared state for text-input-like widgets (single line + textarea). Public so
/// panels in other crates can read the typed `value` (and clear it on submit).
#[derive(Component)]
pub struct EmberTextInput {
    pub value: String,
    pub focused: bool,
    pub text_entity: Entity,
    pub placeholder: String,
    pub caret: Entity,
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
            BackgroundColor(rgb(ACCENT_BLUE)),
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
    let empty = value.is_empty();
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
            BackgroundColor(rgb((28, 28, 34))),
            BorderColor::all(rgb((70, 70, 82))),
            Interaction::default(),
            Styled::new(Role::Input),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Text),
            Name::new("text-input"),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(if empty { placeholder } else { value }),
            ui_font(font, 12.0),
            TextColor(rgb(if empty { TEXT_MUTED } else { TEXT_PRIMARY })),
        ))
        .id();
    let car = caret(commands);
    commands.entity(box_e).insert(EmberTextInput {
        value: value.to_string(),
        focused: false,
        text_entity: text,
        placeholder: placeholder.to_string(),
        caret: car,
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
        let Some((focused, widget_val, text_e, ph)) = world
            .get::<EmberTextInput>(input)
            .map(|i| (i.focused, i.value.clone(), i.text_entity, i.placeholder.clone()))
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
            if let Some(mut t) = world.get_mut::<Text>(text_e) {
                t.0 = if state_val.is_empty() {
                    ph
                } else {
                    state_val.clone()
                };
            }
            if let Some(mut col) = world.get_mut::<TextColor>(text_e) {
                col.0 = rgb(if state_val.is_empty() {
                    TEXT_MUTED
                } else {
                    TEXT_PRIMARY
                });
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
            match &ev.logical_key {
                Key::Character(s) => inp.value.push_str(s),
                Key::Space => inp.value.push(' '),
                Key::Enter => inp.value.push('\n'),
                Key::Backspace => {
                    inp.value.pop();
                }
                _ => {}
            }
            let (text_e, val, ph) = (inp.text_entity, inp.value.clone(), inp.placeholder.clone());
            if let Ok((mut t, mut c)) = texts.get_mut(text_e) {
                if val.is_empty() {
                    *t = Text::new(ph);
                    c.0 = rgb(TEXT_MUTED);
                } else {
                    *t = Text::new(val);
                    c.0 = rgb(TEXT_PRIMARY);
                }
            }
            break;
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
