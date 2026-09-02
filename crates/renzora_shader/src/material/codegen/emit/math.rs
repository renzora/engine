//! `math/*` and `vector/*` node emitters.
//!
//! Math nodes are *rank-polymorphic*: the same Add node compiles to `f32 +
//! f32` or `vec4 + vec4` depending on what got wired to it. The latched rank
//! comes from [`Ctx::math_rank`], and any scalar literal a node needs — an
//! epsilon guard, a `1.0` for one-minus — has to be splatted to that rank via
//! [`Ctx::guard_lit`], because WGSL will not mix a scalar with a vector in
//! these positions.
//!
//! Several arms guard a divisor or a domain (`max(b, eps)`, `sqrt(max(v, 0))`,
//! `asin(clamp(v, -1, 1))`). Those are not defensive noise: one NaN produced
//! here propagates through the rest of the shader and blanks the surface.

use super::super::super::graph::{MaterialNode, NodeId, PinValue};
use super::super::ctx::Ctx;

impl Ctx<'_> {
    pub(crate) fn gen_math_node(&mut self, node: &MaterialNode, id: NodeId) {
        match node.node_type.as_str() {
            "math/add" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("add");
                self.emit(format!("    let {v} = {a} + {b};"));
                self.set_out(id, "result", v);
            }
            "math/subtract" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("sub");
                self.emit(format!("    let {v} = {a} - {b};"));
                self.set_out(id, "result", v);
            }
            "math/multiply" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("mul");
                self.emit(format!("    let {v} = {a} * {b};"));
                self.set_out(id, "result", v);
            }
            "math/divide" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let eps = Self::guard_lit(self.math_rank(node), "0.000001");
                let v = self.next_var("div");
                self.emit(format!("    let {v} = {a} / max({b}, {eps});"));
                self.set_out(id, "result", v);
            }
            "math/power" => {
                let base = self.input(node, "base");
                let exp = self.input(node, "exp");
                let v = self.next_var("pow");
                self.emit(format!("    let {v} = pow(abs({base}), {exp});"));
                self.set_out(id, "result", v);
            }
            "math/abs" => {
                let val = self.input(node, "value");
                let v = self.next_var("abs");
                self.emit(format!("    let {v} = abs({val});"));
                self.set_out(id, "result", v);
            }
            "math/negate" => {
                let val = self.input(node, "value");
                let v = self.next_var("neg");
                self.emit(format!("    let {v} = -{val};"));
                self.set_out(id, "result", v);
            }
            "math/one_minus" => {
                let val = self.input(node, "value");
                let one = Self::guard_lit(self.math_rank(node), "1.0");
                let v = self.next_var("om");
                self.emit(format!("    let {v} = {one} - {val};"));
                self.set_out(id, "result", v);
            }
            "math/fract" => {
                let val = self.input(node, "value");
                let v = self.next_var("frc");
                self.emit(format!("    let {v} = fract({val});"));
                self.set_out(id, "result", v);
            }
            "math/floor" => {
                let val = self.input(node, "value");
                let v = self.next_var("flr");
                self.emit(format!("    let {v} = floor({val});"));
                self.set_out(id, "result", v);
            }
            "math/ceil" => {
                let val = self.input(node, "value");
                let v = self.next_var("cel");
                self.emit(format!("    let {v} = ceil({val});"));
                self.set_out(id, "result", v);
            }
            "math/min" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("mn");
                self.emit(format!("    let {v} = min({a}, {b});"));
                self.set_out(id, "result", v);
            }
            "math/max" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("mx");
                self.emit(format!("    let {v} = max({a}, {b});"));
                self.set_out(id, "result", v);
            }
            "math/clamp" => {
                let val = self.input(node, "value");
                let lo = self.input(node, "min");
                let hi = self.input(node, "max");
                let v = self.next_var("cmp");
                self.emit(format!("    let {v} = clamp({val}, {lo}, {hi});"));
                self.set_out(id, "result", v);
            }
            "math/lerp" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let t = self.input(node, "t");
                let v = self.next_var("lrp");
                self.emit(format!("    let {v} = mix({a}, {b}, {t});"));
                self.set_out(id, "result", v);
            }
            "math/smoothstep" => {
                let e0 = self.input(node, "edge0");
                let e1 = self.input(node, "edge1");
                let val = self.input(node, "value");
                let v = self.next_var("ss");
                self.emit(format!("    let {v} = smoothstep({e0}, {e1}, {val});"));
                self.set_out(id, "result", v);
            }
            "math/step" => {
                let edge = self.input(node, "edge");
                let val = self.input(node, "value");
                let v = self.next_var("stp");
                self.emit(format!("    let {v} = step({edge}, {val});"));
                self.set_out(id, "result", v);
            }
            "math/remap" => {
                let val = self.input(node, "value");
                let in_min = self.input(node, "in_min");
                let in_max = self.input(node, "in_max");
                let out_min = self.input(node, "out_min");
                let out_max = self.input(node, "out_max");
                let eps = Self::guard_lit(self.math_rank(node), "0.000001");
                let v = self.next_var("remap");
                self.emit(format!("    let {v} = {out_min} + ({val} - {in_min}) / max({in_max} - {in_min}, {eps}) * ({out_max} - {out_min});"));
                self.set_out(id, "result", v);
            }
            "math/sin" => {
                let val = self.input(node, "value");
                let v = self.next_var("sin");
                self.emit(format!("    let {v} = sin({val});"));
                self.set_out(id, "result", v);
            }
            "math/cos" => {
                let val = self.input(node, "value");
                let v = self.next_var("cos");
                self.emit(format!("    let {v} = cos({val});"));
                self.set_out(id, "result", v);
            }
            "math/saturate" => {
                let val = self.input(node, "value");
                let v = self.next_var("sat");
                self.emit(format!("    let {v} = saturate({val});"));
                self.set_out(id, "result", v);
            }
            "math/modulo" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let eps = Self::guard_lit(self.math_rank(node), "0.000001");
                let v = self.next_var("mod");
                self.emit(format!(
                    "    let {v} = {a} - {b} * floor({a} / max({b}, {eps}));"
                ));
                self.set_out(id, "result", v);
            }
            "math/sign" => {
                let val = self.input(node, "value");
                let v = self.next_var("sgn");
                self.emit(format!("    let {v} = sign({val});"));
                self.set_out(id, "result", v);
            }
            "math/atan2" => {
                let y = self.input(node, "y");
                let x = self.input(node, "x");
                let v = self.next_var("atan2");
                self.emit(format!("    let {v} = atan2({y}, {x});"));
                self.set_out(id, "result", v);
            }
            "math/trunc" => {
                let val = self.input(node, "value");
                let v = self.next_var("trn");
                self.emit(format!("    let {v} = trunc({val});"));
                self.set_out(id, "result", v);
            }
            "math/round" => {
                let val = self.input(node, "value");
                let v = self.next_var("rnd");
                self.emit(format!("    let {v} = round({val});"));
                self.set_out(id, "result", v);
            }
            "math/exp" => {
                let val = self.input(node, "value");
                let v = self.next_var("exp");
                self.emit(format!("    let {v} = exp({val});"));
                self.set_out(id, "result", v);
            }
            "math/log" => {
                let val = self.input(node, "value");
                let eps = Self::guard_lit(self.math_rank(node), "0.000001");
                let v = self.next_var("log");
                self.emit(format!("    let {v} = log(max({val}, {eps}));"));
                self.set_out(id, "result", v);
            }
            "math/sqrt" => {
                let val = self.input(node, "value");
                let zero = Self::guard_lit(self.math_rank(node), "0.0");
                let v = self.next_var("sqrt");
                self.emit(format!("    let {v} = sqrt(max({val}, {zero}));"));
                self.set_out(id, "result", v);
            }
            "math/reciprocal" => {
                let val = self.input(node, "value");
                let rt = self.math_rank(node);
                let one = Self::guard_lit(rt, "1.0");
                let eps = Self::guard_lit(rt, "0.000001");
                let v = self.next_var("rcp");
                self.emit(format!("    let {v} = {one} / max({val}, {eps});"));
                self.set_out(id, "result", v);
            }
            "math/tan" => {
                let val = self.input(node, "value");
                let v = self.next_var("tan");
                self.emit(format!("    let {v} = tan({val});"));
                self.set_out(id, "result", v);
            }
            "math/asin" => {
                let val = self.input(node, "value");
                let rt = self.math_rank(node);
                let lo = Self::guard_lit(rt, "-1.0");
                let hi = Self::guard_lit(rt, "1.0");
                let v = self.next_var("asin");
                self.emit(format!("    let {v} = asin(clamp({val}, {lo}, {hi}));"));
                self.set_out(id, "result", v);
            }
            "math/acos" => {
                let val = self.input(node, "value");
                let rt = self.math_rank(node);
                let lo = Self::guard_lit(rt, "-1.0");
                let hi = Self::guard_lit(rt, "1.0");
                let v = self.next_var("acos");
                self.emit(format!("    let {v} = acos(clamp({val}, {lo}, {hi}));"));
                self.set_out(id, "result", v);
            }
            "math/radians" => {
                let val = self.input(node, "value");
                let v = self.next_var("rad");
                self.emit(format!("    let {v} = radians({val});"));
                self.set_out(id, "result", v);
            }
            "math/degrees" => {
                let val = self.input(node, "value");
                let v = self.next_var("deg");
                self.emit(format!("    let {v} = degrees({val});"));
                self.set_out(id, "result", v);
            }
            unknown => self.unknown_node(unknown),
        }
    }

    pub(crate) fn gen_vector_node(&mut self, node: &MaterialNode, id: NodeId) {
        match node.node_type.as_str() {
            "vector/split_vec2" => {
                let vec = self.input(node, "vector");
                self.set_out(id, "x", format!("{vec}.x"));
                self.set_out(id, "y", format!("{vec}.y"));
            }
            "vector/split_vec3" => {
                let vec = self.input(node, "vector");
                self.set_out(id, "x", format!("{vec}.x"));
                self.set_out(id, "y", format!("{vec}.y"));
                self.set_out(id, "z", format!("{vec}.z"));
            }
            "vector/combine_vec2" => {
                let x = self.input(node, "x");
                let y = self.input(node, "y");
                let v = self.next_var("v2");
                self.emit(format!("    let {v} = vec2<f32>({x}, {y});"));
                self.set_out(id, "vector", v);
            }
            "vector/combine_vec3" => {
                let x = self.input(node, "x");
                let y = self.input(node, "y");
                let z = self.input(node, "z");
                let v = self.next_var("v3");
                self.emit(format!("    let {v} = vec3<f32>({x}, {y}, {z});"));
                self.set_out(id, "vector", v);
            }
            "vector/combine_vec4" => {
                let x = self.input(node, "x");
                let y = self.input(node, "y");
                let z = self.input(node, "z");
                let w = self.input(node, "w");
                let v = self.next_var("v4");
                self.emit(format!("    let {v} = vec4<f32>({x}, {y}, {z}, {w});"));
                self.set_out(id, "vector", v);
            }
            "vector/dot" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("dot");
                self.emit(format!("    let {v} = dot({a}, {b});"));
                self.set_out(id, "result", v);
            }
            "vector/cross" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("cross");
                self.emit(format!("    let {v} = cross({a}, {b});"));
                self.set_out(id, "result", v);
            }
            "vector/normalize" => {
                let vec = self.input(node, "vector");
                let v = self.next_var("norm");
                self.emit(format!("    let {v} = normalize({vec});"));
                self.set_out(id, "result", v);
            }
            "vector/distance" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("dist");
                self.emit(format!("    let {v} = distance({a}, {b});"));
                self.set_out(id, "result", v);
            }
            "vector/length" => {
                let vec = self.input(node, "vector");
                let v = self.next_var("len");
                self.emit(format!("    let {v} = length({vec});"));
                self.set_out(id, "result", v);
            }
            "vector/reflect" => {
                let inc = self.input(node, "incident");
                let n = self.input(node, "normal");
                let v = self.next_var("refl");
                self.emit(format!("    let {v} = reflect({inc}, {n});"));
                self.set_out(id, "result", v);
            }
            "vector/refract" => {
                let inc = self.input(node, "incident");
                let n = self.input(node, "normal");
                let eta = self.input(node, "eta");
                let v = self.next_var("refr");
                self.emit(format!("    let {v} = refract({inc}, {n}, {eta});"));
                self.set_out(id, "result", v);
            }
            "vector/swizzle" => {
                let vec = self.input(node, "vector");
                let choices = ["out_x", "out_y", "out_z", "out_w"];
                let mut parts = Vec::with_capacity(4);
                for pin in &choices {
                    let sel = node
                        .input_values
                        .get(*pin)
                        .and_then(|v| {
                            if let PinValue::Int(i) = v {
                                Some(*i)
                            } else {
                                None
                            }
                        })
                        .unwrap_or(match *pin {
                            "out_x" => 0,
                            "out_y" => 1,
                            "out_z" => 2,
                            _ => 3,
                        });
                    parts.push(match sel {
                        0 => format!("({vec}).x"),
                        1 => format!("({vec}).y"),
                        2 => format!("({vec}).z"),
                        3 => format!("({vec}).w"),
                        4 => "0.0".to_string(),
                        5 => "1.0".to_string(),
                        _ => format!("({vec}).x"),
                    });
                }
                let v = self.next_var("swz");
                self.emit(format!(
                    "    let {v} = vec4<f32>({}, {}, {}, {});",
                    parts[0], parts[1], parts[2], parts[3]
                ));
                self.set_out(id, "vector", v);
            }
            unknown => self.unknown_node(unknown),
        }
    }
}
