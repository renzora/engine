//! Scripting extension system — allows external crates to register custom
//! script functions, context data, and command processing without coupling
//! to the scripting crate.

use bevy::prelude::*;
use std::any::{Any, TypeId};
use std::collections::HashMap;

// ── Extension data (carried per-entity in ScriptContext) ─────────────────

/// Type-erased data bag that extensions populate before script execution
/// and backends read when setting up globals/scope.
#[derive(Default)]
pub struct ExtensionData {
    data: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl ExtensionData {
    /// Insert typed data. Keyed by the concrete type's `TypeId`.
    pub fn insert<T: Send + Sync + 'static>(&mut self, value: T) {
        self.data.insert(TypeId::of::<T>(), Box::new(value));
    }

    /// Get a reference to typed data.
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.data.get(&TypeId::of::<T>())
            .and_then(|b| b.downcast_ref())
    }
}

// ── Extension command trait ──────────────────────────────────────────────

/// Trait for custom commands produced by script extensions.
/// Implement this on your command struct, then wrap it with
/// `ScriptCommand::Extension(Box::new(MyCommand { ... }))`.
pub trait ScriptExtensionCommand: Send + Sync + std::fmt::Debug + 'static {
    fn as_any(&self) -> &dyn Any;
}

// ── Script extension trait ───────────────────────────────────────────────

/// Trait that external crates implement to extend the scripting system.
///
/// Extensions can:
/// - Inject per-entity data before script execution
/// - Register custom functions for Lua and/or Rhai
/// - Set up per-frame globals/scope from extension data
pub trait ScriptExtension: Send + Sync + 'static {
    /// Human-readable name for this extension (for logging).
    fn name(&self) -> &str;

    /// Populate custom data into `ExtensionData` before script execution.
    /// Called per-entity each frame. Has read-only world access.
    fn populate_context(
        &self,
        world: &World,
        entity: Entity,
        data: &mut ExtensionData,
    );

    /// Register custom Lua functions. Called once per Lua state creation.
    /// Use `push_command(ScriptCommand::Extension(...))` for custom commands.
    #[cfg(all(feature = "lua", not(target_arch = "wasm32")))]
    fn register_lua_functions(&self, _lua: &mlua::Lua) {}

    /// Set up Lua globals from extension data before each script execution.
    #[cfg(all(feature = "lua", not(target_arch = "wasm32")))]
    fn setup_lua_context(&self, _lua: &mlua::Lua, _data: &ExtensionData) {}

    /// Register custom Rhai functions. Called once per engine creation.
    #[cfg(feature = "rhai")]
    fn register_rhai_functions(&self, _engine: &mut rhai::Engine) {}

    /// Set up Rhai scope from extension data before each script execution.
    #[cfg(feature = "rhai")]
    fn setup_rhai_scope(&self, _scope: &mut rhai::Scope, _data: &ExtensionData) {}
}

// ── Registry resource ────────────────────────────────────────────────────

/// Bevy resource holding all registered script extensions.
#[derive(Resource, Default)]
pub struct ScriptExtensions {
    extensions: Vec<Box<dyn ScriptExtension>>,
}

impl ScriptExtensions {
    /// Register a new script extension.
    pub fn register(&mut self, ext: impl ScriptExtension) {
        log::info!("[Scripting] Registered extension: {}", ext.name());
        self.extensions.push(Box::new(ext));
    }

    /// Populate extension data for a given entity.
    pub fn populate_context(
        &self,
        world: &World,
        entity: Entity,
        data: &mut ExtensionData,
    ) {
        for ext in &self.extensions {
            ext.populate_context(world, entity, data);
        }
    }

    /// Register Lua functions from all extensions.
    #[cfg(all(feature = "lua", not(target_arch = "wasm32")))]
    pub fn register_lua_functions(&self, lua: &mlua::Lua) {
        for ext in &self.extensions {
            ext.register_lua_functions(lua);
        }
    }

    /// Set up Lua context from all extensions.
    #[cfg(all(feature = "lua", not(target_arch = "wasm32")))]
    pub fn setup_lua_context(&self, lua: &mlua::Lua, data: &ExtensionData) {
        for ext in &self.extensions {
            ext.setup_lua_context(lua, data);
        }
    }

    /// Register Rhai functions from all extensions.
    #[cfg(feature = "rhai")]
    pub fn register_rhai_functions(&self, engine: &mut rhai::Engine) {
        for ext in &self.extensions {
            ext.register_rhai_functions(engine);
        }
    }

    /// Set up Rhai scope from all extensions.
    #[cfg(feature = "rhai")]
    pub fn setup_rhai_scope(&self, scope: &mut rhai::Scope, data: &ExtensionData) {
        for ext in &self.extensions {
            ext.setup_rhai_scope(scope, data);
        }
    }

    /// Get the list of extensions (for backends that need direct access).
    pub fn iter(&self) -> impl Iterator<Item = &dyn ScriptExtension> {
        self.extensions.iter().map(|e| e.as_ref())
    }
}
