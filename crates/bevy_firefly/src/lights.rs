use bevy::{
    camera::visibility::{RenderLayers, VisibilityClass, add_visibility_class},
    color::palettes::css::WHITE,
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    ecs::{
        change_detection::Tick,
        query::ROQueryItem,
        system::{
            SystemParamItem,
            lifetimeless::{Read, SRes},
        },
    },
    platform::collections::HashMap,
    prelude::*,
    reflect::std_traits::ReflectDefault,
    render::{
        Render, RenderApp, RenderSystems,
        batching::sort_binned_render_phase,
        camera::ExtractedCamera,
        render_phase::{
            AddRenderCommand, BinnedRenderPhaseType, DrawFunctions, InputUniformIndex, PhaseItem,
            RenderCommand, RenderCommandResult, SetItemPipeline, TrackedRenderPass,
            ViewBinnedRenderPhases,
        },
        render_resource::{
            BindGroup, PipelineCache, ShaderType, SpecializedRenderPipelines, StorageBuffer,
        },
        sync_world::SyncToRenderWorld,
        view::{ExtractedView, RenderVisibleEntities, RetainedViewEntity, ViewUniformOffset},
    },
};
use bytemuck::NoUninit;

use crate::{
    LightBatchSetKey,
    buffers::{BinBuffers, BufferIndex},
    change::Changes,
    data::ExtractedCombineLightmapTo,
    phases::LightmapPhase,
    pipelines::{LightPipelineKey, LightmapCreationPipeline},
    visibility::VisibilityTimer,
};

/// Point light with adjustable fields.
#[derive(Debug, Component, Clone, Reflect)]
// renzora: registered reflection data so the editor inspector and the
// reflection-driven scene serializer can insert/round-trip this component.
#[reflect(Component, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[require(
    SyncToRenderWorld,
    Transform,
    VisibilityClass,
    ViewVisibility,
    VisibilityTimer,
    LightHeight,
    Changes,
    RenderLayers
)]
#[component(on_add = add_visibility_class::<PointLight2d>)]
pub struct PointLight2d {
    /// Color of the point light. Alpha is ignored.
    ///
    /// **Default:** White.
    pub color: Color,

    /// Intensity of the point light.
    ///
    /// **Default:** 1.
    pub intensity: f32,

    /// Outer range of the point light.
    pub radius: f32,

    /// Type of falloff for this light.
    ///
    /// **Default:** [InverseSquare](Falloff::InverseSquare).
    pub falloff: Falloff,

    /// The core of the light.
    ///
    /// This is the inner section of the light that is usually brighter.
    ///
    /// The soft shadows are cast based on the radius of the core.
    pub core: LightCore,

    /// Optional parameter to constrain the angle of a light.
    ///
    /// The direction of the angle is based on the **UP** direction of the entity.
    /// Can be moved by rotating the entity.  
    ///
    /// **Default:** LightAngle::FULL.
    pub angle: LightAngle,

    /// Whether this light should cast shadows or not with the existent occluders.
    ///
    /// **Performance Impact:** Major.
    ///
    /// **Default:** true.
    pub cast_shadows: bool,

    /// Offset position of the light.
    ///
    /// Useful if you want to add a light component on an entity and change it's position,
    /// without needing to create a child entity for it.
    ///
    /// **Default:** [Vec3::ZERO].
    pub offset: Vec3,
}

impl Default for PointLight2d {
    fn default() -> Self {
        Self {
            color: bevy::prelude::Color::Srgba(WHITE),
            intensity: 1.,
            radius: 100.,
            falloff: Falloff::InverseSquare { intensity: 0.0 },
            core: default(),
            angle: LightAngle::FULL,
            cast_shadows: true,
            offset: Vec3::ZERO,
        }
    }
}

/// Optional component you can add to lights.
///
/// Describes the light's 2d height, useful for emulating 3d lighting in top-down 2d games.
///
/// This is currently used along with the normal maps.
///
/// **Default:** 0.   
#[derive(Component, Default, Reflect)]
#[reflect(Component, Default)]
pub struct LightHeight(pub f32);

