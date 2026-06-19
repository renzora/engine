//! Blueprint → Lua compiler.
//!
//! Walks the graph and emits Lua, which runs through the normal script VM (a
//! blueprint dropped on a Script component compiles here on load). The compiler
//! owns only the *machinery* — exec/data walking, multi-ref locals, var/counter
//! declarations. Each node's actual emission lives next to its definition in
//! `crate::nodes` and is dispatched through the registry, so a node is one
//! self-contained, testable unit. This is the single execution path; there is no
//! live graph interpreter.

use crate::graph::{BlueprintGraph, NodeId, PinValue};
use std::collections::{HashMap, HashSet};

/// Compile a blueprint graph to Lua source code.
pub fn compile_to_lua(graph: &BlueprintGraph) -> String {
    let mut compiler = Compiler::new(graph);
    compiler.compile()
}

/// Lua emitter + graph-walk state. Node emit fns (in `crate::nodes`) receive
/// `&mut Compiler` and build their output through its `pub(crate)` API.
pub(crate) struct Compiler<'a> {
    graph: &'a BlueprintGraph,
    temp_counter: u32,
    expr_cache: HashMap<(NodeId, String), String>,
    multi_ref: HashSet<(NodeId, String)>,
    lines: Vec<String>,
    indent: usize,
    variables_used: HashSet<String>,
}

impl<'a> Compiler<'a> {
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
        self.scan_multi_refs();

        let event_nodes: Vec<(NodeId, String)> = self
            .graph
            .event_nodes()
            .iter()
            .map(|n| (n.id, n.node_type.clone()))
            .collect();

        let mut sections: Vec<String> = vec!["-- Generated from Blueprint graph".to_string()];

        // Declare blueprint variables (from variable/get|set "name" inputs).
        for node in &self.graph.nodes {
            if node.node_type == "variable/set" || node.node_type == "variable/get" {
                if let Some(PinValue::String(name)) = node.get_input_value("name") {
                    if !name.is_empty() {
                        self.variables_used.insert(name.clone());
                    }
                }
            }
        }
        if !self.variables_used.is_empty() {
            let mut vars: Vec<&String> = self.variables_used.iter().collect();
            vars.sort();
            for var in &vars {
                sections.push(format!("local {} = 0", sanitize_var(var)));
            }
            sections.push(String::new());
        }

        // Compile each recognised event entry into its lifecycle function.
        for (node_id, node_type) in &event_nodes {
            let fn_name = match node_type.as_str() {
                "event/on_ready" => "on_ready",
                "event/on_update" => "on_update",
                _ => continue,
            };
            self.lines.clear();
            self.expr_cache.clear();
            self.temp_counter = 0;
            self.indent = 1;
            self.compile_exec_chain(*node_id, "exec");
            let body = self.lines.join("\n");
            sections.push(format!("function {fn_name}()\n{body}\nend\n"));
        }

