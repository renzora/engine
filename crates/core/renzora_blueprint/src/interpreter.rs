//! Blueprint graph interpreter.
//!
//! Walks the graph starting from event nodes, evaluates data pins,
//! and produces `ScriptCommand`s that feed into the existing command pipeline.

use bevy::prelude::*;
use std::collections::HashMap;

use renzora_scripting::systems::execution::{ScriptCommandQueue, TransformWrite};
use renzora_scripting::{PropertyValue, ScriptCommand, ScriptInput};

use crate::graph::{BlueprintGraph, NodeId, PinValue};
use crate::nodes;

/// Per-entity runtime state for flow control nodes (DoOnce, FlipFlop, Gate, Delay).
#[derive(Component, Default)]
pub struct BlueprintRuntimeState {
    pub initialized: bool,
    /// DoOnce: set of node IDs that have already fired.
    pub do_once_fired: HashMap<NodeId, bool>,
    /// FlipFlop: current side per node.
    pub flip_flop_state: HashMap<NodeId, bool>,
    /// Gate: open/closed per node.
    pub gate_open: HashMap<NodeId, bool>,
}

/// Context passed through the graph during evaluation.
struct EvalContext<'a> {
    /// The entity owning this blueprint.
    entity: Entity,
    /// Cached pin output values (node_id, pin_name) -> value.
    cache: HashMap<(NodeId, String), PinValue>,
    /// The graph being evaluated.
    graph: &'a BlueprintGraph,
    /// Transform of this entity.
    transform: &'a Transform,
    /// Input state.
    input: &'a ScriptInput,
    /// Time info.
    delta: f32,
    elapsed: f64,
    /// Commands to emit.
    commands: Vec<ScriptCommand>,
    /// Transform writes to emit.
    transform_writes: Vec<TransformWrite>,
    /// Runtime state for flow control.
    runtime: &'a mut BlueprintRuntimeState,
    /// Entity name.
    entity_name: String,
}

