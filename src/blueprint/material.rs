//! Blueprint Material - Custom material system using generated WGSL shaders
//!
//! This module provides a custom Material implementation that uses WGSL shaders
//! generated from material blueprints. This enables full procedural materials
//! like water, animated effects, noise patterns, etc.
//!
//! Architecture:
//! - Each material blueprint generates a WGSL shader file
//! - BlueprintMaterial implements Bevy's Material trait
//! - Shaders are stored in a runtime cache and loaded via asset server
//! - Textures are bound dynamically based on the blueprint configuration

use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::pbr::Material;
use bevy::shader::ShaderRef;
use std::collections::HashMap;
use std::path::PathBuf;

use super::{BlueprintGraph, generate_wgsl_code};

/// Maximum number of textures supported in a blueprint material
pub const MAX_BLUEPRINT_TEXTURES: usize = 4;

// ============================================================================
// Blueprint Material Plugin & Resources
// ============================================================================

/// Plugin for blueprint material support
pub struct BlueprintMaterialPlugin;

impl Plugin for BlueprintMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy::pbr::MaterialPlugin::<BlueprintMaterial>::default())
            .init_resource::<BlueprintMaterialCache>()
            .init_resource::<ActiveBlueprintShader>();
    }
}

/// Resource tracking the currently active blueprint shader
/// This is used because Bevy's Material trait has a static fragment_shader() method
#[derive(Resource)]
pub struct ActiveBlueprintShader {
    /// Handle to the currently active shader
    pub shader_handle: Option<Handle<Shader>>,
    /// Path to the shader file (for hot-reloading)
    pub shader_path: Option<PathBuf>,
}

impl Default for ActiveBlueprintShader {
    fn default() -> Self {
        Self {
            shader_handle: None,
            shader_path: None,
        }
    }
}

/// Cache for compiled blueprint materials and their shader handles
#[derive(Resource, Default)]
pub struct BlueprintMaterialCache {
    /// Map from blueprint path to cached shader data
    pub cache: HashMap<String, CachedBlueprintShader>,
    /// Map from blueprint path to shader handle
    pub shader_handles: HashMap<String, Handle<Shader>>,
}

/// Cached shader data for a blueprint material
#[derive(Clone)]
pub struct CachedBlueprintShader {
    /// The generated WGSL code
    pub shader_code: String,
    /// Path to the saved .wgsl file
    pub shader_path: PathBuf,
    /// Texture paths used by this material
    pub texture_paths: Vec<String>,
    /// Whether this is a PBR material
    pub is_pbr: bool,
}

impl BlueprintMaterialCache {
    /// Get or create cached shader for a blueprint
    pub fn get_or_create(
        &mut self,
        blueprint_path: &str,
        graph: &BlueprintGraph,
        cache_dir: &PathBuf,
    ) -> Result<CachedBlueprintShader, String> {
        // Check if already cached
        if let Some(cached) = self.cache.get(blueprint_path) {
            return Ok(cached.clone());
        }

        // Generate the WGSL code
        let result = generate_wgsl_code(graph);

        if !result.errors.is_empty() {
            return Err(result.errors.join(", "));
        }

        // Create shader cache directory
        let shader_cache_dir = cache_dir.join("shader_cache");
        std::fs::create_dir_all(&shader_cache_dir)
            .map_err(|e| format!("Failed to create shader cache: {}", e))?;

        // Save the shader to a file
        let shader_filename = format!("{}.wgsl", sanitize_filename(&graph.name));
        let shader_path = shader_cache_dir.join(&shader_filename);
        std::fs::write(&shader_path, &result.fragment_shader)
            .map_err(|e| format!("Failed to write shader: {}", e))?;

        info!("Generated shader saved to: {:?}", shader_path);

        let cached = CachedBlueprintShader {
            shader_code: result.fragment_shader,
            shader_path,
            texture_paths: result.texture_bindings.iter().map(|b| b.asset_path.clone()).collect(),
            is_pbr: result.is_pbr,
        };

        self.cache.insert(blueprint_path.to_string(), cached.clone());
        Ok(cached)
    }

