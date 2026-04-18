//! Unified scene graph -- the intermediate representation produced by all USD parsers.

use std::collections::HashMap;

/// A fully parsed USD stage containing the entire scene.
#[derive(Debug, Clone, Default)]
pub struct UsdStage {
    /// Flat list of all meshes in the stage.
    pub meshes: Vec<UsdMesh>,
    /// Flat list of all materials.
    pub materials: Vec<UsdMaterial>,
    /// Embedded textures (from USDZ archives or inline data).
    pub textures: Vec<UsdTexture>,
    /// Skeleton definitions.
    pub skeletons: Vec<UsdSkeleton>,
    /// Animation clips.
    pub animations: Vec<UsdAnimation>,
    /// Lights.
    pub lights: Vec<UsdLight>,
    /// Cameras.
    pub cameras: Vec<UsdCamera>,
    /// Scene hierarchy.
    pub root: UsdNode,
    /// Stage-level metadata.
    pub up_axis: UpAxis,
    pub meters_per_unit: f32,
    pub time_codes_per_second: f32,
    /// Non-fatal warnings accumulated during parsing.
    pub warnings: Vec<String>,
}

/// A node in the scene graph tree.
#[derive(Debug, Clone, Default)]
pub struct UsdNode {
    /// Prim name (e.g. "MyMesh", "Root").
    pub name: String,
    /// Full USD path (e.g. "/Root/MyMesh").
    pub path: String,
    /// Local transform (column-major 4x4).
    pub transform: [f32; 16],
    /// What this node references, if anything.
    pub data: NodeData,
    /// Children.
    pub children: Vec<UsdNode>,
}

/// What a scene graph node contains.
#[derive(Debug, Clone, Default)]
pub enum NodeData {
    #[default]
    Empty,
    /// Index into `UsdStage::meshes`.
    Mesh(usize),
    /// Index into `UsdStage::lights`.
    Light(usize),
    /// Index into `UsdStage::cameras`.
    Camera(usize),
    /// Index into `UsdStage::skeletons`.
    Skeleton(usize),
}

// ---------------------------------------------------------------------------
// Mesh
// ---------------------------------------------------------------------------

/// A parsed mesh with all vertex data.
#[derive(Debug, Clone)]
pub struct UsdMesh {
    pub name: String,
    pub path: String,
    /// Vertex positions (vec3).
    pub positions: Vec<[f32; 3]>,
    /// Per-vertex normals (vec3). May be empty.
    pub normals: Vec<[f32; 3]>,
    /// UV sets. Key is the primvar name (e.g. "st", "st1").
    pub uv_sets: HashMap<String, Vec<[f32; 2]>>,
    /// Vertex colors (vec4 RGBA). May be empty.
    pub colors: Vec<[f32; 4]>,
    /// Tangents (vec4, w = handedness). May be empty.
    pub tangents: Vec<[f32; 4]>,
    /// Face vertex counts (e.g. [4, 4, 3] for two quads and a tri).
    pub face_vertex_counts: Vec<u32>,
    /// Face vertex indices.
    pub face_vertex_indices: Vec<u32>,
    /// Per-face material assignment via GeomSubsets.
    pub subsets: Vec<GeomSubset>,
    /// Skeleton binding, if skinned.
    pub skin: Option<MeshSkin>,
    /// Blend shape targets.
    pub blend_shapes: Vec<BlendShape>,
    /// Material binding path (resolved to index after full parse).
    pub material_binding: Option<String>,
    /// Resolved material index into `UsdStage::materials`. Set during finalization.
    pub material_index: Option<usize>,
    /// Subdivision scheme. "none" means polygonal mesh.
    pub subdivision_scheme: String,
}

impl Default for UsdMesh {
    fn default() -> Self {
        Self {
            name: String::new(),
            path: String::new(),
            positions: Vec::new(),
            normals: Vec::new(),
            uv_sets: HashMap::new(),
            colors: Vec::new(),
            tangents: Vec::new(),
            face_vertex_counts: Vec::new(),
            face_vertex_indices: Vec::new(),
            subsets: Vec::new(),
            skin: None,
            blend_shapes: Vec::new(),
            material_binding: None,
            material_index: None,
            subdivision_scheme: "none".into(),
        }
    }
}

