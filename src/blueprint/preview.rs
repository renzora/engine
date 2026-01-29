//! Material Preview System
//!
//! Provides a real-time 3D preview of material blueprints as they're being edited.
//! Uses render-to-texture to display a preview mesh with the material applied.

use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::camera::RenderTarget;
use crate::core::resources::console::{console_log, LogLevel};
use crate::blueprint::{BlueprintGraph, BlueprintNode, PinValue};
use super::preview_eval::get_pin_value;

/// Resource holding the preview render texture handle
#[derive(Resource)]
pub struct MaterialPreviewImage(pub Handle<Image>);

/// Marker component for the material preview camera
#[derive(Component)]
pub struct MaterialPreviewCamera;

/// Marker component for the material preview mesh
#[derive(Component)]
pub struct MaterialPreviewMesh;

/// Marker component for the material preview light
#[derive(Component)]
pub struct MaterialPreviewLight;

/// Marker for all material preview entities (for cleanup)
#[derive(Component)]
pub struct MaterialPreviewEntity;

/// State for the material preview
#[derive(Resource)]
pub struct MaterialPreviewState {
    /// Currently selected preview mesh shape
    pub mesh_shape: PreviewMeshShape,
    /// Camera orbit yaw (radians)
    pub yaw: f32,
    /// Camera orbit pitch (radians)
    pub pitch: f32,
    /// Camera distance from origin
    pub distance: f32,
    /// Whether auto-rotation is enabled
    pub auto_rotate: bool,
    /// Auto-rotation speed (radians per second)
    pub rotation_speed: f32,
    /// Whether the preview needs to recompile the material
    pub needs_recompile: bool,
    /// Last compiled material hash (to detect changes)
    pub last_graph_hash: u64,
    /// Environment lighting intensity
    pub environment_intensity: f32,
    /// Show wireframe overlay
    pub show_wireframe: bool,
    /// Preview texture size
    pub texture_size: u32,
    /// Preview scene offset (to separate from main scene)
    pub scene_offset: Vec3,
}

impl Default for MaterialPreviewState {
    fn default() -> Self {
        Self {
            mesh_shape: PreviewMeshShape::Sphere,
            yaw: 0.5,
            pitch: 0.3,
            distance: 3.0,
            auto_rotate: false,
            rotation_speed: 0.5,
            needs_recompile: true,
            last_graph_hash: 0,
            environment_intensity: 1.0,
            show_wireframe: false,
            texture_size: 512,
            // Offset far from main scene to avoid visibility conflicts
            scene_offset: Vec3::new(10000.0, 0.0, 10000.0),
        }
    }
}

/// Available preview mesh shapes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PreviewMeshShape {
    #[default]
    Sphere,
    Cube,
    Cylinder,
    Torus,
    Plane,
}

impl PreviewMeshShape {
    pub const ALL: &'static [PreviewMeshShape] = &[
        PreviewMeshShape::Sphere,
        PreviewMeshShape::Cube,
        PreviewMeshShape::Cylinder,
        PreviewMeshShape::Torus,
        PreviewMeshShape::Plane,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            PreviewMeshShape::Sphere => "Sphere",
            PreviewMeshShape::Cube => "Cube",
            PreviewMeshShape::Cylinder => "Cylinder",
            PreviewMeshShape::Torus => "Torus",
            PreviewMeshShape::Plane => "Plane",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            PreviewMeshShape::Sphere => "\u{f111}", // circle
            PreviewMeshShape::Cube => "\u{f1b2}",   // cube
            PreviewMeshShape::Cylinder => "\u{f0d0}", // cylinder-like
            PreviewMeshShape::Torus => "\u{f1ce}",  // circle-o
            PreviewMeshShape::Plane => "\u{f0c8}",  // square
        }
    }
}

