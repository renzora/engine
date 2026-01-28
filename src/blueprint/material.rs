//! Blueprint Material - Shader generation from material blueprints
//!
//! This module provides tools for compiling material blueprints to WGSL shader code.
//! The generated shaders can be used with Bevy's custom material system.
//!
//! Note: Full integration with Bevy's Material trait requires using the generated
//! shader code in a custom material implementation. This module focuses on the
//! compilation step.

use bevy::prelude::*;
use std::path::PathBuf;

use super::{BlueprintGraph, generate_wgsl_code};

/// Maximum number of textures supported in a blueprint material
pub const MAX_BLUEPRINT_TEXTURES: usize = 4;

/// Result of compiling a blueprint to a material
#[derive(Debug, Clone)]
pub struct CompiledBlueprintMaterial {
    /// Name of the material (from the blueprint)
    pub name: String,
    /// The generated WGSL shader code
    pub shader_code: String,
    /// Texture asset paths needed by the shader
    pub texture_paths: Vec<String>,
    /// Whether this is a PBR material (vs unlit)
    pub is_pbr: bool,
    /// Any errors during compilation
    pub errors: Vec<String>,
    /// Any warnings during compilation
    pub warnings: Vec<String>,
}

impl CompiledBlueprintMaterial {
    /// Returns true if compilation was successful (no errors)
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    /// Returns true if there were any warnings
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    /// Get the shader code, or None if there were errors
    pub fn shader(&self) -> Option<&str> {
        if self.is_ok() {
            Some(&self.shader_code)
        } else {
            None
        }
    }

    /// Save the generated shader to a file
    pub fn save_shader(&self, path: &PathBuf) -> std::io::Result<()> {
        std::fs::write(path, &self.shader_code)
    }
}

/// Compile a material blueprint graph to shader code
///
/// This function takes a blueprint graph and generates the WGSL shader code.
///
/// # Example
/// ```ignore
/// let graph = BlueprintGraph::new_material("my_material");
/// // ... add nodes and connections ...
/// let result = compile_material_blueprint(&graph);
/// if result.is_ok() {
///     println!("Generated shader:\n{}", result.shader_code);
/// }
/// ```
pub fn compile_material_blueprint(graph: &BlueprintGraph) -> CompiledBlueprintMaterial {
    let result = generate_wgsl_code(graph);

    CompiledBlueprintMaterial {
        name: graph.name.clone(),
        shader_code: result.fragment_shader,
        texture_paths: result.texture_bindings.iter().map(|b| b.asset_path.clone()).collect(),
        is_pbr: result.is_pbr,
        errors: result.errors,
        warnings: result.warnings,
    }
}

/// Helper to create a StandardMaterial with textures from a compiled blueprint
///
/// This is a simpler alternative to full custom material support - it creates
/// a StandardMaterial and loads any textures referenced in the blueprint.
///
/// Note: This doesn't actually apply the generated shader, it just sets up
/// the textures. For full custom shader support, you'll need to implement
/// a custom Material type.
pub fn create_standard_material_with_textures(
    compiled: &CompiledBlueprintMaterial,
    asset_server: &AssetServer,
) -> StandardMaterial {
    let mut material = StandardMaterial::default();

    // Load the first texture as the base color texture if available
    if let Some(path) = compiled.texture_paths.first() {
        if !path.is_empty() {
            material.base_color_texture = Some(asset_server.load(path));
        }
    }

    material
}

/// Metadata for a material blueprint file
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MaterialBlueprintMetadata {
    /// Name of the material
    pub name: String,
    /// Whether this is a PBR material
    pub is_pbr: bool,
    /// List of texture asset paths used
    pub textures: Vec<String>,
    /// Path to the generated shader file (if saved)
    pub shader_path: Option<PathBuf>,
}

impl MaterialBlueprintMetadata {
    /// Create metadata from a compiled material
    pub fn from_compiled(compiled: &CompiledBlueprintMaterial) -> Self {
        Self {
            name: compiled.name.clone(),
            is_pbr: compiled.is_pbr,
            textures: compiled.texture_paths.clone(),
            shader_path: None,
        }
    }
}

/// Save a compiled material's shader and metadata to files
///
/// Creates two files:
/// - `{name}.wgsl` - The shader code
/// - `{name}.material.json` - Metadata including texture paths
pub fn save_compiled_material(
    compiled: &CompiledBlueprintMaterial,
    output_dir: &PathBuf,
) -> std::io::Result<()> {
    if !compiled.is_ok() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Cannot save material with compilation errors: {:?}", compiled.errors),
        ));
    }

    // Ensure output directory exists
    std::fs::create_dir_all(output_dir)?;

    // Save shader
    let shader_filename = format!("{}.wgsl", sanitize_filename(&compiled.name));
    let shader_path = output_dir.join(&shader_filename);
    std::fs::write(&shader_path, &compiled.shader_code)?;

    // Save metadata
    let mut metadata = MaterialBlueprintMetadata::from_compiled(compiled);
    metadata.shader_path = Some(PathBuf::from(&shader_filename));

    let metadata_filename = format!("{}.material.json", sanitize_filename(&compiled.name));
    let metadata_path = output_dir.join(metadata_filename);
    let metadata_json = serde_json::to_string_pretty(&metadata)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(metadata_path, metadata_json)?;

    info!("Saved material '{}' to {:?}", compiled.name, shader_path);
    Ok(())
}

/// Sanitize a filename to remove invalid characters
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blueprint::BlueprintGraph;

    #[test]
    fn test_compile_empty_material() {
        let graph = BlueprintGraph::new_material("test");
        let result = compile_material_blueprint(&graph);
        assert!(!result.is_ok());
        assert!(result.errors[0].contains("output node"));
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("my material"), "my_material");
        assert_eq!(sanitize_filename("test/path"), "test_path");
        assert_eq!(sanitize_filename("valid_name-123"), "valid_name-123");
    }
}
