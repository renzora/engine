//! Module containing core plugins and logic to be added to a bevy app.

use std::f32::consts::{FRAC_PI_2, PI};

use bevy::{
    color::palettes::css::{GREY, PINK, WHITE},
    core_pipeline::{Core2d, core_2d::main_transparent_pass_2d, tonemapping::tonemapping},
    prelude::*,
    render::RenderApp,
};

use crate::{
    buffers::BuffersPlugin,
    change::ChangePlugin,
    extract::ExtractPlugin,
    lights::LightPlugin,
    nodes::{apply_lightmap, create_lightmap, sprite},
    occluders::{Occluder2dShape, OccluderPlugin, translate_vertices},
    pipelines::PipelinePlugin,
    sprites::SpritesPlugin,
    visibility::VisibilityPlugin,
    *,
};
use crate::{prelude::*, prepare::PreparePlugin};

/// Plugin necessary to use Firefly.
///
/// You will also need to add [`FireflyConfig`] to your camera.
pub struct FireflyPlugin;

impl Plugin for FireflyPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            PipelinePlugin,
            PreparePlugin,
            ExtractPlugin,
            BuffersPlugin,
            VisibilityPlugin,
            ChangePlugin,
        ));
        app.add_plugins((LightPlugin, OccluderPlugin, SpritesPlugin));

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .add_systems(Core2d, sprite.after(main_transparent_pass_2d))
            .add_systems(Core2d, create_lightmap.after(sprite))
            .add_systems(
                Core2d,
                apply_lightmap.after(create_lightmap).before(tonemapping),
            );
    }
}

/// Plugin that shows gizmos for firefly occluders.
///
/// Useful for debugging. Insert the [`FireflyGizmoStyle`] resource to configure.
pub struct FireflyGizmosPlugin;

impl Plugin for FireflyGizmosPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FireflyGizmoStyle>();
        app.add_systems(Update, draw_gizmos);
    }
}

/// Resource that can be manually inserted to change the look of Firefly gizmos.
#[derive(Resource)]
pub struct FireflyGizmoStyle {
    pub light_outer_color: Color,
    pub light_inner_color: Color,
    pub occluder_color: Color,
}

impl Default for FireflyGizmoStyle {
    fn default() -> Self {
        Self {
            light_outer_color: Color::Srgba(GREY),
            light_inner_color: Color::Srgba(WHITE),
            occluder_color: Color::Srgba(PINK),
        }
    }
}

fn draw_gizmos(
    mut gizmos: Gizmos,
    style: Res<FireflyGizmoStyle>,
    occluders: Query<(&GlobalTransform, &Occluder2d)>,
    lights: Query<(&GlobalTransform, &PointLight2d)>,
) {
    for (transform, light) in lights {
        let isometry = Isometry2d::from_translation(transform.translation().xy());

        gizmos.circle_2d(isometry, light.core.radius, style.light_inner_color);
        gizmos.circle_2d(isometry, light.radius, style.light_outer_color);
    }

    for (transform, occluder) in &occluders {
        match occluder.shape().clone() {
            Occluder2dShape::Polygon { vertices, .. } => {
                let vertices = translate_vertices(
                    vertices,
                    transform.translation().truncate() + occluder.offset.xy(),
                    Rot2::radians(transform.rotation().to_euler(EulerRot::XYZ).2),
                );

                for line in vertices.windows(2) {
                    gizmos.line_2d(line[0], line[1], style.occluder_color);
                }
                gizmos.line_2d(
                    vertices[0],
                    vertices[vertices.len() - 1],
                    style.occluder_color,
                );
            }
            Occluder2dShape::Polyline { vertices, .. } => {
                let vertices = translate_vertices(
                    vertices,
                    transform.translation().truncate() + occluder.offset.xy(),
                    Rot2::radians(transform.rotation().to_euler(EulerRot::XYZ).2),
                );

                for line in vertices.windows(2) {
                    gizmos.line_2d(line[0], line[1], style.occluder_color);
                }
            }
            Occluder2dShape::RoundRectangle {
                half_width,
                half_height,
                radius,
            } => {
                let center = transform.translation().truncate() + occluder.offset.xy();

                let rot = Rot2::radians(transform.rotation().to_euler(EulerRot::XYZ).2);
                let rotate =
                    |v: Vec2| vec2(v.x * rot.cos - v.y * rot.sin, v.x * rot.sin + v.y * rot.cos);

                // top line
                gizmos.line_2d(
                    center + rotate(vec2(-half_width, half_height + radius)),
                    center + rotate(vec2(half_width, half_height + radius)),
                    style.occluder_color,
                );

                // right line
                gizmos.line_2d(
                    center + rotate(vec2(half_width + radius, half_height)),
                    center + rotate(vec2(half_width + radius, -half_height)),
                    style.occluder_color,
                );

                // bottom line
                gizmos.line_2d(
                    center + rotate(vec2(-half_width, -half_height - radius)),
                    center + rotate(vec2(half_width, -half_height - radius)),
                    style.occluder_color,
                );

                // left line
                gizmos.line_2d(
                    center + rotate(vec2(-half_width - radius, half_height)),
                    center + rotate(vec2(-half_width - radius, -half_height)),
                    style.occluder_color,
                );

                // top-left arc
                gizmos.arc_2d(
                    Isometry2d {
                        translation: center + rotate(vec2(-half_width, half_height)),
                        rotation: Rot2::radians(transform.rotation().to_euler(EulerRot::XYZ).2),
                    },
                    FRAC_PI_2,
                    radius,
                    style.occluder_color,
                );

                // top-right arc
                gizmos.arc_2d(
                    Isometry2d {
                        translation: center + rotate(vec2(half_width, half_height)),
                        rotation: Rot2::radians(
                            transform.rotation().to_euler(EulerRot::XYZ).2 - FRAC_PI_2,
                        ),
                    },
                    FRAC_PI_2,
                    radius,
                    style.occluder_color,
                );

                // bottom-right arc
                gizmos.arc_2d(
                    Isometry2d {
                        translation: center + rotate(vec2(half_width, -half_height)),
                        rotation: Rot2::radians(
                            transform.rotation().to_euler(EulerRot::XYZ).2 + PI,
                        ),
                    },
                    FRAC_PI_2,
                    radius,
                    style.occluder_color,
                );

                // bottom-left arc
                gizmos.arc_2d(
                    Isometry2d {
                        translation: center + rotate(vec2(-half_width, -half_height)),
                        rotation: Rot2::radians(
                            transform.rotation().to_euler(EulerRot::XYZ).2 + FRAC_PI_2,
                        ),
                    },
                    FRAC_PI_2,
                    radius,
                    style.occluder_color,
                );
            }
        }
    }
}
