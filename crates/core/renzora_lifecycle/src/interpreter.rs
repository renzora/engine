//! Lifecycle graph interpreter.
//!
//! Entity-less, project-level interpreter. Walks the lifecycle graph each frame,
//! fires event nodes, handles wait/timer nodes, and produces `ScriptCommand`s.

use bevy::prelude::*;
use std::collections::HashMap;

use renzora_blueprint::graph::{NodeId, PinValue};
use renzora_scripting::systems::execution::ScriptCommandQueue;
use renzora_scripting::ScriptCommand;

use crate::graph::LifecycleGraph;
use crate::nodes;
use crate::state::LifecycleRuntimeState;

/// Evaluation context for the lifecycle graph (no entity, no transform).
struct EvalContext<'a> {
    cache: HashMap<(NodeId, String), PinValue>,
    graph: &'a LifecycleGraph,
    delta: f32,
    elapsed: f64,
    commands: Vec<ScriptCommand>,
    runtime: &'a mut LifecycleRuntimeState,
    net_is_server: bool,
    net_is_connected: bool,
    net_player_count: i32,
    /// Wait timers to start (node_id, seconds).
    new_waits: Vec<(NodeId, f32)>,
    /// Network commands to apply directly to world after evaluation.
    net_commands: Vec<LifecycleNetCommand>,
}

impl<'a> EvalContext<'a> {
    fn resolve_input(&mut self, node_id: NodeId, pin_name: &str) -> PinValue {
        if let Some(conn) = self.graph.connection_to(node_id, pin_name) {
            let from_node = conn.from_node;
            let from_pin = conn.from_pin.clone();
            return self.evaluate_output(from_node, &from_pin);
        }

        if let Some(node) = self.graph.get_node(node_id) {
            if let Some(val) = node.input_values.get(pin_name) {
                return val.clone();
            }
        }

        if let Some(node) = self.graph.get_node(node_id) {
            if let Some(def) = nodes::node_def(&node.node_type) {
                for pin in (def.pins)() {
                    if pin.name == pin_name {
                        return pin.default_value.clone();
                    }
                }
            }
        }

        PinValue::None
    }

    fn evaluate_output(&mut self, node_id: NodeId, pin_name: &str) -> PinValue {
        let cache_key = (node_id, pin_name.to_string());
        if let Some(cached) = self.cache.get(&cache_key) {
            return cached.clone();
        }

        let node_type = match self.graph.get_node(node_id) {
            Some(n) => n.node_type.clone(),
            None => return PinValue::None,
        };

        let value = self.eval_node_output(&node_type, node_id, pin_name);
        self.cache.insert(cache_key, value.clone());
        value
    }

