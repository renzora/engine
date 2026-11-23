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
        let repaired = repair_images(glb);
        let glb = match &repaired {
            Some(repaired) => {
                if repaired.blanked > 0 {
                    warn!(
                        "{}: {} embedded texture(s) are in a format the engine cannot read (TIFF, \
                         for one) and show blank. Import the model again to convert them.",
                        load_context.path(),
                        repaired.blanked
                    );
                }
                &repaired.glb
            }
            None => glb,
        };
        Ok(GltfLoader::load_gltf(&self.gltf, glb, load_context, settings).await?)
    }

    fn extensions(&self) -> &[&str] {
        &["rvg"]
    }
}

/// The image types this build of Bevy decodes, as glTF names them.
const DECODABLE: &[&str] = &[
    "image/png",
    "image/jpeg",
    "image/webp",
    "image/gif",
    "image/bmp",
    "image/vnd-ms.dds",
    "image/ktx2",
];

/// 1x1 white, standing in for an unreadable colour (or any non-normal) texture.
const WHITE_PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP4////fwAJ+wP99djxmgAAAABJRU5ErkJggg==";
/// 1x1 (128, 128, 255): a flat tangent-space normal, so a blank normal map
/// leaves the surface's shading alone instead of tilting it.
const FLAT_NORMAL_PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNoaPj/HwAGggL/bOBDIwAAAABJRU5ErkJggg==";

/// A scene GLB with its images made safe for Bevy's glTF loader.
struct Repaired {
    glb: Vec<u8>,
    /// How many images could not be read and were replaced by a placeholder.
    blanked: usize,
}

/// Make every embedded image in `glb` one Bevy's glTF loader can take, or
/// `None` when they all already are.
///
/// Two ways an embedded image breaks that loader, both seen in models built
/// from Japan's PLATEAU city data, whose textures are TIFF:
///
/// - **No `mimeType`.** glTF requires one on an image stored in a buffer view,
///   and the `gltf` crate unwraps it, so the load does not fail, it panics and
///   takes the editor down (`gltf` 1.4, `image.rs:125`). The importer wrote
///   these whenever a texture failed to bake into an `.rmip`, and TIFF always
///   did until the bake learned to decode it.
/// - **A type this build cannot decode.** Bevy then fails the whole model on
///   that one texture (`result?` over the textures), so the tile vanishes.
///
/// Where the bytes are a format Bevy decodes, the missing type is filled in
/// from the file's own signature. Anything else is pointed at a 1x1
/// placeholder, so the model loads with that texture blank and the caller can
/// say so. The fix happens here, on load, rather than only in the importer,
/// because the files already written stay on disk until re-imported.
fn repair_images(glb: &[u8]) -> Option<Repaired> {
    use serde_json::Value;

    let (mut doc, bin) = crate::glb::split(glb).ok()?;
    let normal_maps = normal_map_images(&doc);
    let views = doc.get("bufferViews").and_then(Value::as_array).cloned().unwrap_or_default();
    let images = doc.get_mut("images")?.as_array_mut()?;

    let (mut changed, mut blanked) = (false, 0);
    for (index, image) in images.iter_mut().enumerate() {
        let Some(image) = image.as_object_mut() else { continue };
        let Some(view) = image.get("bufferView").and_then(Value::as_u64) else { continue };
        let declared = image.get("mimeType").and_then(Value::as_str);
        if declared.is_some_and(|m| DECODABLE.iter().any(|d| d.eq_ignore_ascii_case(m))) {
            continue;
        }
        let bytes = views.get(view as usize).and_then(|v| {
            let start = v.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
            let len = v.get("byteLength").and_then(Value::as_u64)? as usize;
            bin.get(start..start.checked_add(len)?)
        });
        changed = true;
        match bytes.and_then(sniff_mime) {
            Some(mime) => {
                image.insert("mimeType".into(), mime.into());
            }
            None => {
                image.remove("bufferView");
                image.remove("mimeType");
                let placeholder = if normal_maps.contains(&index) { FLAT_NORMAL_PNG } else { WHITE_PNG };
                image.insert("uri".into(), placeholder.into());
                blanked += 1;
            }
        }
    }
    changed.then(|| Repaired { glb: crate::glb::join(&doc, bin), blanked })
}

