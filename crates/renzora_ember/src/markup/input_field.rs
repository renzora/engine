//! Text input kernel — `<input bind="Entity.var" placeholder="..." password="true">`.
//!
//! An `<input>` is a focusable text node. Click to focus; typing edits its
//! value; the value is written back into a **script variable** named by `bind`
//! (`Account.email` → var `email` on the entity named `Account`; bare `email`
//! → the host entity). Because the scripting loop injects script vars into the
//! Lua VM each frame and reads them back, a UI-written value persists and is
//! visible to the script as a normal global — that's the two-way link.
//!
//! This is the focus + text-entry kernel the rest of the widget catalog builds
//! on (search, password, chat, rename dialogs, forms / login).
//!
//! **IME input** (Korean, Japanese, Chinese) follows the editor's text inputs
//! (`widgets::ime`): the syllable being composed is shown before the caret but
//! is not part of `value`, so the bound script variable only ever sees
//! committed text. Moving focus mid-composition commits it into the input
//! being left. The window's IME switch is shared with the editor's inputs and
//! decided in `widgets::ime::ime_focus_sync`.

use bevy::input::ButtonState;
use bevy::input::keyboard::Key;
use bevy::prelude::*;
use bevy::text::TextLayoutInfo;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::{Ime, WindowEvent};
use renzora_scripting::{ScriptComponent, ScriptValue};

use crate::UiInteraction;
use crate::widgets::place_ime;

/// Which input currently has keyboard focus (at most one), and the IME
/// composition in progress in it.
#[derive(Resource, Default)]
pub struct UiFocus {
    pub focused: Option<Entity>,
    /// The preedit the IME is building. It lives here rather than on the input
    /// so that an input despawned mid-composition still leaves a record that
    /// the IME is composing.
    pub preedit: String,
    /// The input `preedit` belongs to. Focus can move before the IME commits,
    /// and the preedit stays with the input it was typed in.
    preedit_owner: Option<Entity>,
    /// Focus left an input mid-composition and the preedit was committed into
    /// it: the commit the IME may still flush for it belongs to no input.
    /// Cleared by the IME's `Enabled`/`Disabled` or a new preedit, as in
    /// `widgets::ime::ImeComposition::discard_commit`.
    pub(crate) discard_commit: bool,
    /// Ask `ime_focus_sync` to withdraw the IME for one frame, which is what
    /// ends the OS-side composition when focus moves within one window.
    pub(crate) ime_bounce: bool,
}

/// Marks an `<input>` entity. The entity also carries `Node` + `Text` +
/// `UiButton` (for click interaction); this holds the edit state + binding.
#[derive(Component)]
pub struct TextInput {
    /// `bind=` target: `"EntityName.var"` or bare `"var"` (host entity).
    bind: String,
    /// Current edited text.
    value: String,
    /// Shown (dimmed) when empty and unfocused.
    placeholder: String,
    /// Mask characters with bullets.
    password: bool,
    /// Host entity for bare `bind` targets.
    host: Entity,
    /// Pulled the initial value from the bound var yet?
    initialized: bool,
}

impl TextInput {
    pub fn new(bind: String, placeholder: String, password: bool, host: Entity) -> Self {
        Self {
            bind,
            value: String::new(),
            placeholder,
            password,
            host,
            initialized: false,
        }
    }
}

/// Resolve a `bind` string to `(entity, var_name)`.
fn resolve_bind(
    bind: &str,
    host: Entity,
    names: &bevy::platform::collections::HashMap<String, Entity>,
) -> Option<(Entity, String)> {
    if let Some((ent_name, var)) = bind.split_once('.') {
        let entity = *names.get(ent_name)?;
        Some((entity, var.to_string()))
    } else {
        Some((host, bind.to_string()))
    }
}

/// Click an input → focus it (and unfocus others). A press anywhere else ends
/// the focus, as it does for any OS text field.
///
/// Before that, an input stayed focused until Enter or Escape: clicking an
/// ember field or the code editor left it typing too, so every key and every
/// IME commit went into both.
fn focus_on_click(
    interactions: Query<(Entity, &UiInteraction), (Changed<UiInteraction>, With<TextInput>)>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    mut focus: ResMut<UiFocus>,
) {
    let mut hit = None;
    for (entity, interaction) in &interactions {
        if *interaction == UiInteraction::Pressed {
            hit = Some(entity);
        }
    }
    if hit.is_some() {
        focus.focused = hit;
    } else if focus.focused.is_some() && mouse.is_some_and(|m| m.get_just_pressed().next().is_some()) {
        focus.focused = None;
    }
}

