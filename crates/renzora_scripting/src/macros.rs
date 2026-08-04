//! Macros and helpers for building scripting extensions with less boilerplate.
//!
//! # Overview
//!
//!
//! To mutate the world from a script function, queue a
//! `ScriptCommand::Action { name, args, .. }` and apply it with an
//! `add_observer(On<ScriptAction>)` handler (see `renzora_physics` /
//! `renzora_runtime`'s font actions).

// ── Context setup helpers ────────────────────────────────────────────────

/// Set a Lua global table from a `HashMap<String, f32>`.
#[cfg(all(feature = "lua", not(target_arch = "wasm32")))]
pub fn lua_set_map(lua: &mlua::Lua, name: &str, map: &std::collections::HashMap<String, f32>) {
    if let Ok(table) = lua.create_table() {
        for (k, v) in map {
            let _ = table.set(k.clone(), *v as f64);
        }
        let _ = lua.globals().set(name, table);
    }
}

/// Set a Lua global nested table from `HashMap<K, HashMap<String, f32>>` where K: Display.
#[cfg(all(feature = "lua", not(target_arch = "wasm32")))]
pub fn lua_set_nested_map<K: std::fmt::Display + std::cmp::Eq + std::hash::Hash>(
    lua: &mlua::Lua,
    name: &str,
    map: &std::collections::HashMap<K, std::collections::HashMap<String, f32>>,
) {
    if let Ok(outer) = lua.create_table() {
        for (key, inner_map) in map {
            if let Ok(inner) = lua.create_table() {
                for (k, v) in inner_map {
                    let _ = inner.set(k.clone(), *v as f64);
                }
                let _ = outer.set(key.to_string(), inner);
            }
        }
        let _ = lua.globals().set(name, outer);
    }
}

// ── dual_register! ───────────────────────────────────────────────────────

///
/// Each function body receives standard Rust types (`String`, `f64`, `i64`, `bool`).
/// converted to `String`, etc.
///
/// # Supported argument types
///
/// |----------|-------------|---------------------|
/// | `String` | `String`    | `ImmutableString` → converted to `String` |
/// | `f64`    | `f64`       | `f64`               |
/// | `i64`    | `i64`       | `i64`               |
/// | `bool`   | `bool`      | `bool`              |
///
///
/// # Example
///
/// ```ignore
/// use renzora_scripting::backends::push_command;
/// use renzora_scripting::ScriptCommand;
/// use std::collections::HashMap;
///
/// renzora_scripting::dual_register! {
///     lua_fn = register_my_lua,
///
///     fn my_action(name: String, value: f64) {
///         let mut args = HashMap::new();
///         args.insert("value".into(), renzora::ScriptActionValue::Float(value as f32));
///         push_command(ScriptCommand::Action { name, target_entity: None, args });
///     }
/// }
/// ```
/// A matching `add_observer(On<ScriptAction>)` handler then applies the action.
///
/// This generates `register_my_lua(lua: &mlua::Lua)` and
#[macro_export]
macro_rules! dual_register {
    (
        lua_fn = $lua_fn:ident,
        rhai_fn = $rhai_fn:ident,

        $(
            fn $fn_name:ident( $($arg_name:ident : $arg_type:ident),* $(,)? ) {
                $($body:tt)*
            }
        )*
    ) => {
        #[cfg(all(feature = "lua", not(target_arch = "wasm32")))]
        pub fn $lua_fn(lua: &mlua::Lua) {
            let __globals = lua.globals();
            $(
                let _ = __globals.set(
                    stringify!($fn_name),
                    lua.create_function(|_lua, ( $($arg_name,)* ): ( $($crate::__lua_param_type!($arg_type),)* )| {
                        $( let $arg_name: $arg_type = $crate::__from_lua_param!($arg_name, $arg_type); )*
                        { $($body)* }
                        Ok(())
                    }).unwrap(),
                );
            )*
        }

    };
}

// ── Internal type-mapping helper macros ──────────────────────────────────

#[macro_export]
#[doc(hidden)]
macro_rules! __lua_param_type {
    (String) => {
        String
    };
    (f64) => {
        f64
    };
    (i64) => {
        i64
    };
    (bool) => {
        bool
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __rhai_param_type {
    (String) => {
        rhai::ImmutableString
    };
    (f64) => {
        f64
    };
    (i64) => {
        i64
    };
    (bool) => {
        bool
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __from_lua_param {
    ($name:ident, String) => {
        $name
    };
    ($name:ident, f64) => {
        $name
    };
    ($name:ident, i64) => {
        $name
    };
    ($name:ident, bool) => {
        $name
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __from_rhai_param {
    ($name:ident, String) => {
        $name.to_string()
    };
    ($name:ident, f64) => {
        $name
    };
    ($name:ident, i64) => {
        $name
    };
    ($name:ident, bool) => {
        $name
    };
}