    /// Create a shader asset from the cached shader code
    pub fn get_or_create_shader_handle(
        &mut self,
        blueprint_path: &str,
        shaders: &mut Assets<Shader>,
    ) -> Option<Handle<Shader>> {
        // Check if we already have a handle
        if let Some(handle) = self.shader_handles.get(blueprint_path) {
            return Some(handle.clone());
        }

        // Get the cached shader
        let cached = self.cache.get(blueprint_path)?;

        // Create a shader asset from the WGSL code
        let shader = Shader::from_wgsl(
            cached.shader_code.clone(),
            cached.shader_path.to_string_lossy().to_string(),
        );
        let handle = shaders.add(shader);

        self.shader_handles.insert(blueprint_path.to_string(), handle.clone());
        Some(handle)
    }
}

// ============================================================================
// Blueprint Material - Custom Material Implementation
// ============================================================================

/// Custom material that uses generated WGSL shaders from blueprints
///
/// This material supports:
/// - Up to 4 textures bound dynamically
/// - Time-based animations (via globals.time)
/// - All procedural nodes (noise, patterns, etc.)
/// - Full PBR lighting
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct BlueprintMaterial {
    /// Base color multiplier
    #[uniform(0)]
    pub base_color: LinearRgba,

    /// Optional texture 0 (usually base color/albedo)
    #[texture(1)]
    #[sampler(2)]
    pub texture_0: Option<Handle<Image>>,

    /// Optional texture 1 (usually normal map)
    #[texture(3)]
    #[sampler(4)]
    pub texture_1: Option<Handle<Image>>,

    /// Optional texture 2 (usually metallic/roughness)
    #[texture(5)]
    #[sampler(6)]
    pub texture_2: Option<Handle<Image>>,

    /// Optional texture 3 (usually emissive/AO)
    #[texture(7)]
    #[sampler(8)]
    pub texture_3: Option<Handle<Image>>,

    /// Alpha mode for transparency
    pub alpha_mode: AlphaMode,
}

impl Default for BlueprintMaterial {
    fn default() -> Self {
        Self {
            base_color: LinearRgba::WHITE,
            texture_0: None,
            texture_1: None,
            texture_2: None,
            texture_3: None,
            alpha_mode: AlphaMode::Opaque,
        }
    }
}

impl Material for BlueprintMaterial {
    fn fragment_shader() -> ShaderRef {
        // Use the embedded default shader
        // In a full implementation, this would dynamically select the shader
        // For now, we use a default PBR-like shader
        ShaderRef::Path("shaders/blueprint_material.wgsl".into())
    }

    fn alpha_mode(&self) -> AlphaMode {
        self.alpha_mode
    }
}

// ============================================================================
// Helper to create StandardMaterial from blueprint with proper PBR values
// ============================================================================

/// Create a StandardMaterial from a compiled blueprint
///
/// This extracts PBR properties and textures from the blueprint graph
/// and applies them to a StandardMaterial. For simple materials this works well.
/// For complex procedural materials, consider using the full shader pipeline.
pub fn create_material_from_blueprint(
    graph: &BlueprintGraph,
    compiled: &CompiledBlueprintMaterial,
    asset_server: &AssetServer,
    project_path: Option<&PathBuf>,
) -> StandardMaterial {
    let mut material = StandardMaterial::default();

    // Extract PBR values from the graph
    let pbr_values = extract_pbr_values(graph);

    // Apply base color
    material.base_color = Color::linear_rgba(
        pbr_values.base_color[0],
        pbr_values.base_color[1],
        pbr_values.base_color[2],
        pbr_values.base_color[3],
    );

    // Apply metallic and roughness
    material.metallic = pbr_values.metallic;
    material.perceptual_roughness = pbr_values.roughness;

    // Apply emissive
    material.emissive = Color::linear_rgb(
        pbr_values.emissive[0],
        pbr_values.emissive[1],
        pbr_values.emissive[2],
    ).into();

    // Load textures if available
    if let Some(path) = compiled.texture_paths.first() {
        if !path.is_empty() {
            let resolved_path = if std::path::Path::new(path).is_absolute() {
                PathBuf::from(path)
            } else if let Some(project) = project_path {
                project.join(path)
            } else {
                PathBuf::from(path)
            };

            info!("Loading texture from: {:?}", resolved_path);
            material.base_color_texture = Some(asset_server.load(resolved_path));
        }
    }

    // Log if material has procedural nodes that can't be fully represented
    if pbr_values.has_procedural_nodes {
        info!("Material '{}' has procedural nodes - using approximated values", graph.name);
    }

    material
}

// ============================================================================
// PBR Value Extraction
// ============================================================================

