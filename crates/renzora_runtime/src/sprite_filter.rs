//! `[rendering_2d] image_filter`, applied live to the sprites already loaded.
//!
//! `renzora_engine`'s sprite rehydration (`scene_io::sprites`) loads each sprite
//! texture with the project's filter baked into its loader settings. That is
//! right for a texture loaded after the setting is known, and it was the only
//! place the setting was read, so changing it while the game ran (from an
//! in-game settings menu, or from Project Settings with a 2D scene open) did
//! nothing to any sprite already on screen. The asset server hands back the
//! existing handle for a path it has loaded, whatever settings the second load
//! asks for, so reloading would not have helped either.
//!
//! So the sampler is changed on the loaded image itself. That re-uploads the
//! texture once, which is what a settings change should cost. It covers both
//! the editor and the game, because the editor viewport shows the game.
//!
//! An image still loading when the filter changes was started with the old
//! settings, so a newly loaded sprite texture is checked as well.

use std::collections::HashSet;

use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use renzora_project_api::{CurrentProject, TextureFilter};
use renzora_scene_api::SpriteImagePath;

pub fn register(app: &mut App) {
    app.add_systems(Update, apply_sprite_filter);
}

/// The sampler a sprite texture gets for `filter`. The same mapping
/// `renzora_engine`'s `load_sprite_image` puts into the loader settings; the
/// two have to agree, or a sprite would change look the first time this runs.
fn sampler_for(filter: TextureFilter) -> ImageSampler {
    ImageSampler::Descriptor(match filter {
        TextureFilter::Nearest => ImageSamplerDescriptor::nearest(),
        TextureFilter::Linear => ImageSamplerDescriptor::linear(),
    })
}

/// Whether every sprite image needs checking: the filter moved since the last
/// pass (or there has been none). Otherwise only the sprite images that just
/// finished loading are checked.
fn filter_moved(applied: Option<TextureFilter>, wanted: TextureFilter) -> bool {
    applied != Some(wanted)
}

fn apply_sprite_filter(
    project: Option<Res<CurrentProject>>,
    mut applied: Local<Option<TextureFilter>>,
    mut events: MessageReader<AssetEvent<Image>>,
    sprites: Query<&Sprite, With<SpriteImagePath>>,
    mut images: ResMut<Assets<Image>>,
) {
    // The same default `scene_io::sprites` uses with no project: nearest, so
    // pixel art stays crisp.
    let wanted = project.map(|p| p.config.rendering_2d.image_filter).unwrap_or_default();
    let all = filter_moved(*applied, wanted);
    let loaded: HashSet<AssetId<Image>> = events
        .read()
        .filter_map(|event| match event {
            AssetEvent::LoadedWithDependencies { id } => Some(*id),
            _ => None,
        })
        .collect();
    *applied = Some(wanted);
    if !all && loaded.is_empty() {
        return;
    }

    let sampler = sampler_for(wanted);
    for sprite in &sprites {
        let id = sprite.image.id();
        if !all && !loaded.contains(&id) {
            continue;
        }
        // Compare before taking the image mutably: `get_mut` marks the asset
        // modified, which re-uploads it, and an image already right must not
        // pay for that (nor fire another round of asset events).
        if images.get(id).is_some_and(|image| image.sampler != sampler) {
            if let Some(mut image) = images.get_mut(id) {
                image.sampler = sampler.clone();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_filter_change_checks_every_sprite() {
        assert!(filter_moved(None, TextureFilter::Nearest));
        assert!(filter_moved(Some(TextureFilter::Nearest), TextureFilter::Linear));
        assert!(!filter_moved(Some(TextureFilter::Linear), TextureFilter::Linear));
    }

    #[test]
    fn samplers_match_the_filter() {
        assert_eq!(sampler_for(TextureFilter::Nearest), ImageSampler::Descriptor(ImageSamplerDescriptor::nearest()));
        assert_eq!(sampler_for(TextureFilter::Linear), ImageSampler::Descriptor(ImageSamplerDescriptor::linear()));
        assert_ne!(sampler_for(TextureFilter::Nearest), sampler_for(TextureFilter::Linear));
    }
}