#[derive(Debug, Clone, Copy, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// The angle of the light. Value is interpolated between inner and outer angles to create a smooth transition.
pub struct LightAngle {
    /// The inner angle of a light, in degrees. Should be less than or equial to the outer angle.
    pub inner: f32,
    /// The outer angle of a light, in degrees. Should be greater than or equal to the inner angle.
    pub outer: f32,
}

impl Default for LightAngle {
    fn default() -> Self {
        Self::FULL
    }
}

impl LightAngle {
    pub const FULL: Self = Self {
        inner: 360.0,
        outer: 360.0,
    };
}

/// An enum describing the falloff of a light's intensity.
#[derive(Debug, Clone, Copy, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Falloff {
    /// The light decreases inversely proportial to the square distance towards the source.  
    ///
    /// The intensity parameter will increase the speed at which the light fades. Can be negative or positive.
    InverseSquare { intensity: f32 },
    /// The light decreases linearly with the distance towards the source.
    ///
    /// The intensity parameter will increase the speed at which the light fades. Can be negative or positive.
    Linear { intensity: f32 },
    /// There is no falloff. The light will have a constant intensity.  
    None,
}

impl Falloff {
    pub const INVERSE_SQUARE: Self = Self::InverseSquare { intensity: 0.0 };
    pub const LINEAR: Self = Self::Linear { intensity: 0.0 };
    pub const NONE: Self = Self::None;

    pub fn inverse_square(intensity: f32) -> Falloff {
        Falloff::InverseSquare { intensity }
    }

    pub fn linear(intensity: f32) -> Falloff {
        Falloff::Linear { intensity }
    }

    pub fn none() -> Falloff {
        Falloff::None
    }

    pub fn intensity(&self) -> f32 {
        match *self {
            Falloff::InverseSquare { intensity } => intensity,
            Falloff::Linear { intensity } => intensity,
            Falloff::None => 0.0,
        }
    }
}

/// The light's core. This is what determines the softness of shadows if [soft_shadows](crate::prelude::FireflyConfig::soft_shadows) is enabled.
#[derive(Clone, Copy, Debug, Reflect)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LightCore {
    /// The radius of the core. This must be less than the actual radius of the light.
    ///
    /// **Default:** 5.0.
    pub radius: f32,
    /// A boost to the core's intensity.
    ///
    /// If set to 0, the core will have a constant intensity equal to that of the light.
    ///
    /// Otherwise, the core will interpolate between `intensity + boost` and `intensity` based on the provided [`Falloff`].
    ///
    /// **Default:** 5.0.
    pub boost: f32,
    /// The core's falloff.
    ///
    ///  **Default:** InverseSquare { intensity: 0.0 }
    pub falloff: Falloff,
}

impl Default for LightCore {
    fn default() -> Self {
        LightCore {
            radius: 5.0,
            boost: 0.0,
            falloff: Falloff::InverseSquare { intensity: 0.0 },
        }
    }
}

impl LightCore {
    pub const NONE: Self = LightCore {
        radius: 0.0,
        boost: 0.0,
        falloff: Falloff::None,
    };

    pub fn from_radius_boost(radius: f32, boost: f32) -> LightCore {
        LightCore {
            radius,
            boost,
            falloff: Falloff::InverseSquare { intensity: 0.0 },
        }
    }
    pub fn from_radius(radius: f32) -> LightCore {
        LightCore {
            radius,
            boost: 5.0,
            falloff: Falloff::InverseSquare { intensity: 0.0 },
        }
    }
    pub fn with_boost(&self, boost: f32) -> LightCore {
        let mut res = *self;
        res.boost = boost;
        res
    }
    pub fn with_falloff(&self, falloff: Falloff) -> LightCore {
        let mut res = *self;
        res.falloff = falloff;
        res
    }
}

/// The data that is extracted to the render world from a [`PointLight2d`].
#[derive(Component, Clone)]
#[require(BinBuffers, LightIndex, LightPointer)]
pub struct ExtractedPointLight {
    pub pos: Vec2,
    pub color: Color,
    pub intensity: f32,
    pub radius: f32,
    pub falloff: Falloff,
    pub core: LightCore,
    pub angle: LightAngle,
    pub cast_shadows: bool,
    pub dir: Vec2,
    pub z: f32,
    pub height: f32,
    pub changes: Changes,
    pub render_layers: RenderLayers,
}

