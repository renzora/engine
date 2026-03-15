//! Blueprint → Lua compiler.
//!
//! Walks the blueprint graph and emits equivalent Lua source code.
//! The generated script uses the same API functions available to hand-written
//! Lua scripts, so it runs at full VM speed through the normal script pipeline.

use std::collections::{HashMap, HashSet};
use crate::graph::{BlueprintGraph, BlueprintNode, NodeId, PinValue};

/// Compile a blueprint graph to Lua source code.
pub fn compile_to_lua(graph: &BlueprintGraph) -> String {
    let mut compiler = LuaCompiler::new(graph);
    compiler.compile()
}

struct LuaCompiler<'a> {
    graph: &'a BlueprintGraph,
    /// Counter for generating unique temp variable names.
    temp_counter: u32,
    /// Cache: (node_id, pin_name) → Lua expression string.
    /// Prevents re-evaluating the same data node multiple times.
    expr_cache: HashMap<(NodeId, String), String>,
    /// Nodes whose output is referenced more than once → need a local variable.
    multi_ref: HashSet<(NodeId, String)>,
    /// Collected lines of Lua code for the current function body.
    lines: Vec<String>,
    /// Current indentation level.
    indent: usize,
    /// Variables used (for blueprint variable get/set).
    variables_used: HashSet<String>,
}

impl<'a> LuaCompiler<'a> {
    fn new(graph: &'a BlueprintGraph) -> Self {
        Self {
            graph,
            temp_counter: 0,
            expr_cache: HashMap::new(),
            multi_ref: HashSet::new(),
            lines: Vec::new(),
            indent: 1,
            variables_used: HashSet::new(),
        }
    }

    fn compile(&mut self) -> String {
        // Pre-scan to find multi-referenced outputs.
        self.scan_multi_refs();

        let event_nodes: Vec<(NodeId, String)> = self.graph.event_nodes()
            .iter()
            .map(|n| (n.id, n.node_type.clone()))
            .collect();

        let mut sections: Vec<String> = Vec::new();

        // Emit variable declarations at top.
        sections.push("-- Generated from Blueprint graph".to_string());

        // Gather all variable names by scanning variable/set and variable/get nodes.
        for node in &self.graph.nodes {
            if node.node_type == "variable/set" || node.node_type == "variable/get" {
                if let Some(PinValue::String(name)) = node.get_input_value("name") {
                    if !name.is_empty() {
                        self.variables_used.insert(name.clone());
                    }
                }
            }
        }

        // Declare counter variables.
        let counter_nodes: Vec<u64> = self.graph.nodes.iter()
            .filter(|n| n.node_type == "flow/counter")
            .map(|n| n.id)
            .collect();
        for id in &counter_nodes {
            sections.push(format!("local _counter_{} = nil", id));
        }

        if !self.variables_used.is_empty() {
            let mut vars: Vec<&String> = self.variables_used.iter().collect();
            vars.sort();
            for var in &vars {
                sections.push(format!("local {} = 0", sanitize_var(var)));
            }
        }

        if !counter_nodes.is_empty() || !self.variables_used.is_empty() {
            sections.push(String::new());
        }

        // Compile each event node into a function.
        for (node_id, node_type) in &event_nodes {
            self.lines.clear();
            self.expr_cache.clear();
            self.temp_counter = 0;
            self.indent = 1;

            match node_type.as_str() {
                "event/on_ready" => {
                    self.compile_exec_chain(*node_id, "exec");
                    let body = self.lines.join("\n");
                    sections.push(format!("function on_ready()\n{}\nend\n", body));
                }
                "event/on_update" => {
                    self.compile_exec_chain(*node_id, "exec");
                    let body = self.lines.join("\n");
                    sections.push(format!("function on_update()\n{}\nend\n", body));
                }
                _ => {}
            }
        }

        sections.join("\n")
    }

    /// Pre-scan connections to find output pins referenced more than once.
    fn scan_multi_refs(&mut self) {
        let mut ref_count: HashMap<(NodeId, String), usize> = HashMap::new();
        for conn in &self.graph.connections {
            *ref_count.entry((conn.from_node, conn.from_pin.clone())).or_default() += 1;
        }
        for (key, count) in ref_count {
            if count > 1 {
                self.multi_ref.insert(key);
            }
        }
    }

