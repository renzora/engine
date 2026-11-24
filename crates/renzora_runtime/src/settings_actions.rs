//! `set_setting` / `get_setting`: every game setting, for scripts in any
//! language.
//!
//! The runtime's live settings store is `CurrentProject.config`: the systems
//! that apply a setting (window mode and vsync in `window_config`, render
//! scale, graphics quality, the mixer) read it, so writing a field there
//! changes the running game. A Rust script has `&mut World` and simply writes
//! it. A Lua script has only the functions declared for it, and the one
//! settings function it had was `action("set_ui_font")`. This module declares
//! the general pair instead, so an in-game options menu can be written in any
//! language:
//!
//! ```lua
//! set_setting("window.vsync", "false")
//! set_setting("rendering.render_scale", "0.75")
//! local fps = get_setting_number("window.max_fps")
//! ```
//!
//! A path is the key as it appears in `project.toml`, resolved generically by
//! `renzora_project_api::settings_path`, so a setting added to the config later
//! is reachable with nothing added here. Two families of path are not in
//! `project.toml` and are handled here:
//!
//! - `ui.scale`: the player's UI size, [`GameUiScale`].
//! - `input.<action>.<slot>`: one binding of the action map, in
//!   `InputBinding::to_spec` form (`"Key:KeyF"`, `"Button:South"`); an empty
//!   value clears the slot.
//!
//! Nothing here writes a file. A change lasts as long as the game runs, and a
//! game that wants it kept saves it itself.
//!
//! # Why reads go through a mirror component
//!
//! A declared script function that returns a value resolves as a reflected
//! field on the calling script's own entity: that is the only synchronous read
//! a language backend has. `ProjectConfig` is not `Reflect`, so the runtime
//! keeps a [`SettingsReadState`] on every scripted entity, rebuilt when a
//! setting changes, and the `get_setting` family reads its maps. A write is
//! applied when the script's commands are, so a `get_setting` straight after a
//! `set_setting` in the same hook still sees the old value; the next frame sees
//! the new one.

use bevy::prelude::*;
use renzora_project_api::settings_path::{self, SettingError, Value};
use renzora_project_api::{CurrentProject, EditorSession, SettingsReadState};
use renzora_scripting_api::component::ScriptComponent;
use renzora_scripting_api::script_extension::{
    Bind, Binding, ParamKind, ScriptExtension, ScriptExtensions,
};
use renzora_scripting_api::{InputBinding, InputMap, ScriptAction, ScriptActionValue};
use renzora_viewport_api::GameUiScale;

/// The path of the player's UI size, which is not a `project.toml` key.
const UI_SCALE_PATH: &str = "ui.scale";
/// The prefix of the action-map paths, `input.<action>.<slot>`.
const INPUT_PREFIX: &str = "input.";

pub(crate) fn register(app: &mut App) {
    app.register_type::<SettingsReadState>()
        .init_resource::<GameUiScale>()
        .add_observer(handle_setting_actions)
        .add_systems(
            PreUpdate,
            (
                sync_settings_read_state.run_if(not(in_editor_session)),
                strip_settings_read_state.run_if(in_editor_session),
            ),
        );
    app.world_mut()
        .get_resource_or_insert_with(ScriptExtensions::default)
        .register(SettingsScriptExtension);
}

fn in_editor_session(session: Option<Res<EditorSession>>) -> bool {
    session.is_some_and(|s| s.0)
}

/// The declared functions. Every language backend builds them from this, so
/// none of them is Lua-specific.
pub struct SettingsScriptExtension;

impl ScriptExtension for SettingsScriptExtension {
    fn name(&self) -> &str {
        "settings"
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            // `value` is text so one function covers every setting type: it is
            // parsed as the setting's own type (`"false"`, `"60"`, `"High"`).
            // `action("set_setting", {path=..., value=...})` passes a typed
            // value instead, and is handled identically.
            Bind::action("set_setting", "set_setting")
                .arg("path", ParamKind::Str)
                .arg("value", ParamKind::Str)
                .doc("Change a game setting by its project.toml path, e.g. set_setting(\"window.vsync\", \"false\").")
                .build(),
            Bind::read("get_setting", "SettingsReadState", "values[\"{0}\"]")
                .arg("path", ParamKind::Str)
                .doc("A game setting as text, e.g. get_setting(\"rendering.graphics_quality\").")
                .build(),
            Bind::read("get_setting_number", "SettingsReadState", "numbers[\"{0}\"]")
                .arg("path", ParamKind::Str)
                .doc("A numeric game setting, e.g. get_setting_number(\"window.max_fps\").")
                .build(),
            Bind::read("get_setting_bool", "SettingsReadState", "flags[\"{0}\"]")
                .arg("path", ParamKind::Str)
                .doc("A true/false game setting, e.g. get_setting_bool(\"window.vsync\").")
                .build(),
        ]
    }
}

