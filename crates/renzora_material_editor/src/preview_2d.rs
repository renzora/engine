//! The Material Preview for a Sprite material: a sample sprite wearing the
//! graph being edited, drawn by a 2D camera into the same image the 3D preview
//! uses.
//!
//! A sprite material cannot go on the 3D preview sphere at all. Its shader
//! reads the 2D pipeline's bindings, which a 3D mesh's pipeline does not have,
//! so the sphere would fail to build its pipeline. So the panel switches rigs
//! by domain: [`crate::preview`]'s 3D camera renders while a 3D material is
//! open, this one while a Sprite material is, and never both.
//!
//! The material is built by the same helpers the scene's sprite resolver uses
//! (`sprite_material::sprite_material_from_compiled`), from the graph in
//! memory rather than the file, so the preview follows every edit before a
//! save, and cannot draw a material differently from a sprite in the scene.
//!
//! The sample sprite is generated, not loaded: a soft-edged disc with a
//! gradient, on a checkerboard, so a graph that changes colour, alpha or UVs
//! shows all three, and the panel needs no asset to exist in the project.

use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::RenderTarget;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite_render::SpriteMaterial;
use uuid::Uuid;

use renzora::core::viewport_types::MATERIAL_PREVIEW_2D_LAYER;
use renzora::core::{EditorLocked, HideInHierarchy, IsolatedCamera};
use renzora_shader::material::runtime::FallbackTexture;
use renzora_shader::material::sprite_material::{
    insert_sprite_shader, sprite_material_from_compiled, SpriteGraphExt,
};

use crate::preview::{MaterialPreviewImage, MaterialPreviewOrbit};
use crate::{MaterialEditMode, MaterialEditorState};

/// The sample sprite's size, in the 2D camera's units (one per target pixel at
/// zoom 1). The preview image is 1024 square, so this leaves a margin round it.
const SAMPLE_SIZE: f32 = 640.0;
/// The checkerboard's square, in target pixels.
const CHECKER_CELL: u32 = 32;

#[derive(Component)]
pub(crate) struct SpritePreviewCamera;

#[derive(Component)]
struct SpritePreviewSample;

#[derive(Component)]
struct SpritePreviewChecker;

/// The two checkerboard images, dark and light, swapped by the panel's
/// background toggle.
#[derive(Resource)]
struct CheckerImages {
    dark: Handle<Image>,
    light: Handle<Image>,
}

/// What the sample sprite currently wears: the hash of the compiled graph it
/// was built from, and the shader asset to drop when it is replaced.
#[derive(Resource, Default)]
struct SpritePreviewTracker {
    hash: Option<u64>,
    shader: Option<Uuid>,
}

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<SpritePreviewTracker>().add_systems(
        Update,
        (spawn_rig, sync_camera, update_material, sync_checker).chain(),
    );
}

/// Whether the open material is a Sprite one, which is the whole switch
/// between this rig and the 3D one.
pub(crate) fn editing_sprite(state: &MaterialEditorState) -> bool {
    !state.graph.domain.is_3d()
}

