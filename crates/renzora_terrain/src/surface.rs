//! Terrain surface: textured layers placed by slope and height.
//!
//! **Superseded by landscape materials** (`crate::landscape`). The editor
//! converts a saved surface into a landscape material the first time a scene
//! is opened (`landscape::migrate`), and nothing offers a surface to add any
//! more. This module stays for a shipped game holding a scene that was never
//! opened in the editor since: it still draws exactly as it did.
//!
//! A [`TerrainSurface`] on a terrain root paints every chunk without any
//! hand-painting. Layer 1 covers the whole terrain; each later layer is laid
//! over the ones below it wherever the ground's slope and height fall inside
//! that layer's bands. The default is grass, gravel on moderate slopes, rock on
//! cliffs and snow on high, gentle ground, which is what a natural landscape
//! looks like before anyone touches it.
//!
//! The rules are evaluated per pixel in `shaders/terrain_surface.wesl`, so they
//! follow the terrain as it is sculpted: nothing is baked, and there is no mask
//! to repaint after an edit.
//!
//! Textures: each layer names a colour, normal (OpenGL convention) and packed
//! AO / roughness / metallic map, and optionally a height map. Colour and
//! normal are resized to [`SURFACE_TEX_SIZE`], the packed map to
//! [`SURFACE_DATA_SIZE`], each given a full mip chain and stacked into three
//! texture arrays, one slice per layer. Terrain is never metal, so the height
//! map is packed into the metallic channel at build time (from the colour
//! map's brightness when a layer has none): the shader uses it to decide which
//! layer shows where two meet, and where the copies of one layer's anti-tiling
//! meet. A layer whose files are missing or fail to load draws its colour
//! instead, so a project without the texture files still gets a sensible
//! ground rather than an error or a white terrain.
//!
//! Chunks that carry their own `MaterialRef` keep it: an explicit material on
//! a chunk is a deliberate choice, and the material resolver does not take its
//! material back off when the reference goes, so overriding it would draw the
//! chunk twice.

use bevy::asset::{LoadState, RenderAssetUsages};
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::shader::ShaderRef;
use renzora::core::MaterialRef;
use serde::{Deserialize, Serialize};

use crate::data::{TerrainChunkOf, TerrainData};
use crate::material::TerrainCheckerboardMaterial;

/// Layers a surface can have: one texture-array slice each.
pub const MAX_SURFACE_LAYERS: usize = 4;

/// Width and height every layer's colour and normal map is resized to.
/// These carry the detail seen up close, so they get the larger size.
pub const SURFACE_TEX_SIZE: u32 = 2048;

/// Width and height of the packed AO / roughness / height map. Smooth data
/// that nobody reads texel by texel; half size costs a quarter the memory.
pub const SURFACE_DATA_SIZE: u32 = 1024;

const SHADER: &str = "embedded://renzora_terrain/shaders/terrain_surface.wesl";

/// One layer of a [`TerrainSurface`].
///
/// Angles are degrees from flat (0 is level ground, 90 a vertical face);
/// heights are world metres. Every field has a reflect default, so a scene
/// saved before a field existed still loads.
#[derive(Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
#[serde(default)]
pub struct SurfaceLayer {
    #[reflect(default)]
    pub name: String,
    /// Project-relative texture paths. Empty, or a file that does not load,
    /// means the layer draws `tint` as a plain colour.
    #[reflect(default)]
    pub albedo: String,
    #[reflect(default)]
    pub normal: String,
    #[reflect(default)]
    pub arm: String,
    /// Optional greyscale height map (white is high). Decides which layer
    /// shows where two meet; without one, the colour map's brightness stands
    /// in for it.
    #[reflect(default)]
    pub height: String,
    /// Multiplies the colour texture; is the colour itself without one.
    #[reflect(default = "white")]
    pub tint: Vec3,
    /// Ground covered by one repeat of the textures, in metres.
    #[reflect(default = "four")]
    pub tile_size: f32,
    #[reflect(default)]
    pub slope_min: f32,
    #[reflect(default = "ninety")]
    pub slope_max: f32,
    /// Width of the fade at each slope edge, in degrees.
    #[reflect(default = "five")]
    pub slope_blend: f32,
    #[reflect(default = "low")]
    pub height_min: f32,
    #[reflect(default = "high")]
    pub height_max: f32,
    /// Width of the fade at each height edge, in metres.
    #[reflect(default = "five")]
    pub height_blend: f32,
    /// 0..1: how much noise moves this layer's edges, so a band ends in a
    /// ragged natural line rather than a contour.
    #[reflect(default)]
    pub breakup: f32,
    /// Project from all three axes. For layers that land on steep faces,
    /// which a top-down projection stretches into streaks.
    #[reflect(default)]
    pub triplanar: bool,
    #[reflect(default = "yes")]
    pub enabled: bool,
}

fn white() -> Vec3 {
    Vec3::ONE
}
fn four() -> f32 {
    4.0
}
fn five() -> f32 {
    5.0
}
fn ninety() -> f32 {
    90.0
}
fn low() -> f32 {
    -100_000.0
}
fn high() -> f32 {
    100_000.0
}
fn yes() -> bool {
    true
}
fn macro_size() -> f32 {
    120.0
}
fn macro_strength() -> f32 {
    0.4
}
fn far_distance() -> f32 {
    150.0
}
fn height_blending() -> f32 {
    0.8
}
fn normal_strength() -> f32 {
    1.15
}

impl Default for SurfaceLayer {
    fn default() -> Self {
        Self {
            name: "Layer".into(),
            albedo: String::new(),
            normal: String::new(),
            arm: String::new(),
            height: String::new(),
            tint: white(),
            tile_size: four(),
            slope_min: 0.0,
            slope_max: ninety(),
            slope_blend: five(),
            height_min: low(),
            height_max: high(),
            height_blend: five(),
            breakup: 0.0,
            triplanar: false,
            enabled: true,
        }
    }
}

