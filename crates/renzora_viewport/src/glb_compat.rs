//! Make third-party glTF/GLB files load in Bevy when they declare extensions
//! that bevy_gltf doesn't implement.
//!
//! Right now this only strips `KHR_materials_pbrSpecularGlossiness` from the
//! `extensionsRequired` array. The spec for that extension requires the same
//! material to also carry `pbrMetallicRoughness` values as a fallback, so
//! removing the requirement lets Bevy load the model with the metallic-roughness
//! values rather than failing the whole load.
//!
//! The file is rewritten in place. Idempotent — running on a clean file is a
//! no-op.

use std::path::Path;

use serde_json::Value;

/// Extensions we silently drop from `extensionsRequired` because Bevy doesn't
/// implement them but the file has a usable PBR fallback.
const DROPPABLE_REQUIRED: &[&str] = &["KHR_materials_pbrSpecularGlossiness"];

/// Patch the file at `path` so Bevy can load it. Quietly does nothing if the
/// extension isn't `.glb`/`.gltf`, the file can't be parsed, or no patch is
/// needed.
pub fn ensure_loadable(path: &Path) {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase());
    match ext.as_deref() {
        Some("glb") => {
            if let Err(err) = patch_glb(path) {
                bevy::log::debug!("glb compat patch skipped for {:?}: {}", path, err);
            }
        }
        Some("gltf") => {
            if let Err(err) = patch_gltf(path) {
                bevy::log::debug!("gltf compat patch skipped for {:?}: {}", path, err);
            }
        }
        _ => {}
    }
}

fn patch_gltf(path: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut json: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if !strip_unsupported_required(&mut json) {
        return Ok(());
    }
    let new_text = serde_json::to_string(&json).map_err(|e| e.to_string())?;
    std::fs::write(path, new_text).map_err(|e| e.to_string())?;
    bevy::log::info!(
        "Stripped unsupported `extensionsRequired` entries from {:?}",
        path
    );
    Ok(())
}

fn patch_glb(path: &Path) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let (json_bytes, bin_chunk) = split_glb(&bytes)?;
    let mut json: Value = serde_json::from_slice(json_bytes).map_err(|e| e.to_string())?;
    if !strip_unsupported_required(&mut json) {
        return Ok(());
    }
    let new_json = serde_json::to_vec(&json).map_err(|e| e.to_string())?;
    let new_glb = repack_glb(&new_json, bin_chunk);
    std::fs::write(path, new_glb).map_err(|e| e.to_string())?;
    bevy::log::info!(
        "Stripped unsupported `extensionsRequired` entries from {:?}",
        path
    );
    Ok(())
}

/// Returns `true` if the JSON was modified. Removes our droppable extensions
/// from both `extensionsRequired` and `extensionsUsed` (the latter just to
/// keep the two arrays consistent).
fn strip_unsupported_required(root: &mut Value) -> bool {
    let Some(obj) = root.as_object_mut() else {
        return false;
    };

    let mut changed = false;
    if let Some(Value::Array(required)) = obj.get_mut("extensionsRequired") {
        let before = required.len();
        required.retain(|v| match v.as_str() {
            Some(s) => !DROPPABLE_REQUIRED.contains(&s),
            None => true,
        });
        if required.len() != before {
            changed = true;
        }
        if required.is_empty() {
            obj.remove("extensionsRequired");
        }
    }

    changed
}

/// GLB layout: 12-byte header, then one JSON chunk, then optionally a BIN
/// chunk. Returns `(json_chunk_bytes, optional_bin_chunk_bytes_with_padding)`.
fn split_glb(bytes: &[u8]) -> Result<(&[u8], Option<&[u8]>), String> {
    if bytes.len() < 12 {
        return Err("file too small for GLB".into());
    }
    let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if magic != 0x46546C67 {
        return Err("not a GLB file".into());
    }

    // Skip header (12 bytes), then read JSON chunk header (8 bytes).
    let json_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
    let json_kind = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    if json_kind != 0x4E4F534A {
        return Err("first chunk is not JSON".into());
    }
    let json_start = 20;
    let json_end = json_start + json_len;
    if json_end > bytes.len() {
        return Err("JSON chunk overruns file".into());
    }

    let bin = if json_end + 8 <= bytes.len() {
        let bin_len =
            u32::from_le_bytes([bytes[json_end], bytes[json_end + 1], bytes[json_end + 2], bytes[json_end + 3]])
                as usize;
        let bin_kind = u32::from_le_bytes([
            bytes[json_end + 4],
            bytes[json_end + 5],
            bytes[json_end + 6],
            bytes[json_end + 7],
        ]);
        if bin_kind == 0x004E4942 {
            let bin_start = json_end + 8;
            let bin_end = bin_start + bin_len;
            if bin_end > bytes.len() {
                return Err("BIN chunk overruns file".into());
            }
            Some(&bytes[bin_start..bin_end])
        } else {
            None
        }
    } else {
        None
    };

    Ok((&bytes[json_start..json_end], bin))
}

fn repack_glb(json: &[u8], bin: Option<&[u8]>) -> Vec<u8> {
    let json_pad = (4 - (json.len() % 4)) % 4;
    let json_chunk_len = json.len() + json_pad;

    let bin_chunk_len = bin
        .map(|b| {
            let pad = (4 - (b.len() % 4)) % 4;
            b.len() + pad
        })
        .unwrap_or(0);

    let total_len = 12
        + 8
        + json_chunk_len
        + if bin.is_some() { 8 + bin_chunk_len } else { 0 };

    let mut out = Vec::with_capacity(total_len);
    out.extend_from_slice(&0x46546C67u32.to_le_bytes());
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total_len as u32).to_le_bytes());

    out.extend_from_slice(&(json_chunk_len as u32).to_le_bytes());
    out.extend_from_slice(&0x4E4F534Au32.to_le_bytes());
    out.extend_from_slice(json);
    for _ in 0..json_pad {
        out.push(b' ');
    }

    if let Some(b) = bin {
        let bin_pad = (4 - (b.len() % 4)) % 4;
        out.extend_from_slice(&(bin_chunk_len as u32).to_le_bytes());
        out.extend_from_slice(&0x004E4942u32.to_le_bytes());
        out.extend_from_slice(b);
        for _ in 0..bin_pad {
            out.push(0);
        }
    }

    out
}