    /// Follow an exec chain from a node's output exec pin.
    fn compile_exec_chain(&mut self, from_node: NodeId, from_pin: &str) {
        let targets: Vec<(NodeId, String)> = self.graph
            .connections_from(from_node, from_pin)
            .iter()
            .map(|c| (c.to_node, c.to_pin.clone()))
            .collect();

        for (target_node, _) in targets {
            self.compile_exec_node(target_node);
        }
    }

    /// Compile a single exec node and follow its exec outputs.
    fn compile_exec_node(&mut self, node_id: NodeId) {
        let node = match self.graph.get_node(node_id) {
            Some(n) => n.clone(),
            None => return,
        };

        match node.node_type.as_str() {
            // ── Flow control ────────────────────────────────────────
            "flow/branch" => {
                let cond = self.compile_data_expr(node_id, "condition");
                self.emit(&format!("if {} then", cond));
                self.indent += 1;
                self.compile_exec_chain(node_id, "true");
                self.indent -= 1;
                // Check if there's a false branch.
                if !self.graph.connections_from(node_id, "false").is_empty() {
                    self.emit("else");
                    self.indent += 1;
                    self.compile_exec_chain(node_id, "false");
                    self.indent -= 1;
                }
                self.emit("end");
            }
            "flow/sequence" => {
                // Sequence fires exec_0, exec_1, exec_2, ... in order.
                for i in 0..10 {
                    let pin = format!("exec_{}", i);
                    if !self.graph.connections_from(node_id, &pin).is_empty() {
                        self.compile_exec_chain(node_id, &pin);
                    }
                }
                // Also try "then" for simple sequences.
                self.compile_exec_chain(node_id, "then");
            }
            "flow/do_once" => {
                let guard = format!("_do_once_{}", node_id);
                self.emit(&format!("if not {} then", guard));
                self.indent += 1;
                self.emit(&format!("{} = true", guard));
                self.compile_exec_chain(node_id, "exec");
                self.indent -= 1;
                self.emit("end");
            }

            "flow/counter" => {
                let var = format!("_counter_{}", node_id);
                let step = self.compile_data_expr(node_id, "step");
                let min = self.compile_data_expr(node_id, "min");
                let max = self.compile_data_expr(node_id, "max");
                let do_loop = self.compile_data_expr(node_id, "loop");
                self.emit(&format!("{} = ({} or {}) + ({}) * delta", var, var, min, step));
                self.emit(&format!("if {} > {} then", var, max));
                self.indent += 1;
                self.emit(&format!("if {} then {} = {} + ({} - {}) else {} = {} end", do_loop, var, min, var, max, var, max));
                self.indent -= 1;
                self.emit("end");
                self.compile_exec_chain(node_id, "then");
            }

            // ── Variable ────────────────────────────────────────────
            "variable/set" => {
                let name = self.resolve_input_value(&node, "name");
                let value = self.compile_data_expr(node_id, "value");
                // Strip quotes from name if it's a literal string.
                let var_name = strip_quotes(&name);
                self.emit(&format!("{} = {}", sanitize_var(&var_name), value));
                self.compile_exec_chain(node_id, "then");
            }

            // ── Transform ───────────────────────────────────────────
            "transform/set_position" => {
                let pos = self.compile_data_expr(node_id, "position");
                self.emit(&format!("set_position(({}).x or {0}[1], ({0}).y or {0}[2], ({0}).z or {0}[3])", pos));
                self.compile_exec_chain(node_id, "then");
            }
            "transform/translate" => {
                let v = self.compile_data_expr(node_id, "offset");
                self.emit(&format!("translate(({}).x or {0}[1], ({0}).y or {0}[2], ({0}).z or {0}[3])", v));
                self.compile_exec_chain(node_id, "then");
            }
            "transform/set_rotation" => {
                let r = self.compile_data_expr(node_id, "rotation");
                self.emit(&format!("set_rotation(({}).x or {0}[1], ({0}).y or {0}[2], ({0}).z or {0}[3])", r));
                self.compile_exec_chain(node_id, "then");
            }
            "transform/rotate" => {
                let r = self.compile_data_expr(node_id, "rotation");
                self.emit(&format!("rotate(({}).x or {0}[1], ({0}).y or {0}[2], ({0}).z or {0}[3])", r));
                self.compile_exec_chain(node_id, "then");
            }
            "transform/look_at" => {
                let t = self.compile_data_expr(node_id, "target");
                self.emit(&format!("look_at(({}).x or {0}[1], ({0}).y or {0}[2], ({0}).z or {0}[3])", t));
                self.compile_exec_chain(node_id, "then");
            }
            "transform/set_scale" => {
                let s = self.compile_data_expr(node_id, "scale");
                self.emit(&format!("set_scale(({}).x or {0}[1], ({0}).y or {0}[2], ({0}).z or {0}[3])", s));
                self.compile_exec_chain(node_id, "then");
            }

            // ── Component reflection ────────────────────────────────
            "component/set_field" => {
                let entity_val = self.resolve_input_value(&node, "entity");
                let comp = self.resolve_input_value(&node, "component");
                let field = self.resolve_input_value(&node, "field");
                let value = self.compile_data_expr(node_id, "value");
                let entity_str = strip_quotes(&entity_val);
                let comp_str = strip_quotes(&comp);
                let field_str = strip_quotes(&field);
                let path = format!("{}.{}", comp_str, field_str);
                if entity_str.is_empty() {
                    self.emit(&format!("set(\"{}\", {})", path, value));
                } else {
                    self.emit(&format!("set_on(\"{}\", \"{}\", {})", entity_str, path, value));
                }
                self.compile_exec_chain(node_id, "then");
            }

            // ── Physics ─────────────────────────────────────────────
            "physics/apply_force" => {
                let f = self.compile_data_expr(node_id, "force");
                self.emit(&format!("apply_force(({}).x or {0}[1], ({0}).y or {0}[2], ({0}).z or {0}[3])", f));
                self.compile_exec_chain(node_id, "then");
            }
            "physics/apply_impulse" => {
                let f = self.compile_data_expr(node_id, "impulse");
                self.emit(&format!("apply_impulse(({}).x or {0}[1], ({0}).y or {0}[2], ({0}).z or {0}[3])", f));
                self.compile_exec_chain(node_id, "then");
            }
            "physics/set_velocity" => {
                let v = self.compile_data_expr(node_id, "velocity");
                self.emit(&format!("set_velocity(({}).x or {0}[1], ({0}).y or {0}[2], ({0}).z or {0}[3])", v));
                self.compile_exec_chain(node_id, "then");
            }

            // ── Audio ───────────────────────────────────────────────
            "audio/play_sound" => {
                let path = self.compile_data_expr(node_id, "path");
                let vol = self.compile_data_expr(node_id, "volume");
                self.emit(&format!("play_sound({}, {})", path, vol));
                self.compile_exec_chain(node_id, "then");
            }
            "audio/play_music" => {
                let path = self.compile_data_expr(node_id, "path");
                let vol = self.compile_data_expr(node_id, "volume");
                let fade = self.compile_data_expr(node_id, "fade_in");
                self.emit(&format!("play_music({}, {}, {})", path, vol, fade));
                self.compile_exec_chain(node_id, "then");
            }
            "audio/stop_music" => {
                let fade = self.compile_data_expr(node_id, "fade_out");
                self.emit(&format!("stop_music({})", fade));
                self.compile_exec_chain(node_id, "then");
            }

            // ── Entity ──────────────────────────────────────────────
            "entity/spawn" => {
                let name = self.compile_data_expr(node_id, "name");
                self.emit(&format!("spawn_entity({})", name));
                self.compile_exec_chain(node_id, "then");
            }
            "entity/despawn" | "entity/despawn_self" => {
                self.emit("despawn_self()");
                self.compile_exec_chain(node_id, "then");
            }

            // ── Rendering ───────────────────────────────────────────
            "rendering/set_visibility" => {
                let vis = self.compile_data_expr(node_id, "visible");
                self.emit(&format!("set_visibility({})", vis));
                self.compile_exec_chain(node_id, "then");
            }
            "rendering/set_material_color" => {
                let c = self.compile_data_expr(node_id, "color");
                self.emit(&format!("set_material_color(({}).x or {0}[1], ({0}).y or {0}[2], ({0}).z or {0}[3], ({0}).w or {0}[4] or 1.0)", c));
                self.compile_exec_chain(node_id, "then");
            }

            // ── UI ──────────────────────────────────────────────────
            "ui/set_visible" => {
                let name = self.compile_data_expr(node_id, "element");
                let vis = self.compile_data_expr(node_id, "visible");
                // If name is empty string, use self_entity_name.
                self.emit(&format!(
                    "do local _n = {}; if _n == \"\" then _n = self_entity_name end; ui_set_visible(_n, {}) end",
                    name, vis
                ));
                self.compile_exec_chain(node_id, "then");
            }
            "ui/show" => {
                let name = self.compile_data_expr(node_id, "path");
                self.emit(&format!("ui_show({})", name));
                self.compile_exec_chain(node_id, "then");
            }
            "ui/hide" => {
                let name = self.compile_data_expr(node_id, "path");
                self.emit(&format!("ui_hide({})", name));
                self.compile_exec_chain(node_id, "then");
            }
            "ui/toggle" => {
                let name = self.compile_data_expr(node_id, "path");
                self.emit(&format!("ui_toggle({})", name));
                self.compile_exec_chain(node_id, "then");
            }
            "ui/set_text" => {
                let name = self.compile_data_expr(node_id, "element");
                let text = self.compile_data_expr(node_id, "text");
                self.emit(&format!("ui_set_text({}, {})", name, text));
                self.compile_exec_chain(node_id, "then");
            }
            "ui/set_progress" => {
                let name = self.compile_data_expr(node_id, "element");
                let val = self.compile_data_expr(node_id, "value");
                self.emit(&format!("ui_set_progress({}, {})", name, val));
                self.compile_exec_chain(node_id, "then");
            }
            "ui/set_health" => {
                let name = self.compile_data_expr(node_id, "element");
                let cur = self.compile_data_expr(node_id, "current");
                let max = self.compile_data_expr(node_id, "max");
                self.emit(&format!("ui_set_health({}, {}, {})", name, cur, max));
                self.compile_exec_chain(node_id, "then");
            }
            "ui/set_slider" => {
                let name = self.compile_data_expr(node_id, "element");
                let val = self.compile_data_expr(node_id, "value");
                self.emit(&format!("ui_set_slider({}, {})", name, val));
                self.compile_exec_chain(node_id, "then");
            }
            "ui/set_checkbox" => {
                let name = self.compile_data_expr(node_id, "element");
                let val = self.compile_data_expr(node_id, "checked");
                self.emit(&format!("ui_set_checkbox({}, {})", name, val));
                self.compile_exec_chain(node_id, "then");
            }
            "ui/set_toggle" => {
                let name = self.compile_data_expr(node_id, "element");
                let val = self.compile_data_expr(node_id, "on");
                self.emit(&format!("ui_set_toggle({}, {})", name, val));
                self.compile_exec_chain(node_id, "then");
            }
            "ui/set_theme" => {
                let theme = self.compile_data_expr(node_id, "theme");
                self.emit(&format!("ui_set_theme({})", theme));
                self.compile_exec_chain(node_id, "then");
            }
            "ui/set_color" => {
                let name = self.compile_data_expr(node_id, "element");
                let r = self.compile_data_expr(node_id, "r");
                let g = self.compile_data_expr(node_id, "g");
                let b = self.compile_data_expr(node_id, "b");
                let a = self.compile_data_expr(node_id, "a");
                self.emit(&format!("ui_set_color({}, {}, {}, {}, {})", name, r, g, b, a));
                self.compile_exec_chain(node_id, "then");
            }

            // ── Animation ───────────────────────────────────────────
            "animation/play" => {
                let name = self.compile_data_expr(node_id, "clip");
                let looping = self.compile_data_expr(node_id, "looping");
                let speed = self.compile_data_expr(node_id, "speed");
                self.emit(&format!("play_animation({}, {}, {})", name, looping, speed));
                self.compile_exec_chain(node_id, "then");
            }
            "animation/stop" => {
                self.emit("stop_animation()");
                self.compile_exec_chain(node_id, "then");
            }
            "animation/pause" => {
                self.emit("pause_animation()");
                self.compile_exec_chain(node_id, "then");
            }
            "animation/resume" => {
                self.emit("resume_animation()");
                self.compile_exec_chain(node_id, "then");
            }
            "animation/set_speed" => {
                let speed = self.compile_data_expr(node_id, "speed");
                self.emit(&format!("set_animation_speed({})", speed));
                self.compile_exec_chain(node_id, "then");
            }
            "animation/crossfade" => {
                let name = self.compile_data_expr(node_id, "clip");
                let dur = self.compile_data_expr(node_id, "duration");
                let looping = self.compile_data_expr(node_id, "looping");
                self.emit(&format!("crossfade_animation({}, {}, {})", name, dur, looping));
                self.compile_exec_chain(node_id, "then");
            }

            // ── Scene ───────────────────────────────────────────────
            "scene/load" => {
                let path = self.compile_data_expr(node_id, "path");
                self.emit(&format!("load_scene({})", path));
                self.compile_exec_chain(node_id, "then");
            }

            // ── Debug ───────────────────────────────────────────────
            "debug/log" => {
                let msg = self.compile_data_expr(node_id, "message");
                self.emit(&format!("print_log(tostring({}))", msg));
                self.compile_exec_chain(node_id, "then");
            }
            "debug/draw_line" => {
                let s = self.compile_data_expr(node_id, "start");
                let e = self.compile_data_expr(node_id, "end");
                let d = self.compile_data_expr(node_id, "duration");
                self.emit(&format!(
                    "do local _s,_e={},{}; draw_line(_s.x or _s[1], _s.y or _s[2], _s.z or _s[3], _e.x or _e[1], _e.y or _e[2], _e.z or _e[3], {}) end",
                    s, e, d
                ));
                self.compile_exec_chain(node_id, "then");
            }

            // ── Timer ───────────────────────────────────────────────
            "flow/start_timer" => {
                let name = self.compile_data_expr(node_id, "name");
                let dur = self.compile_data_expr(node_id, "duration");
                let rep = self.compile_data_expr(node_id, "repeat");
                self.emit(&format!("start_timer({}, {}, {})", name, dur, rep));
                self.compile_exec_chain(node_id, "then");
            }

            _ => {
                // Unknown exec node — emit comment and continue.
                self.emit(&format!("-- TODO: unhandled node type '{}'", node.node_type));
                self.compile_exec_chain(node_id, "then");
            }
        }
    }

