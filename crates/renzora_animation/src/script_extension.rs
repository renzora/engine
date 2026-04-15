//! Animation scripting bindings — owned by `renzora_animation`.
//!
//! Registers Lua helpers for animation parameters and clip length lookup.
//! Mutations flow through the existing `bridge.rs` `ScriptAction` observer.

use renzora_scripting::extension::{ExtensionData, ScriptExtension};

pub struct AnimationScriptExtension;

impl ScriptExtension for AnimationScriptExtension {
    fn name(&self) -> &str {
        "animation"
    }

    fn populate_context(
        &self,
        _world: &bevy::prelude::World,
        _entity: bevy::prelude::Entity,
        _data: &mut ExtensionData,
    ) {
        // Reads go through `get("AnimatorReadState.*")`.
    }

    #[cfg(all(feature = "lua", not(target_arch = "wasm32")))]
    fn register_lua_functions(&self, lua: &mlua::Lua) {
        use renzora::ScriptActionValue;
        use renzora_scripting::backends::push_command;
        use renzora_scripting::ScriptCommand;
        use std::collections::HashMap;

        let globals = lua.globals();

        fn push_action(name: &'static str, args: HashMap<String, ScriptActionValue>) {
            push_command(ScriptCommand::Action {
                name: name.into(),
                target_entity: None,
                args,
            });
        }

        // set_anim_param(name, value)
        let _ = globals.set(
            "set_anim_param",
            lua.create_function(|_, (name, value): (String, f32)| {
                let mut m = HashMap::new();
                m.insert("name".into(), ScriptActionValue::String(name));
                m.insert("value".into(), ScriptActionValue::Float(value));
                push_action("set_anim_param", m);
                Ok(())
            })
            .unwrap(),
        );

        // set_anim_bool(name, bool)
        let _ = globals.set(
            "set_anim_bool",
            lua.create_function(|_, (name, value): (String, bool)| {
                let mut m = HashMap::new();
                m.insert("name".into(), ScriptActionValue::String(name));
                m.insert("value".into(), ScriptActionValue::Bool(value));
                push_action("set_anim_bool", m);
                Ok(())
            })
            .unwrap(),
        );

        // set_anim_trigger(name) — one-shot trigger parameter
        let _ = globals.set(
            "set_anim_trigger",
            lua.create_function(|_, name: String| {
                let mut m = HashMap::new();
                m.insert("name".into(), ScriptActionValue::String(name));
                push_action("trigger_anim", m);
                Ok(())
            })
            .unwrap(),
        );

        // get_animation_length(name) → f32 seconds (0 if not loaded)
        let _ = globals.set(
            "get_animation_length",
            lua.create_function(|_, name: String| {
                let result = renzora_scripting::get_handler::call_get(
                    None,
                    "AnimatorReadState",
                    &format!("clip_lengths.{}", name),
                );
                Ok(match result {
                    Some(renzora::PropertyValue::Float(f)) => f,
                    _ => 0.0,
                })
            })
            .unwrap(),
        );
    }
}
