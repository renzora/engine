//! Reading and writing one project setting by a dotted path, for scripts.
//!
//! `CurrentProject.config` is the running game's live settings store: the
//! runtime's systems read it, so changing a field there changes the game. A
//! Rust script reaches it directly (`ctx.world().resource_mut::<CurrentProject>()`).
//! A Lua script cannot name a Rust struct, so it gets
//! `set_setting("window.vsync", false)` and `get_setting("window.vsync")`, and
//! this module is what those two do.
//!
//! # Why the serde form and not reflection
//!
//! [`ProjectConfig`] is serde, not `Reflect`, and it is what `project.toml`
//! already round-trips through. Going through its TOML value means a path is
//! exactly the key a user sees in `project.toml` (`rendering.render_scale`),
//! every field added to the config later is reachable with no list to keep up
//! to date, and validation is free: the edited value is deserialized back into
//! a `ProjectConfig`, so a wrong type, an unknown enum label or a negative
//! frame rate is refused by the same rules that would refuse it in the file.
//!
//! # What it never does
//!
//! Write `project.toml`. A setting a game changes lives as long as the game
//! does; saving a player's choices is the game's own job (`renzora::save`).
//!
//! Arrays of tables that carry a `key` or `name` (the mixer's buses) are
//! addressed by that, so `audio.buses.Master.volume` reads like the mixer
//! panel does; a number addresses by position instead.

use std::collections::HashMap;

use bevy::prelude::*;
/// The value type every function here speaks, re-exported so a caller can
/// build one without a `toml` dependency of its own.
pub use toml::Value;

use crate::ProjectConfig;

/// Top-level keys a script may not write.
///
/// The two `editor_*` fields are deserialized for migration but never
/// serialized, so a write would land in the config with no file behind it and
/// be picked up by the editor's next save. `profiling_key` is the editor's
/// pairing secret for a profiling export; a game rewriting it could hand its
/// companion port to anyone.
const READ_ONLY: &[&str] = &["editor_last_scene", "editor_open_tabs", "profiling_key"];

/// Why a setting could not be read or written.
#[derive(Clone, Debug, PartialEq)]
pub enum SettingError {
    /// The path names no setting.
    Unknown(String),
    /// The setting exists but scripts may not change it.
    ReadOnly(String),
    /// The value does not fit the setting.
    Invalid { path: String, reason: String },
}

impl std::fmt::Display for SettingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown(p) => write!(f, "no setting `{p}`"),
            Self::ReadOnly(p) => write!(f, "`{p}` cannot be changed by a script"),
            Self::Invalid { path, reason } => write!(f, "bad value for `{path}`: {reason}"),
        }
    }
}

/// The config as a TOML tree. `None` only if serialization itself fails,
/// which a config that loaded from a `project.toml` cannot do.
fn to_value(config: &ProjectConfig) -> Option<Value> {
    Value::try_from(config).ok()
}

fn segments(path: &str) -> Vec<&str> {
    path.split('.').map(str::trim).filter(|s| !s.is_empty()).collect()
}

/// Which element of an array a path segment names: a position, or the element
/// whose `key` (then `name`) equals it.
fn array_index(items: &[Value], seg: &str) -> Option<usize> {
    if let Ok(i) = seg.parse::<usize>() {
        return (i < items.len()).then_some(i);
    }
    ["key", "name"].iter().find_map(|field| {
        items
            .iter()
            .position(|v| v.get(field).and_then(Value::as_str) == Some(seg))
    })
}

fn child<'a>(v: &'a Value, seg: &str) -> Option<&'a Value> {
    match v {
        Value::Table(t) => t.get(seg),
        Value::Array(a) => array_index(a, seg).map(|i| &a[i]),
        _ => None,
    }
}

/// Read the setting at `path`, or `None` if there is none.
///
/// An optional setting that is unset (`ui_font` with no font chosen) has no
/// value, so it reads as `None` too.
pub fn get_setting(config: &ProjectConfig, path: &str) -> Option<Value> {
    let root = to_value(config)?;
    let mut cur = &root;
    for seg in segments(path) {
        cur = child(cur, seg)?;
    }
    Some(cur.clone())
}