/// A GeomSubset -- a set of face indices bound to a specific material.
#[derive(Debug, Clone)]
pub struct GeomSubset {
    pub name: String,
    /// Indices into the parent mesh's face list (not vertex indices).
    pub face_indices: Vec<u32>,
    /// Material binding path.
    pub material_binding: Option<String>,
    /// Resolved material index.
    pub material_index: Option<usize>,
}

/// Skinning data for a mesh.
#[derive(Debug, Clone)]
pub struct MeshSkin {
    /// Joint indices per vertex (vec4 of u16).
    pub joints: Vec<[u16; 4]>,
    /// Joint weights per vertex (vec4).
    pub weights: Vec<[f32; 4]>,
    /// Path to the skeleton prim.
    pub skeleton_path: String,
}

/// A blend shape (morph target).
#[derive(Debug, Clone)]
pub struct BlendShape {
    pub name: String,
    /// Position offsets (deltas from the base mesh).
    pub position_offsets: Vec<[f32; 3]>,
    /// Normal offsets. May be empty.
    pub normal_offsets: Vec<[f32; 3]>,
    /// Which vertex indices are affected (sparse representation).
    pub point_indices: Vec<u32>,
}

// ---------------------------------------------------------------------------
// Material
// ---------------------------------------------------------------------------

/// A UsdPreviewSurface material.
#[derive(Debug, Clone)]
pub struct UsdMaterial {
    pub name: String,
    pub path: String,
    /// Diffuse color (linear RGB). Default: [0.18, 0.18, 0.18].
    pub diffuse_color: [f32; 3],
    /// Diffuse texture.
    pub diffuse_texture: Option<TextureRef>,
    /// Emissive color (linear RGB).
    pub emissive_color: [f32; 3],
    /// Emissive texture.
    pub emissive_texture: Option<TextureRef>,
    /// Metallic factor [0..1].
    pub metallic: f32,
    /// Metallic texture.
    pub metallic_texture: Option<TextureRef>,
    /// Roughness factor [0..1].
    pub roughness: f32,
    /// Roughness texture.
    pub roughness_texture: Option<TextureRef>,
    /// Normal map.
    pub normal_texture: Option<TextureRef>,
    /// Normal map scale.
    pub normal_scale: f32,
    /// Opacity [0..1].
    pub opacity: f32,
    /// Opacity texture.
    pub opacity_texture: Option<TextureRef>,
    /// Index of refraction.
    pub ior: f32,
    /// Occlusion texture.
    pub occlusion_texture: Option<TextureRef>,
}

impl Default for UsdMaterial {
    fn default() -> Self {
        Self {
            name: String::new(),
            path: String::new(),
            diffuse_color: [0.18, 0.18, 0.18],
            diffuse_texture: None,
            emissive_color: [0.0, 0.0, 0.0],
            emissive_texture: None,
            metallic: 0.0,
            metallic_texture: None,
            roughness: 0.5,
            roughness_texture: None,
            normal_texture: None,
            normal_scale: 1.0,
            opacity: 1.0,
            opacity_texture: None,
            ior: 1.5,
            occlusion_texture: None,
        }
    }
}

/// Reference to a texture with UV transform.
#[derive(Debug, Clone)]
pub struct TextureRef {
    /// Index into `UsdStage::textures`, or a file path if not embedded.
    pub source: TextureSource,
    /// Which UV set to use (primvar name, e.g. "st").
    pub uv_set: String,
    /// UV scale.
    pub scale: [f32; 2],
    /// UV rotation (radians).
    pub rotation: f32,
    /// UV translation.
    pub translation: [f32; 2],
}

impl Default for TextureRef {
    fn default() -> Self {
        Self {
            source: TextureSource::File(String::new()),
            uv_set: "st".into(),
            scale: [1.0, 1.0],
            rotation: 0.0,
            translation: [0.0, 0.0],
        }
    }
}

#[derive(Debug, Clone)]
pub enum TextureSource {
    /// Path to an external texture file (relative to the USD file).
    File(String),
    /// Index into `UsdStage::textures` (embedded in USDZ).
    Embedded(usize),
}

// ---------------------------------------------------------------------------
// Texture
// ---------------------------------------------------------------------------

