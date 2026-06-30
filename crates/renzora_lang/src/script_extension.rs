//! Scripting binding for localization — exposes `tr("key")` to Lua and Rhai.
//!
//! `tr` is a pure read of the shared translation table (active language →
//! English → key), so it needs no `ScriptCommand`/observer plumbing: it just
//! returns the translated string. Scripts use it to localize any text they push
//! into game UI, e.g. `set_text(label, tr("hud.score"))`.

use renzora_scripting::extension::{ExtensionData, ScriptExtension};

pub struct LangScriptExtension;

impl ScriptExtension for LangScriptExtension {
    fn name(&self) -> &str {
        "localization"
    }

    fn populate_context(
        &self,
        _world: &bevy::prelude::World,
        _entity: bevy::prelude::Entity,
        _data: &mut ExtensionData,
    ) {
        // `tr` is stateless — no per-entity data to inject.
    }

    #[cfg(all(feature = "lua", not(target_arch = "wasm32")))]
    fn register_lua_functions(&self, lua: &mlua::Lua) {
        let globals = lua.globals();
        let _ = globals.set(
            "tr",
            lua.create_function(|_, key: String| Ok(renzora::lang::t(&key)))
                .unwrap(),
        );
    }

    #[cfg(feature = "rhai")]
    fn register_rhai_functions(&self, engine: &mut rhai::Engine) {
        engine.register_fn("tr", |key: &str| renzora::lang::t(key));
    }
}