/// Write `value` to the setting at `path`. `Ok(true)` if the config changed,
/// `Ok(false)` if it already held that value.
///
/// `value` is converted to the setting's own type first (see [`coerce`]), so a
/// script may pass `"60"` or `60.0` for an integer frame rate. The result must
/// then deserialize into a `ProjectConfig`, and must read back from the path it
/// was written to; a path that names a field the config does not have would
/// otherwise be accepted and silently dropped, since serde ignores unknown
/// keys. On any error `config` is left exactly as it was.
pub fn set_setting(config: &mut ProjectConfig, path: &str, value: Value) -> Result<bool, SettingError> {
    let segs = segments(path);
    if segs.is_empty() {
        return Err(SettingError::Unknown(path.to_string()));
    }
    if READ_ONLY.contains(&segs[0]) {
        return Err(SettingError::ReadOnly(path.to_string()));
    }
    // A setting at its default is often left out of the serialized config
    // (`max_fps = 0` is skipped), so there may be no existing value to take
    // the type from. Then each reading of the value is tried in turn and the
    // first one the config accepts wins; the first error is the one reported.
    let candidates = match get_setting(config, path) {
        Some(existing) => vec![coerce(Some(&existing), value)],
        None => readings(value),
    };
    let mut first_err = None;
    for candidate in candidates {
        match write_one(config, path, &segs, candidate) {
            Ok(changed) => return Ok(changed),
            Err(e) => {
                first_err.get_or_insert(e);
            }
        }
    }
    Err(first_err.unwrap_or_else(|| SettingError::Unknown(path.to_string())))
}

/// The ways a value with no existing setting to match could be meant: as
/// given first, then text read as a bool or a number, a whole float as an
/// integer, an integer as a float.
fn readings(value: Value) -> Vec<Value> {
    let mut out = vec![value.clone()];
    match &value {
        Value::String(s) => {
            let t = s.trim();
            if let Ok(b) = t.parse::<bool>() {
                out.push(Value::Boolean(b));
            }
            if let Ok(i) = t.parse::<i64>() {
                out.push(Value::Integer(i));
            }
            if let Ok(f) = t.parse::<f64>() {
                if let Some(i) = whole(f) {
                    out.push(Value::Integer(i));
                }
                out.push(Value::Float(f));
            }
        }
        Value::Float(f) => {
            if let Some(i) = whole(*f) {
                out.push(Value::Integer(i));
            }
        }
        Value::Integer(i) => out.push(Value::Float(*i as f64)),
        _ => {}
    }
    out
}

/// One attempt of [`set_setting`] with an already-converted value.
fn write_one(config: &mut ProjectConfig, path: &str, segs: &[&str], value: Value) -> Result<bool, SettingError> {
    let Some((last, parents)) = segs.split_last() else {
        return Err(SettingError::Unknown(path.to_string()));
    };
    let invalid = |reason: String| SettingError::Invalid {
        path: path.to_string(),
        reason,
    };

    let mut root = to_value(config).ok_or_else(|| invalid("the config cannot be serialized".into()))?;
    let mut cur = &mut root;
    for seg in parents {
        cur = match cur {
            // A missing table is created rather than refused, so an optional
            // section that is unset (`network`) can be written field by field;
            // deserializing below refuses it if that leaves it incomplete.
            Value::Table(t) => t
                .entry(seg.to_string())
                .or_insert_with(|| Value::Table(Default::default())),
            Value::Array(a) => {
                let i = array_index(a, seg).ok_or_else(|| SettingError::Unknown(path.to_string()))?;
                &mut a[i]
            }
            _ => return Err(SettingError::Unknown(path.to_string())),
        };
    }
    let coerced = match cur {
        Value::Table(t) => {
            let v = coerce(t.get(*last), value);
            t.insert(last.to_string(), v.clone());
            v
        }
        Value::Array(a) => {
            let i = array_index(a, last).ok_or_else(|| SettingError::Unknown(path.to_string()))?;
            let v = coerce(Some(&a[i]), value);
            a[i] = v.clone();
            v
        }
        _ => return Err(SettingError::Unknown(path.to_string())),
    };

    let mut updated: ProjectConfig = root.try_into().map_err(|e: toml::de::Error| invalid(e.message().to_string()))?;
    // Never serialized, so the round trip dropped them; they are not ours to
    // clear.
    updated.editor_last_scene = config.editor_last_scene.clone();
    updated.editor_open_tabs = config.editor_open_tabs.clone();

    match get_setting(&updated, path) {
        Some(read_back) if same_value(&read_back, &coerced) => {}
        _ => return Err(SettingError::Unknown(path.to_string())),
    }
    if updated == *config {
        return Ok(false);
    }
    *config = updated;
    Ok(true)
}

