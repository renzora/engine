//! `PostEffects`: an ordered list of shader-file post-process effects.
//!
//! Each entry names a `.wesl` file that declares its own sliders (see
//! `renzora_postprocess::effect_file`), so an effect is a file a user drops on
//! the component rather than a plugin compiled into the engine. The renderer
//! lives in `renzora_postprocess`; the type is here because the inspector, the
//! scripting API and the scene format all have to name it, and they sit in
//! different crates.
//!
//! Values are stored by field name, not by position. A shader edit that adds,
//! removes or reorders fields then keeps every value that still has a home,
//! where an index-keyed list would hand each one to whichever field moved into
//! its slot.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// The effects a camera renders, in order: each one reads what the previous
/// one wrote.
///
/// Authored on any scene entity (the camera, or the World Environment) and
/// routed to cameras through [`crate::EffectRouting`] like every other effect.
#[derive(Component, Clone, Debug, Default, PartialEq, Reflect, Serialize, Deserialize)]
#[reflect(Component, Default, Serialize, Deserialize)]
pub struct PostEffects {
    pub effects: Vec<PostEffect>,
}

/// One effect in a [`PostEffects`] list.
#[derive(Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
#[reflect(Serialize, Deserialize)]
pub struct PostEffect {
    /// Asset path of the effect's shader, relative to the project's assets.
    pub shader: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// The slider values the user has set. A field with no entry here takes
    /// the default its shader declares.
    #[serde(default)]
    pub values: Vec<PostEffectValue>,
}

/// One named slider value.
#[derive(Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
#[reflect(Serialize, Deserialize)]
pub struct PostEffectValue {
    pub name: String,
    pub value: f32,
}

fn default_true() -> bool {
    true
}

impl PostEffect {
    /// A newly added effect: on, with every field at its shader default.
    pub fn new(shader: impl Into<String>) -> Self {
        Self {
            shader: shader.into(),
            enabled: true,
            values: Vec::new(),
        }
    }

    /// The value set for `name`, if the user has set one.
    pub fn value(&self, name: &str) -> Option<f32> {
        self.values.iter().find(|v| v.name == name).map(|v| v.value)
    }

    /// Set `name` to `value`, adding the entry if it is not there yet.
    pub fn set_value(&mut self, name: &str, value: f32) {
        match self.values.iter_mut().find(|v| v.name == name) {
            Some(v) => v.value = value,
            None => self.values.push(PostEffectValue {
                name: name.to_string(),
                value,
            }),
        }
    }
}

impl PostEffects {
    /// Append an effect at the end of the list.
    pub fn push(&mut self, shader: impl Into<String>) {
        self.effects.push(PostEffect::new(shader));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_is_set_by_name_and_read_back() {
        let mut effect = PostEffect::new("shaders/crt.wesl");
        assert_eq!(effect.value("curvature"), None);
        effect.set_value("curvature", 0.5);
        effect.set_value("curvature", 0.25);
        assert_eq!(effect.value("curvature"), Some(0.25));
        assert_eq!(effect.values.len(), 1);
    }
}