/// The images some material uses as its normal map.
fn normal_map_images(doc: &serde_json::Value) -> std::collections::HashSet<usize> {
    let textures = doc.get("textures").and_then(serde_json::Value::as_array);
    doc.get("materials")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|m| m.get("normalTexture")?.get("index")?.as_u64())
        .filter_map(|t| textures?.get(t as usize)?.get("source")?.as_u64())
        .map(|s| s as usize)
        .collect()
}

/// The glTF image type of `bytes`, from its file signature, when it is one in
/// [`DECODABLE`].
fn sniff_mime(bytes: &[u8]) -> Option<&'static str> {
    const KTX2: [u8; 12] = [0xAB, b'K', b'T', b'X', b' ', b'2', b'0', 0xBB, b'\r', b'\n', 0x1A, b'\n'];
    Some(if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else if bytes.starts_with(b"GIF8") {
        "image/gif"
    } else if bytes.starts_with(b"BM") {
        "image/bmp"
    } else if bytes.starts_with(b"DDS ") {
        "image/vnd-ms.dds"
    } else if bytes.starts_with(&KTX2) {
        "image/ktx2"
    } else {
        return None;
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::gltf::gltf;
    use serde_json::json;

    const TIFF: &[u8] = b"II*\0\x08\0\0\0tiff-body...";
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\npng-body....";

    /// A scene the way the importer wrote PLATEAU tiles: a TIFF normal map and
    /// a PNG colour map in buffer views with no `mimeType`, and a TIFF that
    /// does declare `image/tiff`, which this build cannot decode.
    fn plateau_like() -> Vec<u8> {
        let mut bin = Vec::new();
        let mut views = Vec::new();
        for bytes in [TIFF, PNG, TIFF] {
            views.push(json!({ "buffer": 0, "byteOffset": bin.len(), "byteLength": bytes.len() }));
            bin.extend_from_slice(bytes);
            while bin.len() % 4 != 0 {
                bin.push(0);
            }
        }
        let doc = json!({
            "asset": { "version": "2.0" },
            "buffers": [{ "byteLength": bin.len() }],
            "bufferViews": views,
            "images": [
                { "bufferView": 0 },
                { "bufferView": 1 },
                { "bufferView": 2, "mimeType": "image/tiff" },
            ],
            "textures": [{ "source": 0 }, { "source": 1 }, { "source": 2 }],
            "materials": [{
                "normalTexture": { "index": 0 },
                "pbrMetallicRoughness": { "baseColorTexture": { "index": 1 } },
            }],
        });
        crate::glb::join(&doc, &bin)
    }

    fn sources_of(glb: &[u8]) {
        let doc = gltf::Gltf::from_slice(glb).expect("a valid GLB");
        for image in doc.images() {
            let _ = image.source();
        }
    }

    #[test]
    fn the_unrepaired_scene_panics_where_the_editor_crashed() {
        let glb = plateau_like();
        assert!(std::panic::catch_unwind(|| sources_of(&glb)).is_err());
    }

    #[test]
    fn a_repaired_scene_reads_every_image_without_panicking() {
        let repaired = repair_images(&plateau_like()).expect("the scene needed repair");
        sources_of(&repaired.glb);
        assert_eq!(repaired.blanked, 2, "both TIFFs, not the PNG");

        let (doc, _) = crate::glb::split(&repaired.glb).unwrap();
        let images = doc["images"].as_array().unwrap();
        assert_eq!(images[0]["uri"], FLAT_NORMAL_PNG, "a normal map gets the flat normal");
        assert!(images[0].get("bufferView").is_none());
        assert_eq!(images[1]["mimeType"], "image/png", "a readable image keeps its bytes");
        assert_eq!(images[1]["bufferView"], 1);
        assert_eq!(images[2]["uri"], WHITE_PNG);
    }

    #[test]
    fn a_scene_with_nothing_to_repair_is_left_alone() {
        let doc = json!({
            "asset": { "version": "2.0" },
            "buffers": [{ "byteLength": PNG.len() }],
            "bufferViews": [{ "buffer": 0, "byteLength": PNG.len() }],
            "images": [{ "bufferView": 0, "mimeType": "image/png" }],
        });
        assert!(repair_images(&crate::glb::join(&doc, PNG)).is_none());
        assert!(repair_images(&empty_glb()).is_none());
    }
}