/// Equal, treating floats as `f32`: most config floats are `f32`, which TOML
/// widens to `f64` on the way out, so `0.8` written reads back as
/// `0.800000011920929`.
fn same_value(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Float(x), Value::Float(y)) => (*x as f32) == (*y as f32),
        _ => a == b,
    }
}

/// Convert `value` to the type of the setting it replaces.
///
/// Scripts are loosely typed: Lua has one number type, and a declared binding
/// whose argument is text hands every value over as a string. So a string is
/// parsed as the existing setting's type (`"false"` for a bool, `"60"` for an
/// integer), a whole float becomes an integer where one is expected, and a
/// number or bool becomes text where text is expected. Anything that does not
/// convert is passed through unchanged, for deserialization to refuse with a
/// proper message. With no existing value (an unset optional setting) the value
/// goes in as given.
pub fn coerce(existing: Option<&Value>, value: Value) -> Value {
    let Some(existing) = existing else {
        return value;
    };
    match (existing, value) {
        (Value::Boolean(_), Value::String(s)) => match s.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "on" | "yes" => Value::Boolean(true),
            "false" | "0" | "off" | "no" => Value::Boolean(false),
            _ => Value::String(s),
        },
        (Value::Boolean(_), Value::Integer(i)) if i == 0 || i == 1 => Value::Boolean(i == 1),
        (Value::Integer(_), Value::String(s)) => {
            let t = s.trim();
            if let Ok(i) = t.parse::<i64>() {
                Value::Integer(i)
            } else if let Some(i) = t.parse::<f64>().ok().and_then(whole) {
                Value::Integer(i)
            } else {
                Value::String(s)
            }
        }
        (Value::Integer(_), Value::Float(f)) => whole(f).map_or(Value::Float(f), Value::Integer),
        (Value::Float(_), Value::String(s)) => s.trim().parse::<f64>().map_or(Value::String(s), Value::Float),
        (Value::Float(_), Value::Integer(i)) => Value::Float(i as f64),
        (Value::String(_), Value::Integer(i)) => Value::String(i.to_string()),
        (Value::String(_), Value::Float(f)) => Value::String(value_text(&Value::Float(f))),
        (Value::String(_), Value::Boolean(b)) => Value::String(b.to_string()),
        (_, v) => v,
    }
}

fn whole(f: f64) -> Option<i64> {
    (f.fract() == 0.0 && f.is_finite()).then_some(f as i64)
}

/// A value as text, the way `get_setting` returns it: a string as itself, a
/// number without the `f64` noise an `f32` field picks up in TOML.
pub fn value_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Float(f) => (*f as f32).to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Boolean(b) => b.to_string(),
        other => other.to_string(),
    }
}

/// Every setting the config holds, as `(path, value)`, with arrays of tables
/// addressed by `key`/`name` where they have one.
pub fn setting_leaves(config: &ProjectConfig) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    if let Some(root) = to_value(config) {
        flatten("", &root, &mut out);
    }
    out
}

