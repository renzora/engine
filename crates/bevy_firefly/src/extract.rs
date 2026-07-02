//! This module extracts data from the Main World to the Render World.

use bevy::{
    camera::visibility::RenderLayers,
    platform::collections::{HashSet, hash_map::Entry},
    prelude::*,
    render::{
        Extract, RenderApp,
        batching::gpu_preprocessing::{GpuPreprocessingMode, GpuPreprocessingSupport},
        extract_component::ExtractComponentPlugin,
        render_phase::{ViewBinnedRenderPhases, ViewSortedRenderPhases},
        sync_world::RenderEntity,
        view::{NoIndirectDrawing, RetainedViewEntity},
    },
    sprite::Anchor,
    sprite_render::SpriteSystems,
};

use crate::{
    LightmapPhase,
    change::Changes,
    data::{
        CombineLightmapTo, CombinedLightmaps, ExtractedCombineLightmapTo,
        ExtractedCombinedLightmaps, ExtractedWorldData, FireflyConfig,
    },
    lights::{ExtractedPointLight, LightHeight, PointLight2d},
    occluders::ExtractedOccluder,
    phases::SpritePhase,
    prelude::Occluder2d,
    sprites::{
        ExtractedSlices, ExtractedSprite, ExtractedSpriteKind, ExtractedSprites, NormalMap,
        SpriteAssetEvents, SpriteHeight,
    },
    visibility::{NotVisible, OccluderAabb, VisibilityTimer},
};

/// Plugin that handles extracting data from the Main World to the Render World. Automatically
/// added by [`FireflyPlugin`](crate::prelude::FireflyPlugin).
pub struct ExtractPlugin;
impl Plugin for ExtractPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<FireflyConfig>::default());

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app.add_systems(
            ExtractSchedule,
            (
                extract_camera_phases,
                extract_sprites.in_set(SpriteSystems::ExtractSprites),
                extract_sprite_events,
                extract_world_data,
                extract_lights,
                extract_occluders,
            ),
        );
    }
}

fn extract_camera_phases(
    mut sprite_phases: ResMut<ViewSortedRenderPhases<SpritePhase>>,
    mut lightmap_phases: ResMut<ViewBinnedRenderPhases<LightmapPhase>>,
    cameras: Extract<Query<(Entity, &Camera, Has<NoIndirectDrawing>), With<Camera2d>>>,
    mut live_entities: Local<HashSet<RetainedViewEntity>>,
    gpu_preprocessing_support: Res<GpuPreprocessingSupport>,
) {
    live_entities.clear();
    for (main_entity, camera, no_indirect_drawing) in &cameras {
        if !camera.is_active {
            continue;
        }
        // This is the main camera, so we use the first subview index (0)
        let retained_view_entity = RetainedViewEntity::new(main_entity.into(), None, 0);

        // sprite_phases
        match sprite_phases.entry(retained_view_entity) {
            Entry::Occupied(mut entry) => entry.get_mut().clear(),
            Entry::Vacant(entry) => {
                entry.insert(default());
            }
        }

        let gpu_preprocessing_mode = gpu_preprocessing_support.min(if !no_indirect_drawing {
            GpuPreprocessingMode::Culling
        } else {
            GpuPreprocessingMode::PreprocessingOnly
        });

        lightmap_phases.prepare_for_new_frame(retained_view_entity, gpu_preprocessing_mode);

        live_entities.insert(retained_view_entity);
    }

    // Clear out all dead views.
    sprite_phases.retain(|camera_entity, _| live_entities.contains(camera_entity));
    lightmap_phases.retain(|camera_entity, _| live_entities.contains(camera_entity));
}

fn extract_sprite_events(
    mut events: ResMut<SpriteAssetEvents>,
    mut image_events: Extract<MessageReader<AssetEvent<Image>>>,
) {
    let SpriteAssetEvents { ref mut images } = *events;
    images.clear();

    for event in image_events.read() {
        images.push(*event);
    }
}