/// Where the default layer textures live in a project. The editor writes the
/// engine's copies there the first time a surface asks for them.
///
/// The folder is versioned because the editor only ever writes a file that is
/// missing: a project holding the first default set under
/// `textures/terrain/<layer>/` would otherwise keep those files forever and
/// never see the current ones.
pub fn default_texture_path(layer: &str, map: &str) -> String {
    format!("textures/terrain/v2/{layer}/{map}.jpg")
}

impl SurfaceLayer {
    fn textured(name: &str, folder: &str, tint: Vec3) -> Self {
        Self {
            name: name.into(),
            albedo: default_texture_path(folder, "albedo"),
            normal: default_texture_path(folder, "normal"),
            arm: default_texture_path(folder, "arm"),
            height: default_texture_path(folder, "height"),
            tint,
            ..default()
        }
    }

    /// Colour, normal, packed map, height: the order of [`SurfaceBuild`]'s
    /// sources.
    pub fn paths(&self) -> [&str; 4] {
        [&self.albedo, &self.normal, &self.arm, &self.height]
    }
}

/// The terrain's automatic surface: up to [`MAX_SURFACE_LAYERS`] layers,
/// each covering what is below it where its rules match.
#[derive(Component, Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
#[reflect(Component, Default)]
#[serde(default)]
pub struct TerrainSurface {
    #[reflect(default)]
    pub layers: Vec<SurfaceLayer>,
    /// 0..1: brightness and warmth that drift across the land at
    /// `macro_size`, so a hillside is not one repeated swatch.
    #[reflect(default = "macro_strength")]
    pub macro_variation: f32,
    /// Metres across one patch of that variation.
    #[reflect(default = "macro_size")]
    pub macro_size: f32,
    /// Distance from the camera, in metres, by which the textures have
    /// switched to their larger, rotated tiling (hides the repeat far away).
    #[reflect(default = "far_distance")]
    pub far_tiling_distance: f32,
    /// 0..1: how much the layers' height maps decide where one layer gives way
    /// to the next. High leaves a crisp edge that follows the textures (grass
    /// in a rock's cracks); 0 is an even cross-fade.
    #[reflect(default = "height_blending")]
    pub height_blending: f32,
    /// Multiplies how far the normal maps tilt the surface.
    #[reflect(default = "normal_strength")]
    pub normal_strength: f32,
    /// Lay each texture down in randomly turned and shifted hex cells, so its
    /// repeat never shows. Costs three texture reads where one would do.
    #[reflect(default = "yes")]
    pub anti_tiling: bool,
}

impl Default for TerrainSurface {
    /// Grass, gravel on moderate slopes, rock on cliffs, snow on high gentle
    /// ground: the engine's default ground textures, placed the way a natural
    /// landscape places them.
    fn default() -> Self {
        Self {
            layers: vec![
                // The tint pulls the photo's mix of green blades and red-brown
                // leaves toward a temperate summer green; untinted, a hillside
                // of it reads as autumn.
                SurfaceLayer {
                    tile_size: 3.0,
                    ..SurfaceLayer::textured("Grass", "grass", Vec3::new(0.36, 0.64, 0.32))
                },
                SurfaceLayer {
                    // Where grass thins out on a slope, before it gives way to
                    // rock.
                    tile_size: 2.5,
                    slope_min: 24.0,
                    slope_max: 40.0,
                    slope_blend: 5.0,
                    breakup: 0.7,
                    ..SurfaceLayer::textured("Gravel", "dirt", Vec3::ONE)
                },
                SurfaceLayer {
                    // Large, so the strata read at the scale of a cliff.
                    tile_size: 8.0,
                    slope_min: 34.0,
                    slope_blend: 4.0,
                    breakup: 0.5,
                    triplanar: true,
                    ..SurfaceLayer::textured("Rock", "rock", Vec3::ONE)
                },
                SurfaceLayer {
                    // Peaks only: on a terrain of a few hundred metres the top
                    // third, and none at all on low ground.
                    tile_size: 5.0,
                    slope_max: 40.0,
                    slope_blend: 6.0,
                    height_min: 190.0,
                    height_blend: 20.0,
                    breakup: 0.8,
                    ..SurfaceLayer::textured("Snow", "snow", Vec3::ONE)
                },
            ],
            macro_variation: macro_strength(),
            macro_size: macro_size(),
            far_tiling_distance: far_distance(),
            height_blending: height_blending(),
            normal_strength: normal_strength(),
            anti_tiling: true,
        }
    }
}

