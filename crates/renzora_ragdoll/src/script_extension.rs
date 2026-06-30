//! Ragdoll scripting bindings — owned by `renzora_ragdoll`.
//!
//! Exposes `enable_ragdoll()` / `disable_ragdoll()`, toggling ragdoll
//! simulation on the script's own entity. Both are sugar over the generic
//! `action(name)` path; `toggle::handle_ragdoll_script_actions` does the
//! actual work.

use renzora_scripting::extension::{ExtensionData, ScriptExtension};

pub struct RagdollScriptExtension;

impl ScriptExtension for RagdollScriptExtension {
    fn name(&self) -> &str {
        "ragdoll"
    }

    fn populate_context(
        &self,
        _world: &bevy::prelude::World,
        _entity: bevy::prelude::Entity,
        _data: &mut ExtensionData,
    ) {
        // Nothing per-entity to inject — `enable_ragdoll`/`disable_ragdoll`
        // carry no arguments, and reads go through `get("Ragdoll.active")`
        // via the reflect path dispatcher like any other component field.
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
            "enable_ragdoll",
            lua.create_function(|_, ()| {
                push_action("enable_ragdoll");
                Ok(())
            })
            .unwrap(),
        );

        let _ = globals.set(
            "disable_ragdoll",
            lua.create_function(|_, ()| {
                push_action("disable_ragdoll");
                Ok(())
            })
            .unwrap(),
        );
    }
}