/// A script argument as a settings value.
fn to_value(v: &ScriptActionValue) -> Value {
    match v {
        ScriptActionValue::Float(f) => Value::Float(*f as f64),
        ScriptActionValue::Int(i) => Value::Integer(*i),
        ScriptActionValue::Bool(b) => Value::Boolean(*b),
        ScriptActionValue::String(s) => Value::String(s.clone()),
        ScriptActionValue::Vec3(v) => Value::Array(v.iter().map(|c| Value::Float(*c as f64)).collect()),
    }
}

/// Everything `set_setting` can change, so the routing can be tested without a
/// `World`.
pub(crate) struct SettingTargets<'a> {
    pub project: Option<&'a mut CurrentProject>,
    pub ui_scale: Option<&'a mut GameUiScale>,
    pub input: Option<&'a mut InputMap>,
    /// In an editor session the project config is the one the editor saves,
    /// so a script's write there would end up in `project.toml`.
    pub editor_session: bool,
}

/// Apply one `set_setting`. `Ok(true)` if something changed.
pub(crate) fn apply_setting(targets: SettingTargets, path: &str, value: Value) -> Result<bool, String> {
    let path = path.trim();
    if path == UI_SCALE_PATH {
        let factor = match settings_path::coerce(Some(&Value::Float(1.0)), value) {
            Value::Float(f) => f as f32,
            other => return Err(format!("`{UI_SCALE_PATH}` wants a number, got {other}")),
        };
        let scale = targets.ui_scale.ok_or("game UI is not part of this build")?;
        let changed = scale.factor != factor;
        scale.set(factor);
        return Ok(changed);
    }
    if let Some(rest) = path.strip_prefix(INPUT_PREFIX) {
        let (action, slot) = rest
            .rsplit_once('.')
            .and_then(|(a, s)| Some((a, s.parse::<usize>().ok()?)))
            .ok_or_else(|| format!("`{path}` should be input.<action>.<slot>"))?;
        let spec = settings_path::value_text(&value);
        let binding = match spec.trim() {
            "" | "none" => None,
            s => Some(InputBinding::from_spec(s).ok_or_else(|| format!("`{s}` is not an input binding"))?),
        };
        let map = targets.input.ok_or("input is not part of this build")?;
        let before = map.get(action).cloned();
        map.rebind(action, slot, binding).map_err(|e| e.to_string())?;
        return Ok(before.as_ref() != map.get(action));
    }

    if targets.editor_session {
        return Err("project settings are not changed by scripts in the editor".into());
    }
    let project = targets.project.ok_or("no project is loaded")?;
    // Every runtime system that applies a config setting reads it from here
    // (the frame limiter included), so writing the config is the whole job.
    settings_path::set_setting(&mut project.config, path, value).map_err(|e: SettingError| e.to_string())
}

/// Handle `set_setting(path, value)`.
///
/// Every mutable reference is taken only when the setting actually changes:
/// `CurrentProject` is change-detected, and several systems re-apply the
/// window or rebuild state whenever it is marked changed.
fn handle_setting_actions(
    trigger: On<ScriptAction>,
    session: Option<Res<EditorSession>>,
    mut project: Option<ResMut<CurrentProject>>,
    mut ui_scale: Option<ResMut<GameUiScale>>,
    mut input: Option<ResMut<InputMap>>,
) {
    let action = trigger.event();
    if action.name != "set_setting" {
        return;
    }
    let Some(ScriptActionValue::String(path)) = action.args.get("path") else {
        warn!("[settings] set_setting needs a `path` string");
        return;
    };
    let Some(value) = action.args.get("value") else {
        warn!("[settings] set_setting(\"{path}\") needs a `value`");
        return;
    };
    let targets = SettingTargets {
        project: project.as_mut().map(|p| p.bypass_change_detection()),
        ui_scale: ui_scale.as_mut().map(|s| s.bypass_change_detection()),
        input: input.as_mut().map(|m| m.bypass_change_detection()),
        editor_session: session.is_some_and(|s| s.0),
    };
    match apply_setting(targets, path, to_value(value)) {
        Ok(true) => {
            // Changed through the bypass, so mark exactly the store that moved.
            let path = path.trim();
            if path == UI_SCALE_PATH {
                if let Some(s) = ui_scale.as_mut() {
                    s.set_changed();
                }
            } else if path.starts_with(INPUT_PREFIX) {
                if let Some(m) = input.as_mut() {
                    m.set_changed();
                }
            } else if let Some(p) = project.as_mut() {
                p.set_changed();
            }
        }
        Ok(false) => {}
        Err(e) => warn!("[settings] set_setting(\"{path}\"): {e}"),
    }
}

