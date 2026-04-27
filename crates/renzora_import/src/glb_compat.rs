//! Strip `extensionsRequired` entries that downstream parsers don't understand
//! but which have a usable PBR fallback in the same file.
//!
//! The `gltf` crate (used by both `optimize_glb` and Bevy's loader) refuses to
//! parse a document that lists an unknown extension as required. Several
//! third-party assets ship with `KHR_materials_pbrSpecularGlossiness` flagged
//! required even though they also carry `pbrMetallicRoughness` values — the
//! spec mandates that fallback. Removing the requirement lets the parser
//! consume the metallic-roughness path and load the file.
//!
//! This is the in-memory counterpart to `renzora_viewport::glb_compat`: the
//! viewport version patches files on disk; this one cleans the bytes during
//! the import pipeline so the file *written* to the project is already clean.

use serde_json::Value;

/// Extensions we silently drop from `extensionsRequired` because no consumer
/// in this engine implements them but the file has a usable PBR fallback.
const DROPPABLE_REQUIRED: &[&str] = &["KHR_materials_pbrSpecularGlossiness"];

/// Return GLB bytes with unsupported `extensionsRequired` entries removed. If
/// `bytes` is not a GLB or no patch is needed, returns it unchanged.
pub fn strip_unsupported_extensions(bytes: &[u8]) -> Vec<u8> {
    let Ok((json_bytes, bin_chunk)) = split_glb(bytes) else {
        return bytes.to_vec();
    };
    let Ok(mut json) = serde_json::from_slice::<Value>(json_bytes) else {
        return bytes.to_vec();
    };
    if !strip_unsupported_required(&mut json) {
        return bytes.to_vec();
    }
    let Ok(new_json) = serde_json::to_vec(&json) else {
        return bytes.to_vec();
    };
    repack_glb(&new_json, bin_chunk)
}

/// Returns `true` if the JSON was modified.
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

fn split_glb(bytes: &[u8]) -> Result<(&[u8], Option<&[u8]>), ()> {
    if bytes.len() < 12 {
        return Err(());
    }
    let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if magic != 0x46546C67 {
        return Err(());
    }
    let json_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
    let json_kind = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    if json_kind != 0x4E4F534A {
        return Err(());
    }
    let json_start = 20;
    let json_end = json_start + json_len;
    if json_end > bytes.len() {
        return Err(());
    }
    let bin = if json_end + 8 <= bytes.len() {
        let bin_len = u32::from_le_bytes([
            bytes[json_end],
            bytes[json_end + 1],
            bytes[json_end + 2],
            bytes[json_end + 3],
        ]) as usize;
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
                return Err(());
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