impl PartialEq for ExtractedPointLight {
    fn eq(&self, other: &Self) -> bool {
        self.pos == other.pos && self.radius == other.radius
    }
}

/// Data that is sent to the GPU for each visible [`PointLight2d`].
#[repr(C)]
#[derive(Default, Clone, Copy, ShaderType, NoUninit)]
pub struct UniformPointLight {
    pub pos: Vec2,
    pub intensity: f32,
    pub radius: f32,

    pub color: Vec4,

    pub core_radius: f32,
    pub core_boost: f32,
    pub core_falloff: u32,
    pub core_falloff_intensity: f32,

    pub falloff: u32,
    pub falloff_intensity: f32,

    pub inner_angle: f32,
    pub outer_angle: f32,

    pub dir: Vec2,

    pub z: f32,
    pub height: f32,
}

/// Render World component that contains the buffer a [`PointLight2d`] writes to each frame.   
#[derive(Component, Default)]
pub struct LightPointer(pub StorageBuffer<u32>);

/// Plugin responsible for functionality related to lights. Added automatically
/// by [`FireflyPlugin`](crate::prelude::FireflyPlugin).
pub struct LightPlugin;
impl Plugin for LightPlugin {
    fn build(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<LightBindGroups>();
            render_app.init_resource::<DrawFunctions<LightmapPhase>>();
            render_app.init_resource::<ViewBinnedRenderPhases<LightmapPhase>>();

            render_app.add_render_command::<LightmapPhase, DrawLightmap>();

            render_app.add_systems(
                Render,
                sort_binned_render_phase::<LightmapPhase>.in_set(RenderSystems::PhaseSort),
            );

            render_app.add_systems(Render, queue_lights.in_set(RenderSystems::Queue));
        }
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<LightBatches>();
        }
    }
}

#[derive(Resource, Deref, DerefMut, Default)]
pub(crate) struct LightBatches(pub HashMap<(RetainedViewEntity, Entity), LightBatch>);

#[derive(PartialEq, Eq, Clone, Debug)]
pub(crate) struct LightBatch {
    pub id: Entity,
}

#[derive(Resource, Default)]
pub(crate) struct LightBindGroups {
    pub values: HashMap<Entity, HashMap<RetainedViewEntity, BindGroup>>,
}

#[derive(Component)]
pub(crate) struct LightLut(pub BindGroup);

