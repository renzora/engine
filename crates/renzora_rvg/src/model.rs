//! Loading an `.rvg` as a model.
//!
//! An `.rvg` loads as a [`Gltf`], exactly as a `.glb` does: its scene (see
//! `RvgFile::scene`) goes through Bevy's own glTF loader, with the `.rvg`'s own
//! load context, so a model's sub-assets carry the labels any glTF's do
//! (`model.rvg#Scene0`, `#Mesh0/Primitive0`, ...) and everything that places,
//! binds, animates or inspects a model takes it with no idea where it came
//! from. Its virtual geometry is a labeled sub-asset beside them,
//! `model.rvg#Clusters`.
//!
//! An `.rvg` built beside a `.glb` (no scene of its own) loads the same way,
//! as an empty model with its clusters: the renderer asks every `.rvg` for
//! `#Clusters`, whichever kind it is.

use bevy::asset::io::Reader;
use bevy::asset::{AssetApp, AssetLoader, LoadContext};
use bevy::gltf::{DefaultGltfImageSampler, Gltf, GltfError, GltfLoader, GltfLoaderSettings, GltfPlugin};
use bevy::gltf::extensions::GltfExtensionHandlers;
use bevy::image::{CompressedImageFormatSupport, CompressedImageFormats};
use bevy::prelude::*;
use thiserror::Error;

use crate::{RvgFile, RvgLoadError, CLUSTERS_LABEL};

/// Registers the `.rvg` model loader. Add it after Bevy's `GltfPlugin`: the
/// loader is built in `finish`, from the settings that plugin leaves behind.
pub struct RvgModelPlugin;

impl Plugin for RvgModelPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<RvgFile>().preregister_asset_loader::<RvgModelLoader>(&["rvg"]);
    }

    fn finish(&self, app: &mut App) {
        // The same construction `GltfPlugin::finish` uses, so an `.rvg` model
        // loads exactly as its GLB would have: same compressed texture
        // support, sampler, extension handlers and defaults. Renzora runs the
        // glTF plugin with its defaults, which `GltfPlugin::default()` gives.
        let world = app.world();
        let defaults = GltfPlugin::default();
        let supported_compressed_formats = world
            .get_resource::<CompressedImageFormatSupport>()
            .map_or(CompressedImageFormats::NONE, |r| r.0);
        let default_sampler = match world.get_resource::<DefaultGltfImageSampler>() {
            Some(sampler) => sampler.get_internal(),
            None => DefaultGltfImageSampler::new(&defaults.default_sampler).get_internal(),
        };
        let extensions = world
            .get_resource::<GltfExtensionHandlers>()
            .map(|h| h.0.clone())
            .unwrap_or_default();
        app.register_asset_loader(RvgModelLoader {
            gltf: GltfLoader {
                supported_compressed_formats,
                custom_vertex_attributes: defaults.custom_vertex_attributes,
                default_sampler,
                default_convert_coordinates: defaults.convert_coordinates,
                extensions,
                default_skinned_mesh_bounds_policy: defaults.skinned_mesh_bounds_policy,
                default_mesh_compression: defaults.mesh_compression,
            },
        });
    }
}

#[derive(TypePath)]
pub struct RvgModelLoader {
    gltf: GltfLoader,
}

#[derive(Debug, Error)]
pub enum RvgModelError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Rvg(#[from] RvgLoadError),
    #[error("scene: {0}")]
    Gltf(#[from] GltfError),
}

impl AssetLoader for RvgModelLoader {
    type Asset = Gltf;
    type Settings = GltfLoaderSettings;
    type Error = RvgModelError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &GltfLoaderSettings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Gltf, RvgModelError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let mut file = RvgFile::from_bytes(&bytes)?;
        drop(bytes);
        // The scene is handed to the glTF loader and not kept twice.
        let scene = file.scene.take();
        load_context.add_labeled_asset(CLUSTERS_LABEL.to_string(), file);
        let empty;
        let glb: &[u8] = match &scene {
            Some(scene) => scene,
            None => {
                empty = empty_glb();
                &empty
            }
        };
        Ok(GltfLoader::load_gltf(&self.gltf, glb, load_context, settings).await?)
    }

    fn extensions(&self) -> &[&str] {
        &["rvg"]
    }
}

/// A GLB with nothing in it: the scene of an `.rvg` that has none of its own.
fn empty_glb() -> Vec<u8> {
    let json = br#"{"asset":{"version":"2.0"}}   "#;
    let total = 12 + 8 + json.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
    out.extend_from_slice(json);
    out
}
