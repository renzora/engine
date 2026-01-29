//! Custom Render Pipeline for Blueprint Materials
//!
//! This module provides shader compilation and material rendering for blueprint materials.
//!
//! ## Features
//!
//! - Dynamic shader compilation from blueprint graphs to WGSL
//! - Shader caching for performance
//! - PBR value extraction for visual preview
//! - Shader export functionality for production use
//! - Support for textures, procedural patterns, and animations
//!
//! ## How It Works
//!
//! 1. Blueprint graphs are compiled to WGSL fragment shaders
//! 2. PBR values (base color, metallic, roughness, etc.) are extracted from the graph
//! 3. In the editor, StandardMaterial provides visual preview with extracted PBR values
//! 4. Generated shaders can be exported for use with custom materials in production

use bevy::prelude::*;
use bevy::render::render_resource::*;
use std::collections::HashMap;
use std::path::PathBuf;

use super::{BlueprintGraph, generate_wgsl_code, extract_pbr_values, sanitize_filename};

// ============================================================================
// Constants
// ============================================================================

/// Maximum number of textures per blueprint material
pub const MAX_TEXTURES: usize = 4;

// ============================================================================
// Components
// ============================================================================

/// Component that marks an entity as using a blueprint material.
/// This is the main interface for applying blueprint materials to entities.
#[derive(Component, Clone, Debug)]
pub struct BlueprintMaterialInstance {
    /// Path to the source blueprint file
    pub blueprint_path: String,

    /// Handle to the fragment shader (generated from blueprint)
    pub fragment_shader: Handle<Shader>,

    /// Base color (RGBA)
    pub base_color: LinearRgba,

    /// Metallic factor (0.0 - 1.0)
    pub metallic: f32,

    /// Roughness factor (0.0 - 1.0)
    pub roughness: f32,

    /// Emissive color (RGB, alpha is intensity)
    pub emissive: LinearRgba,

    /// Ambient occlusion factor
    pub ao: f32,

    /// Alpha cutoff for alpha mask mode
    pub alpha_cutoff: f32,

    /// Texture handles (base color, normal, metallic/roughness, emissive)
    pub textures: [Option<Handle<Image>>; MAX_TEXTURES],

    /// Alpha mode (Opaque, Mask, Blend)
    pub alpha_mode: AlphaMode,

    /// Whether this material uses double-sided rendering
    pub double_sided: bool,

    /// Whether this material has procedural nodes (for editor display)
    pub has_procedural: bool,

    /// The generated WGSL code (stored for debugging/export)
    pub shader_code: String,
}

impl Default for BlueprintMaterialInstance {
    fn default() -> Self {
        Self {
            blueprint_path: String::new(),
            fragment_shader: Handle::default(),
            base_color: LinearRgba::WHITE,
            metallic: 0.0,
            roughness: 0.5,
            emissive: LinearRgba::BLACK,
            ao: 1.0,
            alpha_cutoff: 0.5,
            textures: [None, None, None, None],
            alpha_mode: AlphaMode::Opaque,
            double_sided: false,
            has_procedural: false,
            shader_code: String::new(),
        }
    }
}

/// Marker component for entities that have been set up for blueprint material rendering
#[derive(Component)]
pub struct BlueprintMaterialSetup;

// ============================================================================
// GPU Data Structures
// ============================================================================

/// Uniform buffer data for blueprint materials
#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub struct BlueprintMaterialUniform {
    /// Base color (RGBA)
    pub base_color: Vec4,
    /// Emissive color (RGB) + intensity (A)
    pub emissive: Vec4,
    /// Metallic (x), Roughness (y), AO (z), Alpha cutoff (w)
    pub properties: Vec4,
    /// Flags: texture presence bits, double-sided, etc.
    pub flags: u32,
    /// Padding for alignment
    pub _padding: [u32; 3],
}

impl BlueprintMaterialUniform {
    pub fn from_instance(instance: &BlueprintMaterialInstance) -> Self {
        let mut flags = 0u32;
        if instance.textures[0].is_some() { flags |= 1 << 0; } // Base color texture
        if instance.textures[1].is_some() { flags |= 1 << 1; } // Normal map
        if instance.textures[2].is_some() { flags |= 1 << 2; } // Metallic/roughness
        if instance.textures[3].is_some() { flags |= 1 << 3; } // Emissive
        if instance.double_sided { flags |= 1 << 4; }

        Self {
            base_color: Vec4::new(
                instance.base_color.red,
                instance.base_color.green,
                instance.base_color.blue,
                instance.base_color.alpha,
            ),
            emissive: Vec4::new(
                instance.emissive.red,
                instance.emissive.green,
                instance.emissive.blue,
                instance.emissive.alpha,
            ),
            properties: Vec4::new(
                instance.metallic,
                instance.roughness,
                instance.ao,
                instance.alpha_cutoff,
            ),
            flags,
            _padding: [0; 3],
        }
    }
}

// ============================================================================
// Plugin
// ============================================================================

