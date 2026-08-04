//! Animation scripting bindings — owned by `renzora_animation`.
//!
//! Mutations flow through the existing `bridge.rs` `ScriptAction` observer;
//! `get_animation_length` is a read, which the declaration expresses as a
//! reflected-field lookup with the clip name substituted into the path.
//!
//! Other reads go through `get("AnimatorReadState.*")` directly.

use renzora_scripting::extension::{Bind, Binding, ParamKind, ScriptExtension};

pub struct AnimationScriptExtension;

impl ScriptExtension for AnimationScriptExtension {
    fn name(&self) -> &str {
        "animation"
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Bind::action("set_anim_param", "set_anim_param")
                .arg("name", ParamKind::Str)
                .arg("value", ParamKind::Float)
                .doc("Set a float parameter on the animator.")
                .build(),
            Bind::action("set_anim_bool", "set_anim_bool")
                .arg("name", ParamKind::Str)
                .arg("value", ParamKind::Bool)
                .doc("Set a bool parameter on the animator.")
                .build(),
            Bind::action("set_anim_trigger", "trigger_anim")
                .arg("name", ParamKind::Str)
                .doc("Fire a one-shot trigger parameter.")
                .build(),
            Bind::read("get_animation_length", "AnimatorReadState", "clip_lengths.{0}")
                .arg("name", ParamKind::Str)
                .doc("Length of a clip in seconds, or 0 if it is not loaded.")
                .build(),
        ]
    }
}