    /// Compile a data expression for a node's input pin.
    /// Returns a Lua expression string.
    fn compile_data_expr(&mut self, node_id: NodeId, pin_name: &str) -> String {
        // Check if there's a connection feeding this input.
        let conn = self.graph.connection_to(node_id, pin_name).cloned();

        if let Some(conn) = conn {
            // Get the source node's output expression.
            self.compile_output_expr(conn.from_node, &conn.from_pin)
        } else {
            // Use inline value or default.
            let node = match self.graph.get_node(node_id) {
                Some(n) => n,
                None => return "nil".to_string(),
            };
            if let Some(val) = node.get_input_value(pin_name) {
                pin_value_to_lua(val)
            } else {
                // Fall back to node def default.
                if let Some(def) = crate::node_def(&node.node_type) {
                    let pins = (def.pins)();
                    if let Some(pin) = pins.iter().find(|p| p.name == pin_name) {
                        pin_value_to_lua(&pin.default_value)
                    } else {
                        "nil".to_string()
                    }
                } else {
                    "nil".to_string()
                }
            }
        }
    }

    /// Compile the output expression for a data node.
    /// Caches results for nodes referenced multiple times.
    fn compile_output_expr(&mut self, node_id: NodeId, pin_name: &str) -> String {
        let key = (node_id, pin_name.to_string());

        // Return cached expression if already computed.
        if let Some(cached) = self.expr_cache.get(&key) {
            return cached.clone();
        }

        let node = match self.graph.get_node(node_id) {
            Some(n) => n.clone(),
            None => return "nil".to_string(),
        };

        let expr = self.compile_data_node(&node, pin_name);

        // If this output is referenced multiple times, store in a local variable.
        if self.multi_ref.contains(&key) {
            let var = self.fresh_var();
            self.emit(&format!("local {} = {}", var, expr));
            self.expr_cache.insert(key, var.clone());
            var
        } else {
            self.expr_cache.insert(key.clone(), expr.clone());
            expr
        }
    }