/// Focus moved away from an input mid-composition: commit the preedit into
/// the input being left, as an OS text field does, and end the composition.
///
/// Committed here rather than left to the IME, because whether an IME flushes
/// its composition when focus moves differs from one IME to the next. The IME
/// is still composing on its side, so it is withdrawn for a frame, and the
/// duplicate commit it may flush in response is dropped.
fn commit_on_leave(mut focus: ResMut<UiFocus>, mut inputs: Query<&mut TextInput>) {
    if focus.preedit.is_empty() || focus.preedit_owner == focus.focused {
        return;
    }
    let preedit = std::mem::take(&mut focus.preedit);
    // An input despawned mid-composition has nowhere to keep it.
    if let Some(mut input) = focus.preedit_owner.and_then(|e| inputs.get_mut(e).ok()) {
        input.value.push_str(&preedit);
    }
    focus.preedit_owner = None;
    focus.discard_commit = true;
    focus.ime_bounce = true;
}

/// Feed keystrokes and IME input into the focused input.
///
/// Reads `WindowEvent` rather than `KeyboardInput`: an IME commit and the key
/// that finished it arrive interleaved, and only the shared stream keeps their
/// order. It is read even with nothing focused, so that the IME's own end of a
/// composition still clears [`UiFocus::discard_commit`].
fn type_into_focused(
    mut events: MessageReader<WindowEvent>,
    mut focus: ResMut<UiFocus>,
    mut inputs: Query<&mut TextInput>,
) {
    let mut input = focus.focused.and_then(|e| inputs.get_mut(e).ok());
    for event in events.read() {
        let ev = match event {
            WindowEvent::KeyboardInput(ev) => ev,
            WindowEvent::Ime(ime) => {
                type_ime(ime, &mut focus, input.as_deref_mut());
                continue;
            }
            _ => continue,
        };
        // `Key::Process` is a key the IME consumed: the Backspace that removes
        // a jamo, the Space or Enter that finishes a syllable. Its effect
        // arrives as `Ime` messages, so acting on it too would delete a
        // committed character, type a second space, or drop focus.
        if ev.state != ButtonState::Pressed || matches!(ev.logical_key, Key::Process) {
            continue;
        }
        let Some(input) = input.as_deref_mut() else {
            continue;
        };
        match &ev.logical_key {
            Key::Character(s) => input.value.push_str(s.as_str()),
            Key::Space => input.value.push(' '),
            Key::Backspace => {
                input.value.pop();
            }
            Key::Enter | Key::Escape => {
                focus.focused = None;
                break;
            }
            _ => {}
        }
    }
}

/// Apply one IME message to the focused input, if any.
fn type_ime(ime: &Ime, focus: &mut UiFocus, input: Option<&mut TextInput>) {
    match ime {
        Ime::Preedit { value, .. } => {
            // The caret is always at the end of an `<input>`, so the IME's
            // cursor inside the preedit has nowhere to go: the preedit is
            // simply drawn before the caret.
            // An empty preedit is how winit ends a composition, sent ahead of
            // every commit, the IME's flush after a focus move included. It
            // starts nothing, so it must not disarm the guard waiting for
            // that flush, or the syllable lands in the new input as well.
            if input.is_some() && !value.is_empty() {
                focus.preedit = value.clone();
                focus.preedit_owner = focus.focused;
                focus.discard_commit = false;
            } else {
                focus.preedit.clear();
            }
        }
        Ime::Commit { value, .. } => {
            focus.preedit.clear();
            if std::mem::take(&mut focus.discard_commit) || value.is_empty() {
                return;
            }
            if let Some(input) = input {
                input.value.push_str(&value.replace(['\r', '\n'], " "));
            }
        }
        // winit sends any flushed commit before `Disabled`, and starts a new
        // composition with `Enabled`, so past either one there is no
        // duplicate left to discard.
        Ime::Disabled { .. } => {
            focus.preedit.clear();
            focus.discard_commit = false;
        }
        Ime::Enabled { .. } => focus.discard_commit = false,
    }
}

/// Put the IME's candidate window under the end of the focused input's text,
/// where the caret is. The text's own laid-out width is used rather than the
/// node's, because an `<input>` is usually a fixed-width box.
fn ime_position(
    focus: Res<UiFocus>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform, Option<&TextLayoutInfo>), With<TextInput>>,
    mut windows: Query<&mut Window>,
) {
    let Some((cn, xf, layout)) = focus.focused.and_then(|e| nodes.get(e).ok()) else {
        return;
    };
    let size = cn.size();
    let text_w = layout.map_or(0.0, |l| l.size.x);
    let left = xf.affine().translation.x - size.x * 0.5 + cn.content_inset().min_inset.x;
    let x = (left + text_w).min(xf.affine().translation.x + size.x * 0.5);
    place_ime(&mut windows, Vec2::new(x, xf.affine().translation.y + size.y * 0.5));
}