impl<'a> EvalContext<'a> {
    /// Resolve the value of an input pin on a node.
    /// If a connection exists, evaluate the source node's output pin.
    /// Otherwise, use the node's inline value or the pin's default.
    fn resolve_input(&mut self, node_id: NodeId, pin_name: &str) -> PinValue {
        // Check if there's a connection feeding this input.
        if let Some(conn) = self.graph.connection_to(node_id, pin_name) {
            let from_node = conn.from_node;
            let from_pin = conn.from_pin.clone();
            return self.evaluate_output(from_node, &from_pin);
        }

        // Check node's inline values.
        if let Some(node) = self.graph.get_node(node_id) {
            if let Some(val) = node.input_values.get(pin_name) {
                return val.clone();
            }
        }

        // Fall back to the pin template's default.
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

    /// Evaluate a data output pin on a node. Results are cached.
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

    /// Evaluate a specific output pin of a node by type.
    fn eval_node_output(&mut self, node_type: &str, node_id: NodeId, pin_name: &str) -> PinValue {
        match node_type {
            // ── Event outputs ────────────────────────────────────────
            "event/on_update" => match pin_name {
                "delta" => PinValue::Float(self.delta),
                "elapsed" => PinValue::Float(self.elapsed as f32),
                _ => PinValue::None,
            },

            // ── Math ─────────────────────────────────────────────────
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
            "math/negate" => {
                let v = self.resolve_input(node_id, "value").as_float();
                PinValue::Float(-v)
            }
            "math/abs" => {
                let v = self.resolve_input(node_id, "value").as_float();
                PinValue::Float(v.abs())
            }
            "math/clamp" => {
                let v = self.resolve_input(node_id, "value").as_float();
                let min = self.resolve_input(node_id, "min").as_float();
                let max = self.resolve_input(node_id, "max").as_float();
                PinValue::Float(v.clamp(min, max))
            }
            "math/lerp" => {
                let a = self.resolve_input(node_id, "a").as_float();
                let b = self.resolve_input(node_id, "b").as_float();
                let t = self.resolve_input(node_id, "t").as_float();
                PinValue::Float(a + (b - a) * t)
            }
            "math/sin" => {
                let v = self.resolve_input(node_id, "value").as_float();
                PinValue::Float(v.sin())
            }
            "math/cos" => {
                let v = self.resolve_input(node_id, "value").as_float();
                PinValue::Float(v.cos())
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
            "math/not" => {
                let v = self.resolve_input(node_id, "value").as_bool();
                PinValue::Bool(!v)
            }
            "math/combine_vec3" => {
                let x = self.resolve_input(node_id, "x").as_float();
                let y = self.resolve_input(node_id, "y").as_float();
                let z = self.resolve_input(node_id, "z").as_float();
                PinValue::Vec3([x, y, z])
            }
            "math/split_vec3" => {
                let v = self.resolve_input(node_id, "vector").as_vec3();
                match pin_name {
                    "x" => PinValue::Float(v[0]),
                    "y" => PinValue::Float(v[1]),
                    "z" => PinValue::Float(v[2]),
                    _ => PinValue::None,
                }
            }
            "math/random_range" => {
                let min = self.resolve_input(node_id, "min").as_float();
                let max = self.resolve_input(node_id, "max").as_float();
                // Simple deterministic-ish random based on elapsed time + node id.
                // For real randomness we'd use a proper RNG, but this works for now.
                let seed = (self.elapsed * 1000.0) as u32 ^ (node_id as u32);
                let t = ((seed % 10000) as f32) / 10000.0;
                PinValue::Float(min + (max - min) * t)
            }

            // ── Transform reads ──────────────────────────────────────
            "transform/get_position" => {
                let pos = self.transform.translation;
                match pin_name {
                    "position" => PinValue::Vec3([pos.x, pos.y, pos.z]),
                    "x" => PinValue::Float(pos.x),
                    "y" => PinValue::Float(pos.y),
                    "z" => PinValue::Float(pos.z),
                    _ => PinValue::None,
                }
            }
            "transform/get_rotation" => {
                let (x, y, z) = self.transform.rotation.to_euler(EulerRot::XYZ);
                match pin_name {
                    "rotation" => PinValue::Vec3([x.to_degrees(), y.to_degrees(), z.to_degrees()]),
                    "x" => PinValue::Float(x.to_degrees()),
                    "y" => PinValue::Float(y.to_degrees()),
                    "z" => PinValue::Float(z.to_degrees()),
                    _ => PinValue::None,
                }
            }
            "transform/get_forward" => {
                let fwd = self.transform.rotation * Vec3::NEG_Z;
                let right = self.transform.rotation * Vec3::X;
                let up = self.transform.rotation * Vec3::Y;
                match pin_name {
                    "forward" => PinValue::Vec3([fwd.x, fwd.y, fwd.z]),
                    "right" => PinValue::Vec3([right.x, right.y, right.z]),
                    "up" => PinValue::Vec3([up.x, up.y, up.z]),
                    _ => PinValue::None,
                }
            }

            // ── Input reads ──────────────────────────────────────────
            "input/get_movement" => {
                let mv = self.input.get_movement_vector();
                match pin_name {
                    "movement" => PinValue::Vec2([mv.x, mv.y]),
                    "x" => PinValue::Float(mv.x),
                    "y" => PinValue::Float(mv.y),
                    _ => PinValue::None,
                }
            }
            "input/is_key_pressed" => {
                let key_str = self.resolve_input(node_id, "key").as_string();
                let pressed = self.input.keys_pressed.iter().any(|(k, &v)| {
                    v && format!("{:?}", k) == key_str
                });
                PinValue::Bool(pressed)
            }
            "input/is_key_just_pressed" => {
                let key_str = self.resolve_input(node_id, "key").as_string();
                let pressed = self.input.keys_just_pressed.iter().any(|(k, &v)| {
                    v && format!("{:?}", k) == key_str
                });
                PinValue::Bool(pressed)
            }
            "input/get_mouse_position" => {
                match pin_name {
                    "position" => PinValue::Vec2([self.input.mouse_position.x, self.input.mouse_position.y]),
                    "delta" => PinValue::Vec2([self.input.mouse_delta.x, self.input.mouse_delta.y]),
                    _ => PinValue::None,
                }
            }
            "input/is_mouse_pressed" => {
                let btn = self.resolve_input(node_id, "button").as_int();
                let mouse_btn = match btn {
                    0 => MouseButton::Left,
                    1 => MouseButton::Right,
                    2 => MouseButton::Middle,
                    _ => MouseButton::Left,
                };
                let pressed = self.input.mouse_pressed.get(&mouse_btn).copied().unwrap_or(false);
                PinValue::Bool(pressed)
            }
            "input/get_gamepad" => {
                match pin_name {
                    "left_stick" => {
                        let v = self.input.get_gamepad_left_stick(0);
                        PinValue::Vec2([v.x, v.y])
                    }
                    "right_stick" => {
                        let v = self.input.get_gamepad_right_stick(0);
                        PinValue::Vec2([v.x, v.y])
                    }
                    "left_trigger" => PinValue::Float(self.input.get_gamepad_trigger(0, true)),
                    "right_trigger" => PinValue::Float(self.input.get_gamepad_trigger(0, false)),
                    _ => PinValue::None,
                }
            }

            // ── Entity reads ─────────────────────────────────────────
            "entity/get_self" => match pin_name {
                "entity" => PinValue::Entity(self.entity_name.clone()),
                "name" => PinValue::String(self.entity_name.clone()),
                _ => PinValue::None,
            },

            // ── Flow control data outputs ────────────────────────────
            "flow/flip_flop" => match pin_name {
                "is_a" => PinValue::Bool(*self.runtime.flip_flop_state.get(&node_id).unwrap_or(&true)),
                _ => PinValue::None,
            },

            _ => PinValue::None,
        }
    }

    /// Execute an exec-pin chain starting from a node's output exec pin.
    fn follow_exec(&mut self, from_node: NodeId, from_pin: &str) {
        let connections = self.graph.connections_from(from_node, from_pin)
            .into_iter()
            .map(|c| (c.to_node, c.to_pin.clone()))
            .collect::<Vec<_>>();

        for (target_node, target_pin) in connections {
            self.execute_node(target_node, &target_pin);
        }
    }

    /// Execute a node that received an exec signal on the given input pin.
    fn execute_node(&mut self, node_id: NodeId, _exec_pin: &str) {
        let node_type = match self.graph.get_node(node_id) {
            Some(n) => n.node_type.clone(),
            None => return,
        };

        match node_type.as_str() {
            // ── Flow ─────────────────────────────────────────────────
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
                let is_a = self.runtime.flip_flop_state.entry(node_id).or_insert(true);
                if *is_a {
                    *is_a = false;
                    self.follow_exec(node_id, "a");
                } else {
                    *is_a = true;
                    self.follow_exec(node_id, "b");
                }
            }
            "flow/gate" => {
                match _exec_pin {
                    "open" => { self.runtime.gate_open.insert(node_id, true); }
                    "close" => { self.runtime.gate_open.insert(node_id, false); }
                    "toggle" => {
                        let current = *self.runtime.gate_open.get(&node_id).unwrap_or(&true);
                        self.runtime.gate_open.insert(node_id, !current);
                    }
                    "exec" => {
                        let start_open = self.resolve_input(node_id, "start_open").as_bool();
                        let open = *self.runtime.gate_open.get(&node_id).unwrap_or(&start_open);
                        if open {
                            self.follow_exec(node_id, "exit");
                        }
                    }
                    _ => {}
                }
            }

            // ── Transform writes ─────────────────────────────────────
            "transform/set_position" => {
                let pos = self.resolve_input(node_id, "position").as_vec3();
                self.transform_writes.push(TransformWrite {
                    entity: self.entity,
                    new_position: Some(Vec3::new(pos[0], pos[1], pos[2])),
                    new_rotation: None,
                    translation: None,
                    rotation_delta: None,
                    new_scale: None,
                    look_at: None,
                });
                self.follow_exec(node_id, "then");
            }
            "transform/translate" => {
                let offset = self.resolve_input(node_id, "offset").as_vec3();
                self.transform_writes.push(TransformWrite {
                    entity: self.entity,
                    new_position: None,
                    new_rotation: None,
                    translation: Some(Vec3::new(offset[0], offset[1], offset[2])),
                    rotation_delta: None,
                    new_scale: None,
                    look_at: None,
                });
                self.follow_exec(node_id, "then");
            }
            "transform/set_rotation" => {
                let rot = self.resolve_input(node_id, "rotation").as_vec3();
                self.transform_writes.push(TransformWrite {
                    entity: self.entity,
                    new_position: None,
                    new_rotation: Some(Vec3::new(rot[0], rot[1], rot[2])),
                    translation: None,
                    rotation_delta: None,
                    new_scale: None,
                    look_at: None,
                });
                self.follow_exec(node_id, "then");
            }
            "transform/rotate" => {
                let deg = self.resolve_input(node_id, "degrees").as_vec3();
                self.transform_writes.push(TransformWrite {
                    entity: self.entity,
                    new_position: None,
                    new_rotation: None,
                    translation: None,
                    rotation_delta: Some(Vec3::new(deg[0], deg[1], deg[2])),
                    new_scale: None,
                    look_at: None,
                });
                self.follow_exec(node_id, "then");
            }
            "transform/look_at" => {
                let target = self.resolve_input(node_id, "target").as_vec3();
                self.transform_writes.push(TransformWrite {
                    entity: self.entity,
                    new_position: None,
                    new_rotation: None,
                    translation: None,
                    rotation_delta: None,
                    new_scale: None,
                    look_at: Some(Vec3::new(target[0], target[1], target[2])),
                });
                self.follow_exec(node_id, "then");
            }
            "transform/set_scale" => {
                let s = self.resolve_input(node_id, "scale").as_vec3();
                self.transform_writes.push(TransformWrite {
                    entity: self.entity,
                    new_position: None,
                    new_rotation: None,
                    translation: None,
                    rotation_delta: None,
                    new_scale: Some(Vec3::new(s[0], s[1], s[2])),
                    look_at: None,
                });
                self.follow_exec(node_id, "then");
            }

            // ── Physics ──────────────────────────────────────────────
            "physics/apply_force" => {
                let f = self.resolve_input(node_id, "force").as_vec3();
                self.commands.push(ScriptCommand::ApplyForce {
                    entity_id: None,
                    force: Vec3::new(f[0], f[1], f[2]),
                });
                self.follow_exec(node_id, "then");
            }
            "physics/apply_impulse" => {
                let imp = self.resolve_input(node_id, "impulse").as_vec3();
                self.commands.push(ScriptCommand::ApplyImpulse {
                    entity_id: None,
                    impulse: Vec3::new(imp[0], imp[1], imp[2]),
                });
                self.follow_exec(node_id, "then");
            }
            "physics/set_velocity" => {
                let v = self.resolve_input(node_id, "velocity").as_vec3();
                self.commands.push(ScriptCommand::SetVelocity {
                    entity_id: None,
                    velocity: Vec3::new(v[0], v[1], v[2]),
                });
                self.follow_exec(node_id, "then");
            }

            // ── Audio ────────────────────────────────────────────────
            "audio/play_sound" => {
                let path = self.resolve_input(node_id, "path").as_string();
                let volume = self.resolve_input(node_id, "volume").as_float();
                let looping = self.resolve_input(node_id, "looping").as_bool();
                self.commands.push(ScriptCommand::PlaySound {
                    path,
                    volume,
                    looping,
                    bus: "sfx".into(),
                });
                self.follow_exec(node_id, "then");
            }
            "audio/play_music" => {
                let path = self.resolve_input(node_id, "path").as_string();
                let volume = self.resolve_input(node_id, "volume").as_float();
                let fade_in = self.resolve_input(node_id, "fade_in").as_float();
                self.commands.push(ScriptCommand::PlayMusic {
                    path,
                    volume,
                    fade_in,
                    bus: "music".into(),
                });
                self.follow_exec(node_id, "then");
            }
            "audio/stop_music" => {
                let fade_out = self.resolve_input(node_id, "fade_out").as_float();
                self.commands.push(ScriptCommand::StopMusic { fade_out });
                self.follow_exec(node_id, "then");
            }

            // ── Entity ───────────────────────────────────────────────
            "entity/spawn" => {
                let name = self.resolve_input(node_id, "name").as_string();
                self.commands.push(ScriptCommand::SpawnEntity { name });
                self.follow_exec(node_id, "then");
            }
            "entity/despawn" => {
                // For now, despawn by entity bits from context
                self.commands.push(ScriptCommand::DespawnSelf);
                self.follow_exec(node_id, "then");
            }
            "entity/despawn_self" => {
                self.commands.push(ScriptCommand::DespawnSelf);
            }

            // ── Rendering ────────────────────────────────────────────
            "rendering/set_visibility" => {
                let visible = self.resolve_input(node_id, "visible").as_bool();
                self.commands.push(ScriptCommand::SetVisibility {
                    entity_id: None,
                    visible,
                });
                self.follow_exec(node_id, "then");
            }
            "rendering/set_material_color" => {
                let c = self.resolve_input(node_id, "color").as_color();
                self.commands.push(ScriptCommand::SetMaterialColor {
                    entity_id: None,
                    color: c,
                });
                self.follow_exec(node_id, "then");
            }

            // ── Animation ────────────────────────────────────────────
            "animation/play" => {
                let name = self.resolve_input(node_id, "name").as_string();
                let looping = self.resolve_input(node_id, "looping").as_bool();
                let speed = self.resolve_input(node_id, "speed").as_float();
                self.commands.push(ScriptCommand::PlayAnimation {
                    entity_id: None,
                    name,
                    looping,
                    speed,
                });
                self.follow_exec(node_id, "then");
            }
            "animation/tween_position" => {
                let target = self.resolve_input(node_id, "target").as_vec3();
                let duration = self.resolve_input(node_id, "duration").as_float();
                let easing = self.resolve_input(node_id, "easing").as_string();
                self.commands.push(ScriptCommand::TweenPosition {
                    entity_id: None,
                    target: Vec3::new(target[0], target[1], target[2]),
                    duration,
                    easing,
                });
                self.follow_exec(node_id, "then");
            }

            // ── Debug ────────────────────────────────────────────────
            "debug/log" => {
                let message = self.resolve_input(node_id, "message").as_string();
                self.commands.push(ScriptCommand::Log {
                    level: "info".into(),
                    message,
                });
                self.follow_exec(node_id, "then");
            }
            "debug/draw_line" => {
                let start = self.resolve_input(node_id, "start").as_vec3();
                let end = self.resolve_input(node_id, "end").as_vec3();
                let color = self.resolve_input(node_id, "color").as_color();
                let duration = self.resolve_input(node_id, "duration").as_float();
                self.commands.push(ScriptCommand::DrawLine {
                    start: Vec3::new(start[0], start[1], start[2]),
                    end: Vec3::new(end[0], end[1], end[2]),
                    color,
                    duration,
                });
                self.follow_exec(node_id, "then");
            }

            // ── Scene ────────────────────────────────────────────────
            "scene/load" => {
                let path = self.resolve_input(node_id, "path").as_string();
                self.commands.push(ScriptCommand::LoadScene { path });
                self.follow_exec(node_id, "then");
            }

            // ── Timer ────────────────────────────────────────────────
            "flow/start_timer" => {
                let name = self.resolve_input(node_id, "name").as_string();
                let duration = self.resolve_input(node_id, "duration").as_float();
                let repeat = self.resolve_input(node_id, "repeat").as_bool();
                self.commands.push(ScriptCommand::StartTimer { name, duration, repeat });
                self.follow_exec(node_id, "then");
            }
            "flow/delay" => {
                // Delay implemented as a timer + deferred exec.
                // For now, just start a timer with a unique name.
                let timer_name = format!("__bp_delay_{}_{}", self.entity.index(), node_id);
                let duration = self.resolve_input(node_id, "duration").as_float();
                self.commands.push(ScriptCommand::StartTimer {
                    name: timer_name,
                    duration,
                    repeat: false,
                });
                // The "completed" exec will fire on the next ON_TIMER event.
                // TODO: wire delay completion through timer system.
            }

            // ── Component reflection ─────────────────────────────────
            "component/set_field" => {
                let component = self.resolve_input(node_id, "component").as_string();
                let field = self.resolve_input(node_id, "field").as_string();
                let value = self.resolve_input(node_id, "value");
                let prop_value = match value {
                    PinValue::Float(v) => PropertyValue::Float(v),
                    PinValue::Int(v) => PropertyValue::Int(v as i64),
                    PinValue::Bool(v) => PropertyValue::Bool(v),
                    PinValue::String(v) => PropertyValue::String(v),
                    PinValue::Vec3(v) => PropertyValue::Vec3(v),
                    PinValue::Color(v) => PropertyValue::Color(v),
                    _ => PropertyValue::Float(0.0),
                };
                self.commands.push(ScriptCommand::SetComponentField {
                    entity_id: None,
                    entity_name: None,
                    component_type: component,
                    field_path: field,
                    value: prop_value,
                });
                self.follow_exec(node_id, "then");
            }

            _ => {
                // Unknown node type — skip and continue.
                self.follow_exec(node_id, "then");
            }
        }
    }
}

// =============================================================================
// System
// =============================================================================

/// Exclusive system that evaluates all BlueprintGraph components.
/// Produces ScriptCommands and TransformWrites into the shared queue.
pub fn run_blueprints(world: &mut World) {
    let time_delta = world.resource::<Time>().delta_secs();
    let time_elapsed = world.resource::<Time>().elapsed_secs_f64();
    let input = world.resource::<ScriptInput>().clone();

    // Collect entities with blueprints.
    struct BpEntity {
        entity: Entity,
        entity_name: String,
        transform: Transform,
    }
    let mut bp_entities: Vec<BpEntity> = Vec::new();
    {
        let mut query = world.query::<(Entity, &BlueprintGraph, &Transform, Option<&Name>)>();
        for (entity, _graph, transform, name) in query.iter(world) {
            bp_entities.push(BpEntity {
                entity,
                entity_name: name.map(|n| n.as_str().to_string())
                    .unwrap_or_else(|| format!("Entity_{}", entity.index())),
                transform: *transform,
            });
        }
    }

    if bp_entities.is_empty() {
        return;
    }

    renzora_core::clog_info!("Blueprint", "Running {} blueprint(s)", bp_entities.len());

    for bpe in &bp_entities {
        // Take BlueprintGraph and runtime state off the entity.
        let Some(graph) = world.entity_mut(bpe.entity).take::<BlueprintGraph>() else { continue };
        let mut runtime = world.entity_mut(bpe.entity).take::<BlueprintRuntimeState>()
            .unwrap_or_default();

        let was_initialized = runtime.initialized;

        // Find event nodes before creating the eval context.
        let event_nodes: Vec<(NodeId, String)> = graph.event_nodes()
            .iter()
            .map(|n| (n.id, n.node_type.clone()))
            .collect();

        let (commands, transform_writes) = {
            let mut ctx = EvalContext {
                entity: bpe.entity,
                cache: HashMap::new(),
                graph: &graph,
                transform: &bpe.transform,
                input: &input,
                delta: time_delta,
                elapsed: time_elapsed,
                commands: Vec::new(),
                transform_writes: Vec::new(),
                runtime: &mut runtime,
                entity_name: bpe.entity_name.clone(),
            };

            for (node_id, node_type) in &event_nodes {
                match node_type.as_str() {
                    "event/on_ready" => {
                        if !was_initialized {
                            ctx.follow_exec(*node_id, "exec");
                        }
                    }
                    "event/on_update" => {
                        ctx.follow_exec(*node_id, "exec");
                    }
                    // TODO: on_collision, on_timer, on_message
                    _ => {}
                }
            }

            (ctx.commands, ctx.transform_writes)
        };

        runtime.initialized = true;

        // Push results into the shared command queue.
        renzora_core::clog_info!("Blueprint", "entity='{}' cmds={} tw={}", bpe.entity_name, commands.len(), transform_writes.len());
        for tw in &transform_writes {
            renzora_core::clog_info!("Blueprint", "TW entity={:?} rot_delta={:?}", tw.entity, tw.rotation_delta);
        }
        {
            let mut cmd_queue = world.resource_mut::<ScriptCommandQueue>();
            let before = cmd_queue.transform_writes.len();
            for cmd in commands {
                cmd_queue.commands.push((bpe.entity, cmd));
            }
            cmd_queue.transform_writes.extend(transform_writes);
            renzora_core::clog_info!("Blueprint", "Queue: before={} after={}", before, cmd_queue.transform_writes.len());
        }

        // Put components back.
        world.entity_mut(bpe.entity).insert(graph);
        world.entity_mut(bpe.entity).insert(runtime);
    }
}
