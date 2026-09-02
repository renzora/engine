//! `function/*` node emitters — material functions (subgraphs).
//!
//! A `function/call` compiles the referenced function's body into the module
//! prelude exactly once (tracked by `emitted_functions`) and then emits a call
//! to it. `compiling_functions` is the cycle guard: a function that reaches
//! itself, directly or through a chain, degrades to a comment rather than
//! recursing until the stack gives out.
//!
//! Every failure mode here — no registry, empty reference, unknown name,
//! recursion — still writes all four output pins, because a downstream node
//! resolving an absent pin would fall back to a bare `0.0` and produce a WGSL
//! type error far from the actual mistake.

use super::super::super::graph::{MaterialNode, NodeId, PinValue};
use super::super::ctx::Ctx;
use super::super::safe_fn_ident;

impl Ctx<'_> {
    pub(crate) fn gen_function_node(&mut self, node: &MaterialNode, id: NodeId) {
        match node.node_type.as_str() {
            "function/input_point" => {
                // Only meaningful inside a function compilation — its outputs
                // resolve to the function's parameter names. When encountered
                // in a top-level graph we still set them (harmless) so users
                // can preview a function graph directly.
                self.set_out(id, "in_0", "in_0".to_string());
                self.set_out(id, "in_1", "in_1".to_string());
                self.set_out(id, "in_2", "in_2".to_string());
                self.set_out(id, "in_3", "in_3".to_string());
            }
            "function/output_point" => {
                // Handled specially by compile_function_body.
                // In a top-level graph it's inert (no outputs pins to wire).
            }
            "function/call" => {
                let fn_name = node
                    .input_values
                    .get("function")
                    .and_then(|v| {
                        if let PinValue::String(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();

                if fn_name.is_empty() || self.functions.is_none() {
                    // No registry or empty reference — degrade to pass-through.
                    self.lines
                        .push(format!("    // function/call: empty reference (id={id})"));
                    self.set_out(id, "out_0", "vec4<f32>(0.0)".to_string());
                    self.set_out(id, "out_1", "vec4<f32>(0.0)".to_string());
                    self.set_out(id, "out_2", "vec4<f32>(0.0)".to_string());
                    self.set_out(id, "out_3", "vec4<f32>(0.0)".to_string());
                    return;
                }

                // Resolve input expressions up-front (triggers upstream codegen).
                let in0 = self.input(node, "in_0");
                let in1 = self.input(node, "in_1");
                let in2 = self.input(node, "in_2");
                let in3 = self.input(node, "in_3");

                // Emit the function body into module_prelude exactly once.
                if !self.emitted_functions.contains(&fn_name) {
                    if self.compiling_functions.contains(&fn_name) {
                        self.lines.push(format!(
                            "    // function/call: recursive cycle detected for '{fn_name}'"
                        ));
                    } else {
                        let registry = self.functions.unwrap();
                        match registry.get(&fn_name) {
                            Some(mat_fn) => {
                                self.compiling_functions.insert(fn_name.clone());
                                let fn_wgsl = self.compile_function_body(mat_fn);
                                self.compiling_functions.remove(&fn_name);
                                self.emit_prelude(fn_wgsl);
                                self.emitted_functions.insert(fn_name.clone());
                            }
                            None => {
                                self.lines.push(format!(
                                    "    // function/call: unknown function '{fn_name}'"
                                ));
                                self.set_out(id, "out_0", "vec4<f32>(0.0)".to_string());
                                self.set_out(id, "out_1", "vec4<f32>(0.0)".to_string());
                                self.set_out(id, "out_2", "vec4<f32>(0.0)".to_string());
                                self.set_out(id, "out_3", "vec4<f32>(0.0)".to_string());
                                return;
                            }
                        }
                    }
                }

                let ident = safe_fn_ident(&fn_name);
                let v = self.next_var("fcall");
                self.emit(format!(
                    "    let {v} = mfunc_{ident}({in0}, {in1}, {in2}, {in3});"
                ));
                self.set_out(id, "out_0", format!("{v}.out_0"));
                self.set_out(id, "out_1", format!("{v}.out_1"));
                self.set_out(id, "out_2", format!("{v}.out_2"));
                self.set_out(id, "out_3", format!("{v}.out_3"));
            }
            unknown => self.unknown_node(unknown),
        }
    }
}
