use bevy::{platform::collections::HashSet, prelude::*, sprite::Anchor};

use crate::sprites::ExtractedSlice;

// use crate::sprites::stencil::ExtractedSlice;
/// Component storing texture slices for tiled or sliced sprite entities
///
/// This component is automatically inserted and updated
#[derive(Debug, Clone, Component)]
pub struct ComputedTextureSlices(Vec<TextureSlice>);

impl ComputedTextureSlices {
    /// Computes [`ExtractedSlice`] iterator from the sprite slices
    ///
    /// # Arguments
    ///
    /// * `sprite` - The sprite component
    #[must_use]
    pub(crate) fn extract_slices<'a>(
        &'a self,
        sprite: &'a Sprite,
        anchor: &'a Anchor,
    ) -> impl ExactSizeIterator<Item = ExtractedSlice> + 'a {
        let mut flip = Vec2::ONE;
        if sprite.flip_x {
            flip.x *= -1.0;
        }
        if sprite.flip_y {
            flip.y *= -1.0;
        }
        let anchor = anchor.as_vec()
            * sprite
                .custom_size
                .unwrap_or(sprite.rect.unwrap_or_default().size());
        self.0.iter().map(move |slice| ExtractedSlice {
            offset: slice.offset * flip - anchor,
            rect: slice.texture_rect,
            size: slice.draw_size,
        })
    }
}

/// Generates sprite slices for a [`Sprite`] with [`SpriteImageMode::Sliced`] or [`SpriteImageMode::Sliced`]. The slices
/// will be computed according to the `image_handle` dimensions or the sprite rect.
///
/// Returns `None` if the image asset is not loaded
///
/// # Arguments
///
/// * `sprite` - The sprite component with the image handle and image mode
/// * `images` - The image assets, use to retrieve the image dimensions
/// * `atlas_layouts` - The atlas layout assets, used to retrieve the texture atlas section rect
#[must_use]
fn compute_sprite_slices(
    sprite: &Sprite,
    images: &Assets<Image>,
    atlas_layouts: &Assets<TextureAtlasLayout>,
) -> Option<ComputedTextureSlices> {
    let (image_size, texture_rect) = match &sprite.texture_atlas {
        Some(a) => {
            let layout = atlas_layouts.get(&a.layout)?;
            (
                layout.size.as_vec2(),
                layout.textures.get(a.index)?.as_rect(),
            )
        }
        None => {
            let image = images.get(&sprite.image)?;
            let size = Vec2::new(
                image.texture_descriptor.size.width as f32,
                image.texture_descriptor.size.height as f32,
            );
            let rect = sprite.rect.unwrap_or(Rect {
                min: Vec2::ZERO,
                max: size,
            });
            (size, rect)
        }
    };
    let slices = match &sprite.image_mode {
        SpriteImageMode::Sliced(slicer) => slicer.compute_slices(texture_rect, sprite.custom_size),
        SpriteImageMode::Tiled {
            tile_x,
            tile_y,
            stretch_value,
        } => {
            let slice = TextureSlice {
                texture_rect,
                draw_size: sprite.custom_size.unwrap_or(image_size),
                offset: Vec2::ZERO,
            };
            slice.tiled(*stretch_value, (*tile_x, *tile_y))
        }
        SpriteImageMode::Auto => {
            unreachable!("Slices should not be computed for SpriteImageMode::Stretch")
        }
        SpriteImageMode::Scale(_) => {
            unreachable!("Slices should not be computed for SpriteImageMode::Scale")
        }
    };
    Some(ComputedTextureSlices(slices))
}

/// System reacting to added or modified [`Image`] handles, and recompute sprite slices
/// on sprite entities with a matching  [`SpriteImageMode`]
pub(crate) fn compute_slices_on_asset_event(
    mut commands: Commands,
    mut events: MessageReader<AssetEvent<Image>>,
    images: Res<Assets<Image>>,
    atlas_layouts: Res<Assets<TextureAtlasLayout>>,
    sprites: Query<(Entity, &Sprite)>,
) {
    // We store the asset ids of added/modified image assets
    let added_handles: HashSet<_> = events
        .read()
        .filter_map(|e| match e {
            AssetEvent::Added { id } | AssetEvent::Modified { id } => Some(*id),
            _ => None,
        })
        .collect();
    if added_handles.is_empty() {
        return;
    }
    // We recompute the sprite slices for sprite entities with a matching asset handle id
    for (entity, sprite) in &sprites {
        if !sprite.image_mode.uses_slices() {
            continue;
        }
        if !added_handles.contains(&sprite.image.id()) {
            continue;
        }
        if let Some(slices) = compute_sprite_slices(sprite, &images, &atlas_layouts) {
            commands.entity(entity).insert(slices);
        }
    }
}