fn queue_lights(
    light_draw_functions: Res<DrawFunctions<LightmapPhase>>,
    pipeline: Res<LightmapCreationPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<LightmapCreationPipeline>>,
    mut lightmap_phases: ResMut<ViewBinnedRenderPhases<LightmapPhase>>,
    views: Query<(
        &ExtractedView,
        &ExtractedCamera,
        &RenderVisibleEntities,
        &Msaa,
        Option<&Tonemapping>,
        Option<&DebandDither>,
        Option<&ExtractedCombineLightmapTo>,
    )>,
    pipeline_cache: Res<PipelineCache>,
) {
    let draw_lightmap_function = light_draw_functions.read().id::<DrawLightmap>();

    for (view, camera, visible_entities, msaa, tonemapping, dither, combined_lightmap) in &views {
        let Some(lightmap_phase) = lightmap_phases.get_mut(&view.retained_view_entity) else {
            continue;
        };

        let (target_format, msaa) = if let Some(combined_lightmap) = combined_lightmap {
            let view = views.get(combined_lightmap.0).unwrap();
            (view.0.target_format, view.3)
        } else {
            (view.target_format, msaa)
        };

        let msaa_key = LightPipelineKey::from_msaa_samples(msaa.samples());
        let mut view_key = LightPipelineKey::from_target_format(target_format) | msaa_key;

        if camera
            .compositing_space
            .is_some_and(|s| s == CompositingSpace::Srgb)
        {
            view_key |= LightPipelineKey::SRGB_COMPOSITING;
        }
        if camera
            .compositing_space
            .is_some_and(|s| s == CompositingSpace::Oklab)
        {
            view_key |= LightPipelineKey::OKLAB_COMPOSITING;
        }

        if !camera.hdr {
            if let Some(tonemapping) = tonemapping {
                view_key |= LightPipelineKey::TONEMAP_IN_SHADER;
                view_key |= match tonemapping {
                    Tonemapping::None => LightPipelineKey::TONEMAP_METHOD_NONE,
                    Tonemapping::Reinhard => LightPipelineKey::TONEMAP_METHOD_REINHARD,
                    Tonemapping::ReinhardLuminance => {
                        LightPipelineKey::TONEMAP_METHOD_REINHARD_LUMINANCE
                    }
                    Tonemapping::AcesFitted => LightPipelineKey::TONEMAP_METHOD_ACES_FITTED,
                    Tonemapping::AgX => LightPipelineKey::TONEMAP_METHOD_AGX,
                    Tonemapping::SomewhatBoringDisplayTransform => {
                        LightPipelineKey::TONEMAP_METHOD_SOMEWHAT_BORING_DISPLAY_TRANSFORM
                    }
                    Tonemapping::TonyMcMapface => LightPipelineKey::TONEMAP_METHOD_TONY_MC_MAPFACE,
                    Tonemapping::BlenderFilmic => LightPipelineKey::TONEMAP_METHOD_BLENDER_FILMIC,
                    Tonemapping::KhronosPbrNeutral => LightPipelineKey::TONEMAP_METHOD_PBR_NEUTRAL,
                };
            }
            if let Some(DebandDither::Enabled) = dither {
                view_key |= LightPipelineKey::DEBAND_DITHER;
            }
        }

        let pipeline = pipelines.specialize(&pipeline_cache, &pipeline, view_key);

        if let Some(visible_entities) = visible_entities.get::<PointLight2d>() {
            for (render_entity, visible_entity) in visible_entities.iter_visible() {
                let batch_set_key = LightBatchSetKey {
                    pipeline,
                    draw_function: draw_lightmap_function,
                };

                lightmap_phase.add(
                    batch_set_key,
                    (),
                    (*render_entity, *visible_entity),
                    InputUniformIndex::default(),
                    BinnedRenderPhaseType::NonMesh,
                );
            }
        }
    }
}

pub(crate) type DrawLightmap = (SetItemPipeline, SetLightTextureBindGroup, DrawLightBatch);

pub(crate) struct SetLightTextureBindGroup;
impl<P: PhaseItem> RenderCommand<P> for SetLightTextureBindGroup {
    type Param = (SRes<LightBindGroups>, SRes<LightBatches>);
    type ViewQuery = (Read<ExtractedView>, Read<ViewUniformOffset>, Read<LightLut>);
    type ItemQuery = ();

    fn render<'w>(
        item: &P,
        (view, view_uniform_offset, lut): ROQueryItem<'w, '_, Self::ViewQuery>,
        _entity: Option<()>,
        (image_bind_groups, batches): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let image_bind_groups = image_bind_groups.into_inner();
        let Some(batch) = batches.get(&(view.retained_view_entity, item.entity())) else {
            return RenderCommandResult::Skip;
        };

        pass.set_bind_group(0, &lut.0, &[view_uniform_offset.offset]);
        pass.set_bind_group(
            1,
            image_bind_groups
                .values
                .get(&batch.id)
                .unwrap()
                .get(&view.retained_view_entity)
                .unwrap(),
            &[],
        );

        RenderCommandResult::Success
    }
}

pub(crate) struct DrawLightBatch;
impl<P: PhaseItem> RenderCommand<P> for DrawLightBatch {
    type Param = ();
    type ViewQuery = Read<ExtractedView>;
    type ItemQuery = ();

    fn render<'w>(
        _: &P,
        _: ROQueryItem<'w, '_, Self::ViewQuery>,
        _entity: Option<()>,
        _: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        pass.draw(0..3, 0..1);
        RenderCommandResult::Success
    }
}

/// Buffer index that each visible light gets assigned
/// corresponding to its [`BufferManager`](crate::buffers::BufferManager) slot.  
#[derive(Component, Default)]
pub struct LightIndex(pub Option<BufferIndex>);