    /// Compile a data-only node into a Lua expression.
    fn compile_data_node(&mut self, node: &BlueprintNode, pin_name: &str) -> String {
        match node.node_type.as_str() {
            // ── Event outputs ───────────────────────────────────────
            "event/on_update" => match pin_name {
                "delta" => "delta".to_string(),
                "elapsed" => "elapsed".to_string(),
                _ => "nil".to_string(),
            },

            // ── Math ────────────────────────────────────────────────
            "math/add" => {
                let a = self.compile_data_expr(node.id, "a");
                let b = self.compile_data_expr(node.id, "b");
                format!("({} + {})", a, b)
            }
            "math/subtract" => {
                let a = self.compile_data_expr(node.id, "a");
                let b = self.compile_data_expr(node.id, "b");
                format!("({} - {})", a, b)
            }
            "math/multiply" => {
                let a = self.compile_data_expr(node.id, "a");
                let b = self.compile_data_expr(node.id, "b");
                format!("({} * {})", a, b)
            }
            "math/divide" => {
                let a = self.compile_data_expr(node.id, "a");
                let b = self.compile_data_expr(node.id, "b");
                format!("({} / {})", a, b)
            }
            "math/negate" => {
                let v = self.compile_data_expr(node.id, "value");
                format!("(-{})", v)
            }
            "math/abs" => {
                let v = self.compile_data_expr(node.id, "value");
                format!("math.abs({})", v)
            }
            "math/clamp" => {
                let v = self.compile_data_expr(node.id, "value");
                let lo = self.compile_data_expr(node.id, "min");
                let hi = self.compile_data_expr(node.id, "max");
                format!("clamp({}, {}, {})", v, lo, hi)
            }
            "math/lerp" => {
                let a = self.compile_data_expr(node.id, "a");
                let b = self.compile_data_expr(node.id, "b");
                let t = self.compile_data_expr(node.id, "t");
                format!("lerp({}, {}, {})", a, b, t)
            }
            "math/random_range" => {
                let lo = self.compile_data_expr(node.id, "min");
                let hi = self.compile_data_expr(node.id, "max");
                format!("({} + math.random() * ({} - {}))", lo, hi, lo)
            }
            "math/sin" => {
                let v = self.compile_data_expr(node.id, "value");
                format!("math.sin({})", v)
            }
            "math/cos" => {
                let v = self.compile_data_expr(node.id, "value");
                format!("math.cos({})", v)
            }
            "math/min" => {
                let a = self.compile_data_expr(node.id, "a");
                let b = self.compile_data_expr(node.id, "b");
                format!("math.min({}, {})", a, b)
            }
            "math/max" => {
                let a = self.compile_data_expr(node.id, "a");
                let b = self.compile_data_expr(node.id, "b");
                format!("math.max({}, {})", a, b)
            }
            "math/floor" => {
                let v = self.compile_data_expr(node.id, "value");
                format!("math.floor({})", v)
            }
            "math/ceil" => {
                let v = self.compile_data_expr(node.id, "value");
                format!("math.ceil({})", v)
            }
            "math/round" => {
                let v = self.compile_data_expr(node.id, "value");
                format!("math.floor({} + 0.5)", v)
            }
            "math/modulo" => {
                let a = self.compile_data_expr(node.id, "a");
                let b = self.compile_data_expr(node.id, "b");
                format!("({} % {})", a, b)
            }
            "math/compare" => {
                let a = self.compile_data_expr(node.id, "a");
                let b = self.compile_data_expr(node.id, "b");
                match pin_name {
                    "greater" => format!("({} > {})", a, b),
                    "less" => format!("({} < {})", a, b),
                    "equal" => format!("({} == {})", a, b),
                    _ => "false".to_string(),
                }
            }
            "math/and" => {
                let a = self.compile_data_expr(node.id, "a");
                let b = self.compile_data_expr(node.id, "b");
                format!("({} and {})", a, b)
            }
            "math/or" => {
                let a = self.compile_data_expr(node.id, "a");
                let b = self.compile_data_expr(node.id, "b");
                format!("({} or {})", a, b)
            }
            "math/not" => {
                let v = self.compile_data_expr(node.id, "value");
                format!("(not {})", v)
            }
            "math/combine_vec3" => {
                let x = self.compile_data_expr(node.id, "x");
                let y = self.compile_data_expr(node.id, "y");
                let z = self.compile_data_expr(node.id, "z");
                format!("vec3({}, {}, {})", x, y, z)
            }
            "math/split_vec3" => {
                let v = self.compile_data_expr(node.id, "vector");
                match pin_name {
                    "x" => format!("({}).x or ({})[1]", v, v),
                    "y" => format!("({}).y or ({})[2]", v, v),
                    "z" => format!("({}).z or ({})[3]", v, v),
                    _ => "0".to_string(),
                }
            }
            "math/distance" => {
                let a = self.compile_data_expr(node.id, "a");
                let b = self.compile_data_expr(node.id, "b");
                // Use a helper since Lua doesn't have a built-in.
                format!("(function() local _a,_b={},{}; local _d={{(_a.x or _a[1])-(_b.x or _b[1]),(_a.y or _a[2])-(_b.y or _b[2]),(_a.z or _a[3])-(_b.z or _b[3])}}; return math.sqrt(_d[1]*_d[1]+_d[2]*_d[2]+_d[3]*_d[3]) end)()", a, b)
            }
            "math/dot" => {
                let a = self.compile_data_expr(node.id, "a");
                let b = self.compile_data_expr(node.id, "b");
                format!("(function() local _a,_b={},{}; return (_a.x or _a[1])*(_b.x or _b[1])+(_a.y or _a[2])*(_b.y or _b[2])+(_a.z or _a[3])*(_b.z or _b[3]) end)()", a, b)
            }
            "math/normalize" => {
                let v = self.compile_data_expr(node.id, "value");
                format!("(function() local _v={}; local _x,_y,_z=_v.x or _v[1],_v.y or _v[2],_v.z or _v[3]; local _l=math.sqrt(_x*_x+_y*_y+_z*_z); if _l>0 then return vec3(_x/_l,_y/_l,_z/_l) else return vec3(0,0,0) end end)()", v)
            }

            // ── String ──────────────────────────────────────────────
            "string/concat" => {
                let a = self.compile_data_expr(node.id, "a");
                let b = self.compile_data_expr(node.id, "b");
                format!("(tostring({}) .. tostring({}))", a, b)
            }
            "string/format" => {
                let template = self.compile_data_expr(node.id, "template");
                let value = self.compile_data_expr(node.id, "value");
                format!("string.gsub({}, \"{{0}}\", tostring({}))", template, value)
            }
            "string/to_float" => {
                let v = self.compile_data_expr(node.id, "value");
                format!("(tonumber({}) or 0)", v)
            }
            "string/to_int" => {
                let v = self.compile_data_expr(node.id, "value");
                format!("math.floor(tonumber({}) or 0)", v)
            }

            // ── Conversion ──────────────────────────────────────────
            "convert/to_string" => {
                let v = self.compile_data_expr(node.id, "value");
                format!("tostring({})", v)
            }
            "convert/to_float" => {
                let v = self.compile_data_expr(node.id, "value");
                format!("(tonumber({}) or 0)", v)
            }
            "convert/to_int" => {
                let v = self.compile_data_expr(node.id, "value");
                format!("math.floor(tonumber({}) or 0)", v)
            }
            "convert/to_bool" => {
                let v = self.compile_data_expr(node.id, "value");
                format!("(not not {})", v)
            }

            // ── Transform reads ─────────────────────────────────────
            "transform/get_position" => match pin_name {
                "position" => "vec3(position_x, position_y, position_z)".to_string(),
                "x" => "position_x".to_string(),
                "y" => "position_y".to_string(),
                "z" => "position_z".to_string(),
                _ => "nil".to_string(),
            },
            "transform/get_rotation" => match pin_name {
                "rotation" => "vec3(rotation_x, rotation_y, rotation_z)".to_string(),
                "x" => "rotation_x".to_string(),
                "y" => "rotation_y".to_string(),
                "z" => "rotation_z".to_string(),
                _ => "nil".to_string(),
            },
            "transform/get_forward" => match pin_name {
                "forward" => {
                    // Forward vector from rotation (simplified: yaw only)
                    "(function() local _r=math.rad(rotation_y); return vec3(math.sin(_r),0,-math.cos(_r)) end)()".to_string()
                }
                _ => "nil".to_string(),
            },

            // ── Input ───────────────────────────────────────────────
            "input/get_movement" => match pin_name {
                "x" => "input_x".to_string(),
                "y" => "input_y".to_string(),
                _ => "0".to_string(),
            },
            "input/is_key_pressed" => {
                let key = self.resolve_input_value(&node, "key");
                let key_str = strip_quotes(&key);
                format!("is_key_pressed(\"{}\")", key_str)
            }
            "input/is_key_just_pressed" => {
                let key = self.resolve_input_value(&node, "key");
                let key_str = strip_quotes(&key);
                format!("is_key_just_pressed(\"{}\")", key_str)
            }
            "input/get_mouse_position" => match pin_name {
                "x" => "mouse_x".to_string(),
                "y" => "mouse_y".to_string(),
                _ => "0".to_string(),
            },
            "input/is_mouse_pressed" => {
                let btn = self.resolve_input_value(&node, "button");
                let btn_str = strip_quotes(&btn);
                match btn_str.as_str() {
                    "Left" => "mouse_left".to_string(),
                    "Right" => "mouse_right".to_string(),
                    "Middle" => "mouse_middle".to_string(),
                    _ => "false".to_string(),
                }
            }

            // ── Entity ──────────────────────────────────────────────
            "entity/get_self" => match pin_name {
                "entity" => "self_entity_name".to_string(),
                _ => "nil".to_string(),
            },
            "entity/get_entity" => {
                let name = self.resolve_input_value(&node, "name");
                name
            }

            // ── Component reflection read ───────────────────────────
            "component/get_field" => {
                let entity_val = self.resolve_input_value(&node, "entity");
                let comp = self.resolve_input_value(&node, "component");
                let field = self.resolve_input_value(&node, "field");
                let entity_str = strip_quotes(&entity_val);
                let comp_str = strip_quotes(&comp);
                let field_str = strip_quotes(&field);
                let path = format!("{}.{}", comp_str, field_str);
                if entity_str.is_empty() {
                    format!("get(\"{}\")", path)
                } else {
                    format!("get_on(\"{}\", \"{}\")", entity_str, path)
                }
            }

            // ── Variable read ───────────────────────────────────────
            "variable/get" => {
                let name = self.resolve_input_value(&node, "name");
                let var_name = strip_quotes(&name);
                sanitize_var(&var_name)
            }

            // ── Counter read ───────────────────────────────────────
            "flow/counter" => {
                format!("(_counter_{} or 0)", node.id)
            }

            _ => {
                format!("nil --[[ TODO: {} ]]", node.node_type)
            }
        }
    }

