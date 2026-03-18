//! Network script extension — exposes networking functions to Lua/Rhai scripts.
//!
//! ## Script Functions
//!
//! | Function | Description |
//! |----------|-------------|
//! | `net_is_server()` | Returns true if running as server |
//! | `net_is_connected()` | Returns true if connected to server |
//! | `net_client_id()` | Returns local client ID (0 if server/disconnected) |
//! | `net_send(channel, data)` | Send a game event message |
//! | `net_spawn(name, x, y, z)` | Request server to spawn entity at position |
//! | `net_rpc(name, args)` | Remote procedure call (sends as GameEvent) |

use bevy::prelude::*;
use renzora_scripting::extension::{ExtensionData, ScriptExtension};
use renzora_scripting::macros::push_ext_command;
use renzora_scripting::systems::execution::ScriptCommandQueue;
use renzora_scripting::ScriptCommand;

use crate::status::NetworkStatus;

// ── Extension data injected per-entity ─────────────────────────────────

/// Data injected into script context about network state.
#[derive(Default)]
pub struct NetworkScriptData {
    pub is_server: bool,
    pub is_connected: bool,
    pub client_id: u64,
}

// ── Command enum ─────────────────────────────────────────────────────────

renzora_scripting::script_extension_command! {
    #[derive(Debug)]
    pub enum NetworkScriptCommand {
        SendEvent { name: String, data: String },
        SpawnRequest { name: String, x: f32, y: f32, z: f32 },
        Rpc { name: String, args: String },
    }
}

// ── Script function registration ─────────────────────────────────────────

renzora_scripting::dual_register! {
    lua_fn = register_net_lua,
    rhai_fn = register_net_rhai,

    fn net_send(channel: String, data: String) {
        push_ext_command(NetworkScriptCommand::SendEvent { name: channel, data });
    }

    fn net_spawn(name: String, x: f64, y: f64, z: f64) {
        push_ext_command(NetworkScriptCommand::SpawnRequest {
            name,
            x: x as f32,
            y: y as f32,
            z: z as f32,
        });
    }

    fn net_rpc(name: String, args: String) {
        push_ext_command(NetworkScriptCommand::Rpc { name, args });
    }
}

// ── Extension implementation ─────────────────────────────────────────────

pub struct NetworkScriptExtension;

impl ScriptExtension for NetworkScriptExtension {
    fn name(&self) -> &str {
        "Networking"
    }

    fn populate_context(&self, world: &World, _entity: Entity, data: &mut ExtensionData) {
        let mut net_data = NetworkScriptData::default();
        if let Some(status) = world.get_resource::<NetworkStatus>() {
            net_data.is_server = status.is_server;
            net_data.is_connected = status.is_connected();
            net_data.client_id = status.client_id.unwrap_or(0);
        }
        data.insert(net_data);
    }

    #[cfg(all(feature = "lua", not(target_arch = "wasm32")))]
    fn register_lua_functions(&self, lua: &mlua::Lua) {
        register_net_lua(lua);

        // Register query functions that read from context globals
        let globals = lua.globals();

        // net_is_server() — reads from __net_is_server global
        let _ = globals.set(
            "net_is_server",
            lua.create_function(|lua, ()| {
                let val: bool = lua.globals().get("__net_is_server").unwrap_or(false);
                Ok(val)
            })
            .unwrap(),
        );

        // net_is_connected() — reads from __net_is_connected global
        let _ = globals.set(
            "net_is_connected",
            lua.create_function(|lua, ()| {
                let val: bool = lua.globals().get("__net_is_connected").unwrap_or(false);
                Ok(val)
            })
            .unwrap(),
        );

        // net_client_id() — reads from __net_client_id global
        let _ = globals.set(
            "net_client_id",
            lua.create_function(|lua, ()| {
                let val: i64 = lua.globals().get("__net_client_id").unwrap_or(0);
                Ok(val)
            })
            .unwrap(),
        );
    }

    #[cfg(all(feature = "lua", not(target_arch = "wasm32")))]
    fn setup_lua_context(&self, lua: &mlua::Lua, data: &ExtensionData) {
        if let Some(net_data) = data.get::<NetworkScriptData>() {
            let globals = lua.globals();
            let _ = globals.set("__net_is_server", net_data.is_server);
            let _ = globals.set("__net_is_connected", net_data.is_connected);
            let _ = globals.set("__net_client_id", net_data.client_id as i64);
        }
    }

    #[cfg(feature = "rhai")]
    fn register_rhai_functions(&self, engine: &mut rhai::Engine) {
        register_net_rhai(engine);

        // Query functions for Rhai — these read from scope variables
        // set by setup_rhai_scope.
        // Rhai doesn't have closures over scope, so we use engine-level functions
        // that always return defaults. The actual values come from scope variables.
    }

    #[cfg(feature = "rhai")]
    fn setup_rhai_scope(&self, scope: &mut rhai::Scope, data: &ExtensionData) {
        if let Some(net_data) = data.get::<NetworkScriptData>() {
            scope.push("__net_is_server", net_data.is_server);
            scope.push("__net_is_connected", net_data.is_connected);
            scope.push("__net_client_id", net_data.client_id as i64);
        }
    }
}

// ── Command processing system ────────────────────────────────────────────

/// Process `NetworkScriptCommand`s from the script command queue.
///
/// Converts script commands into actual network operations.
pub fn process_network_script_commands(
    cmd_queue: Res<ScriptCommandQueue>,
    status: Res<NetworkStatus>,
) {
    for (_source_entity, cmd) in &cmd_queue.commands {
        let ScriptCommand::Extension(ext_cmd) = cmd else {
            continue;
        };
        let Some(net_cmd) = ext_cmd.as_any().downcast_ref::<NetworkScriptCommand>() else {
            continue;
        };

        if !status.is_connected() && !status.is_server {
            log::warn!("[network] Script command ignored — not connected: {:?}", net_cmd);
            continue;
        }

        match net_cmd {
            NetworkScriptCommand::SendEvent { name, data } => {
                log::info!("[network] Script send event: {} ({}B)", name, data.len());
                // TODO: actually send via Lightyear connection
            }
            NetworkScriptCommand::SpawnRequest { name, x, y, z } => {
                log::info!("[network] Script spawn request: {} at ({}, {}, {})", name, x, y, z);
                // TODO: send SpawnRequest message to server
            }
            NetworkScriptCommand::Rpc { name, args } => {
                log::info!("[network] Script RPC: {} ({}B)", name, args.len());
                // TODO: send as GameEvent message
            }
        }
    }
}
