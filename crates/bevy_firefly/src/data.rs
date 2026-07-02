use std::usize;

use bevy::{
    camera::visibility::RenderLayers,
    color::palettes::css::WHITE,
    prelude::*,
    reflect::std_traits::ReflectDefault,
    render::{extract_component::ExtractComponent, render_resource::ShaderType},
};

#[derive(Component, Default, Clone, ExtractComponent, Reflect)]
pub(crate) struct ExtractedWorldData {
    pub camera_pos: Vec2,
}

/// Component that needs to be added to a camera in order to have it render lights.
///
/// # Panics
/// Panics if added to multiple cameras at once.
#[derive(Debug, Component, ExtractComponent, Clone, Reflect)]
// renzora: registered reflection data so the editor inspector and the
// reflection-driven scene serializer can insert/round-trip this component.
#[reflect(Component, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[require(Transform, RenderLayers)]
pub struct FireflyConfig {
    /// Ambient light that will be added over all other lights.  
    ///
    /// **Default:** White.
    pub ambient_color: Color,

    /// Brightness for the ambient light. If 0 and no lights are present, everything will be completely black.
    ///
    /// **Default:** 0.
    pub ambient_brightness: f32,

    /// Light bands will divide the lightmap into brackets of the given size.
    ///
    /// E.g. with `light_bands: Some(0.3)`, all color channels in the `[0-0.3]` interval will be the same color,
    /// in `[0.3-0.6]` another color, and so on.
    ///
    /// **Performance Impact:** None.
    ///
    /// **Default:** None.
    pub light_bands: Option<f32>,

    /// Whether you want to use soft shadows or not.
    ///
    /// **Default:** true.
    pub soft_shadows: bool,

    /// Whether to use occlusion z-sorting or not.
    ///
    /// If this is enabled, shadows cast by occluders won't affect sprites with a higher z position.
    ///
    /// Very useful for top-down games.
    ///
    /// **Performance Impact:** None.
    ///
    /// **Default:** true.
    pub z_sorting: bool,

    pub z_sorting_error_margin: f32,

    /// Field that controls how the normal maps are applied relative to perspective.
    ///
    /// **Performance Impact:** Very minor.
    ///
    /// **Default:** [None](NormalMapMode::None).
    pub normal_mode: NormalMode,

    /// This will control how much the normal map is attenuated before being applied.
    ///
    /// Inside the shader, we perform `mix(normal_map, vec3f(0), attenuation)` to decrease the 'hardness' of the normal map.
    ///
    /// This has the effect of pulling all channels towards (128, 128, 128), making the overall lighting over the surface more plain.
    ///
    /// **Default:** 0.5.
    pub normal_attenuation: f32,

    /// Specifies how other firefly cameras connected to this camera via the [`CombineLightmapTo`] component will
    /// be combined to the resulting lightmap.
    ///
    /// **Default:** Multiply.
    pub combination_mode: CombinationMode,

    /// Sets the lightmap to a custom size or scale.
    ///
    /// This can be used to significantly improve performance or achieve a pixeled lightmap effect.
    ///
    /// Also check the [`lightmap_filtering`](FireflyConfig::lightmap_filtering) field.
    ///
    /// **Default**: `LightmapSize::Window`.
    pub lightmap_size: LightmapSize,

    /// Enables lightmap filtering.
    ///
    /// When used in combination to [`lightmap_size`](FireflyConfig::lightmap_size),
    /// this will determine whether, when upscaled to the screen size, the lightmap will
    /// use linear or point filtering.
    ///
    /// Turn off to pixelate the lightmap.
    ///
    /// **Default**: true.
    pub lightmap_filtering: bool,

    /// Enables 32 bit sizes for the sprite stencil textures
    /// (textures in which the sprite's z coordinate and other values are stored when
    /// used in e.g. occluion z-sorting).
    ///
    /// Normally, WebGPU limits these to 16 bits, however, this can cause
    /// imprecise z-sorting and normal maps since bevy's f32s will be limited to f16 precision.
    ///
    /// Enabling this fixes those precision issues; however, it will prevent your app
    /// from running on web.    
    ///
    /// **Default**: false.
    pub enable_32bit_stencils: bool,
}