    /// Get a node's inline input value as a Lua expression string.
    fn resolve_input_value(&self, node: &BlueprintNode, pin_name: &str) -> String {
        if let Some(val) = node.get_input_value(pin_name) {
            pin_value_to_lua(val)
        } else if let Some(def) = crate::node_def(&node.node_type) {
            let pins = (def.pins)();
            if let Some(pin) = pins.iter().find(|p| p.name == pin_name) {
                pin_value_to_lua(&pin.default_value)
            } else {
                "nil".to_string()
            }
        } else {
            "nil".to_string()
        }
    }

    fn fresh_var(&mut self) -> String {
        let v = format!("_t{}", self.temp_counter);
        self.temp_counter += 1;
        v
    }

    fn emit(&mut self, line: &str) {
        let prefix = "  ".repeat(self.indent);
        self.lines.push(format!("{}{}", prefix, line));
    }
}

/// Convert a PinValue to a Lua literal.
fn pin_value_to_lua(val: &PinValue) -> String {
    match val {
        PinValue::None => "nil".to_string(),
        PinValue::Float(v) => format!("{}", v),
        PinValue::Int(v) => format!("{}", v),
        PinValue::Bool(v) => format!("{}", v),
        PinValue::String(v) => format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\"")),
        PinValue::Vec2([x, y]) => format!("vec2({}, {})", x, y),
        PinValue::Vec3([x, y, z]) => format!("vec3({}, {}, {})", x, y, z),
        PinValue::Color([r, g, b, a]) => format!("{{{}, {}, {}, {}}}", r, g, b, a),
        PinValue::Entity(name) => {
            if name.is_empty() {
                "\"\"".to_string()
            } else {
                format!("\"{}\"", name.replace('"', "\\\""))
            }
        }
    }
}

/// Sanitize a variable name for Lua (replace spaces, ensure not a keyword).
fn sanitize_var(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' }).collect();
    if s.is_empty() {
        "_var".to_string()
    } else if s.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{}", s)
    } else {
        s
    }
}

/// Strip surrounding quotes from a string literal.
fn strip_quotes(s: &str) -> String {
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        s[1..s.len()-1].to_string()
    } else {
        s.to_string()
    }
}