/// Build the rig once the preview image exists. Lazily rather than at startup,
/// so it does not depend on the order the 3D rig's own startup system ran in.
fn spawn_rig(
    mut commands: Commands,
    mut done: Local<bool>,
    preview: Option<Res<MaterialPreviewImage>>,
    mut images: ResMut<Assets<Image>>,
) {
    if *done {
        return;
    }
    let Some(preview) = preview else { return };
    if preview.handle == Handle::default() {
        return;
    }
    *done = true;
    let layer = RenderLayers::layer(MATERIAL_PREVIEW_2D_LAYER);

    commands.spawn((
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.08, 0.08, 0.1)),
            // Same slot as the 3D preview camera; only one is ever active.
            order: -5,
            is_active: false,
            ..default()
        },
        RenderTarget::Image(preview.handle.clone().into()),
        layer.clone(),
        SpritePreviewCamera,
        IsolatedCamera,
        HideInHierarchy,
        EditorLocked,
        Name::new("Sprite Material Preview Camera"),
    ));

    let dark = images.add(checker([0.11, 0.11, 0.13], [0.15, 0.15, 0.18]));
    let light = images.add(checker([0.62, 0.62, 0.66], [0.78, 0.78, 0.82]));
    commands.spawn((
        Sprite {
            image: dark.clone(),
            custom_size: Some(Vec2::splat(4096.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, -1.0),
        layer.clone(),
        SpritePreviewChecker,
        HideInHierarchy,
        EditorLocked,
        Name::new("Sprite Material Preview Checker"),
    ));
    commands.insert_resource(CheckerImages { dark, light });

    commands.spawn((
        Sprite {
            image: images.add(sample_sprite_image()),
            custom_size: Some(Vec2::splat(SAMPLE_SIZE)),
            ..default()
        },
        Transform::default(),
        layer,
        SpritePreviewSample,
        HideInHierarchy,
        EditorLocked,
        Name::new("Sprite Material Preview Sample"),
    ));
}

/// Render only while a Sprite material is open and the panel is showing, the
/// same visibility rule as the 3D camera. The wheel zoom the panel already has
/// drives this camera's scale, so zooming works the same in both rigs.
fn sync_camera(
    state: Res<MaterialEditorState>,
    orbit: Res<MaterialPreviewOrbit>,
    dock: Option<Res<renzora_ember::dock::Dock>>,
    fixed: Option<Res<renzora_ember::dock::FixedDock>>,
    wins: Option<Res<renzora_ember::dock::DockWindows>>,
    mut camera: Query<(&mut Camera, &mut Projection), With<SpritePreviewCamera>>,
) {
    let visible = renzora_ember::dock::panel_visible_anywhere(
        "material_preview",
        dock.as_deref(),
        fixed.as_deref(),
        wins.as_deref(),
    );
    let active = visible && state.compiled_wgsl.is_some() && editing_sprite(&state);
    // The 3D orbit starts at distance 3, which is zoom 1 here.
    let scale = (orbit.distance / 3.0).clamp(0.2, 5.0);
    for (mut cam, mut projection) in &mut camera {
        if cam.is_active != active {
            cam.is_active = active;
        }
        if let Projection::Orthographic(ortho) = &mut *projection {
            if (ortho.scale - scale).abs() > f32::EPSILON {
                ortho.scale = scale;
            }
        }
    }
}

/// Dress the sample sprite in the graph being edited, whenever its compiled
/// form changes. Compile errors leave the last good material in place, as the
/// 3D preview does; the graph shows the errors.
#[allow(clippy::too_many_arguments)]
fn update_material(
    mut commands: Commands,
    state: Res<MaterialEditorState>,
    asset_server: Res<AssetServer>,
    fallback: Res<FallbackTexture>,
    mut shaders: ResMut<Assets<Shader>>,
    mut materials: ResMut<Assets<SpriteGraphExt>>,
    mut tracker: ResMut<SpritePreviewTracker>,
    sample: Query<Entity, With<SpritePreviewSample>>,
) {
    if !state.is_changed() || !editing_sprite(&state) {
        return;
    }
    if matches!(state.edit_mode, MaterialEditMode::Inactive) || !state.compile_errors.is_empty() {
        return;
    }
    let Ok(sample) = sample.single() else { return };
    let result = renzora_shader::material::codegen::compile(&state.graph);
    if !result.errors.is_empty() {
        return;
    }

    // Shader, textures and parameter defaults: a parameter's default lives in
    // the uniform, not the shader text, so it has to be part of the key.
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    result.fragment_shader.hash(&mut hasher);
    for tb in &result.texture_bindings {
        tb.binding.hash(&mut hasher);
        tb.asset_path.hash(&mut hasher);
    }
    for p in &result.parameters {
        p.name.hash(&mut hasher);
        format!("{:?}", p.default).hash(&mut hasher);
    }
    let hash = hasher.finish();
    if tracker.hash == Some(hash) {
        return;
    }
    tracker.hash = Some(hash);

    // Each edit compiles to a new shader at a new uuid; drop the one it
    // replaces, or a long editing session would keep every version alive.
    if let Some(old) = tracker.shader.take() {
        shaders.remove(&Handle::<Shader>::Uuid(old, PhantomData));
    }
    let uuid = insert_sprite_shader(&mut shaders, result.fragment_shader, "graph_material/sprite_preview.wesl".into());
    tracker.shader = Some(uuid);
    let material = sprite_material_from_compiled(
        &fallback.0,
        uuid,
        &result.texture_bindings,
        &result.parameters,
        "material preview",
        &asset_server,
    );
    commands.entity(sample).insert(SpriteMaterial(materials.add(material)));
}

/// The panel's dark/light background toggle swaps the checkerboard.
fn sync_checker(
    orbit: Res<MaterialPreviewOrbit>,
    images: Option<Res<CheckerImages>>,
    mut checker: Query<&mut Sprite, With<SpritePreviewChecker>>,
) {
    let Some(images) = images else { return };
    let want = if orbit.dark_bg { &images.dark } else { &images.light };
    for mut sprite in &mut checker {
        if sprite.image != *want {
            sprite.image = want.clone();
        }
    }
}

/// A two-tone checkerboard, one cell per [`CHECKER_CELL`] pixels once it is
/// stretched over the backdrop sprite. Sampled nearest so the squares stay
/// crisp.
fn checker(a: [f32; 3], b: [f32; 3]) -> Image {
    // 4096 units of backdrop / 32 per cell = 128 cells, one texel each.
    let cells = 4096 / CHECKER_CELL;
    let mut data = Vec::with_capacity((cells * cells * 4) as usize);
    for y in 0..cells {
        for x in 0..cells {
            let c = if (x + y) % 2 == 0 { a } else { b };
            data.extend(c.map(|v| (v * 255.0) as u8));
            data.push(255);
        }
    }
    let mut image = rgba_image(cells, data);
    image.sampler = ImageSampler::nearest();
    image
}

/// The sample sprite: a disc with a top-to-bottom gradient, a highlight and a
/// soft transparent edge, so colour, UV and alpha effects all have something
/// to act on. Also what a Sprite material's Assets-panel thumbnail is captured
/// on, so the thumbnail and the preview agree.
pub(crate) fn sample_sprite_image() -> Image {
    const SIZE: u32 = 256;
    let top = Vec3::new(0.25, 0.80, 0.85);
    let bottom = Vec3::new(0.55, 0.30, 0.90);
    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5) / SIZE as f32 * 2.0 - 1.0;
            let r = p.length();
            let v = y as f32 / (SIZE - 1) as f32;
            let mut color = top.lerp(bottom, v);
            // A soft highlight up and to the left, so the disc reads as a shape
            // rather than a flat stamp.
            let glow = (1.0 - (p - Vec2::new(-0.35, -0.35)).length() / 0.6).max(0.0);
            color = color.lerp(Vec3::ONE, glow * glow * 0.6);
            // A darker rim just inside the edge.
            let rim = ((r - 0.78) / 0.12).clamp(0.0, 1.0);
            color *= 1.0 - rim * 0.35;
            // Two texels of antialiased edge.
            let edge = 2.0 / SIZE as f32 * 2.0;
            let alpha = ((0.9 - r) / edge).clamp(0.0, 1.0);
            data.extend(color.to_array().map(|c| (c.clamp(0.0, 1.0) * 255.0) as u8));
            data.push((alpha * 255.0) as u8);
        }
    }
    rgba_image(SIZE, data)
}

fn rgba_image(size: u32, data: Vec<u8>) -> Image {
    Image::new(
        Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        // Both worlds: a render-world-only image would be dropped from the
        // main world after upload.
        RenderAssetUsages::default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sample_sprite_is_opaque_inside_and_clear_outside() {
        let image = sample_sprite_image();
        let data = image.data.as_ref().unwrap();
        let alpha = |x: u32, y: u32| data[((y * 256 + x) * 4 + 3) as usize];
        assert_eq!(alpha(128, 128), 255, "centre");
        assert_eq!(alpha(0, 0), 0, "corner");
    }
}
