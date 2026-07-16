//! Bundled SOG (`.sog`) decoder — PlayCanvas's "Spatially Ordered Gaussians"
//! compressed splat format, the default download format of SuperSplat.
//!
//! A bundled SOG is a ZIP (stored or deflate) of `meta.json` plus lossless
//! WebP property images: positions as 16-bit fixed point split across two
//! images with a symmetric-log transform, rotations in smallest-three packing,
//! scales / colors / higher-order SH as 8-bit indices into 256-entry codebooks
//! (spec v2; v1 used min/max ranges instead). Decoding follows PlayCanvas's
//! reference implementation (`gsplat-sog-data.js` / `sog-bundle.js`) and
//! expands everything into the same [`PlanarGaussian3d`] the `.ply` path
//! produces, so downstream rendering is format-agnostic.
//!
//! `.ssog` is registered as an alias: no such extension exists in the
//! reference ecosystem (PlayCanvas's *streamed* SOG is an unbundled
//! `lod-meta.json` + chunk-directory tree, not a single file), but a renamed
//! bundle decodes identically, and a genuine streamed-SOG zip gets a clear
//! "not supported" error instead of a codec panic.

use std::collections::HashMap;
use std::io::{Cursor, Read};

use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    reflect::TypePath,
};
use serde::Deserialize;

// `Planar` (for `from_interleaved`) comes via bevy_gaussian_splatting's
// re-export of the bevy_interleave prelude — no direct dep needed.
use bevy_gaussian_splatting::{
    Gaussian3d, Planar, PlanarGaussian3d,
    material::spherical_harmonics::{SH_CHANNELS, SH_COEFF_COUNT},
};

#[derive(Default, TypePath)]
pub struct SogLoader;

impl AssetLoader for SogLoader {
    type Asset = PlanarGaussian3d;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        decode_sog(&bytes).map_err(std::io::Error::other)
    }

    fn extensions(&self) -> &[&str] {
        &["sog", "ssog"]
    }
}

// ── meta.json (tolerant of v1 and v2) ───────────────────────────────────────
//
// v2 quantizes scales / sh0 / shN through 256-entry `codebook`s; v1 stored
// per-channel `mins`/`maxs` ranges instead. `codebook` entries deserialize as
// `Option<f32>` because early SOG writers emitted a literal `null` at index 0
// (PlayCanvas patches it the same way, see `patch_codebook`).

#[derive(Deserialize)]
struct SogMeta {
    count: usize,
    means: MeansMeta,
    scales: ChannelMeta,
    quats: FilesMeta,
    sh0: ChannelMeta,
    #[serde(rename = "shN", default)]
    sh_n: Option<ShNMeta>,
}

#[derive(Deserialize)]
struct MeansMeta {
    mins: [f32; 3],
    maxs: [f32; 3],
    files: Vec<String>,
}

#[derive(Deserialize)]
struct FilesMeta {
    files: Vec<String>,
}

#[derive(Deserialize)]
struct ChannelMeta {
    #[serde(default)]
    codebook: Option<Vec<Option<f32>>>,
    #[serde(default)]
    mins: Option<Vec<f32>>,
    #[serde(default)]
    maxs: Option<Vec<f32>>,
    files: Vec<String>,
}

#[derive(Deserialize)]
struct ShNMeta {
    #[serde(default)]
    codebook: Option<Vec<Option<f32>>>,
    #[serde(default)]
    mins: Option<f32>,
    #[serde(default)]
    maxs: Option<f32>,
    files: Vec<String>,
}

/// A decoded RGBA8 property image.
struct PropertyImage {
    width: usize,
    data: Vec<u8>,
}

impl PropertyImage {
    /// Channel `ch` (0..4) of the pixel holding gaussian `i` (row-major).
    #[inline]
    fn channel(&self, i: usize, ch: usize) -> u8 {
        self.data[i * 4 + ch]
    }
}