/// Specifies how multiple textures will be combined.
///
/// **Default:** Multiply.
#[derive(Clone, Copy, Reflect, Default, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CombinationMode {
    #[default]
    Multiply,
    Max,
    Min,
    Add,
    None,
}

#[derive(Clone, Copy, Reflect, Default, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LightmapSize {
    #[default]
    Window,
    Fixed(UVec2),
    Scaled(f32),
}

/// Options for how the normal maps should be read and used.
///
/// In order to fully use normal maps, you will need to add the [NormalMap](crate::prelude::NormalMap) component to Sprites.
///
/// **Default:** [None](NormalMapMode::None).
#[derive(Debug, Clone, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NormalMode {
    /// No normal maps will be used in rendering.
    None,

    /// This will make it the normal mapping simply be based on the (x, y, z) difference between each light and sprite.
    ///
    /// [LightHeight](crate::prelude::LightHeight) and [SpriteHeight](crate::prelude::SpriteHeight) will be completely ignored.
    ///
    /// This is recommended for classic 2d perspectives, such as those of side-scroller games.   
    Simple,

    /// This will make the normal mapping be based on the difference between the light's and sprite's x-axis and z-axis, but for the y-axis
    /// it will use the [LightHeight](crate::prelude::LightHeight) and [SpriteHeight](crate::prelude::SpriteHeight) components.
    ///
    /// This is recommended for 2d perspectives where you want to simulate 3d lighting, such as top-down games.
    TopDownY,

    TopDownZ,
}

impl Default for FireflyConfig {
    fn default() -> Self {
        Self {
            ambient_color: Color::Srgba(WHITE),
            ambient_brightness: 0.0,
            light_bands: None,
            soft_shadows: true,
            z_sorting: true,
            z_sorting_error_margin: 0.0,
            normal_mode: NormalMode::None,
            normal_attenuation: 0.5,
            combination_mode: CombinationMode::Multiply,
            lightmap_size: LightmapSize::Window,
            lightmap_filtering: true,
            enable_32bit_stencils: false,
        }
    }
}

/// GPU-alligned data from [`FireflyConfig`].
#[derive(ShaderType, Clone)]
pub struct UniformFireflyConfig {
    pub ambient_color: Vec3,
    pub ambient_brightness: f32,
    pub light_bands: f32,
    pub soft_shadows: u32,
    pub z_sorting: u32,
    pub z_sorting_error_margin: f32,
    pub normal_mode: u32,
    pub normal_attenuation: f32,
    pub n_combined_lightmaps: u32,
    pub combination_mode: u32,
    pub texture_scale: Vec2,
}

/// Add this **relationship** component to a camera in order to combine it's lightmap into the result of another lightmap.
///
/// ## Example
/// ```
/// let main_camera = commands.spawn((
///     FireflyConfig {
///         combination_mode: CombinationMode::Add,
///         ..default()
///     },
///     Camera {
///         msaa_writeback: MsaaWriteback::Off,
///         ..default()
///     }
/// )).id();
///
/// commands.spawn((
///     FireflyConfig::default(),
///     Camera {
///         order: -1,
///         output_mode: CameraOutputMode::Skip,
///         ..default()
///     }
///     CombineLightmapTo(main_camera)
/// ));
///
/// commands.spawn((
///     FireflyConfig::default(),
///     Camera {
///         order: -1,
///         output_mode: CameraOutputMode::Skip,
///         ..default()
///     }
///     CombineLightmapTo(main_camera)
/// ));
///
/// ```
///
/// ## Limitations
///
/// A camera that is already the target of this relationship cannot combine its final result
/// into another camera (only the pre-combination lightmap will be combined).
///
/// Ambient light from lightmaps is not transferred over when combined to other lightmaps.
#[derive(Component)]
#[relationship(relationship_target = CombinedLightmaps)]
pub struct CombineLightmapTo(pub Entity);

#[derive(Component)]
#[relationship_target(relationship = CombineLightmapTo, linked_spawn)]
pub struct CombinedLightmaps(Vec<Entity>);

#[derive(Component)]
pub struct ExtractedCombinedLightmaps(pub Vec<Entity>);

#[derive(Component)]
pub struct ExtractedCombineLightmapTo(pub Entity, pub u32);
