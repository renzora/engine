//! Module containing structs and functions relevant to sprites and normal maps.
//!
//! Firefly uses it's own sprite pipeline inspired by Bevy's. This is needed in order to
//! generate the `Stencil Texture`, a texture containing encoded data of various sprites in the camera view,
//! and the `Normal Map`, the full texture of all normal maps in the view.

use std::ops::Range;

use crate::data::FireflyConfig;
use crate::phases::SpritePhase;
use crate::pipelines::{SpritePipeline, SpritePipelineKey};
use crate::utils::{compute_slices_on_asset_event, compute_slices_on_sprite_change};

use bevy::asset::{AssetEventSystems, AssetPath};
use bevy::image::ImageLoaderSettings;
use bevy::reflect::std_traits::ReflectDefault;
use bevy::render::RenderSystems;
use bevy::render::camera::ExtractedCamera;
use bevy::sprite_render::{SpriteSystems, queue_material2d_meshes};
use bevy::{
    core_pipeline::{
        core_2d::{AlphaMask2d, Opaque2d},
        tonemapping::{DebandDither, Tonemapping},
    },
    ecs::{
        prelude::*,
        query::ROQueryItem,
        system::{SystemParamItem, lifetimeless::*},
    },
    math::{Affine3A, FloatOrd},
    platform::collections::HashMap,
    prelude::*,
    render::{
        Render, RenderApp,
        batching::sort_binned_render_phase,
        render_phase::{
            AddRenderCommand, DrawFunctions, PhaseItem, PhaseItemExtraIndex, RenderCommand,
            RenderCommandResult, SetItemPipeline, TrackedRenderPass, ViewSortedRenderPhases,
            sort_phase_system,
        },
        render_resource::*,
        view::{ExtractedView, Msaa, RenderVisibleEntities, RetainedViewEntity, ViewUniformOffset},
    },
};

use bytemuck::{Pod, Zeroable};
use fixedbitset::FixedBitSet;

pub(crate) struct ExtractedSlice {
    pub offset: Vec2,
    pub rect: Rect,
    pub size: Vec2,
}

pub(crate) struct ExtractedSprite {
    pub main_entity: Entity,
    pub render_entity: Entity,
    pub transform: GlobalTransform,
    /// Change the on-screen size of the sprite
    /// Asset ID of the [`Image`] of this sprite
    /// PERF: storing an `AssetId` instead of `Handle<Image>` enables some optimizations (`ExtractedSprite` becomes `Copy` and doesn't need to be dropped)
    pub image_handle_id: AssetId<Image>,
    pub normal_handle_id: Option<AssetId<Image>>,
    pub flip_x: bool,
    pub flip_y: bool,
    pub kind: ExtractedSpriteKind,
    pub height: f32,
}

pub(crate) enum ExtractedSpriteKind {
    /// A single sprite with custom sizing and scaling options
    Single {
        anchor: Vec2,
        rect: Option<Rect>,
        scaling_mode: Option<SpriteScalingMode>,
        custom_size: Option<Vec2>,
    },
    /// Indexes into the list of [`ExtractedSlice`]s stored in the [`ExtractedSlices`] resource
    /// Used for elements composed from multiple sprites such as text or nine-patched borders
    Slices { indices: Range<usize> },
}

#[derive(Resource, Default)]
pub(crate) struct ExtractedSprites {
    //pub sprites: HashMap<(Entity, MainEntity), ExtractedSprite>,
    pub sprites: Vec<ExtractedSprite>,
}

#[derive(Resource, Default)]
pub(crate) struct ExtractedSlices {
    pub slices: Vec<ExtractedSlice>,
}

