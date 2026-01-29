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
            "math/tan" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("tan");
                lines.push(format!("{}let {} = tan({});", indent, result_var, value));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/asin" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("asin");
                lines.push(format!("{}let {} = asin({});", indent, result_var, value));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/acos" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("acos");
                lines.push(format!("{}let {} = acos({});", indent, result_var, value));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/atan" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("atan");
                lines.push(format!("{}let {} = atan({});", indent, result_var, value));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/atan2" => {
                let y = self.get_input_value(node, "y");
                let x = self.get_input_value(node, "x");
                let result_var = self.next_var("atan2");
                lines.push(format!("{}let {} = atan2({}, {});", indent, result_var, y, x));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/floor" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("floor");
                lines.push(format!("{}let {} = floor({});", indent, result_var, value));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/ceil" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("ceil");
                lines.push(format!("{}let {} = ceiling({});", indent, result_var, value));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/round" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("round");
                lines.push(format!("{}let {} = round({});", indent, result_var, value));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/sqrt" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("sqrt");
                lines.push(format!("{}let {} = sqrt({});", indent, result_var, value));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/pow" => {
                let base = self.get_input_value(node, "base");
                let exp = self.get_input_value(node, "exponent");
                let result_var = self.next_var("pow");
                lines.push(format!("{}let {} = pow({}, {});", indent, result_var, base, exp));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/log" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("log");
                lines.push(format!("{}let {} = log({});", indent, result_var, value));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/exp" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("exp");
                lines.push(format!("{}let {} = exp({});", indent, result_var, value));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/sign" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("sign");
                lines.push(format!("{}let {} = if {} > 0.0 {{ 1.0 }} else if {} < 0.0 {{ -1.0 }} else {{ 0.0 }};", indent, result_var, value, value));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/mod" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let result_var = self.next_var("mod");
                lines.push(format!("{}let {} = {} % {};", indent, result_var, a, b));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/fract" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("fract");
                lines.push(format!("{}let {} = {} - floor({});", indent, result_var, value, value));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/negate" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("neg");
                lines.push(format!("{}let {} = -{};", indent, result_var, value));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/one_minus" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("oneminus");
                lines.push(format!("{}let {} = 1.0 - {};", indent, result_var, value));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/reciprocal" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("recip");
                lines.push(format!("{}let {} = 1.0 / {};", indent, result_var, value));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/smoothstep" => {
                let edge0 = self.get_input_value(node, "edge0");
                let edge1 = self.get_input_value(node, "edge1");
                let x = self.get_input_value(node, "x");
                let result_var = self.next_var("smooth");
                let t_var = self.next_var("t");
                lines.push(format!("{}let {} = clamp(({} - {}) / ({} - {}), 0.0, 1.0);", indent, t_var, x, edge0, edge1, edge0));
                lines.push(format!("{}let {} = {} * {} * (3.0 - 2.0 * {});", indent, result_var, t_var, t_var, t_var));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/step" => {
                let edge = self.get_input_value(node, "edge");
                let x = self.get_input_value(node, "x");
                let result_var = self.next_var("step");
                lines.push(format!("{}let {} = if {} < {} {{ 0.0 }} else {{ 1.0 }};", indent, result_var, x, edge));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/random" => {
                let result_var = self.next_var("rand");
                lines.push(format!("{}let {} = random();", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/random_range" => {
                let min = self.get_input_value(node, "min");
                let max = self.get_input_value(node, "max");
                let result_var = self.next_var("rand");
                lines.push(format!("{}let {} = {} + random() * ({} - {});", indent, result_var, min, max, min));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/random_int" => {
                let min = self.get_input_value(node, "min");
                let max = self.get_input_value(node, "max");
                let result_var = self.next_var("rand_int");
                lines.push(format!("{}let {} = floor({} + random() * ({} - {} + 1));", indent, result_var, min, max, min));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/map_range" => {
                let value = self.get_input_value(node, "value");
                let in_min = self.get_input_value(node, "in_min");
                let in_max = self.get_input_value(node, "in_max");
                let out_min = self.get_input_value(node, "out_min");
                let out_max = self.get_input_value(node, "out_max");
                let result_var = self.next_var("mapped");
                lines.push(format!("{}let {} = {} + ({} - {}) * (({} - {}) / ({} - {}));", indent, result_var, out_min, out_max, out_min, value, in_min, in_max, in_min));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/deg_to_rad" => {
                let degrees = self.get_input_value(node, "degrees");
                let result_var = self.next_var("rad");
                lines.push(format!("{}let {} = {} * 0.017453292519943295;", indent, result_var, degrees));
                self.output_vars.insert(PinId::new(node.id, "radians"), result_var);
            }
            "math/rad_to_deg" => {
                let radians = self.get_input_value(node, "radians");
                let result_var = self.next_var("deg");
                lines.push(format!("{}let {} = {} * 57.29577951308232;", indent, result_var, radians));
                self.output_vars.insert(PinId::new(node.id, "degrees"), result_var);
            }

            // Vector math
            "math/dot" => {
                let ax = self.get_input_value(node, "a_x");
                let ay = self.get_input_value(node, "a_y");
                let az = self.get_input_value(node, "a_z");
                let bx = self.get_input_value(node, "b_x");
                let by = self.get_input_value(node, "b_y");
                let bz = self.get_input_value(node, "b_z");
                let result_var = self.next_var("dot");
                lines.push(format!("{}let {} = {} * {} + {} * {} + {} * {};", indent, result_var, ax, bx, ay, by, az, bz));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/cross" => {
                let ax = self.get_input_value(node, "a_x");
                let ay = self.get_input_value(node, "a_y");
                let az = self.get_input_value(node, "a_z");
                let bx = self.get_input_value(node, "b_x");
                let by = self.get_input_value(node, "b_y");
                let bz = self.get_input_value(node, "b_z");
                let cx = self.next_var("cross_x");
                let cy = self.next_var("cross_y");
                let cz = self.next_var("cross_z");
                lines.push(format!("{}let {} = {} * {} - {} * {};", indent, cx, ay, bz, az, by));
                lines.push(format!("{}let {} = {} * {} - {} * {};", indent, cy, az, bx, ax, bz));
                lines.push(format!("{}let {} = {} * {} - {} * {};", indent, cz, ax, by, ay, bx));
                self.output_vars.insert(PinId::new(node.id, "x"), cx);
                self.output_vars.insert(PinId::new(node.id, "y"), cy);
                self.output_vars.insert(PinId::new(node.id, "z"), cz);
            }
            "math/normalize" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                let len_var = self.next_var("len");
                let nx = self.next_var("norm_x");
                let ny = self.next_var("norm_y");
                let nz = self.next_var("norm_z");
                lines.push(format!("{}let {} = sqrt({} * {} + {} * {} + {} * {});", indent, len_var, x, x, y, y, z, z));
                lines.push(format!("{}let {} = if {} > 0.0 {{ {} / {} }} else {{ 0.0 }};", indent, nx, len_var, x, len_var));
                lines.push(format!("{}let {} = if {} > 0.0 {{ {} / {} }} else {{ 0.0 }};", indent, ny, len_var, y, len_var));
                lines.push(format!("{}let {} = if {} > 0.0 {{ {} / {} }} else {{ 0.0 }};", indent, nz, len_var, z, len_var));
                self.output_vars.insert(PinId::new(node.id, "x"), nx);
                self.output_vars.insert(PinId::new(node.id, "y"), ny);
                self.output_vars.insert(PinId::new(node.id, "z"), nz);
            }
            "math/length" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                let result_var = self.next_var("len");
                lines.push(format!("{}let {} = sqrt({} * {} + {} * {} + {} * {});", indent, result_var, x, x, y, y, z, z));
                self.output_vars.insert(PinId::new(node.id, "length"), result_var);
            }
            "math/distance" => {
                let ax = self.get_input_value(node, "a_x");
                let ay = self.get_input_value(node, "a_y");
                let az = self.get_input_value(node, "a_z");
                let bx = self.get_input_value(node, "b_x");
                let by = self.get_input_value(node, "b_y");
                let bz = self.get_input_value(node, "b_z");
                let dx = self.next_var("dx");
                let dy = self.next_var("dy");
                let dz = self.next_var("dz");
                let result_var = self.next_var("dist");
                lines.push(format!("{}let {} = {} - {};", indent, dx, bx, ax));
                lines.push(format!("{}let {} = {} - {};", indent, dy, by, ay));
                lines.push(format!("{}let {} = {} - {};", indent, dz, bz, az));
                lines.push(format!("{}let {} = sqrt({} * {} + {} * {} + {} * {});", indent, result_var, dx, dx, dy, dy, dz, dz));
                self.output_vars.insert(PinId::new(node.id, "distance"), result_var);
            }
            "math/direction_to" => {
                let ax = self.get_input_value(node, "a_x");
                let ay = self.get_input_value(node, "a_y");
                let az = self.get_input_value(node, "a_z");
                let bx = self.get_input_value(node, "b_x");
                let by = self.get_input_value(node, "b_y");
                let bz = self.get_input_value(node, "b_z");
                let dx = self.next_var("dx");
                let dy = self.next_var("dy");
                let dz = self.next_var("dz");
                let len_var = self.next_var("len");
                let nx = self.next_var("dir_x");
                let ny = self.next_var("dir_y");
                let nz = self.next_var("dir_z");
                lines.push(format!("{}let {} = {} - {};", indent, dx, bx, ax));
                lines.push(format!("{}let {} = {} - {};", indent, dy, by, ay));
                lines.push(format!("{}let {} = {} - {};", indent, dz, bz, az));
                lines.push(format!("{}let {} = sqrt({} * {} + {} * {} + {} * {});", indent, len_var, dx, dx, dy, dy, dz, dz));
                lines.push(format!("{}let {} = if {} > 0.0 {{ {} / {} }} else {{ 0.0 }};", indent, nx, len_var, dx, len_var));
                lines.push(format!("{}let {} = if {} > 0.0 {{ {} / {} }} else {{ 0.0 }};", indent, ny, len_var, dy, len_var));
                lines.push(format!("{}let {} = if {} > 0.0 {{ {} / {} }} else {{ 0.0 }};", indent, nz, len_var, dz, len_var));
                self.output_vars.insert(PinId::new(node.id, "x"), nx);
                self.output_vars.insert(PinId::new(node.id, "y"), ny);
                self.output_vars.insert(PinId::new(node.id, "z"), nz);
            }
            "math/angle_between" => {
                let ax = self.get_input_value(node, "a_x");
                let ay = self.get_input_value(node, "a_y");
                let az = self.get_input_value(node, "a_z");
                let bx = self.get_input_value(node, "b_x");
                let by = self.get_input_value(node, "b_y");
                let bz = self.get_input_value(node, "b_z");
                let dot_var = self.next_var("dot");
                let len_a = self.next_var("len_a");
                let len_b = self.next_var("len_b");
                let rad_var = self.next_var("radians");
                let deg_var = self.next_var("degrees");
                lines.push(format!("{}let {} = {} * {} + {} * {} + {} * {};", indent, dot_var, ax, bx, ay, by, az, bz));
                lines.push(format!("{}let {} = sqrt({} * {} + {} * {} + {} * {});", indent, len_a, ax, ax, ay, ay, az, az));
                lines.push(format!("{}let {} = sqrt({} * {} + {} * {} + {} * {});", indent, len_b, bx, bx, by, by, bz, bz));
                lines.push(format!("{}let {} = acos(clamp({} / ({} * {}), -1.0, 1.0));", indent, rad_var, dot_var, len_a, len_b));
                lines.push(format!("{}let {} = {} * 57.29577951308232;", indent, deg_var, rad_var));
                self.output_vars.insert(PinId::new(node.id, "radians"), rad_var);
                self.output_vars.insert(PinId::new(node.id, "degrees"), deg_var);
            }
            "math/reflect" => {
                let dx = self.get_input_value(node, "dir_x");
                let dy = self.get_input_value(node, "dir_y");
                let dz = self.get_input_value(node, "dir_z");
                let nx = self.get_input_value(node, "normal_x");
                let ny = self.get_input_value(node, "normal_y");
                let nz = self.get_input_value(node, "normal_z");
                let dot_var = self.next_var("dot");
                let rx = self.next_var("refl_x");
                let ry = self.next_var("refl_y");
                let rz = self.next_var("refl_z");
                lines.push(format!("{}let {} = {} * {} + {} * {} + {} * {};", indent, dot_var, dx, nx, dy, ny, dz, nz));
                lines.push(format!("{}let {} = {} - 2.0 * {} * {};", indent, rx, dx, dot_var, nx));
                lines.push(format!("{}let {} = {} - 2.0 * {} * {};", indent, ry, dy, dot_var, ny));
                lines.push(format!("{}let {} = {} - 2.0 * {} * {};", indent, rz, dz, dot_var, nz));
                self.output_vars.insert(PinId::new(node.id, "x"), rx);
                self.output_vars.insert(PinId::new(node.id, "y"), ry);
                self.output_vars.insert(PinId::new(node.id, "z"), rz);
            }
            "math/lerp_vec3" => {
                let ax = self.get_input_value(node, "a_x");
                let ay = self.get_input_value(node, "a_y");
                let az = self.get_input_value(node, "a_z");
                let bx = self.get_input_value(node, "b_x");
                let by = self.get_input_value(node, "b_y");
                let bz = self.get_input_value(node, "b_z");
                let t = self.get_input_value(node, "t");
                let rx = self.next_var("lerp_x");
                let ry = self.next_var("lerp_y");
                let rz = self.next_var("lerp_z");
                lines.push(format!("{}let {} = {} + ({} - {}) * {};", indent, rx, ax, bx, ax, t));
                lines.push(format!("{}let {} = {} + ({} - {}) * {};", indent, ry, ay, by, ay, t));
                lines.push(format!("{}let {} = {} + ({} - {}) * {};", indent, rz, az, bz, az, t));
                self.output_vars.insert(PinId::new(node.id, "x"), rx);
                self.output_vars.insert(PinId::new(node.id, "y"), ry);
                self.output_vars.insert(PinId::new(node.id, "z"), rz);
            }
            "math/make_vec3" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                let result_var = self.next_var("vec3");
                lines.push(format!("{}let {} = vec3({}, {}, {});", indent, result_var, x, y, z));
                self.output_vars.insert(PinId::new(node.id, "vec3"), result_var);
            }
            "math/break_vec3" => {
                let vec3 = self.get_input_value(node, "vec3");
                let x_var = self.next_var("vec3_x");
                let y_var = self.next_var("vec3_y");
                let z_var = self.next_var("vec3_z");
                lines.push(format!("{}let {} = {}.x;", indent, x_var, vec3));
                lines.push(format!("{}let {} = {}.y;", indent, y_var, vec3));
                lines.push(format!("{}let {} = {}.z;", indent, z_var, vec3));
                self.output_vars.insert(PinId::new(node.id, "x"), x_var);
                self.output_vars.insert(PinId::new(node.id, "y"), y_var);
                self.output_vars.insert(PinId::new(node.id, "z"), z_var);
            }
            "math/random_vec3" => {
                let min = self.get_input_value(node, "min");
                let max = self.get_input_value(node, "max");
                let rx = self.next_var("rand_x");
                let ry = self.next_var("rand_y");
                let rz = self.next_var("rand_z");
                lines.push(format!("{}let {} = {} + random() * ({} - {});", indent, rx, min, max, min));
                lines.push(format!("{}let {} = {} + random() * ({} - {});", indent, ry, min, max, min));
                lines.push(format!("{}let {} = {} + random() * ({} - {});", indent, rz, min, max, min));
                self.output_vars.insert(PinId::new(node.id, "x"), rx);
                self.output_vars.insert(PinId::new(node.id, "y"), ry);
                self.output_vars.insert(PinId::new(node.id, "z"), rz);
            }
            "math/random_direction" => {
                let theta = self.next_var("theta");
                let phi = self.next_var("phi");
                let rx = self.next_var("dir_x");
                let ry = self.next_var("dir_y");
                let rz = self.next_var("dir_z");
                lines.push(format!("{}let {} = random() * 6.283185307179586;", indent, theta));
                lines.push(format!("{}let {} = acos(2.0 * random() - 1.0);", indent, phi));
                lines.push(format!("{}let {} = sin({}) * cos({});", indent, rx, phi, theta));
                lines.push(format!("{}let {} = sin({}) * sin({});", indent, ry, phi, theta));
                lines.push(format!("{}let {} = cos({});", indent, rz, phi));
                self.output_vars.insert(PinId::new(node.id, "x"), rx);
                self.output_vars.insert(PinId::new(node.id, "y"), ry);
                self.output_vars.insert(PinId::new(node.id, "z"), rz);
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
            "input/is_key_just_pressed" => {
                let key_name = node.input_values.get("key")
                    .and_then(|v| if let PinValue::String(s) = v { Some(s.as_str()) } else { None })
                    .unwrap_or("Space");
                let result_var = self.next_var("key_just");
                lines.push(format!("{}let {} = is_key_just_pressed(_keys_just_pressed, \"{}\");", indent, result_var, key_name));
                self.output_vars.insert(PinId::new(node.id, "pressed"), result_var);
            }
            "input/is_key_just_released" => {
                let key_name = node.input_values.get("key")
                    .and_then(|v| if let PinValue::String(s) = v { Some(s.as_str()) } else { None })
                    .unwrap_or("Space");
                let result_var = self.next_var("key_rel");
                lines.push(format!("{}let {} = is_key_just_released(_keys_just_released, \"{}\");", indent, result_var, key_name));
                self.output_vars.insert(PinId::new(node.id, "released"), result_var);
            }
            "input/is_mouse_button_pressed" => {
                let button = node.input_values.get("button")
                    .and_then(|v| if let PinValue::String(s) = v { Some(s.as_str()) } else { None })
                    .unwrap_or("Left");
                let result_var = self.next_var("mouse_btn");
                // Map button name to scope variable
                let scope_var = match button {
                    "Left" => "mouse_left",
                    "Right" => "mouse_right",
                    "Middle" => "mouse_middle",
                    _ => "mouse_left",
                };
                lines.push(format!("{}let {} = {};", indent, result_var, scope_var));
                self.output_vars.insert(PinId::new(node.id, "pressed"), result_var);
            }
            "input/get_mouse_scroll" => {
                let x_var = self.next_var("scroll_x");
                let y_var = self.next_var("scroll_y");
                // Only vertical scroll is tracked
                lines.push(format!("{}let {} = 0.0;", indent, x_var));
                lines.push(format!("{}let {} = mouse_scroll;", indent, y_var));
                self.output_vars.insert(PinId::new(node.id, "x"), x_var);
                self.output_vars.insert(PinId::new(node.id, "y"), y_var);
            }
            "input/get_gamepad_left_stick" => {
                let x_var = self.next_var("left_stick_x");
                let y_var = self.next_var("left_stick_y");
                lines.push(format!("{}let {} = gamepad_left_x;", indent, x_var));
                lines.push(format!("{}let {} = gamepad_left_y;", indent, y_var));
                self.output_vars.insert(PinId::new(node.id, "x"), x_var);
                self.output_vars.insert(PinId::new(node.id, "y"), y_var);
            }
            "input/get_gamepad_right_stick" => {
                let x_var = self.next_var("right_stick_x");
                let y_var = self.next_var("right_stick_y");
                lines.push(format!("{}let {} = gamepad_right_x;", indent, x_var));
                lines.push(format!("{}let {} = gamepad_right_y;", indent, y_var));
                self.output_vars.insert(PinId::new(node.id, "x"), x_var);
                self.output_vars.insert(PinId::new(node.id, "y"), y_var);
            }
            "input/is_gamepad_button_pressed" => {
                let button = node.input_values.get("button")
                    .and_then(|v| if let PinValue::String(s) = v { Some(s.as_str()) } else { None })
                    .unwrap_or("South");
                let result_var = self.next_var("gp_btn");
                // Map button name to scope variable
                let scope_var = match button {
                    "South" | "A" => "gamepad_a",
                    "East" | "B" => "gamepad_b",
                    "West" | "X" => "gamepad_x",
                    "North" | "Y" => "gamepad_y",
                    "LeftTrigger" | "LB" => "gamepad_lb",
                    "RightTrigger" | "RB" => "gamepad_rb",
                    "Select" | "Back" => "gamepad_select",
                    "Start" => "gamepad_start",
                    "LeftThumb" | "L3" => "gamepad_l3",
                    "RightThumb" | "R3" => "gamepad_r3",
                    "DPadUp" => "gamepad_dpad_up",
                    "DPadDown" => "gamepad_dpad_down",
                    "DPadLeft" => "gamepad_dpad_left",
                    "DPadRight" => "gamepad_dpad_right",
                    _ => "gamepad_a",
                };
                lines.push(format!("{}let {} = {};", indent, result_var, scope_var));
                self.output_vars.insert(PinId::new(node.id, "pressed"), result_var);
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
            "transform/get_scale" => {
                let x_var = self.next_var("scale_x");
                let y_var = self.next_var("scale_y");
                let z_var = self.next_var("scale_z");
                lines.push(format!("{}let {} = scale_x;", indent, x_var));
                lines.push(format!("{}let {} = scale_y;", indent, y_var));
                lines.push(format!("{}let {} = scale_z;", indent, z_var));
                self.output_vars.insert(PinId::new(node.id, "x"), x_var);
                self.output_vars.insert(PinId::new(node.id, "y"), y_var);
                self.output_vars.insert(PinId::new(node.id, "z"), z_var);
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

            // Physics data nodes
            "physics/get_velocity" => {
                // Uses scope variables for self velocity
                let vx_var = self.next_var("vel_x");
                let vy_var = self.next_var("vel_y");
                let vz_var = self.next_var("vel_z");
                let speed_var = self.next_var("speed");
                lines.push(format!("{}let {} = velocity_x;", indent, vx_var));
                lines.push(format!("{}let {} = velocity_y;", indent, vy_var));
                lines.push(format!("{}let {} = velocity_z;", indent, vz_var));
                lines.push(format!("{}let {} = sqrt({} * {} + {} * {} + {} * {});",
                    indent, speed_var, vx_var, vx_var, vy_var, vy_var, vz_var, vz_var));
                self.output_vars.insert(PinId::new(node.id, "velocity"), format!("vec3({}, {}, {})", vx_var, vy_var, vz_var));
                self.output_vars.insert(PinId::new(node.id, "speed"), speed_var);
            }
            "physics/get_angular_velocity" => {
                let avx_var = self.next_var("ang_vel_x");
                let avy_var = self.next_var("ang_vel_y");
                let avz_var = self.next_var("ang_vel_z");
                lines.push(format!("{}let {} = angular_velocity_x;", indent, avx_var));
                lines.push(format!("{}let {} = angular_velocity_y;", indent, avy_var));
                lines.push(format!("{}let {} = angular_velocity_z;", indent, avz_var));
                self.output_vars.insert(PinId::new(node.id, "angular_velocity"), format!("vec3({}, {}, {})", avx_var, avy_var, avz_var));
            }
            "physics/is_grounded" => {
                let grounded_var = self.next_var("grounded");
                lines.push(format!("{}let {} = is_grounded;", indent, grounded_var));
                self.output_vars.insert(PinId::new(node.id, "grounded"), grounded_var);
            }
            "physics/raycast" => {
                // Generate raycast call and store results
                let ox = self.get_input_value(node, "origin_x");
                let oy = self.get_input_value(node, "origin_y");
                let oz = self.get_input_value(node, "origin_z");
                let dx = self.get_input_value(node, "direction_x");
                let dy = self.get_input_value(node, "direction_y");
                let dz = self.get_input_value(node, "direction_z");
                let max_dist = self.get_input_value(node, "max_distance");
                let result_var = self.next_var("raycast_result");
                lines.push(format!("{}let {} = raycast({}, {}, {}, {}, {}, {}, {}, \"{}\");",
                    indent, result_var, ox, oy, oz, dx, dy, dz, max_dist, result_var));
                // Results are stored in scope variables after raycast command is processed
                self.output_vars.insert(PinId::new(node.id, "hit"), format!("{}_hit", result_var));
                self.output_vars.insert(PinId::new(node.id, "entity"), format!("{}_entity", result_var));
                self.output_vars.insert(PinId::new(node.id, "distance"), format!("{}_distance", result_var));
            }

            // Collision data nodes
            "physics/has_collision" | "physics/is_colliding" => {
                let result_var = self.next_var("is_colliding");
                lines.push(format!("{}let {} = _active_collisions.len() > 0;", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "colliding"), result_var);
            }
            "physics/get_collisions_entered" => {
                let result_var = self.next_var("collisions_entered");
                lines.push(format!("{}let {} = _collisions_entered;", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "entities"), result_var);
            }
            "physics/get_collisions_exited" => {
                let result_var = self.next_var("collisions_exited");
                lines.push(format!("{}let {} = _collisions_exited;", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "entities"), result_var);
            }
            "physics/get_active_collisions" => {
                let result_var = self.next_var("active_collisions");
                lines.push(format!("{}let {} = _active_collisions;", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "entities"), result_var);
            }
            "physics/collision_count" => {
                let result_var = self.next_var("collision_count");
                lines.push(format!("{}let {} = _active_collisions.len();", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "count"), result_var);
            }

            // Rendering data nodes
            "rendering/get_color" | "rendering/get_material_color" => {
                let r_var = self.next_var("mat_r");
                let g_var = self.next_var("mat_g");
                let b_var = self.next_var("mat_b");
                let a_var = self.next_var("mat_a");
                lines.push(format!("{}let {} = self_material_color_r;", indent, r_var));
                lines.push(format!("{}let {} = self_material_color_g;", indent, g_var));
                lines.push(format!("{}let {} = self_material_color_b;", indent, b_var));
                lines.push(format!("{}let {} = self_material_color_a;", indent, a_var));
                self.output_vars.insert(PinId::new(node.id, "r"), r_var);
                self.output_vars.insert(PinId::new(node.id, "g"), g_var);
                self.output_vars.insert(PinId::new(node.id, "b"), b_var);
                self.output_vars.insert(PinId::new(node.id, "a"), a_var);
            }
            "rendering/get_light_intensity" => {
                let result_var = self.next_var("light_intensity");
                lines.push(format!("{}let {} = self_light_intensity;", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "intensity"), result_var);
            }
            "rendering/get_light_color" => {
                let r_var = self.next_var("light_r");
                let g_var = self.next_var("light_g");
                let b_var = self.next_var("light_b");
                lines.push(format!("{}let {} = self_light_color_r;", indent, r_var));
                lines.push(format!("{}let {} = self_light_color_g;", indent, g_var));
                lines.push(format!("{}let {} = self_light_color_b;", indent, b_var));
                self.output_vars.insert(PinId::new(node.id, "r"), r_var);
                self.output_vars.insert(PinId::new(node.id, "g"), g_var);
                self.output_vars.insert(PinId::new(node.id, "b"), b_var);
            }

            // Health component data nodes
            "component/get_health" => {
                let health_var = self.next_var("health");
                let max_var = self.next_var("max_health");
                let percent_var = self.next_var("health_pct");
                lines.push(format!("{}let {} = self_health;", indent, health_var));
                lines.push(format!("{}let {} = self_max_health;", indent, max_var));
                lines.push(format!("{}let {} = self_health_percent;", indent, percent_var));
                self.output_vars.insert(PinId::new(node.id, "health"), health_var);
                self.output_vars.insert(PinId::new(node.id, "max_health"), max_var);
                self.output_vars.insert(PinId::new(node.id, "percent"), percent_var);
            }
            "component/is_dead" => {
                let result_var = self.next_var("is_dead");
                lines.push(format!("{}let {} = self_health <= 0.0;", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "dead"), result_var);
            }
            "component/is_invincible" => {
                let result_var = self.next_var("invincible");
                lines.push(format!("{}let {} = self_is_invincible;", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "invincible"), result_var);
            }

            // ECS query data nodes
            "ecs/find_entity_by_name" => {
                let name = self.get_input_value(node, "name");
                let result_var = self.next_var("found_entity");
                lines.push(format!("{}let {} = find_entity_by_name(_entities_by_name, {});", indent, result_var, name));
                self.output_vars.insert(PinId::new(node.id, "entity"), result_var);
            }
            "ecs/find_by_tag" => {
                let tag = self.get_input_value(node, "tag");
                let result_var = self.next_var("tagged_entities");
                lines.push(format!("{}let {} = get_entities_by_tag(_entities_by_tag, {});", indent, result_var, tag));
                self.output_vars.insert(PinId::new(node.id, "entities"), result_var);
            }
            "ecs/self_entity" => {
                let result_var = self.next_var("self_id");
                lines.push(format!("{}let {} = self_entity_id;", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "entity"), result_var);
            }
            "ecs/entity_valid" => {
                let entity = self.get_input_value(node, "entity");
                let result_var = self.next_var("valid");
                lines.push(format!("{}let {} = {} >= 0;", indent, result_var, entity));
                self.output_vars.insert(PinId::new(node.id, "valid"), result_var);
            }
            "ecs/get_entity_name" => {
                // Returns self_entity_name for self
                let result_var = self.next_var("name");
                lines.push(format!("{}let {} = self_entity_name;", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "name"), result_var);
            }
            "ecs/has_tag" => {
                let tag = self.get_input_value(node, "tag");
                let result_var = self.next_var("has_tag");
                lines.push(format!("{}let {} = has_entities_with_tag(_entities_by_tag, {});", indent, result_var, tag));
                self.output_vars.insert(PinId::new(node.id, "has_tag"), result_var);
            }

            // Debug data nodes
            "debug/get_fps" => {
                let result_var = self.next_var("fps");
                lines.push(format!("{}let {} = fps;", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "fps"), result_var);
            }

            // Time data nodes
            "time/delta" => {
                let delta_var = self.next_var("dt");
                let delta_ms_var = self.next_var("dt_ms");
                lines.push(format!("{}let {} = delta;", indent, delta_var));
                lines.push(format!("{}let {} = delta * 1000.0;", indent, delta_ms_var));
                self.output_vars.insert(PinId::new(node.id, "delta"), delta_var);
                self.output_vars.insert(PinId::new(node.id, "delta_ms"), delta_ms_var);
            }
            "time/elapsed" => {
                let secs_var = self.next_var("elapsed_secs");
                let ms_var = self.next_var("elapsed_ms");
                lines.push(format!("{}let {} = elapsed;", indent, secs_var));
                lines.push(format!("{}let {} = elapsed * 1000.0;", indent, ms_var));
                self.output_vars.insert(PinId::new(node.id, "seconds"), secs_var);
                self.output_vars.insert(PinId::new(node.id, "milliseconds"), ms_var);
            }
            "time/frame_count" => {
                let frame_var = self.next_var("frame_count");
                lines.push(format!("{}let {} = frame;", indent, frame_var));
                self.output_vars.insert(PinId::new(node.id, "frames"), frame_var);
            }
            "time/get_scale" => {
                let scale_var = self.next_var("time_scale");
                lines.push(format!("{}let {} = time_scale;", indent, scale_var));
                self.output_vars.insert(PinId::new(node.id, "scale"), scale_var);
            }
            "time/is_timer_finished" => {
                let timer_name = self.get_input_value(node, "timer");
                let result_var = self.next_var("timer_finished");
                lines.push(format!("{}let {} = timer_just_finished(timers_finished, {});", indent, result_var, timer_name));
                self.output_vars.insert(PinId::new(node.id, "finished"), result_var);
            }
            "time/get_timer_progress" => {
                let timer_name = self.get_input_value(node, "timer");
                let result_var = self.next_var("timer_progress");
                lines.push(format!("{}let {} = timer_progress(timers_progress, {});", indent, result_var, timer_name));
                self.output_vars.insert(PinId::new(node.id, "progress"), result_var);
            }

            // String data nodes
            "string/concat" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let result_var = self.next_var("concat");
                lines.push(format!("{}let {} = {} + {};", indent, result_var, a, b));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "string/concat_multi" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let c = self.get_input_value(node, "c");
                let d = self.get_input_value(node, "d");
                let result_var = self.next_var("concat");
                lines.push(format!("{}let {} = {} + {} + {} + {};", indent, result_var, a, b, c, d));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "string/join" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let sep = self.get_input_value(node, "separator");
                let result_var = self.next_var("joined");
                lines.push(format!("{}let {} = {} + {} + {};", indent, result_var, a, sep, b));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "string/length" => {
                let s = self.get_input_value(node, "string");
                let result_var = self.next_var("str_len");
                lines.push(format!("{}let {} = {}.len();", indent, result_var, s));
                self.output_vars.insert(PinId::new(node.id, "length"), result_var);
            }
            "string/is_empty" => {
                let s = self.get_input_value(node, "string");
                let result_var = self.next_var("is_empty");
                lines.push(format!("{}let {} = {}.is_empty();", indent, result_var, s));
                self.output_vars.insert(PinId::new(node.id, "is_empty"), result_var);
            }
            "string/contains" => {
                let s = self.get_input_value(node, "string");
                let sub = self.get_input_value(node, "substring");
                let result_var = self.next_var("contains");
                lines.push(format!("{}let {} = {}.contains({});", indent, result_var, s, sub));
                self.output_vars.insert(PinId::new(node.id, "contains"), result_var);
            }
            "string/starts_with" => {
                let s = self.get_input_value(node, "string");
                let prefix = self.get_input_value(node, "prefix");
                let result_var = self.next_var("starts_with");
                lines.push(format!("{}let {} = {}.starts_with({});", indent, result_var, s, prefix));
                self.output_vars.insert(PinId::new(node.id, "starts_with"), result_var);
            }
            "string/ends_with" => {
                let s = self.get_input_value(node, "string");
                let suffix = self.get_input_value(node, "suffix");
                let result_var = self.next_var("ends_with");
                lines.push(format!("{}let {} = {}.ends_with({});", indent, result_var, s, suffix));
                self.output_vars.insert(PinId::new(node.id, "ends_with"), result_var);
            }
            "string/index_of" => {
                let s = self.get_input_value(node, "string");
                let sub = self.get_input_value(node, "substring");
                let idx_var = self.next_var("index");
                let found_var = self.next_var("found");
                lines.push(format!("{}let {} = {}.index_of({});", indent, idx_var, s, sub));
                lines.push(format!("{}let {} = {} >= 0;", indent, found_var, idx_var));
                self.output_vars.insert(PinId::new(node.id, "index"), idx_var);
                self.output_vars.insert(PinId::new(node.id, "found"), found_var);
            }
            "string/equals" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let result_var = self.next_var("str_eq");
                lines.push(format!("{}let {} = {} == {};", indent, result_var, a, b));
                self.output_vars.insert(PinId::new(node.id, "equals"), result_var);
            }
            "string/equals_ignore_case" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let result_var = self.next_var("str_eq_ic");
                lines.push(format!("{}let {} = {}.to_lower() == {}.to_lower();", indent, result_var, a, b));
                self.output_vars.insert(PinId::new(node.id, "equals"), result_var);
            }
            "string/substring" => {
                let s = self.get_input_value(node, "string");
                let start = self.get_input_value(node, "start");
                let length = self.get_input_value(node, "length");
                let result_var = self.next_var("substr");
                lines.push(format!("{}let {} = {}.sub_string({}, {});", indent, result_var, s, start, length));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "string/char_at" => {
                let s = self.get_input_value(node, "string");
                let index = self.get_input_value(node, "index");
                let result_var = self.next_var("char");
                lines.push(format!("{}let {} = {}.sub_string({}, 1);", indent, result_var, s, index));
                self.output_vars.insert(PinId::new(node.id, "char"), result_var);
            }
            "string/replace" => {
                let s = self.get_input_value(node, "string");
                let from = self.get_input_value(node, "from");
                let to = self.get_input_value(node, "to");
                let result_var = self.next_var("replaced");
                lines.push(format!("{}let {} = {}.replace({}, {});", indent, result_var, s, from, to));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "string/split" => {
                let s = self.get_input_value(node, "string");
                let delim = self.get_input_value(node, "delimiter");
                let parts = self.next_var("parts");
                let first_var = self.next_var("first");
                let rest_var = self.next_var("rest");
                let count_var = self.next_var("count");
                lines.push(format!("{}let {} = {}.split({});", indent, parts, s, delim));
                lines.push(format!("{}let {} = {}.len();", indent, count_var, parts));
                lines.push(format!("{}let {} = if {} > 0 {{ {}[0] }} else {{ \"\" }};", indent, first_var, count_var, parts));
                lines.push(format!("{}let {} = if {} > 1 {{ {}[1] }} else {{ \"\" }};", indent, rest_var, count_var, parts));
                self.output_vars.insert(PinId::new(node.id, "first"), first_var);
                self.output_vars.insert(PinId::new(node.id, "rest"), rest_var);
                self.output_vars.insert(PinId::new(node.id, "count"), count_var);
            }
            "string/to_upper" => {
                let s = self.get_input_value(node, "string");
                let result_var = self.next_var("upper");
                lines.push(format!("{}let {} = {}.to_upper();", indent, result_var, s));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "string/to_lower" => {
                let s = self.get_input_value(node, "string");
                let result_var = self.next_var("lower");
                lines.push(format!("{}let {} = {}.to_lower();", indent, result_var, s));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "string/capitalize" => {
                let s = self.get_input_value(node, "string");
                let result_var = self.next_var("cap");
                lines.push(format!("{}let {} = {}.sub_string(0, 1).to_upper() + {}.sub_string(1, {}.len() - 1);", indent, result_var, s, s, s));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "string/trim" => {
                let s = self.get_input_value(node, "string");
                let result_var = self.next_var("trimmed");
                lines.push(format!("{}let {} = {}.trim();", indent, result_var, s));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "string/trim_start" => {
                let s = self.get_input_value(node, "string");
                let result_var = self.next_var("trimmed");
                lines.push(format!("{}let {} = {}.trim_start();", indent, result_var, s));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "string/trim_end" => {
                let s = self.get_input_value(node, "string");
                let result_var = self.next_var("trimmed");
                lines.push(format!("{}let {} = {}.trim_end();", indent, result_var, s));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "string/pad_left" => {
                let s = self.get_input_value(node, "string");
                let length = self.get_input_value(node, "length");
                let ch = self.get_input_value(node, "char");
                let result_var = self.next_var("padded");
                lines.push(format!("{}let {} = {}.pad({}, {});", indent, result_var, s, length, ch));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "string/pad_right" => {
                let s = self.get_input_value(node, "string");
                let length = self.get_input_value(node, "length");
                let ch = self.get_input_value(node, "char");
                let result_var = self.next_var("padded");
                // Rhai doesn't have pad_right so we simulate it
                let pad_len = self.next_var("pad_len");
                lines.push(format!("{}let {} = {} - {}.len();", indent, pad_len, length, s));
                lines.push(format!("{}let {} = {} + if {} > 0 {{ {}.repeat({}) }} else {{ \"\" }};", indent, result_var, s, pad_len, ch, pad_len));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "string/format" => {
                let template = self.get_input_value(node, "template");
                let arg0 = self.get_input_value(node, "arg0");
                let arg1 = self.get_input_value(node, "arg1");
                let arg2 = self.get_input_value(node, "arg2");
                let arg3 = self.get_input_value(node, "arg3");
                let result_var = self.next_var("formatted");
                lines.push(format!("{}let {} = {}.replace(\"{{0}}\", {}).replace(\"{{1}}\", {}).replace(\"{{2}}\", {}).replace(\"{{3}}\", {});",
                    indent, result_var, template, arg0, arg1, arg2, arg3));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "string/int_to_string" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("int_str");
                lines.push(format!("{}let {} = ({}).to_string();", indent, result_var, value));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "string/float_to_string" => {
                let value = self.get_input_value(node, "value");
                let _decimals = self.get_input_value(node, "decimals");
                let result_var = self.next_var("float_str");
                lines.push(format!("{}let {} = ({}).to_string();", indent, result_var, value));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "string/bool_to_string" => {
                let value = self.get_input_value(node, "value");
                let true_text = self.get_input_value(node, "true_text");
                let false_text = self.get_input_value(node, "false_text");
                let result_var = self.next_var("bool_str");
                lines.push(format!("{}let {} = if {} {{ {} }} else {{ {} }};", indent, result_var, value, true_text, false_text));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "string/string_to_int" => {
                let s = self.get_input_value(node, "string");
                let default = self.get_input_value(node, "default");
                let result_var = self.next_var("parsed_int");
                let success_var = self.next_var("parse_ok");
                lines.push(format!("{}let {} = parse_int({});", indent, result_var, s));
                lines.push(format!("{}let {} = {} != ();", indent, success_var, result_var));
                lines.push(format!("{}let {} = if {} {{ {} }} else {{ {} }};", indent, result_var, success_var, result_var, default));
                self.output_vars.insert(PinId::new(node.id, "value"), result_var);
                self.output_vars.insert(PinId::new(node.id, "success"), success_var);
            }
            "string/string_to_float" => {
                let s = self.get_input_value(node, "string");
                let default = self.get_input_value(node, "default");
                let result_var = self.next_var("parsed_float");
                let success_var = self.next_var("parse_ok");
                lines.push(format!("{}let {} = parse_float({});", indent, result_var, s));
                lines.push(format!("{}let {} = {} != ();", indent, success_var, result_var));
                lines.push(format!("{}let {} = if {} {{ {} }} else {{ {} }};", indent, result_var, success_var, result_var, default));
                self.output_vars.insert(PinId::new(node.id, "value"), result_var);
                self.output_vars.insert(PinId::new(node.id, "success"), success_var);
            }
            "string/repeat" => {
                let s = self.get_input_value(node, "string");
                let count = self.get_input_value(node, "count");
                let result_var = self.next_var("repeated");
                let temp = self.next_var("rep_temp");
                lines.push(format!("{}let {} = \"\";", indent, temp));
                lines.push(format!("{}for _ in range(0, {}) {{ {} += {}; }}", indent, count, temp, s));
                lines.push(format!("{}let {} = {};", indent, result_var, temp));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "string/reverse" => {
                let s = self.get_input_value(node, "string");
                let result_var = self.next_var("reversed");
                lines.push(format!("{}let {} = {}.chars().rev().collect::<String>();", indent, result_var, s));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }

            // Array data nodes
            "array/create" => {
                let result_var = self.next_var("arr");
                lines.push(format!("{}let {} = [];", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "array"), result_var);
            }
            "array/create_with" | "array/create_int" | "array/create_float" => {
                let i0 = self.get_input_value(node, "item0");
                let i1 = self.get_input_value(node, "item1");
                let i2 = self.get_input_value(node, "item2");
                let i3 = self.get_input_value(node, "item3");
                let result_var = self.next_var("arr");
                lines.push(format!("{}let {} = [{}, {}, {}, {}];", indent, result_var, i0, i1, i2, i3));
                self.output_vars.insert(PinId::new(node.id, "array"), result_var);
            }
            "array/get" => {
                let arr = self.get_input_value(node, "array");
                let index = self.get_input_value(node, "index");
                let item_var = self.next_var("item");
                let valid_var = self.next_var("valid");
                lines.push(format!("{}let {} = {} < {}.len();", indent, valid_var, index, arr));
                lines.push(format!("{}let {} = if {} {{ {}[{}] }} else {{ () }};", indent, item_var, valid_var, arr, index));
                self.output_vars.insert(PinId::new(node.id, "item"), item_var);
                self.output_vars.insert(PinId::new(node.id, "valid"), valid_var);
            }
            "array/first" => {
                let arr = self.get_input_value(node, "array");
                let item_var = self.next_var("first");
                let valid_var = self.next_var("valid");
                lines.push(format!("{}let {} = {}.len() > 0;", indent, valid_var, arr));
                lines.push(format!("{}let {} = if {} {{ {}[0] }} else {{ () }};", indent, item_var, valid_var, arr));
                self.output_vars.insert(PinId::new(node.id, "item"), item_var);
                self.output_vars.insert(PinId::new(node.id, "valid"), valid_var);
            }
            "array/last" => {
                let arr = self.get_input_value(node, "array");
                let item_var = self.next_var("last");
                let valid_var = self.next_var("valid");
                let len_var = self.next_var("len");
                lines.push(format!("{}let {} = {}.len();", indent, len_var, arr));
                lines.push(format!("{}let {} = {} > 0;", indent, valid_var, len_var));
                lines.push(format!("{}let {} = if {} {{ {}[{} - 1] }} else {{ () }};", indent, item_var, valid_var, arr, len_var));
                self.output_vars.insert(PinId::new(node.id, "item"), item_var);
                self.output_vars.insert(PinId::new(node.id, "valid"), valid_var);
            }
            "array/random" => {
                let arr = self.get_input_value(node, "array");
                let item_var = self.next_var("rand_item");
                let idx_var = self.next_var("rand_idx");
                let valid_var = self.next_var("valid");
                let len_var = self.next_var("len");
                lines.push(format!("{}let {} = {}.len();", indent, len_var, arr));
                lines.push(format!("{}let {} = {} > 0;", indent, valid_var, len_var));
                lines.push(format!("{}let {} = if {} {{ floor(random() * {}) }} else {{ -1 }};", indent, idx_var, valid_var, len_var));
                lines.push(format!("{}let {} = if {} {{ {}[{}] }} else {{ () }};", indent, item_var, valid_var, arr, idx_var));
                self.output_vars.insert(PinId::new(node.id, "item"), item_var);
                self.output_vars.insert(PinId::new(node.id, "index"), idx_var);
                self.output_vars.insert(PinId::new(node.id, "valid"), valid_var);
            }
            "array/length" => {
                let arr = self.get_input_value(node, "array");
                let result_var = self.next_var("arr_len");
                lines.push(format!("{}let {} = {}.len();", indent, result_var, arr));
                self.output_vars.insert(PinId::new(node.id, "length"), result_var);
            }
            "array/is_empty" => {
                let arr = self.get_input_value(node, "array");
                let result_var = self.next_var("is_empty");
                lines.push(format!("{}let {} = {}.is_empty();", indent, result_var, arr));
                self.output_vars.insert(PinId::new(node.id, "is_empty"), result_var);
            }
            "array/contains" => {
                let arr = self.get_input_value(node, "array");
                let item = self.get_input_value(node, "item");
                let result_var = self.next_var("contains");
                lines.push(format!("{}let {} = {}.contains({});", indent, result_var, arr, item));
                self.output_vars.insert(PinId::new(node.id, "contains"), result_var);
            }
            "array/find" => {
                let arr = self.get_input_value(node, "array");
                let item = self.get_input_value(node, "item");
                let idx_var = self.next_var("find_idx");
                let found_var = self.next_var("found");
                lines.push(format!("{}let {} = {}.index_of({});", indent, idx_var, arr, item));
                lines.push(format!("{}let {} = {} >= 0;", indent, found_var, idx_var));
                self.output_vars.insert(PinId::new(node.id, "index"), idx_var);
                self.output_vars.insert(PinId::new(node.id, "found"), found_var);
            }
            "array/is_valid_index" => {
                let arr = self.get_input_value(node, "array");
                let index = self.get_input_value(node, "index");
                let result_var = self.next_var("valid");
                lines.push(format!("{}let {} = {} >= 0 && {} < {}.len();", indent, result_var, index, index, arr));
                self.output_vars.insert(PinId::new(node.id, "valid"), result_var);
            }
            "array/copy" => {
                let arr = self.get_input_value(node, "array");
                let result_var = self.next_var("copy");
                lines.push(format!("{}let {} = {}.clone();", indent, result_var, arr));
                self.output_vars.insert(PinId::new(node.id, "copy"), result_var);
            }
            "array/slice" => {
                let arr = self.get_input_value(node, "array");
                let start = self.get_input_value(node, "start");
                let end = self.get_input_value(node, "end");
                let result_var = self.next_var("slice");
                let end_idx = self.next_var("end_idx");
                lines.push(format!("{}let {} = if {} < 0 {{ {}.len() }} else {{ {} }};", indent, end_idx, end, arr, end));
                lines.push(format!("{}let {} = {}.extract({}, {} - {});", indent, result_var, arr, start, end_idx, start));
                self.output_vars.insert(PinId::new(node.id, "slice"), result_var);
            }
            "array/concat" => {
                let arr_a = self.get_input_value(node, "array_a");
                let arr_b = self.get_input_value(node, "array_b");
                let result_var = self.next_var("concat");
                lines.push(format!("{}let {} = {} + {};", indent, result_var, arr_a, arr_b));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "array/sum" => {
                let arr = self.get_input_value(node, "array");
                let result_var = self.next_var("sum");
                let temp = self.next_var("sum_temp");
                lines.push(format!("{}let {} = 0.0;", indent, temp));
                lines.push(format!("{}for v in {} {{ {} += v; }}", indent, arr, temp));
                lines.push(format!("{}let {} = {};", indent, result_var, temp));
                self.output_vars.insert(PinId::new(node.id, "sum"), result_var);
            }
            "array/average" => {
                let arr = self.get_input_value(node, "array");
                let result_var = self.next_var("avg");
                let sum_temp = self.next_var("sum_temp");
                let len_var = self.next_var("len");
                lines.push(format!("{}let {} = 0.0;", indent, sum_temp));
                lines.push(format!("{}for v in {} {{ {} += v; }}", indent, arr, sum_temp));
                lines.push(format!("{}let {} = {}.len();", indent, len_var, arr));
                lines.push(format!("{}let {} = if {} > 0 {{ {} / {} }} else {{ 0.0 }};", indent, result_var, len_var, sum_temp, len_var));
                self.output_vars.insert(PinId::new(node.id, "average"), result_var);
            }
            "array/min" => {
                let arr = self.get_input_value(node, "array");
                let min_var = self.next_var("min");
                let idx_var = self.next_var("min_idx");
                let temp_min = self.next_var("temp_min");
                let temp_idx = self.next_var("temp_idx");
                let i_var = self.next_var("i");
                lines.push(format!("{}let {} = 999999999.0;", indent, temp_min));
                lines.push(format!("{}let {} = -1;", indent, temp_idx));
                lines.push(format!("{}let {} = 0;", indent, i_var));
                lines.push(format!("{}for v in {} {{ if v < {} {{ {} = v; {} = {}; }} {} += 1; }}", indent, arr, temp_min, temp_min, temp_idx, i_var, i_var));
                lines.push(format!("{}let {} = {};", indent, min_var, temp_min));
                lines.push(format!("{}let {} = {};", indent, idx_var, temp_idx));
                self.output_vars.insert(PinId::new(node.id, "min"), min_var);
                self.output_vars.insert(PinId::new(node.id, "index"), idx_var);
            }
            "array/max" => {
                let arr = self.get_input_value(node, "array");
                let max_var = self.next_var("max");
                let idx_var = self.next_var("max_idx");
                let temp_max = self.next_var("temp_max");
                let temp_idx = self.next_var("temp_idx");
                let i_var = self.next_var("i");
                lines.push(format!("{}let {} = -999999999.0;", indent, temp_max));
                lines.push(format!("{}let {} = -1;", indent, temp_idx));
                lines.push(format!("{}let {} = 0;", indent, i_var));
                lines.push(format!("{}for v in {} {{ if v > {} {{ {} = v; {} = {}; }} {} += 1; }}", indent, arr, temp_max, temp_max, temp_idx, i_var, i_var));
                lines.push(format!("{}let {} = {};", indent, max_var, temp_max));
                lines.push(format!("{}let {} = {};", indent, idx_var, temp_idx));
                self.output_vars.insert(PinId::new(node.id, "max"), max_var);
                self.output_vars.insert(PinId::new(node.id, "index"), idx_var);
            }

            // Easing data nodes
            "easing/linear" => {
                let t = self.get_input_value(node, "t");
                self.output_vars.insert(PinId::new(node.id, "result"), t);
            }
            "easing/in_quad" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = {} * {};", indent, result_var, t, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/out_quad" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = 1.0 - (1.0 - {}) * (1.0 - {});", indent, result_var, t, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/inout_quad" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = if {} < 0.5 {{ 2.0 * {} * {} }} else {{ 1.0 - pow(-2.0 * {} + 2.0, 2.0) / 2.0 }};", indent, result_var, t, t, t, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/in_cubic" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = {} * {} * {};", indent, result_var, t, t, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/out_cubic" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = 1.0 - pow(1.0 - {}, 3.0);", indent, result_var, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/inout_cubic" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = if {} < 0.5 {{ 4.0 * {} * {} * {} }} else {{ 1.0 - pow(-2.0 * {} + 2.0, 3.0) / 2.0 }};", indent, result_var, t, t, t, t, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/in_quart" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = {} * {} * {} * {};", indent, result_var, t, t, t, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/out_quart" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = 1.0 - pow(1.0 - {}, 4.0);", indent, result_var, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/inout_quart" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = if {} < 0.5 {{ 8.0 * {} * {} * {} * {} }} else {{ 1.0 - pow(-2.0 * {} + 2.0, 4.0) / 2.0 }};", indent, result_var, t, t, t, t, t, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/in_quint" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = {} * {} * {} * {} * {};", indent, result_var, t, t, t, t, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/out_quint" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = 1.0 - pow(1.0 - {}, 5.0);", indent, result_var, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/inout_quint" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = if {} < 0.5 {{ 16.0 * {} * {} * {} * {} * {} }} else {{ 1.0 - pow(-2.0 * {} + 2.0, 5.0) / 2.0 }};", indent, result_var, t, t, t, t, t, t, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/in_sine" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = 1.0 - cos({} * 1.5707963267948966);", indent, result_var, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/out_sine" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = sin({} * 1.5707963267948966);", indent, result_var, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/inout_sine" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = -(cos(3.141592653589793 * {}) - 1.0) / 2.0;", indent, result_var, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/in_expo" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = if {} == 0.0 {{ 0.0 }} else {{ pow(2.0, 10.0 * {} - 10.0) }};", indent, result_var, t, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/out_expo" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = if {} == 1.0 {{ 1.0 }} else {{ 1.0 - pow(2.0, -10.0 * {}) }};", indent, result_var, t, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/inout_expo" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = if {} == 0.0 {{ 0.0 }} else if {} == 1.0 {{ 1.0 }} else if {} < 0.5 {{ pow(2.0, 20.0 * {} - 10.0) / 2.0 }} else {{ (2.0 - pow(2.0, -20.0 * {} + 10.0)) / 2.0 }};", indent, result_var, t, t, t, t, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/in_circ" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = 1.0 - sqrt(1.0 - pow({}, 2.0));", indent, result_var, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/out_circ" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = sqrt(1.0 - pow({} - 1.0, 2.0));", indent, result_var, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/inout_circ" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = if {} < 0.5 {{ (1.0 - sqrt(1.0 - pow(2.0 * {}, 2.0))) / 2.0 }} else {{ (sqrt(1.0 - pow(-2.0 * {} + 2.0, 2.0)) + 1.0) / 2.0 }};", indent, result_var, t, t, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/in_back" => {
                let t = self.get_input_value(node, "t");
                let overshoot = self.get_input_value(node, "overshoot");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = ({} + 1.0) * {} * {} * {} - {} * {} * {};", indent, result_var, overshoot, t, t, t, overshoot, t, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/out_back" => {
                let t = self.get_input_value(node, "t");
                let overshoot = self.get_input_value(node, "overshoot");
                let result_var = self.next_var("ease");
                let t1 = self.next_var("t1");
                lines.push(format!("{}let {} = {} - 1.0;", indent, t1, t));
                lines.push(format!("{}let {} = 1.0 + ({} + 1.0) * pow({}, 3.0) + {} * pow({}, 2.0);", indent, result_var, overshoot, t1, overshoot, t1));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/inout_back" => {
                let t = self.get_input_value(node, "t");
                let overshoot = self.get_input_value(node, "overshoot");
                let result_var = self.next_var("ease");
                let c2 = self.next_var("c2");
                lines.push(format!("{}let {} = {} * 1.525;", indent, c2, overshoot));
                lines.push(format!("{}let {} = if {} < 0.5 {{ (pow(2.0 * {}, 2.0) * (({} + 1.0) * 2.0 * {} - {})) / 2.0 }} else {{ (pow(2.0 * {} - 2.0, 2.0) * (({} + 1.0) * ({} * 2.0 - 2.0) + {}) + 2.0) / 2.0 }};", indent, result_var, t, t, c2, t, c2, t, c2, t, c2));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/in_elastic" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = if {} == 0.0 {{ 0.0 }} else if {} == 1.0 {{ 1.0 }} else {{ -pow(2.0, 10.0 * {} - 10.0) * sin(({} * 10.0 - 10.75) * 2.0943951023931953) }};", indent, result_var, t, t, t, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/out_elastic" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = if {} == 0.0 {{ 0.0 }} else if {} == 1.0 {{ 1.0 }} else {{ pow(2.0, -10.0 * {}) * sin(({} * 10.0 - 0.75) * 2.0943951023931953) + 1.0 }};", indent, result_var, t, t, t, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/inout_elastic" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = if {} == 0.0 {{ 0.0 }} else if {} == 1.0 {{ 1.0 }} else if {} < 0.5 {{ -(pow(2.0, 20.0 * {} - 10.0) * sin((20.0 * {} - 11.125) * 1.3962634015954636)) / 2.0 }} else {{ (pow(2.0, -20.0 * {} + 10.0) * sin((20.0 * {} - 11.125) * 1.3962634015954636)) / 2.0 + 1.0 }};", indent, result_var, t, t, t, t, t, t, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/in_bounce" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = 1.0 - ease_out_bounce(1.0 - {});", indent, result_var, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/out_bounce" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = ease_out_bounce({});", indent, result_var, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/inout_bounce" => {
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("ease");
                lines.push(format!("{}let {} = if {} < 0.5 {{ (1.0 - ease_out_bounce(1.0 - 2.0 * {})) / 2.0 }} else {{ (1.0 + ease_out_bounce(2.0 * {} - 1.0)) / 2.0 }};", indent, result_var, t, t, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/apply" => {
                let from = self.get_input_value(node, "from");
                let to = self.get_input_value(node, "to");
                let eased_t = self.get_input_value(node, "eased_t");
                let result_var = self.next_var("eased");
                lines.push(format!("{}let {} = {} + ({} - {}) * {};", indent, result_var, from, to, from, eased_t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "easing/inverse_lerp" => {
                let from = self.get_input_value(node, "from");
                let to = self.get_input_value(node, "to");
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("t");
                lines.push(format!("{}let {} = ({} - {}) / ({} - {});", indent, result_var, value, from, to, from));
                self.output_vars.insert(PinId::new(node.id, "t"), result_var);
            }

            // AI data nodes
            "ai/distance_to_target" => {
                let entity = self.get_input_value(node, "entity");
                let target = self.get_input_value(node, "target");
                let dist_var = self.next_var("dist");
                let dist2d_var = self.next_var("dist2d");
                lines.push(format!("{}let {} = distance_to_entity({}, {});", indent, dist_var, entity, target));
                lines.push(format!("{}let {} = distance_to_entity_2d({}, {});", indent, dist2d_var, entity, target));
                self.output_vars.insert(PinId::new(node.id, "distance"), dist_var);
                self.output_vars.insert(PinId::new(node.id, "distance_2d"), dist2d_var);
            }
            "ai/distance_to_position" => {
                let entity = self.get_input_value(node, "entity");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                let dist_var = self.next_var("dist");
                let dist2d_var = self.next_var("dist2d");
                lines.push(format!("{}let {} = distance_to_position({}, {}, {}, {});", indent, dist_var, entity, x, y, z));
                lines.push(format!("{}let {} = distance_to_position_2d({}, {}, {});", indent, dist2d_var, entity, x, z));
                self.output_vars.insert(PinId::new(node.id, "distance"), dist_var);
                self.output_vars.insert(PinId::new(node.id, "distance_2d"), dist2d_var);
            }
            "ai/is_in_range" => {
                let entity = self.get_input_value(node, "entity");
                let target = self.get_input_value(node, "target");
                let range = self.get_input_value(node, "range");
                let in_range_var = self.next_var("in_range");
                let dist_var = self.next_var("dist");
                lines.push(format!("{}let {} = distance_to_entity({}, {});", indent, dist_var, entity, target));
                lines.push(format!("{}let {} = {} <= {};", indent, in_range_var, dist_var, range));
                self.output_vars.insert(PinId::new(node.id, "in_range"), in_range_var);
                self.output_vars.insert(PinId::new(node.id, "distance"), dist_var);
            }
            "ai/is_facing" => {
                let entity = self.get_input_value(node, "entity");
                let target_x = self.get_input_value(node, "target_x");
                let target_y = self.get_input_value(node, "target_y");
                let target_z = self.get_input_value(node, "target_z");
                let threshold = self.get_input_value(node, "threshold");
                let facing_var = self.next_var("is_facing");
                let angle_var = self.next_var("angle");
                lines.push(format!("{}let {} = get_angle_to({}, {}, {}, {});", indent, angle_var, entity, target_x, target_y, target_z));
                lines.push(format!("{}let {} = {} <= {};", indent, facing_var, angle_var, threshold));
                self.output_vars.insert(PinId::new(node.id, "is_facing"), facing_var);
                self.output_vars.insert(PinId::new(node.id, "angle"), angle_var);
            }
            "ai/line_of_sight" => {
                let entity = self.get_input_value(node, "entity");
                let target = self.get_input_value(node, "target");
                let result_var = self.next_var("has_los");
                lines.push(format!("{}let {} = has_line_of_sight({}, {});", indent, result_var, entity, target));
                self.output_vars.insert(PinId::new(node.id, "has_los"), result_var);
            }
            "ai/find_nearest" => {
                let from = self.get_input_value(node, "from");
                let entities = self.get_input_value(node, "entities");
                let nearest_var = self.next_var("nearest");
                let dist_var = self.next_var("dist");
                let found_var = self.next_var("found");
                lines.push(format!("{}let ({}, {}, {}) = find_nearest_entity({}, {});", indent, nearest_var, dist_var, found_var, from, entities));
                self.output_vars.insert(PinId::new(node.id, "nearest"), nearest_var);
                self.output_vars.insert(PinId::new(node.id, "distance"), dist_var);
                self.output_vars.insert(PinId::new(node.id, "found"), found_var);
            }
            "ai/find_in_range" => {
                let from = self.get_input_value(node, "from");
                let entities = self.get_input_value(node, "entities");
                let range = self.get_input_value(node, "range");
                let result_var = self.next_var("in_range");
                let count_var = self.next_var("count");
                lines.push(format!("{}let {} = find_entities_in_range({}, {}, {});", indent, result_var, from, entities, range));
                lines.push(format!("{}let {} = {}.len();", indent, count_var, result_var));
                self.output_vars.insert(PinId::new(node.id, "in_range"), result_var);
                self.output_vars.insert(PinId::new(node.id, "count"), count_var);
            }
            "ai/get_state" => {
                let entity = self.get_input_value(node, "entity");
                let result_var = self.next_var("ai_state");
                lines.push(format!("{}let {} = get_ai_state({});", indent, result_var, entity));
                self.output_vars.insert(PinId::new(node.id, "state"), result_var);
            }
            "ai/is_state" => {
                let entity = self.get_input_value(node, "entity");
                let state = self.get_input_value(node, "state");
                let result_var = self.next_var("is_state");
                lines.push(format!("{}let {} = get_ai_state({}) == {};", indent, result_var, entity, state));
                self.output_vars.insert(PinId::new(node.id, "is_state"), result_var);
            }
            "ai/next_waypoint" => {
                let path = self.get_input_value(node, "path");
                let current = self.get_input_value(node, "current_index");
                let x_var = self.next_var("wp_x");
                let y_var = self.next_var("wp_y");
                let z_var = self.next_var("wp_z");
                let next_var = self.next_var("next_idx");
                let is_last_var = self.next_var("is_last");
                lines.push(format!("{}let {} = {} + 1;", indent, next_var, current));
                lines.push(format!("{}let {} = {} >= {}.len();", indent, is_last_var, next_var, path));
                lines.push(format!("{}let {} = if {} < {}.len() {{ {}[{}].x }} else {{ 0.0 }};", indent, x_var, next_var, path, path, next_var));
                lines.push(format!("{}let {} = if {} < {}.len() {{ {}[{}].y }} else {{ 0.0 }};", indent, y_var, next_var, path, path, next_var));
                lines.push(format!("{}let {} = if {} < {}.len() {{ {}[{}].z }} else {{ 0.0 }};", indent, z_var, next_var, path, path, next_var));
                self.output_vars.insert(PinId::new(node.id, "x"), x_var);
                self.output_vars.insert(PinId::new(node.id, "y"), y_var);
                self.output_vars.insert(PinId::new(node.id, "z"), z_var);
                self.output_vars.insert(PinId::new(node.id, "next_index"), next_var);
                self.output_vars.insert(PinId::new(node.id, "is_last"), is_last_var);
            }
            "ai/is_reachable" => {
                let from_x = self.get_input_value(node, "from_x");
                let from_y = self.get_input_value(node, "from_y");
                let from_z = self.get_input_value(node, "from_z");
                let to_x = self.get_input_value(node, "to_x");
                let to_y = self.get_input_value(node, "to_y");
                let to_z = self.get_input_value(node, "to_z");
                let result_var = self.next_var("reachable");
                lines.push(format!("{}let {} = is_path_reachable({}, {}, {}, {}, {}, {});", indent, result_var, from_x, from_y, from_z, to_x, to_y, to_z));
                self.output_vars.insert(PinId::new(node.id, "reachable"), result_var);
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
            "transform/set_scale" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                lines.push(format!("{}set_scale({}, {}, {});", indent, x, y, z));
            }
            "transform/look_at" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                lines.push(format!("{}look_at({}, {}, {});", indent, x, y, z));
            }

            // Physics actions
            "physics/apply_force" => {
                let fx = self.get_input_value(node, "force_x");
                let fy = self.get_input_value(node, "force_y");
                let fz = self.get_input_value(node, "force_z");
                lines.push(format!("{}apply_force({}, {}, {});", indent, fx, fy, fz));
            }
            "physics/apply_impulse" => {
                let ix = self.get_input_value(node, "impulse_x");
                let iy = self.get_input_value(node, "impulse_y");
                let iz = self.get_input_value(node, "impulse_z");
                lines.push(format!("{}apply_impulse({}, {}, {});", indent, ix, iy, iz));
            }
            "physics/apply_torque" => {
                let tx = self.get_input_value(node, "torque_x");
                let ty = self.get_input_value(node, "torque_y");
                let tz = self.get_input_value(node, "torque_z");
                lines.push(format!("{}apply_torque({}, {}, {});", indent, tx, ty, tz));
            }
            "physics/set_velocity" => {
                let vx = self.get_input_value(node, "velocity_x");
                let vy = self.get_input_value(node, "velocity_y");
                let vz = self.get_input_value(node, "velocity_z");
                lines.push(format!("{}set_velocity({}, {}, {});", indent, vx, vy, vz));
            }
            "physics/set_angular_velocity" => {
                let avx = self.get_input_value(node, "angular_velocity_x");
                let avy = self.get_input_value(node, "angular_velocity_y");
                let avz = self.get_input_value(node, "angular_velocity_z");
                lines.push(format!("{}set_angular_velocity({}, {}, {});", indent, avx, avy, avz));
            }
            "physics/set_gravity_scale" => {
                let scale = self.get_input_value(node, "scale");
                lines.push(format!("{}set_gravity_scale({});", indent, scale));
            }

            // Audio actions
            "audio/play_sound" => {
                let sound = self.get_input_value(node, "sound");
                let volume = self.get_input_value(node, "volume");
                lines.push(format!("{}play_sound_at_volume({}, {});", indent, sound, volume));
            }
            "audio/play_sound_at" => {
                let sound = self.get_input_value(node, "sound");
                let volume = self.get_input_value(node, "volume");
                let px = self.get_input_value(node, "position_x");
                let py = self.get_input_value(node, "position_y");
                let pz = self.get_input_value(node, "position_z");
                lines.push(format!("{}play_sound_3d_at_volume({}, {}, {}, {}, {});", indent, sound, volume, px, py, pz));
            }
            "audio/play_music" => {
                let music = self.get_input_value(node, "music");
                let volume = self.get_input_value(node, "volume");
                let fade_in = self.get_input_value(node, "fade_in");
                lines.push(format!("{}play_music_with_fade({}, {}, {});", indent, music, volume, fade_in));
            }
            "audio/stop_music" => {
                let fade_out = self.get_input_value(node, "fade_out");
                lines.push(format!("{}stop_music_with_fade({});", indent, fade_out));
            }
            "audio/set_volume" | "audio/set_master_volume" => {
                let volume = self.get_input_value(node, "volume");
                lines.push(format!("{}set_master_volume({});", indent, volume));
            }
            "audio/stop_all_sounds" => {
                lines.push(format!("{}stop_all_sounds();", indent));
            }

            // ECS actions
            "ecs/spawn_entity" => {
                let name = self.get_input_value(node, "name");
                lines.push(format!("{}spawn_entity({});", indent, name));
            }
            "ecs/despawn_entity" => {
                let entity = self.get_input_value(node, "entity");
                lines.push(format!("{}despawn_entity({});", indent, entity));
            }
            "ecs/spawn_primitive" | "rendering/spawn_primitive" => {
                let primitive_type = node.input_values.get("primitive_type")
                    .and_then(|v| if let PinValue::String(s) = v { Some(s.as_str()) } else { None })
                    .unwrap_or("cube");
                let name = self.get_input_value(node, "name");
                let x = self.get_input_value(node, "position_x");
                let y = self.get_input_value(node, "position_y");
                let z = self.get_input_value(node, "position_z");
                match primitive_type {
                    "cube" => lines.push(format!("{}spawn_cube_at({}, {}, {}, {});", indent, name, x, y, z)),
                    "sphere" => lines.push(format!("{}spawn_sphere_at({}, {}, {}, {});", indent, name, x, y, z)),
                    "plane" => lines.push(format!("{}spawn_plane_at({}, {}, {}, {});", indent, name, x, y, z)),
                    "cylinder" => lines.push(format!("{}spawn_cylinder_at({}, {}, {}, {});", indent, name, x, y, z)),
                    "capsule" => lines.push(format!("{}spawn_capsule_at({}, {}, {}, {});", indent, name, x, y, z)),
                    _ => lines.push(format!("{}spawn_cube_at({}, {}, {}, {});", indent, name, x, y, z)),
                }
            }
            "ecs/despawn_self" => {
                lines.push(format!("{}despawn_self();", indent));
            }
            "ecs/add_tag" => {
                let tag = self.get_input_value(node, "tag");
                lines.push(format!("{}add_tag({});", indent, tag));
            }
            "ecs/remove_tag" => {
                let tag = self.get_input_value(node, "tag");
                lines.push(format!("{}remove_tag({});", indent, tag));
            }

            // Rendering actions
            "rendering/set_color" | "rendering/set_material_color" => {
                let r = self.get_input_value(node, "r");
                let g = self.get_input_value(node, "g");
                let b = self.get_input_value(node, "b");
                let a = self.get_input_value(node, "a");
                lines.push(format!("{}set_color({}, {}, {}, {});", indent, r, g, b, a));
            }
            "rendering/set_color_of" => {
                let entity = self.get_input_value(node, "entity");
                let r = self.get_input_value(node, "r");
                let g = self.get_input_value(node, "g");
                let b = self.get_input_value(node, "b");
                let a = self.get_input_value(node, "a");
                lines.push(format!("{}set_color_of({}, {}, {}, {}, {});", indent, entity, r, g, b, a));
            }
            "rendering/set_visibility" => {
                let visible = self.get_input_value(node, "visible");
                lines.push(format!("{}set_visible({});", indent, visible));
            }
            "rendering/set_visibility_of" => {
                let entity = self.get_input_value(node, "entity");
                let visible = self.get_input_value(node, "visible");
                lines.push(format!("{}set_visible_of({}, {});", indent, entity, visible));
            }
            "rendering/show" => {
                lines.push(format!("{}show();", indent));
            }
            "rendering/hide" => {
                lines.push(format!("{}hide();", indent));
            }
            "rendering/set_light_intensity" => {
                let intensity = self.get_input_value(node, "intensity");
                lines.push(format!("{}set_light_intensity({});", indent, intensity));
            }
            "rendering/set_light_intensity_of" => {
                let entity = self.get_input_value(node, "entity");
                let intensity = self.get_input_value(node, "intensity");
                lines.push(format!("{}set_light_intensity_of({}, {});", indent, entity, intensity));
            }
            "rendering/set_light_color" => {
                let r = self.get_input_value(node, "r");
                let g = self.get_input_value(node, "g");
                let b = self.get_input_value(node, "b");
                lines.push(format!("{}set_light_color({}, {}, {});", indent, r, g, b));
            }
            "rendering/set_light_color_of" => {
                let entity = self.get_input_value(node, "entity");
                let r = self.get_input_value(node, "r");
                let g = self.get_input_value(node, "g");
                let b = self.get_input_value(node, "b");
                lines.push(format!("{}set_light_color_of({}, {}, {}, {});", indent, entity, r, g, b));
            }
            "rendering/set_opacity" => {
                let alpha = self.get_input_value(node, "alpha");
                lines.push(format!("{}set_opacity({});", indent, alpha));
            }

            // Animation actions
            "animation/play" | "animation/play_animation" => {
                let name = self.get_input_value(node, "name");
                let looping = self.get_input_value(node, "looping");
                let speed = self.get_input_value(node, "speed");
                lines.push(format!("{}play_animation({}, {}, {});", indent, name, looping, speed));
            }
            "animation/play_animation_of" => {
                let entity = self.get_input_value(node, "entity");
                let name = self.get_input_value(node, "name");
                let looping = self.get_input_value(node, "looping");
                let speed = self.get_input_value(node, "speed");
                lines.push(format!("{}play_animation_of({}, {}, {}, {});", indent, entity, name, looping, speed));
            }
            "animation/stop" | "animation/stop_animation" => {
                lines.push(format!("{}stop_animation();", indent));
            }
            "animation/stop_animation_of" => {
                let entity = self.get_input_value(node, "entity");
                lines.push(format!("{}stop_animation_of({});", indent, entity));
            }
            "animation/set_speed" | "animation/set_animation_speed" => {
                let speed = self.get_input_value(node, "speed");
                lines.push(format!("{}set_animation_speed({});", indent, speed));
            }
            "animation/set_animation_speed_of" => {
                let entity = self.get_input_value(node, "entity");
                let speed = self.get_input_value(node, "speed");
                lines.push(format!("{}set_animation_speed_of({}, {});", indent, entity, speed));
            }

            // Camera actions
            "camera/set_target" | "camera/look_at" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                lines.push(format!("{}camera_look_at({}, {}, {});", indent, x, y, z));
            }
            "camera/set_zoom" => {
                let zoom = self.get_input_value(node, "zoom");
                lines.push(format!("{}set_camera_zoom({});", indent, zoom));
            }
            "camera/screen_shake" => {
                let intensity = self.get_input_value(node, "intensity");
                let duration = self.get_input_value(node, "duration");
                lines.push(format!("{}screen_shake({}, {});", indent, intensity, duration));
            }
            "camera/follow_entity" => {
                let entity = self.get_input_value(node, "entity");
                let offset_x = self.get_input_value(node, "offset_x");
                let offset_y = self.get_input_value(node, "offset_y");
                let offset_z = self.get_input_value(node, "offset_z");
                let smoothing = self.get_input_value(node, "smoothing");
                lines.push(format!("{}camera_follow({}, {}, {}, {}, {});", indent, entity, offset_x, offset_y, offset_z, smoothing));
            }
            "camera/stop_follow" => {
                lines.push(format!("{}camera_stop_follow();", indent));
            }

            // Scene actions
            "scene/load" | "scene/load_scene" => {
                let path = self.get_input_value(node, "path");
                lines.push(format!("{}load_scene({});", indent, path));
            }
            "scene/spawn_prefab" => {
                let path = self.get_input_value(node, "path");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                lines.push(format!("{}spawn_prefab({}, {}, {}, {});", indent, path, x, y, z));
            }
            "scene/spawn_prefab_rotated" => {
                let path = self.get_input_value(node, "path");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                let rx = self.get_input_value(node, "rotation_x");
                let ry = self.get_input_value(node, "rotation_y");
                let rz = self.get_input_value(node, "rotation_z");
                lines.push(format!("{}spawn_prefab_rotated({}, {}, {}, {}, {}, {}, {});", indent, path, x, y, z, rx, ry, rz));
            }
            "scene/spawn_prefab_here" => {
                let path = self.get_input_value(node, "path");
                lines.push(format!("{}spawn_prefab_here({});", indent, path));
            }

            // Debug actions
            "debug/log_message" | "debug/print" => {
                let message = self.get_input_value(node, "message");
                lines.push(format!("{}log({});", indent, message));
            }
            "debug/log_warning" => {
                let message = self.get_input_value(node, "message");
                lines.push(format!("{}log_warn({});", indent, message));
            }
            "debug/log_error" => {
                let message = self.get_input_value(node, "message");
                lines.push(format!("{}log_error({});", indent, message));
            }
            "debug/debug_line" => {
                let sx = self.get_input_value(node, "start_x");
                let sy = self.get_input_value(node, "start_y");
                let sz = self.get_input_value(node, "start_z");
                let ex = self.get_input_value(node, "end_x");
                let ey = self.get_input_value(node, "end_y");
                let ez = self.get_input_value(node, "end_z");
                lines.push(format!("{}draw_line({}, {}, {}, {}, {}, {});", indent, sx, sy, sz, ex, ey, ez));
            }
            "debug/debug_sphere" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                let radius = self.get_input_value(node, "radius");
                lines.push(format!("{}draw_sphere({}, {}, {}, {});", indent, x, y, z, radius));
            }
            "debug/debug_box" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                let hx = self.get_input_value(node, "half_x");
                let hy = self.get_input_value(node, "half_y");
                let hz = self.get_input_value(node, "half_z");
                lines.push(format!("{}draw_box({}, {}, {}, {}, {}, {});", indent, x, y, z, hx, hy, hz));
            }
            "debug/debug_point" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                let size = self.get_input_value(node, "size");
                lines.push(format!("{}draw_point({}, {}, {}, {});", indent, x, y, z, size));
            }
            "debug/debug_ray" => {
                let ox = self.get_input_value(node, "origin_x");
                let oy = self.get_input_value(node, "origin_y");
                let oz = self.get_input_value(node, "origin_z");
                let dx = self.get_input_value(node, "direction_x");
                let dy = self.get_input_value(node, "direction_y");
                let dz = self.get_input_value(node, "direction_z");
                let length = self.get_input_value(node, "length");
                lines.push(format!("{}draw_ray({}, {}, {}, {}, {}, {}, {});", indent, ox, oy, oz, dx, dy, dz, length));
            }
            "debug/assert" => {
                let condition = self.get_input_value(node, "condition");
                let message = self.get_input_value(node, "message");
                lines.push(format!("{}assert({}, {});", indent, condition, message));
            }

            // Utility actions
            "utility/print" => {
                let message = self.get_input_value(node, "message");
                lines.push(format!("{}print({});", indent, message));
            }

            // Health actions
            "component/damage" => {
                let entity = self.get_input_value(node, "entity");
                let amount = self.get_input_value(node, "amount");
                lines.push(format!("{}damage_entity({}, {});", indent, entity, amount));
            }
            "component/heal" => {
                let entity = self.get_input_value(node, "entity");
                let amount = self.get_input_value(node, "amount");
                lines.push(format!("{}heal_entity({}, {});", indent, entity, amount));
            }
            "component/set_health" => {
                let entity = self.get_input_value(node, "entity");
                let health = self.get_input_value(node, "health");
                lines.push(format!("{}set_health_of({}, {});", indent, entity, health));
            }
            "component/set_max_health" => {
                let entity = self.get_input_value(node, "entity");
                let max_health = self.get_input_value(node, "max_health");
                lines.push(format!("{}set_max_health_of({}, {});", indent, entity, max_health));
            }
            "component/set_invincible" => {
                let entity = self.get_input_value(node, "entity");
                let invincible = self.get_input_value(node, "invincible");
                let duration = self.get_input_value(node, "duration");
                lines.push(format!("{}set_invincible_of_duration({}, {}, {});", indent, entity, invincible, duration));
            }
            "component/kill" => {
                let entity = self.get_input_value(node, "entity");
                lines.push(format!("{}kill_entity({});", indent, entity));
            }
            "component/revive" => {
                let entity = self.get_input_value(node, "entity");
                lines.push(format!("{}revive_entity({});", indent, entity));
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

            // Timer flow nodes
            "time/start_timer" => {
                let name = self.get_input_value(node, "name");
                let duration = self.get_input_value(node, "duration");
                lines.push(format!("{}start_timer({}, {});", indent, name, duration));
            }
            "time/start_timer_repeating" => {
                let name = self.get_input_value(node, "name");
                let duration = self.get_input_value(node, "duration");
                lines.push(format!("{}start_timer_repeating({}, {});", indent, name, duration));
            }
            "time/stop_timer" => {
                let name = self.get_input_value(node, "name");
                lines.push(format!("{}stop_timer({});", indent, name));
            }
            "time/pause_timer" => {
                let name = self.get_input_value(node, "name");
                lines.push(format!("{}pause_timer({});", indent, name));
            }
            "time/resume_timer" => {
                let name = self.get_input_value(node, "name");
                lines.push(format!("{}resume_timer({});", indent, name));
            }
            "time/delay" => {
                let seconds = self.get_input_value(node, "duration");
                let callback = self.get_input_value(node, "callback");
                lines.push(format!("{}delay({}, {});", indent, seconds, callback));
            }

            // Flow control - For loop
            "flow/for" => {
                let start = self.get_input_value(node, "start");
                let end = self.get_input_value(node, "end");
                let step = self.get_input_value(node, "step");
                let index_var = self.next_var("i");

                // Make index available as output
                self.output_vars.insert(PinId::new(node.id, "index"), index_var.clone());

                lines.push(format!("{}for {} in range({}, {}, {}) {{", indent, index_var, start, end, step));

                // Generate loop body
                self.indent += 1;
                let body_lines = self.follow_flow_from(node.id, "loop");
                lines.extend(body_lines);
                self.indent -= 1;

                lines.push(format!("{}}}", indent));

                // Follow completed
                let completed_lines = self.follow_flow_from(node.id, "completed");
                lines.extend(completed_lines);

                return lines;
            }

            // Flow control - While loop
            "flow/while" => {
                let condition = self.get_input_value(node, "condition");

                lines.push(format!("{}while {} {{", indent, condition));

                // Generate loop body
                self.indent += 1;
                let body_lines = self.follow_flow_from(node.id, "loop");
                lines.extend(body_lines);
                self.indent -= 1;

                lines.push(format!("{}}}", indent));

                // Follow completed
                let completed_lines = self.follow_flow_from(node.id, "completed");
                lines.extend(completed_lines);

                return lines;
            }

            // Flow control - break
            "flow/break" => {
                lines.push(format!("{}break;", indent));
                return lines; // Don't follow exec after break
            }

            // Flow control - continue
            "flow/continue" => {
                lines.push(format!("{}continue;", indent));
                return lines; // Don't follow exec after continue
            }

            // Switch on int
            "flow/switch_int" => {
                let value = self.get_input_value(node, "value");
                lines.push(format!("{}switch {} {{", indent, value));

                for i in 0..4 {
                    let case_name = format!("case_{}", i);
                    self.indent += 1;
                    let case_lines = self.follow_flow_from(node.id, &case_name);
                    if !case_lines.is_empty() {
                        lines.push(format!("{}{} => {{", self.indent_str(), i));
                        self.indent += 1;
                        lines.extend(case_lines);
                        self.indent -= 1;
                        lines.push(format!("{}}}", self.indent_str()));
                    }
                    self.indent -= 1;
                }

                // Default case
                self.indent += 1;
                let default_lines = self.follow_flow_from(node.id, "default");
                if !default_lines.is_empty() {
                    lines.push(format!("{}_ => {{", self.indent_str()));
                    self.indent += 1;
                    lines.extend(default_lines);
                    self.indent -= 1;
                    lines.push(format!("{}}}", self.indent_str()));
                }
                self.indent -= 1;

                lines.push(format!("{}}}", indent));
                return lines;
            }

            // Do once
            "flow/do_once" => {
                let flag_var = self.next_var("do_once");
                lines.push(format!("{}if !{} {{", indent, flag_var));
                lines.push(format!("{}    {} = true;", indent, flag_var));

                self.indent += 1;
                let body_lines = self.follow_flow_from(node.id, "exec");
                lines.extend(body_lines);
                self.indent -= 1;

                lines.push(format!("{}}}", indent));
                return lines;
            }

            // Flip flop
            "flow/flip_flop" => {
                let state_var = self.next_var("flip_state");
                lines.push(format!("{}if {} {{", indent, state_var));

                self.indent += 1;
                let a_lines = self.follow_flow_from(node.id, "a");
                lines.extend(a_lines);
                self.indent -= 1;

                lines.push(format!("{}}} else {{", indent));

                self.indent += 1;
                let b_lines = self.follow_flow_from(node.id, "b");
                lines.extend(b_lines);
                self.indent -= 1;

                lines.push(format!("{}}}", indent));
                lines.push(format!("{}{} = !{};", indent, state_var, state_var));
                return lines;
            }

            // Do-while loop
            "flow/do_while" => {
                let condition = self.get_input_value(node, "condition");

                lines.push(format!("{}loop {{", indent));

                // Generate loop body
                self.indent += 1;
                let body_lines = self.follow_flow_from(node.id, "loop");
                lines.extend(body_lines);

                // Break condition at end
                lines.push(format!("{}if !({}) {{ break; }}", self.indent_str(), condition));
                self.indent -= 1;

                lines.push(format!("{}}}", indent));

                // Follow completed
                let completed_lines = self.follow_flow_from(node.id, "completed");
                lines.extend(completed_lines);

                return lines;
            }

            // For-each loop (over array)
            "flow/for_each" => {
                let array = self.get_input_value(node, "array");
                let item_var = self.next_var("item");
                let index_var = self.next_var("idx");

                // Make item and index available as outputs
                self.output_vars.insert(PinId::new(node.id, "element"), item_var.clone());
                self.output_vars.insert(PinId::new(node.id, "index"), index_var.clone());

                lines.push(format!("{}let {} = 0;", indent, index_var));
                lines.push(format!("{}for {} in {} {{", indent, item_var, array));

                // Generate loop body
                self.indent += 1;
                let body_lines = self.follow_flow_from(node.id, "loop");
                lines.extend(body_lines);
                lines.push(format!("{}{} += 1;", self.indent_str(), index_var));
                self.indent -= 1;

                lines.push(format!("{}}}", indent));

                // Follow completed
                let completed_lines = self.follow_flow_from(node.id, "completed");
                lines.extend(completed_lines);

                return lines;
            }

            // Switch on string
            "flow/switch_string" => {
                let value = self.get_input_value(node, "value");

                // Generate if-else chain for string matching
                let mut first = true;
                for i in 0..4 {
                    let case_val = node.input_values.get(&format!("case_{}_value", i))
                        .and_then(|v| if let PinValue::String(s) = v { Some(s.clone()) } else { None });

                    if let Some(case_str) = case_val {
                        self.indent += 1;
                        let case_lines = self.follow_flow_from(node.id, &format!("case_{}", i));
                        self.indent -= 1;

                        if !case_lines.is_empty() {
                            if first {
                                lines.push(format!("{}if {} == \"{}\" {{", indent, value, case_str));
                                first = false;
                            } else {
                                lines.push(format!("{}}} else if {} == \"{}\" {{", indent, value, case_str));
                            }
                            self.indent += 1;
                            lines.extend(case_lines);
                            self.indent -= 1;
                        }
                    }
                }

                // Default case
                self.indent += 1;
                let default_lines = self.follow_flow_from(node.id, "default");
                self.indent -= 1;

                if !default_lines.is_empty() {
                    if first {
                        // No cases matched, just run default
                        lines.extend(default_lines);
                    } else {
                        lines.push(format!("{}}} else {{", indent));
                        self.indent += 1;
                        lines.extend(default_lines);
                        self.indent -= 1;
                        lines.push(format!("{}}}", indent));
                    }
                } else if !first {
                    lines.push(format!("{}}}", indent));
                }

                return lines;
            }

            // Return (early exit from function)
            "flow/return" => {
                lines.push(format!("{}return;", indent));
                return lines; // Don't follow exec after return
            }

            // Gate (conditional pass-through)
            "flow/gate" => {
                let condition = self.get_input_value(node, "open");
                lines.push(format!("{}if {} {{", indent, condition));

                self.indent += 1;
                let pass_lines = self.follow_flow_from(node.id, "exec");
                lines.extend(pass_lines);
                self.indent -= 1;

                lines.push(format!("{}}}", indent));
                return lines;
            }

            // Multi-gate (round robin execution)
            "flow/multi_gate" => {
                let state_var = self.next_var("gate_idx");
                let num_outputs = 4; // Fixed number of outputs

                lines.push(format!("{}switch {} {{", indent, state_var));

                for i in 0..num_outputs {
                    self.indent += 1;
                    let out_lines = self.follow_flow_from(node.id, &format!("out_{}", i));
                    if !out_lines.is_empty() {
                        lines.push(format!("{}{} => {{", self.indent_str(), i));
                        self.indent += 1;
                        lines.extend(out_lines);
                        self.indent -= 1;
                        lines.push(format!("{}}}", self.indent_str()));
                    }
                    self.indent -= 1;
                }

                lines.push(format!("{}}}", indent));
                lines.push(format!("{}{} = ({} + 1) % {};", indent, state_var, state_var, num_outputs));
                return lines;
            }

            // Do N times
            "flow/do_n" => {
                let n = self.get_input_value(node, "n");
                let counter_var = self.next_var("do_n_count");
                lines.push(format!("{}if {} < {} {{", indent, counter_var, n));
                lines.push(format!("{}    {} += 1;", indent, counter_var));
                self.indent += 1;
                let body_lines = self.follow_flow_from(node.id, "exec");
                lines.extend(body_lines);
                self.indent -= 1;
                lines.push(format!("{}}}", indent));
                return lines;
            }

            // Parallel execution
            "flow/parallel" => {
                for i in 0..4 {
                    let branch = format!("branch_{}", i);
                    let branch_lines = self.follow_flow_from(node.id, &branch);
                    lines.extend(branch_lines);
                }
                return lines;
            }

            // Sequence flow
            "flow/sequence" => {
                for i in 0..4 {
                    let output_name = format!("then_{}", i);
                    let branch_lines = self.follow_flow_from(node.id, &output_name);
                    lines.extend(branch_lines);
                }
                return lines;
            }

            // Select nodes (data with flow for execution context)
            "flow/select_int" | "flow/select_float" | "flow/select_string" | "flow/select_vec3" | "flow/select_entity" => {
                let condition = self.get_input_value(node, "condition");
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let result_var = self.next_var("select");
                lines.push(format!("{}let {} = if {} {{ {} }} else {{ {} }};", indent, result_var, condition, a, b));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }

            // Array flow actions
            "array/push" => {
                let arr = self.get_input_value(node, "array");
                let item = self.get_input_value(node, "item");
                lines.push(format!("{}{}.push({});", indent, arr, item));
            }
            "array/pop" => {
                let arr = self.get_input_value(node, "array");
                let item_var = self.next_var("popped");
                let success_var = self.next_var("pop_ok");
                lines.push(format!("{}let {} = {}.len() > 0;", indent, success_var, arr));
                lines.push(format!("{}let {} = if {} {{ {}.pop() }} else {{ () }};", indent, item_var, success_var, arr));
                self.output_vars.insert(PinId::new(node.id, "item"), item_var);
                self.output_vars.insert(PinId::new(node.id, "success"), success_var);
            }
            "array/insert" => {
                let arr = self.get_input_value(node, "array");
                let index = self.get_input_value(node, "index");
                let item = self.get_input_value(node, "item");
                lines.push(format!("{}{}.insert({}, {});", indent, arr, index, item));
            }
            "array/remove_at" => {
                let arr = self.get_input_value(node, "array");
                let index = self.get_input_value(node, "index");
                let item_var = self.next_var("removed");
                let success_var = self.next_var("remove_ok");
                lines.push(format!("{}let {} = {} < {}.len();", indent, success_var, index, arr));
                lines.push(format!("{}let {} = if {} {{ {}.remove({}) }} else {{ () }};", indent, item_var, success_var, arr, index));
                self.output_vars.insert(PinId::new(node.id, "item"), item_var);
                self.output_vars.insert(PinId::new(node.id, "success"), success_var);
            }
            "array/remove" => {
                let arr = self.get_input_value(node, "array");
                let item = self.get_input_value(node, "item");
                let idx_var = self.next_var("rm_idx");
                let removed_var = self.next_var("removed");
                lines.push(format!("{}let {} = {}.index_of({});", indent, idx_var, arr, item));
                lines.push(format!("{}let {} = {} >= 0;", indent, removed_var, idx_var));
                lines.push(format!("{}if {} {{ {}.remove({}); }}", indent, removed_var, arr, idx_var));
                self.output_vars.insert(PinId::new(node.id, "removed"), removed_var);
            }
            "array/set" => {
                let arr = self.get_input_value(node, "array");
                let index = self.get_input_value(node, "index");
                let item = self.get_input_value(node, "item");
                lines.push(format!("{}{}[{}] = {};", indent, arr, index, item));
            }
            "array/clear" => {
                let arr = self.get_input_value(node, "array");
                lines.push(format!("{}{}.clear();", indent, arr));
            }
            "array/shuffle" => {
                let arr = self.get_input_value(node, "array");
                lines.push(format!("{}shuffle({});", indent, arr));
            }
            "array/reverse" => {
                let arr = self.get_input_value(node, "array");
                lines.push(format!("{}{}.reverse();", indent, arr));
            }
            "array/sort" => {
                let arr = self.get_input_value(node, "array");
                let ascending = self.get_input_value(node, "ascending");
                lines.push(format!("{}{}.sort();", indent, arr));
                lines.push(format!("{}if !{} {{ {}.reverse(); }}", indent, ascending, arr));
            }

            // AI flow actions
            "ai/find_path" => {
                let sx = self.get_input_value(node, "start_x");
                let sy = self.get_input_value(node, "start_y");
                let sz = self.get_input_value(node, "start_z");
                let ex = self.get_input_value(node, "end_x");
                let ey = self.get_input_value(node, "end_y");
                let ez = self.get_input_value(node, "end_z");
                let path_var = self.next_var("path");
                let found_var = self.next_var("path_found");
                let len_var = self.next_var("path_len");
                lines.push(format!("{}let ({}, {}) = find_path({}, {}, {}, {}, {}, {});", indent, path_var, found_var, sx, sy, sz, ex, ey, ez));
                lines.push(format!("{}let {} = {}.len();", indent, len_var, path_var));
                self.output_vars.insert(PinId::new(node.id, "path"), path_var);
                self.output_vars.insert(PinId::new(node.id, "found"), found_var);
                self.output_vars.insert(PinId::new(node.id, "length"), len_var);
            }
            "ai/move_to" => {
                let entity = self.get_input_value(node, "entity");
                let tx = self.get_input_value(node, "target_x");
                let ty = self.get_input_value(node, "target_y");
                let tz = self.get_input_value(node, "target_z");
                let speed = self.get_input_value(node, "speed");
                let reached_var = self.next_var("reached");
                lines.push(format!("{}let {} = move_to({}, {}, {}, {}, {});", indent, reached_var, entity, tx, ty, tz, speed));
                self.output_vars.insert(PinId::new(node.id, "reached"), reached_var);
            }
            "ai/move_along_path" => {
                let entity = self.get_input_value(node, "entity");
                let path = self.get_input_value(node, "path");
                let speed = self.get_input_value(node, "speed");
                let idx_var = self.next_var("path_idx");
                lines.push(format!("{}let {} = move_along_path({}, {}, {});", indent, idx_var, entity, path, speed));
                self.output_vars.insert(PinId::new(node.id, "current_index"), idx_var);
            }
            "ai/stop_movement" => {
                let entity = self.get_input_value(node, "entity");
                lines.push(format!("{}stop_movement({});", indent, entity));
            }
            "ai/look_at_position" => {
                let entity = self.get_input_value(node, "entity");
                let tx = self.get_input_value(node, "target_x");
                let ty = self.get_input_value(node, "target_y");
                let tz = self.get_input_value(node, "target_z");
                let speed = self.get_input_value(node, "speed");
                lines.push(format!("{}look_at_position({}, {}, {}, {}, {});", indent, entity, tx, ty, tz, speed));
            }
            "ai/look_at_target" => {
                let entity = self.get_input_value(node, "entity");
                let target = self.get_input_value(node, "target");
                let speed = self.get_input_value(node, "speed");
                lines.push(format!("{}look_at_entity({}, {}, {});", indent, entity, target, speed));
            }
            "ai/flee_from" => {
                let entity = self.get_input_value(node, "entity");
                let fx = self.get_input_value(node, "from_x");
                let fy = self.get_input_value(node, "from_y");
                let fz = self.get_input_value(node, "from_z");
                let speed = self.get_input_value(node, "speed");
                let dx_var = self.next_var("flee_dir_x");
                let dy_var = self.next_var("flee_dir_y");
                let dz_var = self.next_var("flee_dir_z");
                lines.push(format!("{}let ({}, {}, {}) = flee_from({}, {}, {}, {}, {});", indent, dx_var, dy_var, dz_var, entity, fx, fy, fz, speed));
                self.output_vars.insert(PinId::new(node.id, "direction_x"), dx_var);
                self.output_vars.insert(PinId::new(node.id, "direction_y"), dy_var);
                self.output_vars.insert(PinId::new(node.id, "direction_z"), dz_var);
            }
            "ai/wander" => {
                let entity = self.get_input_value(node, "entity");
                let radius = self.get_input_value(node, "radius");
                let speed = self.get_input_value(node, "speed");
                let tx_var = self.next_var("wander_x");
                let ty_var = self.next_var("wander_y");
                let tz_var = self.next_var("wander_z");
                lines.push(format!("{}let ({}, {}, {}) = wander({}, {}, {});", indent, tx_var, ty_var, tz_var, entity, radius, speed));
                self.output_vars.insert(PinId::new(node.id, "target_x"), tx_var);
                self.output_vars.insert(PinId::new(node.id, "target_y"), ty_var);
                self.output_vars.insert(PinId::new(node.id, "target_z"), tz_var);
            }
            "ai/patrol" => {
                let entity = self.get_input_value(node, "entity");
                let waypoints = self.get_input_value(node, "waypoints");
                let speed = self.get_input_value(node, "speed");
                let looping = self.get_input_value(node, "loop");
                let wp_var = self.next_var("patrol_wp");
                lines.push(format!("{}let {} = patrol({}, {}, {}, {});", indent, wp_var, entity, waypoints, speed, looping));
                self.output_vars.insert(PinId::new(node.id, "current_waypoint"), wp_var);
            }
            "ai/set_state" => {
                let entity = self.get_input_value(node, "entity");
                let state = self.get_input_value(node, "state");
                lines.push(format!("{}set_ai_state({}, {});", indent, entity, state));
            }

            // Time flow nodes
            "time/cooldown" => {
                let duration = self.get_input_value(node, "duration");
                let cd_var = self.next_var("cooldown");
                lines.push(format!("{}if {} <= 0.0 {{", indent, cd_var));
                lines.push(format!("{}    {} = {};", indent, cd_var, duration));
                self.indent += 1;
                let body_lines = self.follow_flow_from(node.id, "exec");
                lines.extend(body_lines);
                self.indent -= 1;
                lines.push(format!("{}}} else {{", indent));
                lines.push(format!("{}    {} -= delta;", indent, cd_var));
                lines.push(format!("{}}}", indent));
                return lines;
            }
            "time/every_n_seconds" => {
                let interval = self.get_input_value(node, "interval");
                let timer_var = self.next_var("periodic_timer");
                lines.push(format!("{}{} += delta;", indent, timer_var));
                lines.push(format!("{}while {} >= {} {{", indent, timer_var, interval));
                lines.push(format!("{}    {} -= {};", indent, timer_var, interval));
                self.indent += 1;
                let tick_lines = self.follow_flow_from(node.id, "tick");
                lines.extend(tick_lines);
                self.indent -= 1;
                lines.push(format!("{}}}", indent));
                return lines;
            }
            "time/every_n_frames" => {
                let n = self.get_input_value(node, "n");
                let counter_var = self.next_var("frame_counter");
                lines.push(format!("{}{} += 1;", indent, counter_var));
                lines.push(format!("{}if {} >= {} {{", indent, counter_var, n));
                lines.push(format!("{}    {} = 0;", indent, counter_var));
                self.indent += 1;
                let tick_lines = self.follow_flow_from(node.id, "tick");
                lines.extend(tick_lines);
                self.indent -= 1;
                lines.push(format!("{}}}", indent));
                return lines;
            }

            // Hierarchy actions
            "hierarchy/set_parent" => {
                let child = self.get_input_value(node, "child");
                let parent = self.get_input_value(node, "parent");
                lines.push(format!("{}set_parent({}, {});", indent, child, parent));
            }
            "hierarchy/remove_parent" => {
                let entity = self.get_input_value(node, "entity");
                lines.push(format!("{}remove_parent({});", indent, entity));
            }
            "hierarchy/add_child" => {
                let parent = self.get_input_value(node, "parent");
                let child = self.get_input_value(node, "child");
                lines.push(format!("{}add_child({}, {});", indent, parent, child));
            }
            "hierarchy/remove_child" => {
                let parent = self.get_input_value(node, "parent");
                let child = self.get_input_value(node, "child");
                lines.push(format!("{}remove_child({}, {});", indent, parent, child));
            }

            // State actions
            "state/set_state" => {
                let state = self.get_input_value(node, "state");
                lines.push(format!("{}set_game_state({});", indent, state));
            }
            "state/push_state" => {
                let state = self.get_input_value(node, "state");
                lines.push(format!("{}push_game_state({});", indent, state));
            }
            "state/pop_state" => {
                lines.push(format!("{}pop_game_state();", indent));
            }
            "state/pause_game" => {
                lines.push(format!("{}pause_game();", indent));
            }
            "state/resume_game" => {
                lines.push(format!("{}resume_game();", indent));
            }
            "state/toggle_pause" => {
                lines.push(format!("{}toggle_pause();", indent));
            }
            "state/quit_game" => {
                lines.push(format!("{}quit_game();", indent));
            }
            "state/restart_game" => {
                lines.push(format!("{}restart_game();", indent));
            }
            "state/set_global_var" => {
                let name = self.get_input_value(node, "name");
                let value = self.get_input_value(node, "value");
                lines.push(format!("{}set_global({}, {});", indent, name, value));
            }
            "state/save_game_data" => {
                let slot = self.get_input_value(node, "slot");
                let data = self.get_input_value(node, "data");
                lines.push(format!("{}save_game({}, {});", indent, slot, data));
            }
            "state/load_game_data" => {
                let slot = self.get_input_value(node, "slot");
                let result_var = self.next_var("save_data");
                lines.push(format!("{}let {} = load_game({});", indent, result_var, slot));
                self.output_vars.insert(PinId::new(node.id, "data"), result_var);
            }
            "state/delete_save_data" => {
                let slot = self.get_input_value(node, "slot");
                lines.push(format!("{}delete_save({});", indent, slot));
            }

            // Window actions
            "window/set_window_size" => {
                let width = self.get_input_value(node, "width");
                let height = self.get_input_value(node, "height");
                lines.push(format!("{}set_window_size({}, {});", indent, width, height));
            }
            "window/set_window_position" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                lines.push(format!("{}set_window_position({}, {});", indent, x, y));
            }
            "window/center_window" => {
                lines.push(format!("{}center_window();", indent));
            }
            "window/set_window_title" => {
                let title = self.get_input_value(node, "title");
                lines.push(format!("{}set_window_title({});", indent, title));
            }
            "window/set_fullscreen" => {
                let enabled = self.get_input_value(node, "enabled");
                lines.push(format!("{}set_fullscreen({});", indent, enabled));
            }
            "window/toggle_fullscreen" => {
                lines.push(format!("{}toggle_fullscreen();", indent));
            }
            "window/set_borderless" => {
                let enabled = self.get_input_value(node, "enabled");
                lines.push(format!("{}set_borderless({});", indent, enabled));
            }
            "window/minimize_window" => {
                lines.push(format!("{}minimize_window();", indent));
            }
            "window/maximize_window" => {
                lines.push(format!("{}maximize_window();", indent));
            }
            "window/restore_window" => {
                lines.push(format!("{}restore_window();", indent));
            }
            "window/set_resizable" => {
                let enabled = self.get_input_value(node, "resizable");
                lines.push(format!("{}set_resizable({});", indent, enabled));
            }
            "window/set_decorations" => {
                let enabled = self.get_input_value(node, "enabled");
                lines.push(format!("{}set_decorations({});", indent, enabled));
            }
            "window/set_always_on_top" => {
                let enabled = self.get_input_value(node, "enabled");
                lines.push(format!("{}set_always_on_top({});", indent, enabled));
            }
            "window/show_cursor" => {
                lines.push(format!("{}show_cursor();", indent));
            }
            "window/hide_cursor" => {
                lines.push(format!("{}hide_cursor();", indent));
            }
            "window/lock_cursor" => {
                lines.push(format!("{}lock_cursor();", indent));
            }
            "window/confine_cursor" => {
                lines.push(format!("{}confine_cursor();", indent));
            }
            "window/set_cursor_position" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                lines.push(format!("{}set_cursor_position({}, {});", indent, x, y));
            }
            "window/set_cursor_icon" => {
                let icon = self.get_input_value(node, "icon");
                lines.push(format!("{}set_cursor_icon({});", indent, icon));
            }
            "window/set_vsync" => {
                let enabled = self.get_input_value(node, "enabled");
                lines.push(format!("{}set_vsync({});", indent, enabled));
            }

            // UI actions
            "ui/spawn_text" => {
                let text = self.get_input_value(node, "text");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let entity_var = self.next_var("text_entity");
                lines.push(format!("{}let {} = spawn_text({}, {}, {});", indent, entity_var, text, x, y));
                self.output_vars.insert(PinId::new(node.id, "entity"), entity_var);
            }
            "ui/set_text" => {
                let entity = self.get_input_value(node, "entity");
                let text = self.get_input_value(node, "text");
                lines.push(format!("{}set_text({}, {});", indent, entity, text));
            }
            "ui/set_text_color" => {
                let entity = self.get_input_value(node, "entity");
                let r = self.get_input_value(node, "r");
                let g = self.get_input_value(node, "g");
                let b = self.get_input_value(node, "b");
                let a = self.get_input_value(node, "a");
                lines.push(format!("{}set_text_color({}, {}, {}, {}, {});", indent, entity, r, g, b, a));
            }
            "ui/set_font_size" => {
                let entity = self.get_input_value(node, "entity");
                let size = self.get_input_value(node, "size");
                lines.push(format!("{}set_font_size({}, {});", indent, entity, size));
            }
            "ui/spawn_button" => {
                let text = self.get_input_value(node, "text");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let width = self.get_input_value(node, "width");
                let height = self.get_input_value(node, "height");
                let entity_var = self.next_var("button_entity");
                lines.push(format!("{}let {} = spawn_button({}, {}, {}, {}, {});", indent, entity_var, text, x, y, width, height));
                self.output_vars.insert(PinId::new(node.id, "entity"), entity_var);
            }
            "ui/set_button_enabled" => {
                let entity = self.get_input_value(node, "entity");
                let enabled = self.get_input_value(node, "enabled");
                lines.push(format!("{}set_button_enabled({}, {});", indent, entity, enabled));
            }
            "ui/spawn_ui_image" => {
                let image = self.get_input_value(node, "image");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let entity_var = self.next_var("image_entity");
                lines.push(format!("{}let {} = spawn_ui_image({}, {}, {});", indent, entity_var, image, x, y));
                self.output_vars.insert(PinId::new(node.id, "entity"), entity_var);
            }
            "ui/set_ui_image" => {
                let entity = self.get_input_value(node, "entity");
                let image = self.get_input_value(node, "image");
                lines.push(format!("{}set_ui_image({}, {});", indent, entity, image));
            }
            "ui/set_image_color" => {
                let entity = self.get_input_value(node, "entity");
                let r = self.get_input_value(node, "r");
                let g = self.get_input_value(node, "g");
                let b = self.get_input_value(node, "b");
                let a = self.get_input_value(node, "a");
                lines.push(format!("{}set_image_color({}, {}, {}, {}, {});", indent, entity, r, g, b, a));
            }
            "ui/spawn_ui_node" => {
                let entity_var = self.next_var("ui_node");
                lines.push(format!("{}let {} = spawn_ui_node();", indent, entity_var));
                self.output_vars.insert(PinId::new(node.id, "entity"), entity_var);
            }
            "ui/set_ui_position" => {
                let entity = self.get_input_value(node, "entity");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                lines.push(format!("{}set_ui_position({}, {}, {});", indent, entity, x, y));
            }
            "ui/set_ui_size" => {
                let entity = self.get_input_value(node, "entity");
                let width = self.get_input_value(node, "width");
                let height = self.get_input_value(node, "height");
                lines.push(format!("{}set_ui_size({}, {}, {});", indent, entity, width, height));
            }
            "ui/set_background_color" => {
                let entity = self.get_input_value(node, "entity");
                let r = self.get_input_value(node, "r");
                let g = self.get_input_value(node, "g");
                let b = self.get_input_value(node, "b");
                let a = self.get_input_value(node, "a");
                lines.push(format!("{}set_background_color({}, {}, {}, {}, {});", indent, entity, r, g, b, a));
            }
            "ui/set_ui_visibility" => {
                let entity = self.get_input_value(node, "entity");
                let visible = self.get_input_value(node, "visible");
                lines.push(format!("{}set_ui_visibility({}, {});", indent, entity, visible));
            }
            "ui/toggle_ui_visibility" => {
                let entity = self.get_input_value(node, "entity");
                lines.push(format!("{}toggle_ui_visibility({});", indent, entity));
            }
            "ui/spawn_progress_bar" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let width = self.get_input_value(node, "width");
                let height = self.get_input_value(node, "height");
                let entity_var = self.next_var("progress_bar");
                lines.push(format!("{}let {} = spawn_progress_bar({}, {}, {}, {});", indent, entity_var, x, y, width, height));
                self.output_vars.insert(PinId::new(node.id, "entity"), entity_var);
            }
            "ui/set_progress_value" => {
                let entity = self.get_input_value(node, "entity");
                let value = self.get_input_value(node, "value");
                lines.push(format!("{}set_progress_value({}, {});", indent, entity, value));
            }
            "ui/add_ui_child" => {
                let parent = self.get_input_value(node, "parent");
                let child = self.get_input_value(node, "child");
                lines.push(format!("{}add_ui_child({}, {});", indent, parent, child));
            }
            "ui/remove_ui_child" => {
                let parent = self.get_input_value(node, "parent");
                let child = self.get_input_value(node, "child");
                lines.push(format!("{}remove_ui_child({}, {});", indent, parent, child));
            }
            "ui/set_z_index" => {
                let entity = self.get_input_value(node, "entity");
                let z = self.get_input_value(node, "z_index");
                lines.push(format!("{}set_z_index({}, {});", indent, entity, z));
            }
            "ui/bring_to_front" => {
                let entity = self.get_input_value(node, "entity");
                lines.push(format!("{}bring_to_front({});", indent, entity));
            }
            "ui/send_to_back" => {
                let entity = self.get_input_value(node, "entity");
                lines.push(format!("{}send_to_back({});", indent, entity));
            }

            // Physics configuration
            "physics/add_rigid_body" => {
                let entity = self.get_input_value(node, "entity");
                let body_type = self.get_input_value(node, "body_type");
                lines.push(format!("{}add_rigid_body({}, {});", indent, entity, body_type));
            }
            "physics/set_body_type" => {
                let entity = self.get_input_value(node, "entity");
                let body_type = self.get_input_value(node, "body_type");
                lines.push(format!("{}set_body_type({}, {});", indent, entity, body_type));
            }
            "physics/set_mass" => {
                let entity = self.get_input_value(node, "entity");
                let mass = self.get_input_value(node, "mass");
                lines.push(format!("{}set_mass({}, {});", indent, entity, mass));
            }
            "physics/add_box_collider" => {
                let hx = self.get_input_value(node, "half_x");
                let hy = self.get_input_value(node, "half_y");
                let hz = self.get_input_value(node, "half_z");
                lines.push(format!("{}add_box_collider({}, {}, {});", indent, hx, hy, hz));
            }
            "physics/add_sphere_collider" => {
                let radius = self.get_input_value(node, "radius");
                lines.push(format!("{}add_sphere_collider({});", indent, radius));
            }
            "physics/add_capsule_collider" => {
                let radius = self.get_input_value(node, "radius");
                let half_height = self.get_input_value(node, "half_height");
                lines.push(format!("{}add_capsule_collider({}, {});", indent, radius, half_height));
            }
            "physics/add_cylinder_collider" => {
                let radius = self.get_input_value(node, "radius");
                let half_height = self.get_input_value(node, "half_height");
                lines.push(format!("{}add_cylinder_collider({}, {});", indent, radius, half_height));
            }
            "physics/set_friction" => {
                let entity = self.get_input_value(node, "entity");
                let friction = self.get_input_value(node, "friction");
                lines.push(format!("{}set_friction({}, {});", indent, entity, friction));
            }
            "physics/set_restitution" => {
                let entity = self.get_input_value(node, "entity");
                let restitution = self.get_input_value(node, "restitution");
                lines.push(format!("{}set_restitution({}, {});", indent, entity, restitution));
            }
            "physics/set_linear_damping" => {
                let entity = self.get_input_value(node, "entity");
                let damping = self.get_input_value(node, "damping");
                lines.push(format!("{}set_linear_damping({}, {});", indent, entity, damping));
            }
            "physics/set_angular_damping" => {
                let entity = self.get_input_value(node, "entity");
                let damping = self.get_input_value(node, "damping");
                lines.push(format!("{}set_angular_damping({}, {});", indent, entity, damping));
            }
            "physics/lock_rotation" => {
                let entity = self.get_input_value(node, "entity");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                lines.push(format!("{}lock_rotation({}, {}, {}, {});", indent, entity, x, y, z));
            }
            "physics/lock_position" => {
                let entity = self.get_input_value(node, "entity");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                lines.push(format!("{}lock_position({}, {}, {}, {});", indent, entity, x, y, z));
            }
            "physics/apply_force_at_point" => {
                let fx = self.get_input_value(node, "force_x");
                let fy = self.get_input_value(node, "force_y");
                let fz = self.get_input_value(node, "force_z");
                let px = self.get_input_value(node, "point_x");
                let py = self.get_input_value(node, "point_y");
                let pz = self.get_input_value(node, "point_z");
                lines.push(format!("{}apply_force_at_point({}, {}, {}, {}, {}, {});", indent, fx, fy, fz, px, py, pz));
            }
            "physics/apply_torque_impulse" => {
                let tx = self.get_input_value(node, "torque_x");
                let ty = self.get_input_value(node, "torque_y");
                let tz = self.get_input_value(node, "torque_z");
                lines.push(format!("{}apply_torque_impulse({}, {}, {});", indent, tx, ty, tz));
            }
            "physics/add_character_controller" => {
                let entity = self.get_input_value(node, "entity");
                lines.push(format!("{}add_character_controller({});", indent, entity));
            }
            "physics/move_character" => {
                let dx = self.get_input_value(node, "direction_x");
                let dy = self.get_input_value(node, "direction_y");
                let dz = self.get_input_value(node, "direction_z");
                let speed = self.get_input_value(node, "speed");
                lines.push(format!("{}move_character({}, {}, {}, {});", indent, dx, dy, dz, speed));
            }

            // Animation actions
            "animation/play_animation_once" => {
                let name = self.get_input_value(node, "name");
                let speed = self.get_input_value(node, "speed");
                lines.push(format!("{}play_animation({}, false, {});", indent, name, speed));
            }
            "animation/pause_animation" => {
                lines.push(format!("{}pause_animation();", indent));
            }
            "animation/resume_animation" => {
                lines.push(format!("{}resume_animation();", indent));
            }
            "animation/set_animation_time" => {
                let time = self.get_input_value(node, "time");
                lines.push(format!("{}set_animation_time({});", indent, time));
            }
            "animation/crossfade_animation" => {
                let from = self.get_input_value(node, "from");
                let to = self.get_input_value(node, "to");
                let duration = self.get_input_value(node, "duration");
                lines.push(format!("{}crossfade_animation({}, {}, {});", indent, from, to, duration));
            }
            "animation/set_animation_weight" => {
                let name = self.get_input_value(node, "name");
                let weight = self.get_input_value(node, "weight");
                lines.push(format!("{}set_animation_weight({}, {});", indent, name, weight));
            }
            "animation/tween_position" => {
                let entity = self.get_input_value(node, "entity");
                let tx = self.get_input_value(node, "target_x");
                let ty = self.get_input_value(node, "target_y");
                let tz = self.get_input_value(node, "target_z");
                let duration = self.get_input_value(node, "duration");
                let easing = self.get_input_value(node, "easing");
                lines.push(format!("{}tween_position({}, {}, {}, {}, {}, {});", indent, entity, tx, ty, tz, duration, easing));
            }
            "animation/tween_rotation" => {
                let entity = self.get_input_value(node, "entity");
                let tx = self.get_input_value(node, "target_x");
                let ty = self.get_input_value(node, "target_y");
                let tz = self.get_input_value(node, "target_z");
                let duration = self.get_input_value(node, "duration");
                let easing = self.get_input_value(node, "easing");
                lines.push(format!("{}tween_rotation({}, {}, {}, {}, {}, {});", indent, entity, tx, ty, tz, duration, easing));
            }
            "animation/tween_scale" => {
                let entity = self.get_input_value(node, "entity");
                let tx = self.get_input_value(node, "target_x");
                let ty = self.get_input_value(node, "target_y");
                let tz = self.get_input_value(node, "target_z");
                let duration = self.get_input_value(node, "duration");
                let easing = self.get_input_value(node, "easing");
                lines.push(format!("{}tween_scale({}, {}, {}, {}, {}, {});", indent, entity, tx, ty, tz, duration, easing));
            }
            "animation/cancel_tween" => {
                let entity = self.get_input_value(node, "entity");
                lines.push(format!("{}cancel_tween({});", indent, entity));
            }
            "animation/play_sprite_animation" => {
                let name = self.get_input_value(node, "name");
                let looping = self.get_input_value(node, "looping");
                lines.push(format!("{}play_sprite_animation({}, {});", indent, name, looping));
            }
            "animation/set_sprite_frame" => {
                let frame = self.get_input_value(node, "frame");
                lines.push(format!("{}set_sprite_frame({});", indent, frame));
            }

            // Rendering lights
            "rendering/spawn_point_light" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                let intensity = self.get_input_value(node, "intensity");
                let entity_var = self.next_var("light_entity");
                lines.push(format!("{}let {} = spawn_point_light({}, {}, {}, {});", indent, entity_var, x, y, z, intensity));
                self.output_vars.insert(PinId::new(node.id, "entity"), entity_var);
            }
            "rendering/spawn_spot_light" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                let intensity = self.get_input_value(node, "intensity");
                let entity_var = self.next_var("light_entity");
                lines.push(format!("{}let {} = spawn_spot_light({}, {}, {}, {});", indent, entity_var, x, y, z, intensity));
                self.output_vars.insert(PinId::new(node.id, "entity"), entity_var);
            }
            "rendering/spawn_directional_light" => {
                let dx = self.get_input_value(node, "direction_x");
                let dy = self.get_input_value(node, "direction_y");
                let dz = self.get_input_value(node, "direction_z");
                let intensity = self.get_input_value(node, "intensity");
                let entity_var = self.next_var("light_entity");
                lines.push(format!("{}let {} = spawn_directional_light({}, {}, {}, {});", indent, entity_var, dx, dy, dz, intensity));
                self.output_vars.insert(PinId::new(node.id, "entity"), entity_var);
            }
            "rendering/set_light_range" => {
                let entity = self.get_input_value(node, "entity");
                let range = self.get_input_value(node, "range");
                lines.push(format!("{}set_light_range({}, {});", indent, entity, range));
            }
            "rendering/set_light_shadows" => {
                let entity = self.get_input_value(node, "entity");
                let enabled = self.get_input_value(node, "enabled");
                lines.push(format!("{}set_light_shadows({}, {});", indent, entity, enabled));
            }
            "rendering/set_ambient_light" => {
                let r = self.get_input_value(node, "r");
                let g = self.get_input_value(node, "g");
                let b = self.get_input_value(node, "b");
                let intensity = self.get_input_value(node, "intensity");
                lines.push(format!("{}set_ambient_light({}, {}, {}, {});", indent, r, g, b, intensity));
            }
            "rendering/set_fog" => {
                let r = self.get_input_value(node, "r");
                let g = self.get_input_value(node, "g");
                let b = self.get_input_value(node, "b");
                let start = self.get_input_value(node, "start");
                let end = self.get_input_value(node, "end");
                lines.push(format!("{}set_fog({}, {}, {}, {}, {});", indent, r, g, b, start, end));
            }
            "rendering/spawn_sprite" => {
                let texture = self.get_input_value(node, "texture");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let entity_var = self.next_var("sprite_entity");
                lines.push(format!("{}let {} = spawn_sprite({}, {}, {});", indent, entity_var, texture, x, y));
                self.output_vars.insert(PinId::new(node.id, "entity"), entity_var);
            }
            "rendering/set_sprite" => {
                let entity = self.get_input_value(node, "entity");
                let texture = self.get_input_value(node, "texture");
                lines.push(format!("{}set_sprite({}, {});", indent, entity, texture));
            }
            "rendering/set_sprite_color" => {
                let entity = self.get_input_value(node, "entity");
                let r = self.get_input_value(node, "r");
                let g = self.get_input_value(node, "g");
                let b = self.get_input_value(node, "b");
                let a = self.get_input_value(node, "a");
                lines.push(format!("{}set_sprite_color({}, {}, {}, {}, {});", indent, entity, r, g, b, a));
            }
            "rendering/set_sprite_flip" => {
                let entity = self.get_input_value(node, "entity");
                let flip_x = self.get_input_value(node, "flip_x");
                let flip_y = self.get_input_value(node, "flip_y");
                lines.push(format!("{}set_sprite_flip({}, {}, {});", indent, entity, flip_x, flip_y));
            }
            "rendering/spawn_particles" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                let entity_var = self.next_var("particles");
                lines.push(format!("{}let {} = spawn_particles({}, {}, {});", indent, entity_var, x, y, z));
                self.output_vars.insert(PinId::new(node.id, "entity"), entity_var);
            }
            "rendering/play_particles" => {
                let entity = self.get_input_value(node, "entity");
                lines.push(format!("{}play_particles({});", indent, entity));
            }
            "rendering/stop_particles" => {
                let entity = self.get_input_value(node, "entity");
                lines.push(format!("{}stop_particles({});", indent, entity));
            }

            // Camera actions
            "camera/set_perspective" => {
                let fov = self.get_input_value(node, "fov");
                lines.push(format!("{}set_camera_perspective({});", indent, fov));
            }
            "camera/set_orthographic" => {
                let size = self.get_input_value(node, "size");
                lines.push(format!("{}set_camera_orthographic({});", indent, size));
            }
            "camera/set_fov" => {
                let fov = self.get_input_value(node, "fov");
                lines.push(format!("{}set_camera_fov({});", indent, fov));
            }
            "camera/set_clear_color" => {
                let r = self.get_input_value(node, "r");
                let g = self.get_input_value(node, "g");
                let b = self.get_input_value(node, "b");
                lines.push(format!("{}set_clear_color({}, {}, {});", indent, r, g, b));
            }
            "camera/camera_orbit" => {
                let target_x = self.get_input_value(node, "target_x");
                let target_y = self.get_input_value(node, "target_y");
                let target_z = self.get_input_value(node, "target_z");
                let distance = self.get_input_value(node, "distance");
                lines.push(format!("{}camera_orbit({}, {}, {}, {});", indent, target_x, target_y, target_z, distance));
            }

            // Debug toggles
            "debug/toggle_physics_debug" => {
                lines.push(format!("{}toggle_physics_debug();", indent));
            }
            "debug/toggle_wireframe" => {
                lines.push(format!("{}toggle_wireframe();", indent));
            }
            "debug/toggle_bounding_boxes" => {
                lines.push(format!("{}toggle_bounding_boxes();", indent));
            }
            "debug/clear_debug_draws" => {
                lines.push(format!("{}clear_debug_draws();", indent));
            }
            "debug/log_value" => {
                let label = self.get_input_value(node, "label");
                let value = self.get_input_value(node, "value");
                lines.push(format!("{}log({} + \": \" + {});", indent, label, value));
            }
            "debug/start_timer" => {
                let name = self.get_input_value(node, "name");
                lines.push(format!("{}debug_start_timer({});", indent, name));
            }
            "debug/stop_timer" => {
                let name = self.get_input_value(node, "name");
                lines.push(format!("{}debug_stop_timer({});", indent, name));
            }
            "debug/assert_equal" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let message = self.get_input_value(node, "message");
                lines.push(format!("{}assert({} == {}, {});", indent, a, b, message));
            }
            "debug/breakpoint" => {
                lines.push(format!("{}breakpoint();", indent));
            }
            "debug/debug_arrow" => {
                let sx = self.get_input_value(node, "start_x");
                let sy = self.get_input_value(node, "start_y");
                let sz = self.get_input_value(node, "start_z");
                let dx = self.get_input_value(node, "direction_x");
                let dy = self.get_input_value(node, "direction_y");
                let dz = self.get_input_value(node, "direction_z");
                let length = self.get_input_value(node, "length");
                lines.push(format!("{}draw_arrow({}, {}, {}, {}, {}, {}, {});", indent, sx, sy, sz, dx, dy, dz, length));
            }
            "debug/debug_axes" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                let size = self.get_input_value(node, "size");
                lines.push(format!("{}draw_axes({}, {}, {}, {});", indent, x, y, z, size));
            }
            "debug/debug_capsule" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                let radius = self.get_input_value(node, "radius");
                let half_height = self.get_input_value(node, "half_height");
                lines.push(format!("{}draw_capsule({}, {}, {}, {}, {});", indent, x, y, z, radius, half_height));
            }
            "debug/debug_text_3d" => {
                let text = self.get_input_value(node, "text");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                lines.push(format!("{}draw_text_3d({}, {}, {}, {});", indent, text, x, y, z));
            }
            "debug/debug_text_2d" => {
                let text = self.get_input_value(node, "text");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                lines.push(format!("{}draw_text_2d({}, {}, {});", indent, text, x, y));
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