impl TerrainSurface {
    /// The default surface as the engine first shipped it: the first texture
    /// set at its unversioned paths, and the rules that went with it.
    ///
    /// Kept only to recognise it. A scene saved with that surface stored every
    /// one of these values, so it would keep them after the defaults improved;
    /// one that still matches exactly was never touched by anyone and is
    /// upgraded to the current default (see [`upgrade_untouched_surfaces`]).
    fn first_default() -> Self {
        let layer = |name: &str, folder: &str, tint: Vec3| SurfaceLayer {
            name: name.into(),
            albedo: format!("textures/terrain/{folder}/albedo.jpg"),
            normal: format!("textures/terrain/{folder}/normal.jpg"),
            arm: format!("textures/terrain/{folder}/arm.jpg"),
            tint,
            ..default()
        };
        Self {
            layers: vec![
                SurfaceLayer { tile_size: 5.0, ..layer("Grass", "grass", Vec3::new(0.72, 0.95, 0.6)) },
                SurfaceLayer {
                    tile_size: 7.0,
                    slope_min: 26.0,
                    slope_max: 40.0,
                    slope_blend: 6.0,
                    breakup: 0.7,
                    ..layer("Gravel", "dirt", Vec3::ONE)
                },
                SurfaceLayer {
                    tile_size: 10.0,
                    slope_min: 34.0,
                    slope_blend: 5.0,
                    breakup: 0.5,
                    triplanar: true,
                    ..layer("Rock", "rock", Vec3::new(0.72, 0.72, 0.74))
                },
                SurfaceLayer {
                    tile_size: 6.0,
                    slope_max: 38.0,
                    slope_blend: 6.0,
                    height_min: 60.0,
                    height_blend: 8.0,
                    breakup: 0.8,
                    ..layer("Snow", "snow", Vec3::ONE)
                },
            ],
            macro_variation: 0.12,
            macro_size: 90.0,
            far_tiling_distance: 150.0,
            ..Self::default()
        }
    }
}

/// A surface that is still exactly the first shipped default becomes the
/// current default. Compared after the newer fields have taken their own
/// defaults, so an old scene matches; any edit at all, to any field, and the
/// surface is left as it is.
fn upgrade_untouched_surfaces(mut surfaces: Query<&mut TerrainSurface, Changed<TerrainSurface>>) {
    let first = TerrainSurface::first_default();
    for mut surface in &mut surfaces {
        if *surface == first {
            *surface = TerrainSurface::default();
        }
    }
}

// ── Material ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub struct SurfaceLayerGpu {
    pub tint_scale: Vec4,
    pub slope: Vec4,
    pub height: Vec4,
    pub flags: Vec4,
}

#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub struct SurfaceParams {
    pub layers: [SurfaceLayerGpu; MAX_SURFACE_LAYERS],
    pub settings: Vec4,
    /// x = height blending, y = normal strength, z = anti-tiling (0 or 1).
    pub look: Vec4,
}

/// The surface half of [`TerrainSurfaceMaterial`]: the rules and the three
/// texture arrays. Bindings start at 100, past StandardMaterial's. `arm`
/// holds AO, roughness and height (see the module doc).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct TerrainSurfaceExt {
    #[uniform(100)]
    pub params: SurfaceParams,
    #[texture(101, dimension = "2d_array")]
    #[sampler(102)]
    pub albedo: Handle<Image>,
    #[texture(103, dimension = "2d_array")]
    pub normal: Handle<Image>,
    #[texture(104, dimension = "2d_array")]
    pub arm: Handle<Image>,
}

impl MaterialExtension for TerrainSurfaceExt {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    /// The same shader writes the G-buffer under deferred rendering, so the
    /// surface looks the same there (and to anything that shades from the
    /// G-buffer) as it does forward.
    fn deferred_fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

/// On StandardMaterial, so the terrain gets everything a lit surface gets:
/// shadows, image-based light, SSAO, fog and the atmosphere.
pub type TerrainSurfaceMaterial = ExtendedMaterial<StandardMaterial, TerrainSurfaceExt>;

fn pack(surface: &TerrainSurface, textured: [bool; MAX_SURFACE_LAYERS]) -> SurfaceParams {
    let mut params = SurfaceParams::default();
    for (i, layer) in surface.layers.iter().take(MAX_SURFACE_LAYERS).enumerate() {
        params.layers[i] = SurfaceLayerGpu {
            tint_scale: layer.tint.extend(1.0 / layer.tile_size.max(0.01)),
            slope: Vec4::new(layer.slope_min, layer.slope_max, layer.slope_blend.max(0.0), 0.0),
            height: Vec4::new(
                layer.height_min,
                layer.height_max,
                layer.height_blend.max(0.0),
                layer.breakup.clamp(0.0, 1.0),
            ),
            flags: Vec4::new(
                layer.enabled as u32 as f32,
                layer.triplanar as u32 as f32,
                textured[i] as u32 as f32,
                0.0,
            ),
        };
    }
    params.settings = Vec4::new(
        surface.layers.len().min(MAX_SURFACE_LAYERS) as f32,
        surface.macro_variation.max(0.0),
        surface.macro_size.max(1.0),
        surface.far_tiling_distance.max(1.0),
    );
    params.look = Vec4::new(
        surface.height_blending.clamp(0.0, 1.0),
        surface.normal_strength.max(0.0),
        surface.anti_tiling as u32 as f32,
        0.0,
    );
    params
}

// ── Runtime state ────────────────────────────────────────────────────────────

/// What a terrain's surface has loaded and built. Not saved: rebuilt from the
/// [`TerrainSurface`] on load.
#[derive(Component)]
pub struct SurfaceBuild {
    /// The texture paths the arrays were (or are being) built from.
    paths: Vec<[String; 4]>,
    /// One handle per map per layer; `None` where the path is empty.
    sources: Vec<[Option<Handle<Image>>; 4]>,
    /// The arrays are current for `paths`.
    built: bool,
    /// Which layers got their textures in the last build.
    textured: [bool; MAX_SURFACE_LAYERS],
    pub material: Handle<TerrainSurfaceMaterial>,
    /// What a ray tracer shades a hit on this terrain with (see
    /// [`stand_in_chunks`]): the base layer's average colour.
    pub stand_in: Handle<StandardMaterial>,
}

pub struct TerrainSurfacePlugin;

impl Plugin for TerrainSurfacePlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "shaders/terrain_surface.wesl");
        app.add_plugins(MaterialPlugin::<TerrainSurfaceMaterial>::default())
            .register_type::<TerrainSurface>()
            .register_type::<SurfaceLayer>()
            .add_systems(
                Update,
                (
                    upgrade_untouched_surfaces,
                    follow_surface,
                    build_surface_arrays,
                    dress_chunks,
                    undress_chunks,
                    stand_in_chunks,
                )
                    .chain(),
            );
    }
}