#[derive(Resource, Default)]
pub(crate) struct SpriteAssetEvents {
    pub images: Vec<AssetEvent<Image>>,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub(crate) struct SpriteInstance {
    // Affine 4x3 transposed to 3x4
    pub i_model_transpose: [Vec4; 3],
    pub i_uv_offset_scale: [f32; 4],
    pub z: f32,
    pub height: f32,
    pub y: f32,
    pub _padding: f32,
}

impl SpriteInstance {
    #[inline]
    pub fn from(transform: &Affine3A, uv_offset_scale: &Vec4, z: f32, height: f32, y: f32) -> Self {
        let transpose_model_3x3 = transform.matrix3.transpose();
        Self {
            i_model_transpose: [
                transpose_model_3x3.x_axis.extend(transform.translation.x),
                transpose_model_3x3.y_axis.extend(transform.translation.y),
                transpose_model_3x3.z_axis.extend(transform.translation.z),
            ],
            z,
            i_uv_offset_scale: uv_offset_scale.to_array(),
            height,
            y,
            _padding: 0.0,
        }
    }
}

#[derive(Resource)]
pub(crate) struct SpriteMeta {
    pub sprite_index_buffer: RawBufferVec<u32>,
    pub sprite_instance_buffer: RawBufferVec<SpriteInstance>,
}

impl Default for SpriteMeta {
    fn default() -> Self {
        Self {
            sprite_index_buffer: RawBufferVec::<u32>::new(BufferUsages::INDEX),
            sprite_instance_buffer: RawBufferVec::<SpriteInstance>::new(BufferUsages::VERTEX),
        }
    }
}

#[derive(Component)]
pub(crate) struct SpriteViewBindGroup {
    pub value: BindGroup,
}

#[derive(Resource, Deref, DerefMut, Default)]
pub(crate) struct SpriteBatches(pub HashMap<(RetainedViewEntity, Entity), SpriteBatch>);

#[derive(PartialEq, Eq, Clone, Debug)]
pub(crate) struct SpriteBatch {
    pub image_handle_id: AssetId<Image>,
    pub normal_handle_id: AssetId<Image>,
    pub normal_dummy: bool,
    pub range: Range<u32>,
}

#[derive(Resource, Default)]
pub(crate) struct ImageBindGroups {
    pub values: HashMap<(AssetId<Image>, AssetId<Image>, bool), BindGroup>,
}

/// Component you can add to an entity that also has a Sprite, containing the corresponding sprite's normal map.
///
/// The image **MUST** correspond 1:1 with the size and format of the sprite image.
/// E.g. if the sprite image is a sprite sheet, the normal map will also need to be a sprite sheet of exactly the same dimensions, padding, etc.
///
/// # Example
///
/// Automatic image loading:
/// ```
/// commands.spawn((
///     Sprite::from_image(asset_server.load("some_sprite.png")),
///     NormalMap::from_file("some_sprite_normal.png"),
/// ));
/// ```
///
/// Manual image loading:
///
/// ```
/// let image: Handle<Image> = asset_server.load_with_settings("some_sprite_normal.png", |x: &mut ImageLoaderSettings| x.is_srgb = false);
///
/// commands.spawn((
///     Sprite::from_image(asset_server.load("some_sprite.png")),
///     NormalMap::from_image(image),
/// ));
/// ```
///  
/// See [Sprite] for more information on using sprites.
#[derive(Component)]
pub struct NormalMap {
    image: Handle<Image>,
}

/// Optional component you can add to sprites.
///
/// Describes the sprite object's 2d height, useful for emulating 3d lighting in top-down 2d games.
///
/// This is currently used along with the normal maps. It defaults to 0.   
#[derive(Component, Default, Reflect)]
// renzora: registered reflection data so the editor inspector and the
// reflection-driven scene serializer can insert/round-trip this component.
#[reflect(Component, Default)]
pub struct SpriteHeight(pub f32);

impl NormalMap {
    /// Get the handle of the normal map image.
    ///
    /// Useful if e.g. you want to track its loading state.
    pub fn handle(&self) -> Handle<Image> {
        self.image.clone()
    }

    /// Construct a new [NormalMap] from the [path](AssetPath) to the image and the [AssetServer].
    ///
    /// This image file needs to match the corresponding [Sprite] image 1:1.  
    ///
    /// You can use [`.handle()`](NormalMap::handle) to get the resulting image handle.
    pub fn from_file<'a>(path: impl Into<AssetPath<'a>>, asset_server: &AssetServer) -> Self {
        let image: Handle<Image> =
            asset_server.load_with_settings(path, |x: &mut ImageLoaderSettings| x.is_srgb = false);

        Self { image }
    }

    /// Construct a new [NormalMap] from an image handle. It's important that this image is loaded without gamma correction:
    ///
    /// ```
    /// let image: Handle<Image> = asset_server.load_with_settings(path, |x: &mut ImageLoaderSettings| x.is_srgb = false);
    /// ```
    ///
    /// You can use the [`from_file`](NormalMap::from_file) constructor to handle this automatically for you, and later grab the handle
    /// via the [`.handle()`](NormalMap::handle) method.
    pub fn from_image(image: Handle<Image>) -> Self {
        Self { image }
    }
}

