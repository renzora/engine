//! Module that prepares BindGroups for GPU use.

use core::f32;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

use crate::{
    CombinedLightMapTextures, LightmapPhase, NormalMapTexture, SpriteStencilTexture,
    buffers::{BinBuffer, BinBuffers, BufferManager, OccluderData, OccluderPointer, VertexBuffer},
    data::{
        CombinationMode, ExtractedCombinedLightmaps, ExtractedWorldData, LightmapSize, NormalMode,
    },
    lights::{LightBatch, LightBatches, LightBindGroups, LightIndex, LightLut, LightPointer},
    occluders::{PolyOccluderIndex, RoundOccluderIndex, point_inside_poly, translate_vertices},
    phases::SpritePhase,
    pipelines::{
        LightPipelineKey, LightmapApplicationPipeline, LightmapCreationPipeline,
        SpecializedApplicationPipeline, SpritePipeline,
    },
    sprites::{
        ExtractedSlices, ExtractedSpriteKind, ExtractedSprites, ImageBindGroups, SpriteAssetEvents,
        SpriteBatch, SpriteBatches, SpriteInstance, SpriteMeta, SpriteViewBindGroup,
    },
    utils::apply_scaling,
};

use bevy::{
    camera::visibility::RenderLayers,
    core_pipeline::tonemapping::{Tonemapping, TonemappingLuts, get_lut_bindings},
    math::{
        Affine3A,
        bounding::{Aabb2d, IntersectsVolume},
    },
    platform::{
        collections::{HashMap, HashSet},
        hash::FixedHasher,
    },
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        render_asset::RenderAssets,
        render_phase::{PhaseItem, ViewBinnedRenderPhases, ViewSortedRenderPhases},
        render_resource::{
            BindGroup, BindGroupEntries, Extent3d, PipelineCache, SpecializedRenderPipelines,
            TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, UniformBuffer,
        },
        renderer::{RenderDevice, RenderQueue},
        texture::{FallbackImage, GpuImage, TextureCache},
        view::{ExtractedView, RetainedViewEntity, ViewTarget, ViewUniforms},
    },
    tasks::{ComputeTaskPool, ParallelSliceMut},
};

use crate::{
    LightMapTexture,
    data::{FireflyConfig, UniformFireflyConfig},
    lights::{ExtractedPointLight, UniformPointLight},
    occluders::{ExtractedOccluder, Occluder2dShape, UniformOccluder, UniformRoundOccluder},
};

/// Camera buffer component containing the data extracted from [`FireflyConfig`].
#[derive(Component)]
pub struct BufferedFireflyConfig(pub UniformBuffer<UniformFireflyConfig>);

/// Plugin responsible for processing extracted entities and
/// sending relevant BindGroups to the GPU. Automatically added by
/// [`FireflyPlugin`](crate::prelude::FireflyPlugin).  
///
/// This is where all the heavy CPU precomputations are done.
pub struct PreparePlugin;

impl Plugin for PreparePlugin {
    fn build(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app.add_systems(
            Render,
            specialize_light_application_pipeline.in_set(RenderSystems::Prepare),
        );

        render_app.add_systems(Render, prepare_data.in_set(RenderSystems::Prepare));
        render_app.add_systems(Render, prepare_config.in_set(RenderSystems::Prepare));
        render_app.add_systems(Render, prepare_lightmap.in_set(RenderSystems::Prepare));

        render_app.add_systems(
            Render,
            (
                prepare_light_luts.in_set(RenderSystems::PrepareBindGroups),
                prepare_sprite_view_bind_groups.in_set(RenderSystems::PrepareBindGroups),
                prepare_sprite_image_bind_groups.in_set(RenderSystems::PrepareBindGroups),
            ),
        );
    }
}

fn specialize_light_application_pipeline(
    views: Query<(
        Entity,
        &ExtractedView,
        &Msaa,
        &FireflyConfig,
        Has<CombinedLightMapTextures>,
    )>,
    pipeline_cache: Res<PipelineCache>,
    pipeline: Res<LightmapApplicationPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<LightmapApplicationPipeline>>,
    mut commands: Commands,
) {
    for (entity, view, _msaa, config, is_combined) in views {
        let mut key = LightPipelineKey::from_target_format(view.target_format);
        if is_combined {
            key |= LightPipelineKey::COMBINE_LIGHTMAPS;
        }

        if config.lightmap_filtering {
            key |= LightPipelineKey::LIGHTMAP_FILTERING;
        }

        let pipeline_id = pipelines.specialize(&pipeline_cache, &pipeline, key);

        commands
            .entity(entity)
            .insert(SpecializedApplicationPipeline {
                id: pipeline_id,
                is_combined,
                filter_lightmap: config.lightmap_filtering,
            });
    }
}