/// Load a surface's textures when its paths change, and push rule edits
/// straight into the material (they need no rebuild).
fn follow_surface(
    mut commands: Commands,
    surfaces: Query<(Entity, &TerrainSurface, Option<&mut SurfaceBuild>), Changed<TerrainSurface>>,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<TerrainSurfaceMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut standard: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, surface, build) in surfaces {
        let paths: Vec<[String; 4]> = surface
            .layers
            .iter()
            .take(MAX_SURFACE_LAYERS)
            .map(|l| l.paths().map(str::to_string))
            .collect();
        match build {
            Some(mut build) => {
                if build.paths != paths {
                    build.sources = load_sources(&asset_server, &paths);
                    build.paths = paths;
                    build.built = false;
                } else if !build.built || retry_failed(&asset_server, &build.sources) {
                    // Same files, but one failed earlier and may exist now (the
                    // editor writes the default set on first use).
                    build.built = false;
                }
                if let Some(mut mat) = materials.get_mut(&build.material) {
                    mat.extension.params = pack(surface, build.textured);
                }
            }
            None => {
                let material = materials.add(TerrainSurfaceMaterial {
                    base: StandardMaterial {
                        // What anything that reads only the base sees: a mid
                        // grass colour rather than white.
                        base_color: Color::srgb(0.32, 0.36, 0.2),
                        perceptual_roughness: 0.9,
                        // Ground reflects less at normal incidence than the
                        // default 4%: the sheen that adds at grazing angles
                        // is a large part of what greys out distant slopes.
                        reflectance: 0.4,
                        ..default()
                    },
                    // Placeholder arrays until the textures load: a material
                    // naming an image that does not exist yet cannot build its
                    // bind group, and its meshes would not draw at all. With
                    // no layer marked textured, the shader draws the colours.
                    extension: TerrainSurfaceExt {
                        params: pack(surface, [false; MAX_SURFACE_LAYERS]),
                        albedo: images.add(placeholder_array(TextureFormat::Rgba8UnormSrgb)),
                        normal: images.add(placeholder_array(TextureFormat::Rgba8Unorm)),
                        arm: images.add(placeholder_array(TextureFormat::Rgba8Unorm)),
                    },
                });
                let stand_in = standard.add(stand_in_material(
                    surface.layers.first().map_or(Vec3::splat(0.3), |l| l.tint * 0.35),
                ));
                commands.entity(entity).insert(SurfaceBuild {
                    sources: load_sources(&asset_server, &paths),
                    paths,
                    built: false,
                    textured: [false; MAX_SURFACE_LAYERS],
                    material,
                    stand_in,
                });
            }
        }
    }
}

fn load_sources(asset_server: &AssetServer, paths: &[[String; 4]]) -> Vec<[Option<Handle<Image>>; 4]> {
    paths
        .iter()
        .map(|maps| {
            let mut out: [Option<Handle<Image>>; 4] = Default::default();
            for (slot, path) in maps.iter().enumerate() {
                if path.is_empty() {
                    continue;
                }
                // The colour map is sRGB; normals, ARM and height are data.
                let srgb = slot == 0;
                out[slot] = Some(
                    asset_server
                        .load_builder()
                        .with_settings(move |s: &mut ImageLoaderSettings| s.is_srgb = srgb)
                        .load(path.clone()),
                );
            }
            out
        })
        .collect()
}

/// Ask again for any source that failed. Returns whether any did.
fn retry_failed(asset_server: &AssetServer, sources: &[[Option<Handle<Image>>; 4]]) -> bool {
    let mut any = false;
    for handle in sources.iter().flatten().flatten() {
        if matches!(asset_server.load_state(handle.id()), LoadState::Failed(_)) {
            if let Some(path) = handle.path() {
                asset_server.reload(path.clone());
                any = true;
            }
        }
    }
    any
}

