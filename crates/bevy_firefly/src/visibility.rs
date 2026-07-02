//! Module with logic for determining whether entities (lights, occluders) are visibile or not.
//! Visibility is based on whether they can affect what is rendered on-screen or not,
//! for instance occluders can be off-screen and still visible because they can block light
//! that would be otherwise visible on-screen.

use std::any::TypeId;

use bevy::{
    camera::visibility::{SetViewVisibility, VisibilitySystems, VisibleEntities},
    math::bounding::{Aabb2d, BoundingVolume, IntersectsVolume},
    prelude::*,
};

use crate::{
    data::FireflyConfig,
    lights::{LightHeight, PointLight2d},
    occluders::{Occluder2dEnabled, Occluder2dShape},
    prelude::Occluder2d,
};

/// Timer that starts ticking down when an entity no longer affects
/// what the player sees. When it finished, the [`NotVisible`] component
/// is added to the corresponding Render World entity.
#[derive(Component)]
pub struct VisibilityTimer(pub Timer);

/// Component added to Render World entities when they are no longer visible
/// in the Main World. Visibility is based on [`VisibilityTimer`].
#[derive(Component, Default)]
pub struct NotVisible;

impl Default for VisibilityTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(0.1, TimerMode::Once))
    }
}

#[derive(Component)]
pub struct OccluderAabb(pub Aabb2d);

impl Default for OccluderAabb {
    fn default() -> Self {
        Self(Aabb2d::new(default(), default()))
    }
}

/// Handles entity visibility. Added automatically through [`FireflyPlugin`](crate::prelude::FireflyPlugin).
pub struct VisibilityPlugin;

impl Plugin for VisibilityPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LightRect>();

        app.add_systems(Update, occluder_aabb);

        app.add_systems(
            PostUpdate,
            (mark_visible_lights, mark_visible_occluders)
                .chain()
                .in_set(VisibilitySystems::CheckVisibility),
        );
    }
}

#[derive(Resource, Default)]
struct LightRect(pub Rect);

fn mark_visible_lights(
    mut lights: Query<(
        Entity,
        &GlobalTransform,
        &PointLight2d,
        &LightHeight,
        &mut ViewVisibility,
        &mut VisibilityTimer,
    )>,
    mut cameras: Query<(&GlobalTransform, &mut VisibleEntities, &Projection), With<FireflyConfig>>,
    mut light_rect: ResMut<LightRect>,
    time: Res<Time>,
) {
    let mut camera_rects = cameras
        .iter_mut()
        .filter_map(|camera| {
            let Projection::Orthographic(projection) = camera.2 else {
                return None;
            };
            Some((
                Aabb2d {
                    min: projection.area.min + camera.0.translation().truncate(),
                    max: projection.area.max + camera.0.translation().truncate(),
                },
                Rect {
                    min: projection.area.min + camera.0.translation().truncate(),
                    max: projection.area.max + camera.0.translation().truncate(),
                },
                camera.1,
            ))
        })
        .collect::<Vec<_>>();

    light_rect.0 = Rect::EMPTY;

    for (entity, transform, light, height, mut visibility, mut visibility_timer) in &mut lights {
        let pos = transform.translation().truncate() - vec2(0.0, height.0) + light.offset.xy();

        let light_aabb = Aabb2d {
            min: pos - light.radius,
            max: pos + light.radius,
        };

        for (camera_aabb, camera_rect, visible_entities) in camera_rects.iter_mut() {
            if light_aabb.intersects(camera_aabb) {
                if !visibility.get() {
                    visibility.set_visible();
                    *visibility_timer = default();
                }

                let visible_lights = visible_entities.get_mut(TypeId::of::<PointLight2d>());
                visible_lights.push(entity);

                light_rect.0 = light_rect
                    .0
                    .union(camera_rect.union_point(pos).intersect(Rect {
                        min: pos - light.radius,
                        max: pos + light.radius,
                    }));
            }
        }

        visibility_timer.0.tick(time.delta());
    }
}

fn mark_visible_occluders(
    mut occluders: Query<(
        &OccluderAabb,
        &Occluder2dEnabled,
        &mut ViewVisibility,
        &mut VisibilityTimer,
    )>,
    light_rect: Res<LightRect>,
    time: Res<Time>,
) {
    let light_rect_aabb = Aabb2d {
        min: light_rect.0.min,
        max: light_rect.0.max,
    };

    for (aabb, enabled, mut visibility, mut visibility_timer) in &mut occluders {
        if enabled.0 && aabb.0.intersects(&light_rect_aabb) && !visibility.get() {
            visibility.set_visible();

            // let visible_occluders = camera.get_mut(TypeId::of::<Occluder2d>());
            // visible_occluders.push(entity);

            *visibility_timer = default();
        }

        visibility_timer.0.tick(time.delta());
    }
}

fn occluder_aabb(
    mut occluders: Query<
        (&Occluder2d, &GlobalTransform, &mut OccluderAabb),
        Or<(Changed<GlobalTransform>, Changed<Occluder2d>)>,
    >,
) {
    for (occluder, transform, mut rect) in &mut occluders {
        let isometry = Isometry2d {
            rotation: Rot2::radians(transform.rotation().to_euler(EulerRot::XYZ).2),
            translation: transform.translation().truncate() + occluder.offset.truncate(),
        };

        rect.0 = match occluder.shape() {
            Occluder2dShape::RoundRectangle {
                half_width,
                half_height,
                radius,
            } => Aabb2d {
                min: vec2(-half_width, -half_height) - radius,
                max: vec2(*half_width, *half_height) + radius,
            }
            .transformed_by(isometry.translation, isometry.rotation),

            Occluder2dShape::Polygon { vertices, .. } => {
                Aabb2d::from_point_cloud(isometry, vertices)
            }
            Occluder2dShape::Polyline { vertices } => Aabb2d::from_point_cloud(isometry, vertices),
        }
    }
}