/// Set up the material preview texture and scene
pub fn setup_material_preview(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    preview_state: Res<MaterialPreviewState>,
) {
    let size = preview_state.texture_size;
    let offset = preview_state.scene_offset;

    // Create render texture
    let extent = Extent3d {
        width: size,
        height: size,
        depth_or_array_layers: 1,
    };

    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("material_preview_texture"),
            size: extent,
            dimension: TextureDimension::D2,
            format: TextureFormat::Bgra8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        },
        ..default()
    };
    image.resize(extent);

    let image_handle = images.add(image);
    commands.insert_resource(MaterialPreviewImage(image_handle.clone()));

    // Create preview camera
    let cam_pos = calculate_camera_position(preview_state.yaw, preview_state.pitch, preview_state.distance) + offset;

    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.1, 0.1, 0.12)),
            order: -5, // Render before main camera but after splash scene camera (-10)
            ..default()
        },
        RenderTarget::Image(image_handle.into()),
        Transform::from_translation(cam_pos).looking_at(offset, Vec3::Y),
        MaterialPreviewCamera,
        MaterialPreviewEntity,
    ));

    // Create preview mesh (sphere by default)
    let mesh = meshes.add(Sphere::new(1.0).mesh().ico(5).unwrap());
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.8, 0.8),
        metallic: 0.0,
        perceptual_roughness: 0.5,
        ..default()
    });

    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(offset),
        MaterialPreviewMesh,
        MaterialPreviewEntity,
    ));

    // Create preview lights at offset position
    // Key light (main directional)
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_translation(offset + Vec3::new(5.0, 10.0, 5.0))
            .looking_at(offset, Vec3::Y),
        MaterialPreviewLight,
        MaterialPreviewEntity,
    ));

    // Fill light (softer, opposite side)
    commands.spawn((
        PointLight {
            intensity: 500000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_translation(offset + Vec3::new(-3.0, 2.0, -3.0)),
        MaterialPreviewLight,
        MaterialPreviewEntity,
    ));
}

/// Calculate camera position from orbit parameters
fn calculate_camera_position(yaw: f32, pitch: f32, distance: f32) -> Vec3 {
    Vec3::new(
        distance * pitch.cos() * yaw.sin(),
        distance * pitch.sin(),
        distance * pitch.cos() * yaw.cos(),
    )
}

/// System to update preview camera based on state
pub fn update_preview_camera(
    preview_state: Res<MaterialPreviewState>,
    time: Res<Time>,
    mut camera_query: Query<&mut Transform, With<MaterialPreviewCamera>>,
    mut local_yaw: Local<f32>,
) {
    let Ok(mut transform) = camera_query.single_mut() else {
        return;
    };

    let offset = preview_state.scene_offset;

    // Update auto-rotation
    let yaw = if preview_state.auto_rotate {
        *local_yaw += preview_state.rotation_speed * time.delta_secs();
        *local_yaw
    } else {
        *local_yaw = preview_state.yaw;
        preview_state.yaw
    };

    let cam_pos = calculate_camera_position(yaw, preview_state.pitch, preview_state.distance) + offset;
    transform.translation = cam_pos;
    transform.look_at(offset, Vec3::Y);
}

/// System to update preview mesh shape when changed
pub fn update_preview_mesh(
    preview_state: Res<MaterialPreviewState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mesh_query: Query<&mut Mesh3d, With<MaterialPreviewMesh>>,
    mut last_shape: Local<Option<PreviewMeshShape>>,
) {
    if *last_shape == Some(preview_state.mesh_shape) {
        return;
    }
    *last_shape = Some(preview_state.mesh_shape);

    let Ok(mut mesh_handle) = mesh_query.single_mut() else {
        return;
    };

    let new_mesh: Mesh = match preview_state.mesh_shape {
        PreviewMeshShape::Sphere => Sphere::new(1.0).mesh().ico(5).unwrap(),
        PreviewMeshShape::Cube => Cuboid::new(1.5, 1.5, 1.5).into(),
        PreviewMeshShape::Cylinder => Cylinder::new(0.8, 2.0).mesh().resolution(32).build(),
        PreviewMeshShape::Torus => Torus::new(0.5, 1.0).mesh().minor_resolution(24).major_resolution(48).build(),
        PreviewMeshShape::Plane => Plane3d::new(Vec3::Y, Vec2::splat(2.0)).mesh().subdivisions(10).build(),
    };

    mesh_handle.0 = meshes.add(new_mesh);
}