/// Once every source has loaded or failed, stack them into the arrays.
///
/// A source that loads or changes after a build (a texture edited on disk, or
/// one the editor wrote after the first attempt to load it had failed) makes
/// the build stale again.
fn build_surface_arrays(
    mut builds: Query<(&mut SurfaceBuild, &TerrainSurface)>,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<TerrainSurfaceMaterial>>,
    mut standard: ResMut<Assets<StandardMaterial>>,
    mut events: MessageReader<AssetEvent<Image>>,
) {
    let changed: Vec<AssetId<Image>> = events
        .read()
        .filter_map(|e| match e {
            AssetEvent::LoadedWithDependencies { id } | AssetEvent::Modified { id } => Some(*id),
            _ => None,
        })
        .collect();
    if !changed.is_empty() {
        for (mut build, _) in &mut builds {
            if build.built && build.sources.iter().flatten().flatten().any(|h| changed.contains(&h.id())) {
                build.built = false;
            }
        }
    }
    for (mut build, surface) in &mut builds {
        if build.built {
            continue;
        }
        let settled = build.sources.iter().flatten().flatten().all(|h| {
            matches!(asset_server.load_state(h.id()), LoadState::Loaded | LoadState::Failed(_))
        });
        if !settled {
            continue;
        }
        let mut textured = [false; MAX_SURFACE_LAYERS];
        let (mut albedo, mut normal, mut arm) = (Vec::new(), Vec::new(), Vec::new());
        for (layer, maps) in build.sources.iter().enumerate() {
            let image = |slot: usize| maps[slot].as_ref().and_then(|h| images.get(h));
            let colour = image(0).and_then(|i| rgba8_at_size(i, SURFACE_TEX_SIZE));
            let tilt = image(1).and_then(|i| rgba8_at_size(i, SURFACE_TEX_SIZE));
            let mut packed = image(2).and_then(|i| rgba8_at_size(i, SURFACE_DATA_SIZE));
            // A layer is drawn textured only with all three maps: a colour map
            // with a missing normal would light flat, which is worse than the
            // honest plain colour. The height map is optional.
            textured[layer] = colour.is_some() && tilt.is_some() && packed.is_some();
            if let Some(packed) = packed.as_mut() {
                let height = image(3)
                    .and_then(|i| luma_at_size(i, SURFACE_DATA_SIZE))
                    .or_else(|| image(0).and_then(|i| luma_at_size(i, SURFACE_DATA_SIZE)));
                if let Some(height) = height {
                    pack_height(packed, &height);
                }
            }
            albedo.push(colour);
            normal.push(tilt);
            arm.push(packed);
        }
        // The base layer's average colour, for the ray tracer's stand-in.
        let average = match (textured[0], albedo.first().and_then(|s| s.as_ref())) {
            (true, Some(pixels)) => average_srgb(pixels),
            _ => Vec3::ONE,
        } * surface.layers.first().map_or(Vec3::ONE, |l| l.tint);
        if let Some(mut stand_in) = standard.get_mut(&build.stand_in) {
            *stand_in = stand_in_material(average);
        }
        let albedo = images.add(texture_array(
            &albedo,
            SURFACE_TEX_SIZE,
            [255, 255, 255, 255],
            TextureFormat::Rgba8UnormSrgb,
            false,
        ));
        let normal =
            images.add(texture_array(&normal, SURFACE_TEX_SIZE, [128, 128, 255, 255], TextureFormat::Rgba8Unorm, true));
        let arm =
            images.add(texture_array(&arm, SURFACE_DATA_SIZE, [255, 230, 128, 255], TextureFormat::Rgba8Unorm, false));
        if let Some(mut mat) = materials.get_mut(&build.material) {
            mat.extension.albedo = albedo;
            mat.extension.normal = normal;
            mat.extension.arm = arm;
            mat.extension.params = pack(surface, textured);
        }
        build.textured = textured;
        build.built = true;
    }
}

/// `image` as RGBA8 at `size` square, or `None` if it has no CPU data or is
/// not an 8-bit RGBA format.
fn rgba8_at_size(image: &Image, size: u32) -> Option<Vec<u8>> {
    if !matches!(
        image.texture_descriptor.format,
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb
    ) {
        return None;
    }
    let data = image.data.as_ref()?;
    let (w, h) = (image.width() as usize, image.height() as usize);
    if w == 0 || h == 0 || data.len() < w * h * 4 {
        return None;
    }
    let size = size as usize;
    if w == size && h == size {
        return Some(data[..size * size * 4].to_vec());
    }
    Some(resize_bilinear(&data[..w * h * 4], w, h, size, 4))
}

/// `image` as one byte per texel at `size` square: the grey of a greyscale
/// image (which loads as one channel, 8 or 16 bit), or the brightness of a
/// colour one. `None` without CPU data or in a format this does not read.
fn luma_at_size(image: &Image, size: u32) -> Option<Vec<u8>> {
    let data = image.data.as_ref()?;
    let (w, h) = (image.width() as usize, image.height() as usize);
    let n = w * h;
    if n == 0 {
        return None;
    }
    let grey: Vec<u8> = match image.texture_descriptor.format {
        TextureFormat::R8Unorm => data.get(..n)?.to_vec(),
        TextureFormat::Rg8Unorm => data.get(..n * 2)?.chunks_exact(2).map(|p| p[0]).collect(),
        // Little-endian: the second byte is the high one.
        TextureFormat::R16Unorm => data.get(..n * 2)?.chunks_exact(2).map(|p| p[1]).collect(),
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => data
            .get(..n * 4)?
            .chunks_exact(4)
            .map(|p| ((p[0] as u32 * 54 + p[1] as u32 * 183 + p[2] as u32 * 19) >> 8) as u8)
            .collect(),
        _ => return None,
    };
    let size = size as usize;
    if w == size && h == size {
        return Some(grey);
    }
    Some(resize_bilinear(&grey, w, h, size, 1))
}

/// Write `height` (one byte per texel, same size) into the blue channel of
/// the packed AO / roughness / metallic map, where the shader reads it.
///
/// Rescaled to a mean of one half and a fixed spread first: height maps come
/// at any exposure (the default grass one averages a third, the rock two
/// thirds), and the shader compares one layer's height against another's. Raw,
/// the brighter map would win every edge and push its layer's band outward.
fn pack_height(packed: &mut [u8], height: &[u8]) {
    let n = height.len().max(1) as f32;
    let mean = height.iter().map(|&h| h as f32).sum::<f32>() / n;
    let variance = height.iter().map(|&h| (h as f32 - mean).powi(2)).sum::<f32>() / n;
    // A spread of 0.18 of full range; a flat map stays flat at the middle.
    let gain = if variance > 1.0 { 0.18 * 255.0 / variance.sqrt() } else { 0.0 };
    for (px, &h) in packed.chunks_exact_mut(4).zip(height) {
        px[2] = (128.0 + (h as f32 - mean) * gain).round().clamp(0.0, 255.0) as u8;
    }
}

