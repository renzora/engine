//! Rhai code generation from blueprint graphs
//!
//! Compiles a BlueprintGraph into Rhai script code that can be executed
//! by the scripting engine.

use std::collections::{HashMap, HashSet};
use super::{BlueprintGraph, BlueprintNode, NodeId, PinId, PinValue};

/// Result of code generation
pub struct CodegenResult {
    /// Generated Rhai code
    pub code: String,
    /// Errors encountered during generation
    pub errors: Vec<String>,
    /// Warnings
    pub warnings: Vec<String>,
}

/// Context for code generation
struct CodegenContext<'a> {
    graph: &'a BlueprintGraph,
    /// Generated variable names for node outputs
    output_vars: HashMap<PinId, String>,
    /// Counter for generating unique variable names
    var_counter: usize,
    /// Set of nodes that have been processed
    processed_nodes: HashSet<NodeId>,
    /// Indentation level
    indent: usize,
}

impl<'a> CodegenContext<'a> {
    fn new(graph: &'a BlueprintGraph) -> Self {
        Self {
            graph,
            output_vars: HashMap::new(),
            var_counter: 0,
            processed_nodes: HashSet::new(),
            indent: 1, // Start at 1 for function body
        }
    }

    fn next_var(&mut self, prefix: &str) -> String {
        let name = format!("{}_{}", prefix, self.var_counter);
        self.var_counter += 1;
        name
    }

    fn indent_str(&self) -> String {
        "    ".repeat(self.indent)
    }

    /// Get the value expression for an input pin
    fn get_input_value(&mut self, node: &BlueprintNode, pin_name: &str) -> String {
        // Check if there's a connection to this pin
        let pin_id = PinId::new(node.id, pin_name);
        if let Some(conn) = self.graph.connection_to(&pin_id) {
            // Get the variable from the connected output
            if let Some(var_name) = self.output_vars.get(&conn.from) {
                return var_name.clone();
            }

            // Need to generate the source node first
            if let Some(source_node) = self.graph.get_node(conn.from.node_id) {
                self.generate_data_node(source_node);
                if let Some(var_name) = self.output_vars.get(&conn.from) {
                    return var_name.clone();
                }
            }
        }

        // Use the input value override or default
        if let Some(value) = node.get_input_value(pin_name) {
            return value.to_rhai();
        }

        // Fallback
        "0.0".to_string()
    }

