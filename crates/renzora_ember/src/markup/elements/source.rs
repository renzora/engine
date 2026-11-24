//! What a value element is tied to, and the `bind_2way` glue for each kind of
//! value: on/off, a number, a choice, and text.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use renzora_project_api::settings_path::{self, SettingsReadState, Value};
use renzora_project_api::{CurrentProject, EditorSession};
use renzora_scripting_api::PropertyValue;
use renzora_viewport_api::GameUiScale;

use crate::markup::binding::read_path;
use crate::markup::widgets::write_target;
use crate::reactive::bind_2way;

/// Where an element reads and writes its value.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Source {
    /// A project setting by its dotted path, or `ui.scale` / `language`.
    Setting(String),
    /// A script variable on the template's host entity.
    Var { host: Entity, path: String },
    /// Neither attribute: the element works on its own and is read by nothing.
    None,
}

impl Source {
    /// `setting="..."` wins over `bind="..."`.
    pub(crate) fn from_attrs(defs: &HashMap<String, String>, host: Entity) -> Self {
        if let Some(path) = defs.get("setting") {
            Self::Setting(path.clone())
        } else if let Some(path) = defs.get("bind") {
            Self::Var { host, path: path.clone() }
        } else {
            Self::None
        }
    }
}

/// Every project setting, flattened, for elements to read.
///
/// A binding reads its value every frame, and reading one setting by path
/// serializes the whole project config, so a menu of a dozen elements did
/// that a dozen times a frame. This is rebuilt once, when the project changes.
#[derive(Resource, Default)]
pub(crate) struct ElementSettings(pub SettingsReadState);

pub(crate) fn refresh_settings(project: Res<CurrentProject>, mut settings: ResMut<ElementSettings>) {
    settings.0 = SettingsReadState::from_config(&project.config);
}

/// The source's value as text, the form every kind can be read from.
pub(crate) fn read_text(world: &World, source: &Source) -> Option<String> {
    match source {
        Source::Setting(path) => match path.as_str() {
            "language" => Some(renzora_lang_api::active_code()),
            "ui.scale" => world.get_resource::<GameUiScale>().map(|s| s.factor.to_string()),
            _ => world.get_resource::<ElementSettings>()?.0.values.get(path).cloned(),
        },
        Source::Var { host, path } => read_path(world, *host, path),
        Source::None => None,
    }
}

/// Write a new value, in a game only. In the editor an element shows its value
/// and never writes it: a setting there is the project's own saved value and a
/// script variable is saved with the scene, so a click while laying out a menu
/// would otherwise change the project.
pub(crate) fn write(world: &mut World, source: &Source, value: Value) {
    if world.get_resource::<EditorSession>().is_some_and(|s| s.is_editor()) {
        return;
    }
    match source {
        Source::Setting(path) => match path.as_str() {
            "language" => renzora_lang_api::set_active(&settings_path::value_text(&value)),
            "ui.scale" => {
                if let Some(f) = number_of(&value) {
                    world.get_resource_or_insert_with(GameUiScale::default).set(f);
                }
            }
            _ => {
                let Some(mut project) = world.get_resource_mut::<CurrentProject>() else { return };
                if let Err(e) = settings_path::set_setting(&mut project.config, path, value) {
                    warn!("[markup] `{path}` could not be set: {e}");
                }
            }
        },
        Source::Var { host, path } => {
            let value = match value {
                Value::Boolean(b) => PropertyValue::Bool(b),
                Value::Float(f) => PropertyValue::Float(f as f32),
                Value::Integer(i) => PropertyValue::Float(i as f32),
                other => PropertyValue::String(settings_path::value_text(&other)),
            };
            write_target(world, *host, path, value);
        }
        Source::None => {}
    }
}