/// Extracted PBR values from a material blueprint graph
#[derive(Debug, Clone)]
pub struct ExtractedPbrValues {
    /// Base color (RGBA)
    pub base_color: [f32; 4],
    /// Metallic factor (0.0-1.0)
    pub metallic: f32,
    /// Roughness factor (0.0-1.0)
    pub roughness: f32,
    /// Emissive color (RGB)
    pub emissive: [f32; 3],
    /// Whether the material uses procedural nodes
    pub has_procedural_nodes: bool,
}

impl Default for ExtractedPbrValues {
    fn default() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            emissive: [0.0, 0.0, 0.0],
            has_procedural_nodes: false,
        }
    }
}

/// Extract PBR values from a material blueprint graph
pub fn extract_pbr_values(graph: &BlueprintGraph) -> ExtractedPbrValues {
    use super::{PinValue, PinId};

    let mut values = ExtractedPbrValues::default();

    // Find the PBR output node
    let output_node = graph.nodes.iter()
        .find(|n| n.node_type == "shader/pbr_output");

    let Some(output_node) = output_node else {
        return values;
    };

    // Extract base_color - check connection first
    let base_color_pin = PinId::input(output_node.id, "base_color");
    if let Some(conn) = graph.connection_to(&base_color_pin) {
        // There's a connection - trace it
        if let Some(color) = trace_color_value(graph, conn.from.node_id, &conn.from.pin_name, &mut values.has_procedural_nodes) {
            values.base_color = color;
        }
    } else if let Some(PinValue::Color(c)) = output_node.input_values.get("base_color") {
        values.base_color = *c;
    }

    // Extract metallic
    if let Some(PinValue::Float(m)) = output_node.input_values.get("metallic") {
        values.metallic = *m;
    }

    // Extract roughness
    if let Some(PinValue::Float(r)) = output_node.input_values.get("roughness") {
        values.roughness = *r;
    }

    // Extract emissive
    if let Some(PinValue::Color(c)) = output_node.input_values.get("emissive") {
        values.emissive = [c[0], c[1], c[2]];
    }

    values
}

/// Trace a color value back through the node graph
fn trace_color_value(
    graph: &BlueprintGraph,
    node_id: super::NodeId,
    _pin_name: &str,
    has_procedural: &mut bool,
) -> Option<[f32; 4]> {
    use super::PinValue;

    let node = graph.get_node(node_id)?;

    match node.node_type.as_str() {
        // Constant color node
        "shader/color" => {
            if let Some(PinValue::Color(c)) = node.input_values.get("color") {
                return Some(*c);
            }
        }
        // Lerp between colors - compute midpoint approximation
        "shader/lerp_color" => {
            *has_procedural = true;

            // Get color A
            let color_a = if let Some(conn) = graph.connection_to(&super::PinId::input(node.id, "a")) {
                trace_color_value(graph, conn.from.node_id, &conn.from.pin_name, has_procedural)
            } else {
                node.input_values.get("a").and_then(|v| {
                    if let PinValue::Color(c) = v { Some(*c) } else { None }
                })
            }.unwrap_or([0.0, 0.0, 0.0, 1.0]);

            // Get color B
            let color_b = if let Some(conn) = graph.connection_to(&super::PinId::input(node.id, "b")) {
                trace_color_value(graph, conn.from.node_id, &conn.from.pin_name, has_procedural)
            } else {
                node.input_values.get("b").and_then(|v| {
                    if let PinValue::Color(c) = v { Some(*c) } else { None }
                })
            }.unwrap_or([1.0, 1.0, 1.0, 1.0]);

            // Get lerp factor (default to 0.5 for procedural inputs)
            let t = node.input_values.get("t")
                .and_then(|v| if let PinValue::Float(f) = v { Some(*f) } else { None })
                .unwrap_or(0.5);

            return Some([
                color_a[0] * (1.0 - t) + color_b[0] * t,
                color_a[1] * (1.0 - t) + color_b[1] * t,
                color_a[2] * (1.0 - t) + color_b[2] * t,
                color_a[3] * (1.0 - t) + color_b[3] * t,
            ]);
        }
        // Procedural patterns - mark as procedural, return default
        "shader/checkerboard" | "shader/noise" | "shader/gradient" |
        "shader/brick" | "shader/voronoi" | "shader/fbm" => {
            *has_procedural = true;
            return Some([0.5, 0.5, 0.5, 1.0]); // Grey approximation
        }
        // Texture - can't extract color
        "shader/texture_color" => {
            return None;
        }
        _ => {
            *has_procedural = true;
        }
    }

    None
}