    fn eval_node_output(&mut self, node_type: &str, node_id: NodeId, pin_name: &str) -> PinValue {
        match node_type {
            // ── Lifecycle data queries ──────────────────────────────
            "lifecycle/is_server" => PinValue::Bool(self.net_is_server),
            "lifecycle/is_connected" => PinValue::Bool(self.net_is_connected),
            "lifecycle/get_scene_name" => PinValue::String(self.runtime.current_scene.clone()),
            "lifecycle/get_player_count" => PinValue::Int(self.net_player_count),
            "lifecycle/get_variable" => {
                let name = self.resolve_input(node_id, "name").as_string();
                self.runtime
                    .variables
                    .get(&name)
                    .cloned()
                    .unwrap_or(PinValue::None)
            }
            "lifecycle/on_scene_loaded" => match pin_name {
                "scene" => PinValue::String(
                    self.runtime
                        .scene_just_loaded
                        .clone()
                        .unwrap_or_default(),
                ),
                _ => PinValue::None,
            },
            "lifecycle/on_player_joined" | "lifecycle/on_player_left" => match pin_name {
                "player_id" => PinValue::Int(0), // populated by event firing
                _ => PinValue::None,
            },

            // ── Flow control data outputs ──────────────────────────
            "flow/flip_flop" => match pin_name {
                "is_a" => PinValue::Bool(
                    *self
                        .runtime
                        .flip_flop_state
                        .get(&node_id)
                        .unwrap_or(&true),
                ),
                _ => PinValue::None,
            },
            "flow/counter" => {
                let val = self
                    .runtime
                    .counter_values
                    .get(&node_id)
                    .copied()
                    .unwrap_or(0.0);
                PinValue::Float(val)
            }

            // ── Shared math/string/convert from blueprint ──────────
            "math/add" => {
                let a = self.resolve_input(node_id, "a").as_float();
                let b = self.resolve_input(node_id, "b").as_float();
                PinValue::Float(a + b)
            }
            "math/subtract" => {
                let a = self.resolve_input(node_id, "a").as_float();
                let b = self.resolve_input(node_id, "b").as_float();
                PinValue::Float(a - b)
            }
            "math/multiply" => {
                let a = self.resolve_input(node_id, "a").as_float();
                let b = self.resolve_input(node_id, "b").as_float();
                PinValue::Float(a * b)
            }
            "math/divide" => {
                let a = self.resolve_input(node_id, "a").as_float();
                let b = self.resolve_input(node_id, "b").as_float();
                PinValue::Float(if b == 0.0 { 0.0 } else { a / b })
            }
            "math/negate" => PinValue::Float(-self.resolve_input(node_id, "value").as_float()),
            "math/abs" => PinValue::Float(self.resolve_input(node_id, "value").as_float().abs()),
            "math/clamp" => {
                let v = self.resolve_input(node_id, "value").as_float();
                let min = self.resolve_input(node_id, "min").as_float();
                let max = self.resolve_input(node_id, "max").as_float();
                PinValue::Float(v.clamp(min, max))
            }
            "math/compare" => {
                let a = self.resolve_input(node_id, "a").as_float();
                let b = self.resolve_input(node_id, "b").as_float();
                match pin_name {
                    "greater" => PinValue::Bool(a > b),
                    "less" => PinValue::Bool(a < b),
                    "equal" => PinValue::Bool((a - b).abs() < f32::EPSILON),
                    _ => PinValue::None,
                }
            }
            "math/and" => {
                let a = self.resolve_input(node_id, "a").as_bool();
                let b = self.resolve_input(node_id, "b").as_bool();
                PinValue::Bool(a && b)
            }
            "math/or" => {
                let a = self.resolve_input(node_id, "a").as_bool();
                let b = self.resolve_input(node_id, "b").as_bool();
                PinValue::Bool(a || b)
            }
            "math/not" => PinValue::Bool(!self.resolve_input(node_id, "value").as_bool()),
            "math/min" => {
                let a = self.resolve_input(node_id, "a").as_float();
                let b = self.resolve_input(node_id, "b").as_float();
                PinValue::Float(a.min(b))
            }
            "math/max" => {
                let a = self.resolve_input(node_id, "a").as_float();
                let b = self.resolve_input(node_id, "b").as_float();
                PinValue::Float(a.max(b))
            }
            "string/concat" => {
                let a = self.resolve_input(node_id, "a").as_string();
                let b = self.resolve_input(node_id, "b").as_string();
                PinValue::String(format!("{}{}", a, b))
            }
            "string/format" => {
                let template = self.resolve_input(node_id, "template").as_string();
                let value = self.resolve_input(node_id, "value");
                let replacement = match value {
                    PinValue::Float(v) => format!("{:.2}", v),
                    PinValue::Int(v) => format!("{}", v),
                    PinValue::Bool(v) => format!("{}", v),
                    PinValue::String(v) => v,
                    _ => String::new(),
                };
                PinValue::String(template.replace("{0}", &replacement))
            }
            "convert/to_string" => PinValue::String(self.resolve_input(node_id, "value").as_string()),
            "convert/to_float" => PinValue::Float(self.resolve_input(node_id, "value").as_float()),
            "convert/to_int" => PinValue::Int(self.resolve_input(node_id, "value").as_int()),
            "convert/to_bool" => PinValue::Bool(self.resolve_input(node_id, "value").as_bool()),

            _ => PinValue::None,
        }
    }

