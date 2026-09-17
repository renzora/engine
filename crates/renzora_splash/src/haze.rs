//! The drifting haze behind the loading screen's terminal.
//!
//! A fullscreen UI material, and the last survivor of a larger effect. The
//! splash used to render a volumetric Light Chamber to an offscreen image and
//! composite it through a film grade (halation, lateral chromatic aberration,
//! vignette, grain) that a UI overlay cannot do because it cannot read what is
//! behind it. All of that existed to give a full-window dashboard a background,
//! and the dashboard is a panel over a live editor now: the editor is the
//! background.
//!
//! The haze stayed because it is not part of that. It belongs to the *loading*
//! screen, which is still a full-window surface, and it is one shader on one
//! node rather than a camera, a render target and a resize pass.

use bevy::asset::Asset;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui::ComputedNode;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial};
use bevy::ui_render::UiMaterialPlugin;

/// Marker for the fullscreen haze node behind the loading terminal.
#[derive(Component)]
pub(crate) struct HazeView;

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct HazeMaterial {
    /// x = time, y = width(px), z = height(px).
    #[uniform(0)]
    params: Vec4,
}

impl UiMaterial for HazeMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_splash/haze.wgsl".into()
    }
}

pub(crate) fn register(app: &mut App) {
    bevy::asset::embedded_asset!(app, "haze.wgsl");
    app.add_plugins(UiMaterialPlugin::<HazeMaterial>::default());
    app.add_systems(Update, (attach_haze, sync_haze));
}

fn attach_haze(
    mut commands: Commands,
    mut materials: ResMut<Assets<HazeMaterial>>,
    views: Query<Entity, (With<HazeView>, Without<MaterialNode<HazeMaterial>>)>,
) {
    for e in &views {
        let handle = materials.add(HazeMaterial { params: Vec4::ZERO });
        commands.entity(e).insert(MaterialNode(handle));
    }
}

fn sync_haze(
    time: Res<Time>,
    mut materials: ResMut<Assets<HazeMaterial>>,
    views: Query<(&ComputedNode, &MaterialNode<HazeMaterial>), With<HazeView>>,
) {
    let t = time.elapsed_secs();
    for (cn, mat) in &views {
        if let Some(mut m) = materials.get_mut(&mat.0) {
            let size = cn.size();
            m.params = Vec4::new(t, size.x, size.y, 0.0);
        }
    }
}
