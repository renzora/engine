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
        let pin_id = PinId::input(node.id, pin_name);
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

            // Extended math nodes
            "math/smootherstep" => {
                let edge0 = self.get_input_value(node, "edge0");
                let edge1 = self.get_input_value(node, "edge1");
                let x = self.get_input_value(node, "x");
                let result_var = self.next_var("smootherstep");
                lines.push(format!("{}let _t = clamp(({} - {}) / ({} - {}), 0.0, 1.0);", indent, x, edge0, edge1, edge0));
                lines.push(format!("{}let {} = _t * _t * _t * (_t * (_t * 6.0 - 15.0) + 10.0);", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/move_towards" => {
                let current = self.get_input_value(node, "current");
                let target = self.get_input_value(node, "target");
                let max_delta = self.get_input_value(node, "max_delta");
                let result_var = self.next_var("move_towards");
                lines.push(format!("{}let {} = move_towards({}, {}, {});", indent, result_var, current, target, max_delta));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/normalize_angle" => {
                let angle = self.get_input_value(node, "angle");
                let result_var = self.next_var("norm_angle");
                lines.push(format!("{}let {} = normalize_angle({});", indent, result_var, angle));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/angle_difference" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let result_var = self.next_var("angle_diff");
                lines.push(format!("{}let {} = angle_difference({}, {});", indent, result_var, a, b));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/lerp_angle" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let t = self.get_input_value(node, "t");
                let result_var = self.next_var("lerp_angle");
                lines.push(format!("{}let {} = lerp_angle({}, {}, {});", indent, result_var, a, b, t));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/inverse_lerp" => {
                let a = self.get_input_value(node, "a");
                let b = self.get_input_value(node, "b");
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("inv_lerp");
                lines.push(format!("{}let {} = ({} - {}) / ({} - {});", indent, result_var, value, a, b, a));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/pi" => {
                let result_var = self.next_var("pi");
                lines.push(format!("{}let {} = 3.141592653589793;", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "value"), result_var);
            }
            "math/tau" => {
                let result_var = self.next_var("tau");
                lines.push(format!("{}let {} = 6.283185307179586;", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "value"), result_var);
            }
            "math/e" => {
                let result_var = self.next_var("e");
                lines.push(format!("{}let {} = 2.718281828459045;", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "value"), result_var);
            }
            "math/trunc" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("trunc");
                lines.push(format!("{}let {} = trunc({});", indent, result_var, value));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/log10" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("log10");
                lines.push(format!("{}let {} = log10({});", indent, result_var, value));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/log2" => {
                let value = self.get_input_value(node, "value");
                let result_var = self.next_var("log2");
                lines.push(format!("{}let {} = log2({});", indent, result_var, value));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/distance_2d" => {
                let x1 = self.get_input_value(node, "x1");
                let y1 = self.get_input_value(node, "y1");
                let x2 = self.get_input_value(node, "x2");
                let y2 = self.get_input_value(node, "y2");
                let result_var = self.next_var("dist2d");
                lines.push(format!("{}let {} = sqrt(({} - {}) * ({} - {}) + ({} - {}) * ({} - {}));", indent, result_var, x2, x1, x2, x1, y2, y1, y2, y1));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "math/length_2d" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let result_var = self.next_var("len2d");
                lines.push(format!("{}let {} = sqrt({} * {} + {} * {});", indent, result_var, x, x, y, y));
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
                lines.push(format!("{}let {} = is_key_pressed(_keys_pressed, \"{}\");", indent, result_var, key_name));
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
            "ecs/find_by_name" | "ecs/find_entity_by_name" => {
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
            "ecs/self" | "ecs/self_entity" => {
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
            "ecs/get_name" | "ecs/get_entity_name" => {
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
            "time/timer_finished" | "time/is_timer_finished" => {
                let timer_name = self.get_input_value(node, "timer");
                let result_var = self.next_var("timer_finished");
                lines.push(format!("{}let {} = timer_just_finished(timers_finished, {});", indent, result_var, timer_name));
                self.output_vars.insert(PinId::new(node.id, "finished"), result_var);
            }
            "time/timer_progress" | "time/get_timer_progress" => {
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

            // ── Hierarchy data nodes ──────────────────────────────────
            "hierarchy/get_parent" => {
                let entity = self.get_input_value(node, "entity");
                let result_var = self.next_var("parent");
                lines.push(format!("{}let {} = get_parent({});", indent, result_var, entity));
                self.output_vars.insert(PinId::new(node.id, "parent"), result_var);
            }
            "hierarchy/has_parent" => {
                let entity = self.get_input_value(node, "entity");
                let result_var = self.next_var("has_parent");
                lines.push(format!("{}let {} = has_parent({});", indent, result_var, entity));
                self.output_vars.insert(PinId::new(node.id, "has_parent"), result_var);
            }
            "hierarchy/get_children" => {
                let entity = self.get_input_value(node, "entity");
                let result_var = self.next_var("children");
                lines.push(format!("{}let {} = get_children({});", indent, result_var, entity));
                self.output_vars.insert(PinId::new(node.id, "children"), result_var);
            }
            "hierarchy/get_child_at" => {
                let entity = self.get_input_value(node, "entity");
                let index = self.get_input_value(node, "index");
                let result_var = self.next_var("child");
                lines.push(format!("{}let {} = get_child_at({}, {});", indent, result_var, entity, index));
                self.output_vars.insert(PinId::new(node.id, "child"), result_var);
            }
            "hierarchy/get_child_count" => {
                let entity = self.get_input_value(node, "entity");
                let result_var = self.next_var("child_count");
                lines.push(format!("{}let {} = get_child_count({});", indent, result_var, entity));
                self.output_vars.insert(PinId::new(node.id, "count"), result_var);
            }
            "hierarchy/has_children" => {
                let entity = self.get_input_value(node, "entity");
                let result_var = self.next_var("has_children");
                lines.push(format!("{}let {} = get_child_count({}) > 0;", indent, result_var, entity));
                self.output_vars.insert(PinId::new(node.id, "has_children"), result_var);
            }
            "hierarchy/get_root" => {
                let entity = self.get_input_value(node, "entity");
                let result_var = self.next_var("root");
                lines.push(format!("{}let {} = get_root({});", indent, result_var, entity));
                self.output_vars.insert(PinId::new(node.id, "root"), result_var);
            }
            "hierarchy/is_root" => {
                let entity = self.get_input_value(node, "entity");
                let result_var = self.next_var("is_root");
                lines.push(format!("{}let {} = !has_parent({});", indent, result_var, entity));
                self.output_vars.insert(PinId::new(node.id, "is_root"), result_var);
            }
            "hierarchy/is_ancestor_of" => {
                let ancestor = self.get_input_value(node, "ancestor");
                let descendant = self.get_input_value(node, "descendant");
                let result_var = self.next_var("is_ancestor");
                lines.push(format!("{}let {} = is_ancestor_of({}, {});", indent, result_var, ancestor, descendant));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "hierarchy/is_descendant_of" => {
                let descendant = self.get_input_value(node, "descendant");
                let ancestor = self.get_input_value(node, "ancestor");
                let result_var = self.next_var("is_descendant");
                lines.push(format!("{}let {} = is_ancestor_of({}, {});", indent, result_var, ancestor, descendant));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "hierarchy/get_all_descendants" => {
                let entity = self.get_input_value(node, "entity");
                let result_var = self.next_var("descendants");
                lines.push(format!("{}let {} = get_all_descendants({});", indent, result_var, entity));
                self.output_vars.insert(PinId::new(node.id, "descendants"), result_var);
            }
            "hierarchy/get_depth" => {
                let entity = self.get_input_value(node, "entity");
                let result_var = self.next_var("depth");
                lines.push(format!("{}let {} = get_depth({});", indent, result_var, entity));
                self.output_vars.insert(PinId::new(node.id, "depth"), result_var);
            }
            "hierarchy/get_local_position" => {
                let entity = self.get_input_value(node, "entity");
                let x_var = self.next_var("lx");
                let y_var = self.next_var("ly");
                let z_var = self.next_var("lz");
                lines.push(format!("{}let {} = get_local_position_x({});", indent, x_var, entity));
                lines.push(format!("{}let {} = get_local_position_y({});", indent, y_var, entity));
                lines.push(format!("{}let {} = get_local_position_z({});", indent, z_var, entity));
                self.output_vars.insert(PinId::new(node.id, "x"), x_var);
                self.output_vars.insert(PinId::new(node.id, "y"), y_var);
                self.output_vars.insert(PinId::new(node.id, "z"), z_var);
            }
            "hierarchy/get_local_rotation" => {
                let entity = self.get_input_value(node, "entity");
                let x_var = self.next_var("lrx");
                let y_var = self.next_var("lry");
                let z_var = self.next_var("lrz");
                lines.push(format!("{}let {} = get_local_rotation_x({});", indent, x_var, entity));
                lines.push(format!("{}let {} = get_local_rotation_y({});", indent, y_var, entity));
                lines.push(format!("{}let {} = get_local_rotation_z({});", indent, z_var, entity));
                self.output_vars.insert(PinId::new(node.id, "x"), x_var);
                self.output_vars.insert(PinId::new(node.id, "y"), y_var);
                self.output_vars.insert(PinId::new(node.id, "z"), z_var);
            }
            "hierarchy/get_local_scale" => {
                let entity = self.get_input_value(node, "entity");
                let x_var = self.next_var("lsx");
                let y_var = self.next_var("lsy");
                let z_var = self.next_var("lsz");
                lines.push(format!("{}let {} = get_local_scale_x({});", indent, x_var, entity));
                lines.push(format!("{}let {} = get_local_scale_y({});", indent, y_var, entity));
                lines.push(format!("{}let {} = get_local_scale_z({});", indent, z_var, entity));
                self.output_vars.insert(PinId::new(node.id, "x"), x_var);
                self.output_vars.insert(PinId::new(node.id, "y"), y_var);
                self.output_vars.insert(PinId::new(node.id, "z"), z_var);
            }
            "hierarchy/local_to_world" => {
                let entity = self.get_input_value(node, "entity");
                let lx = self.get_input_value(node, "x");
                let ly = self.get_input_value(node, "y");
                let lz = self.get_input_value(node, "z");
                let x_var = self.next_var("wx");
                let y_var = self.next_var("wy");
                let z_var = self.next_var("wz");
                lines.push(format!("{}let _ltw = local_to_world({}, {}, {}, {});", indent, entity, lx, ly, lz));
                lines.push(format!("{}let {} = _ltw[0];", indent, x_var));
                lines.push(format!("{}let {} = _ltw[1];", indent, y_var));
                lines.push(format!("{}let {} = _ltw[2];", indent, z_var));
                self.output_vars.insert(PinId::new(node.id, "x"), x_var);
                self.output_vars.insert(PinId::new(node.id, "y"), y_var);
                self.output_vars.insert(PinId::new(node.id, "z"), z_var);
            }
            "hierarchy/world_to_local" => {
                let entity = self.get_input_value(node, "entity");
                let wx = self.get_input_value(node, "x");
                let wy = self.get_input_value(node, "y");
                let wz = self.get_input_value(node, "z");
                let x_var = self.next_var("lx");
                let y_var = self.next_var("ly");
                let z_var = self.next_var("lz");
                lines.push(format!("{}let _wtl = world_to_local({}, {}, {}, {});", indent, entity, wx, wy, wz));
                lines.push(format!("{}let {} = _wtl[0];", indent, x_var));
                lines.push(format!("{}let {} = _wtl[1];", indent, y_var));
                lines.push(format!("{}let {} = _wtl[2];", indent, z_var));
                self.output_vars.insert(PinId::new(node.id, "x"), x_var);
                self.output_vars.insert(PinId::new(node.id, "y"), y_var);
                self.output_vars.insert(PinId::new(node.id, "z"), z_var);
            }

            // ── Camera data nodes ────────────────────────────────────
            "camera/world_to_screen" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                let sx_var = self.next_var("sx");
                let sy_var = self.next_var("sy");
                lines.push(format!("{}let _wts = world_to_screen({}, {}, {});", indent, x, y, z));
                lines.push(format!("{}let {} = _wts[0];", indent, sx_var));
                lines.push(format!("{}let {} = _wts[1];", indent, sy_var));
                self.output_vars.insert(PinId::new(node.id, "screen_x"), sx_var);
                self.output_vars.insert(PinId::new(node.id, "screen_y"), sy_var);
            }
            "camera/screen_to_world" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let ox_var = self.next_var("ray_ox");
                let oy_var = self.next_var("ray_oy");
                let oz_var = self.next_var("ray_oz");
                let dx_var = self.next_var("ray_dx");
                let dy_var = self.next_var("ray_dy");
                let dz_var = self.next_var("ray_dz");
                lines.push(format!("{}let _stw = screen_to_ray({}, {});", indent, x, y));
                lines.push(format!("{}let {} = _stw[0];", indent, ox_var));
                lines.push(format!("{}let {} = _stw[1];", indent, oy_var));
                lines.push(format!("{}let {} = _stw[2];", indent, oz_var));
                lines.push(format!("{}let {} = _stw[3];", indent, dx_var));
                lines.push(format!("{}let {} = _stw[4];", indent, dy_var));
                lines.push(format!("{}let {} = _stw[5];", indent, dz_var));
                self.output_vars.insert(PinId::new(node.id, "origin_x"), ox_var);
                self.output_vars.insert(PinId::new(node.id, "origin_y"), oy_var);
                self.output_vars.insert(PinId::new(node.id, "origin_z"), oz_var);
                self.output_vars.insert(PinId::new(node.id, "direction_x"), dx_var);
                self.output_vars.insert(PinId::new(node.id, "direction_y"), dy_var);
                self.output_vars.insert(PinId::new(node.id, "direction_z"), dz_var);
            }
            "camera/screen_to_plane" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let plane_y = self.get_input_value(node, "plane_y");
                let wx_var = self.next_var("plane_x");
                let wz_var = self.next_var("plane_z");
                lines.push(format!("{}let _stp = screen_to_world_plane({}, {}, {});", indent, x, y, plane_y));
                lines.push(format!("{}let {} = _stp[0];", indent, wx_var));
                lines.push(format!("{}let {} = _stp[1];", indent, wz_var));
                self.output_vars.insert(PinId::new(node.id, "world_x"), wx_var);
                self.output_vars.insert(PinId::new(node.id, "world_z"), wz_var);
            }
            "camera/get_viewport" => {
                let w_var = self.next_var("vp_w");
                let h_var = self.next_var("vp_h");
                lines.push(format!("{}let {} = window_width;", indent, w_var));
                lines.push(format!("{}let {} = window_height;", indent, h_var));
                self.output_vars.insert(PinId::new(node.id, "width"), w_var);
                self.output_vars.insert(PinId::new(node.id, "height"), h_var);
            }
            "camera/get_fov" => {
                let result_var = self.next_var("fov");
                lines.push(format!("{}let {} = camera_fov;", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "fov"), result_var);
            }
            "camera/get_main" => {
                let result_var = self.next_var("main_cam");
                lines.push(format!("{}let {} = get_main_camera();", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "entity"), result_var);
            }

            // ── State data nodes ─────────────────────────────────────
            "state/get_current" => {
                let result_var = self.next_var("current_state");
                lines.push(format!("{}let {} = current_state;", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "state"), result_var);
            }
            "state/is_in" => {
                let state = self.get_input_value(node, "state");
                let result_var = self.next_var("in_state");
                lines.push(format!("{}let {} = is_in_state({});", indent, result_var, state));
                self.output_vars.insert(PinId::new(node.id, "result"), result_var);
            }
            "state/is_paused" => {
                let result_var = self.next_var("is_paused");
                lines.push(format!("{}let {} = game_paused;", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "paused"), result_var);
            }
            "state/get_global" => {
                let name = self.get_input_value(node, "name");
                let result_var = self.next_var("global_val");
                lines.push(format!("{}let {} = get_global({});", indent, result_var, name));
                self.output_vars.insert(PinId::new(node.id, "value"), result_var);
            }
            "state/has_global" => {
                let name = self.get_input_value(node, "name");
                let result_var = self.next_var("has_global");
                lines.push(format!("{}let {} = has_global({});", indent, result_var, name));
                self.output_vars.insert(PinId::new(node.id, "exists"), result_var);
            }
            "state/has_save" => {
                let slot = self.get_input_value(node, "slot");
                let result_var = self.next_var("has_save");
                lines.push(format!("{}let {} = has_save_data({});", indent, result_var, slot));
                self.output_vars.insert(PinId::new(node.id, "exists"), result_var);
            }
            "state/get_saves" => {
                let result_var = self.next_var("save_slots");
                lines.push(format!("{}let {} = get_save_slots();", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "slots"), result_var);
            }

            // ── Audio data nodes ─────────────────────────────────────
            "audio/is_playing" => {
                let sound = self.get_input_value(node, "handle");
                let result_var = self.next_var("is_playing");
                lines.push(format!("{}let {} = is_playing({});", indent, result_var, sound));
                self.output_vars.insert(PinId::new(node.id, "playing"), result_var);
            }
            "audio/get_position" => {
                let sound = self.get_input_value(node, "handle");
                let result_var = self.next_var("playback_pos");
                lines.push(format!("{}let {} = get_playback_position({});", indent, result_var, sound));
                self.output_vars.insert(PinId::new(node.id, "position"), result_var);
            }

            // ── Animation data nodes ─────────────────────────────────
            "animation/get_time" => {
                let result_var = self.next_var("anim_time");
                lines.push(format!("{}let {} = get_animation_time();", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "time"), result_var);
            }
            "animation/is_playing" => {
                let result_var = self.next_var("anim_playing");
                lines.push(format!("{}let {} = is_animation_playing();", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "playing"), result_var);
            }
            "animation/get_sprite_frame" => {
                let result_var = self.next_var("sprite_frame");
                lines.push(format!("{}let {} = get_sprite_frame();", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "frame"), result_var);
            }

            // ── Time data nodes ──────────────────────────────────────
            "time/unscaled_delta" => {
                let result_var = self.next_var("unscaled_dt");
                lines.push(format!("{}let {} = unscaled_delta;", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "delta"), result_var);
            }
            "time/unscaled_elapsed" => {
                let result_var = self.next_var("unscaled_elapsed");
                lines.push(format!("{}let {} = unscaled_elapsed;", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "elapsed"), result_var);
            }
            "time/timer_running" => {
                let name = self.get_input_value(node, "timer");
                let result_var = self.next_var("timer_running");
                lines.push(format!("{}let {} = is_timer_running({});", indent, result_var, name));
                self.output_vars.insert(PinId::new(node.id, "running"), result_var);
            }
            "time/is_on_cooldown" => {
                let name = self.get_input_value(node, "name");
                let result_var = self.next_var("on_cooldown");
                lines.push(format!("{}let {} = is_on_cooldown({});", indent, result_var, name));
                self.output_vars.insert(PinId::new(node.id, "on_cooldown"), result_var);
            }
            "time/system_time" => {
                let h_var = self.next_var("hour");
                let m_var = self.next_var("minute");
                let s_var = self.next_var("second");
                lines.push(format!("{}let _st = get_system_time();", indent));
                lines.push(format!("{}let {} = _st[0];", indent, h_var));
                lines.push(format!("{}let {} = _st[1];", indent, m_var));
                lines.push(format!("{}let {} = _st[2];", indent, s_var));
                self.output_vars.insert(PinId::new(node.id, "hour"), h_var);
                self.output_vars.insert(PinId::new(node.id, "minute"), m_var);
                self.output_vars.insert(PinId::new(node.id, "second"), s_var);
            }
            "time/system_date" => {
                let y_var = self.next_var("year");
                let m_var = self.next_var("month");
                let d_var = self.next_var("day");
                lines.push(format!("{}let _sd = get_system_date();", indent));
                lines.push(format!("{}let {} = _sd[0];", indent, y_var));
                lines.push(format!("{}let {} = _sd[1];", indent, m_var));
                lines.push(format!("{}let {} = _sd[2];", indent, d_var));
                self.output_vars.insert(PinId::new(node.id, "year"), y_var);
                self.output_vars.insert(PinId::new(node.id, "month"), m_var);
                self.output_vars.insert(PinId::new(node.id, "day"), d_var);
            }
            "time/timestamp" => {
                let result_var = self.next_var("timestamp");
                lines.push(format!("{}let {} = get_timestamp();", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "timestamp"), result_var);
            }

            // ── Window data nodes ────────────────────────────────────
            "window/get_size" | "window/get_window_size" => {
                let w_var = self.next_var("win_w");
                let h_var = self.next_var("win_h");
                lines.push(format!("{}let {} = window_width;", indent, w_var));
                lines.push(format!("{}let {} = window_height;", indent, h_var));
                self.output_vars.insert(PinId::new(node.id, "width"), w_var);
                self.output_vars.insert(PinId::new(node.id, "height"), h_var);
            }
            "window/get_position" | "window/get_window_position" => {
                let x_var = self.next_var("win_x");
                let y_var = self.next_var("win_y");
                lines.push(format!("{}let {} = window_x;", indent, x_var));
                lines.push(format!("{}let {} = window_y;", indent, y_var));
                self.output_vars.insert(PinId::new(node.id, "x"), x_var);
                self.output_vars.insert(PinId::new(node.id, "y"), y_var);
            }
            "window/get_title" | "window/get_window_title" => {
                let result_var = self.next_var("win_title");
                lines.push(format!("{}let {} = window_title;", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "title"), result_var);
            }
            "window/is_fullscreen" => {
                let result_var = self.next_var("is_fs");
                lines.push(format!("{}let {} = is_fullscreen;", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "fullscreen"), result_var);
            }
            "window/is_minimized" => {
                let result_var = self.next_var("is_min");
                lines.push(format!("{}let {} = is_minimized;", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "minimized"), result_var);
            }
            "window/is_maximized" => {
                let result_var = self.next_var("is_max");
                lines.push(format!("{}let {} = is_maximized;", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "maximized"), result_var);
            }
            "window/get_cursor" | "window/get_cursor_position" => {
                let x_var = self.next_var("cursor_x");
                let y_var = self.next_var("cursor_y");
                lines.push(format!("{}let {} = cursor_x;", indent, x_var));
                lines.push(format!("{}let {} = cursor_y;", indent, y_var));
                self.output_vars.insert(PinId::new(node.id, "x"), x_var);
                self.output_vars.insert(PinId::new(node.id, "y"), y_var);
            }
            "window/get_monitor_size" => {
                let w_var = self.next_var("mon_w");
                let h_var = self.next_var("mon_h");
                lines.push(format!("{}let _ms = get_monitor_size();", indent));
                lines.push(format!("{}let {} = _ms[0];", indent, w_var));
                lines.push(format!("{}let {} = _ms[1];", indent, h_var));
                self.output_vars.insert(PinId::new(node.id, "width"), w_var);
                self.output_vars.insert(PinId::new(node.id, "height"), h_var);
            }
            "window/get_monitor_count" => {
                let result_var = self.next_var("mon_count");
                lines.push(format!("{}let {} = get_monitor_count();", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "count"), result_var);
            }
            "window/get_scale_factor" => {
                let result_var = self.next_var("scale_factor");
                lines.push(format!("{}let {} = scale_factor;", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "factor"), result_var);
            }
            "window/is_focused" | "window/is_window_focused" => {
                let result_var = self.next_var("is_focused");
                lines.push(format!("{}let {} = is_focused;", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "focused"), result_var);
            }
            "window/is_vsync" | "window/is_vsync_enabled" => {
                let result_var = self.next_var("vsync");
                lines.push(format!("{}let {} = vsync_enabled;", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "enabled"), result_var);
            }

            // ── Rendering data nodes ─────────────────────────────────
            "rendering/get_visibility" => {
                let entity = self.get_input_value(node, "entity");
                let result_var = self.next_var("visible");
                lines.push(format!("{}let {} = is_visible({});", indent, result_var, entity));
                self.output_vars.insert(PinId::new(node.id, "visible"), result_var);
            }

            // ── ECS data nodes ───────────────────────────────────────
            "ecs/has_component" => {
                let entity = self.get_input_value(node, "entity");
                let name = self.get_input_value(node, "name");
                let result_var = self.next_var("has_comp");
                lines.push(format!("{}let {} = has_component({}, {});", indent, result_var, entity, name));
                self.output_vars.insert(PinId::new(node.id, "has"), result_var);
            }
            "ecs/get_all_entities" => {
                let result_var = self.next_var("all_entities");
                lines.push(format!("{}let {} = get_all_entities();", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "entities"), result_var);
            }
            "ecs/get_closest" | "ecs/get_closest_entity" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                let tag = self.get_input_value(node, "tag");
                let result_var = self.next_var("closest");
                lines.push(format!("{}let {} = get_closest_entity({}, {}, {}, {});", indent, result_var, x, y, z, tag));
                self.output_vars.insert(PinId::new(node.id, "entity"), result_var);
            }
            "ecs/get_in_radius" | "ecs/get_entities_in_radius" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                let radius = self.get_input_value(node, "radius");
                let result_var = self.next_var("in_radius");
                lines.push(format!("{}let {} = get_entities_in_radius({}, {}, {}, {});", indent, result_var, x, y, z, radius));
                self.output_vars.insert(PinId::new(node.id, "entities"), result_var);
            }

            // ── UI data nodes ────────────────────────────────────────
            "ui/get_text" => {
                let entity = self.get_input_value(node, "entity");
                let result_var = self.next_var("ui_text");
                lines.push(format!("{}let {} = get_text({});", indent, result_var, entity));
                self.output_vars.insert(PinId::new(node.id, "text"), result_var);
            }
            "ui/get_size" | "ui/get_ui_size" => {
                let entity = self.get_input_value(node, "entity");
                let w_var = self.next_var("ui_w");
                let h_var = self.next_var("ui_h");
                lines.push(format!("{}let _us = get_ui_size({});", indent, entity));
                lines.push(format!("{}let {} = _us[0];", indent, w_var));
                lines.push(format!("{}let {} = _us[1];", indent, h_var));
                self.output_vars.insert(PinId::new(node.id, "width"), w_var);
                self.output_vars.insert(PinId::new(node.id, "height"), h_var);
            }
            "ui/get_input_value" => {
                let entity = self.get_input_value(node, "entity");
                let result_var = self.next_var("input_val");
                lines.push(format!("{}let {} = get_text_input_value({});", indent, result_var, entity));
                self.output_vars.insert(PinId::new(node.id, "value"), result_var);
            }
            "ui/get_slider_value" => {
                let entity = self.get_input_value(node, "entity");
                let result_var = self.next_var("slider_val");
                lines.push(format!("{}let {} = get_slider_value({});", indent, result_var, entity));
                self.output_vars.insert(PinId::new(node.id, "value"), result_var);
            }

            // ── Scene data nodes ─────────────────────────────────────
            "scene/get_current" => {
                let result_var = self.next_var("current_scene");
                lines.push(format!("{}let {} = get_current_scene();", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "scene"), result_var);
            }
            "scene/is_loaded" => {
                let handle = self.get_input_value(node, "handle");
                let result_var = self.next_var("scene_loaded");
                lines.push(format!("{}let {} = is_scene_loaded({});", indent, result_var, handle));
                self.output_vars.insert(PinId::new(node.id, "loaded"), result_var);
            }
            "scene/gltf_scene_count" => {
                let handle = self.get_input_value(node, "handle");
                let result_var = self.next_var("gltf_count");
                lines.push(format!("{}let {} = get_gltf_scene_count({});", indent, result_var, handle));
                self.output_vars.insert(PinId::new(node.id, "count"), result_var);
            }

            // ── Component getter nodes (auto-generated) ──────────────
            "component/get_property" => {
                let name = self.get_input_value(node, "name");
                let prop = self.get_input_value(node, "property");
                let ent_var = self.next_var("ent");
                let val_var = self.next_var("prop_val");
                lines.push(format!("{}let {} = entity({});", indent, ent_var, name));
                lines.push(format!("{}let {} = {}.get({});", indent, val_var, ent_var, prop));
                self.output_vars.insert(PinId::new(node.id, "value"), val_var);
            }

            _ if node.node_type.starts_with("component/get_") => {
                let name = self.get_input_value(node, "name");
                let ent_var = self.next_var("ent");
                lines.push(format!("{}let {} = entity({});", indent, ent_var, name));
                for pin in node.output_pins() {
                    let var = self.next_var(&pin.name);
                    lines.push(format!("{}let {} = {}.{};", indent, var, ent_var, pin.name));
                    self.output_vars.insert(PinId::new(node.id, &pin.name), var);
                }
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
            "camera/zoom" | "camera/set_zoom" => {
                let zoom = self.get_input_value(node, "zoom");
                lines.push(format!("{}set_camera_zoom({});", indent, zoom));
            }
            "camera/shake" | "camera/screen_shake" => {
                let intensity = self.get_input_value(node, "intensity");
                let duration = self.get_input_value(node, "duration");
                lines.push(format!("{}screen_shake({}, {});", indent, intensity, duration));
            }
            "camera/follow" => {
                let target = self.get_input_value(node, "target");
                let offset = self.get_input_value(node, "offset");
                let smooth = self.get_input_value(node, "smooth");
                let ofs_var = self.next_var("ofs");
                lines.push(format!("{}let {} = {};", indent, ofs_var, offset));
                lines.push(format!("{}camera_follow({}, {}[0], {}[1], {}[2], {});", indent, target, ofs_var, ofs_var, ofs_var, smooth));
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
            "scene/instantiate" | "scene/spawn_prefab" => {
                let path = self.get_input_value(node, "path");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                lines.push(format!("{}spawn_prefab({}, {}, {}, {});", indent, path, x, y, z));
            }
            "scene/instantiate_at" | "scene/spawn_prefab_rotated" => {
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
            "debug/log" | "debug/log_message" | "debug/print" => {
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
            "debug/line" | "debug/debug_line" => {
                let sx = self.get_input_value(node, "start_x");
                let sy = self.get_input_value(node, "start_y");
                let sz = self.get_input_value(node, "start_z");
                let ex = self.get_input_value(node, "end_x");
                let ey = self.get_input_value(node, "end_y");
                let ez = self.get_input_value(node, "end_z");
                lines.push(format!("{}draw_line({}, {}, {}, {}, {}, {});", indent, sx, sy, sz, ex, ey, ez));
            }
            "debug/sphere" | "debug/debug_sphere" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                let radius = self.get_input_value(node, "radius");
                lines.push(format!("{}draw_sphere({}, {}, {}, {});", indent, x, y, z, radius));
            }
            "debug/box" | "debug/debug_box" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                let hx = self.get_input_value(node, "half_x");
                let hy = self.get_input_value(node, "half_y");
                let hz = self.get_input_value(node, "half_z");
                lines.push(format!("{}draw_box({}, {}, {}, {}, {}, {});", indent, x, y, z, hx, hy, hz));
            }
            "debug/point" | "debug/debug_point" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                let size = self.get_input_value(node, "size");
                lines.push(format!("{}draw_point({}, {}, {}, {});", indent, x, y, z, size));
            }
            "debug/ray" | "debug/debug_ray" => {
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
            "time/every_seconds" | "time/every_n_seconds" => {
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
            "time/every_frames" | "time/every_n_frames" => {
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
            "state/set" | "state/set_state" => {
                let state = self.get_input_value(node, "state");
                lines.push(format!("{}set_game_state({});", indent, state));
            }
            "state/push" | "state/push_state" => {
                let state = self.get_input_value(node, "state");
                lines.push(format!("{}push_game_state({});", indent, state));
            }
            "state/pop" | "state/pop_state" => {
                lines.push(format!("{}pop_game_state();", indent));
            }
            "state/pause" | "state/pause_game" => {
                lines.push(format!("{}pause_game();", indent));
            }
            "state/resume" | "state/resume_game" => {
                lines.push(format!("{}resume_game();", indent));
            }
            "state/toggle_pause" => {
                lines.push(format!("{}toggle_pause();", indent));
            }
            "state/quit" | "state/quit_game" => {
                lines.push(format!("{}quit_game();", indent));
            }
            "state/restart" | "state/restart_game" => {
                lines.push(format!("{}restart_game();", indent));
            }
            "state/set_global" | "state/set_global_var" => {
                let name = self.get_input_value(node, "name");
                let value = self.get_input_value(node, "value");
                lines.push(format!("{}set_global({}, {});", indent, name, value));
            }
            "state/save_data" | "state/save_game_data" => {
                let slot = self.get_input_value(node, "slot");
                let data = self.get_input_value(node, "data");
                lines.push(format!("{}save_game({}, {});", indent, slot, data));
            }
            "state/load_data" | "state/load_game_data" => {
                let slot = self.get_input_value(node, "slot");
                let result_var = self.next_var("save_data");
                lines.push(format!("{}let {} = load_game({});", indent, result_var, slot));
                self.output_vars.insert(PinId::new(node.id, "data"), result_var);
            }
            "state/delete_save" | "state/delete_save_data" => {
                let slot = self.get_input_value(node, "slot");
                lines.push(format!("{}delete_save({});", indent, slot));
            }

            // Window actions
            "window/set_size" | "window/set_window_size" => {
                let width = self.get_input_value(node, "width");
                let height = self.get_input_value(node, "height");
                lines.push(format!("{}set_window_size({}, {});", indent, width, height));
            }
            "window/set_position" | "window/set_window_position" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                lines.push(format!("{}set_window_position({}, {});", indent, x, y));
            }
            "window/center" | "window/center_window" => {
                lines.push(format!("{}center_window();", indent));
            }
            "window/set_title" | "window/set_window_title" => {
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
            "window/minimize" | "window/minimize_window" => {
                lines.push(format!("{}minimize_window();", indent));
            }
            "window/maximize" | "window/maximize_window" => {
                lines.push(format!("{}maximize_window();", indent));
            }
            "window/restore" | "window/restore_window" => {
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
            "window/set_cursor" | "window/set_cursor_position" => {
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
            "ui/spawn_image" | "ui/spawn_ui_image" => {
                let image = self.get_input_value(node, "image");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let entity_var = self.next_var("image_entity");
                lines.push(format!("{}let {} = spawn_ui_image({}, {}, {});", indent, entity_var, image, x, y));
                self.output_vars.insert(PinId::new(node.id, "entity"), entity_var);
            }
            "ui/set_image" | "ui/set_ui_image" => {
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
            "ui/spawn_node" | "ui/spawn_ui_node" => {
                let entity_var = self.next_var("ui_node");
                lines.push(format!("{}let {} = spawn_ui_node();", indent, entity_var));
                self.output_vars.insert(PinId::new(node.id, "entity"), entity_var);
            }
            "ui/set_position" | "ui/set_ui_position" => {
                let entity = self.get_input_value(node, "entity");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                lines.push(format!("{}set_ui_position({}, {}, {});", indent, entity, x, y));
            }
            "ui/set_size" | "ui/set_ui_size" => {
                let entity = self.get_input_value(node, "entity");
                let width = self.get_input_value(node, "width");
                let height = self.get_input_value(node, "height");
                lines.push(format!("{}set_ui_size({}, {}, {});", indent, entity, width, height));
            }
            "ui/set_background" | "ui/set_background_color" => {
                let entity = self.get_input_value(node, "entity");
                let r = self.get_input_value(node, "r");
                let g = self.get_input_value(node, "g");
                let b = self.get_input_value(node, "b");
                let a = self.get_input_value(node, "a");
                lines.push(format!("{}set_background_color({}, {}, {}, {}, {});", indent, entity, r, g, b, a));
            }
            "ui/set_visibility" | "ui/set_ui_visibility" => {
                let entity = self.get_input_value(node, "entity");
                let visible = self.get_input_value(node, "visible");
                lines.push(format!("{}set_ui_visibility({}, {});", indent, entity, visible));
            }
            "ui/toggle_visibility" | "ui/toggle_ui_visibility" => {
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
            "ui/add_child" | "ui/add_ui_child" => {
                let parent = self.get_input_value(node, "parent");
                let child = self.get_input_value(node, "child");
                lines.push(format!("{}add_ui_child({}, {});", indent, parent, child));
            }
            "ui/remove_child" | "ui/remove_ui_child" => {
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
            "animation/play_once" | "animation/play_animation_once" => {
                let name = self.get_input_value(node, "name");
                let speed = self.get_input_value(node, "speed");
                lines.push(format!("{}play_animation({}, false, {});", indent, name, speed));
            }
            "animation/pause" | "animation/pause_animation" => {
                lines.push(format!("{}pause_animation();", indent));
            }
            "animation/resume" | "animation/resume_animation" => {
                lines.push(format!("{}resume_animation();", indent));
            }
            "animation/set_time" | "animation/set_animation_time" => {
                let time = self.get_input_value(node, "time");
                lines.push(format!("{}set_animation_time({});", indent, time));
            }
            "animation/crossfade" | "animation/crossfade_animation" => {
                let from = self.get_input_value(node, "from");
                let to = self.get_input_value(node, "to");
                let duration = self.get_input_value(node, "duration");
                lines.push(format!("{}crossfade_animation({}, {}, {});", indent, from, to, duration));
            }
            "animation/set_weight" | "animation/set_animation_weight" => {
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
            "animation/play_sprite" | "animation/play_sprite_animation" => {
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
            "rendering/set_ambient" | "rendering/set_ambient_light" => {
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
            "camera/orbit" | "camera/camera_orbit" => {
                let target_x = self.get_input_value(node, "target_x");
                let target_y = self.get_input_value(node, "target_y");
                let target_z = self.get_input_value(node, "target_z");
                let distance = self.get_input_value(node, "distance");
                lines.push(format!("{}camera_orbit({}, {}, {}, {});", indent, target_x, target_y, target_z, distance));
            }

            // Debug toggles
            "debug/toggle_physics" | "debug/toggle_physics_debug" => {
                lines.push(format!("{}toggle_physics_debug();", indent));
            }
            "debug/toggle_wireframe" => {
                lines.push(format!("{}toggle_wireframe();", indent));
            }
            "debug/toggle_aabb" | "debug/toggle_bounding_boxes" => {
                lines.push(format!("{}toggle_bounding_boxes();", indent));
            }
            "debug/clear" | "debug/clear_debug_draws" => {
                lines.push(format!("{}clear_debug_draws();", indent));
            }
            "debug/log_value" => {
                let label = self.get_input_value(node, "label");
                let value = self.get_input_value(node, "value");
                lines.push(format!("{}log(\"\" + {} + \": \" + {});", indent, label, value));
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
            "debug/arrow" | "debug/debug_arrow" => {
                let sx = self.get_input_value(node, "start_x");
                let sy = self.get_input_value(node, "start_y");
                let sz = self.get_input_value(node, "start_z");
                let dx = self.get_input_value(node, "direction_x");
                let dy = self.get_input_value(node, "direction_y");
                let dz = self.get_input_value(node, "direction_z");
                let length = self.get_input_value(node, "length");
                lines.push(format!("{}draw_arrow({}, {}, {}, {}, {}, {}, {});", indent, sx, sy, sz, dx, dy, dz, length));
            }
            "debug/axes" | "debug/debug_axes" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                let size = self.get_input_value(node, "size");
                lines.push(format!("{}draw_axes({}, {}, {}, {});", indent, x, y, z, size));
            }
            "debug/capsule" | "debug/debug_capsule" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                let radius = self.get_input_value(node, "radius");
                let half_height = self.get_input_value(node, "half_height");
                lines.push(format!("{}draw_capsule({}, {}, {}, {}, {});", indent, x, y, z, radius, half_height));
            }
            "debug/text_3d" | "debug/debug_text_3d" => {
                let text = self.get_input_value(node, "text");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                lines.push(format!("{}draw_text_3d({}, {}, {}, {});", indent, text, x, y, z));
            }
            "debug/text_2d" | "debug/debug_text_2d" => {
                let text = self.get_input_value(node, "text");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                lines.push(format!("{}draw_text_2d({}, {}, {});", indent, text, x, y));
            }

            // ── Hierarchy flow nodes ──────────────────────────────────
            "hierarchy/set_local_position" => {
                let entity = self.get_input_value(node, "entity");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                lines.push(format!("{}set_local_position({}, {}, {}, {});", indent, entity, x, y, z));
            }
            "hierarchy/set_local_rotation" => {
                let entity = self.get_input_value(node, "entity");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                lines.push(format!("{}set_local_rotation({}, {}, {}, {});", indent, entity, x, y, z));
            }
            "hierarchy/set_local_scale" => {
                let entity = self.get_input_value(node, "entity");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                lines.push(format!("{}set_local_scale({}, {}, {}, {});", indent, entity, x, y, z));
            }
            "hierarchy/for_each_child" => {
                let entity = self.get_input_value(node, "entity");
                let child_var = self.next_var("child");
                lines.push(format!("{}let _children = get_children({});", indent, entity));
                lines.push(format!("{}for {} in _children {{", indent, child_var));
                self.output_vars.insert(PinId::new(node.id, "child"), child_var.clone());
                self.indent += 1;
                let body_lines = self.follow_flow_from(node.id, "body");
                lines.extend(body_lines);
                self.indent -= 1;
                lines.push(format!("{}}}", indent));
                let after_lines = self.follow_flow_from(node.id, "exec");
                lines.extend(after_lines);
                return lines;
            }

            // ── Camera flow nodes ────────────────────────────────────
            "camera/set_main" => {
                let entity = self.get_input_value(node, "entity");
                lines.push(format!("{}set_main_camera({});", indent, entity));
            }
            "camera/set_active" => {
                let entity = self.get_input_value(node, "entity");
                let active = self.get_input_value(node, "active");
                lines.push(format!("{}set_camera_active({}, {});", indent, entity, active));
            }
            "camera/set_order" => {
                let entity = self.get_input_value(node, "entity");
                let order = self.get_input_value(node, "order");
                lines.push(format!("{}set_camera_order({}, {});", indent, entity, order));
            }

            // ── Rendering flow nodes ─────────────────────────────────
            "rendering/toggle_visibility" => {
                let entity = self.get_input_value(node, "entity");
                lines.push(format!("{}toggle_visibility({});", indent, entity));
            }
            "rendering/spawn_mesh" => {
                let name = self.get_input_value(node, "name");
                let path = self.get_input_value(node, "path");
                let entity_var = self.next_var("mesh_entity");
                lines.push(format!("{}let {} = spawn_mesh({}, {});", indent, entity_var, name, path));
                self.output_vars.insert(PinId::new(node.id, "entity"), entity_var);
            }
            "rendering/set_mesh" => {
                let entity = self.get_input_value(node, "entity");
                let path = self.get_input_value(node, "path");
                lines.push(format!("{}set_mesh({}, {});", indent, entity, path));
            }
            "rendering/set_material" => {
                let entity = self.get_input_value(node, "entity");
                let path = self.get_input_value(node, "path");
                lines.push(format!("{}set_material({}, {});", indent, entity, path));
            }
            "rendering/set_emissive" => {
                let entity = self.get_input_value(node, "entity");
                let r = self.get_input_value(node, "r");
                let g = self.get_input_value(node, "g");
                let b = self.get_input_value(node, "b");
                lines.push(format!("{}set_material_emissive({}, {}, {}, {});", indent, entity, r, g, b));
            }
            "rendering/set_pbr" | "rendering/set_pbr_properties" => {
                let entity = self.get_input_value(node, "entity");
                let roughness = self.get_input_value(node, "roughness");
                let metallic = self.get_input_value(node, "metallic");
                lines.push(format!("{}set_pbr({}, {}, {});", indent, entity, roughness, metallic));
            }
            "rendering/set_texture" => {
                let entity = self.get_input_value(node, "entity");
                let path = self.get_input_value(node, "path");
                lines.push(format!("{}set_texture({}, {});", indent, entity, path));
            }
            "rendering/set_skybox" => {
                let path = self.get_input_value(node, "path");
                lines.push(format!("{}set_skybox({});", indent, path));
            }

            // ── Environment flow nodes (new) ─────────────────────────
            "rendering/set_sun_angles" => {
                let azimuth = self.get_input_value(node, "azimuth");
                let elevation = self.get_input_value(node, "elevation");
                lines.push(format!("{}set_sun_angles({}, {});", indent, azimuth, elevation));
            }
            "rendering/set_sun_direction" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                lines.push(format!("{}set_sun_direction({}, {}, {});", indent, x, y, z));
            }
            "rendering/set_ambient_brightness" => {
                let brightness = self.get_input_value(node, "brightness");
                lines.push(format!("{}set_ambient_brightness({});", indent, brightness));
            }
            "rendering/set_ambient_color" => {
                let r = self.get_input_value(node, "r");
                let g = self.get_input_value(node, "g");
                let b = self.get_input_value(node, "b");
                lines.push(format!("{}set_ambient_color({}, {}, {});", indent, r, g, b));
            }
            "rendering/set_sky_top_color" => {
                let r = self.get_input_value(node, "r");
                let g = self.get_input_value(node, "g");
                let b = self.get_input_value(node, "b");
                lines.push(format!("{}set_sky_top_color({}, {}, {});", indent, r, g, b));
            }
            "rendering/set_sky_horizon_color" => {
                let r = self.get_input_value(node, "r");
                let g = self.get_input_value(node, "g");
                let b = self.get_input_value(node, "b");
                lines.push(format!("{}set_sky_horizon_color({}, {}, {});", indent, r, g, b));
            }
            "rendering/set_fog_color" => {
                let r = self.get_input_value(node, "r");
                let g = self.get_input_value(node, "g");
                let b = self.get_input_value(node, "b");
                lines.push(format!("{}set_fog_color({}, {}, {});", indent, r, g, b));
            }
            "rendering/set_exposure" => {
                let exposure = self.get_input_value(node, "exposure");
                lines.push(format!("{}set_exposure({});", indent, exposure));
            }

            // ── Transform extended flow nodes (new) ──────────────────
            "transform/set_scale_uniform" => {
                let scale = self.get_input_value(node, "scale");
                lines.push(format!("{}set_scale({}, {}, {});", indent, scale, scale, scale));
            }
            "transform/parent_set_position" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                lines.push(format!("{}parent_set_position({}, {}, {});", indent, x, y, z));
            }
            "transform/parent_set_rotation" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                lines.push(format!("{}parent_set_rotation({}, {}, {});", indent, x, y, z));
            }
            "transform/parent_translate" => {
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                lines.push(format!("{}parent_translate({}, {}, {});", indent, x, y, z));
            }
            "transform/set_child_position" => {
                let name = self.get_input_value(node, "name");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                lines.push(format!("{}set_child_position({}, {}, {}, {});", indent, name, x, y, z));
            }
            "transform/set_child_rotation" => {
                let name = self.get_input_value(node, "name");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                lines.push(format!("{}set_child_rotation({}, {}, {}, {});", indent, name, x, y, z));
            }
            "transform/child_translate" => {
                let name = self.get_input_value(node, "name");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                lines.push(format!("{}child_translate({}, {}, {}, {});", indent, name, x, y, z));
            }

            // ── Particle flow nodes (new) ────────────────────────────
            "particles/burst" => {
                let entity = self.get_input_value(node, "entity");
                let count = self.get_input_value(node, "count");
                lines.push(format!("{}particle_burst({}, {});", indent, entity, count));
            }
            "particles/set_rate" => {
                let entity = self.get_input_value(node, "entity");
                let rate = self.get_input_value(node, "rate");
                lines.push(format!("{}set_particle_rate({}, {});", indent, entity, rate));
            }
            "particles/set_scale" => {
                let entity = self.get_input_value(node, "entity");
                let scale = self.get_input_value(node, "scale");
                lines.push(format!("{}set_particle_scale({}, {});", indent, entity, scale));
            }
            "particles/set_time_scale" => {
                let entity = self.get_input_value(node, "entity");
                let time_scale = self.get_input_value(node, "time_scale");
                lines.push(format!("{}set_particle_time_scale({}, {});", indent, entity, time_scale));
            }
            "particles/set_tint" => {
                let entity = self.get_input_value(node, "entity");
                let r = self.get_input_value(node, "r");
                let g = self.get_input_value(node, "g");
                let b = self.get_input_value(node, "b");
                let a = self.get_input_value(node, "a");
                lines.push(format!("{}set_particle_tint({}, {}, {}, {}, {});", indent, entity, r, g, b, a));
            }
            "particles/reset" => {
                let entity = self.get_input_value(node, "entity");
                lines.push(format!("{}reset_particles({});", indent, entity));
            }
            "particles/set_variable_float" => {
                let entity = self.get_input_value(node, "entity");
                let name = self.get_input_value(node, "name");
                let value = self.get_input_value(node, "value");
                lines.push(format!("{}set_particle_float({}, {}, {});", indent, entity, name, value));
            }
            "particles/set_variable_color" => {
                let entity = self.get_input_value(node, "entity");
                let name = self.get_input_value(node, "name");
                let r = self.get_input_value(node, "r");
                let g = self.get_input_value(node, "g");
                let b = self.get_input_value(node, "b");
                let a = self.get_input_value(node, "a");
                lines.push(format!("{}set_particle_color({}, {}, {}, {}, {}, {});", indent, entity, name, r, g, b, a));
            }
            "particles/set_variable_vec3" => {
                let entity = self.get_input_value(node, "entity");
                let name = self.get_input_value(node, "name");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                lines.push(format!("{}set_particle_vec3({}, {}, {}, {}, {});", indent, entity, name, x, y, z));
            }
            "particles/emit_at" => {
                let entity = self.get_input_value(node, "entity");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                lines.push(format!("{}emit_particle_at({}, {}, {}, {});", indent, entity, x, y, z));
            }
            "particles/emit_at_count" => {
                let entity = self.get_input_value(node, "entity");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let z = self.get_input_value(node, "z");
                let count = self.get_input_value(node, "count");
                lines.push(format!("{}emit_particles_at({}, {}, {}, {}, {});", indent, entity, x, y, z, count));
            }

            // ── Audio extended flow nodes (new) ──────────────────────
            "audio/play_sound_looping" => {
                let path = self.get_input_value(node, "path");
                let volume = self.get_input_value(node, "volume");
                let result_var = self.next_var("loop_sound");
                lines.push(format!("{}let {} = play_sound_looping({}, {});", indent, result_var, path, volume));
                self.output_vars.insert(PinId::new(node.id, "handle"), result_var);
            }
            "audio/play_music_fade" => {
                let path = self.get_input_value(node, "path");
                let volume = self.get_input_value(node, "volume");
                let fade = self.get_input_value(node, "fade_duration");
                lines.push(format!("{}play_music_fade({}, {}, {});", indent, path, volume, fade));
            }
            "audio/stop_music_fade" => {
                let fade = self.get_input_value(node, "fade_duration");
                lines.push(format!("{}stop_music_fade({});", indent, fade));
            }

            // ── Audio flow nodes ─────────────────────────────────────
            "audio/play_sound_attached" => {
                let path = self.get_input_value(node, "path");
                let entity = self.get_input_value(node, "entity");
                let volume = self.get_input_value(node, "volume");
                let result_var = self.next_var("sound");
                lines.push(format!("{}let {} = play_sound_attached({}, {}, {});", indent, result_var, path, entity, volume));
                self.output_vars.insert(PinId::new(node.id, "handle"), result_var);
            }
            "audio/stop_sound" => {
                let handle = self.get_input_value(node, "handle");
                lines.push(format!("{}stop_sound({});", indent, handle));
            }
            "audio/pause_sound" => {
                let handle = self.get_input_value(node, "handle");
                lines.push(format!("{}pause_sound({});", indent, handle));
            }
            "audio/resume_sound" => {
                let handle = self.get_input_value(node, "handle");
                lines.push(format!("{}resume_sound({});", indent, handle));
            }
            "audio/crossfade_music" => {
                let path = self.get_input_value(node, "path");
                let duration = self.get_input_value(node, "duration");
                lines.push(format!("{}crossfade_music({}, {});", indent, path, duration));
            }
            "audio/set_pitch" => {
                let handle = self.get_input_value(node, "handle");
                let pitch = self.get_input_value(node, "pitch");
                lines.push(format!("{}set_pitch({}, {});", indent, handle, pitch));
            }
            "audio/set_panning" => {
                let handle = self.get_input_value(node, "handle");
                let pan = self.get_input_value(node, "pan");
                lines.push(format!("{}set_panning({}, {});", indent, handle, pan));
            }
            "audio/set_position" | "audio/set_playback_position" => {
                let handle = self.get_input_value(node, "handle");
                let position = self.get_input_value(node, "position");
                lines.push(format!("{}set_playback_position({}, {});", indent, handle, position));
            }
            "audio/set_listener" | "audio/set_audio_listener" => {
                let entity = self.get_input_value(node, "entity");
                lines.push(format!("{}set_audio_listener({});", indent, entity));
            }
            "audio/set_spatial" | "audio/set_spatial_properties" => {
                let entity = self.get_input_value(node, "entity");
                let min_dist = self.get_input_value(node, "min_distance");
                let max_dist = self.get_input_value(node, "max_distance");
                lines.push(format!("{}set_spatial_properties({}, {}, {});", indent, entity, min_dist, max_dist));
            }

            // ── ECS flow nodes ───────────────────────────────────────
            "ecs/set_name" | "ecs/set_entity_name" => {
                let entity = self.get_input_value(node, "entity");
                let name = self.get_input_value(node, "name");
                lines.push(format!("{}set_entity_name({}, {});", indent, entity, name));
            }
            "ecs/add_component" => {
                let entity = self.get_input_value(node, "entity");
                let name = self.get_input_value(node, "name");
                lines.push(format!("{}add_component({}, {});", indent, entity, name));
            }
            "ecs/remove_component" => {
                let entity = self.get_input_value(node, "entity");
                let name = self.get_input_value(node, "name");
                lines.push(format!("{}remove_component({}, {});", indent, entity, name));
            }
            "ecs/for_each_entity" => {
                let tag = self.get_input_value(node, "tag");
                let entity_var = self.next_var("ent");
                lines.push(format!("{}let _ents = find_entities_by_tag({});", indent, tag));
                lines.push(format!("{}for {} in _ents {{", indent, entity_var));
                self.output_vars.insert(PinId::new(node.id, "entity"), entity_var.clone());
                self.indent += 1;
                let body_lines = self.follow_flow_from(node.id, "body");
                lines.extend(body_lines);
                self.indent -= 1;
                lines.push(format!("{}}}", indent));
                let after_lines = self.follow_flow_from(node.id, "exec");
                lines.extend(after_lines);
                return lines;
            }

            // ── Physics flow nodes ───────────────────────────────────
            "physics/add_mesh_collider" => {
                let entity = self.get_input_value(node, "entity");
                lines.push(format!("{}add_mesh_collider({});", indent, entity));
            }

            // ── Animation flow nodes ─────────────────────────────────
            "animation/tween_float" => {
                let property = self.get_input_value(node, "property");
                let target = self.get_input_value(node, "target");
                let duration = self.get_input_value(node, "duration");
                let easing = self.get_input_value(node, "easing");
                lines.push(format!("{}tween_to({}, {}, {}, {});", indent, property, target, duration, easing));
            }
            "animation/tween_color" => {
                let r = self.get_input_value(node, "r");
                let g = self.get_input_value(node, "g");
                let b = self.get_input_value(node, "b");
                let a = self.get_input_value(node, "a");
                let duration = self.get_input_value(node, "duration");
                let easing = self.get_input_value(node, "easing");
                lines.push(format!("{}tween_color({}, {}, {}, {}, {}, {});", indent, r, g, b, a, duration, easing));
            }

            // ── Time flow nodes ──────────────────────────────────────
            "time/set_scale" => {
                let scale = self.get_input_value(node, "scale");
                lines.push(format!("{}set_time_scale({});", indent, scale));
            }
            "time/create_timer" => {
                let name = self.get_input_value(node, "name");
                let duration = self.get_input_value(node, "duration");
                let repeating = self.get_input_value(node, "repeating");
                lines.push(format!("{}create_timer({}, {}, {});", indent, name, duration, repeating));
            }
            "time/reset_timer" => {
                let name = self.get_input_value(node, "name");
                lines.push(format!("{}reset_timer({});", indent, name));
            }
            "time/delay_frames" => {
                let count = self.get_input_value(node, "count");
                lines.push(format!("{}delay_frames({});", indent, count));
            }
            "time/retrigger_delay" | "time/retriggerable_delay" => {
                let name = self.get_input_value(node, "name");
                let duration = self.get_input_value(node, "duration");
                lines.push(format!("{}retriggerable_delay({}, {});", indent, name, duration));
            }

            // ── State flow nodes ─────────────────────────────────────
            "state/remove_global" | "state/remove_global_var" => {
                let name = self.get_input_value(node, "name");
                lines.push(format!("{}remove_global({});", indent, name));
            }

            // ── UI flow nodes ────────────────────────────────────────
            "ui/set_border" | "ui/set_ui_border" => {
                let entity = self.get_input_value(node, "entity");
                let width = self.get_input_value(node, "width");
                lines.push(format!("{}set_ui_border({}, {});", indent, entity, width));
            }
            "ui/set_border_radius" => {
                let entity = self.get_input_value(node, "entity");
                let radius = self.get_input_value(node, "radius");
                lines.push(format!("{}set_border_radius({}, {});", indent, entity, radius));
            }
            "ui/spawn_text_input" => {
                let text = self.get_input_value(node, "text");
                let x = self.get_input_value(node, "x");
                let y = self.get_input_value(node, "y");
                let entity_var = self.next_var("input_entity");
                lines.push(format!("{}let {} = spawn_text_input({}, {}, {});", indent, entity_var, text, x, y));
                self.output_vars.insert(PinId::new(node.id, "entity"), entity_var);
            }
            "ui/set_input_value" | "ui/set_text_input_value" => {
                let entity = self.get_input_value(node, "entity");
                let text = self.get_input_value(node, "value");
                lines.push(format!("{}set_text_input_value({}, {});", indent, entity, text));
            }
            "ui/spawn_slider" => {
                let min = self.get_input_value(node, "min");
                let max = self.get_input_value(node, "max");
                let default = self.get_input_value(node, "default");
                let entity_var = self.next_var("slider_entity");
                lines.push(format!("{}let {} = spawn_slider({}, {}, {});", indent, entity_var, min, max, default));
                self.output_vars.insert(PinId::new(node.id, "entity"), entity_var);
            }
            "ui/set_slider_value" => {
                let entity = self.get_input_value(node, "entity");
                let value = self.get_input_value(node, "value");
                lines.push(format!("{}set_slider_value({}, {});", indent, entity, value));
            }

            // ── Scene flow nodes ─────────────────────────────────────
            "scene/load_async" => {
                let path = self.get_input_value(node, "path");
                let result_var = self.next_var("scene_handle");
                lines.push(format!("{}let {} = load_scene_async({});", indent, result_var, path));
                self.output_vars.insert(PinId::new(node.id, "handle"), result_var);
            }
            "scene/spawn" => {
                let handle = self.get_input_value(node, "handle");
                let entity_var = self.next_var("scene_entity");
                lines.push(format!("{}let {} = spawn_scene({});", indent, entity_var, handle));
                self.output_vars.insert(PinId::new(node.id, "entity"), entity_var);
            }
            "scene/unload" => {
                let handle = self.get_input_value(node, "handle");
                lines.push(format!("{}unload_scene({});", indent, handle));
            }
            "scene/change" => {
                let path = self.get_input_value(node, "path");
                lines.push(format!("{}change_scene({});", indent, path));
            }
            "scene/reload" => {
                lines.push(format!("{}reload_scene();", indent));
            }
            "scene/load_prefab" => {
                let path = self.get_input_value(node, "path");
                let result_var = self.next_var("prefab_handle");
                lines.push(format!("{}let {} = load_prefab({});", indent, result_var, path));
                self.output_vars.insert(PinId::new(node.id, "handle"), result_var);
            }
            "scene/load_gltf" => {
                let path = self.get_input_value(node, "path");
                let result_var = self.next_var("gltf_handle");
                lines.push(format!("{}let {} = load_gltf({});", indent, result_var, path));
                self.output_vars.insert(PinId::new(node.id, "handle"), result_var);
            }
            "scene/spawn_gltf" | "scene/spawn_gltf_scene" => {
                let handle = self.get_input_value(node, "handle");
                let index = self.get_input_value(node, "index");
                let entity_var = self.next_var("gltf_entity");
                lines.push(format!("{}let {} = spawn_gltf_scene({}, {});", indent, entity_var, handle, index));
                self.output_vars.insert(PinId::new(node.id, "entity"), entity_var);
            }
            "scene/find" | "scene/find_in_scene" => {
                let name = self.get_input_value(node, "name");
                let result_var = self.next_var("found_entity");
                lines.push(format!("{}let {} = find_in_scene({});", indent, result_var, name));
                self.output_vars.insert(PinId::new(node.id, "entity"), result_var);
            }
            "scene/find_all" | "scene/find_all_in_scene" => {
                let tag = self.get_input_value(node, "tag");
                let result_var = self.next_var("found_entities");
                lines.push(format!("{}let {} = find_all_in_scene({});", indent, result_var, tag));
                self.output_vars.insert(PinId::new(node.id, "entities"), result_var);
            }
            "scene/get_root" | "scene/get_scene_root" => {
                let result_var = self.next_var("scene_root");
                lines.push(format!("{}let {} = get_scene_root();", indent, result_var));
                self.output_vars.insert(PinId::new(node.id, "root"), result_var);
            }
            "scene/save" | "scene/save_scene" => {
                let path = self.get_input_value(node, "path");
                lines.push(format!("{}save_scene({});", indent, path));
            }
            "scene/clone_tree" | "scene/clone_entity_tree" => {
                let entity = self.get_input_value(node, "entity");
                let result_var = self.next_var("cloned_entity");
                lines.push(format!("{}let {} = clone_entity_tree({});", indent, result_var, entity));
                self.output_vars.insert(PinId::new(node.id, "entity"), result_var);
            }

            // ── Window flow nodes ────────────────────────────────────
            "window/set_fullscreen" => {
                let fullscreen = self.get_input_value(node, "fullscreen");
                lines.push(format!("{}set_fullscreen({});", indent, fullscreen));
            }
            "window/set_borderless" => {
                let borderless = self.get_input_value(node, "borderless");
                lines.push(format!("{}set_borderless({});", indent, borderless));
            }
            "window/set_resizable" => {
                let resizable = self.get_input_value(node, "resizable");
                lines.push(format!("{}set_resizable({});", indent, resizable));
            }
            "window/set_decorations" => {
                let decorations = self.get_input_value(node, "decorations");
                lines.push(format!("{}set_decorations({});", indent, decorations));
            }
            "window/set_always_on_top" => {
                let on_top = self.get_input_value(node, "on_top");
                lines.push(format!("{}set_always_on_top({});", indent, on_top));
            }
            "window/set_vsync" => {
                let enabled = self.get_input_value(node, "enabled");
                lines.push(format!("{}set_vsync({});", indent, enabled));
            }
            "window/set_cursor_icon" => {
                let icon = self.get_input_value(node, "icon");
                lines.push(format!("{}set_cursor_icon({});", indent, icon));
            }

            // ── Component setter nodes (auto-generated) ──────────────
            "component/set_property" => {
                let name = self.get_input_value(node, "name");
                let prop = self.get_input_value(node, "property");
                let value = self.get_input_value(node, "value");
                let ent_var = self.next_var("ent");
                lines.push(format!("{}let {} = entity({});", indent, ent_var, name));
                lines.push(format!("{}set({}, {}, {});", indent, ent_var, prop, value));
            }

            _ if node.node_type.starts_with("component/set_") => {
                let name = self.get_input_value(node, "name");
                let ent_var = self.next_var("ent");
                lines.push(format!("{}let {} = entity({});", indent, ent_var, name));
                // Only set properties whose input pins have connections
                for pin in node.input_pins() {
                    if pin.name == "exec" || pin.name == "name" {
                        continue;
                    }
                    let check_pin = PinId::input(node.id, &pin.name);
                    if self.graph.is_pin_connected(&check_pin) {
                        let value = self.get_input_value(node, &pin.name);
                        lines.push(format!("{}set({}, \"{}\", {});", indent, ent_var, pin.name, value));
                    }
                }
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
            let pin_id = PinId::input(node.id, &pin.name);
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

    // Collect event nodes that need polling (non on_ready/on_update events)
    let polling_events: Vec<_> = graph.nodes.iter()
        .filter(|n| {
            n.node_type != "event/on_ready" && n.node_type != "event/on_update" && (
                n.node_type.starts_with("physics/on_") ||
                n.node_type.starts_with("component/on_") ||
                n.node_type.starts_with("time/on_") ||
                n.node_type.starts_with("ui/on_") ||
                n.node_type.starts_with("state/on_") ||
                n.node_type.starts_with("audio/on_") ||
                n.node_type.starts_with("window/on_") ||
                n.node_type.starts_with("scene/on_") ||
                n.node_type.starts_with("animation/on_")
            )
        })
        .collect();

    // Generate on_update function if there's an On Update event OR polling events
    let update_events: Vec<_> = graph.nodes.iter()
        .filter(|n| n.node_type == "event/on_update")
        .collect();

    let needs_update = !update_events.is_empty() || !polling_events.is_empty();

    if needs_update {
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

        // Generate polling checks for event nodes
        for event in &polling_events {
            ctx.processed_nodes.clear();
            ctx.processed_nodes.insert(event.id);

            let indent = "    ";
            match event.node_type.as_str() {
                // Physics collision events
                "physics/on_collision_enter" => {
                    code_lines.push(format!("{}for _col_ent in collisions_entered {{", indent));
                    ctx.output_vars.insert(PinId::new(event.id, "entity"), "_col_ent".to_string());
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }
                "physics/on_collision_exit" => {
                    code_lines.push(format!("{}for _col_ent in collisions_exited {{", indent));
                    ctx.output_vars.insert(PinId::new(event.id, "entity"), "_col_ent".to_string());
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }
                "physics/on_collision_stay" => {
                    code_lines.push(format!("{}for _col_ent in active_collisions {{", indent));
                    ctx.output_vars.insert(PinId::new(event.id, "entity"), "_col_ent".to_string());
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }
                "physics/on_trigger_enter" => {
                    code_lines.push(format!("{}for _trig_ent in triggers_entered {{", indent));
                    ctx.output_vars.insert(PinId::new(event.id, "entity"), "_trig_ent".to_string());
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }
                "physics/on_trigger_exit" => {
                    code_lines.push(format!("{}for _trig_ent in triggers_exited {{", indent));
                    ctx.output_vars.insert(PinId::new(event.id, "entity"), "_trig_ent".to_string());
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }

                // Health events
                "component/on_damage" => {
                    code_lines.push(format!("{}if health_damage_taken > 0.0 {{", indent));
                    ctx.output_vars.insert(PinId::new(event.id, "amount"), "health_damage_taken".to_string());
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }
                "component/on_death" => {
                    code_lines.push(format!("{}if health_just_died {{", indent));
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }
                "component/on_heal" => {
                    code_lines.push(format!("{}if health_healed > 0.0 {{", indent));
                    ctx.output_vars.insert(PinId::new(event.id, "amount"), "health_healed".to_string());
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }

                // Timer events
                "time/on_timer_finished" => {
                    let timer_name = ctx.get_input_value(event, "timer");
                    code_lines.push(format!("{}if timer_just_finished(timers_finished, {}) {{", indent, timer_name));
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }

                // UI events
                "ui/on_button_clicked" => {
                    let entity = ctx.get_input_value(event, "entity");
                    code_lines.push(format!("{}if button_just_clicked(buttons_clicked, {}) {{", indent, entity));
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }
                "ui/on_button_hovered" => {
                    let entity = ctx.get_input_value(event, "entity");
                    code_lines.push(format!("{}if button_hovered(buttons_hovered, {}) {{", indent, entity));
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }
                "ui/on_input_changed" => {
                    let entity = ctx.get_input_value(event, "entity");
                    code_lines.push(format!("{}if input_changed(inputs_changed, {}) {{", indent, entity));
                    ctx.output_vars.insert(PinId::new(event.id, "value"), format!("get_text_input_value({})", entity));
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }
                "ui/on_input_submitted" => {
                    let entity = ctx.get_input_value(event, "entity");
                    code_lines.push(format!("{}if input_submitted(inputs_submitted, {}) {{", indent, entity));
                    ctx.output_vars.insert(PinId::new(event.id, "value"), format!("get_text_input_value({})", entity));
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }
                "ui/on_slider_changed" => {
                    let entity = ctx.get_input_value(event, "entity");
                    code_lines.push(format!("{}if slider_changed(sliders_changed, {}) {{", indent, entity));
                    ctx.output_vars.insert(PinId::new(event.id, "value"), format!("get_slider_value({})", entity));
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }

                // State events
                "state/on_enter" => {
                    let state = ctx.get_input_value(event, "state");
                    code_lines.push(format!("{}if state_just_entered({}) {{", indent, state));
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }
                "state/on_exit" => {
                    let state = ctx.get_input_value(event, "state");
                    code_lines.push(format!("{}if state_just_exited({}) {{", indent, state));
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }
                "state/on_transition" => {
                    let from = ctx.get_input_value(event, "from");
                    let to = ctx.get_input_value(event, "to");
                    code_lines.push(format!("{}if state_transitioned({}, {}) {{", indent, from, to));
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }
                "state/on_pause" => {
                    code_lines.push(format!("{}if game_just_paused {{", indent));
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }
                "state/on_resume" => {
                    code_lines.push(format!("{}if game_just_resumed {{", indent));
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }

                // Audio events
                "audio/on_finished" => {
                    let handle = ctx.get_input_value(event, "handle");
                    code_lines.push(format!("{}if sound_just_finished(sounds_finished, {}) {{", indent, handle));
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }

                // Animation events
                "animation/on_finished" => {
                    code_lines.push(format!("{}if animation_just_finished {{", indent));
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }
                "animation/on_loop" => {
                    code_lines.push(format!("{}if animation_just_looped {{", indent));
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }

                // Window events
                "window/on_resized" => {
                    code_lines.push(format!("{}if window_just_resized {{", indent));
                    ctx.output_vars.insert(PinId::new(event.id, "width"), "window_width".to_string());
                    ctx.output_vars.insert(PinId::new(event.id, "height"), "window_height".to_string());
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }
                "window/on_moved" => {
                    code_lines.push(format!("{}if window_just_moved {{", indent));
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }
                "window/on_focused" => {
                    code_lines.push(format!("{}if window_just_focused {{", indent));
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }
                "window/on_close_requested" => {
                    code_lines.push(format!("{}if close_requested {{", indent));
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }

                // Scene events
                "scene/on_loaded" => {
                    code_lines.push(format!("{}if scene_just_loaded {{", indent));
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }

                // Quit events
                "state/on_quit_requested" => {
                    code_lines.push(format!("{}if quit_requested {{", indent));
                    let body = ctx.follow_flow_from(event.id, "exec");
                    code_lines.extend(body);
                    code_lines.push(format!("{}}}", indent));
                }

                _ => {
                    // Unknown event node, generate a warning
                    warnings.push(format!(
                        "Event node '{}' has no codegen handler",
                        event.node_type
                    ));
                }
            }
        }

        code_lines.push("}".to_string());
    }

    // Check for unconnected required pins
    for node in &graph.nodes {
        for pin in node.input_pins() {
            if pin.required {
                let pin_id = PinId::input(node.id, &pin.name);
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
    // Also mark polling event nodes and their descendants as reachable
    for node in &graph.nodes {
        if node.node_type.contains("/on_") && !node.node_type.starts_with("event/") {
            collect_reachable(graph, node.id, &mut reachable);
        }
    }

    for node in &graph.nodes {
        if !node.node_type.starts_with("event/")
            && !node.node_type.starts_with("utility/comment")
            && !node.node_type.contains("/on_")
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
            let to_pin = PinId::input(start, &pin.name);
            if let Some(conn) = graph.connection_to(&to_pin) {
                collect_reachable(graph, conn.from.node_id, visited);
            }
        }
    }
}
