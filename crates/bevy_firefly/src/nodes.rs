//! Module containg `Render Graph Nodes` used by Firefly.  

use bevy::{
    ecs::{query::QueryItem, system::lifetimeless::Read},
    prelude::*,
    render::{
        render_phase::{ViewBinnedRenderPhases, ViewSortedRenderPhases},
        render_resource::{
            BindGroupEntries, PipelineCache, RenderPassColorAttachment, RenderPassDescriptor,
            TextureAspect, TextureFormat, TextureUsages, TextureViewDescriptor,
            TextureViewDimension,
        },
        renderer::{RenderContext, ViewQuery},
        view::{ExtractedView, ViewTarget},
    },
};

use crate::{
    CombinedLightMapTextures, LightMapTexture, LightmapPhase, NormalMapTexture,
    SpriteStencilTexture,
    data::ExtractedCombineLightmapTo,
    phases::SpritePhase,
    pipelines::{LightmapApplicationPipeline, SpecializedApplicationPipeline},
    prepare::BufferedFireflyConfig,
};

pub fn create_lightmap(
    mut render_context: RenderContext,
    lightmap_phases: Res<ViewBinnedRenderPhases<LightmapPhase>>,
    view_query: ViewQuery<(
        &'static ExtractedView,
        &LightMapTexture,
        Option<&ExtractedCombineLightmapTo>,
    )>,
    world: &World,
) {
    let view_entity = view_query.entity();
    let (view, lightmap_texture, combine_lightmap_to) = view_query.into_inner();

    let Some(lightmap_phase) = lightmap_phases.get(&view.retained_view_entity) else {
        return;
    };

    let view = if let Some(combine_lightmap_to) = combine_lightmap_to {
        let lightmap = world
            .get::<CombinedLightMapTextures>(combine_lightmap_to.0)
            .unwrap();

        let format = world
            .get::<ExtractedView>(combine_lightmap_to.0)
            .unwrap()
            .target_format;

        &lightmap.0.texture.create_view(&TextureViewDescriptor {
            label: "layer of combined lightmap texture array".into(),
            format: Some(format),
            dimension: Some(TextureViewDimension::D2Array),
            usage: Some(TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING),
            aspect: TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: Some(1),
            base_array_layer: combine_lightmap_to.1,
            array_layer_count: Some(1),
        })
    } else {
        &lightmap_texture.0.default_view
    };
    let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("lightmap pass"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view,
            resolve_target: None,
            ops: default(),
            depth_slice: None,
        })],
        ..default()
    });

    if let Err(err) = lightmap_phase.render(&mut render_pass, world, view_entity) {
        error!("Error encountered while rendering the stencil phase {err:?}");
    }
}

pub fn apply_lightmap(
    view_query: ViewQuery<(
        Read<ExtractedView>,
        Read<SpecializedApplicationPipeline>,
        Read<BufferedFireflyConfig>,
        Read<ViewTarget>,
        Read<LightMapTexture>,
        Option<Read<CombinedLightMapTextures>>,
        Has<ExtractedCombineLightmapTo>,
    )>,
    mut render_context: RenderContext,

    world: &World,
) {
    let (
        view,
        pipeline_id,
        config,
        view_target,
        light_map_texture,
        combined_textures,
        is_combined_to,
    ) = view_query.into_inner();

    if is_combined_to {
        return;
    }

    let pipeline_cache = world.resource::<PipelineCache>();
    let pipeline = world.resource::<LightmapApplicationPipeline>();

    let Some(render_pipeline) = pipeline_cache.get_render_pipeline(pipeline_id.id) else {
        return;
    };

    let post_process = view_target.post_process_write();
    let Some(config) = config.0.binding() else {
        return;
    };

    let format = view.target_format;

    let bind_group = if !pipeline_id.is_combined {
        render_context.render_device().create_bind_group(
            "apply lightmap bind group simple",
            &pipeline_cache.get_bind_group_layout(
                &pipeline.specialize_layout(pipeline_id.is_combined, pipeline_id.filter_lightmap),
            ),
            &BindGroupEntries::sequential((
                post_process.source,
                &light_map_texture.0.default_view,
                &pipeline.filtering_sampler,
                if pipeline_id.filter_lightmap {
                    &pipeline.filtering_sampler
                } else {
                    &pipeline.non_filtering_sampler
                },
                config,
            )),
        )
    } else {
        let combined_view =
            combined_textures
                .unwrap()
                .0
                .texture
                .create_view(&TextureViewDescriptor {
                    label: "combined lightmap texture array".into(),
                    format: Some(format),
                    dimension: Some(TextureViewDimension::D2Array),
                    usage: Some(TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING),
                    aspect: TextureAspect::All,
                    base_mip_level: 0,
                    mip_level_count: Some(1),
                    base_array_layer: 0,
                    array_layer_count: None,
                });

        render_context.render_device().create_bind_group(
            "apply lightmap bind group combined",
            &pipeline_cache.get_bind_group_layout(
                &pipeline.specialize_layout(pipeline_id.is_combined, pipeline_id.filter_lightmap),
            ),
            &BindGroupEntries::sequential((
                post_process.source,
                &light_map_texture.0.default_view,
                &pipeline.filtering_sampler,
                &pipeline.filtering_sampler,
                config,
                &combined_view,
            )),
        )
    };

    let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("apply lightmap pass"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: post_process.destination,
            resolve_target: None,
            ops: default(),
            depth_slice: None,
        })],
        ..default()
    });

    render_pass.set_render_pipeline(render_pipeline);
    render_pass.set_bind_group(0, &bind_group, &[]);
    render_pass.draw(0..3, 0..1);
}

pub fn sprite(
    view_query: ViewQuery<(&ExtractedView, &SpriteStencilTexture, &NormalMapTexture)>,
    mut render_context: RenderContext,
    world: &World,
) {
    let view_entity = view_query.entity();
    let (view, stencil_texture, normal_map_texture) = view_query.into_inner();

    let Some(sprite_phases) = world.get_resource::<ViewSortedRenderPhases<SpritePhase>>() else {
        return;
    };

    let Some(sprite_phase) = sprite_phases.get(&view.retained_view_entity) else {
        return;
    };

    let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("stencil pass"),
        color_attachments: &[
            Some(RenderPassColorAttachment {
                view: &stencil_texture.0.default_view,
                resolve_target: None,
                ops: default(),
                depth_slice: None,
            }),
            Some(RenderPassColorAttachment {
                view: &normal_map_texture.0.default_view,
                resolve_target: None,
                ops: default(),
                depth_slice: None,
            }),
        ],
        ..default()
    });

    if let Err(err) = sprite_phase.render(&mut render_pass, world, view_entity) {
        error!("Error encountered while rendering the stencil phase {err:?}");
    }
}
