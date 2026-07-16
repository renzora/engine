//! Renzora Grid — the editor's infinite ground grid.
//!
//! Wraps **Bevy 0.19's infinite grid** (`InfiniteGrid`/`InfiniteGridSettings`),
//! vendored from `bevy_dev_tools` into the local [`infinite_grid`] module so we
//! get the built-in grid WITHOUT enabling the whole `bevy_dev_tools` feature
//! (which would otherwise ship its FPS overlay / CI tooling in every build).
//! renzora's [`GridConfig`] drives the colors + line scale, the viewport's
//! show-grid toggle controls visibility, and a Blender-style zoom fade adjusts
//! the fadeout distance from the camera height.

mod infinite_grid;

use bevy::prelude::*;
use infinite_grid::InfiniteGridPlugin;
use renzora::core::viewport_types::ViewportSettings;

// Re-export the grid primitives so other editor crates can drop a grid onto their
// own offscreen preview cameras (e.g. the marketplace 3D model viewer) without
// re-implementing the render feature. The `InfiniteGridPlugin` itself is added
// once by `GridPlugin` below, so consumers only need to spawn the entity.
pub use infinite_grid::{InfiniteGrid, InfiniteGridSettings};

#[derive(Component)]
pub struct EditorGrid;

#[derive(Resource)]
pub struct GridConfig {
    pub visible: bool,
    pub minor_color: Color,
    pub major_color: Color,
    pub x_axis_color: Color,
    pub z_axis_color: Color,
    /// Distance between grid lines (smaller = wider spacing).
    pub scale: f32,
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            visible: true,
            minor_color: Color::srgba(0.55, 0.58, 0.63, 0.55),
            major_color: Color::srgba(0.78, 0.82, 0.88, 0.85),
            x_axis_color: Color::srgb(0.92, 0.32, 0.36),
            z_axis_color: Color::srgb(0.30, 0.58, 0.95),
            scale: 1.0,
        }
    }
}

#[derive(Default)]
pub struct GridPlugin;

impl Plugin for GridPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] GridPlugin (bevy infinite grid)");
        app.add_plugins(InfiniteGridPlugin)
            .init_resource::<GridConfig>()
            .add_systems(PostStartup, spawn_grid)
            .add_systems(
                Update,
                (
                    sync_grid_from_viewport,
                    apply_grid_config,
                    sync_per_camera_grid,
                )
                    .run_if(in_state(renzora::SplashState::Editor)),
            );
    }
}

fn spawn_grid(mut commands: Commands, config: Res<GridConfig>) {
    // `InfiniteGrid` `#[require]`s its settings + Transform/Visibility, so the
    // explicit `InfiniteGridSettings` just overrides the colors/scale.
    commands.spawn((
        InfiniteGrid,
        InfiniteGridSettings {
            x_axis_color: config.x_axis_color,
            z_axis_color: config.z_axis_color,
            minor_line_color: config.minor_color,
            major_line_color: config.major_color,
            fadeout_distance: 100.0,
            dot_fadeout_strength: 0.25,
            scale: config.scale,
        },
        EditorGrid,
        renzora::HideInHierarchy,
    ));
}

fn sync_grid_from_viewport(vp: Res<ViewportSettings>, mut config: ResMut<GridConfig>) {
    if !vp.is_changed() {
        return;
    }
    config.visible = vp.show_grid;
}

/// Push `GridConfig` colors/scale + visibility onto the grid entity when they
/// change. `fadeout_distance` is owned by [`update_fade_distance`] (zoom-driven),
/// so it's left untouched here.
fn apply_grid_config(
    config: Res<GridConfig>,
    mut grid: Query<(&mut Visibility, &mut InfiniteGridSettings), With<EditorGrid>>,
) {
    if !config.is_changed() {
        return;
    }
    for (mut vis, mut settings) in &mut grid {
        *vis = if config.visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        settings.x_axis_color = config.x_axis_color;
        settings.z_axis_color = config.z_axis_color;
        settings.minor_line_color = config.minor_color;
        settings.major_line_color = config.major_color;
        settings.scale = config.scale;
    }
}

/// Give every viewport camera its **own** grid settings, scaled to its **own**
/// height — so each open viewport fades the grid at a radius that matches its own
/// zoom (Blender-style), instead of every view inheriting the focused camera's
/// fade radius. The infinite-grid renderer reads an `InfiniteGridSettings` sitting
/// on a camera as a per-view override (`PerCameraSettingsUniformOffset`); we keep
/// each override's colors/scale in lock-step with `GridConfig` and only its
/// `fadeout_distance` varies per camera.
///
/// The single grid *entity* keeps the shared base settings, so offscreen preview
/// cameras (material/model viewers) that carry no override still get a grid.
fn sync_per_camera_grid(
    mut commands: Commands,
    config: Res<GridConfig>,
    cameras: Query<
        (Entity, &GlobalTransform, Option<&InfiniteGridSettings>),
        With<renzora::core::ViewportCamera>,
    >,
) {
    for (entity, cam_tf, existing) in &cameras {
        let elev = cam_tf.translation().y.abs().max(1.5);
        let fadeout = (elev * 8.0).max(20.0);
        let want = InfiniteGridSettings {
            x_axis_color: config.x_axis_color,
            z_axis_color: config.z_axis_color,
            minor_line_color: config.minor_color,
            major_line_color: config.major_color,
            fadeout_distance: fadeout,
            dot_fadeout_strength: 0.25,
            scale: config.scale,
        };
        // Only re-insert when something actually moved past a threshold, so a
        // steady view doesn't churn the component (and its render-world extract)
        // every frame.
        let changed = match existing {
            None => true,
            Some(cur) => {
                (cur.fadeout_distance - want.fadeout_distance).abs() > 0.5
                    || cur.scale != want.scale
                    || cur.minor_line_color != want.minor_line_color
                    || cur.major_line_color != want.major_line_color
            }
        };
        if changed {
            commands.entity(entity).insert(want);
        }
    }
}

renzora::add!(GridPlugin, Editor);