/// System to update preview material from blueprint
pub fn update_preview_material(
    mut preview_state: ResMut<MaterialPreviewState>,
    blueprint_editor: Res<crate::blueprint::BlueprintEditorState>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mesh_query: Query<&MeshMaterial3d<StandardMaterial>, With<MaterialPreviewMesh>>,
    asset_server: Res<AssetServer>,
    current_project: Option<Res<crate::project::CurrentProject>>,
) {
    // Only update if there's an active material blueprint
    let Some(graph) = blueprint_editor.active_graph() else {
        return;
    };

    if !graph.is_material() {
        return;
    }

    // Calculate a simple hash of the graph to detect changes
    let graph_hash = calculate_graph_hash(graph);
    if graph_hash == preview_state.last_graph_hash && !preview_state.needs_recompile {
        return;
    }

    // Log that we're updating
    console_log(LogLevel::Info, "Preview", format!(
        "Updating material preview ({} nodes, {} connections)",
        graph.nodes.len(), graph.connections.len()
    ));

    preview_state.last_graph_hash = graph_hash;
    preview_state.needs_recompile = false;

    let Ok(material_handle) = mesh_query.single() else {
        return;
    };

    let Some(material) = materials.get_mut(&material_handle.0) else {
        return;
    };

    // Default values - reset everything
    material.base_color = Color::srgb(0.8, 0.8, 0.8);
    material.metallic = 0.0;
    material.perceptual_roughness = 0.5;
    material.emissive = LinearRgba::BLACK;
    material.unlit = false;
    material.alpha_mode = bevy::prelude::AlphaMode::Opaque;
    material.base_color_texture = None;
    material.normal_map_texture = None;
    material.metallic_roughness_texture = None;
    material.occlusion_texture = None;
    material.emissive_texture = None;
    material.depth_map = None;

    // Find output node
    let Some(output_node) = graph.nodes.iter().find(|n| {
        n.node_type == "shader/pbr_output" || n.node_type == "shader/unlit_output"
    }) else {
        console_log(LogLevel::Warning, "Preview", "No output node found");
        return;
    };

    // For unlit materials, handle separately
    if output_node.node_type == "shader/unlit_output" {
        material.unlit = true;

        // Process color input
        if let Some(value) = get_pin_value(graph, output_node, "color") {
            match value {
                PinValue::Vec4(c) | PinValue::Color(c) => {
                    material.base_color = Color::srgba(c[0], c[1], c[2], c[3]);
                }
                PinValue::Vec3(c) => {
                    material.base_color = Color::srgb(c[0], c[1], c[2]);
                }
                PinValue::Texture2D(ref path) if !path.is_empty() => {
                    let full_path = resolve_texture_path(path, current_project.as_deref());
                    console_log(LogLevel::Success, "Preview", format!("Loading unlit color texture: {:?}", full_path));
                    material.base_color_texture = Some(asset_server.load(full_path));
                    material.base_color = Color::WHITE;
                }
                _ => {}
            }
        }

        // Process alpha input
        if let Some(value) = get_pin_value(graph, output_node, "alpha") {
            match value {
                PinValue::Float(a) => {
                    if a < 1.0 {
                        material.base_color = material.base_color.with_alpha(a);
                        material.alpha_mode = bevy::prelude::AlphaMode::Blend;
                    }
                }
                PinValue::Texture2D(ref path) if !path.is_empty() => {
                    // Alpha from texture - use the base color texture's alpha or load separate
                    console_log(LogLevel::Info, "Preview", format!("Alpha texture: {:?} (using base color alpha)", path));
                    material.alpha_mode = bevy::prelude::AlphaMode::Blend;
                }
                _ => {}
            }
        }
        return;
    }

    // PBR Output handling

    // Process base_color input - check for procedural patterns first
    let has_procedural = chain_has_procedural_pattern(graph, output_node, "base_color");

    if has_procedural {
        // Generate procedural texture
        console_log(LogLevel::Info, "Preview", "Generating procedural texture for base color...");
        if let Some(proc_image) = generate_procedural_texture(graph, output_node, "base_color", 256) {
            let texture_handle = images.add(proc_image);
            material.base_color_texture = Some(texture_handle);
            material.base_color = Color::WHITE;
            console_log(LogLevel::Success, "Preview", "Procedural texture generated successfully");
        }
    } else if let Some(value) = get_pin_value(graph, output_node, "base_color") {
        match value {
            PinValue::Vec4(c) | PinValue::Color(c) => {
                material.base_color = Color::srgba(c[0], c[1], c[2], c[3]);
            }
            PinValue::Vec3(c) => {
                material.base_color = Color::srgb(c[0], c[1], c[2]);
            }
            PinValue::Texture2D(ref path) if !path.is_empty() => {
                let full_path = resolve_texture_path(path, current_project.as_deref());
                console_log(LogLevel::Success, "Preview", format!("Loading base color texture: {:?}", full_path));
                material.base_color_texture = Some(asset_server.load(full_path));
                material.base_color = Color::WHITE;
            }
            _ => {}
        }
    }

    // Process metallic input (float or texture)
    if let Some(value) = get_pin_value(graph, output_node, "metallic") {
        match value {
            PinValue::Float(m) => {
                material.metallic = m.clamp(0.0, 1.0);
            }
            PinValue::Texture2D(ref path) if !path.is_empty() => {
                // Metallic texture - Bevy expects combined metallic_roughness
                // For separate metallic textures, we load it but it won't work correctly
                // The blue channel should contain metallic in glTF format
                let full_path = resolve_texture_path(path, current_project.as_deref());
                console_log(LogLevel::Success, "Preview", format!("Loading metallic texture: {:?}", full_path));
                // If no roughness texture is set, use this as metallic_roughness
                if material.metallic_roughness_texture.is_none() {
                    material.metallic_roughness_texture = Some(asset_server.load(full_path));
                    material.metallic = 1.0; // Let texture control it
                }
            }
            _ => {}
        }
    }

    // Process roughness input (float or texture)
    if let Some(value) = get_pin_value(graph, output_node, "roughness") {
        match value {
            PinValue::Float(r) => {
                material.perceptual_roughness = r.clamp(0.0, 1.0);
            }
            PinValue::Texture2D(ref path) if !path.is_empty() => {
                let full_path = resolve_texture_path(path, current_project.as_deref());
                console_log(LogLevel::Success, "Preview", format!("Loading roughness texture: {:?}", full_path));
                material.metallic_roughness_texture = Some(asset_server.load(full_path));
                material.perceptual_roughness = 1.0; // Let texture control it
            }
            _ => {}
        }
    }

    // Process normal input (texture only)
    if let Some(value) = get_pin_value(graph, output_node, "normal") {
        match value {
            PinValue::Texture2D(ref path) if !path.is_empty() => {
                let full_path = resolve_texture_path(path, current_project.as_deref());
                console_log(LogLevel::Success, "Preview", format!("Loading normal map: {:?}", full_path));
                material.normal_map_texture = Some(asset_server.load(full_path));
            }
            PinValue::Vec3(_) => {
                // Direct normal vector - can't apply to StandardMaterial without texture
            }
            _ => {}
        }
    }

    // Process emissive input (color or texture)
    if let Some(value) = get_pin_value(graph, output_node, "emissive") {
        match value {
            PinValue::Vec4(e) | PinValue::Color(e) => {
                // Use RGB, ignore alpha for emissive
                material.emissive = LinearRgba::rgb(e[0], e[1], e[2]);
            }
            PinValue::Vec3(e) => {
                material.emissive = LinearRgba::rgb(e[0], e[1], e[2]);
            }
            PinValue::Texture2D(ref path) if !path.is_empty() => {
                let full_path = resolve_texture_path(path, current_project.as_deref());
                console_log(LogLevel::Success, "Preview", format!("Loading emissive texture: {:?}", full_path));
                material.emissive_texture = Some(asset_server.load(full_path));
                material.emissive = LinearRgba::WHITE; // Let texture control color
            }
            _ => {}
        }
    }

    // Process AO input (float or texture)
    if let Some(value) = get_pin_value(graph, output_node, "ao") {
        match value {
            PinValue::Float(_ao) => {
                // Bevy doesn't have a scalar AO value, only texture
                // We could potentially modify base_color, but that's not accurate
            }
            PinValue::Texture2D(ref path) if !path.is_empty() => {
                let full_path = resolve_texture_path(path, current_project.as_deref());
                console_log(LogLevel::Success, "Preview", format!("Loading AO texture: {:?}", full_path));
                material.occlusion_texture = Some(asset_server.load(full_path));
            }
            _ => {}
        }
    }

    // Process alpha input (float or texture via opacity node)
    if let Some(value) = get_pin_value(graph, output_node, "alpha") {
        match value {
            PinValue::Float(a) => {
                if a < 1.0 {
                    material.base_color = material.base_color.with_alpha(a);
                    material.alpha_mode = bevy::prelude::AlphaMode::Blend;
                    console_log(LogLevel::Info, "Preview", format!("Alpha set to {}", a));
                }
            }
            PinValue::Texture2D(ref path) if !path.is_empty() => {
                // For alpha textures, Bevy uses the alpha channel of base_color_texture
                // We can't easily use a separate alpha texture without a custom shader
                console_log(LogLevel::Info, "Preview", format!("Alpha texture: {:?} (requires base color with alpha)", path));
                material.alpha_mode = bevy::prelude::AlphaMode::Blend;
            }
            _ => {}
        }
    }
}