/// System reacting to changes on the [`Sprite`] component to compute the sprite slices
pub(crate) fn compute_slices_on_sprite_change(
    mut commands: Commands,
    images: Res<Assets<Image>>,
    atlas_layouts: Res<Assets<TextureAtlasLayout>>,
    changed_sprites: Query<(Entity, &Sprite), Changed<Sprite>>,
) {
    for (entity, sprite) in &changed_sprites {
        if !sprite.image_mode.uses_slices() {
            continue;
        }
        if let Some(slices) = compute_sprite_slices(sprite, &images, &atlas_layouts) {
            commands.entity(entity).insert(slices);
        }
    }
}

/// Scales a texture to fit within a given quad size with keeping the aspect ratio.
pub(crate) fn apply_scaling(
    scaling_mode: SpriteScalingMode,
    texture_size: Vec2,
    quad_size: &mut Vec2,
    quad_translation: &mut Vec2,
    uv_offset_scale: &mut Vec4,
) {
    let quad_ratio = quad_size.x / quad_size.y;
    let texture_ratio = texture_size.x / texture_size.y;
    let tex_quad_scale = texture_ratio / quad_ratio;
    let quad_tex_scale = quad_ratio / texture_ratio;

    match scaling_mode {
        SpriteScalingMode::FillCenter => {
            if quad_ratio > texture_ratio {
                // offset texture to center by y coordinate
                uv_offset_scale.y += (uv_offset_scale.w - uv_offset_scale.w * tex_quad_scale) * 0.5;
                // sum up scales
                uv_offset_scale.w *= tex_quad_scale;
            } else {
                // offset texture to center by x coordinate
                uv_offset_scale.x += (uv_offset_scale.z - uv_offset_scale.z * quad_tex_scale) * 0.5;
                uv_offset_scale.z *= quad_tex_scale;
            };
        }
        SpriteScalingMode::FillStart => {
            if quad_ratio > texture_ratio {
                uv_offset_scale.y += uv_offset_scale.w - uv_offset_scale.w * tex_quad_scale;
                uv_offset_scale.w *= tex_quad_scale;
            } else {
                uv_offset_scale.z *= quad_tex_scale;
            }
        }
        SpriteScalingMode::FillEnd => {
            if quad_ratio > texture_ratio {
                uv_offset_scale.w *= tex_quad_scale;
            } else {
                uv_offset_scale.x += uv_offset_scale.z - uv_offset_scale.z * quad_tex_scale;
                uv_offset_scale.z *= quad_tex_scale;
            }
        }
        SpriteScalingMode::FitCenter => {
            if texture_ratio > quad_ratio {
                // Scale based on width
                quad_size.y *= quad_tex_scale;
            } else {
                // Scale based on height
                quad_size.x *= tex_quad_scale;
            }
        }
        SpriteScalingMode::FitStart => {
            if texture_ratio > quad_ratio {
                // The quad is scaled to match the image ratio, and the quad translation is adjusted
                // to start of the quad within the original quad size.
                let scale = Vec2::new(1.0, quad_tex_scale);
                let new_quad = *quad_size * scale;
                let offset = *quad_size - new_quad;
                *quad_translation = Vec2::new(0.0, -offset.y);
                *quad_size = new_quad;
            } else {
                let scale = Vec2::new(tex_quad_scale, 1.0);
                let new_quad = *quad_size * scale;
                let offset = *quad_size - new_quad;
                *quad_translation = Vec2::new(offset.x, 0.0);
                *quad_size = new_quad;
            }
        }
        SpriteScalingMode::FitEnd => {
            if texture_ratio > quad_ratio {
                let scale = Vec2::new(1.0, quad_tex_scale);
                let new_quad = *quad_size * scale;
                let offset = *quad_size - new_quad;
                *quad_translation = Vec2::new(0.0, offset.y);
                *quad_size = new_quad;
            } else {
                let scale = Vec2::new(tex_quad_scale, 1.0);
                let new_quad = *quad_size * scale;
                let offset = *quad_size - new_quad;
                *quad_translation = Vec2::new(-offset.x, 0.0);
                *quad_size = new_quad;
            }
        }
    }
}