/// Plugin that sets up blueprint material rendering.
///
/// This plugin provides:
/// - Shader compilation and caching
/// - Material instance creation from blueprints
/// - Visual preview using StandardMaterial with extracted PBR values
/// - Shader export functionality
pub struct BlueprintMaterialRenderPlugin;

impl Plugin for BlueprintMaterialRenderPlugin {
    fn build(&self, app: &mut App) {
        // Main world resources and systems
        app.init_resource::<BlueprintShaderCache>()
            .add_systems(Update, (
                setup_blueprint_material_entities,
                update_blueprint_material_entities,
            ));
    }
}

// ============================================================================
// Main World Resources
// ============================================================================

/// Cache of compiled blueprint shaders in the main world
#[derive(Resource, Default)]
pub struct BlueprintShaderCache {
    /// Map from blueprint path to compiled shader data
    pub compiled: HashMap<String, CompiledBlueprintShader>,
}

/// Compiled shader data
#[derive(Clone)]
pub struct CompiledBlueprintShader {
    /// Handle to the fragment shader asset
    pub shader_handle: Handle<Shader>,
    /// The generated WGSL code
    pub shader_code: String,
    /// Texture paths referenced by the shader
    pub texture_paths: Vec<String>,
    /// Whether this is a PBR shader
    pub is_pbr: bool,
}

impl BlueprintShaderCache {
    /// Compile a blueprint and cache the shader
    pub fn compile(
        &mut self,
        blueprint_path: &str,
        graph: &BlueprintGraph,
        shaders: &mut Assets<Shader>,
    ) -> Result<CompiledBlueprintShader, String> {
        // Return cached if available
        if let Some(compiled) = self.compiled.get(blueprint_path) {
            return Ok(compiled.clone());
        }

        // Generate WGSL code
        let result = generate_wgsl_code(graph);

        if !result.errors.is_empty() {
            return Err(format!("Shader compilation errors: {}", result.errors.join(", ")));
        }

        // Log warnings
        for warning in &result.warnings {
            warn!("Blueprint shader warning: {}", warning);
        }

        // Create shader asset
        let shader = Shader::from_wgsl(
            result.fragment_shader.clone(),
            format!("blueprint://{}", blueprint_path),
        );
        let shader_handle = shaders.add(shader);

        let compiled = CompiledBlueprintShader {
            shader_handle,
            shader_code: result.fragment_shader,
            texture_paths: result.texture_bindings.iter().map(|b| b.asset_path.clone()).collect(),
            is_pbr: result.is_pbr,
        };

        self.compiled.insert(blueprint_path.to_string(), compiled.clone());
        info!("Compiled blueprint shader: {} ({} texture bindings)",
              blueprint_path, result.texture_bindings.len());

        Ok(compiled)
    }

    /// Get a compiled shader by path
    pub fn get(&self, blueprint_path: &str) -> Option<&CompiledBlueprintShader> {
        self.compiled.get(blueprint_path)
    }

    /// Invalidate cache for a specific blueprint (for hot-reloading)
    pub fn invalidate(&mut self, blueprint_path: &str) {
        self.compiled.remove(blueprint_path);
    }

    /// Clear all cached shaders
    pub fn clear(&mut self) {
        self.compiled.clear();
    }
}

// ============================================================================
// Main World Systems
// ============================================================================

/// System to set up entities with BlueprintMaterialInstance for rendering.
/// Creates StandardMaterial for visual preview with extracted PBR values.
fn setup_blueprint_material_entities(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    query: Query<(Entity, &BlueprintMaterialInstance), Without<BlueprintMaterialSetup>>,
) {
    for (entity, instance) in query.iter() {
        // Create StandardMaterial for editor preview using extracted PBR values
        let mut material = StandardMaterial {
            base_color: Color::LinearRgba(instance.base_color),
            metallic: instance.metallic,
            perceptual_roughness: instance.roughness,
            emissive: instance.emissive.into(),
            alpha_mode: instance.alpha_mode,
            double_sided: instance.double_sided,
            ..default()
        };

        // Apply textures if available
        if let Some(ref tex) = instance.textures[0] {
            material.base_color_texture = Some(tex.clone());
        }
        if let Some(ref tex) = instance.textures[1] {
            material.normal_map_texture = Some(tex.clone());
        }
        if let Some(ref tex) = instance.textures[2] {
            material.metallic_roughness_texture = Some(tex.clone());
        }
        if let Some(ref tex) = instance.textures[3] {
            material.emissive_texture = Some(tex.clone());
        }

        let material_handle = materials.add(material);

        commands.entity(entity)
            .insert(MeshMaterial3d(material_handle))
            .insert(BlueprintMaterialSetup);

        if instance.has_procedural {
            info!("Blueprint material '{}' has procedural nodes - shader code stored for export",
                  instance.blueprint_path);
        }
    }
}

