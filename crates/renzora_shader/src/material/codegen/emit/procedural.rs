//! `procedural/*` node emitters — noise, gradients, and the two
//! height-to-normal reconstructions.
//!
//! Each noise family sets the `uses_*` flag for the WGSL helper it calls, so a
//! graph only pays for the noise it actually uses. The triplanar variants all
//! share [`Ctx::emit_triplanar_noise`] — project onto three world planes,
//! weight by the world normal, sum — because only the helper name differs.
//!
//! `normal_from_height` and `world_normal_from_height` are deliberately
//! separate nodes: the first produces a tangent-space normal, the second
//! reconstructs a world-space frame per fragment from the screen-space
//! derivatives of `world_position` (Schüler's no-precomputed-tangents trick).
//! Wiring the tangent-space one into a world-space pin is exactly the mistake
//! the split exists to make visible.

use super::super::super::graph::{MaterialNode, NodeId};
use super::super::ctx::Ctx;

impl Ctx<'_> {
    pub(crate) fn gen_procedural_node(&mut self, node: &MaterialNode, id: NodeId) {
        match node.node_type.as_str() {
            "procedural/noise_perlin" | "procedural/noise_simplex" => {
                self.uses_noise = true;
                let uv = self.input(node, "uv");
                let scale = self.input(node, "scale");
                let v = self.next_var("noise");
                self.emit(format!("    let {v} = mat_noise({uv} * {scale});"));
                self.set_out(id, "value", v);
            }
            "procedural/noise_voronoi" => {
                self.uses_voronoi_full = true;
                self.uses_hash = true;
                let uv = self.input(node, "uv");
                let scale = self.input(node, "scale");
                let v = self.next_var("vor");
                self.emit(format!("    let {v} = mat_voronoi_full({uv} * {scale});"));
                self.set_out(id, "distance", format!("{v}.x"));
                self.set_out(id, "f2", format!("{v}.y"));
                self.set_out(id, "edge", format!("{v}.z"));
                self.set_out(id, "cell_id", format!("{v}.w"));
            }
            "procedural/noise_fbm" => {
                self.uses_noise = true;
                self.uses_fbm = true;
                let uv = self.input(node, "uv");
                let scale = self.input(node, "scale");
                let octaves = self.input(node, "octaves");
                let lac = self.input(node, "lacunarity");
                let pers = self.input(node, "persistence");
                let v = self.next_var("fbm");
                self.emit(format!(
                    "    let {v} = mat_fbm({uv} * {scale}, i32({octaves}), {lac}, {pers});"
                ));
                self.set_out(id, "value", v);
            }
            "procedural/checkerboard" => {
                let uv = self.input(node, "uv");
                let scale = self.input(node, "scale");
                let v = self.next_var("check");
                self.emit(format!("    let {v} = step(0.5, fract(floor({uv}.x * {scale}) * 0.5 + floor({uv}.y * {scale}) * 0.5 + 0.25));"));
                self.set_out(id, "value", v);
            }
            "procedural/gradient" => {
                let uv = self.input(node, "uv");
                self.set_out(id, "u", format!("{uv}.x"));
                self.set_out(id, "v", format!("{uv}.y"));
            }
            "procedural/brick" => {
                let uv = self.input(node, "uv");
                let scale = self.input(node, "scale");
                let mortar = self.input(node, "mortar");
                let v = self.next_var("brick");
                let buv = self.next_var("buv");
                self.emit(format!("    var {buv} = {uv} * {scale};"));
                self.emit(format!(
                    "    {buv}.x = {buv}.x + step(1.0, fract({buv}.y * 0.5)) * 0.5;"
                ));
                self.emit(format!("    let {v} = step({mortar}, fract({buv}.x)) * step({mortar}, fract({buv}.y));"));
                self.set_out(id, "value", v);
            }
            "procedural/normal_from_height" => {
                let height = self.input(node, "height");
                let strength = self.input(node, "strength");
                let v = self.next_var("nfh");
                self.emit(format!("    let {v} = normalize(vec3<f32>(dpdx({height}) * {strength}, dpdy({height}) * {strength}, 1.0));"));
                self.set_out(id, "normal", v);
            }
            "procedural/world_normal_from_height" => {
                // Reconstruct a world-space tangent frame per-fragment from the
                // screen-space derivatives of `world_position`, then perturb the
                // world normal by the height gradient in that frame.
                //
                // Based on Christian Schüler's "Normal Mapping Without Precomputed
                // Tangents" trick — works on any surface orientation without
                // requiring mesh-supplied tangents.
                let height = self.input(node, "height");
                let strength = self.input(node, "strength");
                let v = self.next_var("wnfh");
                self.emit(format!("    let {v}_dpdx = dpdx(in.world_position.xyz);"));
                self.emit(format!("    let {v}_dpdy = dpdy(in.world_position.xyz);"));
                self.emit(format!("    let {v}_dhdx = dpdx({height});"));
                self.emit(format!("    let {v}_dhdy = dpdy({height});"));
                self.emit(format!("    let {v}_n0 = normalize(in.world_normal);"));
                self.emit(format!("    let {v}_r1 = cross({v}_dpdy, {v}_n0);"));
                self.emit(format!("    let {v}_r2 = cross({v}_n0, {v}_dpdx);"));
                self.emit(format!(
                    "    let {v}_det = max(dot({v}_dpdx, {v}_r1), 0.0000001);"
                ));
                self.emit(format!(
                    "    let {v}_grad = ({v}_dhdx * {v}_r1 + {v}_dhdy * {v}_r2) / {v}_det;"
                ));
                self.emit(format!(
                    "    let {v} = normalize({v}_n0 - {v}_grad * {strength});"
                ));
                self.set_out(id, "normal", v);
            }
            "procedural/domain_warp" => {
                self.uses_noise = true;
                self.uses_fbm = true;
                let uv = self.input(node, "uv");
                let scale = self.input(node, "scale");
                let strength = self.input(node, "strength");
                let offset = self.input(node, "offset");
                let v = self.next_var("warp");
                self.emit(format!(
                    "    let {v} = {uv} + vec2<f32>(mat_fbm({uv} * {scale}, 3, 2.0, 0.5), mat_fbm(({uv} + {offset}) * {scale}, 3, 2.0, 0.5)) * {strength};"
                ));
                self.set_out(id, "uv", v);
            }
            "procedural/noise_ridged" => {
                self.uses_noise = true;
                self.uses_fbm_ridged = true;
                let uv = self.input(node, "uv");
                let scale = self.input(node, "scale");
                let octaves = self.input(node, "octaves");
                let lac = self.input(node, "lacunarity");
                let pers = self.input(node, "persistence");
                let v = self.next_var("ridged");
                self.emit(format!(
                    "    let {v} = mat_fbm_ridged({uv} * {scale}, i32({octaves}), {lac}, {pers});"
                ));
                self.set_out(id, "value", v);
            }
            "procedural/noise_turbulence" => {
                self.uses_noise = true;
                self.uses_fbm_turbulence = true;
                let uv = self.input(node, "uv");
                let scale = self.input(node, "scale");
                let octaves = self.input(node, "octaves");
                let lac = self.input(node, "lacunarity");
                let pers = self.input(node, "persistence");
                let v = self.next_var("turb");
                self.emit(format!("    let {v} = mat_fbm_turbulence({uv} * {scale}, i32({octaves}), {lac}, {pers});"));
                self.set_out(id, "value", v);
            }
            "procedural/noise_billow" => {
                self.uses_noise = true;
                self.uses_fbm_billow = true;
                let uv = self.input(node, "uv");
                let scale = self.input(node, "scale");
                let octaves = self.input(node, "octaves");
                let lac = self.input(node, "lacunarity");
                let pers = self.input(node, "persistence");
                let v = self.next_var("billow");
                self.emit(format!(
                    "    let {v} = mat_fbm_billow({uv} * {scale}, i32({octaves}), {lac}, {pers});"
                ));
                self.set_out(id, "value", v);
            }
            "procedural/noise_white" => {
                self.uses_hash = true;
                let uv = self.input(node, "uv");
                let scale = self.input(node, "scale");
                let v = self.next_var("wh");
                self.emit(format!("    let {v} = mat_hash(floor({uv} * {scale}));"));
                self.set_out(id, "value", v);
            }
            "procedural/noise_curl" => {
                self.uses_noise = true;
                self.uses_curl = true;
                let uv = self.input(node, "uv");
                let scale = self.input(node, "scale");
                let eps = self.input(node, "epsilon");
                let v = self.next_var("curl");
                self.emit(format!(
                    "    let {v} = mat_curl_noise({uv} * {scale}, {eps});"
                ));
                self.set_out(id, "flow", v);
            }
            "procedural/gradient_radial" => {
                let uv = self.input(node, "uv");
                let center = self.input(node, "center");
                let radius = self.input(node, "radius");
                let soft = self.input(node, "softness");
                let v = self.next_var("grad_r");
                self.emit(format!(
                    "    let {v} = 1.0 - smoothstep({radius} - {soft}, {radius}, length({uv} - {center}));"
                ));
                self.set_out(id, "value", v);
            }
            "procedural/gradient_linear" => {
                let uv = self.input(node, "uv");
                let angle = self.input(node, "angle");
                let center = self.input(node, "center");
                let v = self.next_var("grad_l");
                self.emit(format!(
                    "    let {v} = saturate(dot({uv} - {center}, vec2<f32>(cos({angle}), sin({angle}))) + 0.5);"
                ));
                self.set_out(id, "value", v);
            }
            "procedural/gradient_angular" => {
                let uv = self.input(node, "uv");
                let center = self.input(node, "center");
                let off = self.input(node, "offset");
                let v = self.next_var("grad_a");
                self.emit(format!(
                    "    let {v} = fract((atan2(({uv} - {center}).y, ({uv} - {center}).x) / 6.2831853) + {off});"
                ));
                self.set_out(id, "value", v);
            }
            "procedural/gradient_diamond" => {
                let uv = self.input(node, "uv");
                let center = self.input(node, "center");
                let size = self.input(node, "size");
                let v = self.next_var("grad_d");
                self.emit(format!(
                    "    let {v} = 1.0 - saturate((abs(({uv} - {center}).x) + abs(({uv} - {center}).y)) / max({size}, 0.0001));"
                ));
                self.set_out(id, "value", v);
            }
            "procedural/bump_offset" => {
                let uv = self.input(node, "uv");
                let height = self.input(node, "height");
                let reference = self.input(node, "reference");
                let strength = self.input(node, "strength");
                let v = self.next_var("bump");
                // Simplified: approximate view as tangent-space (0,0,1) and offset by (height-ref)*strength
                // toward fake view direction using UV derivatives.
                self.emit(format!(
                    "    let {v} = {uv} + normalize(vec2<f32>(dpdx({height}), dpdy({height})) + vec2<f32>(0.0001)) * (({height} - {reference}) * {strength});"
                ));
                self.set_out(id, "uv", v);
            }
            "procedural/noise_triplanar_fbm" => {
                self.emit_triplanar_noise(
                    node, id, "mat_fbm", "tri_fbm", /*extra_arg_arity=*/ 3,
                );
                self.uses_noise = true;
                self.uses_fbm = true;
            }
            "procedural/noise_triplanar_ridged" => {
                self.emit_triplanar_noise(node, id, "mat_fbm_ridged", "tri_ridged", 3);
                self.uses_noise = true;
                self.uses_fbm_ridged = true;
            }
            "procedural/noise_triplanar_turbulence" => {
                self.emit_triplanar_noise(node, id, "mat_fbm_turbulence", "tri_turb", 3);
                self.uses_noise = true;
                self.uses_fbm_turbulence = true;
            }
            "procedural/noise_triplanar_billow" => {
                self.emit_triplanar_noise(node, id, "mat_fbm_billow", "tri_billow", 3);
                self.uses_noise = true;
                self.uses_fbm_billow = true;
            }
            "procedural/noise_triplanar_voronoi" => {
                // Voronoi's full helper returns vec4 (f1, f2, edge, cell_id).
                // We project onto 3 world planes and blend by world normal.
                self.uses_voronoi_full = true;
                self.uses_hash = true;
                let scale = self.input(node, "scale");
                let sharp = self.input(node, "sharpness");
                let v = self.next_var("tri_vor");
                self.emit(format!("    let {v}_p = in.world_position.xyz * {scale};"));
                self.emit(format!(
                    "    let {v}_wa = pow(abs(in.world_normal), vec3<f32>({sharp}));"
                ));
                self.emit(format!(
                    "    let {v}_w = {v}_wa / ({v}_wa.x + {v}_wa.y + {v}_wa.z + 0.000001);"
                ));
                self.emit(format!("    let {v}_x = mat_voronoi_full({v}_p.yz);"));
                self.emit(format!("    let {v}_y = mat_voronoi_full({v}_p.xz);"));
                self.emit(format!("    let {v}_z = mat_voronoi_full({v}_p.xy);"));
                self.emit(format!(
                    "    let {v} = {v}_x * {v}_w.x + {v}_y * {v}_w.y + {v}_z * {v}_w.z;"
                ));
                self.set_out(id, "distance", format!("{v}.x"));
                self.set_out(id, "cell_id", format!("{v}.w"));
            }

            "procedural/hex_tile" => {
                self.uses_hex_tile = true;
                self.uses_hash = true;
                let uv = if self.graph.connection_to(node.id, "uv").is_some() {
                    self.input(node, "uv")
                } else {
                    "mat_uv".to_string()
                };
                let scale = self.input(node, "scale");
                let variation = self.input(node, "variation");
                let v = self.next_var("hex");
                self.emit(format!(
                    "    let {v} = mat_hex_tile({uv} * {scale}, {variation});"
                ));
                self.set_out(id, "uv1", format!("{v}.uv_a"));
                self.set_out(id, "uv2", format!("{v}.uv_b"));
                self.set_out(id, "uv3", format!("{v}.uv_c"));
                self.set_out(id, "w1", format!("{v}.w.x"));
                self.set_out(id, "w2", format!("{v}.w.y"));
                self.set_out(id, "w3", format!("{v}.w.z"));
            }
            unknown => self.unknown_node(unknown),
        }
    }
}