        sections.join("\n")
    }

    fn scan_multi_refs(&mut self) {
        let mut ref_count: HashMap<(NodeId, String), usize> = HashMap::new();
        for conn in &self.graph.connections {
            *ref_count
                .entry((conn.from_node, conn.from_pin.clone()))
                .or_default() += 1;
        }
        for (key, count) in ref_count {
            if count > 1 {
                self.multi_ref.insert(key);
            }
        }
    }

    /// Follow an exec output pin, compiling each connected node.
    fn compile_exec_chain(&mut self, from_node: NodeId, from_pin: &str) {
        let targets: Vec<NodeId> = self
            .graph
            .connections_from(from_node, from_pin)
            .iter()
            .map(|c| c.to_node)
            .collect();
        for target in targets {
            self.compile_exec_node(target);
        }
    }

    /// Dispatch an exec node to its registry emit fn.
    fn compile_exec_node(&mut self, node_id: NodeId) {
        let Some(node_type) = self.graph.get_node(node_id).map(|n| n.node_type.clone()) else {
            return;
        };
        match crate::nodes::entry(&node_type) {
            Some(entry) => match entry.exec {
                Some(f) => f(self, node_id),
                // A pure node wired into an exec chain: nothing to emit, continue.
                None => self.compile_exec_chain(node_id, "then"),
            },
            None => {
                self.emit(&format!("-- TODO: unhandled node '{node_type}'"));
                self.compile_exec_chain(node_id, "then");
            }
        }
    }

    /// Resolve an input pin to a Lua expression: a connected output, else the
    /// node's inline value, else the pin default.
    fn compile_data_expr(&mut self, node_id: NodeId, pin_name: &str) -> String {
        if let Some(conn) = self.graph.connection_to(node_id, pin_name).cloned() {
            return self.compile_output_expr(conn.from_node, &conn.from_pin);
        }
        let Some(node) = self.graph.get_node(node_id) else {
            return "nil".to_string();
        };
        if let Some(val) = node.get_input_value(pin_name) {
            pin_value_to_lua(val)
        } else {
            self.default_for(&node.node_type, pin_name)
        }
    }

    /// Compile a data node's output, caching (and local-binding multi-ref outputs).
    fn compile_output_expr(&mut self, node_id: NodeId, pin_name: &str) -> String {
        let key = (node_id, pin_name.to_string());
        if let Some(cached) = self.expr_cache.get(&key) {
            return cached.clone();
        }
        let Some(node) = self.graph.get_node(node_id).cloned() else {
            return "nil".to_string();
        };
        let expr = match crate::nodes::entry(&node.node_type) {
            Some(entry) => (entry.data)(self, node.id, pin_name),
            None => format!("nil --[[ TODO: {} ]]", node.node_type),
        };
        if self.multi_ref.contains(&key) {
            let var = self.fresh_var();
            self.emit(&format!("local {var} = {expr}"));
            self.expr_cache.insert(key, var.clone());
            var
        } else {
            self.expr_cache.insert(key, expr.clone());
            expr
        }
    }

    fn default_for(&self, node_type: &str, pin_name: &str) -> String {
        if let Some(def) = crate::node_def(node_type) {
            if let Some(pin) = (def.pins)().iter().find(|p| p.name == pin_name) {
                return pin_value_to_lua(&pin.default_value);
            }
        }
        "nil".to_string()
    }

    fn fresh_var(&mut self) -> String {
        let v = format!("_t{}", self.temp_counter);
        self.temp_counter += 1;
        v
    }

    // ── pub(crate) API for node emit fns ─────────────────────────────────────

    /// Lua expression feeding input `pin` of `node`.
    pub(crate) fn data(&mut self, node: NodeId, pin: &str) -> String {
        self.compile_data_expr(node, pin)
    }

    /// Compile the chain hanging off exec output `pin` of `node`.
    pub(crate) fn exec(&mut self, node: NodeId, pin: &str) {
        self.compile_exec_chain(node, pin);
    }

    /// Inline/default value of `pin` as a Lua literal (no connection following).
    pub(crate) fn inline(&self, node: NodeId, pin: &str) -> String {
        match self.graph.get_node(node) {
            Some(n) => match n.get_input_value(pin) {
                Some(v) => pin_value_to_lua(v),
                None => self.default_for(&n.node_type, pin),
            },
            None => "nil".to_string(),
        }
    }

    /// Emit one Lua statement at the current indent.
    pub(crate) fn emit(&mut self, line: &str) {
        let prefix = "  ".repeat(self.indent);
        self.lines.push(format!("{prefix}{line}"));
    }

    pub(crate) fn indent_inc(&mut self) {
        self.indent += 1;
    }

    pub(crate) fn indent_dec(&mut self) {
        self.indent = self.indent.saturating_sub(1);
    }

    /// Does exec output `pin` of `node` connect to anything?
    pub(crate) fn has_exec(&self, node: NodeId, pin: &str) -> bool {
        !self.graph.connections_from(node, pin).is_empty()
    }
}

/// Convert a PinValue to a Lua literal.
pub(crate) fn pin_value_to_lua(val: &PinValue) -> String {
    match val {
        PinValue::None => "nil".to_string(),
        PinValue::Float(v) => format!("{v}"),
        PinValue::Int(v) => format!("{v}"),
        PinValue::Bool(v) => format!("{v}"),
        PinValue::String(v) => format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\"")),
        PinValue::Vec2([x, y]) => format!("vec2({x}, {y})"),
        PinValue::Vec3([x, y, z]) => format!("vec3({x}, {y}, {z})"),
        PinValue::Color([r, g, b, a]) => format!("{{{r}, {g}, {b}, {a}}}"),
        PinValue::Entity(name) => {
            if name.is_empty() {
                "\"\"".to_string()
            } else {
                format!("\"{}\"", name.replace('"', "\\\""))
            }
        }
    }
}

/// Sanitize a name into a safe Lua identifier.
pub(crate) fn sanitize_var(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    if s.is_empty() {
        "_var".to_string()
    } else if s.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{s}")
    } else {
        s
    }
}