/// Plugin that processed and queues sprites into render phases. Added
/// automatically by [`FireflyPlugin`](crate::prelude::FireflyPlugin).
pub struct SpritesPlugin;
impl Plugin for SpritesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            ((
                compute_slices_on_asset_event.before(AssetEventSystems),
                compute_slices_on_sprite_change,
            )
                .in_set(SpriteSystems::ComputeSlices),),
        );

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<ImageBindGroups>()
                .init_resource::<DrawFunctions<SpritePhase>>()
                .init_resource::<SpriteMeta>()
                .init_resource::<ExtractedSprites>()
                .init_resource::<ExtractedSlices>()
                .init_resource::<SpriteAssetEvents>()
                .add_render_command::<SpritePhase, DrawSprite>()
                .init_resource::<ViewSortedRenderPhases<SpritePhase>>()
                .add_systems(
                    Render,
                    (
                        sort_phase_system::<SpritePhase>.in_set(RenderSystems::PhaseSort),
                        queue_sprites
                            .in_set(RenderSystems::Queue)
                            .ambiguous_with(queue_material2d_meshes::<ColorMaterial>),
                        sort_binned_render_phase::<Opaque2d>.in_set(RenderSystems::PhaseSort),
                        sort_binned_render_phase::<AlphaMask2d>.in_set(RenderSystems::PhaseSort),
                    ),
                );
        };
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<SpriteBatches>();
        }
    }
}

fn queue_sprites(
    mut view_entities: Local<FixedBitSet>,
    draw_functions: Res<DrawFunctions<SpritePhase>>,
    pipeline: Res<SpritePipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<SpritePipeline>>,
    pipeline_cache: Res<PipelineCache>,
    extracted_sprites: Res<ExtractedSprites>,
    mut phases: ResMut<ViewSortedRenderPhases<SpritePhase>>,
    mut views: Query<(
        &FireflyConfig,
        &RenderVisibleEntities,
        &ExtractedCamera,
        &ExtractedView,
        &Msaa,
        Option<&Tonemapping>,
        Option<&DebandDither>,
    )>,
) {
    let draw_function = draw_functions.read().id::<DrawSprite>();

    for (config, visible_entities, camera, view, msaa, tonemapping, dither) in &mut views {
        let Some(phase) = phases.get_mut(&view.retained_view_entity) else {
            continue;
        };

        let msaa_key = SpritePipelineKey::from_msaa_samples(msaa.samples());
        let mut view_key = SpritePipelineKey::from_target_format(view.target_format) | msaa_key;

        if !camera.hdr {
            if let Some(tonemapping) = tonemapping {
                view_key |= SpritePipelineKey::TONEMAP_IN_SHADER;
                view_key |= match tonemapping {
                    Tonemapping::None => SpritePipelineKey::TONEMAP_METHOD_NONE,
                    Tonemapping::Reinhard => SpritePipelineKey::TONEMAP_METHOD_REINHARD,
                    Tonemapping::ReinhardLuminance => {
                        SpritePipelineKey::TONEMAP_METHOD_REINHARD_LUMINANCE
                    }
                    Tonemapping::AcesFitted => SpritePipelineKey::TONEMAP_METHOD_ACES_FITTED,
                    Tonemapping::AgX => SpritePipelineKey::TONEMAP_METHOD_AGX,
                    Tonemapping::SomewhatBoringDisplayTransform => {
                        SpritePipelineKey::TONEMAP_METHOD_SOMEWHAT_BORING_DISPLAY_TRANSFORM
                    }
                    Tonemapping::TonyMcMapface => SpritePipelineKey::TONEMAP_METHOD_TONY_MC_MAPFACE,
                    Tonemapping::BlenderFilmic => SpritePipelineKey::TONEMAP_METHOD_BLENDER_FILMIC,
                    Tonemapping::KhronosPbrNeutral => SpritePipelineKey::TONEMAP_METHOD_PBR_NEUTRAL,
                };
            }
            if let Some(DebandDither::Enabled) = dither {
                view_key |= SpritePipelineKey::DEBAND_DITHER;
            }
        }

        if config.enable_32bit_stencils {
            view_key |= SpritePipelineKey::ENABLED_32BIT_STENCIL;
        }

        let pipeline = pipelines.specialize(&pipeline_cache, &pipeline, view_key);

        view_entities.clear();
        if let Some(visible_entities) = visible_entities.get::<Sprite>() {
            view_entities.extend(
                visible_entities
                    .iter_visible()
                    .map(|(_, e)| e.index_u32() as usize),
            );
        }

        phase.items.reserve(extracted_sprites.sprites.len());

        for (index, extracted_sprite) in extracted_sprites.sprites.iter().enumerate() {
            let view_index = extracted_sprite.main_entity.index_u32();

            if !view_entities.contains(view_index as usize) {
                continue;
            }

            // These items will be sorted by depth with other phase items
            let sort_key = FloatOrd(extracted_sprite.transform.translation().z);

            // Add the item to the render phase
            phase.add_transient(SpritePhase {
                draw_function,
                pipeline,
                entity: (
                    extracted_sprite.render_entity,
                    extracted_sprite.main_entity.into(),
                ),
                sort_key,
                // `batch_range` is calculated in `prepare_sprite_image_bind_groups`
                batch_range: 0..0,
                extra_index: PhaseItemExtraIndex::None,
                extracted_index: index,
                indexed: true,
            });
        }
    }
}