fn number_of(value: &Value) -> Option<f32> {
    match value {
        Value::Float(f) => Some(*f as f32),
        Value::Integer(i) => Some(*i as f32),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn truthy(text: &str) -> bool {
    let t = text.trim();
    !(t.is_empty() || t.eq_ignore_ascii_case("false") || t == "0")
}

// ── Binding each kind of value ──────────────────────────────────────────────

/// An on/off widget.
pub(crate) fn bind_bool(commands: &mut Commands, widget: Entity, source: Source) {
    if source == Source::None {
        return;
    }
    let read = source.clone();
    bind_2way(
        commands,
        widget,
        move |w: &World| read_text(w, &read).is_some_and(|t| truthy(&t)),
        move |w: &mut World, on: &bool| write(w, &source, Value::Boolean(*on)),
    );
}

/// How a number widget's own value relates to the value it is tied to.
#[derive(Clone, Copy)]
pub(crate) struct NumberMap {
    /// The tied value's range, when the widget works in 0..1 (fader, knob).
    pub unit_range: Option<(f32, f32)>,
    /// Snap the tied value to this, so a drag writes only when it crosses a
    /// step rather than on every pixel.
    pub step: f32,
}

/// A number widget.
pub(crate) fn bind_number(commands: &mut Commands, widget: Entity, source: Source, map: NumberMap) {
    if source == Source::None {
        return;
    }
    let read = source.clone();
    let parse = |t: String| t.trim().parse::<f32>().ok();
    bind_2way(
        commands,
        widget,
        move |w: &World| {
            let v = read_text(w, &read).and_then(parse).unwrap_or(0.0);
            match map.unit_range {
                Some((min, max)) if (max - min).abs() > f32::EPSILON => ((v - min) / (max - min)).clamp(0.0, 1.0),
                _ => v,
            }
        },
        move |w: &mut World, v: &f32| {
            let mut v = match map.unit_range {
                Some((min, max)) => min + (max - min) * v,
                None => *v,
            };
            if map.step > 0.0 {
                v = (v / map.step).round() * map.step;
            }
            if read_text(w, &source).and_then(parse) != Some(v) {
                write(w, &source, Value::Float(v as f64));
            }
        },
    );
}

/// A widget that picks one of `values`, by index.
pub(crate) fn bind_choice(commands: &mut Commands, widget: Entity, source: Source, values: Vec<String>) {
    if source == Source::None {
        return;
    }
    let (read, read_values) = (source.clone(), values.clone());
    bind_2way(
        commands,
        widget,
        move |w: &World| {
            let current = read_text(w, &read).unwrap_or_default();
            read_values.iter().position(|v| *v == current).unwrap_or(0)
        },
        move |w: &mut World, i: &usize| {
            if let Some(value) = values.get(*i) {
                if read_text(w, &source).as_deref() != Some(value) {
                    write(w, &source, Value::String(value.clone()));
                }
            }
        },
    );
}

/// A text widget.
pub(crate) fn bind_text(commands: &mut Commands, widget: Entity, source: Source) {
    if source == Source::None {
        return;
    }
    let read = source.clone();
    bind_2way(
        commands,
        widget,
        move |w: &World| read_text(w, &read).unwrap_or_default(),
        move |w: &mut World, text: &String| {
            if read_text(w, &source).as_deref() != Some(text) {
                write(w, &source, Value::String(text.clone()));
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defs(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn setting_wins_over_bind_and_neither_is_allowed() {
        let host = Entity::from_raw_u32(7).unwrap();
        assert_eq!(
            Source::from_attrs(&defs(&[("setting", "window.vsync"), ("bind", "v")]), host),
            Source::Setting("window.vsync".into())
        );
        assert_eq!(Source::from_attrs(&defs(&[("bind", "v")]), host), Source::Var { host, path: "v".into() });
        assert_eq!(Source::from_attrs(&defs(&[]), host), Source::None);
    }

    /// In the editor an element shows its setting and never changes it.
    #[test]
    fn the_editor_never_writes_a_setting() {
        let mut world = World::new();
        world.insert_resource(EditorSession(true));
        world.insert_resource(CurrentProject { path: "p".into(), config: Default::default() });
        let before = world.resource::<CurrentProject>().config.window.vsync;
        write(&mut world, &Source::Setting("window.vsync".into()), Value::Boolean(!before));
        assert_eq!(world.resource::<CurrentProject>().config.window.vsync, before);

        world.insert_resource(EditorSession(false));
        write(&mut world, &Source::Setting("window.vsync".into()), Value::Boolean(!before));
        assert_eq!(world.resource::<CurrentProject>().config.window.vsync, !before);
    }
}
