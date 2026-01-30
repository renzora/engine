//! Default brush material loading and management
//!
//! Loads the checkerboard material blueprint and compiles it for use with brush geometry.

use bevy::prelude::*;
use std::path::PathBuf;

use crate::blueprint::{
    compile_material_blueprint, create_material_from_blueprint,
    serialization::BlueprintFile,
};
use crate::project::CurrentProject;

/// Resource holding the default material for brush geometry
#[derive(Resource, Default)]
pub struct DefaultBrushMaterial {
    /// Handle to the compiled checkerboard material
    pub material_handle: Option<Handle<StandardMaterial>>,
    /// Whether the material has been loaded
    pub loaded: bool,
    /// Path to the loaded material blueprint
    pub material_path: Option<PathBuf>,
}

impl DefaultBrushMaterial {
    /// Get the material handle, or None if not loaded
    pub fn get(&self) -> Option<Handle<StandardMaterial>> {
        self.material_handle.clone()
    }
}

/// Startup system to load and compile the default brush material
pub fn setup_default_brush_material(
    mut default_material: ResMut<DefaultBrushMaterial>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    current_project: Option<Res<CurrentProject>>,
) {
    // Try loading from project assets first, then fallback to embedded path
    let blueprint_paths = [
        // Project assets folder
        current_project.as_ref().map(|p| p.path.join("assets/materials/checkerboard_default.material_bp")),
        // Editor assets folder (relative to working directory)
        Some(PathBuf::from("assets/materials/checkerboard_default.material_bp")),
    ];

    for path_option in blueprint_paths.iter().flatten() {
        if path_option.exists() {
            match load_and_compile_material(path_option, &mut materials, &asset_server, current_project.as_deref()) {
                Ok(handle) => {
                    info!("Loaded default brush material from: {:?}", path_option);
                    default_material.material_handle = Some(handle);
                    default_material.loaded = true;
                    default_material.material_path = Some(path_option.clone());
                    return;
                }
                Err(e) => {
                    warn!("Failed to load material from {:?}: {}", path_option, e);
                }
            }
        }
    }

    // Fallback: create a simple checkerboard material programmatically
    info!("Creating fallback checkerboard material");
    let material = create_fallback_checkerboard_material();
    default_material.material_handle = Some(materials.add(material));
    default_material.loaded = true;
}

/// Load a material blueprint file and compile it to a StandardMaterial
fn load_and_compile_material(
    path: &PathBuf,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
    current_project: Option<&CurrentProject>,
) -> Result<Handle<StandardMaterial>, String> {
    // Load the blueprint file
    let blueprint_file = BlueprintFile::load(path)
        .map_err(|e| format!("Failed to load blueprint: {}", e))?;

    // Get the graph
    let graph = &blueprint_file.graph;

    // Compile the material
    let compiled = compile_material_blueprint(graph);

    if !compiled.is_ok() {
        return Err(format!("Compilation errors: {:?}", compiled.errors));
    }

    // Create StandardMaterial from the compiled blueprint
    let project_path = current_project.map(|p| &p.path);
    let material = create_material_from_blueprint(graph, &compiled, asset_server, project_path);

    Ok(materials.add(material))
}

/// Create a fallback checkerboard material when the blueprint can't be loaded
fn create_fallback_checkerboard_material() -> StandardMaterial {
    // Create a simple gray material as fallback
    // In a full implementation, this could generate a checkerboard texture procedurally
    StandardMaterial {
        base_color: Color::srgb(0.35, 0.45, 0.73), // Blue-ish color matching the blueprint
        perceptual_roughness: 0.5,
        metallic: 0.0,
        ..default()
    }
}

/// System to reload the brush material when project changes
pub fn reload_brush_material_on_project_change(
    mut default_material: ResMut<DefaultBrushMaterial>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    current_project: Option<Res<CurrentProject>>,
    mut last_project_path: Local<Option<PathBuf>>,
) {
    let current_path = current_project.as_ref().map(|p| p.path.clone());

    if *last_project_path != current_path {
        *last_project_path = current_path.clone();

        // Reset the material state
        default_material.loaded = false;
        default_material.material_handle = None;
        default_material.material_path = None;

        // Reload the material
        setup_default_brush_material(
            default_material,
            materials,
            asset_server,
            current_project,
        );
    }
}

/// Create a material for a specific brush, potentially with variations
pub fn create_brush_material(
    default_material: &DefaultBrushMaterial,
    materials: &mut Assets<StandardMaterial>,
) -> Handle<StandardMaterial> {
    // Use the default checkerboard material if available
    if let Some(handle) = default_material.get() {
        return handle;
    }

    // Fallback: create a simple gray material
    materials.add(StandardMaterial {
        base_color: Color::srgb(0.7, 0.7, 0.7),
        perceptual_roughness: 0.9,
        ..default()
    })
}
