//! Navigation scripting bindings — owned by `renzora_navmesh`.
//!
//! Exposes Lua helpers that wrap the `nav_set_destination` /
//! `nav_clear_destination` script actions. For reads (has_path,
//! distance_to_destination, is_at_destination), scripts use the reflect
//! path dispatcher with the auto-mirrored `NavReadState` component.

use renzora_scripting::extension::{ExtensionData, ScriptExtension};

pub struct NavScriptExtension;

impl ScriptExtension for NavScriptExtension {
    fn name(&self) -> &str {
        "navigation"
    }

    fn populate_context(
        &self,
        _world: &bevy::prelude::World,
        _entity: bevy::prelude::Entity,
        _data: &mut ExtensionData,
    ) {
        // Per-entity reads go through `get("NavReadState.*")`.
    }

    #[cfg(all(feature = "lua", not(target_arch = "wasm32")))]
    fn register_lua_functions(&self, lua: &mlua::Lua) {
        use renzora::ScriptActionValue;
        use renzora_scripting::backends::push_command;
        use renzora_scripting::ScriptCommand;
        use std::collections::HashMap;

        let globals = lua.globals();

        fn push_nav_action(name: &'static str, args: HashMap<String, ScriptActionValue>) {
            push_command(ScriptCommand::Action {
                name: name.into(),
                target_entity: None,
                args,
            });
        }

        let _ = globals.set(
            "nav_set_destination",
            lua.create_function(|_, (x, y, z): (f32, f32, f32)| {
                let mut args = HashMap::new();
                args.insert("target".into(), ScriptActionValue::Vec3([x, y, z]));
                push_nav_action("nav_set_destination", args);
                Ok(())
            })
            .unwrap(),
        );

        let _ = globals.set(
            "nav_clear_destination",
            lua.create_function(|_, ()| {
                push_nav_action("nav_clear_destination", HashMap::new());
                Ok(())
            })
            .unwrap(),
        );

        let _ = globals.set(
            "nav_stop",
            lua.create_function(|_, ()| {
                push_nav_action("nav_clear_destination", HashMap::new());
                Ok(())
            })
            .unwrap(),
        );
    }
}