fn prepare_config(
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    configs: Query<(
        Entity,
        &FireflyConfig,
        &ViewTarget,
        Option<&ExtractedCombinedLightmaps>,
    )>,
    mut commands: Commands,
) {
    for (entity, config, view_target, combined_lightmap) in &configs {
        let window_size = view_target.main_texture().size();
        let scale = match config.lightmap_size {
            LightmapSize::Window => vec2(1.0, 1.0),
            LightmapSize::Fixed(size) => vec2(
                size.x as f32 / window_size.width as f32,
                size.y as f32 / window_size.height as f32,
            ),
            LightmapSize::Scaled(scale) => vec2(1.0 / scale, 1.0 / scale),
        };

        let uniform = UniformFireflyConfig {
            ambient_color: config.ambient_color.to_linear().to_vec3(),
            ambient_brightness: config.ambient_brightness,

            light_bands: config.light_bands.unwrap_or(0.0),

            soft_shadows: match config.soft_shadows {
                true => 1,
                false => 0,
            },

            z_sorting: match config.z_sorting {
                false => 0,
                true => 1,
            },

            z_sorting_error_margin: config.z_sorting_error_margin,

            normal_mode: match config.normal_mode {
                NormalMode::None => 0,
                NormalMode::Simple => 1,
                NormalMode::TopDownY => 2,
                NormalMode::TopDownZ => 3,
            },

            normal_attenuation: config.normal_attenuation,

            n_combined_lightmaps: match combined_lightmap {
                None => 0,
                Some(x) => x.0.len() as u32,
            },

            combination_mode: match config.combination_mode {
                CombinationMode::Multiply => 0,
                CombinationMode::Add => 1,
                CombinationMode::Max => 2,
                CombinationMode::Min => 3,
                CombinationMode::None => 4,
            },

            texture_scale: scale,
        };
        let mut buffer = UniformBuffer::<UniformFireflyConfig>::from(uniform);
        buffer.write_buffer(&render_device, &render_queue);
        commands
            .entity(entity)
            .insert(BufferedFireflyConfig(buffer));
    }
}