    /// Generate code for a data node (pure function, no flow pins)
    fn generate_data_node(&mut self, node: &BlueprintNode) -> Vec<String> {
        if self.processed_nodes.contains(&node.id) {
            return Vec::new();
        }
        self.processed_nodes.insert(node.id);

        let mut lines = Vec::new();
        let indent = self.indent_str();

        match node.node_type.as_str() {
            // Math nodes
            "math/add" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let result_var = self.next_var("add");
                lines.push(format!("{}let {} = {} + {};", indent, result_var, a, b));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/subtract" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let result_var = self.next_var("sub");
                lines.push(format!("{}let {} = {} - {};", indent, result_var, a, b));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/multiply" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let result_var = self.next_var("mul");
                lines.push(format!("{}let {} = {} * {};", indent, result_var, a, b));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/divide" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let result_var = self.next_var("div");
                lines.push(format!("{}let {} = {} / {};", indent, result_var, a, b));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/lerp" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("lerp");
                lines.push(format!("{}let {} = {} + ({} - {}) * {};", indent, result_var, a, b, a, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/clamp" => {
                let value = self.get_input_value(node, "value");
                let min = self.get_input_value(node, "min");
                let max = self.get_input_value(node, "max");
                let result_var = self.next_var("clamp");
                lines.push(format!("{}let {} = clamp({}, {}, {});", indent, result_var, value, min, max));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/abs" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("abs");
                lines.push(format!("{}let {} = abs({});", indent, result_var, value));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/min" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let result_var = self.next_var("min");
                lines.push(format!("{}let {} = min({}, {});", indent, result_var, a, b));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/max" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let result_var = self.next_var("max");
                lines.push(format!("{}let {} = max({}, {});", indent, result_var, a, b));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/sin" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("sin");
                lines.push(format!("{}let {} = sin({});", indent, result_var, value));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/cos" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("cos");
                lines.push(format!("{}let {} = cos({});", indent, result_var, value));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }

            // Logic nodes
            "logic/compare" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let mode = node.input_values.get("mode")
                    .and_then(|v| if let PinValue::String(s) = v { Some(s.as_str()) } else { None })
                    .unwrap_or("==");
                let result_var = self.next_var("cmp");
                lines.push(format!("{}let {} = {} {} {};", indent, result_var, a, mode, b));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "logic/and" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let result_var = self.next_var("and");
                lines.push(format!("{}let {} = {} && {};", indent, result_var, a, b));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "logic/or" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let result_var = self.next_var("or");
                lines.push(format!("{}let {} = {} || {};", indent, result_var, a, b));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "logic/not" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("not");
                lines.push(format!("{}let {} = !{};", indent, result_var, value));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }

            // Input nodes
            "input/get_axis" => {
                let x_var = self.next_var("input_x");
                let y_var = self.next_var("input_y");
                lines.push(format!("{}let {} = input_x;", indent, x_var));
                lines.push(format!("{}let {} = input_y;", indent, y_var));
                self.output_vars.insert(PinId::new(node.id, "x"), x_var);
                self.output_vars.insert(PinId::new(node.id, "y"), y_var);
            }
            "input/is_key_pressed" => {
                let key_name = node.input_values.get("key")
                    .and_then(|v| if let PinValue::String(s) = v { Some(s.as_str()) } else { None })
                    .unwrap_or("Space");
                let result_var = self.next_var("key");
                lines.push(format!("{}let {} = is_key_pressed(\"{}\");", indent, result_var, key_name));
                self.output_vars.insert(PinId::new(node.id, "pressed"), result_var);
            }
            "input/get_mouse_position" => {
                let x_var = self.next_var("mouse_x");
                let y_var = self.next_var("mouse_y");
                lines.push(format!("{}let {} = mouse_x;", indent, x_var));
                lines.push(format!("{}let {} = mouse_y;", indent, y_var));
                self.output_vars.insert(PinId::new(node.id, "x"), x_var);
                self.output_vars.insert(PinId::new(node.id, "y"), y_var);
            }
            "input/get_mouse_delta" => {
                let x_var = self.next_var("mouse_dx");
                let y_var = self.next_var("mouse_dy");
                lines.push(format!("{}let {} = mouse_delta_x;", indent, x_var));
                lines.push(format!("{}let {} = mouse_delta_y;", indent, y_var));
                self.output_vars.insert(PinId::new(node.id, "x"), x_var);
                self.output_vars.insert(PinId::new(node.id, "y"), y_var);
            }

            // Transform getters
            "transform/get_position" => {
                let x_var = self.next_var("pos_x");
                let y_var = self.next_var("pos_y");
                let z_var = self.next_var("pos_z");
                lines.push(format!("{}let {} = position_x;", indent, x_var));
                lines.push(format!("{}let {} = position_y;", indent, y_var));
                lines.push(format!("{}let {} = position_z;", indent, z_var));
                self.output_vars.insert(PinId::new(node.id, "x"), x_var);
                self.output_vars.insert(PinId::new(node.id, "y"), y_var);
                self.output_vars.insert(PinId::new(node.id, "z"), z_var);
            }
            "transform/get_rotation" => {
                let pitch_var = self.next_var("rot_pitch");
                let yaw_var = self.next_var("rot_yaw");
                let roll_var = self.next_var("rot_roll");
                lines.push(format!("{}let {} = rotation_x;", indent, pitch_var));
                lines.push(format!("{}let {} = rotation_y;", indent, yaw_var));
                lines.push(format!("{}let {} = rotation_z;", indent, roll_var));
                self.output_vars.insert(PinId::new(node.id, "pitch"), pitch_var);
                self.output_vars.insert(PinId::new(node.id, "yaw"), yaw_var);
                self.output_vars.insert(PinId::new(node.id, "roll"), roll_var);
            }

            // Time nodes
            "utility/get_delta" => {
                let delta_var = self.next_var("dt");
                lines.push(format!("{}let {} = delta;", indent, delta_var));
                self.output_vars.insert(PinId::new(node.id, "delta"), delta_var);
            }
            "utility/get_elapsed" => {
                let elapsed_var = self.next_var("elapsed");
                lines.push(format!("{}let {} = elapsed;", indent, elapsed_var));
                self.output_vars.insert(PinId::new(node.id, "elapsed"), elapsed_var);
            }

            // Variable getter
            "variable/get" => {
                let var_name = node.input_values.get("var_name")
                    .and_then(|v| if let PinValue::String(s) = v { Some(s.clone()) } else { None })
                    .unwrap_or_else(|| "unknown".to_string());
                self.output_vars.insert(PinId::new(node.id, "value"), var_name);
            }

            _ => {}
        }

        lines
    }