fn resize_bilinear(src: &[u8], w: usize, h: usize, size: usize, channels: usize) -> Vec<u8> {
    let mut out = vec![0u8; size * size * channels];
    for y in 0..size {
        let fy = ((y as f32 + 0.5) * h as f32 / size as f32 - 0.5).clamp(0.0, (h - 1) as f32);
        let (y0, ty) = (fy.floor() as usize, fy.fract());
        let y1 = (y0 + 1).min(h - 1);
        for x in 0..size {
            let fx = ((x as f32 + 0.5) * w as f32 / size as f32 - 0.5).clamp(0.0, (w - 1) as f32);
            let (x0, tx) = (fx.floor() as usize, fx.fract());
            let x1 = (x0 + 1).min(w - 1);
            for c in 0..channels {
                let p = |xx: usize, yy: usize| src[(yy * w + xx) * channels + c] as f32;
                let top = p(x0, y0) + (p(x1, y0) - p(x0, y0)) * tx;
                let bottom = p(x0, y1) + (p(x1, y1) - p(x0, y1)) * tx;
                out[(y * size + x) * channels + c] = (top + (bottom - top) * ty).round() as u8;
            }
        }
    }
    out
}

/// The next mip down: each texel the average of four. A normal map is
/// renormalized after averaging, or distant ground would light as if flatter
/// than it is.
fn downsample(src: &[u8], size: usize, normal: bool) -> Vec<u8> {
    let half = size / 2;
    let mut out = vec![0u8; half * half * 4];
    for y in 0..half {
        for x in 0..half {
            let mut acc = [0u32; 4];
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let i = ((y * 2 + dy) * size + x * 2 + dx) * 4;
                for c in 0..4 {
                    acc[c] += src[i + c] as u32;
                }
            }
            let o = (y * half + x) * 4;
            if normal {
                let v = Vec3::new(acc[0] as f32, acc[1] as f32, acc[2] as f32) / (4.0 * 127.5) - Vec3::ONE;
                let v = v.try_normalize().unwrap_or(Vec3::Z);
                for (c, n) in [v.x, v.y, v.z].into_iter().enumerate() {
                    out[o + c] = ((n + 1.0) * 127.5).round().clamp(0.0, 255.0) as u8;
                }
                out[o + 3] = (acc[3] / 4) as u8;
            } else {
                for c in 0..4 {
                    out[o + c] = ((acc[c] + 2) / 4) as u8;
                }
            }
        }
    }
    out
}

/// A [`MAX_SURFACE_LAYERS`]-slice array of `side` square with full mips,
/// `fallback` filling any slice without pixels. GPU-only once uploaded.
fn texture_array(slices: &[Option<Vec<u8>>], side: u32, fallback: [u8; 4], format: TextureFormat, normal: bool) -> Image {
    let size = side as usize;
    let levels = side.ilog2() + 1;
    let mut data = Vec::new();
    for layer in 0..MAX_SURFACE_LAYERS {
        // Layer-major: every mip of slice 0, then every mip of slice 1.
        let mut level = match slices.get(layer).and_then(|s| s.as_ref()) {
            Some(pixels) => pixels.clone(),
            None => fallback.repeat(size * size),
        };
        let mut level_size = size;
        loop {
            data.extend_from_slice(&level);
            if level_size == 1 {
                break;
            }
            level = downsample(&level, level_size, normal);
            level_size /= 2;
        }
    }
    let mut image = Image::new_uninit(
        Extent3d { width: side, height: side, depth_or_array_layers: MAX_SURFACE_LAYERS as u32 },
        TextureDimension::D2,
        format,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.data = Some(data);
    image.texture_descriptor.mip_level_count = levels;
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        // Terrain is nearly always seen at a grazing angle; 16x keeps the
        // middle distance sharp instead of smearing it into grey.
        anisotropy_clamp: 16,
        ..default()
    });
    image
}

/// The mean of RGBA8 sRGB pixels, in linear colour.
fn average_srgb(pixels: &[u8]) -> Vec3 {
    let mut sum = Vec3::ZERO;
    let mut n = 0.0;
    // Every 7th texel is plenty for a mean, and keeps this off the profile.
    for px in pixels.chunks_exact(4).step_by(7) {
        let c = Color::srgb_u8(px[0], px[1], px[2]).to_linear();
        sum += Vec3::new(c.red, c.green, c.blue);
        n += 1.0;
    }
    if n > 0.0 { sum / n } else { Vec3::splat(0.3) }
}

/// The plain material a ray tracer shades terrain hits with.
fn stand_in_material(linear: Vec3) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::linear_rgb(linear.x, linear.y, linear.z),
        perceptual_roughness: 0.9,
        ..default()
    }
}

