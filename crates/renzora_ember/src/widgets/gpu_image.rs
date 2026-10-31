//! UI images kept on the GPU only, and the sizes their nodes still need.
//!
//! Every image cache in the UI (files beside the executable, URLs, marketplace
//! thumbnails, avatars) keeps what it decoded for the whole session, and Bevy's
//! default for a new `Image` is to keep a CPU copy of the pixels beside the GPU
//! texture. Nothing reads those pixels again, so with the launcher's store
//! pages loaded the copies came to well over 100 MB.
//!
//! An image made with `RenderAssetUsages::RENDER_WORLD` alone leaves the
//! main-world `Assets<Image>` once it is on the GPU, and that is the catch: an
//! `ImageNode` with no size of its own (a lightbox, an image in markdown) takes
//! its natural size from that asset, so it would collapse to nothing. Bevy reads
//! `ImageNode::rect` before it looks at the asset, and works out the texture
//! coordinates from the GPU image, so a rect covering the whole image gives the
//! node its size back without changing what is drawn. [`add_gpu_image`] records
//! each image's size and [`size_gpu_images`] fills the rect in.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// The pixel size of every image made by [`add_gpu_image`].
#[derive(Resource, Default)]
pub struct GpuImageSizes(HashMap<AssetId<Image>, UVec2>);

/// Register decoded sRGB RGBA8 pixels as a GPU-only `Image`, remembering its
/// size so an `ImageNode` showing it can still be laid out.
pub fn add_gpu_image(
    images: &mut Assets<Image>,
    sizes: &mut GpuImageSizes,
    rgba: Vec<u8>,
    width: u32,
    height: u32,
) -> Handle<Image> {
    let image = Image::new(
        Extent3d { width, height, depth_or_array_layers: 1 },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    let handle = images.add(image);
    sizes.0.insert(handle.id(), UVec2::new(width, height));
    handle
}

/// Which image this node's rect was filled in for, so the rect is taken back
/// when the node moves on to a different image.
#[derive(Component)]
pub(crate) struct RectFromGpuImage(AssetId<Image>);

/// Give every `ImageNode` showing a GPU-only image a rect covering the whole
/// image. A node with a rect of its own, or an atlas, is left alone.
pub(crate) fn size_gpu_images(
    mut commands: Commands,
    sizes: Res<GpuImageSizes>,
    mut nodes: Query<(Entity, &mut ImageNode, Option<&RectFromGpuImage>), Changed<ImageNode>>,
) {
    for (entity, mut node, filled) in &mut nodes {
        let id = node.image.id();
        let filled_for = filled.map(|f| f.0);
        match sizes.0.get(&id) {
            Some(size) if node.texture_atlas.is_none() => {
                if filled_for == Some(id) && node.rect.is_some() {
                    continue;
                }
                if node.rect.is_some() && filled_for.is_none() {
                    continue;
                }
                node.rect = Some(Rect::from_corners(Vec2::ZERO, size.as_vec2()));
                commands.entity(entity).insert(RectFromGpuImage(id));
            }
            _ => {
                if filled_for.is_some() {
                    node.rect = None;
                    commands.entity(entity).remove::<RectFromGpuImage>();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::uuid::Uuid;
    use bevy::ecs::system::RunSystemOnce;

    fn handle(n: u128) -> Handle<Image> {
        Handle::Uuid(Uuid::from_u128(n), Default::default())
    }

    fn rect_after(world: &mut World, entity: Entity) -> Option<Rect> {
        world.run_system_once(size_gpu_images).unwrap();
        world.get::<ImageNode>(entity).unwrap().rect
    }

    #[test]
    fn a_node_gets_the_size_of_its_gpu_image_and_loses_it_on_a_different_image() {
        let mut world = World::new();
        let (gpu, other) = (handle(1), handle(2));
        let mut sizes = GpuImageSizes::default();
        sizes.0.insert(gpu.id(), UVec2::new(640, 360));
        world.insert_resource(sizes);

        let entity = world.spawn(ImageNode::new(gpu)).id();
        let whole = Some(Rect::from_corners(Vec2::ZERO, Vec2::new(640.0, 360.0)));
        assert_eq!(rect_after(&mut world, entity), whole);

        world.get_mut::<ImageNode>(entity).unwrap().image = other;
        assert_eq!(rect_after(&mut world, entity), None);
        assert!(world.get::<RectFromGpuImage>(entity).is_none());
    }

    #[test]
    fn a_rect_the_caller_set_is_left_alone() {
        let mut world = World::new();
        let gpu = handle(1);
        let mut sizes = GpuImageSizes::default();
        sizes.0.insert(gpu.id(), UVec2::new(640, 360));
        world.insert_resource(sizes);

        let own = Some(Rect::from_corners(Vec2::ZERO, Vec2::splat(32.0)));
        let entity = world.spawn(ImageNode { rect: own, ..ImageNode::new(gpu) }).id();
        assert_eq!(rect_after(&mut world, entity), own);
    }
}