/// An embedded texture (extracted from USDZ or inline data).
#[derive(Debug, Clone)]
pub struct UsdTexture {
    /// Original file name/path within the archive.
    pub name: String,
    /// MIME type (e.g. "image/png", "image/jpeg").
    pub mime_type: String,
    /// Raw image bytes.
    pub data: Vec<u8>,
}

// ---------------------------------------------------------------------------
// Skeleton
// ---------------------------------------------------------------------------

/// A UsdSkel skeleton definition.
#[derive(Debug, Clone)]
pub struct UsdSkeleton {
    pub name: String,
    pub path: String,
    /// Joint paths (e.g. ["Hips", "Hips/Spine", "Hips/Spine/Chest"]).
    pub joints: Vec<String>,
    /// Joint parent indices (-1 for roots).
    pub parent_indices: Vec<i32>,
    /// Bind transforms (world-space, column-major 4x4) per joint.
    pub bind_transforms: Vec<[f32; 16]>,
    /// Rest transforms (local-space, column-major 4x4) per joint.
    pub rest_transforms: Vec<[f32; 16]>,
}

impl Default for UsdSkeleton {
    fn default() -> Self {
        Self {
            name: String::new(),
            path: String::new(),
            joints: Vec::new(),
            parent_indices: Vec::new(),
            bind_transforms: Vec::new(),
            rest_transforms: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Animation
// ---------------------------------------------------------------------------

/// A USD animation clip (time-sampled transforms or blend shape weights).
#[derive(Debug, Clone)]
pub struct UsdAnimation {
    pub name: String,
    pub path: String,
    /// Duration in seconds.
    pub duration: f32,
    /// Per-joint transform tracks.
    pub joint_tracks: Vec<JointTrack>,
    /// Blend shape weight tracks.
    pub blend_shape_tracks: Vec<BlendShapeTrack>,
}

/// Animation track for a single joint.
#[derive(Debug, Clone)]
pub struct JointTrack {
    pub joint_path: String,
    /// (time_seconds, [tx, ty, tz]).
    pub translations: Vec<(f32, [f32; 3])>,
    /// (time_seconds, [qx, qy, qz, qw]).
    pub rotations: Vec<(f32, [f32; 4])>,
    /// (time_seconds, [sx, sy, sz]).
    pub scales: Vec<(f32, [f32; 3])>,
}

/// Animation track for blend shape weights.
#[derive(Debug, Clone)]
pub struct BlendShapeTrack {
    pub target_name: String,
    /// (time_seconds, weight).
    pub weights: Vec<(f32, f32)>,
}

// ---------------------------------------------------------------------------
// Lights
// ---------------------------------------------------------------------------

/// Axis convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UpAxis {
    #[default]
    YUp,
    ZUp,
}

/// A USD light.
#[derive(Debug, Clone)]
pub struct UsdLight {
    pub name: String,
    pub path: String,
    pub kind: LightKind,
    /// Color (linear RGB).
    pub color: [f32; 3],
    /// Intensity.
    pub intensity: f32,
}

#[derive(Debug, Clone)]
pub enum LightKind {
    /// Directional / distant light.
    Distant { angle: f32 },
    /// Point / sphere light.
    Sphere { radius: f32 },
    /// Rectangular area light.
    Rect { width: f32, height: f32 },
    /// Disk area light.
    Disk { radius: f32 },
    /// Dome / environment light.
    Dome { texture_path: Option<String> },
}

// ---------------------------------------------------------------------------
// Camera
// ---------------------------------------------------------------------------

/// A USD camera.
#[derive(Debug, Clone)]
pub struct UsdCamera {
    pub name: String,
    pub path: String,
    pub projection: Projection,
    pub near_clip: f32,
    pub far_clip: f32,
}

#[derive(Debug, Clone)]
pub enum Projection {
    Perspective {
        /// Horizontal field of view in degrees.
        fov_horizontal: f32,
        /// Focal length in mm.
        focal_length: f32,
    },
    Orthographic {
        /// Horizontal aperture size.
        width: f32,
    },
}

impl Default for UsdCamera {
    fn default() -> Self {
        Self {
            name: String::new(),
            path: String::new(),
            projection: Projection::Perspective {
                fov_horizontal: 60.0,
                focal_length: 50.0,
            },
            near_clip: 0.1,
            far_clip: 10000.0,
        }
    }
}