fn flatten(prefix: &str, v: &Value, out: &mut Vec<(String, Value)>) {
    let join = |seg: &str| {
        if prefix.is_empty() {
            seg.to_string()
        } else {
            format!("{prefix}.{seg}")
        }
    };
    match v {
        Value::Table(t) => {
            for (k, child) in t {
                flatten(&join(k), child, out);
            }
        }
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                let label = ["key", "name"]
                    .iter()
                    .find_map(|f| item.get(f).and_then(Value::as_str).map(str::to_string))
                    .unwrap_or_else(|| i.to_string());
                flatten(&join(&label), item, out);
            }
        }
        leaf => out.push((prefix.to_string(), leaf.clone())),
    }
}

/// A read-only mirror of the settings, for the `get_setting` family of script
/// functions.
///
/// A declared script read resolves as a reflected field on the calling
/// script's own entity: that is the only synchronous read a language backend
/// has. So the runtime keeps one of these on every scripted entity in a game,
/// refreshed when a setting changes, and `get_setting(path)` reads
/// `values["<path>"]`. Three maps because a reflected read returns one
/// concrete type: `values` holds every setting as text, `numbers` the numeric
/// ones, `flags` the true/false ones.
///
/// Runtime only, never authored: the runtime does not insert it in an editor
/// session, and strips one that arrives there.
#[derive(Component, Clone, Debug, Default, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct SettingsReadState {
    pub values: HashMap<String, String>,
    pub numbers: HashMap<String, f32>,
    pub flags: HashMap<String, bool>,
}

impl SettingsReadState {
    /// Record one setting under `path`, in every map its type belongs in.
    pub fn insert(&mut self, path: impl Into<String>, value: &Value) {
        let path = path.into();
        match value {
            Value::Boolean(b) => {
                self.flags.insert(path.clone(), *b);
            }
            Value::Integer(i) => {
                self.numbers.insert(path.clone(), *i as f32);
            }
            Value::Float(f) => {
                self.numbers.insert(path.clone(), *f as f32);
            }
            _ => {}
        }
        self.values.insert(path, value_text(value));
    }

