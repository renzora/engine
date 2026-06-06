//! Shader `@param` re-injection helpers shared by the native shader panels.
//!
//! The egui code-editor panel that previously lived here was removed during the
//! egui to bevy_ui migration; only the param-injection logic consumed by the
//! native properties panel remains.

use bevy::prelude::*;

use crate::ShaderEditorState;

/// Re-inject `@param` constants into the base WGSL using current param values.
/// Called when the properties panel edits a value (avoids full re-transpilation).
pub fn reapply_params(world: &mut World) {
    let mut state = world.resource_mut::<ShaderEditorState>();
    let Some(ref base_wgsl) = state.base_wgsl else {
        return;
    };
    let final_wgsl = inject_params_into_wgsl(base_wgsl, &state.shader_file.params);
    state.compiled_wgsl = Some(final_wgsl);
}

fn inject_params_into_wgsl(
    base_wgsl: &str,
    params: &std::collections::HashMap<String, renzora_shader::file::ShaderParam>,
) -> String {
    let param_block = renzora_shader::file::params_to_wgsl(params);
    if param_block.is_empty() {
        base_wgsl.to_string()
    } else {
        renzora_shader::registry::inject_param_constants(base_wgsl.to_string(), &param_block)
    }
}
