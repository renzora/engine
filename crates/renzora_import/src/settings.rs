//! Model import settings and related types.

/// Import settings applied when converting foreign model formats to GLB.
#[derive(Clone, Debug)]
pub struct ModelImportSettings {
    // === Transform ===
    /// Scale factor to apply to imported models
    pub scale: f32,
    /// Rotation offset in degrees (X, Y, Z)
    pub rotation_offset: (f32, f32, f32),
    /// Translation offset
    pub translation_offset: (f32, f32, f32),
    /// Whether to flip Y and Z coordinates (some formats use different up axes)
    pub convert_axes: ConvertAxes,

    // === Mesh ===
    /// How to handle mesh extraction
    pub mesh_handling: MeshHandling,
    /// Whether to combine meshes into a single mesh
    pub combine_meshes: bool,
    /// Whether to generate LODs automatically
    pub generate_lods: bool,
    /// Number of LOD levels to generate
    pub lod_count: u32,
    /// LOD reduction percentage per level
    pub lod_reduction: f32,

    // === Normals & Tangents ===
    /// How to handle normals
    pub normal_import: NormalImportMethod,
    /// How to handle tangents
    pub tangent_import: TangentImportMethod,
    /// Smoothing angle for computed normals (degrees)
    pub smoothing_angle: f32,

    // === Materials & Textures ===
    /// Whether to import materials
    pub import_materials: bool,
    /// Whether to extract and copy textures
    pub extract_textures: bool,
    /// Texture extraction subfolder name
    pub texture_subfolder: String,
    /// Whether to import vertex colors
    pub import_vertex_colors: bool,

    // === Animation ===
    /// Whether to import animations
    pub import_animations: bool,
    /// Whether to import as skeletal mesh
    pub import_as_skeletal: bool,
    /// Whether to import skeleton/bones
    pub import_skeleton: bool,

    // === Compression ===
    /// Whether to apply Draco compression (for glTF export)
    pub draco_compression: bool,
    /// Draco compression level (0-10, higher = smaller file, slower)
    pub draco_compression_level: u32,
    /// Draco quantization bits for positions (8-16)
    pub draco_position_bits: u32,
    /// Draco quantization bits for normals (8-16)
    pub draco_normal_bits: u32,
    /// Draco quantization bits for UVs (8-16)
    pub draco_uv_bits: u32,

    // === Physics ===
    /// Whether to generate collision shapes
    pub generate_colliders: bool,
    /// Type of collider to generate
    pub collider_type: ColliderImportType,
    /// Whether to use a simplified mesh for collision
    pub simplify_collision: bool,
    /// Collision mesh simplification ratio (0.0-1.0)
    pub collision_simplification: f32,

    // === Lightmapping ===
    /// Whether to generate lightmap UVs
    pub generate_lightmap_uvs: bool,
    /// Lightmap UV channel index
    pub lightmap_uv_channel: u32,
    /// Minimum lightmap resolution
    pub lightmap_resolution: u32,
}

impl Default for ModelImportSettings {
    fn default() -> Self {
        Self {
            // Transform
            scale: 1.0,
            rotation_offset: (0.0, 0.0, 0.0),
            translation_offset: (0.0, 0.0, 0.0),
            convert_axes: ConvertAxes::None,

            // Mesh
            mesh_handling: MeshHandling::KeepHierarchy,
            combine_meshes: false,
            generate_lods: false,
            lod_count: 3,
            lod_reduction: 50.0,

            // Normals & Tangents
            normal_import: NormalImportMethod::Import,
            tangent_import: TangentImportMethod::Import,
            smoothing_angle: 60.0,

            // Materials & Textures
            import_materials: true,
            extract_textures: true,
            texture_subfolder: "textures".to_string(),
            import_vertex_colors: true,

            // Animation
            import_animations: true,
            import_as_skeletal: false,
            import_skeleton: true,

            // Compression
            draco_compression: false,
            draco_compression_level: 7,
            draco_position_bits: 14,
            draco_normal_bits: 10,
            draco_uv_bits: 12,

            // Physics
            generate_colliders: false,
            collider_type: ColliderImportType::ConvexHull,
            simplify_collision: true,
            collision_simplification: 0.5,

            // Lightmapping
            generate_lightmap_uvs: false,
            lightmap_uv_channel: 1,
            lightmap_resolution: 64,
        }
    }
}

impl ModelImportSettings {
    /// Auto-configure defaults based on source file extension.
    pub fn apply_format_defaults(&mut self, ext: &str) {
        match ext {
            "fbx" => {
                // FBX files typically use Z-Up (Blender, Maya, 3ds Max)
                self.convert_axes = ConvertAxes::ZUpToYUp;
            }
            "obj" => {
                // OBJ is ambiguous but many exporters use Y-Up already
                self.convert_axes = ConvertAxes::None;
            }
            "usd" | "usdz" => {
                // USD uses Y-Up by default
                self.convert_axes = ConvertAxes::None;
            }
            _ => {}
        }
    }

    /// Return a human-readable description for the format.
    pub fn format_description(ext: &str) -> &'static str {
        match ext {
            "fbx" => "FBX (Autodesk) — typically Z-Up. Axis conversion auto-enabled.",
            "obj" => "OBJ (Wavefront) — Y-Up by convention. Sidecar .mtl + textures will be copied.",
            "usd" | "usdz" => "USD (Universal Scene Description) — Y-Up by default.",
            "glb" | "gltf" => "glTF/GLB — native engine format. No conversion needed.",
            _ => "Unknown format",
        }
    }
}

/// How to handle coordinate system conversion
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConvertAxes {
    /// No conversion
    #[default]
    None,
    /// Convert from Z-up to Y-up (Blender, 3ds Max default)
    ZUpToYUp,
    /// Convert from Y-up to Z-up
    YUpToZUp,
    /// Flip X axis
    FlipX,
    /// Flip Z axis (front/back)
    FlipZ,
}

/// How to handle mesh extraction from the source file
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MeshHandling {
    /// Keep the original hierarchy, reference the source file
    #[default]
    KeepHierarchy,
    /// Extract each mesh as a separate asset file
    ExtractMeshes,
    /// Flatten hierarchy but keep meshes separate
    FlattenHierarchy,
    /// Combine all meshes into a single mesh asset
    CombineAll,
}

/// How to handle normal vectors during import
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NormalImportMethod {
    /// Import normals from file
    #[default]
    Import,
    /// Compute normals (smooth)
    ComputeSmooth,
    /// Compute normals (flat/faceted)
    ComputeFlat,
    /// Import and recompute tangent space
    ImportAndRecompute,
}

/// How to handle tangent vectors during import
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TangentImportMethod {
    /// Import tangents from file
    #[default]
    Import,
    /// Compute tangents using MikkTSpace algorithm
    ComputeMikkTSpace,
    /// Don't import tangents
    None,
}

/// Type of collider to generate on import
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColliderImportType {
    /// Convex hull collider (faster, less accurate)
    #[default]
    ConvexHull,
    /// Trimesh collider (slower, more accurate)
    Trimesh,
    /// Axis-aligned bounding box
    AABB,
    /// Oriented bounding box
    OBB,
    /// Capsule (auto-fit)
    Capsule,
    /// Sphere (auto-fit)
    Sphere,
    /// Use decomposed convex hulls (V-HACD)
    Decomposed,
    /// Use simplified mesh
    SimplifiedMesh,
}
