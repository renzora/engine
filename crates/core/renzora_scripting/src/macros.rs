//! Macros and helpers for building scripting extensions with less boilerplate.
//!
//! # Overview
//!
//! - [`script_extension_command!`] — auto-implements `ScriptExtensionCommand` on a command enum
//! - [`dual_register!`] — defines script functions once, generates both Lua and Rhai bindings
//! - Helper functions for context setup (`lua_set_map`, `rhai_set_map`, etc.)
//! - [`push_ext_command`] — shorthand for pushing extension commands

// ── script_extension_command! ────────────────────────────────────────────

/// Auto-implements `ScriptExtensionCommand` for a command enum.
///
/// ```ignore
/// script_extension_command! {
///     #[derive(Debug)]
///     pub enum MyCommand {
///         SetValue { key: String, value: f32 },
///         DoAction { target: u64 },
///     }
/// }
/// ```
#[macro_export]
macro_rules! script_extension_command {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $($body:tt)*
        }
    ) => {
        $(#[$meta])*
        $vis enum $name {
            $($body)*
        }

        impl $crate::extension::ScriptExtensionCommand for $name {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }
    };
}

// ── push_ext_command ─────────────────────────────────────────────────────

/// Push a command from a `ScriptExtensionCommand` implementor.
/// Shorthand for `push_command(ScriptCommand::Extension(Box::new(cmd)))`.
pub fn push_ext_command(cmd: impl crate::extension::ScriptExtensionCommand) {
    crate::backends::push_command(crate::ScriptCommand::Extension(Box::new(cmd)));
}

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

/// Push a Rhai scope variable from a `HashMap<String, f32>`.
#[cfg(feature = "rhai")]
pub fn rhai_set_map(scope: &mut rhai::Scope, name: &str, map: &std::collections::HashMap<String, f32>) {
    let mut rhai_map = rhai::Map::new();
    for (k, v) in map {
        rhai_map.insert(k.clone().into(), rhai::Dynamic::from(*v as f64));
    }
    scope.push(name.to_string(), rhai_map);
}

/// Push a Rhai scope variable from a nested `HashMap<K, HashMap<String, f32>>`.
#[cfg(feature = "rhai")]
pub fn rhai_set_nested_map<K: std::fmt::Display + std::cmp::Eq + std::hash::Hash>(
    scope: &mut rhai::Scope,
    name: &str,
    map: &std::collections::HashMap<K, std::collections::HashMap<String, f32>>,
) {
    let mut outer = rhai::Map::new();
    for (key, inner_map) in map {
        let mut inner = rhai::Map::new();
        for (k, v) in inner_map {
            inner.insert(k.clone().into(), rhai::Dynamic::from(*v as f64));
        }
        outer.insert(key.to_string().into(), rhai::Dynamic::from(inner));
    }
    scope.push(name.to_string(), outer);
}

// ── dual_register! ───────────────────────────────────────────────────────

/// Define script functions once, generating both Lua and Rhai registration functions.
///
/// Each function body receives standard Rust types (`String`, `f64`, `i64`, `bool`).
/// The macro handles type conversion automatically — Rhai's `ImmutableString` is
/// converted to `String`, etc.
///
/// # Supported argument types
///
/// | Type     | Lua receives | Rhai receives       |
/// |----------|-------------|---------------------|
/// | `String` | `String`    | `ImmutableString` → converted to `String` |
/// | `f64`    | `f64`       | `f64`               |
/// | `i64`    | `i64`       | `i64`               |
/// | `bool`   | `bool`      | `bool`              |
///
/// **Note:** Rhai doesn't support `u64` natively — use `i64` and cast in the body.
///
/// # Example
///
/// ```ignore
/// use renzora_scripting::macros::push_ext_command;
///
/// renzora_scripting::dual_register! {
///     lua_fn = register_my_lua,
///     rhai_fn = register_my_rhai,
///
///     fn my_set(name: String, value: f64) {
///         push_ext_command(MyCommand::Set {
///             key: name, value: value as f32,
///         });
///     }
///
///     fn my_action(target_id: i64, name: String) {
///         push_ext_command(MyCommand::DoAction {
///             target: target_id as u64, name,
///         });
///     }
/// }
/// ```
///
/// This generates `register_my_lua(lua: &mlua::Lua)` and
/// `register_my_rhai(engine: &mut rhai::Engine)` with all functions registered.
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

        #[cfg(feature = "rhai")]
        pub fn $rhai_fn(engine: &mut rhai::Engine) {
            $(
                engine.register_fn(
                    stringify!($fn_name),
                    | $( $arg_name: $crate::__rhai_param_type!($arg_type) ),* | {
                        $( let $arg_name: $arg_type = $crate::__from_rhai_param!($arg_name, $arg_type); )*
                        { $($body)* }
                    },
                );
            )*
        }
    };
}

// ── Internal type-mapping helper macros ──────────────────────────────────

#[macro_export]
#[doc(hidden)]
macro_rules! __lua_param_type {
    (String) => { String };
    (f64) => { f64 };
    (i64) => { i64 };
    (bool) => { bool };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __rhai_param_type {
    (String) => { rhai::ImmutableString };
    (f64) => { f64 };
    (i64) => { i64 };
    (bool) => { bool };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __from_lua_param {
    ($name:ident, String) => { $name };
    ($name:ident, f64) => { $name };
    ($name:ident, i64) => { $name };
    ($name:ident, bool) => { $name };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __from_rhai_param {
    ($name:ident, String) => { $name.to_string() };
    ($name:ident, f64) => { $name };
    ($name:ident, i64) => { $name };
    ($name:ident, bool) => { $name };
}