    fn follow_exec(&mut self, from_node: NodeId, from_pin: &str) {
        let connections = self
            .graph
            .connections_from(from_node, from_pin)
            .into_iter()
            .map(|c| (c.to_node, c.to_pin.clone()))
            .collect::<Vec<_>>();

        for (target_node, target_pin) in connections {
            self.execute_node(target_node, &target_pin);
        }
    }

    fn execute_node(&mut self, node_id: NodeId, _exec_pin: &str) {
        let node_type = match self.graph.get_node(node_id) {
            Some(n) => n.node_type.clone(),
            None => return,
        };

        match node_type.as_str() {
            // ── Flow ────────────────────────────────────────────────
            "flow/branch" => {
                let cond = self.resolve_input(node_id, "condition").as_bool();
                if cond {
                    self.follow_exec(node_id, "true");
                } else {
                    self.follow_exec(node_id, "false");
                }
            }
            "flow/sequence" => {
                self.follow_exec(node_id, "then_0");
                self.follow_exec(node_id, "then_1");
                self.follow_exec(node_id, "then_2");
            }
            "flow/do_once" => {
                if _exec_pin == "reset" {
                    self.runtime.do_once_fired.remove(&node_id);
                    return;
                }
                if !self.runtime.do_once_fired.contains_key(&node_id) {
                    self.runtime.do_once_fired.insert(node_id, true);
                    self.follow_exec(node_id, "completed");
                }
            }
            "flow/flip_flop" => {
                let is_a = self
                    .runtime
                    .flip_flop_state
                    .entry(node_id)
                    .or_insert(true);
                if *is_a {
                    *is_a = false;
                    self.follow_exec(node_id, "a");
                } else {
                    *is_a = true;
                    self.follow_exec(node_id, "b");
                }
            }
            "flow/gate" => match _exec_pin {
                "open" => {
                    self.runtime.gate_open.insert(node_id, true);
                }
                "close" => {
                    self.runtime.gate_open.insert(node_id, false);
                }
                "toggle" => {
                    let current = *self.runtime.gate_open.get(&node_id).unwrap_or(&true);
                    self.runtime.gate_open.insert(node_id, !current);
                }
                "exec" => {
                    let start_open = self.resolve_input(node_id, "start_open").as_bool();
                    let open = *self
                        .runtime
                        .gate_open
                        .get(&node_id)
                        .unwrap_or(&start_open);
                    if open {
                        self.follow_exec(node_id, "exit");
                    }
                }
                _ => {}
            },
            "flow/counter" => {
                let step = self.resolve_input(node_id, "step").as_float();
                let min = self.resolve_input(node_id, "min").as_float();
                let max = self.resolve_input(node_id, "max").as_float();
                let do_loop = self.resolve_input(node_id, "loop").as_bool();
                let val = self.runtime.counter_values.entry(node_id).or_insert(min);
                *val += step * self.delta;
                if *val > max {
                    if do_loop {
                        *val = min + (*val - max);
                    } else {
                        *val = max;
                    }
                }
                self.follow_exec(node_id, "then");
            }

            // ── Lifecycle actions ───────────────────────────────────
            "lifecycle/load_scene" => {
                let path = self.resolve_input(node_id, "path").as_string();
                if path.is_empty() {
                    log::warn!("[lifecycle] Load Scene: empty path");
                    self.follow_exec(node_id, "error");
                } else if path != self.runtime.current_scene {
                    self.runtime.current_scene = path.clone();
                    self.commands.push(ScriptCommand::LoadScene { path });
                    self.follow_exec(node_id, "success");
                } else {
                    // Already loaded — still success
                    self.follow_exec(node_id, "success");
                }
            }
            "lifecycle/wait" => {
                let seconds = self.resolve_input(node_id, "seconds").as_float();
                if seconds <= 0.0 {
                    self.follow_exec(node_id, "error");
                } else {
                    self.new_waits.push((node_id, seconds));
                    // Do NOT follow_exec — continuation is deferred.
                }
            }
            "lifecycle/start_timer" => {
                let name = self.resolve_input(node_id, "name").as_string();
                let seconds = self.resolve_input(node_id, "seconds").as_float();
                let repeat = self.resolve_input(node_id, "repeat").as_bool();
                if name.is_empty() || seconds <= 0.0 {
                    self.follow_exec(node_id, "error");
                } else {
                    self.runtime.named_timers.insert(
                        name,
                        crate::state::LifecycleTimer::new(seconds, repeat),
                    );
                    self.follow_exec(node_id, "success");
                }
            }
            "lifecycle/connect" => {
                let address = self.resolve_input(node_id, "address").as_string();
                let port = self.resolve_input(node_id, "port").as_int();
                if address.is_empty() || port <= 0 || port > 65535 {
                    log::warn!("[lifecycle] Connect: invalid address/port");
                    self.follow_exec(node_id, "error");
                } else {
                    log::info!("[lifecycle] Connect to {}:{}", address, port);
                    self.net_commands.push(LifecycleNetCommand::Connect {
                        address,
                        port: port as u16,
                    });
                    self.follow_exec(node_id, "success");
                }
            }
            "lifecycle/disconnect" => {
                self.net_commands.push(LifecycleNetCommand::Disconnect);
                self.follow_exec(node_id, "success");
            }
            "lifecycle/host_server" => {
                let port = self.resolve_input(node_id, "port").as_int();
                let max_clients = self.resolve_input(node_id, "max_clients").as_int();
                if port <= 0 || port > 65535 || max_clients <= 0 {
                    log::warn!("[lifecycle] Host Server: invalid port/max_clients");
                    self.follow_exec(node_id, "error");
                } else {
                    log::info!("[lifecycle] Host server on port {} (max {})", port, max_clients);
                    self.net_commands.push(LifecycleNetCommand::HostServer {
                        port: port as u16,
                        max_clients: max_clients as u16,
                    });
                    self.follow_exec(node_id, "success");
                }
            }
            "lifecycle/send_message" => {
                let channel = self.resolve_input(node_id, "channel").as_string();
                let data = self.resolve_input(node_id, "data").as_string();
                if channel.is_empty() {
                    self.follow_exec(node_id, "error");
                } else {
                    self.net_commands.push(LifecycleNetCommand::SendMessage { channel, data });
                    self.follow_exec(node_id, "success");
                }
            }
            "lifecycle/spawn_networked" => {
                let name = self.resolve_input(node_id, "name").as_string();
                let position = self.resolve_input(node_id, "position").as_vec3();
                if name.is_empty() {
                    self.follow_exec(node_id, "error");
                } else {
                    self.net_commands.push(LifecycleNetCommand::SpawnNetworked {
                        name,
                        position,
                    });
                    self.follow_exec(node_id, "success");
                }
            }
            "lifecycle/log" => {
                let message = self.resolve_input(node_id, "message").as_string();
                self.commands.push(ScriptCommand::Log {
                    level: "info".into(),
                    message,
                });
                self.follow_exec(node_id, "success");
            }
            "lifecycle/set_variable" => {
                let name = self.resolve_input(node_id, "name").as_string();
                let value = self.resolve_input(node_id, "value");
                if name.is_empty() {
                    self.follow_exec(node_id, "error");
                } else {
                    self.runtime.variables.insert(name, value);
                    self.follow_exec(node_id, "success");
                }
            }

            // ── Debug (from shared) ─────────────────────────────────
            "debug/log" => {
                let message = self.resolve_input(node_id, "message").as_string();
                self.commands.push(ScriptCommand::Log {
                    level: "info".into(),
                    message,
                });
                self.follow_exec(node_id, "then");
            }

            _ => {
                // Unknown node type — skip
            }
        }
    }
}