/// Resolve a texture path relative to the project
fn resolve_texture_path(path: &str, current_project: Option<&crate::project::CurrentProject>) -> std::path::PathBuf {
    if let Some(project) = current_project {
        project.path.join(path)
    } else {
        std::path::PathBuf::from(path)
    }
}

/// Check if a node chain contains procedural patterns that need texture generation
fn chain_has_procedural_pattern(graph: &BlueprintGraph, node: &BlueprintNode, _pin: &str) -> bool {
    // Check if this node is a procedural pattern generator
    let procedural_types = [
        "shader/checkerboard",
        "shader/noise_simple",
        "shader/noise_gradient",
        "shader/noise_voronoi",
        "shader/noise_fbm",
        "shader/noise_turbulence",
        "shader/noise_ridged",
        "shader/gradient",
        "shader/domain_warp",
        "shader/brick",
        "shader/wave_sine",
        "shader/wave_square",
        "shader/wave_sawtooth",
        "shader/radial_gradient",
        "shader/spiral",
        "shader/sdf_circle",
        "shader/sdf_box",
    ];

    if procedural_types.contains(&node.node_type.as_str()) {
        return true;
    }

    // Check if any input to this node comes from a procedural pattern
    for conn in &graph.connections {
        if conn.to.node_id == node.id {
            if let Some(source_node) = graph.nodes.iter().find(|n| n.id == conn.from.node_id) {
                if chain_has_procedural_pattern(graph, source_node, &conn.from.pin_name) {
                    return true;
                }
            }
        }
    }

    false
}