    /// The mirror of a whole config.
    pub fn from_config(config: &ProjectConfig) -> Self {
        let mut state = Self::default();
        for (path, value) in setting_leaves(config) {
            state.insert(path, &value);
        }
        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::reflect::GetPath;

    fn config() -> ProjectConfig {
        ProjectConfig::default()
    }

    #[test]
    fn several_paths_round_trip() {
        let mut c = config();

        assert_eq!(set_setting(&mut c, "window.vsync", Value::Boolean(false)), Ok(true));
        assert!(!c.window.vsync);
        assert_eq!(get_setting(&c, "window.vsync"), Some(Value::Boolean(false)));

        // A Lua number is a float; the frame rate is an integer.
        assert_eq!(set_setting(&mut c, "window.max_fps", Value::Float(60.0)), Ok(true));
        assert_eq!(c.window.max_fps, 60);
        assert_eq!(get_setting(&c, "window.max_fps"), Some(Value::Integer(60)));

        assert_eq!(set_setting(&mut c, "rendering.render_scale", Value::Float(0.75)), Ok(true));
        assert_eq!(c.rendering.render_scale, 0.75);

        assert_eq!(
            set_setting(&mut c, "rendering.graphics_quality", Value::String("High".into())),
            Ok(true)
        );
        assert_eq!(c.rendering.graphics_quality, crate::GraphicsQuality::High);
        assert_eq!(
            get_setting(&c, "rendering.graphics_quality").map(|v| value_text(&v)),
            Some("High".to_string())
        );
    }

    #[test]
    fn text_values_are_parsed_as_the_settings_own_type() {
        // What a declared `set_setting(path, value)` with a text argument
        // delivers for every value.
        let mut c = config();
        set_setting(&mut c, "window.vsync", Value::String("false".into())).unwrap();
        assert!(!c.window.vsync);
        set_setting(&mut c, "window.max_fps", Value::String("144".into())).unwrap();
        assert_eq!(c.window.max_fps, 144);
        set_setting(&mut c, "rendering.render_scale", Value::String("0.5".into())).unwrap();
        assert_eq!(c.rendering.render_scale, 0.5);
    }

    #[test]
    fn an_unchanged_value_reports_no_change() {
        let mut c = config();
        let vsync = c.window.vsync;
        assert_eq!(set_setting(&mut c, "window.vsync", Value::Boolean(vsync)), Ok(false));
    }

    #[test]
    fn a_value_of_the_wrong_kind_is_refused_and_nothing_changes() {
        let mut c = config();
        let before = c.clone();
        for (path, value) in [
            ("window.vsync", Value::String("maybe".into())),
            ("window.max_fps", Value::Integer(-5)),
            ("window.max_fps", Value::Float(59.5)),
            ("rendering.graphics_quality", Value::String("Ultra".into())),
        ] {
            assert!(
                matches!(set_setting(&mut c, path, value), Err(SettingError::Invalid { .. })),
                "{path} should be refused"
            );
        }
        assert_eq!(c, before);
    }

    #[test]
    fn a_path_naming_no_setting_is_refused() {
        let mut c = config();
        let before = c.clone();
        assert_eq!(
            set_setting(&mut c, "window.not_a_field", Value::Boolean(true)),
            Err(SettingError::Unknown("window.not_a_field".into()))
        );
        assert_eq!(
            set_setting(&mut c, "window.vsync.deeper", Value::Boolean(true)),
            Err(SettingError::Unknown("window.vsync.deeper".into()))
        );
        assert_eq!(c, before);
        assert_eq!(get_setting(&c, "window.nope"), None);
    }

    #[test]
    fn editor_only_and_secret_keys_are_read_only() {
        let mut c = config();
        assert_eq!(
            set_setting(&mut c, "profiling_key", Value::String("x".into())),
            Err(SettingError::ReadOnly("profiling_key".into()))
        );
        assert_eq!(
            set_setting(&mut c, "editor_last_scene", Value::String("x".into())),
            Err(SettingError::ReadOnly("editor_last_scene".into()))
        );
    }

    #[test]
    fn a_write_keeps_the_fields_that_never_serialize() {
        let mut c = config();
        c.editor_last_scene = Some("scenes/level.bsn".into());
        set_setting(&mut c, "window.vsync", Value::Boolean(false)).unwrap();
        assert_eq!(c.editor_last_scene.as_deref(), Some("scenes/level.bsn"));
    }

    #[test]
    fn mixer_buses_are_addressed_by_their_key() {
        let mut c = config();
        // From TOML rather than a struct literal, so a field added to the bus
        // later does not break this test.
        let master: crate::BusConfig = toml::from_str("key = \"Master\"\nvolume = 1.0").unwrap();
        c.audio.buses.push(master);
        set_setting(&mut c, "audio.buses.Master.volume", Value::Float(0.25)).unwrap();
        assert_eq!(c.audio.buses[0].volume, 0.25);
        assert_eq!(get_setting(&c, "audio.buses.0.volume"), Some(Value::Float(0.25)));
        assert!(set_setting(&mut c, "audio.buses.Music.volume", Value::Float(0.5)).is_err());
    }

    #[test]
    fn the_mirror_answers_a_reflected_read_by_path() {
        // `get_setting(path)` is declared as a read of `values["{0}"]`; prove
        // that path resolves through Bevy's reflection the way the script
        // backend resolves it.
        let mut c = config();
        c.window.max_fps = 30;
        let state = SettingsReadState::from_config(&c);
        let text = state
            .reflect_path(r#"values["window.max_fps"]"#)
            .ok()
            .and_then(|v| v.try_downcast_ref::<String>().cloned());
        assert_eq!(text.as_deref(), Some("30"));
        let number = state
            .reflect_path(r#"numbers["window.max_fps"]"#)
            .ok()
            .and_then(|v| v.try_downcast_ref::<f32>().copied());
        assert_eq!(number, Some(30.0));
        let flag = state
            .reflect_path(r#"flags["window.vsync"]"#)
            .ok()
            .and_then(|v| v.try_downcast_ref::<bool>().copied());
        assert_eq!(flag, Some(c.window.vsync));
    }
}