/// Network commands produced by lifecycle nodes.
/// These are consumed by `process_lifecycle_net_commands`.
#[derive(Debug)]
pub enum LifecycleNetCommand {
    Connect { address: String, port: u16 },
    Disconnect,
    HostServer { port: u16, max_clients: u16 },
    SendMessage { channel: String, data: String },
    SpawnNetworked { name: String, position: [f32; 3] },
}

// ── Main system ─────────────────────────────────────────────────────────────

/// Exclusive system: runs the lifecycle graph each frame.
pub fn run_lifecycle(world: &mut World) {
    // Bail early if no lifecycle graph.
    let has_graph = world.get_resource::<LifecycleGraph>().is_some();
    if !has_graph {
        return;
    }

    let time_delta = world.resource::<Time>().delta_secs();
    let time_elapsed = world.resource::<Time>().elapsed_secs_f64();

    // Take resources for exclusive access.
    let graph = world.resource::<LifecycleGraph>().clone();
    let mut runtime = world.remove_resource::<LifecycleRuntimeState>().unwrap_or_default();

    if graph.nodes.is_empty() {
        world.insert_resource(runtime);
        return;
    }

    let was_initialized = runtime.initialized;

    // Tick active wait timers.
    let mut completed_waits = Vec::new();
    for (node_id, timer) in runtime.active_waits.iter_mut() {
        if timer.tick(time_delta) {
            completed_waits.push(*node_id);
        }
    }
    for node_id in &completed_waits {
        runtime.active_waits.remove(node_id);
        runtime.pending_continuations.push((*node_id, "success".to_string()));
    }

    // Tick named timers.
    runtime.timers_just_finished.clear();
    let mut finished_names = Vec::new();
    for (name, timer) in runtime.named_timers.iter_mut() {
        if timer.tick(time_delta) {
            finished_names.push(name.clone());
        }
    }
    runtime.timers_just_finished = finished_names.clone();
    // Remove non-repeating timers that finished.
    runtime.named_timers.retain(|_name, timer| {
        timer.repeat || timer.remaining > 0.0
    });

    // Check network status.
    let (net_is_server, net_is_connected, net_player_count) = world
        .get_resource::<renzora_network::NetworkStatus>()
        .map(|s| (s.is_server, s.is_connected(), s.connected_clients.len() as i32))
        .unwrap_or((false, false, 0));

    // Detect connection edge.
    let connected_edge = net_is_connected && !runtime.prev_connected;
    let disconnected_edge = !net_is_connected && runtime.prev_connected;
    runtime.prev_connected = net_is_connected;

    // Take pending continuations.
    let continuations = std::mem::take(&mut runtime.pending_continuations);

    // Collect event nodes.
    let event_nodes: Vec<(NodeId, String)> = graph
        .event_nodes()
        .iter()
        .map(|n| (n.id, n.node_type.clone()))
        .collect();

    let scene_just_loaded = runtime.scene_just_loaded.take();

    let (commands, new_waits, net_commands) = {
        let mut ctx = EvalContext {
            cache: HashMap::new(),
            graph: &graph,
            delta: time_delta,
            elapsed: time_elapsed,
            commands: Vec::new(),
            runtime: &mut runtime,
            net_is_server,
            net_is_connected,
            net_player_count,
            new_waits: Vec::new(),
            net_commands: Vec::new(),
        };

        // Resume continuations from completed waits.
        for (node_id, pin_name) in &continuations {
            ctx.follow_exec(*node_id, pin_name);
        }

        // Fire event nodes.
        for (node_id, node_type) in &event_nodes {
            match node_type.as_str() {
                "lifecycle/on_game_start" => {
                    if !was_initialized {
                        ctx.follow_exec(*node_id, "exec");
                    }
                }
                "lifecycle/on_scene_loaded" => {
                    if let Some(ref scene_name) = scene_just_loaded {
                        ctx.cache.insert(
                            (*node_id, "scene".to_string()),
                            PinValue::String(scene_name.clone()),
                        );
                        ctx.follow_exec(*node_id, "exec");
                    }
                }
                "lifecycle/on_connected" => {
                    if connected_edge {
                        ctx.follow_exec(*node_id, "exec");
                    }
                }
                "lifecycle/on_disconnected" => {
                    if disconnected_edge {
                        ctx.follow_exec(*node_id, "exec");
                    }
                }
                "lifecycle/on_timer" => {
                    let name = ctx.resolve_input(*node_id, "name").as_string();
                    if ctx.runtime.timers_just_finished.contains(&name) {
                        ctx.follow_exec(*node_id, "exec");
                    }
                }
                // on_player_joined, on_player_left, on_message: TODO when network events are available
                _ => {}
            }
        }

        (ctx.commands, ctx.new_waits, ctx.net_commands)
    };

    runtime.initialized = true;

    // Register new wait timers.
    for (node_id, seconds) in new_waits {
        runtime.active_waits.insert(
            node_id,
            crate::state::LifecycleTimer::new(seconds, false),
        );
    }

    // Push commands to shared queue.
    if !commands.is_empty() {
        let mut cmd_queue = world.resource_mut::<ScriptCommandQueue>();
        for cmd in commands {
            cmd_queue.commands.push((Entity::PLACEHOLDER, cmd));
        }
    }

    world.insert_resource(runtime);

    // Process network commands directly on the world.
    for net_cmd in net_commands {
        match net_cmd {
            LifecycleNetCommand::Connect { address, port } => {
                world.insert_resource(renzora_network::PendingNetworkConnect {
                    address,
                    port,
                });
            }
            LifecycleNetCommand::Disconnect => {
                world.insert_resource(renzora_network::PendingNetworkDisconnect);
            }
            LifecycleNetCommand::HostServer { port, max_clients } => {
                log::info!(
                    "[lifecycle] HostServer requested (port={}, max={}). \
                     Use renzora-server binary for dedicated servers.",
                    port,
                    max_clients
                );
            }
            LifecycleNetCommand::SendMessage { channel, data } => {
                log::info!("[lifecycle] SendMessage '{}' ({}B)", channel, data.len());
                // TODO: send via Lightyear when message API is wired
            }
            LifecycleNetCommand::SpawnNetworked { name, position } => {
                log::info!(
                    "[lifecycle] SpawnNetworked '{}' at ({:.1}, {:.1}, {:.1})",
                    name,
                    position[0],
                    position[1],
                    position[2]
                );
                // TODO: send SpawnRequest message to server
            }
        }
    }
}