fn extract_sprites(
    mut extracted_sprites: ResMut<ExtractedSprites>,
    mut extracted_slices: ResMut<ExtractedSlices>,
    texture_atlases: Extract<Res<Assets<TextureAtlasLayout>>>,
    sprite_query: Extract<
        Query<(
            Entity,
            RenderEntity,
            &ViewVisibility,
            &Sprite,
            &Anchor,
            Option<&SpriteHeight>,
            Option<&NormalMap>,
            &GlobalTransform,
            Option<&super::utils::ComputedTextureSlices>,
        )>,
    >,
) {
    extracted_sprites.sprites.clear();
    extracted_slices.slices.clear();
    for (
        main_entity,
        render_entity,
        view_visibility,
        sprite,
        anchor,
        height,
        normal_map,
        transform,
        slices,
    ) in sprite_query.iter()
    {
        if !view_visibility.get() {
            continue;
        }

        let height = height.map_or(0., |h| h.0);

        if let Some(slices) = slices {
            let start = extracted_slices.slices.len();
            extracted_slices
                .slices
                .extend(slices.extract_slices(sprite, anchor));
            let end = extracted_slices.slices.len();
            extracted_sprites.sprites.push(ExtractedSprite {
                main_entity,
                render_entity,

                transform: *transform,
                flip_x: sprite.flip_x,
                flip_y: sprite.flip_y,
                image_handle_id: sprite.image.id(),
                normal_handle_id: normal_map.map(|x| x.handle().id()),
                kind: ExtractedSpriteKind::Slices {
                    indices: start..end,
                },
                height,
            });
        } else {
            let atlas_rect = sprite
                .texture_atlas
                .as_ref()
                .and_then(|s| s.texture_rect(&texture_atlases).map(|r| r.as_rect()));
            let rect = match (atlas_rect, sprite.rect) {
                (None, None) => None,
                (None, Some(sprite_rect)) => Some(sprite_rect),
                (Some(atlas_rect), None) => Some(atlas_rect),
                (Some(atlas_rect), Some(mut sprite_rect)) => {
                    sprite_rect.min += atlas_rect.min;
                    sprite_rect.max += atlas_rect.min;
                    Some(sprite_rect)
                }
            };

            // PERF: we don't check in this function that the `Image` asset is ready, since it should be in most cases and hashing the handle is expensive
            extracted_sprites.sprites.push(ExtractedSprite {
                main_entity,
                render_entity,
                transform: *transform,
                flip_x: sprite.flip_x,
                flip_y: sprite.flip_y,
                image_handle_id: sprite.image.id(),
                normal_handle_id: normal_map.map(|x| x.handle().id()),
                kind: ExtractedSpriteKind::Single {
                    anchor: anchor.as_vec(),
                    rect,
                    scaling_mode: sprite.image_mode.scale(),
                    // Pass the custom size
                    custom_size: sprite.custom_size,
                },
                height,
            });
        }
    }
}

fn extract_world_data(
    mut commands: Commands,
    cameras: Extract<Query<(&RenderEntity, &Camera), With<CombineLightmapTo>>>,
    camera: Extract<
        Query<(
            &RenderEntity,
            &GlobalTransform,
            &FireflyConfig,
            Option<&CombinedLightmaps>,
        )>,
    >,
) {
    for (entity, transform, _, combined_lightmaps) in &camera {
        commands.entity(entity.id()).insert(ExtractedWorldData {
            camera_pos: transform.translation().truncate(),
        });

        if let Some(combined_lightmaps) = combined_lightmaps {
            let mut extracted_collection = vec![];
            for (i, main_entity) in combined_lightmaps.collection().iter().enumerate() {
                let (render_entity, camera) = cameras.get(*main_entity).unwrap();
                if !camera.is_active {
                    continue;
                }

                commands
                    .entity(render_entity.entity())
                    .insert(ExtractedCombineLightmapTo(entity.id(), i as u32));
                extracted_collection.push(**render_entity);
            }

            commands
                .entity(entity.id())
                .insert(ExtractedCombinedLightmaps(extracted_collection));
        }
    }
}

fn extract_lights(
    mut commands: Commands,
    lights: Extract<
        Query<(
            RenderEntity,
            &GlobalTransform,
            &PointLight2d,
            &LightHeight,
            &ViewVisibility,
            &VisibilityTimer,
            &Changes,
            &RenderLayers,
        )>,
    >,
) {
    for (entity, transform, light, height, visibility, visibility_timer, changes, render_layers) in
        &lights
    {
        if !visibility.get() {
            if visibility_timer.0.just_finished() {
                commands.entity(entity).insert(NotVisible);
            }
            continue;
        }

        let pos = transform.translation().truncate() /*+ vec2(0.0, height.0)*/ + light.offset.xy();
        commands.entity(entity).insert(ExtractedPointLight {
            pos,
            color: light.color,
            intensity: light.intensity,
            radius: light.radius,
            z: transform.translation().z + light.offset.z,
            core: light.core,
            falloff: light.falloff,
            angle: light.angle,
            cast_shadows: light.cast_shadows,
            dir: (transform.rotation() * Vec3::Y).xy(),
            height: height.0,
            changes: changes.clone(),
            render_layers: render_layers.clone(),
        });
    }
}

fn extract_occluders(
    mut commands: Commands,
    mut previous_len: Local<usize>,
    occluders: Extract<
        Query<(
            RenderEntity,
            &Occluder2d,
            &GlobalTransform,
            &OccluderAabb,
            &ViewVisibility,
            &VisibilityTimer,
            &Changes,
            &RenderLayers,
        )>,
    >,
) {
    let mut values = Vec::with_capacity(*previous_len);

    for (
        entity,
        occluder,
        global_transform,
        aabb,
        visibility,
        visibility_timer,
        changes,
        render_layers,
    ) in &occluders
    {
        if !visibility.get() {
            if visibility_timer.0.just_finished() {
                commands.entity(entity).insert(NotVisible);
            }
            continue;
        }

        let pos = global_transform.translation().truncate() + occluder.offset.xy();

        let extracted_occluder = ExtractedOccluder {
            pos,
            rot: global_transform.rotation().to_euler(EulerRot::XYZ).2,
            shape: occluder.shape().clone(),
            aabb: aabb.0,
            z: global_transform.translation().z + occluder.offset.z,
            color: occluder.color,
            opacity: occluder.opacity,
            z_sorting: occluder.z_sorting,
            changes: changes.clone(),
            render_layers: render_layers.clone(),
        };

        values.push((entity, extracted_occluder));
    }

    *previous_len = values.len();
    commands.try_insert_batch(values);
}