/// A 1×1 array of [`MAX_SURFACE_LAYERS`] slices, for the material to bind
/// before its textures exist.
fn placeholder_array(format: TextureFormat) -> Image {
    let mut image = Image::new(
        Extent3d { width: 1, height: 1, depth_or_array_layers: MAX_SURFACE_LAYERS as u32 },
        TextureDimension::D2,
        [128u8, 128, 255, 255].repeat(MAX_SURFACE_LAYERS),
        format,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    image
}

// ── Chunks ───────────────────────────────────────────────────────────────────

/// Give every chunk of a surfaced terrain the surface material, in place of
/// the checkerboard. Chunks with their own `MaterialRef` are left alone.
#[allow(clippy::type_complexity)]
fn dress_chunks(
    mut commands: Commands,
    // A terrain with a landscape material wears that, even if it still has a
    // surface for the moment (the editor converts one into the other).
    builds: Query<&SurfaceBuild, (With<TerrainData>, Without<crate::landscape::TerrainMaterial>)>,
    chunks: Query<
        (Entity, &TerrainChunkOf, Option<&MeshMaterial3d<TerrainSurfaceMaterial>>),
        (With<Mesh3d>, Without<MaterialRef>),
    >,
) {
    for (chunk, of, current) in &chunks {
        let Ok(build) = builds.get(of.0) else { continue };
        if current.is_some_and(|m| m.0 == build.material) {
            continue;
        }
        commands
            .entity(chunk)
            .remove::<MeshMaterial3d<TerrainCheckerboardMaterial>>()
            .insert(MeshMaterial3d(build.material.clone()));
    }
}

/// While a ray tracer asks for them, give every full-detail chunk a plain
/// material to be traced with.
///
/// A ray hit cannot run the surface shader, so Solari traces the terrain with
/// a [`TracedStandIn`](renzora::TracedStandIn) instead: the base layer's
/// average colour, which is what shadows, bounced light and reflections of
/// the ground see. What the camera sees directly is still the full surface,
/// through the G-buffer. A chunk without one is left out of the ray-traced
/// scene entirely, so it neither casts shadows nor bounces light, and the
/// world cache that lights everything else under Solari has nothing to say
/// about it.
///
/// Only full-detail chunks are traced; their coarser levels (`lod`) are for
/// the rasterizer. A checkerboard chunk gets a neutral grey.
///
/// A chunk wearing a landscape material is not this system's: the material
/// crate gives every graph material mesh its own stand-in
/// (`renzora_shader`'s `traced_stand_in`), and a plain `StandardMaterial` is
/// traced as it is. Giving those a stand-in here too had the two systems
/// overwrite each other every frame.
#[allow(clippy::type_complexity)]
fn stand_in_chunks(
    mut commands: Commands,
    wanted: Option<Res<renzora::TracedStandInsWanted>>,
    builds: Query<&SurfaceBuild>,
    chunks: Query<
        (Entity, &TerrainChunkOf, Option<&renzora::TracedStandIn>),
        (
            With<Mesh3d>,
            Without<MaterialRef>,
            Without<MeshMaterial3d<renzora_shader::material::GraphMaterial>>,
            Without<MeshMaterial3d<StandardMaterial>>,
        ),
    >,
    stale: Query<
        Entity,
        (
            With<renzora::TracedStandIn>,
            With<TerrainChunkOf>,
            Without<MeshMaterial3d<renzora_shader::material::GraphMaterial>>,
        ),
    >,
    mut standard: ResMut<Assets<StandardMaterial>>,
    mut grey: Local<Option<Handle<StandardMaterial>>>,
) {
    if wanted.is_none() {
        for chunk in &stale {
            commands.entity(chunk).try_remove::<renzora::TracedStandIn>();
        }
        return;
    }
    for (chunk, of, current) in &chunks {
        let want = match builds.get(of.0) {
            Ok(build) => build.stand_in.clone(),
            Err(_) => grey.get_or_insert_with(|| standard.add(stand_in_material(Vec3::splat(0.25)))).clone(),
        };
        if current.is_some_and(|c| c.0 == want) {
            continue;
        }
        commands.entity(chunk).try_insert(renzora::TracedStandIn(want));
    }
}

/// A terrain whose surface was removed goes back to the checkerboard.
fn undress_chunks(
    mut commands: Commands,
    mut removed: RemovedComponents<TerrainSurface>,
    chunks: Query<(Entity, &TerrainChunkOf), With<MeshMaterial3d<TerrainSurfaceMaterial>>>,
    mut checkers: ResMut<Assets<TerrainCheckerboardMaterial>>,
) {
    let roots: Vec<Entity> = removed.read().collect();
    if roots.is_empty() {
        return;
    }
    let checker = checkers.add(TerrainCheckerboardMaterial::default());
    for root in &roots {
        if let Ok(mut e) = commands.get_entity(*root) {
            e.remove::<SurfaceBuild>();
        }
    }
    for (chunk, of) in &chunks {
        if roots.contains(&of.0) {
            commands
                .entity(chunk)
                .remove::<MeshMaterial3d<TerrainSurfaceMaterial>>()
                .insert(MeshMaterial3d(checker.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_surface_is_four_textured_layers_over_grass() {
        let s = TerrainSurface::default();
        assert_eq!(s.layers.len(), MAX_SURFACE_LAYERS);
        assert_eq!(s.layers[0].name, "Grass");
        assert!(s.layers.iter().all(|l| !l.albedo.is_empty() && l.enabled));
        // Rock takes the steep ground and is projected from the sides.
        assert!(s.layers[2].triplanar && s.layers[2].slope_min > s.layers[1].slope_min);
    }

    #[test]
    fn packing_carries_rules_and_marks_untextured_layers() {
        let s = TerrainSurface::default();
        let p = pack(&s, [true, false, true, true]);
        assert_eq!(p.settings.x, 4.0);
        assert_eq!(p.layers[2].slope.x, 34.0);
        assert_eq!(p.layers[1].flags.z, 0.0);
        assert_eq!(p.layers[0].flags.z, 1.0);
        assert!((p.layers[0].tint_scale.w - 1.0 / 3.0).abs() < 1e-6);
        assert_eq!(p.look, Vec4::new(s.height_blending, s.normal_strength, 1.0, 0.0));
    }

    #[test]
    fn an_array_holds_every_mip_of_every_slice() {
        for side in [SURFACE_TEX_SIZE, SURFACE_DATA_SIZE] {
            let img = texture_array(&[None, None], side, [1, 2, 3, 4], TextureFormat::Rgba8Unorm, false);
            let per_slice: usize = (0..=side.ilog2()).map(|l| ((side >> l) as usize).pow(2) * 4).sum();
            assert_eq!(img.data.as_ref().unwrap().len(), per_slice * MAX_SURFACE_LAYERS);
            assert_eq!(img.texture_descriptor.mip_level_count, side.ilog2() + 1);
        }
    }

    #[test]
    fn the_default_names_a_height_map_for_every_layer_and_keeps_snow_high() {
        let s = TerrainSurface::default();
        assert!(s.layers.iter().all(|l| !l.height.is_empty()));
        // Snow belongs on the peaks of a terrain a few hundred metres tall,
        // not on every hill above 60 m.
        assert!(s.layers[3].height_min >= 150.0);
    }

    #[test]
    fn an_untouched_first_default_is_upgraded_and_an_edited_one_is_not() {
        let mut app = App::new();
        app.add_systems(Update, upgrade_untouched_surfaces);
        let untouched = app.world_mut().spawn(TerrainSurface::first_default()).id();
        let mut edited = TerrainSurface::first_default();
        edited.layers[3].height_min = 80.0;
        let edited = app.world_mut().spawn(edited).id();
        app.update();
        assert_eq!(*app.world().get::<TerrainSurface>(untouched).unwrap(), TerrainSurface::default());
        assert_eq!(app.world().get::<TerrainSurface>(edited).unwrap().layers[3].height_min, 80.0);
    }

    #[test]
    fn packed_heights_are_centred_whatever_the_maps_exposure() {
        let dark: Vec<u8> = (0..64).map(|i| 20 + (i % 8) as u8 * 5).collect();
        let mut packed = vec![0u8; 64 * 4];
        pack_height(&mut packed, &dark);
        let mean = packed.chunks_exact(4).map(|p| p[2] as f32).sum::<f32>() / 64.0;
        assert!((mean - 128.0).abs() < 2.0, "{mean}");
        // A flat map is flat at the middle, not black.
        pack_height(&mut packed, &[200u8; 64]);
        assert!(packed.chunks_exact(4).all(|p| p[2] == 128));
    }

    #[test]
    fn a_greyscale_height_map_is_read_from_its_one_channel() {
        let img = Image::new(
            Extent3d { width: 2, height: 2, depth_or_array_layers: 1 },
            TextureDimension::D2,
            vec![10, 20, 30, 40],
            TextureFormat::R8Unorm,
            RenderAssetUsages::MAIN_WORLD,
        );
        assert_eq!(luma_at_size(&img, 2), Some(vec![10, 20, 30, 40]));
        assert_eq!(luma_at_size(&img, 4).map(|v| v.len()), Some(16));
    }

    #[test]
    fn a_normal_mip_stays_unit_length() {
        // Two opposite tilts average to straight up, not to a short vector.
        let left = [64u8, 128, 230, 255];
        let right = [192u8, 128, 230, 255];
        let src = [left, right, left, right].concat();
        let out = downsample(&src, 2, true);
        let n = Vec3::new(out[0] as f32, out[1] as f32, out[2] as f32) / 127.5 - Vec3::ONE;
        assert!((n.length() - 1.0).abs() < 0.02, "{n}");
        assert!(n.z > 0.99);
    }

    /// Bevy compiles the shader at run time, so a mistake in it is a terrain
    /// that silently fails to draw, not a build error. naga does not speak
    /// WESL's imports or `@if`, so this checks the forward variant: imports
    /// and prepass-only lines go, and the few Bevy items the shader uses are
    /// stood in for. Field names follow Bevy's; the stand-ins check our code,
    /// not Bevy's.
    #[test]
    fn the_surface_shader_validates() {
        const STUBS: &str = "
struct View { world_position: vec3<f32> };
@group(0) @binding(0) var<uniform> view: View;
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
};
struct FragmentOutput { @location(0) color: vec4<f32> };
struct StandardMaterialInput { base_color: vec4<f32>, metallic: f32, perceptual_roughness: f32 };
struct PbrInput { material: StandardMaterialInput, diffuse_occlusion: vec3<f32>, N: vec3<f32> };
fn pbr_input_from_standard_material(in: VertexOutput, is_front: bool) -> PbrInput {
    var p: PbrInput;
    p.diffuse_occlusion = vec3<f32>(1.0);
    p.N = in.world_normal;
    return p;
}
fn apply_pbr_lighting(p: PbrInput) -> vec4<f32> { return p.material.base_color; }
fn main_pass_post_lighting_processing(p: PbrInput, c: vec4<f32>) -> vec4<f32> { return c; }
";
        let body: String = include_str!("shaders/terrain_surface.wesl")
            .lines()
            .filter(|l| !l.trim_start().starts_with("import ") && !l.contains("import "))
            .filter(|l| !l.trim_start().starts_with("@if(PREPASS_PIPELINE)"))
            .filter(|l| !l.trim_start().starts_with("@if(VISIBILITY_RANGE_DITHER)"))
            .filter(|l| !l.trim_start().starts_with("@if(MESHLET_MESH_MATERIAL_PASS)"))
            .map(|l| {
                l.replace("@if(!PREPASS_PIPELINE) ", "")
                    .replace("@if(!MESHLET_MESH_MATERIAL_PASS) ", "")
                    .replace("pbr_functions::", "")
            })
            .collect::<Vec<_>>()
            .join("\n");
        renzora::wgsl::check(&format!("{STUBS}\n{body}"))
            .unwrap_or_else(|err| panic!("terrain_surface.wesl: {err}"));
    }

    #[test]
    fn an_old_scene_without_newer_fields_still_reflects_a_surface() {
        use bevy::reflect::structs::DynamicStruct;
        use bevy::reflect::FromReflect;
        let mut d = DynamicStruct::default();
        d.insert("macro_variation", 0.3f32);
        let s = TerrainSurface::from_reflect(&d).expect("missing fields take their defaults");
        assert_eq!(s.macro_variation, 0.3);
        assert_eq!(s.macro_size, macro_size());
        assert_eq!(s.height_blending, height_blending());
        assert!(s.anti_tiling);
    }
}
