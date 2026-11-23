//! Linear twins of plain image files, for textures that hold data.
//!
//! A normal map in a PNG or JPEG has to be read as linear data, but Bevy's
//! `ImageLoader` decodes those files as sRGB colour by default, and asking for
//! other settings when loading does not help: the asset server keys an asset
//! by its path alone, so if anything loaded the file first (the Assets panel's
//! thumbnail, a traced stand-in's normal map), every later load gets that
//! colour copy and its settings are ignored. Decoded as colour, a flat normal's
//! 0.5 became about 0.21, which tilted every normal about 45 degrees: the
//! ground of a landscape faced away from the sun and drew near black under
//! Solari. A `.meta` file is no way out either, since the project asset reader
//! serves none.
//!
//! So a texture marked linear is bound to a twin instead: the same pixels in
//! the same texture, with only its format's sRGB flag dropped, which is exactly
//! the difference between reading the bytes as colour and as data. The twin is
//! an asset of its own under a stable id derived from the path, so every
//! material sampling the file as data shares one, whoever loaded the file.

use std::marker::PhantomData;
use std::sync::Mutex;

use bevy::asset::uuid::Uuid;
use bevy::image::Image;
use bevy::prelude::*;

/// Twins asked for since the last frame: the source, and the handle the twin
/// goes in. A static because the asking happens where only the `AssetServer`
/// is to hand (`codegen::load_texture_binding`).
static REQUESTED: Mutex<Vec<(Handle<Image>, Handle<Image>)>> = Mutex::new(Vec::new());

pub(crate) struct LinearTexturesPlugin;

impl Plugin for LinearTexturesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, fill_linear_twins);
    }
}

/// The twin's id: stable per path, so the same file always shares one twin.
fn twin_id(path: &str) -> Uuid {
    let fnv = |offset: u64| {
        path.bytes().chain(*b"#linear").fold(offset, |h, b| (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3))
    };
    Uuid::from_u64_pair(fnv(0xcbf2_9ce4_8422_2325), fnv(0x84222325_cbf29ce4))
}

/// The handle a material binds for `path` read as data. The twin is filled in
/// once the source has loaded; until then the material waits on it, as it
/// would on any texture still loading.
pub(crate) fn linear_twin(asset_server: &AssetServer, path: &str) -> Handle<Image> {
    let twin: Handle<Image> = Handle::Uuid(twin_id(path), PhantomData);
    let source: Handle<Image> = asset_server.load(path.to_string());
    if let Ok(mut requested) = REQUESTED.lock() {
        requested.push((source, twin.clone()));
    }
    twin
}

/// The same image read as data: identical bytes, the sRGB flag dropped from
/// the texture's format and from its view's.
fn as_linear(source: &Image) -> Image {
    let mut twin = source.clone();
    twin.texture_descriptor.format = twin.texture_descriptor.format.remove_srgb_suffix();
    if let Some(view) = twin.texture_view_descriptor.as_mut() {
        if let Some(format) = view.format.as_mut() {
            *format = format.remove_srgb_suffix();
        }
    }
    twin
}

/// Make each requested twin once its source has loaded. The source handle is
/// held only until then.
fn fill_linear_twins(
    mut waiting: Local<Vec<(Handle<Image>, Handle<Image>)>>,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
) {
    if let Ok(mut requested) = REQUESTED.lock() {
        waiting.append(&mut requested);
    }
    if waiting.is_empty() {
        return;
    }
    waiting.retain(|(source, twin)| {
        if images.contains(twin) {
            return false;
        }
        let Some(image) = images.get(source) else {
            // Still loading, unless it never will.
            return !asset_server.load_state(source).is_failed();
        };
        let linear = if image.data.is_some() {
            as_linear(image)
        } else {
            // Its pixels are on the GPU only, so there is nothing to copy.
            // The colour copy is still better than no texture at all.
            warn!("[materials] a normal map's pixels were not kept on the CPU; it is read as colour");
            image.clone()
        };
        let _ = images.insert(twin, linear);
        false
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::render::render_resource::TextureFormat;

    #[test]
    fn a_twin_reads_the_same_bytes_as_data() {
        let mut source = Image::default();
        source.texture_descriptor.format = TextureFormat::Rgba8UnormSrgb;
        let twin = as_linear(&source);
        assert_eq!(twin.texture_descriptor.format, TextureFormat::Rgba8Unorm);
        assert_eq!(twin.data, source.data);
    }

    #[test]
    fn each_path_has_one_twin_and_paths_do_not_share() {
        assert_eq!(twin_id("textures/a_nor.jpg"), twin_id("textures/a_nor.jpg"));
        assert_ne!(twin_id("textures/a_nor.jpg"), twin_id("textures/b_nor.jpg"));
    }
}
