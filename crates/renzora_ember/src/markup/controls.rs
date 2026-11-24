//! Ember widgets declared in markup: `<dropdown>`, `<switch>`, `<checkbox>`
//! and `<slider>`.
//!
//! ```html
//! <dropdown setting="window.mode" options="Windowed,Fullscreen,Borderless" />
//! <dropdown setting="window.max_fps" options="Unlimited,30 FPS,60 FPS" values="0,30,60" />
//! <switch setting="window.vsync" />
//! <slider setting="audio.buses.Music.volume" min="0" max="1" step="0.01" />
//! <slider setting="ui.scale" min="0.75" max="1.5" step="0.05" />
//! <dropdown setting="language" />
//! <checkbox bind="show_hints" />
//! ```
//!
//! Each is the same ember widget the editor's panels are built from, so a
//! template shows real controls wherever it is drawn: in the game, in the
//! editor's viewport and in the UI editor. Building them from a script instead
//! left the editor with empty slots, because scripts do not run while editing.
//!
//! **What a control is tied to.** `setting="<path>"` is a project setting, the
//! dotted path `set_setting` takes (`window.vsync`, `rendering.render_scale`),
//! plus two that are not in the project config: `ui.scale` (the player's UI
//! size, `GameUiScale`) and `language` (the active language, whose dropdown
//! fills itself from the loaded packs when it has no `options`).
//! `bind="<var>"` is a variable on the host's script, as with `<input>`.
//!
//! **The editor shows, the game changes.** In the editor a control displays
//! its value and does not write it: a setting there is the project's own
//! saved value and a script variable is saved with the scene, so a click
//! while laying out a menu would otherwise change the project. A game writes
//! straight into the runtime's settings, which apply at once.
//!
//! The loader styles the element like any node (`width`, `margin`, ...) and
//! stamps a [`MarkupControl`]; [`build_controls`] puts the widget inside it once
//! the UI fonts exist, which is a few frames after startup.

use bevy::ecs::world::CommandQueue;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use renzora_project_api::settings_path::{self, SettingsReadState, Value};
use renzora_project_api::{CurrentProject, EditorSession};
use renzora_scripting_api::PropertyValue;
use renzora_viewport_api::GameUiScale;

use crate::font::EmberFonts;
use crate::markup::binding::read_path;
use crate::markup::widgets::write_target;
use crate::reactive::bind_2way;
use crate::widgets::{checkbox, dropdown, slider_ranged, toggle_switch};

/// The markup tag names this module builds, and what each becomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ControlKind {
    Dropdown,
    Switch,
    Checkbox,
    Slider,
}

impl ControlKind {
    /// The control a tag name stands for.
    pub(crate) fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "dropdown" => Some(Self::Dropdown),
            "switch" => Some(Self::Switch),
            "checkbox" => Some(Self::Checkbox),
            "slider" => Some(Self::Slider),
            _ => None,
        }
    }
}

/// Where a control reads and writes its value.
#[derive(Clone, Debug, PartialEq)]
enum Source {
    /// A project setting by its dotted path, or `ui.scale` / `language`.
    Setting(String),
    /// A script variable on the template's host entity.
    Var { host: Entity, path: String },
    /// Neither attribute: the control works on its own and is read by nothing.
    None,
}

/// A markup element waiting for its ember widget. Stamped by the loader.
#[derive(Component, Clone, Debug)]
pub(crate) struct MarkupControl {
    kind: ControlKind,
    source: Source,
    defs: HashMap<String, String>,
}

impl MarkupControl {
    pub(crate) fn new(kind: ControlKind, defs: &HashMap<String, String>, host: Entity) -> Self {
        let source = if let Some(path) = defs.get("setting") {
            Source::Setting(path.clone())
        } else if let Some(path) = defs.get("bind") {
            Source::Var { host, path: path.clone() }
        } else {
            Source::None
        };
        Self { kind, source, defs: defs.clone() }
    }

    fn number(&self, key: &str, default: f32) -> f32 {
        self.defs.get(key).and_then(|v| v.trim().parse().ok()).unwrap_or(default)
    }
}

/// The widget has been built into this element.
#[derive(Component)]
struct ControlBuilt;

/// Every project setting, flattened, for the controls to read.
///
/// A binding reads its value every frame, and reading one setting by path
/// serializes the whole project config, so a menu of a dozen controls did that
/// a dozen times a frame. This is rebuilt once, when the project changes.
#[derive(Resource, Default)]
struct ControlSettings(SettingsReadState);

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<ControlSettings>().add_systems(
        Update,
        (
            refresh_control_settings.run_if(resource_exists_and_changed::<CurrentProject>),
            build_controls,
        )
            .chain(),
    );
}

fn refresh_control_settings(project: Res<CurrentProject>, mut settings: ResMut<ControlSettings>) {
    settings.0 = SettingsReadState::from_config(&project.config);
}