    /// Generate code for a flow node (has exec pins)
    fn generate_flow_node(&mut self, node: &BlueprintNode) -> Vec<String> {
        if self.processed_nodes.contains(&node.id) {
            return Vec::new();
        }
        self.processed_nodes.insert(node.id);

        let mut lines = Vec::new();
        let indent = self.indent_str();

        match node.node_type.as_str() {
            // Transform actions
            "transform/set_position" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                lines.push(format!("{}set_position({}, {}, {});", indent, x, y, z));
            }
            "transform/translate" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                lines.push(format!("{}translate({}, {}, {});", indent, x, y, z));
            }
            "transform/set_rotation" => {
                let pitch = self.get_input_value(node, "pitch");
                let yaw = self.get_input_value(node, "yaw");
                let roll = self.get_input_value(node, "roll");
                lines.push(format!("{}set_rotation({}, {}, {});", indent, pitch, yaw, roll));
            }
            "transform/rotate" => {
                let pitch = self.get_input_value(node, "pitch");
                let yaw = self.get_input_value(node, "yaw");
                let roll = self.get_input_value(node, "roll");
                lines.push(format!("{}rotate({}, {}, {});", indent, pitch, yaw, roll));
            }

            // Utility actions
            "utility/print" => {
                let message = self.get_input_value(node, "message");
                lines.push(format!("{}print({});", indent, message));
            }

            // Variable setter
            "variable/set" => {
                let var_name = node.input_values.get("var_name")
                    .and_then(|v| if let PinValue::String(s) = v { Some(s.clone()) } else { None })
                    .unwrap_or_else(|| "unknown".to_string());
                let value = self.get_input_value(node, "value");
                lines.push(format!("{}{} = {};", indent, var_name, value));
            }

            // Branch (if)
            "logic/if" => {
                let condition = self.get_input_value(node, "condition");
                lines.push(format!("{}if {} {{", indent, condition));

                // Generate true branch
                self.indent += 1;
                let true_lines = self.follow_flow_from(node.id, "true");
                lines.extend(true_lines);
                self.indent -= 1;

                lines.push(format!("{}}} else {{", indent));

                // Generate false branch
                self.indent += 1;
                let false_lines = self.follow_flow_from(node.id, "false");
                lines.extend(false_lines);
                self.indent -= 1;

                lines.push(format!("{}}}", indent));

                // Don't follow normal exec output for branch nodes
                return lines;
            }

            // Sequence
            "utility/sequence" => {
                // Execute each output in order
                for i in 0..4 {
                    let output_name = format!("then_{}", i);
                    let branch_lines = self.follow_flow_from(node.id, &output_name);
                    lines.extend(branch_lines);
                }
                return lines;
            }

            _ => {}
        }

        // Follow the exec output
        let exec_lines = self.follow_flow_from(node.id, "exec");
        lines.extend(exec_lines);

        lines
    }

    /// Follow flow from a node's output pin and generate code
    fn follow_flow_from(&mut self, node_id: NodeId, output_pin: &str) -> Vec<String> {
        let mut lines = Vec::new();

        let from_pin = PinId::new(node_id, output_pin);

        // Find connections from this pin
        let connections: Vec<_> = self.graph.connections_from(&from_pin).cloned().collect();

        for conn in connections {
            if let Some(next_node) = self.graph.get_node(conn.to.node_id) {
                // Skip if already processed
                if self.processed_nodes.contains(&next_node.id) {
                    continue;
                }

                // Generate any data nodes needed
                let data_lines = self.generate_dependencies(next_node);
                lines.extend(data_lines);

                // Generate the flow node
                let node_lines = self.generate_flow_node(next_node);
                lines.extend(node_lines);
            }
        }

        lines
    }

    /// Generate code for any data dependencies of a node
    fn generate_dependencies(&mut self, node: &BlueprintNode) -> Vec<String> {
        let mut lines = Vec::new();

        // Find all input connections
        for pin in node.input_pins() {
            let pin_id = PinId::new(node.id, &pin.name);
            if let Some(conn) = self.graph.connection_to(&pin_id) {
                if let Some(source_node) = self.graph.get_node(conn.from.node_id) {
                    if !self.processed_nodes.contains(&source_node.id) {
                        // Generate dependencies of the source first
                        let dep_lines = self.generate_dependencies(source_node);
                        lines.extend(dep_lines);

                        // Generate the source node
                        let source_lines = self.generate_data_node(source_node);
                        lines.extend(source_lines);
                    }
                }
            }
        }

        lines
    }
}