/// The whole mirror: every config setting, plus the two families that are not
/// in the config.
pub(crate) fn build_read_state(
    project: Option<&CurrentProject>,
    ui_scale: Option<&GameUiScale>,
    input: Option<&InputMap>,
) -> SettingsReadState {
    let mut state = project
        .map(|p| SettingsReadState::from_config(&p.config))
        .unwrap_or_default();
    if let Some(scale) = ui_scale {
        state.insert(UI_SCALE_PATH, &Value::Float(scale.factor as f64));
    }
    if let Some(map) = input {
        for action in &map.actions {
            for (slot, binding) in action.bindings.iter().enumerate() {
                state.insert(
                    format!("{INPUT_PREFIX}{}.{slot}", action.name),
                    &Value::String(binding.to_spec()),
                );
            }
        }
    }
    state
}

/// Keep [`SettingsReadState`] on every scripted entity, current.
///
/// Rebuilt only when one of its sources changed; a new scripted entity gets
/// the cached copy, so a script's first `get_setting` already has an answer.
fn sync_settings_read_state(
    mut commands: Commands,
    project: Option<Res<CurrentProject>>,
    ui_scale: Option<Res<GameUiScale>>,
    input: Option<Res<InputMap>>,
    mut cache: Local<Option<SettingsReadState>>,
    missing: Query<Entity, (With<ScriptComponent>, Without<SettingsReadState>)>,
    mut present: Query<&mut SettingsReadState, With<ScriptComponent>>,
) {
    let stale = cache.is_none()
        || project.as_ref().is_some_and(|p| p.is_changed())
        || ui_scale.as_ref().is_some_and(|s| s.is_changed())
        || input.as_ref().is_some_and(|m| m.is_changed());
    if stale {
        let fresh = build_read_state(project.as_deref(), ui_scale.as_deref(), input.as_deref());
        for mut state in &mut present {
            state.set_if_neq(fresh.clone());
        }
        *cache = Some(fresh);
    }
    let Some(current) = cache.as_ref() else {
        return;
    };
    for entity in &missing {
        commands.entity(entity).try_insert(current.clone());
    }
}