/// Build the ember widget into every new control and tie it to its value.
fn build_controls(world: &mut World) {
    let pending: Vec<(Entity, MarkupControl)> = {
        let mut q = world.query_filtered::<(Entity, &MarkupControl), Without<ControlBuilt>>();
        q.iter(world).map(|(e, c)| (e, c.clone())).collect()
    };
    if pending.is_empty() {
        return;
    }
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };

    let mut queue = CommandQueue::default();
    let mut sliders = Vec::new();
    {
        let mut commands = Commands::new(&mut queue, world);
        for (element, control) in pending {
            let widget = match control.kind {
                ControlKind::Switch => {
                    let w = toggle_switch(&mut commands, false);
                    bind_bool(&mut commands, w, control.source.clone());
                    w
                }
                ControlKind::Checkbox => {
                    let w = checkbox(&mut commands, false);
                    bind_bool(&mut commands, w, control.source.clone());
                    w
                }
                ControlKind::Slider => {
                    let (min, max) = (control.number("min", 0.0), control.number("max", 1.0));
                    let step = control.number("step", 0.0);
                    let w = slider_ranged(&mut commands, min, min, max);
                    bind_number(&mut commands, w, control.source.clone(), step);
                    sliders.push(w);
                    w
                }
                ControlKind::Dropdown => {
                    let (labels, values) = dropdown_options(&control);
                    let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
                    let w = dropdown(&mut commands, &fonts, &refs, 0);
                    bind_choice(&mut commands, w, control.source.clone(), values);
                    w
                }
            };
            commands.entity(element).add_child(widget).insert(ControlBuilt);
        }
    }
    queue.apply(world);

    // The ember slider is a fixed width for the editor's inspector rows; in
    // markup it fills the element, whose own `width` sets the size.
    for slider in sliders {
        if let Some(mut node) = world.get_mut::<Node>(slider) {
            node.width = Val::Percent(100.0);
        }
    }
}

/// A dropdown's labels and the value each stands for. `values` defaults to the
/// labels; a `language` dropdown with no `options` lists the loaded packs.
fn dropdown_options(control: &MarkupControl) -> (Vec<String>, Vec<String>) {
    let split = |s: &String| s.split(',').map(|p| p.trim().to_string()).collect::<Vec<_>>();
    if control.source == Source::Setting("language".into()) && !control.defs.contains_key("options") {
        let packs = renzora_lang_api::available();
        return (
            packs.iter().map(|p| p.name.clone()).collect(),
            packs.iter().map(|p| p.code.clone()).collect(),
        );
    }
    let labels = control.defs.get("options").map(split).unwrap_or_default();
    let values = control.defs.get("values").map(split).unwrap_or_else(|| labels.clone());
    (labels, values)
}

// ── Reading and writing a source ────────────────────────────────────────────

/// The source's value as text, the form every kind can be read from.
fn read_text(world: &World, source: &Source) -> Option<String> {
    match source {
        Source::Setting(path) => match path.as_str() {
            "language" => Some(renzora_lang_api::active_code()),
            "ui.scale" => world.get_resource::<GameUiScale>().map(|s| s.factor.to_string()),
            _ => world.get_resource::<ControlSettings>()?.0.values.get(path).cloned(),
        },
        Source::Var { host, path } => read_path(world, *host, path),
        Source::None => None,
    }
}

/// Write a new value, in a game only. See the module docs.
fn write(world: &mut World, source: &Source, value: Value) {
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

// ── Binding each kind ───────────────────────────────────────────────────────

fn bind_bool(commands: &mut Commands, widget: Entity, source: Source) {
    let read = source.clone();
    bind_2way(
        commands,
        widget,
        move |w: &World| read_text(w, &read).is_some_and(|t| truthy(&t)),
        move |w: &mut World, on: &bool| write(w, &source, Value::Boolean(*on)),
    );
}

fn bind_number(commands: &mut Commands, widget: Entity, source: Source, step: f32) {
    let read = source.clone();
    bind_2way(
        commands,
        widget,
        move |w: &World| read_text(w, &read).and_then(|t| t.trim().parse().ok()).unwrap_or(0.0),
        move |w: &mut World, v: &f32| {
            // Snapped, so a drag writes only when it crosses a step rather than
            // on every pixel of the drag.
            let v = if step > 0.0 { (v / step).round() * step } else { *v };
            if read_text(w, &source).and_then(|t| t.trim().parse::<f32>().ok()) != Some(v) {
                write(w, &source, Value::Float(v as f64));
            }
        },
    );
}

fn bind_choice(commands: &mut Commands, widget: Entity, source: Source, values: Vec<String>) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_name_their_controls() {
        assert_eq!(ControlKind::from_tag("dropdown"), Some(ControlKind::Dropdown));
        assert_eq!(ControlKind::from_tag("switch"), Some(ControlKind::Switch));
        assert_eq!(ControlKind::from_tag("checkbox"), Some(ControlKind::Checkbox));
        assert_eq!(ControlKind::from_tag("slider"), Some(ControlKind::Slider));
        assert_eq!(ControlKind::from_tag("node"), None);
    }

    fn defs(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn setting_wins_over_bind_and_neither_is_allowed() {
        let host = Entity::from_raw_u32(7).unwrap();
        let both = MarkupControl::new(ControlKind::Switch, &defs(&[("setting", "window.vsync"), ("bind", "v")]), host);
        assert_eq!(both.source, Source::Setting("window.vsync".into()));
        let var = MarkupControl::new(ControlKind::Switch, &defs(&[("bind", "v")]), host);
        assert_eq!(var.source, Source::Var { host, path: "v".into() });
        let none = MarkupControl::new(ControlKind::Switch, &defs(&[]), host);
        assert_eq!(none.source, Source::None);
    }

    #[test]
    fn dropdown_values_default_to_their_labels() {
        let host = Entity::from_raw_u32(1).unwrap();
        let plain = MarkupControl::new(ControlKind::Dropdown, &defs(&[("options", "A, B ,C")]), host);
        assert_eq!(dropdown_options(&plain), (vec!["A".into(), "B".into(), "C".into()], vec!["A".into(), "B".into(), "C".into()]));
        let mapped = MarkupControl::new(
            ControlKind::Dropdown,
            &defs(&[("options", "Unlimited,30 FPS"), ("values", "0,30")]),
            host,
        );
        assert_eq!(dropdown_options(&mapped).1, vec!["0".to_string(), "30".to_string()]);
    }

    /// In the editor a control shows its setting and never changes it.
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