/// Generate a procedural texture by evaluating the node graph at each pixel
fn generate_procedural_texture(
    graph: &BlueprintGraph,
    output_node: &BlueprintNode,
    pin_name: &str,
    size: u32,
) -> Option<Image> {
    let mut pixels = vec![0u8; (size * size * 4) as usize];

    for y in 0..size {
        for x in 0..size {
            // Calculate UV coordinates (0.0 to 1.0)
            let u = x as f32 / size as f32;
            let v = y as f32 / size as f32;

            // Evaluate the graph at this UV coordinate
            let color = evaluate_at_uv(graph, output_node, pin_name, [u, v]);

            // Write pixel (RGBA)
            let idx = ((y * size + x) * 4) as usize;
            pixels[idx] = (color[0] * 255.0).clamp(0.0, 255.0) as u8;
            pixels[idx + 1] = (color[1] * 255.0).clamp(0.0, 255.0) as u8;
            pixels[idx + 2] = (color[2] * 255.0).clamp(0.0, 255.0) as u8;
            pixels[idx + 3] = (color[3] * 255.0).clamp(0.0, 255.0) as u8;
        }
    }

    let extent = Extent3d {
        width: size,
        height: size,
        depth_or_array_layers: 1,
    };

    Some(Image::new(
        extent,
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        default(),
    ))
}