/// In the editor, remove any mirror that arrived (the Play child's live link
/// can carry one across), so it is never saved into a scene.
fn strip_settings_read_state(mut commands: Commands, q: Query<Entity, With<SettingsReadState>>) {
    for entity in &q {
        commands.entity(entity).try_remove::<SettingsReadState>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_scripting_api::InputAction;

    fn project() -> CurrentProject {
        CurrentProject {
            path: "game".into(),
            config: Default::default(),
        }
    }

    fn targets<'a>(
        project: &'a mut CurrentProject,
        scale: &'a mut GameUiScale,
        input: &'a mut InputMap,
    ) -> SettingTargets<'a> {
        SettingTargets {
            project: Some(project),
            ui_scale: Some(scale),
            input: Some(input),
            editor_session: false,
        }
    }

    #[test]
    fn set_then_get_round_trips_through_the_mirror() {
        let (mut p, mut s, mut i) = (project(), GameUiScale::default(), InputMap::default());

        for (path, value) in [
            ("window.vsync", Value::String("false".into())),
            ("window.max_fps", Value::Float(30.0)),
            ("rendering.render_scale", Value::String("0.5".into())),
            ("rendering.graphics_quality", Value::String("Low".into())),
            ("ui.scale", Value::Float(1.25)),
            ("input.jump.0", Value::String("Key:KeyF".into())),
        ] {
            assert_eq!(
                apply_setting(targets(&mut p, &mut s, &mut i), path, value),
                Ok(true),
                "{path}"
            );
        }

        let state = build_read_state(Some(&p), Some(&s), Some(&i));
        assert_eq!(state.flags.get("window.vsync"), Some(&false));
        assert_eq!(state.numbers.get("window.max_fps"), Some(&30.0));
        assert_eq!(state.numbers.get("rendering.render_scale"), Some(&0.5));
        assert_eq!(state.values.get("rendering.graphics_quality").map(String::as_str), Some("Low"));
        assert_eq!(state.numbers.get("ui.scale"), Some(&1.25));
        assert_eq!(state.values.get("input.jump.0").map(String::as_str), Some("Key:KeyF"));
    }

    /// The frame limiter reads `window.max_fps` from the config every frame,
    /// so landing there is all a limit change needs.
    #[test]
    fn max_fps_lands_in_the_config_the_limiter_reads() {
        let (mut p, mut s, mut i) = (project(), GameUiScale::default(), InputMap::default());
        apply_setting(targets(&mut p, &mut s, &mut i), "window.max_fps", Value::Integer(60)).unwrap();
        assert_eq!(p.config.window.max_fps, 60);
    }

    #[test]
    fn bad_values_are_refused_and_nothing_changes() {
        let (mut p, mut s, mut i) = (project(), GameUiScale::default(), InputMap::default());
        let config = p.config.clone();
        let map = i.clone();
        for (path, value) in [
            ("window.vsync", Value::String("sometimes".into())),
            ("window.no_such_setting", Value::Boolean(true)),
            ("ui.scale", Value::String("big".into())),
            ("input.jump.0", Value::String("Key:NotAKey".into())),
            ("input.fly.0", Value::String("Key:KeyF".into())),
            ("input.jump", Value::String("Key:KeyF".into())),
        ] {
            assert!(
                apply_setting(targets(&mut p, &mut s, &mut i), path, value).is_err(),
                "{path} should be refused"
            );
        }
        assert_eq!(p.config, config);
        assert_eq!(i, map);
        assert_eq!(s, GameUiScale::default());
    }

    #[test]
    fn an_editor_session_never_writes_the_project_config() {
        let (mut p, mut s, mut i) = (project(), GameUiScale::default(), InputMap::default());
        let mut t = targets(&mut p, &mut s, &mut i);
        t.editor_session = true;
        assert!(apply_setting(t, "window.vsync", Value::Boolean(false)).is_err());
        assert!(p.config.window.vsync);
    }

    /// The whole path a Lua `set_setting` takes: the action is observed, the
    /// config changes, and the scripted entity's mirror answers the next
    /// `get_setting` with the new value.
    #[test]
    fn a_set_setting_action_reaches_the_config_and_the_mirror() {
        let mut app = App::new();
        app.insert_resource(project()).init_resource::<InputMap>();
        register(&mut app);
        let entity = app.world_mut().spawn(ScriptComponent::default()).id();
        app.update();
        let mirror = app.world().get::<SettingsReadState>(entity).expect("mirror inserted");
        assert_eq!(mirror.flags.get("window.vsync"), Some(&true));

        app.world_mut().trigger(ScriptAction {
            name: "set_setting".into(),
            entity,
            target_entity: None,
            args: [
                ("path".to_string(), ScriptActionValue::String("window.vsync".into())),
                ("value".to_string(), ScriptActionValue::String("false".into())),
            ]
            .into_iter()
            .collect(),
        });
        assert!(!app.world().resource::<CurrentProject>().config.window.vsync);
        app.update();
        let mirror = app.world().get::<SettingsReadState>(entity).unwrap();
        assert_eq!(mirror.flags.get("window.vsync"), Some(&false));
        assert!(app
            .world()
            .resource::<ScriptExtensions>()
            .bindings()
            .iter()
            .any(|b| b.name == "get_setting"));
    }

    #[test]
    fn an_empty_binding_clears_the_slot() {
        let (mut p, mut s) = (project(), GameUiScale::default());
        let mut i = InputMap {
            actions: vec![InputAction::button(
                "jump",
                vec![InputBinding::Key("Space".into()), InputBinding::GamepadButton("South".into())],
            )],
        };
        apply_setting(targets(&mut p, &mut s, &mut i), "input.jump.0", Value::String(String::new())).unwrap();
        assert_eq!(i.get("jump").unwrap().bindings, vec![InputBinding::GamepadButton("South".into())]);
    }
}
