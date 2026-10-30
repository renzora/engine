//! A UI image whose pixels the panel draws itself: the falloff curve and the
//! chunk map.
//!
//! Both are pictures of data rather than controls, and as UI nodes they would
//! be hundreds of entities (a 32 x 32 chunk grid with its ring of empty cells
//! is over a thousand) each with its own reactive binding. One image rewritten
//! in place when its data changes costs one texture upload and no layout.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// A transparent `w` x `h` RGBA image, sampled nearest.
///
/// The chunk map is drawn at a few pixels per cell and stretched to the panel
/// width, so a small grid is magnified ten times or more; the default linear
/// filter smeared every cell into a glow. Nearest keeps the cells crisp, and
/// at 1:1 (the falloff curve) it is the same picture.
pub(super) fn blank_image(w: u32, h: u32) -> Image {
    let mut image = Image::new(
        Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
        TextureDimension::D2,
        vec![0; (w.max(1) * h.max(1) * 4) as usize],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = bevy::image::ImageSampler::nearest();
    image
}

/// Give `node` an image of its own to draw into. The asset is made when the
/// spawn commands apply, since a builder only has `Commands`.
pub(super) fn attach_canvas(commands: &mut Commands, node: Entity, w: u32, h: u32) {
    commands.queue(move |world: &mut World| {
        let handle = world.resource_mut::<Assets<Image>>().add(blank_image(w, h));
        if let Ok(mut e) = world.get_entity_mut(node) {
            e.insert(ImageNode::new(handle));
        }
    });
}

/// Replace the pixels of `node`'s image, resizing it when the picture's size
/// changed.
pub(super) fn write_pixels(images: &mut Assets<Image>, node: &ImageNode, w: u32, h: u32, pixels: Vec<u8>) {
    let Some(mut img) = images.get_mut(&node.image) else { return };
    let size = img.texture_descriptor.size;
    if size.width == w && size.height == h {
        img.data = Some(pixels);
    } else {
        let mut fresh = blank_image(w, h);
        fresh.data = Some(pixels);
        *img = fresh;
    }
}

/// A colour as the image's sRGB bytes.
pub(super) fn rgba8(c: Color, alpha: f32) -> [u8; 4] {
    let s = c.to_srgba();
    [
        (s.red.clamp(0.0, 1.0) * 255.0).round() as u8,
        (s.green.clamp(0.0, 1.0) * 255.0).round() as u8,
        (s.blue.clamp(0.0, 1.0) * 255.0).round() as u8,
        (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

/// A pixel buffer with the two drawing operations the panel needs.
pub(crate) struct Pixels {
    pub w: u32,
    pub h: u32,
    pub data: Vec<u8>,
}

impl Pixels {
    pub fn new(w: u32, h: u32) -> Self {
        Self { w, h, data: vec![0; (w * h * 4) as usize] }
    }

    pub fn put(&mut self, x: i32, y: i32, c: [u8; 4]) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let i = ((y as u32 * self.w + x as u32) * 4) as usize;
        self.data[i..i + 4].copy_from_slice(&c);
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: [u8; 4]) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.put(xx, yy, c);
            }
        }
    }

    /// The outline of a rectangle, one pixel wide.
    pub fn frame(&mut self, x: i32, y: i32, w: i32, h: i32, c: [u8; 4]) {
        for xx in x..x + w {
            self.put(xx, y, c);
            self.put(xx, y + h - 1, c);
        }
        for yy in y..y + h {
            self.put(x, yy, c);
            self.put(x + w - 1, yy, c);
        }
    }
}