pub(crate) type DrawSprite = (
    SetItemPipeline,
    SetSpriteViewBindGroup<0>,
    SetSpriteTextureBindGroup<1>,
    DrawSpriteBatch,
);

pub(crate) struct SetSpriteViewBindGroup<const I: usize>;
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetSpriteViewBindGroup<I> {
    type Param = ();
    type ViewQuery = (Read<ViewUniformOffset>, Read<SpriteViewBindGroup>);
    type ItemQuery = ();

    fn render<'w>(
        _item: &P,
        (view_uniform, sprite_view_bind_group): ROQueryItem<'w, '_, Self::ViewQuery>,
        _entity: Option<()>,
        _param: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        pass.set_bind_group(I, &sprite_view_bind_group.value, &[view_uniform.offset]);
        RenderCommandResult::Success
    }
}
pub(crate) struct SetSpriteTextureBindGroup<const I: usize>;
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetSpriteTextureBindGroup<I> {
    type Param = (SRes<ImageBindGroups>, SRes<SpriteBatches>);
    type ViewQuery = Read<ExtractedView>;
    type ItemQuery = ();

    fn render<'w>(
        item: &P,
        view: ROQueryItem<'w, '_, Self::ViewQuery>,
        _entity: Option<()>,
        (image_bind_groups, batches): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let image_bind_groups = image_bind_groups.into_inner();
        let Some(batch) = batches.get(&(view.retained_view_entity, item.entity())) else {
            return RenderCommandResult::Skip;
        };

        let Some(bind_group) = image_bind_groups.values.get(&(
            batch.image_handle_id,
            batch.normal_handle_id,
            batch.normal_dummy,
        )) else {
            return RenderCommandResult::Skip;
        };

        pass.set_bind_group(I, bind_group, &[]);
        RenderCommandResult::Success
    }
}

pub(crate) struct DrawSpriteBatch;
impl<P: PhaseItem> RenderCommand<P> for DrawSpriteBatch {
    type Param = (SRes<SpriteMeta>, SRes<SpriteBatches>);
    type ViewQuery = Read<ExtractedView>;
    type ItemQuery = ();

    fn render<'w>(
        item: &P,
        view: ROQueryItem<'w, '_, Self::ViewQuery>,
        _entity: Option<()>,
        (sprite_meta, batches): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let sprite_meta = sprite_meta.into_inner();
        let Some(batch) = batches.get(&(view.retained_view_entity, item.entity())) else {
            return RenderCommandResult::Skip;
        };

        let Some(index_buffer) = sprite_meta.sprite_index_buffer.buffer() else {
            return RenderCommandResult::Skip;
        };

        let Some(instance_buffer) = sprite_meta.sprite_instance_buffer.buffer() else {
            return RenderCommandResult::Skip;
        };

        pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint32);
        pass.set_vertex_buffer(0, instance_buffer.slice(..));
        pass.draw_indexed(0..6, 0, batch.range.clone());
        RenderCommandResult::Success
    }
}
