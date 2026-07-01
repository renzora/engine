//! Hair scripting bindings — owned by `renzora_hair`.
//!
//! Exposes `enable_hair()` / `disable_hair()`, toggling hair simulation on the
//! script's own entity. Both are sugar over the generic `action(name)` path;
//! `toggle::handle_hair_script_actions` does the actual work. Reads of the
//! tuning fields go through the reflect path (`get("Hair.stiffness")`, …) like
//! any other component field, so no per-entity context is injected here.

use renzora_scripting::extension::{ExtensionData, ScriptExtension};

pub struct HairScriptExtension;

impl ScriptExtension for HairScriptExtension {
    fn name(&self) -> &str {
        "hair"
    }

    fn populate_context(
        &self,
        _world: &bevy::prelude::World,
        _entity: bevy::prelude::Entity,
        _data: &mut ExtensionData,
    ) {
    }

    #[cfg(all(feature = "lua", not(target_arch = "wasm32")))]
    fn register_lua_functions(&self, lua: &mlua::Lua) {
        use renzora_scripting::backends::push_command;
        use renzora_scripting::ScriptCommand;
        use std::collections::HashMap;

        let globals = lua.globals();

        fn push_action(name: &'static str) {
            push_command(ScriptCommand::Action {
                name: name.into(),
                target_entity: None,
                args: HashMap::new(),
            });
        }

        let _ = globals.set(
            "enable_hair",
            lua.create_function(|_, ()| {
                push_action("enable_hair");
                Ok(())
            })
            .unwrap(),
        );

        let _ = globals.set(
            "disable_hair",
            lua.create_function(|_, ()| {
                push_action("disable_hair");
                Ok(())
            })
            .unwrap(),
        );
    }
}
