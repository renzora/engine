//! `texture/*` node emitters.
//!
//! Every 2D sampler allocates its own binding slot; the cubemap, array and
//! volume samplers share one fixed binding each, which is why they set a
//! `uses_*` flag rather than incrementing a counter.
//!
//! The normal-map arm carries the most history: it derives Z from XY rather
//! than reading blue (the import pipeline bakes normal maps to two-channel
//! `Bc5RgUnorm`, where blue reads 0 and decodes to a normal pointing into the
//! surface), and it maps the result through the mikktspace TBN because the
//! Surface Output `normal` pin is world-space.

use super::super::super::graph::{MaterialDomain, MaterialNode, NodeId, PinValue};
use super::super::ctx::Ctx;
use super::super::{TextureBinding, TextureKind};

impl Ctx<'_> {
    pub(crate) fn gen_texture_node(&mut self, node: &MaterialNode, id: NodeId) {
        match node.node_type.as_str() {
            "texture/sample" => {
                // Use in.uv when UV pin is unconnected (most common case)
                let uv = if self.graph.connection_to(node.id, "uv").is_some() {
                    self.input(node, "uv")
                } else {
                    "mat_uv".to_string()
                };
                let path = node
                    .input_values
                    .get("texture")
                    .and_then(|v| {
                        if let PinValue::TexturePath(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();

                let slot = self.next_texture_binding;
                let tex_name = format!("texture_{slot}");
                self.next_texture_binding += 1;

                self.texture_bindings.push(TextureBinding {
                    name: tex_name.clone(),
                    binding: slot,
                    asset_path: path,
                    kind: TextureKind::D2,
                });

                let v = self.next_var("tex");
                let sample = self.sample_call(&tex_name, &uv);
                self.emit(format!("    let {v} = {sample};"));
                self.set_out(id, "color", v.clone());
                self.set_out(id, "rgb", format!("{v}.rgb"));
                self.set_out(id, "r", format!("{v}.r"));
                self.set_out(id, "g", format!("{v}.g"));
                self.set_out(id, "b", format!("{v}.b"));
                self.set_out(id, "a", format!("{v}.a"));
            }

            "texture/sample_normal" => {
                let uv = if self.graph.connection_to(node.id, "uv").is_some() {
                    self.input(node, "uv")
                } else {
                    "mat_uv".to_string()
                };
                let strength = self.input(node, "strength");
                let path = node
                    .input_values
                    .get("texture")
                    .and_then(|v| {
                        if let PinValue::TexturePath(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();

                let slot = self.next_texture_binding;
                let tex_name = format!("texture_{slot}");
                self.next_texture_binding += 1;

                self.texture_bindings.push(TextureBinding {
                    name: tex_name.clone(),
                    binding: slot,
                    asset_path: path,
                    kind: TextureKind::D2,
                });

                let flip = self.input(node, "flip_green");
                let raw = self.next_var("nraw");
                let nxy = self.next_var("nxy");
                let nt = self.next_var("ntan");
                let n = self.next_var("nmap");
                let sample = self.sample_call(&tex_name, &uv);
                // Read X and Y only and *derive* Z, rather than trusting the
                // blue channel. The import pipeline bakes normal maps to
                // `Bc5RgUnorm` — two channels, no blue — because that is the
                // right GPU format for them. Sampling `.b` off one of those
                // returns 0, which decodes to z = -1: a normal pointing
                // straight *into* the surface, so the model lights inside-out.
                // Bevy hits the same problem on `StandardMaterial` and solves it
                // with its `TWO_COMPONENT_NORMAL_MAP` flag; codegen has no
                // material flags to consult and does not need them, because a
                // tangent-space normal is a unit vector with z > 0 and Z is
                // therefore recoverable from XY — exactly, for a three-channel
                // map just as much as for a two-channel one.
                self.emit(format!("    let {raw} = {sample}.rg * 2.0 - 1.0;"));
                // DirectX normal maps store green inverted relative to OpenGL.
                // Negating Y at decode time is the whole difference; without it
                // the surface lights as though every bump were a dent, and only
                // along one axis, which reads as "subtly wrong" rather than as
                // an obvious error.
                self.emit(format!(
                    "    let {nxy} = {raw} * {strength} * vec2<f32>(1.0, select(1.0, -1.0, {flip}));"
                ));
                // `max(0.0, ..)` because a strength above 1 can push the pair
                // outside the unit disc, and `sqrt` of a negative is NaN — one
                // NaN normal poisons the whole lighting result.
                self.emit(format!(
                    "    let {nt} = vec3<f32>({nxy}, sqrt(max(0.0, 1.0 - dot({nxy}, {nxy}))));"
                ));

                // That decode is a *tangent-space* normal, but the Surface
                // Output `normal` pin is world-space — which is the whole reason
                // `normal_from_height` has a separate `world_normal_from_height`
                // sibling. Handing the tangent-space vector straight to
                // `pbr_input.N` makes a flat region of a normal map, (0,0,1),
                // become world +Z: a floor claiming to face sideways. Half of
                // every light then lands on the wrong side of `N·L` and is
                // clamped away, which shows up as a spot light lighting a clean
                // half-disc with a hard straight edge, and as the light
                // behaving wrongly under X and Z rotation. Reproduce it by
                // pointing a spot light at any surface with a normal map wired.
                //
                // Only the codegen path is affected: a graph that is just
                // base-colour/normal/AO compiles to a plain StandardMaterial
                // (see `standard_build`), where Bevy applies the TBN itself.
                // Wiring displacement or a standalone roughness texture pushes
                // the same graph off that fast path and onto this code, which is
                // why adding either pin appeared to "break the lighting".
                //
                // Mikktspace, matching `apply_normal_mapping` exactly, so both
                // paths shade identically. With no vertex tangents there is no
                // frame to map through, so fall back to the interpolated vertex
                // normal — what StandardMaterial renders for a mesh that has a
                // normal map but no tangents.
                if self.graph.domain == MaterialDomain::TerrainLayer {
                    // Terrain compiles to `layer_main()`, whose `FakeIn` has no
                    // `world_tangent` and which never imports `pbr_functions`.
                    // Its layer shader consumes the tangent-space value
                    // directly, so that domain stays exactly as it was.
                    self.set_out(id, "normal", nt);
                } else {
                    let tbn = self.next_var("ntbn");
                    self.emit("#ifdef VERTEX_TANGENTS".to_string());
                    self.emit(format!(
                        "    let {tbn} = pbr_functions::calculate_tbn_mikktspace(in.world_normal, in.world_tangent);"
                    ));
                    self.emit(format!("    let {n} = normalize({tbn} * {nt});"));
                    self.emit("#else".to_string());
                    self.emit(format!("    let {n} = normalize(in.world_normal);"));
                    self.emit("#endif".to_string());
                    self.set_out(id, "normal", n);
                }
            }

            "texture/sample_lod" => {
                let uv = if self.graph.connection_to(node.id, "uv").is_some() {
                    self.input(node, "uv")
                } else {
                    "mat_uv".to_string()
                };
                let lod = self.input(node, "lod");
                let path = node
                    .input_values
                    .get("texture")
                    .and_then(|v| {
                        if let PinValue::TexturePath(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();

                let slot = self.next_texture_binding;
                let tex_name = format!("texture_{slot}");
                self.next_texture_binding += 1;

                self.texture_bindings.push(TextureBinding {
                    name: tex_name.clone(),
                    binding: slot,
                    asset_path: path,
                    kind: TextureKind::D2,
                });

                let v = self.next_var("texl");
                self.emit(format!(
                    "    let {v} = textureSampleLevel({tex_name}, texture_sampler, {uv}, {lod});"
                ));
                self.set_out(id, "color", v.clone());
                self.set_out(id, "rgb", format!("{v}.rgb"));
                self.set_out(id, "r", format!("{v}.r"));
                self.set_out(id, "g", format!("{v}.g"));
                self.set_out(id, "b", format!("{v}.b"));
                self.set_out(id, "a", format!("{v}.a"));
            }

            "texture/sample_grad" => {
                let uv = if self.graph.connection_to(node.id, "uv").is_some() {
                    self.input(node, "uv")
                } else {
                    "mat_uv".to_string()
                };
                let ddx = self.input(node, "ddx");
                let ddy = self.input(node, "ddy");
                let path = node
                    .input_values
                    .get("texture")
                    .and_then(|v| {
                        if let PinValue::TexturePath(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();

                let slot = self.next_texture_binding;
                let tex_name = format!("texture_{slot}");
                self.next_texture_binding += 1;

                self.texture_bindings.push(TextureBinding {
                    name: tex_name.clone(),
                    binding: slot,
                    asset_path: path,
                    kind: TextureKind::D2,
                });

                let v = self.next_var("texg");
                self.emit(format!(
                    "    let {v} = textureSampleGrad({tex_name}, texture_sampler, {uv}, {ddx}, {ddy});"
                ));
                self.set_out(id, "color", v.clone());
                self.set_out(id, "rgb", format!("{v}.rgb"));
                self.set_out(id, "r", format!("{v}.r"));
                self.set_out(id, "g", format!("{v}.g"));
                self.set_out(id, "b", format!("{v}.b"));
                self.set_out(id, "a", format!("{v}.a"));
            }

            "texture/sample_cubemap" => {
                self.uses_cube_0 = true;
                let dir = self.input(node, "direction");
                let lod = self.input(node, "lod");
                let path = node
                    .input_values
                    .get("texture")
                    .and_then(|v| {
                        if let PinValue::TexturePath(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();
                if !path.is_empty() {
                    self.texture_bindings.push(TextureBinding {
                        name: "cube_0".to_string(),
                        binding: 0,
                        asset_path: path,
                        kind: TextureKind::Cube,
                    });
                }
                let v = self.next_var("cubes");
                self.emit(format!("    let {v} = textureSampleLevel(cube_0, texture_sampler, normalize({dir}), {lod});"));
                self.set_out(id, "color", v.clone());
                self.set_out(id, "rgb", format!("{v}.rgb"));
                self.set_out(id, "a", format!("{v}.a"));
            }

            "texture/sample_2d_array" => {
                self.uses_array_0 = true;
                let uv = if self.graph.connection_to(node.id, "uv").is_some() {
                    self.input(node, "uv")
                } else {
                    "mat_uv".to_string()
                };
                let layer = self.input(node, "layer");
                let path = node
                    .input_values
                    .get("texture")
                    .and_then(|v| {
                        if let PinValue::TexturePath(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();
                if !path.is_empty() {
                    self.texture_bindings.push(TextureBinding {
                        name: "array_0".to_string(),
                        binding: 0,
                        asset_path: path,
                        kind: TextureKind::D2Array,
                    });
                }
                let v = self.next_var("tarr");
                self.emit(format!("    let {v} = textureSample(array_0, texture_sampler, {uv}, i32(round({layer})));"));
                self.set_out(id, "color", v.clone());
                self.set_out(id, "rgb", format!("{v}.rgb"));
                self.set_out(id, "r", format!("{v}.r"));
                self.set_out(id, "g", format!("{v}.g"));
                self.set_out(id, "b", format!("{v}.b"));
                self.set_out(id, "a", format!("{v}.a"));
            }

            "texture/sample_3d" => {
                self.uses_volume_0 = true;
                let uvw = self.input(node, "uvw");
                let path = node
                    .input_values
                    .get("texture")
                    .and_then(|v| {
                        if let PinValue::TexturePath(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();
                if !path.is_empty() {
                    self.texture_bindings.push(TextureBinding {
                        name: "volume_0".to_string(),
                        binding: 0,
                        asset_path: path,
                        kind: TextureKind::D3,
                    });
                }
                let v = self.next_var("t3d");
                self.emit(format!(
                    "    let {v} = textureSample(volume_0, texture_sampler, {uvw});"
                ));
                self.set_out(id, "color", v.clone());
                self.set_out(id, "rgb", format!("{v}.rgb"));
                self.set_out(id, "r", format!("{v}.r"));
                self.set_out(id, "g", format!("{v}.g"));
                self.set_out(id, "b", format!("{v}.b"));
                self.set_out(id, "a", format!("{v}.a"));
            }

            "texture/triplanar" => {
                let scale = self.input(node, "scale");
                let sharpness = self.input(node, "sharpness");
                let path = node
                    .input_values
                    .get("texture")
                    .and_then(|v| {
                        if let PinValue::TexturePath(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();

                let slot = self.next_texture_binding;
                let tex_name = format!("texture_{slot}");
                self.next_texture_binding += 1;

                self.texture_bindings.push(TextureBinding {
                    name: tex_name.clone(),
                    binding: slot,
                    asset_path: path,
                    kind: TextureKind::D2,
                });

                let w = self.next_var("tri_w");
                let v = self.next_var("tri");
                // `var`, not `let` — WGSL has no shadowing, so the next line has to assign, not redeclare.
                self.emit(format!(
                    "    var {w} = pow(abs(in.world_normal), vec3<f32>({sharpness}));"
                ));
                self.emit(format!("    {w} = {w} / ({w}.x + {w}.y + {w}.z);"));
                // Brackets, or `.yz` below attaches to the last operand instead of the whole product.
                let p = format!("(in.world_position.xyz * {scale})");
                self.emit(format!("    let {v} = textureSample({tex_name}, texture_sampler, {p}.yz) * {w}.x + textureSample({tex_name}, texture_sampler, {p}.xz) * {w}.y + textureSample({tex_name}, texture_sampler, {p}.xy) * {w}.z;"));
                self.set_out(id, "color", v.clone());
                self.set_out(id, "rgb", format!("{v}.rgb"));
            }

            unknown => self.unknown_node(unknown),
        }
    }
}