/// Early SOG writers emitted `null` as `codebook[0]`; synthesize the value the
/// same way PlayCanvas's `_patchCodebooks` does so those assets stay loadable.
fn patch_codebook(codebook: &[Option<f32>]) -> Vec<f32> {
    let mut out: Vec<f32> = codebook.iter().map(|v| v.unwrap_or(0.0)).collect();
    if codebook.first() == Some(&None) && out.len() == 256 {
        out[0] = out[1] + (out[1] - out[255]) / 255.0;
    }
    out
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

fn decode_sog(bytes: &[u8]) -> Result<PlanarGaussian3d, String> {
    // ── unzip ────────────────────────────────────────────────────────────
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| format!("not a SOG bundle (zip): {e}"))?;
    let mut files: HashMap<String, Vec<u8>> = HashMap::new();
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|e| format!("corrupt SOG bundle: {e}"))?;
        if !file.is_file() {
            continue;
        }
        let mut data = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut data)
            .map_err(|e| format!("corrupt SOG bundle entry: {e}"))?;
        files.insert(file.name().to_string(), data);
    }

    let meta_bytes = files.get("meta.json").ok_or_else(|| {
        if files.contains_key("lod-meta.json") {
            "this is a STREAMED SOG bundle (lod-meta.json); only plain bundled \
             .sog is supported — re-export without LOD chunking (e.g. \
             `splat-transform scene.ply scene.sog`)"
            .to_string()
        } else {
            "meta.json not found in SOG bundle".to_string()
        }
    })?;
    let meta: SogMeta = serde_json::from_slice(meta_bytes)
        .map_err(|e| format!("invalid SOG meta.json: {e}"))?;

    // File resolution follows the reference parser: positional entries in each
    // property's `files` array, matched by exact name in the archive.
    let image = |name: &str| -> Result<PropertyImage, String> {
        let bytes = files
            .get(name)
            .ok_or_else(|| format!("SOG bundle is missing '{name}'"))?;
        let decoded = image::load_from_memory(bytes)
            .map_err(|e| format!("failed to decode '{name}': {e}"))?
            .to_rgba8();
        Ok(PropertyImage {
            width: decoded.width() as usize,
            data: decoded.into_raw(),
        })
    };
    let file_name = |files: &[String], index: usize, what: &str| -> Result<String, String> {
        files
            .get(index)
            .cloned()
            .ok_or_else(|| format!("SOG meta.json lists no {what} file"))
    };

    let means_l = image(&file_name(&meta.means.files, 0, "means (low)")?)?;
    let means_u = image(&file_name(&meta.means.files, 1, "means (high)")?)?;
    let quats = image(&file_name(&meta.quats.files, 0, "quats")?)?;
    let scales = image(&file_name(&meta.scales.files, 0, "scales")?)?;
    let sh0 = image(&file_name(&meta.sh0.files, 0, "sh0")?)?;

    let count = meta.count;
    let capacity = means_l.data.len() / 4;
    if count > capacity {
        return Err(format!(
            "SOG meta.json count ({count}) exceeds image capacity ({capacity})"
        ));
    }

    let scales_codebook = meta.scales.codebook.as_deref().map(patch_codebook);
    let sh0_codebook = meta.sh0.codebook.as_deref().map(patch_codebook);

    // Higher-order SH palette: 64 entries per centroid row; bands are implied
    // by the centroids image width (64 * coeffs-per-channel).
    let sh_n = match &meta.sh_n {
        Some(sh_n_meta) if sh_n_meta.files.len() >= 2 => {
            let centroids = image(&sh_n_meta.files[0])?;
            let labels = image(&sh_n_meta.files[1])?;
            let coeffs = match centroids.width {
                192 => 3,  // 1 band
                512 => 8,  // 2 bands
                960 => 15, // 3 bands
                other => {
                    return Err(format!(
                        "unrecognized SOG shN centroids width {other} (expected 192/512/960)"
                    ));
                }
            };
            let codebook = sh_n_meta.codebook.as_deref().map(patch_codebook);
            Some((centroids, labels, coeffs, codebook))
        }
        _ => None,
    };

    // ── expand into gaussians ────────────────────────────────────────────
    let mut cloud = Vec::with_capacity(count + 32);
    for i in 0..count {
        let mut gaussian = Gaussian3d::default();

        // Positions: 16-bit fixed point across two images, then the inverse
        // of the symmetric log transform sign(n) * (e^|n| - 1).
        for axis in 0..3 {
            let q = ((means_u.channel(i, axis) as u32) << 8) | means_l.channel(i, axis) as u32;
            let n = lerp(
                meta.means.mins[axis],
                meta.means.maxs[axis],
                q as f32 / 65535.0,
            );
            gaussian.position_visibility.position[axis] = n.signum() * (n.abs().exp() - 1.0);
        }

        // Rotations: smallest-three in RGB, alpha 252..=255 says which
        // component was dropped. Component order below matches the reference
        // decoder; Gaussian3d stores [w, x, y, z] (ply rot_0..rot_3).
        {
            let comp = |ch: usize| {
                (quats.channel(i, ch) as f32 / 255.0 - 0.5) * std::f32::consts::SQRT_2
            };
            let (a, b, c) = (comp(0), comp(1), comp(2));
            let d = (1.0 - (a * a + b * b + c * c)).max(0.0).sqrt();
            let (x, y, z, w) = match quats.channel(i, 3).saturating_sub(252) {
                0 => (a, b, c, d),
                1 => (d, b, c, a),
                2 => (b, d, c, a),
                _ => (b, c, d, a),
            };
            gaussian.rotation.rotation = [w, x, y, z];
        }

        // Scales: codebook (v2) or min/max range (v1), both log-domain;
        // Gaussian3d wants linear scale (the .ply path exp()s too).
        for axis in 0..3 {
            let byte = scales.channel(i, axis);
            let log_scale = match (&scales_codebook, &meta.scales.mins, &meta.scales.maxs) {
                (Some(codebook), _, _) => codebook[byte as usize],
                (None, Some(mins), Some(maxs)) => {
                    lerp(mins[axis], maxs[axis], byte as f32 / 255.0)
                }
                _ => return Err("SOG scales have neither codebook nor mins/maxs".into()),
            };
            gaussian.scale_opacity.scale[axis] = log_scale.exp();
        }

        // Base color (SH DC) + opacity. v2 stores linear opacity in alpha;
        // v1 stored a logit that still needs the sigmoid.
        for ch in 0..3 {
            let byte = sh0.channel(i, ch);
            let f_dc = match (&sh0_codebook, &meta.sh0.mins, &meta.sh0.maxs) {
                (Some(codebook), _, _) => codebook[byte as usize],
                (None, Some(mins), Some(maxs)) => lerp(mins[ch], maxs[ch], byte as f32 / 255.0),
                _ => return Err("SOG sh0 has neither codebook nor mins/maxs".into()),
            };
            gaussian.spherical_harmonic.set(ch, f_dc);
        }
        let alpha = sh0.channel(i, 3) as f32 / 255.0;
        gaussian.scale_opacity.opacity = if sh0_codebook.is_some() {
            alpha
        } else {
            let logit = lerp(
                meta.sh0.mins.as_ref().map_or(0.0, |m| m[3]),
                meta.sh0.maxs.as_ref().map_or(0.0, |m| m[3]),
                alpha,
            );
            1.0 / (1.0 + (-logit).exp())
        };

        // Higher-order SH: 16-bit palette label per gaussian, palette pixels
        // hold per-coefficient values (codebook indices in v2). Coefficient
        // `k` of channel `j` lands at interleaved index (k+1)*3 + j — the
        // layout the renderer reads, DC occupying indices 0..3.
        if let Some((centroids, labels, coeffs, codebook)) = &sh_n {
            let n = labels.channel(i, 0) as usize + ((labels.channel(i, 1) as usize) << 8);
            let u = (n % 64) * coeffs;
            let v = n / 64;
            for j in 0..SH_CHANNELS {
                for k in 0..*coeffs {
                    let byte = centroids.data[(u + k) * 4 + j + v * centroids.width * 4];
                    let value = match (codebook, meta.sh_n.as_ref()) {
                        (Some(codebook), _) => codebook[byte as usize],
                        (None, Some(sh_n_meta)) => lerp(
                            sh_n_meta.mins.unwrap_or(0.0),
                            sh_n_meta.maxs.unwrap_or(0.0),
                            byte as f32 / 255.0,
                        ),
                        _ => 0.0,
                    };
                    let interleaved = (k + 1) * SH_CHANNELS + j;
                    if interleaved < SH_COEFF_COUNT {
                        gaussian.spherical_harmonic.set(interleaved, value);
                    }
                }
            }
        }

        cloud.push(gaussian);
    }

    // Pad to a multiple of 32, matching the .ply / .gcloud paths.
    let pad = 32 - (cloud.len() % 32);
    cloud.extend(std::iter::repeat_n(Gaussian3d::default(), pad));

    Ok(PlanarGaussian3d::from_interleaved(cloud))
}