/// Generate Rhai code from a blueprint graph
pub fn generate_rhai_code(graph: &BlueprintGraph) -> CodegenResult {
    let mut ctx = CodegenContext::new(graph);
    let mut code_lines = Vec::new();
    let errors = Vec::new();
    let mut warnings = Vec::new();

    // Generate variable declarations
    if !graph.variables.is_empty() {
        code_lines.push("// Variables".to_string());
        for var in &graph.variables {
            let default = var.default_value.to_rhai();
            code_lines.push(format!("let {} = {};", var.name, default));
        }
        code_lines.push(String::new());
    }

    // Generate on_ready function if there's an On Ready event
    let ready_events: Vec<_> = graph.nodes.iter()
        .filter(|n| n.node_type == "event/on_ready")
        .collect();

    if !ready_events.is_empty() {
        code_lines.push("fn on_ready() {".to_string());

        for event in ready_events {
            ctx.processed_nodes.clear();
            ctx.processed_nodes.insert(event.id);

            let flow_lines = ctx.follow_flow_from(event.id, "exec");
            code_lines.extend(flow_lines);
        }

        code_lines.push("}".to_string());
        code_lines.push(String::new());
    }

    // Generate on_update function if there's an On Update event
    let update_events: Vec<_> = graph.nodes.iter()
        .filter(|n| n.node_type == "event/on_update")
        .collect();

    if !update_events.is_empty() {
        code_lines.push("fn on_update() {".to_string());

        // The delta output from On Update is available as 'delta'
        for event in &update_events {
            ctx.output_vars.insert(PinId::new(event.id, "delta"), "delta".to_string());
        }

        for event in update_events {
            ctx.processed_nodes.clear();
            ctx.processed_nodes.insert(event.id);

            let flow_lines = ctx.follow_flow_from(event.id, "exec");
            code_lines.extend(flow_lines);
        }

        code_lines.push("}".to_string());
    }

    // Check for unconnected required pins
    for node in &graph.nodes {
        for pin in node.input_pins() {
            if pin.required {
                let pin_id = PinId::new(node.id, &pin.name);
                if graph.connection_to(&pin_id).is_none() {
                    warnings.push(format!(
                        "Node '{}' has unconnected required input '{}'",
                        node.node_type, pin.name
                    ));
                }
            }
        }
    }

    // Check for nodes not connected to any event
    let mut reachable = HashSet::new();
    for event in graph.event_nodes() {
        collect_reachable(graph, event.id, &mut reachable);
    }

    for node in &graph.nodes {
        if !node.node_type.starts_with("event/")
            && !node.node_type.starts_with("utility/comment")
            && !reachable.contains(&node.id)
        {
            warnings.push(format!(
                "Node '{}' (type: {}) is not connected to any event",
                node.id.0, node.node_type
            ));
        }
    }

    CodegenResult {
        code: code_lines.join("\n"),
        errors,
        warnings,
    }
}

/// Collect all nodes reachable from a starting node
fn collect_reachable(graph: &BlueprintGraph, start: NodeId, visited: &mut HashSet<NodeId>) {
    if visited.contains(&start) {
        return;
    }
    visited.insert(start);

    // Follow all connections from this node's outputs
    if let Some(node) = graph.get_node(start) {
        for pin in node.output_pins() {
            let from_pin = PinId::new(start, &pin.name);
            for conn in graph.connections_from(&from_pin) {
                collect_reachable(graph, conn.to.node_id, visited);
            }
        }

        // Also follow data connections to this node's inputs
        for pin in node.input_pins() {
            let to_pin = PinId::new(start, &pin.name);
            if let Some(conn) = graph.connection_to(&to_pin) {
                collect_reachable(graph, conn.from.node_id, visited);
            }
        }
    }
}
