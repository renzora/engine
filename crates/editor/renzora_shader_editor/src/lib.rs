//! Shader Editor — code-based shader authoring with multi-language support.

pub mod code_panel;
pub mod compiler_log;
pub mod properties;
pub mod preview;

use bevy::prelude::*;
use renzora_editor::AppEditorExt;
use renzora_shader::backend::ShaderCompileError;
use renzora_shader::file::ShaderFile;

/// Persistent editor state for the shader code editor.
#[derive(Resource)]
pub struct ShaderEditorState {
    /// The shader file currently being edited.
    pub shader_file: ShaderFile,
    /// File path if loaded from / saved to disk.
    pub file_path: Option<String>,
    /// Dirty flag — source has been modified since last save.
    pub is_modified: bool,
    /// Transpiled WGSL before `@param` constant injection.
    /// Stored so param value changes can re-inject without re-transpiling.
    pub base_wgsl: Option<String>,
    /// Last compiled WGSL output (for preview), with `@param` constants injected.
    pub compiled_wgsl: Option<String>,
    /// Whether the compiled shader is compatible with CodeShaderMaterial preview.
    /// Shaders with custom material bindings (textures, samplers) can't preview.
    pub preview_compatible: bool,
    /// Compilation errors (shown in UI).
    pub compile_errors: Vec<ShaderCompileError>,
    /// Whether to auto-compile on every keystroke.
    pub auto_compile: bool,
    /// Which mesh to display in the shader preview.
    pub preview_mesh: preview::PreviewMesh,
}

impl Default for ShaderEditorState {
    fn default() -> Self {
        Self {
            shader_file: ShaderFile::default(),
            file_path: None,
            is_modified: false,
            base_wgsl: None,
            compiled_wgsl: None,
            compile_errors: Vec::new(),
            auto_compile: true,
            preview_compatible: true,
            preview_mesh: preview::PreviewMesh::default(),
        }
    }
}

pub struct ShaderEditorPlugin;

impl Plugin for ShaderEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ShaderEditorPlugin");
        app.init_resource::<ShaderEditorState>();
        app.add_plugins(preview::ShaderPreviewPlugin);
        app.register_panel(code_panel::ShaderCodePanel::default());
        app.register_panel(compiler_log::ShaderCompilerLogPanel);
        app.register_panel(properties::ShaderPropertiesPanel);
        app.register_panel(preview::ShaderPreviewPanel);
    }
}