// ============================================================================
// Compiled Material
// ============================================================================

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
///
/// The `project_path` parameter is used to resolve relative texture paths.
pub fn create_standard_material_with_textures(
    compiled: &CompiledBlueprintMaterial,
    asset_server: &AssetServer,
    project_path: Option<&PathBuf>,
) -> StandardMaterial {
    let mut material = StandardMaterial::default();

    // Load the first texture as the base color texture if available
    if let Some(path) = compiled.texture_paths.first() {
        if !path.is_empty() {
            // Resolve relative path against project directory
            let resolved_path = if std::path::Path::new(path).is_absolute() {
                PathBuf::from(path)
            } else if let Some(project) = project_path {
                project.join(path)
            } else {
                PathBuf::from(path)
            };

            info!("Loading texture from: {:?}", resolved_path);
            material.base_color_texture = Some(asset_server.load(resolved_path));
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
pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect()
}

/// Create a material blueprint from a texture path
///
/// This creates a simple PBR material that uses the given texture as the base color.
/// The resulting blueprint can be saved to a .material_bp file.
///
/// # Arguments
/// * `name` - Name for the material
/// * `texture_path` - Relative path to the texture (e.g., "assets/textures/wood.jpg")
///
/// # Returns
/// A BlueprintGraph configured as a material with the texture connected to PBR output
pub fn create_material_from_texture(name: &str, texture_path: &str) -> BlueprintGraph {
    use super::{BlueprintNode, BlueprintType, Connection, NodeId, Pin, PinDirection, PinId, PinType, PinValue};
    use std::collections::HashMap;

    let mut graph = BlueprintGraph::new_with_type(name, BlueprintType::Material);

    // Create texture_color node (ID 1)
    let texture_node_id = NodeId::new(1);
    let mut texture_node = BlueprintNode::new(
        texture_node_id,
        "shader/texture_color",
        vec![
            Pin::input("uv", "UV", PinType::Vec2).with_default(PinValue::Vec2([0.0, 0.0])),
            Pin::output("color", "Color", PinType::Color),
            Pin::output("rgb", "RGB", PinType::Vec3),
            Pin::output("a", "Alpha", PinType::Float),
        ],
    );
    texture_node.position = [100.0, 200.0];
    texture_node.input_values.insert("path".to_string(), PinValue::Texture2D(texture_path.to_string()));
    graph.add_node(texture_node);

    // Create UV node (ID 2)
    let uv_node_id = NodeId::new(2);
    let uv_node = BlueprintNode {
        id: uv_node_id,
        node_type: "shader/uv".to_string(),
        position: [-100.0, 200.0],
        pins: vec![
            Pin::output("uv", "UV", PinType::Vec2),
            Pin::output("u", "U", PinType::Float),
            Pin::output("v", "V", PinType::Float),
        ],
        input_values: HashMap::new(),
        comment: None,
    };
    graph.add_node(uv_node);

    // Create PBR output node (ID 3)
    let pbr_node_id = NodeId::new(3);
    let pbr_node = BlueprintNode {
        id: pbr_node_id,
        node_type: "shader/pbr_output".to_string(),
        position: [400.0, 200.0],
        pins: vec![
            Pin::input("base_color", "Base Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
            Pin::input("metallic", "Metallic", PinType::Float).with_default(PinValue::Float(0.0)),
            Pin::input("roughness", "Roughness", PinType::Float).with_default(PinValue::Float(0.5)),
            Pin::input("normal", "Normal", PinType::Vec3),
            Pin::input("emissive", "Emissive", PinType::Color).with_default(PinValue::Color([0.0, 0.0, 0.0, 1.0])),
            Pin::input("ao", "Ambient Occlusion", PinType::Float).with_default(PinValue::Float(1.0)),
            Pin::input("alpha", "Alpha", PinType::Float).with_default(PinValue::Float(1.0)),
        ],
        input_values: HashMap::new(),
        comment: None,
    };
    graph.add_node(pbr_node);

    // Connect UV to texture
    graph.connections.push(Connection {
        from: PinId::output(uv_node_id, "uv"),
        to: PinId::input(texture_node_id, "uv"),
    });

    // Connect texture color to PBR base_color
    graph.connections.push(Connection {
        from: PinId::output(texture_node_id, "color"),
        to: PinId::input(pbr_node_id, "base_color"),
    });

    // Ensure next_node_id is updated by getting a new id (this advances the counter)
    let _ = graph.next_node_id();

    graph
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