/// System to update materials when BlueprintMaterialInstance changes.
/// This allows for real-time preview updates when editing blueprint materials.
fn update_blueprint_material_entities(
    mut materials: ResMut<Assets<StandardMaterial>>,
    query: Query<(&BlueprintMaterialInstance, &MeshMaterial3d<StandardMaterial>), Changed<BlueprintMaterialInstance>>,
) {
    for (instance, material_handle) in query.iter() {
        if let Some(material) = materials.get_mut(&material_handle.0) {
            // Update material properties
            material.base_color = Color::LinearRgba(instance.base_color);
            material.metallic = instance.metallic;
            material.perceptual_roughness = instance.roughness;
            material.emissive = instance.emissive.into();
            material.alpha_mode = instance.alpha_mode;
            material.double_sided = instance.double_sided;

            // Update textures
            material.base_color_texture = instance.textures[0].clone();
            material.normal_map_texture = instance.textures[1].clone();
            material.metallic_roughness_texture = instance.textures[2].clone();
            material.emissive_texture = instance.textures[3].clone();
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a BlueprintMaterialInstance from a blueprint graph
pub fn create_blueprint_material_instance(
    blueprint_path: &str,
    graph: &BlueprintGraph,
    shader_cache: &mut BlueprintShaderCache,
    shaders: &mut Assets<Shader>,
    asset_server: &AssetServer,
    project_path: Option<&PathBuf>,
) -> Result<BlueprintMaterialInstance, String> {
    // Compile the shader
    let compiled = shader_cache.compile(blueprint_path, graph, shaders)?;

    // Extract PBR values from the graph
    let pbr = extract_pbr_values(graph);

    // Load textures
    let mut textures: [Option<Handle<Image>>; MAX_TEXTURES] = [None, None, None, None];
    for (i, path) in compiled.texture_paths.iter().enumerate().take(MAX_TEXTURES) {
        if !path.is_empty() {
            let resolved = if std::path::Path::new(path).is_absolute() {
                PathBuf::from(path)
            } else if let Some(project) = project_path {
                project.join(path)
            } else {
                PathBuf::from(path)
            };
            textures[i] = Some(asset_server.load(resolved));
        }
    }

    Ok(BlueprintMaterialInstance {
        blueprint_path: blueprint_path.to_string(),
        fragment_shader: compiled.shader_handle.clone(),
        base_color: LinearRgba::new(
            pbr.base_color[0],
            pbr.base_color[1],
            pbr.base_color[2],
            pbr.base_color[3],
        ),
        metallic: pbr.metallic,
        roughness: pbr.roughness,
        emissive: LinearRgba::new(pbr.emissive[0], pbr.emissive[1], pbr.emissive[2], 1.0),
        ao: 1.0,
        alpha_cutoff: 0.5,
        textures,
        alpha_mode: AlphaMode::Opaque,
        double_sided: false,
        has_procedural: pbr.has_procedural_nodes,
        shader_code: compiled.shader_code.clone(),
    })
}

/// Save a generated shader to a file
pub fn save_generated_shader(
    graph: &BlueprintGraph,
    output_dir: &PathBuf,
) -> Result<PathBuf, String> {
    let result = generate_wgsl_code(graph);

    if !result.errors.is_empty() {
        return Err(result.errors.join(", "));
    }

    // Ensure directory exists
    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    // Write shader file
    let filename = format!("{}.wgsl", sanitize_filename(&graph.name));
    let path = output_dir.join(&filename);
    std::fs::write(&path, &result.fragment_shader)
        .map_err(|e| format!("Failed to write shader: {}", e))?;

    info!("Saved generated shader: {:?}", path);
    Ok(path)
}

/// Export all blueprint materials in a directory to shader files
pub fn export_project_shaders(
    blueprints_dir: &PathBuf,
    output_dir: &PathBuf,
) -> Result<Vec<PathBuf>, String> {
    use super::BlueprintFile;

    let mut exported = Vec::new();

    // Read directory and find material blueprints
    let entries = std::fs::read_dir(blueprints_dir)
        .map_err(|e| format!("Failed to read blueprints directory: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();

        // Check if it's a material blueprint
        if path.extension().map_or(false, |ext| ext == "material_bp") {
            // Load and compile
            match BlueprintFile::load(&path) {
                Ok(blueprint) => {
                    match save_generated_shader(&blueprint.graph, output_dir) {
                        Ok(shader_path) => exported.push(shader_path),
                        Err(e) => warn!("Failed to export {:?}: {}", path, e),
                    }
                }
                Err(e) => warn!("Failed to load {:?}: {}", path, e),
            }
        }
    }

    Ok(exported)
}

/// Get shader code for a compiled blueprint
pub fn get_compiled_shader_code(
    shader_cache: &BlueprintShaderCache,
    blueprint_path: &str,
) -> Option<String> {
    shader_cache.get(blueprint_path).map(|c| c.shader_code.clone())
}

/// Reload a blueprint material by invalidating cache and recompiling
pub fn reload_blueprint_material(
    shader_cache: &mut BlueprintShaderCache,
    blueprint_path: &str,
) {
    shader_cache.invalidate(blueprint_path);
    info!("Invalidated shader cache for: {}", blueprint_path);
}