/// Strip surrounding quotes from a Lua string literal.
pub(crate) fn strip_quotes(s: &str) -> String {
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::compile_to_lua;
    use crate::graph::{BlueprintGraph, NodeId, PinValue};

    fn set(g: &mut BlueprintGraph, id: NodeId, pin: &str, v: PinValue) {
        g.get_node_mut(id).unwrap().input_values.insert(pin.to_string(), v);
    }

    #[test]
    fn on_update_rotate_compiles_degrees() {
        let mut g = BlueprintGraph::new();
        let ev = g.add_node("event/on_update", [0.0, 0.0]);
        let rot = g.add_node("transform/rotate", [200.0, 0.0]);
        set(&mut g, rot, "degrees", PinValue::Vec3([0.0, 90.0, 0.0]));
        g.connect(ev, "exec", rot, "exec");
        let lua = compile_to_lua(&g);
        assert!(lua.contains("function on_update()"), "missing on_update:\n{lua}");
        assert!(lua.contains("rotate("), "missing rotate:\n{lua}");
        assert!(lua.contains("90"), "degrees dropped:\n{lua}");
        // Rotate is rate-based: it must scale by delta on its own.
        assert!(lua.contains("delta"), "rotate not delta-scaled:\n{lua}");
    }

    /// A spin is exactly two nodes: on_update -> rotate (rate on the pin).
    #[test]
    fn spin_is_two_nodes() {
        let mut g = BlueprintGraph::new();
        let ev = g.add_node("event/on_update", [0.0, 0.0]);
        let rot = g.add_node("transform/rotate", [200.0, 0.0]);
        set(&mut g, rot, "degrees", PinValue::Vec3([0.0, 90.0, 0.0]));
        g.connect(ev, "exec", rot, "exec");
        let lua = compile_to_lua(&g);
        assert!(lua.contains("rotate(") && lua.contains("delta") && lua.contains("90"), "{lua}");
    }

    #[test]
    fn on_ready_variable_set() {
        let mut g = BlueprintGraph::new();
        let ev = g.add_node("event/on_ready", [0.0, 0.0]);
        let setv = g.add_node("variable/set", [200.0, 0.0]);
        set(&mut g, setv, "name", PinValue::String("x".into()));
        set(&mut g, setv, "value", PinValue::Float(5.0));
        g.connect(ev, "exec", setv, "exec");
        let lua = compile_to_lua(&g);
        assert!(lua.contains("function on_ready()"), "missing on_ready:\n{lua}");
        assert!(lua.contains("x = 5"), "var assignment wrong:\n{lua}");
    }

    #[test]
    fn branch_compiles_to_if() {
        let mut g = BlueprintGraph::new();
        let ev = g.add_node("event/on_update", [0.0, 0.0]);
        let br = g.add_node("flow/branch", [200.0, 0.0]);
        set(&mut g, br, "condition", PinValue::Bool(true));
        let log = g.add_node("debug/log", [400.0, 0.0]);
        set(&mut g, log, "message", PinValue::String("hi".into()));
        g.connect(ev, "exec", br, "exec");
        g.connect(br, "true", log, "exec");
        let lua = compile_to_lua(&g);
        assert!(lua.contains("if "), "missing if:\n{lua}");
        assert!(lua.contains("hi"), "missing log:\n{lua}");
    }

    #[test]
    fn crossfade_uses_name_pin() {
        let mut g = BlueprintGraph::new();
        let ev = g.add_node("event/on_update", [0.0, 0.0]);
        let xf = g.add_node("animation/crossfade", [200.0, 0.0]);
        set(&mut g, xf, "name", PinValue::String("Run".into()));
        g.connect(ev, "exec", xf, "exec");
        let lua = compile_to_lua(&g);
        assert!(lua.contains("crossfade_animation("), "missing crossfade:\n{lua}");
        assert!(lua.contains("Run"), "clip name dropped:\n{lua}");
    }

    #[test]
    fn data_flow_delta_into_variable() {
        let mut g = BlueprintGraph::new();
        let ev = g.add_node("event/on_update", [0.0, 0.0]);
        let mul = g.add_node("math/multiply", [200.0, 0.0]);
        set(&mut g, mul, "b", PinValue::Float(90.0));
        let setv = g.add_node("variable/set", [400.0, 0.0]);
        set(&mut g, setv, "name", PinValue::String("spin".into()));
        g.connect(ev, "delta", mul, "a");
        g.connect(mul, "result", setv, "value");
        g.connect(ev, "exec", setv, "exec");
        let lua = compile_to_lua(&g);
        assert!(lua.contains("delta"), "delta not wired:\n{lua}");
        assert!(lua.contains("90"), "operand dropped:\n{lua}");
        assert!(lua.contains("spin ="), "var not assigned:\n{lua}");
    }
}
