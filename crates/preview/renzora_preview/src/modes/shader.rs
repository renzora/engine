//! Shader / Material / Post-Process preview mode.

use bevy::prelude::*;
use renzora_shader::file::{self, ShaderType};
use renzora_shader::registry::ShaderBackendRegistry;
use renzora_shader::runtime::{CodeShaderMaterial, ShaderCache, ShaderUniforms};

use crate::bridge::{PreviewCommand, PreviewCommandQueue};
use crate::scene::PreviewSubject;
use super::PreviewMode;

#[derive(Resource, Default)]
pub struct ShaderPreviewState {
    pub shader_type: Option<ShaderType>,
    pub source: Option<String>,
    pub params: std::collections::HashMap<String, file::ShaderParam>,
}

fn handle_shader_commands(
    mut queue: ResMut<PreviewCommandQueue>,
    mut state: ResMut<ShaderPreviewState>,
    registry: Res<ShaderBackendRegistry>,
    mut cache: ResMut<ShaderCache>,
    mut shaders: ResMut<Assets<Shader>>,
    mut code_materials: ResMut<Assets<CodeShaderMaterial>>,
    mut subject_q: Query<Entity, With<PreviewSubject>>,
    mat_q: Query<&MeshMaterial3d<CodeShaderMaterial>, With<PreviewSubject>>,
    mut commands: Commands,
    mut next_mode: ResMut<NextState<PreviewMode>>,
) {
    let mut remaining = Vec::new();

    for cmd in queue.commands.drain(..) {
        match cmd {
            PreviewCommand::LoadShader(event) => {
                let shader_type = match event.shader_type.to_lowercase().as_str() {
                    "fragment" | "shader" => ShaderType::Fragment,
                    "material" => ShaderType::Material,
                    "postprocess" | "post-process" | "post_process" => ShaderType::PostProcess,
                    _ => ShaderType::Fragment,
                };

                let params = file::extract_params(&event.source);
                let language = file::detect_language(&event.source);

                let wgsl = match registry.transpile(language, &event.source) {
                    Ok(compiled) => {
                        let param_consts = file::params_to_wgsl(&params);
                        if param_consts.is_empty() {
                            compiled
                        } else {
                            renzora_shader::registry::inject_param_constants(compiled, &param_consts)
                        }
                    }
                    Err(err) => {
                        warn!("[preview] Shader compile error: {err}");
                        continue;
                    }
                };

                let label = format!("preview://{language}");
                let shader_handle = cache.get_or_insert(&wgsl, &label, &mut shaders);

                let material = CodeShaderMaterial {
                    uniforms: ShaderUniforms::default(),
                    shader_handle,
                    alpha_mode: AlphaMode::Blend,
                };
                let mat_handle = code_materials.add(material);

                for entity in subject_q.iter_mut() {
                    commands.entity(entity)
                        .remove::<MeshMaterial3d<StandardMaterial>>()
                        .insert(MeshMaterial3d(mat_handle.clone()));
                }

                state.shader_type = Some(shader_type);
                state.source = Some(event.source.clone());
                state.params = params;
                next_mode.set(PreviewMode::Shader);

                info!("[preview] Shader loaded: {language} ({shader_type:?})");
            }
            PreviewCommand::SetParam(event) => {
                if let Some(param) = state.params.get_mut(&event.name) {
                    param.default_value = match event.value {
                        crate::bridge::ParamValueJs::Float(v) => file::ParamValue::Float(v),
                        crate::bridge::ParamValueJs::Vec2(v) => file::ParamValue::Vec2(v),
                        crate::bridge::ParamValueJs::Vec3(v) => file::ParamValue::Vec3(v),
                        crate::bridge::ParamValueJs::Vec4(v) => file::ParamValue::Vec4(v),
                        crate::bridge::ParamValueJs::Color(v) => file::ParamValue::Color(v),
                        crate::bridge::ParamValueJs::Int(v) => file::ParamValue::Int(v),
                        crate::bridge::ParamValueJs::Bool(v) => file::ParamValue::Bool(v),
                    };

                    // Recompile with updated params
                    if let Some(source) = &state.source {
                        let language = file::detect_language(source);
                        if let Ok(compiled) = registry.transpile(language, source) {
                            let param_consts = file::params_to_wgsl(&state.params);
                            let wgsl = if param_consts.is_empty() {
                                compiled
                            } else {
                                renzora_shader::registry::inject_param_constants(compiled, &param_consts)
                            };

                            let label = format!("preview://{language}");
                            let shader_handle = cache.get_or_insert(&wgsl, &label, &mut shaders);

                            for mat_handle in mat_q.iter() {
                                if let Some(mat) = code_materials.get_mut(&mat_handle.0) {
                                    mat.shader_handle = shader_handle.clone();
                                }
                            }
                        }
                    }
                }
            }
            other => remaining.push(other),
        }
    }

    queue.commands = remaining;
}

pub struct ShaderPreviewPlugin;

impl Plugin for ShaderPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShaderPreviewState>()
            .add_systems(Update, handle_shader_commands);
    }
}