/// Detects when a scene finishes loading and sets `scene_just_loaded`.
pub fn detect_scene_loaded(
    pending: Res<renzora_core::PendingSceneLoad>,
    mut runtime: ResMut<LifecycleRuntimeState>,
    mut last_requests: Local<Vec<String>>,
) {
    // If there were pending requests last frame but none this frame, the scene loaded.
    if !last_requests.is_empty() && pending.requests.is_empty() {
        if let Some(scene_name) = last_requests.last() {
            runtime.scene_just_loaded = Some(scene_name.clone());
            runtime.current_scene = scene_name.clone();
        }
    }
    *last_requests = pending.requests.clone();
}

/// Reset lifecycle runtime state when play mode starts.
pub fn reset_lifecycle_on_play_start(
    play_mode: Option<Res<renzora_core::PlayModeState>>,
    project: Option<Res<renzora_core::CurrentProject>>,
    mut runtime: ResMut<LifecycleRuntimeState>,
    mut was_running: Local<bool>,
) {
    let running = play_mode
        .as_ref()
        .map(|pm| pm.is_scripts_running())
        .unwrap_or(false);
    if running && !*was_running {
        *runtime = LifecycleRuntimeState::default();
        // Seed current_scene with what's already loaded so Load Scene
        // can skip reloading the same scene.
        if let Some(proj) = project.as_ref() {
            runtime.current_scene = proj.config.main_scene.clone();
        }
    }
    *was_running = running;
}
