//! Cross-system keyed value store.
//!
//! `GlobalStore` is a `Resource` that any subsystem (blueprint, script) can
//! read and write. A `GlobalChanged` event fires once per frame for every key that was
//! mutated since the last drain.

use bevy::prelude::*;
use renzora::PinValue;
use std::collections::{HashMap, HashSet};

#[derive(Resource, Default)]
pub struct GlobalStore {
    values: HashMap<String, PinValue>,
    changed_keys: HashSet<String>,
}

impl GlobalStore {
    pub fn get(&self, key: &str) -> Option<&PinValue> {
        self.values.get(key)
    }

    pub fn set(&mut self, key: impl Into<String>, value: PinValue) {
        let key = key.into();
        self.changed_keys.insert(key.clone());
        self.values.insert(key, value);
    }

    pub fn has(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    pub fn clear(&mut self, key: &str) {
        if self.values.remove(key).is_some() {
            self.changed_keys.insert(key.to_string());
        }
    }

    pub fn drain_changed(&mut self) -> HashSet<String> {
        std::mem::take(&mut self.changed_keys)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &PinValue)> {
        self.values.iter()
    }
}

/// Fired (as an observer trigger) once per changed key, each frame.
#[derive(Event, Clone, Debug)]
pub struct GlobalChanged {
    pub key: String,
}

pub struct GlobalsPlugin;

impl Plugin for GlobalsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GlobalStore>()
            .add_systems(First, emit_global_changed);
    }
}

fn emit_global_changed(world: &mut World) {
    let keys: Vec<String> = {
        let Some(mut store) = world.get_resource_mut::<GlobalStore>() else {
            return;
        };
        if store.changed_keys.is_empty() {
            return;
        }
        store.drain_changed().into_iter().collect()
    };
    for key in keys {
        world.trigger(GlobalChanged { key });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_has_clear_roundtrip() {
        let mut store = GlobalStore::default();
        assert!(!store.has("score"));
        store.set("score", PinValue::Int(5));
        assert_eq!(store.get("score"), Some(&PinValue::Int(5)));
        assert!(store.has("score"));
        store.clear("score");
        assert!(!store.has("score"));
    }

    #[test]
    fn drain_changed_reports_writes() {
        let mut store = GlobalStore::default();
        store.set("a", PinValue::Int(1));
        store.set("b", PinValue::Bool(true));
        let changed = store.drain_changed();
        assert!(changed.contains("a"));
        assert!(changed.contains("b"));
        assert!(store.drain_changed().is_empty());
    }

    #[test]
    fn clear_is_a_change() {
        let mut store = GlobalStore::default();
        store.set("x", PinValue::Int(1));
        let _ = store.drain_changed();
        store.clear("x");
        assert!(store.drain_changed().contains("x"));
    }
}
