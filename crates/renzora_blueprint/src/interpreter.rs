#![allow(unreachable_patterns)]

//! Blueprint graph interpreter.
//!
//! Walks the graph starting from event nodes, evaluates data pins,
//! and produces `ScriptAction` events and `TransformWrite`s.
//!
//! This crate depends only on `renzora` — not on `renzora_scripting`.

use bevy::prelude::*;
use std::collections::HashMap;

use renzora::{
    ActionState, CharacterCommand, CharacterCommandQueue,
    PropertyValue, ScriptAction, ScriptActionValue, ScriptInput,
    TransformWrite, TransformWriteQueue,
};

use crate::graph::{BlueprintGraph, NodeId, PinValue};
use crate::nodes;

/// Look up an entity by its `Name` component.
fn resolve_entity_by_name(world: &World, name: &str) -> Option<Entity> {
    for archetype in world.archetypes().iter() {
        for arch_entity in archetype.entities() {
            let entity = arch_entity.id();
            if let Some(n) = world.get::<Name>(entity) {
                if n.as_str() == name {
                    return Some(entity);
                }
            }
        }
    }
    None
}

/// Tracks the previous frame's `PendingSceneLoad` list so blueprints can
/// detect the frame a scene finishes loading.
#[derive(Resource, Default)]
pub struct BlueprintSceneLoadTracker {
    last_pending: Vec<String>,
}

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
    /// Set by the animation system when a non-looping clip finishes.
    /// Consumed by the interpreter for `animation/on_finished` events.
    pub anim_finished_clip: Option<String>,
    /// User-defined variables stored per blueprint instance.
    pub variables: HashMap<String, PinValue>,
    /// Counter node accumulated values.
    pub counter_values: HashMap<NodeId, f32>,
}

