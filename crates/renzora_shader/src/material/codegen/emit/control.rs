//! `control/*` node emitters — branching, comparison and masking.
//!
//! `control/if` is a runtime `select`; `control/static_switch` is the
//! compile-time one, and the difference matters more than it looks. The static
//! switch never calls `input()` on the unselected pin, so that pin's entire
//! upstream subgraph is never walked and never reaches the shader — which is
//! how a graph carrying two expensive alternatives only pays for one.

use super::super::super::graph::{MaterialNode, NodeId, PinValue};
use super::super::ctx::Ctx;

impl Ctx<'_> {
    pub(crate) fn gen_control_node(&mut self, node: &MaterialNode, id: NodeId) {
        match node.node_type.as_str() {
            "control/if" => {
                let cond = self.input(node, "condition");
                let thresh = self.input(node, "threshold");
                let a = self.input(node, "if_true");
                let b = self.input(node, "if_false");
                let v = self.next_var("ifn");
                self.emit(format!(
                    "    let {v} = select({b}, {a}, {cond} > {thresh});"
                ));
                self.set_out(id, "result", v);
            }
            "control/static_switch" => {
                // Compile-time: only emit the selected branch. `input()` on the
                // unselected pin is never called, so its upstream subgraph is not
                // walked — that chain stays out of the shader entirely.
                let use_a = node
                    .input_values
                    .get("use_a")
                    .and_then(|v| {
                        if let PinValue::Bool(b) = v {
                            Some(*b)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(true);
                let selected = if use_a {
                    self.input(node, "a")
                } else {
                    self.input(node, "b")
                };
                self.set_out(id, "result", selected);
            }
            "control/component_mask" => {
                let vec = self.input(node, "vector");
                let get_bool = |name: &str, default: bool| {
                    node.input_values
                        .get(name)
                        .and_then(|v| {
                            if let PinValue::Bool(b) = v {
                                Some(*b)
                            } else {
                                None
                            }
                        })
                        .unwrap_or(default)
                };
                let kr = if get_bool("keep_r", true) {
                    format!("({vec}).x")
                } else {
                    "0.0".to_string()
                };
                let kg = if get_bool("keep_g", true) {
                    format!("({vec}).y")
                } else {
                    "0.0".to_string()
                };
                let kb = if get_bool("keep_b", true) {
                    format!("({vec}).z")
                } else {
                    "0.0".to_string()
                };
                let ka = if get_bool("keep_a", false) {
                    format!("({vec}).w")
                } else {
                    "0.0".to_string()
                };
                let v = self.next_var("mask");
                self.emit(format!("    let {v} = vec4<f32>({kr}, {kg}, {kb}, {ka});"));
                self.set_out(id, "vector", v);
            }
            "control/greater_than" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("gt");
                self.emit(format!("    let {v} = select(0.0, 1.0, {a} > {b});"));
                self.set_out(id, "result", v);
            }
            "control/less_than" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("lt");
                self.emit(format!("    let {v} = select(0.0, 1.0, {a} < {b});"));
                self.set_out(id, "result", v);
            }
            "control/equal" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let eps = self.input(node, "epsilon");
                let v = self.next_var("eq");
                self.emit(format!(
                    "    let {v} = select(0.0, 1.0, abs({a} - {b}) < {eps});"
                ));
                self.set_out(id, "result", v);
            }
            "control/not_equal" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let eps = self.input(node, "epsilon");
                let v = self.next_var("neq");
                self.emit(format!(
                    "    let {v} = select(0.0, 1.0, abs({a} - {b}) >= {eps});"
                ));
                self.set_out(id, "result", v);
            }
            "control/and" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("and");
                self.emit(format!("    let {v} = min({a}, {b});"));
                self.set_out(id, "result", v);
            }
            "control/or" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("or");
                self.emit(format!("    let {v} = max({a}, {b});"));
                self.set_out(id, "result", v);
            }
            "control/not" => {
                let val = self.input(node, "value");
                let v = self.next_var("not");
                self.emit(format!("    let {v} = 1.0 - {val};"));
                self.set_out(id, "result", v);
            }
            unknown => self.unknown_node(unknown),
        }
    }
}