fn prepare_lightmap(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    mut texture_cache: ResMut<TextureCache>,
    view_targets: Query<(
        Entity,
        &ViewTarget,
        &ExtractedView,
        Option<&ExtractedCombinedLightmaps>,
        &FireflyConfig,
        &Msaa,
    )>,
) {
    for (entity, view_target, view, combined_lightmaps, config, _msaa) in &view_targets {
        let format = view.target_format;
        let window_size = view_target.main_texture().size();

        let size = match config.lightmap_size {
            LightmapSize::Window => window_size,
            LightmapSize::Fixed(size) => Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
            LightmapSize::Scaled(scale) => Extent3d {
                width: (window_size.width as f32 * scale) as u32,
                height: (window_size.height as f32 * scale) as u32,
                depth_or_array_layers: 1,
            },
        };

        let light_map_texture = texture_cache.get(
            &render_device,
            TextureDescriptor {
                label: Some("lightmap"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );

        let stencil_format = match config.enable_32bit_stencils {
            false => TextureFormat::Rgba16Float,
            true => TextureFormat::Rgba32Float,
        };

        let sprite_stencil_texture = texture_cache.get(
            &render_device,
            TextureDescriptor {
                label: Some("sprite stencil"),
                size: view_target.main_texture().size(),
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: stencil_format,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );

        let normal_map_texture = texture_cache.get(
            &render_device,
            TextureDescriptor {
                label: Some("normal map"),
                size: view_target.main_texture().size(),
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba16Float,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );

        commands.entity(entity).insert((
            LightMapTexture(light_map_texture),
            SpriteStencilTexture(sprite_stencil_texture),
            NormalMapTexture(normal_map_texture),
        ));

        if let Some(combined_lightmaps) = combined_lightmaps
            && !combined_lightmaps.0.is_empty()
        {
            let mut size = size;
            size.depth_or_array_layers = combined_lightmaps.0.len() as u32;

            let texture = texture_cache.get(
                &render_device,
                TextureDescriptor {
                    label: Some("combined"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format,
                    usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
            );

            commands
                .entity(entity)
                .insert(CombinedLightMapTextures(texture));
        }
    }
}

pub(crate) fn prepare_data(
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut lights: Query<(
        Entity,
        &ExtractedPointLight,
        &mut LightPointer,
        &LightIndex,
        &mut BinBuffers,
    )>,
    occluders: Query<(&ExtractedOccluder, &RoundOccluderIndex, &PolyOccluderIndex)>,
    cameras: Query<(
        &ExtractedView,
        &RenderLayers,
        &ExtractedWorldData,
        &Projection,
        &SpriteStencilTexture,
        &NormalMapTexture,
        &BufferedFireflyConfig,
        &FireflyConfig,
    )>,
    _phases: Res<ViewBinnedRenderPhases<LightmapPhase>>,
    lightmap_pipeline: Res<LightmapCreationPipeline>,
    mut light_bind_groups: ResMut<LightBindGroups>,
    mut batches: ResMut<LightBatches>,
    round_occluders: Res<BufferManager<UniformRoundOccluder>>,
    poly_occluders: Res<BufferManager<UniformOccluder>>,
    light_buffer: Res<BufferManager<UniformPointLight>>,
    vertices: Res<VertexBuffer>,
    pipeline_cache: Res<PipelineCache>,
) {
    batches.clear();

    let light_bind_groups = &mut *light_bind_groups;

    let mut lights: Vec<_> = lights.iter_mut().collect();

    lights
        .par_splat_map_mut(ComputeTaskPool::get(), None, |_, lights| {
            let mut bind_groups: Vec<(Entity, HashMap<RetainedViewEntity, BindGroup>)> = vec![];

            for (entity, light, light_pointer, light_index, bins) in lights {
                let Some(index) = light_index.0 else {
                    continue;
                };

                light_pointer.0.set(index.index as u32);
                light_pointer.0.write_buffer(&render_device, &render_queue);

                let Some(light_pointer_binding) = light_pointer.0.binding() else {
                    continue;
                };

                let cameras = cameras
                    .iter()
                    .filter_map(|camera| {
                        if !camera.1.intersects(&light.render_layers) {
                            return None;
                        }

                        let Projection::Orthographic(projection) = camera.3 else {
                            return None;
                        };

                        let camera_rect = Rect {
                            min: projection.area.min + camera.2.camera_pos,
                            max: projection.area.max + camera.2.camera_pos,
                        };

                        let light_rect = camera_rect.union_point(light.pos).intersect(Rect {
                            min: light.pos - light.radius,
                            max: light.pos + light.radius,
                        });

                        if light_rect.is_empty() {
                            return None;
                        }

                        let light_aabb = Aabb2d {
                            min: light_rect.min,
                            max: light_rect.max,
                        };

                        let bins = bins
                            .0
                            .entry(camera.0.retained_view_entity)
                            .or_insert(default());
                        bins.reset();

                        Some((camera, light_aabb))
                    })
                    .collect::<Vec<_>>();

                for (occluder, round_index, poly_index) in &occluders {
                    if !light.cast_shadows
                        || !light.render_layers.intersects(&occluder.render_layers)
                    {
                        continue;
                    }

                    let mut any_soft_shadows = false;

                    let mut retained_views: HashSet<_, FixedHasher> = HashSet::default();

                    cameras.iter().for_each(|(camera, light_aabb)| {
                        if !occluder.aabb.intersects(light_aabb)
                            || !camera.1.intersects(&occluder.render_layers)
                        {
                            return;
                        }

                        any_soft_shadows |= camera.7.soft_shadows;

                        retained_views.insert(camera.0.retained_view_entity);
                    });

                    let bins = bins
                        .0
                        .iter_mut()
                        .filter(|(retained_view, _bin)| retained_views.contains(*retained_view))
                        .map(|(_, x)| x)
                        .collect::<Vec<_>>();

                    if let Occluder2dShape::RoundRectangle {
                        half_width,
                        half_height,
                        radius,
                    } = occluder.shape
                    {
                        let Some(occluder_index) = round_index.0 else {
                            continue;
                        };

                        let vertices = vec![
                            vec2(-half_width - radius, -half_height - radius),
                            vec2(-half_width - radius, half_height + radius),
                            vec2(half_width + radius, half_height + radius),
                            vec2(half_width + radius, -half_height - radius),
                        ];

                        let light_pos =
                            Vec2::from_angle(-occluder.rot).rotate(light.pos - occluder.pos);

                        let aabb = Aabb2d {
                            min: vec2(-half_width - radius, -half_height - radius),
                            max: vec2(half_width + radius, half_height + radius),
                        };

                        let isometry = Isometry2d {
                            translation: occluder.pos,
                            rotation: Rot2::radians(occluder.rot),
                        };

                        let vertices =
                            translate_vertices(vertices, isometry.translation, isometry.rotation);

                        let closest = aabb.closest_point(light_pos);
                        let light_inside_occluder = closest == light_pos;

                        push_vertices(
                            bins,
                            &vertices,
                            light.pos,
                            light.core.radius,
                            0,
                            occluder_index.index as u32,
                            closest.distance(light_pos),
                            // 0.0,
                            light_inside_occluder,
                            false,
                            any_soft_shadows,
                            true,
                        );
                    } else {
                        let Some(occluder_index) = poly_index.occluder else {
                            continue;
                        };

                        let Some(vertex_index) = poly_index.vertices else {
                            continue;
                        };

                        let vertices = occluder.vertices();

                        let light_inside_occluder =
                            matches!(occluder.shape, Occluder2dShape::Polygon { .. })
                                && point_inside_poly(
                                    light.pos,
                                    &vertices,
                                    occluder.aabb,
                                    occluder.shape.is_concave(),
                                );

                        let closest = occluder.aabb.closest_point(light.pos);

                        push_vertices(
                            bins,
                            &vertices,
                            light.pos,
                            light.core.radius,
                            vertex_index.index as u32,
                            occluder_index.index as u32,
                            closest.distance(light.pos),
                            light_inside_occluder,
                            true,
                            any_soft_shadows,
                            occluder.shape.is_concave(),
                        );
                    }
                }

                let mut bind_group = HashMap::default();
                for (camera, _) in cameras {
                    let bins = bins.0.get_mut(&camera.0.retained_view_entity).unwrap();
                    bins.write(&render_device, &render_queue);
                    bind_group.insert(
                        camera.0.retained_view_entity,
                        render_device.create_bind_group(
                            "light bind group",
                            &pipeline_cache.get_bind_group_layout(&lightmap_pipeline.layout),
                            &BindGroupEntries::sequential((
                                &lightmap_pipeline.sampler,
                                light_buffer.binding(),
                                light_pointer_binding.clone(),
                                round_occluders.binding(),
                                poly_occluders.binding(),
                                vertices.binding(),
                                bins.bin_binding(),
                                bins.bin_indices_binding(),
                                &camera.4.0.default_view,
                                &camera.5.0.default_view,
                                camera.6.0.binding().unwrap(),
                            )),
                        ),
                    );
                }

                bind_groups.push((*entity, bind_group));
            }
            bind_groups
        })
        .iter()
        .for_each(|bind_groups| {
            for (entity, bind_group) in bind_groups {
                light_bind_groups
                    .values
                    .entry(*entity)
                    .insert(bind_group.clone());

                for retained_view in bind_group.keys() {
                    batches
                        .entry((*retained_view, *entity))
                        .insert(LightBatch { id: *entity });
                }
            }
        });
}

#[derive(Debug, Default)]
struct OccluderSlice {
    pub start_index: usize,
    pub start_vertex: u32,

    pub split: Option<u32>,
    pub length: u32,

    pub start_angle: f32,
    pub angle: f32,
}

impl OccluderSlice {
    fn new(index: usize, vertex: &Vertex) -> Self {
        OccluderSlice {
            start_index: index,
            start_vertex: vertex.index,
            split: None,
            length: 1,
            start_angle: vertex.angle,
            angle: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Vertex {
    pub index: u32,
    pub angle: f32,
}

fn push_vertices(
    mut bins: Vec<&mut BinBuffer>,
    occluder_vertices: &[Vec2],
    light_pos: Vec2,
    light_radius: f32,
    start_vertex: u32,
    index: u32,
    distance: f32,
    rev: bool,
    poly: bool,
    soft_shadows: bool,
    concave: bool,
) {
    let index = match poly {
        true => (1 << 31) | index,
        false => index,
    };

    if !poly && rev {
        bins.iter_mut().for_each(|bins| {
            bins.add_occluder(&OccluderData {
                pointer: OccluderPointer {
                    index,
                    distance,
                    ..default()
                },
                min_angle: 0.0,
                angle: TAU,
            })
        });
        return;
    }
    // info!("pushing vertices: {occluder_vertices:?}");
    // info!("start vertex: {start_vertex}");

    let vertices = occluder_vertices.iter().enumerate().map(|(i, v)| Vertex {
        index: i as u32,
        angle: (v.y - light_pos.y).atan2(v.x - light_pos.x),
    });

    let mut vertices: Vec<_> = if !rev {
        vertices.collect()
    } else {
        vertices.rev().collect()
    };

    let mut round_occlusion = false;

    if concave && rev {
        round_occlusion = true;
    } else {
        loop {
            let last = vertices.last().unwrap().angle;
            let vertex = vertices.first().unwrap().angle;

            let loops = (vertex - last).abs() > PI;

            if (!loops && vertex <= last) || (loops && vertex >= last) {
                break;
            }

            vertices.rotate_right(1);

            if rev && vertices.last().unwrap().index == 0 {
                round_occlusion = true;
                break;
            }
        }
    }

    // info!("");
    // info!("---------");
    let mut push_slice = |slice: &OccluderSlice, vertices: &[Vertex]| {
        if slice.length > 1 {
            let rev: u32 = match rev {
                true => 1,
                false => 0,
            };

            // info!("pushing {slice:?}! poly: {poly}");

            // info!(
            //     "slice start: {}, slice length: {}",
            //     slice.start_vertex, slice.length
            // );

            let min_v = (rev << 29) | (slice.start_vertex + start_vertex);
            let length = slice.length;

            let angle_left = if !soft_shadows || light_radius <= 0.0 {
                0.0
            } else {
                let left = occluder_vertices[vertices[slice.start_index].index as usize];
                (light_pos - left)
                    .normalize()
                    .dot(
                        (light_pos
                            + Vec2::from_angle(FRAC_PI_2)
                                .rotate(left - light_pos)
                                .normalize()
                                * light_radius
                            - left)
                            .normalize(),
                    )
                    .acos()
            };

            let angle_right = if !soft_shadows || light_radius <= 0.0 {
                0.0
            } else {
                let right = occluder_vertices
                    [vertices[slice.start_index + slice.length as usize - 1].index as usize];
                (light_pos - right)
                    .normalize()
                    .dot(
                        (light_pos
                            + Vec2::from_angle(FRAC_PI_2)
                                .rotate(right - light_pos)
                                .normalize()
                                * light_radius
                            - right)
                            .normalize(),
                    )
                    .acos()
            };

            match slice.split {
                None => {
                    let data = OccluderData {
                        pointer: OccluderPointer {
                            index,
                            min_v,
                            split: 0,
                            length,
                            distance,
                        },
                        min_angle: slice.start_angle - angle_left,
                        angle: slice.angle + angle_left + angle_right,
                    };

                    bins.iter_mut().for_each(|bins| {
                        bins.add_occluder(&data);
                    });
                }
                Some(split) => {
                    let data1 = OccluderData {
                        pointer: OccluderPointer {
                            index,
                            min_v: (1 << 30) | min_v,
                            split,
                            length,
                            distance,
                        },
                        min_angle: slice.start_angle - angle_left,
                        angle: slice.angle + angle_left + angle_right,
                    };

                    let data2 = OccluderData {
                        pointer: OccluderPointer {
                            index,
                            min_v: (2 << 30) | min_v,
                            split,
                            length,
                            distance,
                        },
                        min_angle: slice.start_angle - angle_left,
                        angle: slice.angle + angle_left + angle_right,
                    };

                    bins.iter_mut().for_each(|bins| {
                        bins.add_occluder(&data1);
                        bins.add_occluder(&data2);
                    });
                }
            }
        }
    };

    let mut last: Option<&Vertex> = None;
    let mut slice: OccluderSlice = default();

    if !round_occlusion {
        for (index, vertex) in vertices.iter().enumerate() {
            if let Some(last) = last {
                let loops = (vertex.angle - last.angle).abs() > PI;

                // if the next vertex is decreasing
                if (!loops && vertex.angle <= last.angle) || (loops && vertex.angle >= last.angle) {
                    push_slice(&slice, &vertices);
                    slice = OccluderSlice::new(index, vertex);
                }
                // if the next vertex is increasing, simple case
                else if !loops && vertex.angle > last.angle {
                    slice.length += 1;
                    slice.angle += vertex.angle - last.angle;
                }
                // if the next vertex is increasing and loops over
                else {
                    if poly {
                        slice.split = Some(slice.length);
                    }
                    slice.length += 1;

                    slice.angle += vertex.angle - last.angle + TAU;
                }
            } else {
                slice = OccluderSlice::new(index, vertex);
            }

            last = Some(vertex);
        }

        push_slice(&slice, &vertices);
    } else {
        vertices.push(vertices[0]);
        for (index, vertex) in vertices.iter().enumerate() {
            if let Some(last) = last {
                let loops = (vertex.angle - last.angle).abs() > PI;

                if !loops {
                    slice.length += 1;
                    slice.angle += vertex.angle - last.angle;
                } else {
                    slice.split = Some(slice.length);
                    slice.length += 1;

                    slice.angle += vertex.angle - last.angle + TAU;
                }
            } else {
                slice = OccluderSlice::new(index, vertex);
            }

            last = Some(vertex);
        }
        push_slice(&slice, &vertices);
    }
}

fn prepare_light_luts(
    mut commands: Commands,
    view_uniforms: Res<ViewUniforms>,
    render_device: Res<RenderDevice>,
    light_pipeline: Res<LightmapCreationPipeline>,
    // light_pipelines: Res<SpecializedRenderPipelines<LightmapCreationPipeline>>,
    views: Query<(Entity, &Tonemapping), With<ExtractedView>>,
    tonemapping_luts: Res<TonemappingLuts>,
    images: Res<RenderAssets<GpuImage>>,
    fallback_image: Res<FallbackImage>,
    pipeline_cache: Res<PipelineCache>,
) {
    for (entity, tonemapping) in &views {
        let lut_bindings =
            get_lut_bindings(&images, &tonemapping_luts, tonemapping, &fallback_image);
        let view_bind_group = render_device.create_bind_group(
            "light_lut_bind_group",
            &pipeline_cache.get_bind_group_layout(&light_pipeline.lut_layout),
            &BindGroupEntries::with_indices((
                (0, view_uniforms.uniforms.binding().unwrap()),
                (1, lut_bindings.0),
                (2, lut_bindings.1),
            )),
        );

        commands.entity(entity).insert(LightLut(view_bind_group));
    }
}

fn prepare_sprite_view_bind_groups(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    sprite_pipeline: Res<SpritePipeline>,
    view_uniforms: Res<ViewUniforms>,
    views: Query<(Entity, &Tonemapping), With<ExtractedView>>,
    tonemapping_luts: Res<TonemappingLuts>,
    images: Res<RenderAssets<GpuImage>>,
    fallback_image: Res<FallbackImage>,
    pipeline_cache: Res<PipelineCache>,
) {
    let Some(view_binding) = view_uniforms.uniforms.binding() else {
        return;
    };

    for (entity, tonemapping) in &views {
        let lut_bindings =
            get_lut_bindings(&images, &tonemapping_luts, tonemapping, &fallback_image);
        let view_bind_group = render_device.create_bind_group(
            "mesh2d_view_bind_group",
            &pipeline_cache.get_bind_group_layout(&sprite_pipeline.view_layout),
            &BindGroupEntries::with_indices((
                (0, view_binding.clone()),
                (1, lut_bindings.0),
                (2, lut_bindings.1),
            )),
        );

        commands.entity(entity).insert(SpriteViewBindGroup {
            value: view_bind_group,
        });
    }
}

fn prepare_sprite_image_bind_groups(
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut sprite_meta: ResMut<SpriteMeta>,
    sprite_pipeline: Res<SpritePipeline>,
    mut image_bind_groups: ResMut<ImageBindGroups>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    extracted_sprites: Res<ExtractedSprites>,
    extracted_slices: Res<ExtractedSlices>,
    mut phases: ResMut<ViewSortedRenderPhases<SpritePhase>>,
    events: Res<SpriteAssetEvents>,
    mut batches: ResMut<SpriteBatches>,
    pipeline_cache: Res<PipelineCache>,
) {
    let mut is_dummy = UniformBuffer::<u32>::from(0);
    is_dummy.write_buffer(&render_device, &render_queue);

    // If an image has changed, the GpuImage has (probably) changed
    for event in &events.images {
        match event {
            AssetEvent::Added { .. } |
            // Images don't have dependencies
            AssetEvent::LoadedWithDependencies { .. } => {}
            AssetEvent::Unused { id } | AssetEvent::Modified { id } | AssetEvent::Removed { id } => {
                image_bind_groups.values.retain(|k, _| k.0 != *id && k.1 != *id);
            }
        };
    }

    batches.clear();

    // Clear the sprite instances
    sprite_meta.sprite_instance_buffer.clear();

    // Index buffer indices
    let mut index = 0;

    let image_bind_groups = &mut *image_bind_groups;

    for (retained_view, transparent_phase) in phases.iter_mut() {
        let mut current_batch = None;
        let mut batch_item_index = 0;
        let mut batch_image_size = Vec2::ZERO;
        let mut batch_image_handle = AssetId::invalid();
        let mut batch_normal_handle;
        let mut is_dummy;

        // Iterate through the phase items and detect when successive sprites that can be batched.
        // Spawn an entity with a `SpriteBatch` component for each possible batch.
        // Compatible items share the same entity.
        for item_index in 0..transparent_phase.items.len() {
            let item = &transparent_phase.items[item_index];

            let Some(extracted_sprite) = extracted_sprites
                .sprites
                .get(item.extracted_index)
                .filter(|extracted_sprite| extracted_sprite.render_entity == item.entity())
            else {
                // If there is a phase item that is not a sprite, then we must start a new
                // batch to draw the other phase item(s) and to respect draw order. This can be
                // done by invalidating the batch_image_handle
                batch_image_handle = AssetId::invalid();
                continue;
            };

            if batch_image_handle != extracted_sprite.image_handle_id {
                let Some(gpu_image) = gpu_images.get(extracted_sprite.image_handle_id) else {
                    continue;
                };

                batch_image_size = gpu_image.size_2d().as_vec2();
                batch_image_handle = extracted_sprite.image_handle_id;

                (batch_normal_handle, is_dummy) = match extracted_sprite.normal_handle_id {
                    None => (batch_image_handle, true),
                    Some(x) => (x, false),
                };

                let Some(normal_image) = (if is_dummy {
                    Some(gpu_image)
                } else {
                    gpu_images.get(batch_normal_handle)
                }) else {
                    continue;
                };

                let mut dummy_buffer = UniformBuffer::<u32>::from(if is_dummy { 1 } else { 0 });
                dummy_buffer.write_buffer(&render_device, &render_queue);

                let Some(dummy_buffer_binding) = dummy_buffer.binding() else {
                    continue;
                };

                image_bind_groups
                    .values
                    .entry((batch_image_handle, batch_normal_handle, is_dummy))
                    .or_insert_with(|| {
                        render_device.create_bind_group(
                            "sprite_material_bind_group",
                            &pipeline_cache.get_bind_group_layout(&sprite_pipeline.material_layout),
                            &BindGroupEntries::sequential((
                                &gpu_image.texture_view,
                                &normal_image.texture_view,
                                &gpu_image.sampler,
                                dummy_buffer_binding,
                            )),
                        )
                    });

                batch_item_index = item_index;
                current_batch = Some(batches.entry((*retained_view, item.entity())).insert(
                    SpriteBatch {
                        image_handle_id: batch_image_handle,
                        normal_handle_id: batch_normal_handle,
                        normal_dummy: is_dummy,
                        range: index..index,
                    },
                ));
            }
            match extracted_sprite.kind {
                ExtractedSpriteKind::Single {
                    anchor,
                    rect,
                    scaling_mode,
                    custom_size,
                } => {
                    // By default, the size of the quad is the size of the texture
                    let mut quad_size = batch_image_size;
                    let mut texture_size = batch_image_size;

                    // Calculate vertex data for this item
                    // If a rect is specified, adjust UVs and the size of the quad
                    let mut uv_offset_scale = if let Some(rect) = rect {
                        let rect_size = rect.size();
                        quad_size = rect_size;
                        // Update texture size to the rect size
                        // It will help scale properly only portion of the image
                        texture_size = rect_size;
                        Vec4::new(
                            rect.min.x / batch_image_size.x,
                            rect.max.y / batch_image_size.y,
                            rect_size.x / batch_image_size.x,
                            -rect_size.y / batch_image_size.y,
                        )
                    } else {
                        Vec4::new(0.0, 1.0, 1.0, -1.0)
                    };

                    if extracted_sprite.flip_x {
                        uv_offset_scale.x += uv_offset_scale.z;
                        uv_offset_scale.z *= -1.0;
                    }
                    if extracted_sprite.flip_y {
                        uv_offset_scale.y += uv_offset_scale.w;
                        uv_offset_scale.w *= -1.0;
                    }

                    // Override the size if a custom one is specified
                    quad_size = custom_size.unwrap_or(quad_size);

                    // Used for translation of the quad if `TextureScale::Fit...` is specified.
                    let mut quad_translation = Vec2::ZERO;

                    // Scales the texture based on the `texture_scale` field.
                    if let Some(scaling_mode) = scaling_mode {
                        apply_scaling(
                            scaling_mode,
                            texture_size,
                            &mut quad_size,
                            &mut quad_translation,
                            &mut uv_offset_scale,
                        );
                    }

                    let transform = extracted_sprite.transform.affine()
                        * Affine3A::from_scale_rotation_translation(
                            quad_size.extend(1.0),
                            Quat::IDENTITY,
                            ((quad_size + quad_translation) * (-anchor - Vec2::splat(0.5)))
                                .extend(0.0),
                        );

                    // Store the vertex data and add the item to the render phase
                    sprite_meta
                        .sprite_instance_buffer
                        .push(SpriteInstance::from(
                            &transform,
                            &uv_offset_scale,
                            extracted_sprite.transform.translation().z,
                            extracted_sprite.height,
                            extracted_sprite.transform.translation().y,
                        ));

                    if let Some(batch) = current_batch.as_mut() {
                        batch.get_mut().range.end += 1;
                    }
                    // current_batch.as_mut().unwrap().get_mut().range.end += 1;
                    index += 1;
                }
                ExtractedSpriteKind::Slices { ref indices } => {
                    for i in indices.clone() {
                        let slice = &extracted_slices.slices[i];
                        let rect = slice.rect;
                        let rect_size = rect.size();

                        // Calculate vertex data for this item
                        let mut uv_offset_scale: Vec4;

                        // If a rect is specified, adjust UVs and the size of the quad
                        uv_offset_scale = Vec4::new(
                            rect.min.x / batch_image_size.x,
                            rect.max.y / batch_image_size.y,
                            rect_size.x / batch_image_size.x,
                            -rect_size.y / batch_image_size.y,
                        );

                        if extracted_sprite.flip_x {
                            uv_offset_scale.x += uv_offset_scale.z;
                            uv_offset_scale.z *= -1.0;
                        }
                        if extracted_sprite.flip_y {
                            uv_offset_scale.y += uv_offset_scale.w;
                            uv_offset_scale.w *= -1.0;
                        }

                        let transform = extracted_sprite.transform.affine()
                            * Affine3A::from_scale_rotation_translation(
                                slice.size.extend(1.0),
                                Quat::IDENTITY,
                                (slice.size * -Vec2::splat(0.5) + slice.offset).extend(0.0),
                            );

                        // Store the vertex data and add the item to the render phase
                        sprite_meta
                            .sprite_instance_buffer
                            .push(SpriteInstance::from(
                                &transform,
                                &uv_offset_scale,
                                extracted_sprite.transform.translation().z,
                                extracted_sprite.height,
                                extracted_sprite.transform.translation().y,
                            ));

                        if let Some(batch) = current_batch.as_mut() {
                            batch.get_mut().range.end += 1;
                        }
                        // current_batch.as_mut().unwrap().get_mut().range.end += 1;
                        index += 1;
                    }
                }
            }
            transparent_phase.items[batch_item_index]
                .batch_range_mut()
                .end += 1;
        }
        sprite_meta
            .sprite_instance_buffer
            .write_buffer(&render_device, &render_queue);

        if sprite_meta.sprite_index_buffer.len() != 6 {
            sprite_meta.sprite_index_buffer.clear();

            // NOTE: This code is creating 6 indices pointing to 4 vertices.
            // The vertices form the corners of a quad based on their two least significant bits.
            // 10   11
            //
            // 00   01
            // The sprite shader can then use the two least significant bits as the vertex index.
            // The rest of the properties to transform the vertex positions and UVs (which are
            // implicit) are baked into the instance transform, and UV offset and scale.
            // See bevy_sprite/src/render/sprite.wgsl for the details.
            sprite_meta.sprite_index_buffer.push(2);
            sprite_meta.sprite_index_buffer.push(0);
            sprite_meta.sprite_index_buffer.push(1);
            sprite_meta.sprite_index_buffer.push(1);
            sprite_meta.sprite_index_buffer.push(3);
            sprite_meta.sprite_index_buffer.push(2);

            sprite_meta
                .sprite_index_buffer
                .write_buffer(&render_device, &render_queue);
        }
    }
}
