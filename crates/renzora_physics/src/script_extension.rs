//! Physics scripting bindings — owned by `renzora_physics`.
//!
//! Registers Lua (and later Rhai) helper functions that map to the existing
//! physics `ScriptAction`s. Advanced users can still call
//! `action("apply_force", {x=1,y=0,z=0})` directly; these helpers are sugar.

use renzora_scripting::extension::{ExtensionData, ScriptExtension};

pub struct PhysicsScriptExtension;

impl ScriptExtension for PhysicsScriptExtension {
    fn name(&self) -> &str {
        "physics"
    }

    fn populate_context(
        &self,
        _world: &bevy::prelude::World,
        _entity: bevy::prelude::Entity,
        _data: &mut ExtensionData,
    ) {
        // No per-entity context needed — reads go through `get("PhysicsReadState.*")`
        // which the reflect path dispatcher handles generically.
    }

    #[cfg(all(feature = "lua", not(target_arch = "wasm32")))]
    fn register_lua_functions(&self, lua: &mlua::Lua) {
        use renzora::ScriptActionValue;
        use renzora_scripting::backends::push_command;
        use renzora_scripting::ScriptCommand;
        use std::collections::HashMap;

        let globals = lua.globals();

        fn xyz(x: f32, y: f32, z: f32) -> HashMap<String, ScriptActionValue> {
            let mut m = HashMap::new();
            m.insert("x".into(), ScriptActionValue::Float(x));
            m.insert("y".into(), ScriptActionValue::Float(y));
            m.insert("z".into(), ScriptActionValue::Float(z));
            m
        }

        fn push_action(name: &'static str, args: HashMap<String, ScriptActionValue>) {
            push_command(ScriptCommand::Action {
                name: name.into(),
                target_entity: None,
                args,
            });
        }

        // move_controller(dx, dy, dz) — kinematic collide-and-slide.
        let _ = globals.set("move_controller", lua.create_function(|_, (x, y, z): (f32, f32, f32)| {
            push_action("kinematic_slide", xyz(x, y, z));
            Ok(())
        }).unwrap());

        // apply_force(x, y, z)
        let _ = globals.set("apply_force", lua.create_function(|_, (x, y, z): (f32, f32, f32)| {
            push_action("apply_force", xyz(x, y, z));
            Ok(())
        }).unwrap());

        // apply_impulse(x, y, z)
        let _ = globals.set("apply_impulse", lua.create_function(|_, (x, y, z): (f32, f32, f32)| {
            push_action("apply_impulse", xyz(x, y, z));
            Ok(())
        }).unwrap());

        // set_linear_velocity(x, y, z)
        let _ = globals.set("set_linear_velocity", lua.create_function(|_, (x, y, z): (f32, f32, f32)| {
            push_action("set_velocity", xyz(x, y, z));
            Ok(())
        }).unwrap());

        // Reads — use `get("PhysicsReadState.grounded")` etc. from the existing
        // reflect path dispatcher. No extra bindings needed.
    }
}
