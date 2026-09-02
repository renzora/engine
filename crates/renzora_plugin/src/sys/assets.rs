//! Meshes, images and materials a plugin creates, and the handles it gets back.

use super::{ComponentId, StrRef, Vec3};

/// A mesh or material the host created for a plugin. Opaque index into a
/// host-side table — a plugin never sees a real `Handle`.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AssetHandle(pub u64);

impl AssetHandle {
    pub const INVALID: AssetHandle = AssetHandle(u64::MAX);
    pub const fn is_valid(self) -> bool {
        self.0 != u64::MAX
    }
}

/// Built-in mesh shapes.
///
/// A closed set rather than arbitrary vertex data: a plugin handing over raw
/// buffers needs the whole asset/GPU surface, whereas primitives cover the cases
/// that actually come up — spawning markers, blockout geometry, particles,
/// procedural layouts.
/// Newtype rather than an `enum`, and that is a soundness requirement rather
/// than a style choice.
///
/// The **plugin writes this value and the host reads it** — out of plugin memory
/// for the ones that live in structs, and straight off the FFI boundary for the
/// ones passed by value. Materialising an out-of-range discriminant into a Rust
/// enum is undefined behaviour, and not the harmless kind: rustc attaches
/// `!range` metadata to the load, so LLVM may legally assume the impossible and
/// a `match` can take an arbitrary arm.
///
/// That is exactly what a MINOR bump would cause. A plugin built against a newer
/// ABI writes a discriminant the older host has no variant for. The version
/// handshake is supposed to refuse that plugin — but then the soundness of every
/// appended variant rests on the handshake being bug-free, forever. A newtype
/// removes the question: any `u32` is a valid value, unknown ones fall to the
/// `_` arm, and "appending a variant is a MINOR change" is true rather than
/// merely usually true.
///
/// The constants below keep the variant names, so this is a source-compatible
/// change at every call site.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Primitive(pub u32);

#[allow(non_upper_case_globals)]
impl Primitive {
    pub const Cuboid: Self = Self(0);
    pub const Sphere: Self = Self(1);
    pub const Plane: Self = Self(2);
    pub const Cylinder: Self = Self(3);
    pub const Capsule: Self = Self(4);
    pub const Torus: Self = Self(5);

    /// Whether this is a value this build knows. Anything else came from a
    /// plugin built against a newer ABI.
    pub const fn is_known(self) -> bool {
        self.0 < 6
    }

    /// The variant name, or `"?"` for a value from a newer ABI.
    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "Cuboid",
            1 => "Sphere",
            2 => "Plane",
            3 => "Cylinder",
            4 => "Capsule",
            5 => "Torus",
            _ => "?",
        }
    }
}

impl core::fmt::Debug for Primitive {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_known() {
            f.write_str(self.name())
        } else {
            write!(f, "Primitive({})", self.0)
        }
    }
}

/// `size` is interpreted per primitive: full extents for `Cuboid` and `Plane`,
/// `x` = radius for `Sphere`, `x` = radius and `y` = height for `Cylinder` and
/// `Capsule`, `x` = major and `y` = minor radius for `Torus`.
#[repr(C)]
pub struct MeshDesc {
    pub primitive: Primitive,
    pub size: Vec3,
}

/// Geometry a plugin generated itself, for [`Interface::add_mesh_data`].
///
/// Pointers are borrowed for the duration of the call only — the host copies
/// every slice into a `Mesh` before returning — so a plugin may point at stack
/// locals or at a `Vec` it drops immediately after.
///
/// Everything but `positions` is optional, and a null pointer is the documented
/// way to say "derive it". That matters more than it looks: a plugin that
/// generates geometry procedurally usually has positions and indices and
/// nothing else, and computing flat normals correctly (per-face, then averaged
/// per vertex) is the kind of thing every author would otherwise reimplement
/// slightly differently.
///
/// [`Interface::add_mesh_data`]: super::Interface::add_mesh_data
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MeshDataDesc {
    pub positions: *const Vec3,
    pub position_count: usize,
    /// Null to have the host compute them from the faces.
    pub normals: *const Vec3,
    pub normal_count: usize,
    /// Null for zeroed UVs. `[u, v]` per vertex.
    pub uvs: *const [f32; 2],
    pub uv_count: usize,
    /// Null for an unindexed triangle list, i.e. every three positions are one
    /// face. Indexed geometry is strongly preferred for anything non-trivial.
    pub indices: *const u32,
    pub index_count: usize,
}

/// Pixel layout of a plugin-created image.
///
/// A deliberately short list. Every format here has to be one the host can
/// validate a byte count against and one wgpu will accept as a sampled texture
/// on every backend — widening it later is additive, guessing now is not.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageFormat(pub u32);

#[allow(non_upper_case_globals)]
impl ImageFormat {
    /// 8-bit RGBA, colour data. Sampled through sRGB decode — the right choice
    /// for anything an artist authored.
    pub const Rgba8Srgb: Self = Self(0);
    /// 8-bit RGBA, raw values. For data an artist did not author: masks,
    /// normal maps, packed channels.
    pub const Rgba8: Self = Self(1);
    /// Single-channel 32-bit float. Heightfields, distance fields, simulation
    /// state a plugin steps each frame.
    pub const R32Float: Self = Self(2);

    pub const fn is_known(self) -> bool {
        self.0 < 3
    }

