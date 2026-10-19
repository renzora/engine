//! `scene/*` node emitters — reads of the frame's other buffers.
//!
//! Every one of these depends on a pass that may not be running, so each is
//! gated with `@if` on its prepass flag, with a sentinel for the `@if(!...)`
//! side (see [`Ctx::emit_if`]). The
//! fallback is not politeness: without it the material fails to *compile* for
//! any camera lacking that prepass, which is a different and much worse
//! failure than a wrong-looking value.
//!
//! `scene/scene_color` carries the sharpest caveat — Bevy only populates
//! `view_transmission_texture` when something in the scene actually has PBR
//! transmission, so a graph that reaches for it in isolation gets black.
//! `scene/env_map_sample` is the reliable way to get sky into a refraction.

use super::super::super::graph::{MaterialNode, NodeId};
use super::super::ctx::Ctx;

impl Ctx<'_> {
    pub(crate) fn gen_scene_node(&mut self, node: &MaterialNode, id: NodeId) {
        // The 2D pipeline has no prepasses, no environment map and no
        // transmission texture; `refuses_node` is the shared list.
        if let Some(reason) = self.graph.domain.refuses_node(&node.node_type) {
            self.errors.push(reason);
            // Still define the outputs, so the rest of the graph compiles and
            // this is the only error reported.
            for pin in ["depth", "fade", "normal", "velocity", "speed", "color", "rgb"] {
                self.set_out(id, pin, "vec4<f32>(0.0)".into());
            }
            return;
        }
        match node.node_type.as_str() {
            "scene/pixel_depth" => {
                self.uses_scene_depth = true;
                let v = self.next_var("pxdepth");
                self.emit(format!("    let {v} = mat_linearize_depth(in.position.z);"));
                self.set_out(id, "depth", v);
            }
            "scene/scene_depth" => {
                self.uses_scene_depth = true;
                let v = self.next_var("scdepth");
                // Guard the prepass sample — if no DepthPrepass is active, the
                // shader still compiles but returns a "far away" sentinel.
                self.emit_if(
                    "DEPTH_PREPASS",
                    &[format!(
                        "let {v} = mat_linearize_depth(prepass_utils::prepass_depth(in.position, 0u));"
                    )],
                    &[format!("let {v} = 1.0e6;")],
                );
                self.set_out(id, "depth", v);
            }
            "scene/depth_fade" => {
                self.uses_scene_depth = true;
                let distance = self.input(node, "distance");
                let v = self.next_var("sdfade");
                self.emit_if(
                    "DEPTH_PREPASS",
                    &[
                        format!(
                            "let {v}_scene = mat_linearize_depth(prepass_utils::prepass_depth(in.position, 0u));"
                        ),
                        format!("let {v}_pixel = mat_linearize_depth(in.position.z);"),
                        format!(
                            "let {v} = saturate(({v}_scene - {v}_pixel) / max({distance}, 0.0001));"
                        ),
                    ],
                    &[format!("let {v} = 1.0;")],
                );
                self.set_out(id, "fade", v);
            }
            "scene/scene_normal" => {
                self.uses_scene_normal = true;
                let v = self.next_var("snrm");
                self.emit_if(
                    "NORMAL_PREPASS",
                    &[format!("let {v} = prepass_utils::prepass_normal(in.position, 0u);")],
                    &[format!("let {v} = vec3<f32>(0.0, 1.0, 0.0);")],
                );
                self.set_out(id, "normal", v);
            }
            "scene/motion_vector" => {
                self.uses_motion_vector = true;
                let vel = self.next_var("mv");
                self.emit_if(
                    "MOTION_VECTOR_PREPASS",
                    &[format!(
                        "let {vel} = prepass_utils::prepass_motion_vector(in.position, 0u);"
                    )],
                    &[format!("let {vel} = vec2<f32>(0.0, 0.0);")],
                );
                self.set_out(id, "velocity", vel.clone());
                self.set_out(id, "speed", format!("length({vel})"));
            }
            "scene/refraction_uv_offset" => {
                let n = self.input(node, "normal");
                let s = self.input(node, "strength");
                let v = self.next_var("refuv");
                self.emit(format!("    let {v} = ({n}).xy * {s};"));
                self.set_out(id, "offset", v);
            }
            "scene/screen_uv" => {
                let v = self.next_var("suv");
                // view.viewport = (x, y, width, height) in physical pixels
                self.emit(format!(
                    "    let {v} = (in.position.xy - view.viewport.xy) / view.viewport.zw;"
                ));
                self.set_out(id, "uv", v);
            }
            "scene/scene_color" => {
                // Samples Bevy's built-in `view_transmission_texture` — the
                // scene color grab that Bevy populates between opaque and
                // transparent phases for its transmission pipeline.
                //
                // IMPORTANT: this texture is only populated when Bevy actually
                // runs a transmissive pass, which it does when there are
                // materials with PBR transmission > 0 in the scene. If none
                // exists, this returns black (or stale previous content).
                // For reliable "sky-in-refraction", use `scene/env_map_sample`
                // instead — that samples the env cubemap directly and works
                // regardless of transmission pipeline state.
                //
                // Since Bevy 0.20 the texture is not even bound unless the
                // pipeline sets `VIEW_TRANSMISSION_TEXTURE`, which needs the
                // camera to carry `ScreenSpaceTransmission`. Without it the
                // node reads black instead of failing to compile.
                self.uses_transmission = true;
                let uv = self.input(node, "uv");
                let v = self.next_var("scenec");
                self.emit_if(
                    "VIEW_TRANSMISSION_TEXTURE",
                    &[format!(
                        "let {v} = textureSample(view_transmission_texture, view_transmission_sampler, {uv});"
                    )],
                    &[format!("let {v} = vec4<f32>(0.0, 0.0, 0.0, 1.0);")],
                );
                self.set_out(id, "color", v.clone());
                self.set_out(id, "rgb", format!("{v}.rgb"));
            }
            "scene/env_map_sample" => {
                self.uses_env_map = true;
                let dir = self.input(node, "direction");
                let mip = self.input(node, "mip_level");
                let v = self.next_var("env");
                // No ENVIRONMENT_MAP means no binding at all, so a camera with
                // no environment map light needs the fallback below or the
                // material fails to compile for it.
                self.emit_env_map_sample(&v, &format!("normalize({dir})"), &mip);
                self.set_out(id, "color", v.clone());
                self.set_out(id, "rgb", format!("{v}.rgb"));
            }
            "scene/env_map_reflect" => {
                self.uses_env_map = true;
                let n = self.input(node, "normal");
                let mip = self.input(node, "mip_level");
                let v = self.next_var("envr");
                // view_dir points FROM fragment TO camera; reflect incoming
                // (negated view_dir) around the surface normal to get the
                // outgoing reflection direction.
                self.emit(format!(
                    "    let {v}_vd = normalize(view.world_position.xyz - in.world_position.xyz);"
                ));
                self.emit(format!(
                    "    let {v}_rd = reflect(-{v}_vd, normalize({n}));"
                ));
                // Same fallback as `scene/env_map_sample`: see there.
                self.emit_env_map_sample(&v, &format!("{v}_rd"), &mip);
                self.set_out(id, "color", v.clone());
                self.set_out(id, "rgb", format!("{v}.rgb"));
            }
            unknown => self.unknown_node(unknown),
        }
    }

    /// `let {v}` = the environment map sampled along `dir`, or black when the
    /// view has none.
    ///
    /// Three ways, because Bevy binds the map two different ways: an array
    /// when light probes are batched, a single cube otherwise. The imports in
    /// `emit_ext_shader_header` carry the same two conditions.
    fn emit_env_map_sample(&mut self, v: &str, dir: &str, mip: &str) {
        self.emit(format!(
            "    @if(ENVIRONMENT_MAP && MULTIPLE_LIGHT_PROBES_IN_ARRAY) let {v} = textureSampleLevel(specular_environment_maps[0], environment_map_sampler, {dir}, {mip});"
        ));
        self.emit(format!(
            "    @if(ENVIRONMENT_MAP && !MULTIPLE_LIGHT_PROBES_IN_ARRAY) let {v} = textureSampleLevel(specular_environment_map, environment_map_sampler, {dir}, {mip});"
        ));
        self.emit(format!("    @if(!ENVIRONMENT_MAP) let {v} = vec4<f32>(0.0, 0.0, 0.0, 1.0);"));
    }
}
