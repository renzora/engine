//! Animated splash background as a `UiMaterial` (mirrors ember's gradient/curve
//! widgets). A fullscreen UI node carrying [`BgBackground`] gets a [`BgMaterial`]
//! whose `params` (time + aspect) are updated each frame; the shader in
//! `splash_bg.wgsl` paints the synthwave grid + starfield. Self-contained: it
//! renders in the normal UI pass on the existing default UI camera, so it needs
//! no extra camera/render-target plumbing.

use bevy::asset::Asset;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui::ComputedNode;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial};
use bevy::ui_render::UiMaterialPlugin;

/// Marker for the fullscreen splash-background node (gets a [`BgMaterial`]).
#[derive(Component)]
pub(crate) struct BgBackground;

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct BgMaterial {
    /// x = time (s), y = aspect (w/h).
    #[uniform(0)]
    params: Vec4,
}

impl UiMaterial for BgMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_splash/splash_bg.wgsl".into()
    }
}

pub(crate) fn register(app: &mut App) {
    bevy::asset::embedded_asset!(app, "splash_bg.wgsl");
    app.add_plugins(UiMaterialPlugin::<BgMaterial>::default());
    app.add_systems(Update, (bg_attach, bg_sync));
}

fn bg_attach(
    mut commands: Commands,
    mut materials: ResMut<Assets<BgMaterial>>,
    nodes: Query<Entity, (With<BgBackground>, Without<MaterialNode<BgMaterial>>)>,
) {
    for e in &nodes {
        let handle = materials.add(BgMaterial { params: Vec4::ZERO });
        commands.entity(e).insert(MaterialNode(handle));
    }
}

fn bg_sync(
    time: Res<Time>,
    mut materials: ResMut<Assets<BgMaterial>>,
    nodes: Query<(&ComputedNode, &MaterialNode<BgMaterial>), With<BgBackground>>,
) {
    let t = time.elapsed_secs();
    for (cn, mat) in &nodes {
        if let Some(m) = materials.get_mut(&mat.0) {
            let size = cn.size();
            let aspect = if size.y > 0.0 { size.x / size.y } else { 1.0 };
            m.params = Vec4::new(t, aspect, 0.0, 0.0);
        }
    }
}