/// Each frame: seed the initial value from the bound var (once), write the
/// current value back to it, and refresh the displayed text (placeholder /
/// password mask / caret).
fn sync_inputs(
    focus: Res<UiFocus>,
    names_q: Query<(Entity, &Name)>,
    mut scripts: Query<&mut ScriptComponent>,
    mut inputs: Query<(Entity, &mut TextInput)>,
    mut texts: Query<&mut Text>,
) {
    // This system is unconditional in `Update`, and the name map below is a scan
    // of *every named entity in the world* with a `String` allocation per entry.
    // Built eagerly it cost ~0.62 ms/frame in a loaded scene — paid in full on the
    // overwhelmingly common frames where there is no `<input>` on screen at all,
    // purely to be thrown away. Two gates, cheapest first.
    if inputs.is_empty() {
        return;
    }

    // Only a dotted `Entity.var` bind consults the map (`resolve_bind` uses
    // `host` otherwise), so a screen of plain inputs shouldn't pay for it either.
    // An empty map is also *correct* if we're wrong: `resolve_bind` just returns
    // `None`, the same as an unresolved name.
    let mut names = bevy::platform::collections::HashMap::default();
    if inputs.iter().any(|(_, i)| i.bind.contains('.')) {
        for (e, n) in &names_q {
            names.insert(n.as_str().to_string(), e);
        }
    }

    for (entity, mut input) in &mut inputs {
        let focused = focus.focused == Some(entity);

        if let Some((target, var)) = resolve_bind(&input.bind, input.host, &names) {
            if let Ok(mut sc) = scripts.get_mut(target) {
                if !input.initialized {
                    // Pull current value from the script var (if any).
                    if let Some(v) = script_var_get(&sc, &var) {
                        input.value = v;
                    }
                    input.initialized = true;
                } else {
                    // Push the edited value back into the script var.
                    let value = input.value.clone();
                    script_var_set(&mut sc, &var, value);
                }
            } else {
                input.initialized = true; // no script yet — don't block editing
            }
        }

        // Rendered text, with the IME's preedit in front of the caret.
        let preedit = if focused && focus.preedit_owner == Some(entity) {
            focus.preedit.as_str()
        } else {
            ""
        };
        let display = if input.value.is_empty() && !focused {
            input.placeholder.clone()
        } else if input.password {
            "•".repeat(input.value.chars().count() + preedit.chars().count())
        } else {
            format!("{}{preedit}", input.value)
        };
        let display = if focused {
            format!("{display}|")
        } else {
            display
        };
        if let Ok(mut text) = texts.get_mut(entity) {
            if text.0 != display {
                text.0 = display;
            }
        }
    }
}

fn script_var_get(sc: &ScriptComponent, var: &str) -> Option<String> {
    for entry in &sc.scripts {
        if let Some(v) = entry.variables.get(var) {
            return Some(match v {
                ScriptValue::String(s) => s.clone(),
                ScriptValue::Float(f) => f.to_string(),
                ScriptValue::Int(i) => i.to_string(),
                ScriptValue::Bool(b) => b.to_string(),
                _ => String::new(),
            });
        }
    }
    None
}