/// Context passed through the graph during evaluation.
struct EvalContext<'a> {
    /// The entity owning this blueprint.
    entity: Entity,
    /// Cached pin output values (node_id, pin_name) -> value.
    cache: HashMap<(NodeId, String), PinValue>,
    /// The graph being evaluated.
    graph: &'a BlueprintGraph,
    /// Read-only world reference for reflection queries.
    world: &'a World,
    /// Transform of this entity.
    transform: &'a Transform,
    /// Input state.
    input: &'a ScriptInput,
    /// Action-mapped input state.
    action_state: &'a ActionState,
    /// Time info.
    delta: f32,
    elapsed: f64,
    /// Actions to emit as events.
    actions: Vec<ScriptAction>,
    /// Character commands to emit.
    character_commands: Vec<CharacterCommand>,
    /// Transform writes to emit.
    transform_writes: Vec<TransformWrite>,
    /// Runtime state for flow control.
    runtime: &'a mut BlueprintRuntimeState,
    /// Entity name.
    entity_name: String,
    /// Network state: is this instance a server?
    net_is_server: bool,
    /// Network state: is this instance connected?
    net_is_connected: bool,
    /// Name of a scene that just finished loading this frame, if any.
    scene_just_loaded: Option<String>,
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
            "lifecycle/on_scene_loaded" => {
                if pin_name == "scene" {
                    PinValue::String(self.scene_just_loaded.clone().unwrap_or_default())
                } else {
                    PinValue::None
                }
            }
            "lifecycle/global_get" => {
                let key = self.resolve_input(node_id, "key").as_string();
                self.world
                    .get_resource::<renzora_globals::GlobalStore>()
                    .and_then(|s| s.get(&key).cloned())
                    .unwrap_or(PinValue::None)
            }

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
                let (y, x, z) = self.transform.rotation.to_euler(EulerRot::YXZ);
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

            // ── Action-mapped input reads ─────────────────────────────
            "input/is_action_pressed" => {
                let action = self.resolve_input(node_id, "action").as_string();
                PinValue::Bool(self.action_state.pressed(&action))
            }
            "input/is_action_just_pressed" => {
                let action = self.resolve_input(node_id, "action").as_string();
                PinValue::Bool(self.action_state.just_pressed(&action))
            }
            "input/get_action_axis" => {
                let action = self.resolve_input(node_id, "action").as_string();
                PinValue::Float(self.action_state.axis_1d(&action))
            }
            "input/get_action_axis2d" => {
                let action = self.resolve_input(node_id, "action").as_string();
                let v = self.action_state.axis_2d(&action);
                match pin_name {
                    "value" => PinValue::Vec2([v.x, v.y]),
                    "x" => PinValue::Float(v.x),
                    "y" => PinValue::Float(v.y),
                    _ => PinValue::None,
                }
            }

            // ── Physics reads (from PhysicsReadState) ────────────────
            "physics/is_grounded" => {
                let g = renzora::reflection::get_reflected_field(
                    self.world, self.entity, "PhysicsReadState", "grounded"
                ).and_then(|v| match v { PropertyValue::Bool(b) => Some(b), _ => None })
                .unwrap_or(false);
                PinValue::Bool(g)
            }
            "physics/get_velocity" => {
                let read = |field: &str| -> f32 {
                    renzora::reflection::get_reflected_field(self.world, self.entity, "PhysicsReadState", field)
                        .and_then(|v| match v { PropertyValue::Float(f) => Some(f), _ => None })
                        .unwrap_or(0.0)
                };
                match pin_name {
                    "velocity" => PinValue::Vec3([read("velocity.x"), read("velocity.y"), read("velocity.z")]),
                    "speed" => PinValue::Float(read("speed")),
                    _ => PinValue::None,
                }
            }

            // ── Navigation reads (from NavReadState) ─────────────────
            "navigation/has_path" => {
                let v = renzora::reflection::get_reflected_field(
                    self.world, self.entity, "NavReadState", "has_path"
                ).and_then(|v| match v { PropertyValue::Bool(b) => Some(b), _ => None })
                .unwrap_or(false);
                PinValue::Bool(v)
            }
            "navigation/has_target" => {
                let v = renzora::reflection::get_reflected_field(
                    self.world, self.entity, "NavReadState", "has_target"
                ).and_then(|v| match v { PropertyValue::Bool(b) => Some(b), _ => None })
                .unwrap_or(false);
                PinValue::Bool(v)
            }
            "navigation/is_at_destination" => {
                let v = renzora::reflection::get_reflected_field(
                    self.world, self.entity, "NavReadState", "is_at_destination"
                ).and_then(|v| match v { PropertyValue::Bool(b) => Some(b), _ => None })
                .unwrap_or(false);
                PinValue::Bool(v)
            }
            "navigation/distance_to_destination" => {
                let v = renzora::reflection::get_reflected_field(
                    self.world, self.entity, "NavReadState", "distance_to_destination"
                ).and_then(|v| match v { PropertyValue::Float(f) => Some(f), _ => None })
                .unwrap_or(0.0);
                PinValue::Float(v)
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

            // ── Animation data queries ──────────────────────────────
            // These return placeholder values; the actual runtime values
            // are filled in by the animation system when it provides
            // query context to the blueprint interpreter.
            "animation/get_time" => match pin_name {
                "time" => PinValue::Float(self.elapsed as f32),
                _ => PinValue::None,
            },
            "animation/is_playing" => match pin_name {
                "playing" => PinValue::Bool(false),
                _ => PinValue::None,
            },
            "animation/get_length" => {
                let name = self.resolve_input(node_id, "name").as_string();
                let v = renzora::reflection::get_reflected_field(
                    self.world,
                    self.entity,
                    "AnimatorReadState",
                    &format!("clip_lengths.{}", name),
                )
                .and_then(|v| match v { PropertyValue::Float(f) => Some(f), _ => None })
                .unwrap_or(0.0);
                PinValue::Float(v)
            }
            "animation/get_param" => {
                let name = self.resolve_input(node_id, "name").as_string();
                let v = renzora::reflection::get_reflected_field(
                    self.world,
                    self.entity,
                    "AnimatorReadState",
                    &format!("params.{}", name),
                )
                .and_then(|v| match v { PropertyValue::Float(f) => Some(f), _ => None })
                .unwrap_or(0.0);
                PinValue::Float(v)
            }
            "animation/get_bool" => {
                let name = self.resolve_input(node_id, "name").as_string();
                let v = renzora::reflection::get_reflected_field(
                    self.world,
                    self.entity,
                    "AnimatorReadState",
                    &format!("bool_params.{}", name),
                )
                .and_then(|v| match v { PropertyValue::Bool(b) => Some(b), _ => None })
                .unwrap_or(false);
                PinValue::Bool(v)
            }

            // ── Component reflection read ─────────────────────────────
            "component/get_field" => {
                let entity_val = self.resolve_input(node_id, "entity").as_string();
                let component = self.resolve_input(node_id, "component").as_string();
                let field = self.resolve_input(node_id, "field").as_string();

                // Resolve target entity: empty = self, otherwise look up by name.
                let target = if entity_val.is_empty() {
                    Some(self.entity)
                } else {
                    resolve_entity_by_name(self.world, &entity_val)
                };

                match target {
                    Some(e) => {
                        let result = renzora::reflection::get_reflected_field(
                            self.world, e, &component, &field,
                        );
                        match result {
                            Some(PropertyValue::Float(v)) => PinValue::Float(v),
                            Some(PropertyValue::Int(v)) => PinValue::Int(v as i32),
                            Some(PropertyValue::Bool(v)) => PinValue::Bool(v),
                            Some(PropertyValue::String(v)) => PinValue::String(v),
                            Some(PropertyValue::Vec3(v)) => PinValue::Vec3(v),
                            Some(PropertyValue::Color(v)) => PinValue::Color(v),
                            _ => PinValue::None,
                        }
                    }
                    None => PinValue::None,
                }
            }

            // ── Variable read ─────────────────────────────────────────
            "variable/get" => {
                let name = self.resolve_input(node_id, "name").as_string();
                self.runtime.variables.get(&name).cloned().unwrap_or(PinValue::None)
            }

            // ── Counter read ──────────────────────────────────────────
            "flow/counter" => {
                let val = self.runtime.counter_values.get(&node_id).copied().unwrap_or(0.0);
                PinValue::Float(val)
            }

            // ── Entity self ───────────────────────────────────────────
            "entity/get_self" => match pin_name {
                "entity" => PinValue::Entity(self.entity_name.clone()),
                _ => PinValue::None,
            },

            // ── Entity by name ────────────────────────────────────────
            "entity/get_entity" => {
                let name = self.resolve_input(node_id, "name").as_string();
                PinValue::Entity(name)
            }

            // ── Additional math ───────────────────────────────────────
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
            "math/floor" => PinValue::Float(self.resolve_input(node_id, "value").as_float().floor()),
            "math/ceil" => PinValue::Float(self.resolve_input(node_id, "value").as_float().ceil()),
            "math/round" => PinValue::Float(self.resolve_input(node_id, "value").as_float().round()),
            "math/modulo" => {
                let a = self.resolve_input(node_id, "a").as_float();
                let b = self.resolve_input(node_id, "b").as_float();
                PinValue::Float(if b != 0.0 { a % b } else { 0.0 })
            }
            "math/distance" => {
                let a = self.resolve_input(node_id, "a").as_vec3();
                let b = self.resolve_input(node_id, "b").as_vec3();
                let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
                PinValue::Float((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt())
            }
            "math/dot" => {
                let a = self.resolve_input(node_id, "a").as_vec3();
                let b = self.resolve_input(node_id, "b").as_vec3();
                PinValue::Float(a[0] * b[0] + a[1] * b[1] + a[2] * b[2])
            }
            "math/cross" => {
                let a = self.resolve_input(node_id, "a").as_vec3();
                let b = self.resolve_input(node_id, "b").as_vec3();
                PinValue::Vec3([
                    a[1] * b[2] - a[2] * b[1],
                    a[2] * b[0] - a[0] * b[2],
                    a[0] * b[1] - a[1] * b[0],
                ])
            }
            "math/normalize" => {
                let v = self.resolve_input(node_id, "value").as_vec3();
                let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                if len > 0.0 {
                    PinValue::Vec3([v[0] / len, v[1] / len, v[2] / len])
                } else {
                    PinValue::Vec3([0.0, 0.0, 0.0])
                }
            }

            // ── String ops ────────────────────────────────────────────
            "string/concat" => {
                let a = self.resolve_input(node_id, "a").as_string();
                let b = self.resolve_input(node_id, "b").as_string();
                PinValue::String(format!("{}{}", a, b))
            }
            "string/format" => {
                let template = self.resolve_input(node_id, "template").as_string();
                let value = self.resolve_input(node_id, "value");
                // Simple {0} replacement.
                let replacement = match value {
                    PinValue::Float(v) => format!("{:.2}", v),
                    PinValue::Int(v) => format!("{}", v),
                    PinValue::Bool(v) => format!("{}", v),
                    PinValue::String(v) => v,
                    PinValue::Vec3([x, y, z]) => format!("{:.1}, {:.1}, {:.1}", x, y, z),
                    _ => String::new(),
                };
                PinValue::String(template.replace("{0}", &replacement))
            }
            "string/to_float" => PinValue::Float(self.resolve_input(node_id, "value").as_string().parse().unwrap_or(0.0)),
            "string/to_int" => PinValue::Int(self.resolve_input(node_id, "value").as_string().parse().unwrap_or(0)),

            // ── Type conversion ───────────────────────────────────────
            "convert/to_string" => {
                let val = self.resolve_input(node_id, "value");
                PinValue::String(val.as_string())
            }
            "convert/to_float" => PinValue::Float(self.resolve_input(node_id, "value").as_float()),
            "convert/to_int" => PinValue::Int(self.resolve_input(node_id, "value").as_int()),
            "convert/to_bool" => PinValue::Bool(self.resolve_input(node_id, "value").as_bool()),

            // ── Network data queries ────────────────────────────────
            "network/is_server" => PinValue::Bool(self.net_is_server),
            "network/is_connected" => PinValue::Bool(self.net_is_connected),

            _ => PinValue::None,
        }
    }

    /// Push a ScriptAction with string args.
    fn push_action<const N: usize>(&mut self, name: &str, args: [(&str, String); N]) {
        let mut map = HashMap::new();
        for (k, v) in args {
            map.insert(k.to_string(), ScriptActionValue::String(v));
        }
        self.actions.push(ScriptAction {
            name: name.to_string(),
            entity: self.entity,
            target_entity: None,
            args: map,
        });
    }

    /// Push a ScriptAction with mixed arg types.
    fn push_action_mixed(&mut self, name: &str, str_args: &[(&str, ScriptActionValue)], float_args: &[(&str, f32)]) {
        let mut map = HashMap::new();
        for (k, v) in str_args {
            map.insert(k.to_string(), v.clone());
        }
        for (k, v) in float_args {
            map.insert(k.to_string(), ScriptActionValue::Float(*v));
        }
        self.actions.push(ScriptAction {
            name: name.to_string(),
            entity: self.entity,
            target_entity: None,
            args: map,
        });
    }

    /// Push a ScriptAction with Vec3 args.
    fn push_action_vec3(&mut self, name: &str, str_args: &[(&str, ScriptActionValue)], vec3_args: &[(&str, [f32; 3])]) {
        let mut map = HashMap::new();
        for (k, v) in str_args {
            map.insert(k.to_string(), v.clone());
        }
        for (k, v) in vec3_args {
            map.insert(k.to_string(), ScriptActionValue::Vec3(*v));
        }
        self.actions.push(ScriptAction {
            name: name.to_string(),
            entity: self.entity,
            target_entity: None,
            args: map,
        });
    }

    /// Push a ScriptAction targeting a specific entity (by name).
    fn push_action_targeted(&mut self, name: &str, target: Option<String>, args: HashMap<String, ScriptActionValue>) {
        self.actions.push(ScriptAction {
            name: name.to_string(),
            entity: self.entity,
            target_entity: target,
            args,
        });
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

            "flow/counter" => {
                let step = self.resolve_input(node_id, "step").as_float();
                let min = self.resolve_input(node_id, "min").as_float();
                let max = self.resolve_input(node_id, "max").as_float();
                let do_loop = self.resolve_input(node_id, "loop").as_bool();
                let val = self.runtime.counter_values.entry(node_id).or_insert(min);
                *val += step * self.delta;
                if *val > max {
                    if do_loop { *val = min + (*val - max); } else { *val = max; }
                }
                self.follow_exec(node_id, "then");
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
                self.push_action_vec3("apply_force", &[], &[("x", [f[0], f[1], f[2]])]);
                self.follow_exec(node_id, "then");
            }
            "physics/apply_impulse" => {
                let imp = self.resolve_input(node_id, "impulse").as_vec3();
                self.push_action_vec3("apply_impulse", &[], &[("x", [imp[0], imp[1], imp[2]])]);
                self.follow_exec(node_id, "then");
            }
            "physics/set_velocity" => {
                let v = self.resolve_input(node_id, "velocity").as_vec3();
                self.push_action_vec3("set_velocity", &[], &[("x", [v[0], v[1], v[2]])]);
                self.follow_exec(node_id, "then");
            }
            "physics/kinematic_slide" => {
                let d = self.resolve_input(node_id, "delta").as_vec3();
                self.push_action_vec3("kinematic_slide", &[], &[("x", [d[0], d[1], d[2]])]);
                self.follow_exec(node_id, "then");
            }

            // ── Navigation ───────────────────────────────────────────
            "navigation/set_destination" => {
                let t = self.resolve_input(node_id, "target").as_vec3();
                self.push_action_vec3("nav_set_destination", &[], &[("target", t)]);
                self.follow_exec(node_id, "then");
            }
            "navigation/clear_destination" => {
                self.push_action("nav_clear_destination", []);
                self.follow_exec(node_id, "then");
            }

            // ── Character Controller ─────────────────────────────────

            // ── Audio ────────────────────────────────────────────────
            "audio/play_sound" => {
                let path = self.resolve_input(node_id, "path").as_string();
                let volume = self.resolve_input(node_id, "volume").as_float();
                let looping = self.resolve_input(node_id, "looping").as_bool();
                self.push_action_mixed("play_sound", &[
                    ("path", ScriptActionValue::String(path)),
                    ("looping", ScriptActionValue::Bool(looping)),
                    ("bus", ScriptActionValue::String("sfx".into())),
                ], &[("volume", volume)]);
                self.follow_exec(node_id, "then");
            }
            "audio/play_music" => {
                let path = self.resolve_input(node_id, "path").as_string();
                let volume = self.resolve_input(node_id, "volume").as_float();
                let fade_in = self.resolve_input(node_id, "fade_in").as_float();
                self.push_action_mixed("play_music", &[
                    ("path", ScriptActionValue::String(path)),
                    ("bus", ScriptActionValue::String("music".into())),
                ], &[("volume", volume), ("fade_in", fade_in)]);
                self.follow_exec(node_id, "then");
            }
            "audio/stop_music" => {
                let fade_out = self.resolve_input(node_id, "fade_out").as_float();
                self.push_action_mixed("stop_music", &[], &[("fade_out", fade_out)]);
                self.follow_exec(node_id, "then");
            }

            // ── Entity ───────────────────────────────────────────────
            "entity/spawn" => {
                let name = self.resolve_input(node_id, "name").as_string();
                self.push_action("spawn_entity", [("name", name)]);
                self.follow_exec(node_id, "then");
            }
            "entity/despawn" => {
                self.push_action("despawn_self", [("_", String::new())]);
                self.follow_exec(node_id, "then");
            }
            "entity/despawn_self" => {
                self.push_action("despawn_self", [("_", String::new())]);
            }

            // ── Rendering ────────────────────────────────────────────
            "rendering/set_visibility" => {
                let visible = self.resolve_input(node_id, "visible").as_bool();
                self.push_action_mixed("set_visibility", &[
                    ("visible", ScriptActionValue::Bool(visible)),
                ], &[]);
                self.follow_exec(node_id, "then");
            }
            "rendering/set_material_color" => {
                let c = self.resolve_input(node_id, "color").as_color();
                self.push_action_mixed("set_material_color", &[], &[
                    ("r", c[0]), ("g", c[1]), ("b", c[2]), ("a", c[3]),
                ]);
                self.follow_exec(node_id, "then");
            }

            // ── Animation ────────────────────────────────────────────
            "animation/play" => {
                let name = self.resolve_input(node_id, "name").as_string();
                let looping = self.resolve_input(node_id, "looping").as_bool();
                let speed = self.resolve_input(node_id, "speed").as_float();
                self.push_action_mixed("play_animation", &[
                    ("name", ScriptActionValue::String(name)),
                    ("looping", ScriptActionValue::Bool(looping)),
                ], &[("speed", speed)]);
                self.follow_exec(node_id, "then");
            }
            "animation/stop" => {
                self.push_action("stop_animation", [("_", String::new())]);
                self.follow_exec(node_id, "then");
            }
            "animation/pause" => {
                self.push_action("pause_animation", [("_", String::new())]);
                self.follow_exec(node_id, "then");
            }
            "animation/resume" => {
                self.push_action("resume_animation", [("_", String::new())]);
                self.follow_exec(node_id, "then");
            }
            "animation/set_speed" => {
                let speed = self.resolve_input(node_id, "speed").as_float();
                self.push_action_mixed("set_animation_speed", &[], &[("speed", speed)]);
                self.follow_exec(node_id, "then");
            }
            "animation/crossfade" => {
                let name = self.resolve_input(node_id, "name").as_string();
                let duration = self.resolve_input(node_id, "duration").as_float();
                let looping = self.resolve_input(node_id, "looping").as_bool();
                self.push_action_mixed("crossfade_animation", &[
                    ("name", ScriptActionValue::String(name)),
                    ("looping", ScriptActionValue::Bool(looping)),
                ], &[("duration", duration)]);
                self.follow_exec(node_id, "then");
            }
            "animation/set_param" => {
                let name = self.resolve_input(node_id, "name").as_string();
                let value = self.resolve_input(node_id, "value").as_float();
                self.push_action_mixed("set_animation_param", &[
                    ("name", ScriptActionValue::String(name)),
                ], &[("value", value)]);
                self.follow_exec(node_id, "then");
            }
            "animation/set_bool_param" => {
                let name = self.resolve_input(node_id, "name").as_string();
                let value = self.resolve_input(node_id, "value").as_bool();
                self.push_action_mixed("set_animation_bool_param", &[
                    ("name", ScriptActionValue::String(name)),
                    ("value", ScriptActionValue::Bool(value)),
                ], &[]);
                self.follow_exec(node_id, "then");
            }
            "animation/trigger" => {
                let name = self.resolve_input(node_id, "name").as_string();
                self.push_action("trigger_animation", [("name", name)]);
                self.follow_exec(node_id, "then");
            }
            "animation/set_layer_weight" => {
                let layer_name = self.resolve_input(node_id, "layer").as_string();
                let weight = self.resolve_input(node_id, "weight").as_float();
                self.push_action_mixed("set_animation_layer_weight", &[
                    ("layer_name", ScriptActionValue::String(layer_name)),
                ], &[("weight", weight)]);
                self.follow_exec(node_id, "then");
            }
            "animation/tween_position" => {
                let target = self.resolve_input(node_id, "target").as_vec3();
                let duration = self.resolve_input(node_id, "duration").as_float();
                let easing = self.resolve_input(node_id, "easing").as_string();
                self.push_action_mixed("tween_position", &[
                    ("easing", ScriptActionValue::String(easing)),
                ], &[("tx", target[0]), ("ty", target[1]), ("tz", target[2]), ("duration", duration)]);
                self.follow_exec(node_id, "then");
            }

            // ── Debug ────────────────────────────────────────────────
            "debug/log" => {
                let message = self.resolve_input(node_id, "message").as_string();
                log::info!("[Blueprint] {}", message);
                self.push_action("log", [("message", message)]);
                self.follow_exec(node_id, "then");
            }
            "debug/draw_line" => {
                let start = self.resolve_input(node_id, "start").as_vec3();
                let end = self.resolve_input(node_id, "end").as_vec3();
                let color = self.resolve_input(node_id, "color").as_color();
                let duration = self.resolve_input(node_id, "duration").as_float();
                self.push_action_mixed("draw_line", &[], &[
                    ("sx", start[0]), ("sy", start[1]), ("sz", start[2]),
                    ("ex", end[0]), ("ey", end[1]), ("ez", end[2]),
                    ("r", color[0]), ("g", color[1]), ("b", color[2]), ("a", color[3]),
                    ("duration", duration),
                ]);
                self.follow_exec(node_id, "then");
            }

            // ── UI (via generic ScriptAction) ───────────────────────
            "ui/show" => {
                let name = self.resolve_input(node_id, "path").as_string();
                self.push_action("ui_show", [("name", name)]);
                self.follow_exec(node_id, "then");
            }
            "ui/hide" => {
                let name = self.resolve_input(node_id, "path").as_string();
                self.push_action("ui_hide", [("name", name)]);
                self.follow_exec(node_id, "then");
            }
            "ui/toggle" => {
                let name = self.resolve_input(node_id, "name").as_string();
                self.push_action("ui_toggle", [("name", name)]);
                self.follow_exec(node_id, "then");
            }
            "ui/set_text" => {
                let name = self.resolve_input(node_id, "element").as_string();
                let text = self.resolve_input(node_id, "text").as_string();
                self.push_action("ui_set_text", [("name", name), ("text", text)]);
                self.follow_exec(node_id, "then");
            }
            "ui/set_progress" => {
                let name = self.resolve_input(node_id, "element").as_string();
                let value = self.resolve_input(node_id, "value").as_float();
                self.push_action_mixed("ui_set_progress", &[("name", renzora::ScriptActionValue::String(name))], &[("value", value)]);
                self.follow_exec(node_id, "then");
            }
            "ui/set_health" => {
                let name = self.resolve_input(node_id, "element").as_string();
                let current = self.resolve_input(node_id, "current").as_float();
                let max = self.resolve_input(node_id, "max").as_float();
                self.push_action_mixed("ui_set_health", &[("name", renzora::ScriptActionValue::String(name))], &[("current", current), ("max", max)]);
                self.follow_exec(node_id, "then");
            }
            "ui/set_slider" => {
                let name = self.resolve_input(node_id, "element").as_string();
                let value = self.resolve_input(node_id, "value").as_float();
                self.push_action_mixed("ui_set_slider", &[("name", renzora::ScriptActionValue::String(name))], &[("value", value)]);
                self.follow_exec(node_id, "then");
            }
            "ui/set_checkbox" => {
                let name = self.resolve_input(node_id, "element").as_string();
                let checked = self.resolve_input(node_id, "checked").as_bool();
                self.push_action_mixed("ui_set_checkbox", &[("name", renzora::ScriptActionValue::String(name)), ("checked", renzora::ScriptActionValue::Bool(checked))], &[]);
                self.follow_exec(node_id, "then");
            }
            "ui/set_toggle" => {
                let name = self.resolve_input(node_id, "element").as_string();
                let on = self.resolve_input(node_id, "on").as_bool();
                self.push_action_mixed("ui_set_toggle", &[("name", renzora::ScriptActionValue::String(name)), ("on", renzora::ScriptActionValue::Bool(on))], &[]);
                self.follow_exec(node_id, "then");
            }
            "ui/set_visible" => {
                let name = self.resolve_input(node_id, "element").as_string();
                let name = if name.is_empty() { self.entity_name.clone() } else { name };
                let visible = self.resolve_input(node_id, "visible").as_bool();
                self.push_action_mixed("ui_set_visible", &[("name", renzora::ScriptActionValue::String(name)), ("visible", renzora::ScriptActionValue::Bool(visible))], &[]);
                self.follow_exec(node_id, "then");
            }
            "ui/set_theme" => {
                let theme_name = self.resolve_input(node_id, "theme").as_string();
                self.push_action("ui_set_theme", [("theme", theme_name)]);
                self.follow_exec(node_id, "then");
            }
            "ui/set_color" => {
                let name = self.resolve_input(node_id, "element").as_string();
                let color = self.resolve_input(node_id, "color").as_color();
                self.push_action_mixed("ui_set_color", &[("name", renzora::ScriptActionValue::String(name))], &[("r", color[0]), ("g", color[1]), ("b", color[2]), ("a", color[3])]);
                self.follow_exec(node_id, "then");
            }

            // ── Scene ────────────────────────────────────────────────
            "scene/load" => {
                let path = self.resolve_input(node_id, "path").as_string();
                self.push_action("load_scene", [("path", path)]);
                self.follow_exec(node_id, "then");
            }

            // ── Lifecycle ────────────────────────────────────────────
            "lifecycle/global_set" => {
                let key = self.resolve_input(node_id, "key").as_string();
                let value = self.resolve_input(node_id, "value").as_string();
                self.push_action("global_set", [("key", key), ("value", value)]);
                self.follow_exec(node_id, "then");
            }

            // ── Timer ────────────────────────────────────────────────
            "flow/start_timer" => {
                let name = self.resolve_input(node_id, "name").as_string();
                let duration = self.resolve_input(node_id, "duration").as_float();
                let repeat = self.resolve_input(node_id, "repeat").as_bool();
                self.push_action_mixed("start_timer", &[
                    ("name", ScriptActionValue::String(name)),
                    ("repeat", ScriptActionValue::Bool(repeat)),
                ], &[("duration", duration)]);
                self.follow_exec(node_id, "then");
            }
            "flow/delay" => {
                // Delay implemented as a timer + deferred exec.
                // For now, just start a timer with a unique name.
                let timer_name = format!("__bp_delay_{}_{}", self.entity.index(), node_id);
                let duration = self.resolve_input(node_id, "duration").as_float();
                self.push_action_mixed("start_timer", &[
                    ("name", ScriptActionValue::String(timer_name)),
                    ("repeat", ScriptActionValue::Bool(false)),
                ], &[("duration", duration)]);
                // The "completed" exec will fire on the next ON_TIMER event.
                // TODO: wire delay completion through timer system.
            }

            // ── Variable set ─────────────────────────────────────────
            "variable/set" => {
                let name = self.resolve_input(node_id, "name").as_string();
                let value = self.resolve_input(node_id, "value");
                self.runtime.variables.insert(name, value);
                self.follow_exec(node_id, "then");
            }

            // ── Component reflection ─────────────────────────────────
            "component/set_field" => {
                let entity_val = self.resolve_input(node_id, "entity").as_string();
                let component = self.resolve_input(node_id, "component").as_string();
                let field = self.resolve_input(node_id, "field").as_string();
                let value = self.resolve_input(node_id, "value");
                // Convert PinValue to ScriptActionValue for the action args
                let value_sav = match &value {
                    PinValue::Float(v) => ScriptActionValue::Float(*v),
                    PinValue::Int(v) => ScriptActionValue::Float(*v as f32),
                    PinValue::Bool(v) => ScriptActionValue::Bool(*v),
                    PinValue::String(v) => ScriptActionValue::String(v.clone()),
                    PinValue::Vec3(v) => ScriptActionValue::Vec3(*v),
                    PinValue::Color(v) => ScriptActionValue::Vec3([v[0], v[1], v[2]]),
                    _ => ScriptActionValue::Float(0.0),
                };
                // Empty entity = self, otherwise target by name.
                let target = if entity_val.is_empty() { None } else { Some(entity_val) };
                let mut args = HashMap::new();
                args.insert("component".to_string(), ScriptActionValue::String(component));
                args.insert("field".to_string(), ScriptActionValue::String(field));
                args.insert("value".to_string(), value_sav);
                self.push_action_targeted("set_component_field", target, args);
                self.follow_exec(node_id, "then");
            }

            // ── Network (via generic ScriptAction) ─────────────────
            "network/send_message" => {
                let channel = self.resolve_input(node_id, "channel").as_string();
                let data = self.resolve_input(node_id, "data").as_string();
                self.push_action("net_send", [("channel", channel), ("data", data)]);
                self.follow_exec(node_id, "then");
            }
            "network/spawn" => {
                let name = self.resolve_input(node_id, "name").as_string();
                let pos = self.resolve_input(node_id, "position").as_vec3();
                self.push_action_mixed("net_spawn", &[("name", renzora::ScriptActionValue::String(name))], &[("x", pos[0]), ("y", pos[1]), ("z", pos[2])]);
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
/// Produces `ScriptAction` events and `TransformWrite`s into the shared queues.
pub fn run_blueprints(world: &mut World) {
    let time_delta = world.resource::<Time>().delta_secs();
    let time_elapsed = world.resource::<Time>().elapsed_secs_f64();
    let input = world.resource::<ScriptInput>().clone();
    let action_state = world.resource::<ActionState>().clone();

    // Detect scene-load completion (diffs PendingSceneLoad frame to frame).
    let pending_now = world
        .get_resource::<renzora::PendingSceneLoad>()
        .map(|p| p.requests.clone())
        .unwrap_or_default();
    let scene_just_loaded = {
        let mut tracker = world.resource_mut::<BlueprintSceneLoadTracker>();
        let loaded = if !tracker.last_pending.is_empty() && pending_now.is_empty() {
            tracker.last_pending.last().cloned()
        } else {
            None
        };
        tracker.last_pending = pending_now;
        loaded
    };

    // Collect entities with blueprints.
    struct BpEntity {
        entity: Entity,
        entity_name: String,
        transform: Transform,
    }
    let mut bp_entities: Vec<BpEntity> = Vec::new();
    {
        let mut query = world.query::<(Entity, &BlueprintGraph, Option<&Transform>, Option<&Name>)>();
        for (entity, _graph, transform, name) in query.iter(world) {
            bp_entities.push(BpEntity {
                entity,
                entity_name: name.map(|n| n.as_str().to_string())
                    .unwrap_or_else(|| format!("Entity_{}", entity.index())),
                transform: transform.copied().unwrap_or_default(),
            });
        }
    }

    if bp_entities.is_empty() {
        return;
    }

    renzora::clog_info!("Blueprint", "Running {} blueprint(s)", bp_entities.len());

    for bpe in &bp_entities {
        // Take BlueprintGraph and runtime state off the entity.
        let Some(graph) = world.entity_mut(bpe.entity).take::<BlueprintGraph>() else { continue };
        let mut runtime = world.entity_mut(bpe.entity).take::<BlueprintRuntimeState>()
            .unwrap_or_default();

        let was_initialized = runtime.initialized;

        // Check if the animation system flagged a clip as finished this frame.
        let anim_finished_clip: Option<String> = runtime.anim_finished_clip.take();

        // Find event nodes before creating the eval context.
        let event_nodes: Vec<(NodeId, String)> = graph.event_nodes()
            .iter()
            .map(|n| (n.id, n.node_type.clone()))
            .collect();

        let (actions, transform_writes, character_commands) = {
            // Network status — defaults to false when network crate isn't loaded.
            let (net_is_server, net_is_connected) = (false, false);

            let mut ctx = EvalContext {
                entity: bpe.entity,
                cache: HashMap::new(),
                graph: &graph,
                world: &world,
                transform: &bpe.transform,
                input: &input,
                action_state: &action_state,
                delta: time_delta,
                elapsed: time_elapsed,
                actions: Vec::new(),
                character_commands: Vec::new(),
                transform_writes: Vec::new(),
                runtime: &mut runtime,
                entity_name: bpe.entity_name.clone(),
                net_is_server,
                net_is_connected,
                scene_just_loaded: scene_just_loaded.clone(),
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
                    "animation/on_finished" => {
                        if let Some(ref clip_name) = anim_finished_clip {
                            ctx.cache.insert((*node_id, "name".to_string()), PinValue::String(clip_name.clone()));
                            ctx.follow_exec(*node_id, "exec");
                        }
                    }
                    "lifecycle/on_scene_loaded" => {
                        if let Some(ref scene) = scene_just_loaded {
                            ctx.cache.insert(
                                (*node_id, "scene".to_string()),
                                PinValue::String(scene.clone()),
                            );
                            ctx.follow_exec(*node_id, "exec");
                        }
                    }
                    // TODO: on_collision, on_timer, on_message
                    _ => {}
                }
            }

            (ctx.actions, ctx.transform_writes, ctx.character_commands)
        };

        runtime.initialized = true;

        // Push transform writes into the shared TransformWriteQueue.
        renzora::clog_info!("Blueprint", "entity='{}' actions={} tw={} cc={}", bpe.entity_name, actions.len(), transform_writes.len(), character_commands.len());
        for tw in &transform_writes {
            renzora::clog_info!("Blueprint", "TW entity={:?} rot_delta={:?}", tw.entity, tw.rotation_delta);
        }
        {
            let mut tw_queue = world.resource_mut::<TransformWriteQueue>();
            tw_queue.writes.extend(transform_writes);
        }

        // Push character commands into CharacterCommandQueue.
        if !character_commands.is_empty() {
            let mut cc_queue = world.resource_mut::<CharacterCommandQueue>();
            for cc in character_commands {
                cc_queue.commands.push((bpe.entity, cc));
            }
        }

        // Apply global_set actions directly to the GlobalStore; trigger the rest as events.
        let mut remaining = Vec::with_capacity(actions.len());
        for action in actions {
            if action.name == "global_set" {
                let key = match action.args.get("key") {
                    Some(ScriptActionValue::String(s)) => s.clone(),
                    _ => continue,
                };
                let value = action
                    .args
                    .get("value")
                    .cloned()
                    .map(|v| match v {
                        ScriptActionValue::String(s) => PinValue::String(s),
                        ScriptActionValue::Bool(b) => PinValue::Bool(b),
                        ScriptActionValue::Int(i) => PinValue::Float(i as f32),
                        ScriptActionValue::Float(f) => PinValue::Float(f),
                        ScriptActionValue::Vec3(v) => PinValue::Vec3(v),
                    })
                    .unwrap_or(PinValue::None);
                if let Some(mut store) = world.get_resource_mut::<renzora_globals::GlobalStore>() {
                    store.set(key, value);
                }
            } else {
                remaining.push(action);
            }
        }
        for action in remaining {
            world.trigger(action);
        }

        // Put components back.
        world.entity_mut(bpe.entity).insert(graph);
        world.entity_mut(bpe.entity).insert(runtime);
    }
}
