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

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;
use renzora_scripting::{ScriptComponent, ScriptValue};

/// Which input currently has keyboard focus (at most one).
#[derive(Resource, Default)]
pub struct UiFocus {
    pub focused: Option<Entity>,
}

/// Marks an `<input>` entity. The entity also carries `Node` + `Text` +
/// `Button` (for click interaction); this holds the edit state + binding.
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

/// Click an input → focus it (and unfocus others).
fn focus_on_click(
    interactions: Query<(Entity, &Interaction), (Changed<Interaction>, With<TextInput>)>,
    mut focus: ResMut<UiFocus>,
) {
    for (entity, interaction) in &interactions {
        if *interaction == Interaction::Pressed {
            focus.focused = Some(entity);
        }
    }
}

/// Feed keystrokes into the focused input.
fn type_into_focused(
    mut key_events: MessageReader<KeyboardInput>,
    mut focus: ResMut<UiFocus>,
    mut inputs: Query<&mut TextInput>,
) {
    let Some(focused) = focus.focused else {
        key_events.clear();
        return;
    };
    let Ok(mut input) = inputs.get_mut(focused) else {
        key_events.clear();
        return;
    };
    for ev in key_events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match &ev.logical_key {
            Key::Character(s) => input.value.push_str(s.as_str()),
            Key::Space => input.value.push(' '),
            Key::Backspace => {
                input.value.pop();
            }
            Key::Enter | Key::Escape => {
                focus.focused = None;
                return;
            }
            _ => {}
        }
    }
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
    // Name → entity map for `Entity.var` binds.
    let mut names = bevy::platform::collections::HashMap::default();
    for (e, n) in &names_q {
        names.insert(n.as_str().to_string(), e);
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

        // Rendered text.
        let display = if input.value.is_empty() && !focused {
            input.placeholder.clone()
        } else if input.password {
            "•".repeat(input.value.chars().count())
        } else {
            input.value.clone()
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
        (focus_on_click, type_into_focused, sync_inputs).chain(),
    );
}