fn script_var_set(sc: &mut ScriptComponent, var: &str, value: String) {
    // Write to the script that already declares the var, else the first one.
    for entry in &mut sc.scripts {
        if entry.variables.get(var).is_some() {
            entry.variables.set(var.to_string(), ScriptValue::String(value));
            return;
        }
    }
    if let Some(entry) = sc.scripts.first_mut() {
        entry.variables.set(var.to_string(), ScriptValue::String(value));
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<UiFocus>().add_systems(
        Update,
        (
            focus_on_click,
            commit_on_leave,
            type_into_focused,
            sync_inputs,
            ime_position,
        )
            .chain(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::message::Messages;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::input::keyboard::KeyboardInput;

    use crate::widgets::{ime_focus_sync, ImeComposition};

    /// A world holding one focused `<input>` showing `value`.
    fn typing_world(value: &str) -> (World, Entity) {
        let mut world = World::new();
        world.init_resource::<Messages<WindowEvent>>();
        world.init_resource::<ImeComposition>();
        let mut input = TextInput::new(String::new(), String::new(), false, Entity::PLACEHOLDER);
        input.value = value.to_string();
        let e = world.spawn((input, Text::default())).id();
        world.insert_resource(UiFocus {
            focused: Some(e),
            ..default()
        });
        (world, e)
    }

    /// Apply `events` as one frame. The queue is cleared first: every
    /// `run_system_once` starts a fresh reader, which would replay earlier
    /// frames' messages as well.
    fn send(world: &mut World, events: impl IntoIterator<Item = WindowEvent>) {
        world.resource_mut::<Messages<WindowEvent>>().clear();
        for ev in events {
            world.write_message(ev);
        }
        world.run_system_once(type_into_focused).unwrap();
    }

    fn preedit(value: &str) -> WindowEvent {
        WindowEvent::Ime(Ime::Preedit {
            window: Entity::PLACEHOLDER,
            value: value.into(),
            cursor: Some((0, 0)),
        })
    }

    fn commit(value: &str) -> WindowEvent {
        WindowEvent::Ime(Ime::Commit {
            window: Entity::PLACEHOLDER,
            value: value.into(),
        })
    }

    fn value(world: &World, e: Entity) -> &str {
        &world.get::<TextInput>(e).unwrap().value
    }

    #[test]
    fn a_preedit_is_shown_before_the_caret_but_not_bound() {
        let (mut world, e) = typing_world("가");
        send(&mut world, [preedit("ㄴ")]);
        assert_eq!(value(&world, e), "가");
        world.run_system_once(sync_inputs).unwrap();
        assert_eq!(world.get::<Text>(e).unwrap().0, "가ㄴ|");
        send(&mut world, [preedit(""), commit("나")]);
        assert_eq!(value(&world, e), "가나");
    }

    #[test]
    fn a_key_the_ime_consumed_edits_nothing() {
        let (mut world, e) = typing_world("가");
        // The Enter that finishes a syllable must not drop focus either.
        let enter = KeyboardInput {
            key_code: KeyCode::Enter,
            logical_key: Key::Process,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        };
        send(&mut world, [WindowEvent::KeyboardInput(enter)]);
        assert_eq!(value(&world, e), "가");
        assert_eq!(world.resource::<UiFocus>().focused, Some(e));
    }

    /// Clicking another `<input>` mid-syllable keeps the syllable where it was
    /// typed, and the IME's own flush of it lands nowhere.
    #[test]
    fn leaving_an_input_mid_composition_commits_it_there() {
        let (mut world, e) = typing_world("가");
        send(&mut world, [preedit("한")]);
        let other = world
            .spawn((
                TextInput::new(String::new(), String::new(), false, Entity::PLACEHOLDER),
                Text::default(),
            ))
            .id();
        world.resource_mut::<UiFocus>().focused = Some(other);
        world.run_system_once(commit_on_leave).unwrap();
        assert_eq!(value(&world, e), "가한");
        send(
            &mut world,
            [
                commit("한"),
                WindowEvent::Ime(Ime::Disabled {
                    window: Entity::PLACEHOLDER,
                }),
            ],
        );
        assert_eq!(value(&world, other), "");
        // Past `Disabled` the guard is down, so the next commit is typed.
        send(&mut world, [commit("글")]);
        assert_eq!(value(&world, other), "글");
    }

    /// The same, with the flush as winit delivers it: an empty preedit ahead
    /// of the commit, which must not disarm the guard.
    #[test]
    fn the_flush_after_a_focus_move_is_dropped_behind_an_empty_preedit() {
        let (mut world, e) = typing_world("가");
        send(&mut world, [preedit("한")]);
        let other = world
            .spawn((
                TextInput::new(String::new(), String::new(), false, Entity::PLACEHOLDER),
                Text::default(),
            ))
            .id();
        world.resource_mut::<UiFocus>().focused = Some(other);
        world.run_system_once(commit_on_leave).unwrap();
        send(&mut world, [preedit(""), commit("한")]);
        assert_eq!(value(&world, e), "가한");
        assert_eq!(value(&world, other), "");
    }

    /// A press that lands on no `<input>` ends the focus, so typing meant for
    /// another field does not reach this one as well.
    #[test]
    fn a_press_elsewhere_unfocuses_the_input() {
        let (mut world, _) = typing_world("가");
        let mut mouse = ButtonInput::<MouseButton>::default();
        mouse.press(MouseButton::Left);
        world.insert_resource(mouse);
        world.run_system_once(focus_on_click).unwrap();
        assert_eq!(world.resource::<UiFocus>().focused, None);
    }

    /// The editor's text inputs and the game UI's share one window switch:
    /// with only an `<input>` focused it must stay on, and a focus move
    /// mid-composition must turn it off for exactly one frame.
    #[test]
    fn a_focused_input_allows_the_ime_and_a_focus_move_bounces_it() {
        let (mut world, _) = typing_world("");
        let window = world.spawn(Window::default()).id();
        world.run_system_once(ime_focus_sync).unwrap();
        assert!(world.get::<Window>(window).unwrap().ime_enabled);
        world.resource_mut::<UiFocus>().ime_bounce = true;
        world.run_system_once(ime_focus_sync).unwrap();
        assert!(!world.get::<Window>(window).unwrap().ime_enabled);
        world.run_system_once(ime_focus_sync).unwrap();
        assert!(world.get::<Window>(window).unwrap().ime_enabled);
    }
}