/// Evaluate the node graph at a specific UV coordinate
fn evaluate_at_uv(
    graph: &BlueprintGraph,
    output_node: &BlueprintNode,
    pin_name: &str,
    uv: [f32; 2],
) -> [f32; 4] {
    // Create a modified graph context where UV nodes return our specified UV
    // For simplicity, we'll use a thread-local to pass the UV coordinate
    UV_OVERRIDE.with(|cell| {
        *cell.borrow_mut() = Some(uv);
    });

    let result = get_pin_value(graph, output_node, pin_name);

    UV_OVERRIDE.with(|cell| {
        *cell.borrow_mut() = None;
    });

    match result {
        Some(PinValue::Color(c)) | Some(PinValue::Vec4(c)) => c,
        Some(PinValue::Vec3(c)) => [c[0], c[1], c[2], 1.0],
        Some(PinValue::Float(f)) => [f, f, f, 1.0],
        _ => [0.5, 0.5, 0.5, 1.0],
    }
}

// Thread-local storage for UV override during procedural texture generation
thread_local! {
    pub static UV_OVERRIDE: std::cell::RefCell<Option<[f32; 2]>> = std::cell::RefCell::new(None);
}

/// Calculate a simple hash of the graph for change detection
/// Only includes material-relevant data (node types, values, connections)
/// Excludes node positions since those don't affect the material output
fn calculate_graph_hash(graph: &crate::blueprint::BlueprintGraph) -> u64 {
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;

    let mut hasher = DefaultHasher::new();

    // Hash node count and types (but NOT positions - they don't affect material)
    graph.nodes.len().hash(&mut hasher);
    for node in &graph.nodes {
        node.id.0.hash(&mut hasher);
        node.node_type.hash(&mut hasher);

        // Hash input values (sorted keys for deterministic ordering)
        let mut keys: Vec<_> = node.input_values.keys().collect();
        keys.sort();
        for key in keys {
            key.hash(&mut hasher);
            if let Some(value) = node.input_values.get(key) {
                format!("{:?}", value).hash(&mut hasher);
            }
        }
    }

    // Hash connections
    graph.connections.len().hash(&mut hasher);
    for conn in &graph.connections {
        conn.from.node_id.0.hash(&mut hasher);
        conn.from.pin_name.hash(&mut hasher);
        conn.to.node_id.0.hash(&mut hasher);
        conn.to.pin_name.hash(&mut hasher);
    }

    hasher.finish()
}

/// Cleanup material preview entities
pub fn cleanup_material_preview(
    mut commands: Commands,
    query: Query<Entity, With<MaterialPreviewEntity>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

/// Plugin for material preview functionality
pub struct MaterialPreviewPlugin;

impl Plugin for MaterialPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MaterialPreviewState>()
            // Setup preview scene when entering Editor state (not during Startup to avoid splash conflicts)
            .add_systems(OnEnter(crate::core::AppState::Editor), setup_material_preview)
            .add_systems(
                Update,
                (
                    update_preview_camera,
                    update_preview_mesh,
                    update_preview_material,
                ).chain()
                    .run_if(in_state(crate::core::AppState::Editor)),
            );
    }
}