    /// Bytes per pixel, used to check a plugin's buffer against its dimensions.
    pub const fn bytes_per_pixel(self) -> usize {
        match self.0 {
            0 | 1 => 4,
            2 => 4,
            _ => 0,
        }
    }

    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "Rgba8Srgb",
            1 => "Rgba8",
            2 => "R32Float",
            _ => "?",
        }
    }
}

impl core::fmt::Debug for ImageFormat {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}

/// An image a plugin generated, for [`Interface::add_image`].
///
/// `data` is borrowed for the call only — the host copies it before returning —
/// so a plugin may point at a buffer it drops immediately after.
///
/// [`Interface::add_image`]: super::Interface::add_image
#[repr(C)]
pub struct ImageDesc {
    pub width: u32,
    pub height: u32,
    pub format: ImageFormat,
    pub data: *const u8,
    /// Must be exactly `width * height * format.bytes_per_pixel()`. A mismatch
    /// is refused rather than padded: a short buffer uploaded as a full texture
    /// is a read past the plugin's heap into a GPU upload.
    pub data_len: usize,
}

/// Replaces the pixels of an image already created, during one system call.
///
/// Same shape and same reason as [`MeshSource`]: [`Interface::add_image`] needs
/// the init-time host handle, so without this a plugin could generate a texture
/// once and never again — and a simulation that steps a heightfield every frame
/// is the main thing textures are for on this side.
///
/// [`MeshSource`]: super::MeshSource
/// [`Interface::add_image`]: super::Interface::add_image
#[repr(C)]
pub struct ImageSource {
    /// Overwrite `handle`'s pixels. The dimensions and format are fixed at
    /// creation; only the contents change, so `len` must still match.
    ///
    /// Returns `false` if the handle is unknown or the length is wrong, leaving
    /// the existing pixels untouched.
    pub write: unsafe extern "C" fn(
        src: *mut ImageSource,
        handle: AssetHandle,
        data: *const u8,
        len: usize,
    ) -> bool,
}

/// Textures a material binds, beyond its uniform block.
///
/// Bound from `@group(3) @binding(1)` upward, each texture followed by its
/// sampler — so the first is `1`/`2`, the second `3`/`4`, and so on.
pub const MAX_MATERIAL_TEXTURES: usize = 4;

/// How a custom material blends.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct AlphaMode(pub u32);

#[allow(non_upper_case_globals)]
impl AlphaMode {
    pub const Opaque: Self = Self(0);
    /// Alpha-tested: a fragment is drawn or discarded, never blended.
    pub const Mask: Self = Self(1);
    /// Sorted and blended, drawn after opaques — which is also what makes the
    /// scene's colour available for screen-space refraction.
    pub const Blend: Self = Self(2);

    pub const fn is_known(self) -> bool {
        self.0 < 3
    }
}

impl Default for AlphaMode {
    fn default() -> Self {
        Self::Opaque
    }
}

/// Bytes a plugin material's uniform block may occupy.
///
/// Fixed, and it has to be: a material's bind-group layout is decided once per
/// *type* in Bevy, not per instance, and every plugin material shares one type
/// on the host. So the layout reserves this much and a plugin uses what it
/// needs — 256 bytes is comfortably more than a material's worth of parameters
/// and still one small uniform buffer per instance.
pub const MATERIAL_UNIFORM_CAP: u64 = 256;

/// A custom shaded material: WGSL plus a component whose bytes are its uniform.
///
/// The settings component is the same idea [`PostProcessDesc`] uses — the
/// plugin declares an ordinary component, and its bytes are uploaded as the
/// uniform block. That keeps one description of the parameters, editable in the
/// inspector, serialised into scenes, and readable by the plugin's own systems,
/// instead of a second parallel struct that exists only for the GPU.
///
/// [`PostProcessDesc`]: super::PostProcessDesc
#[repr(C)]
pub struct MaterialShaderDesc {
    /// Stable name, used for the shader asset and for hot-reload.
    pub id: StrRef,
    /// WGSL source. Needs a `fragment` entry point only — the vertex stage is
    /// Bevy's, so skinning, morph targets and the instance-indexed model
    /// transform come for free.
    ///
    /// Compiled through Bevy's normal pipeline rather than validated directly
    /// the way a post-process shader is, so naga_oil imports work here.
    /// `#import bevy_pbr::forward_io::VertexOutput` is required, since that is
    /// what the vertex stage hands the fragment.
    pub wgsl: StrRef,
    /// Component whose bytes become the uniform at `@group(3) @binding(0)`.
    pub settings: ComponentId,
    /// Size of that component. Refused above [`MATERIAL_UNIFORM_CAP`].
    pub settings_size: u64,
    pub alpha_mode: AlphaMode,
    /// Handles from [`Interface::add_image`], bound from `@group(3) @binding(1)`
    /// upward with each texture followed by its sampler. Null for none.
    ///
    /// Fixed at registration rather than per-instance, because the bind-group
    /// layout is decided once for the shared material type — the same
    /// constraint that fixes the uniform size. A material that needs its
    /// texture to change writes new pixels into the same handle with
    /// [`ImageSource::write`] instead of swapping the binding.
    ///
    /// [`Interface::add_image`]: super::Interface::add_image
    pub textures: *const AssetHandle,
    /// Refused above [`MAX_MATERIAL_TEXTURES`].
    pub texture_count: usize,
}

#[repr(C)]
pub struct MaterialDesc {
    /// Linear RGBA.
    pub color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: [f32; 4],
}
