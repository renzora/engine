//! Splitting a GLB into its JSON and BIN chunks and putting it back together.
//!
//! Shared by the builder, which edits the scene it stores (`scene.rs`), and the
//! model loader, which repairs a stored scene before Bevy's glTF loader reads it
//! (`model.rs`). A module of its own because the two are compiled in different
//! builds: a game loads models but never builds one.

use serde_json::Value;

pub(crate) const JSON_CHUNK: u32 = 0x4E4F_534A;
pub(crate) const BIN_CHUNK: u32 = 0x004E_4942;

pub(crate) fn split(glb: &[u8]) -> Result<(Value, &[u8]), String> {
    let u32_at = |at: usize| -> Option<u32> { glb.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])) };
    if glb.get(..4) != Some(b"glTF") {
        return Err("not a GLB".into());
    }
    let json_len = u32_at(12).ok_or("truncated GLB")? as usize;
    if u32_at(16) != Some(JSON_CHUNK) {
        return Err("GLB does not start with its JSON chunk".into());
    }
    let json = glb.get(20..20 + json_len).ok_or("truncated GLB JSON")?;
    let doc: Value = serde_json::from_slice(json).map_err(|e| format!("GLB JSON: {e}"))?;
    let bin_at = 20 + json_len;
    let bin = match (u32_at(bin_at), u32_at(bin_at + 4)) {
        (Some(len), Some(BIN_CHUNK)) => glb.get(bin_at + 8..bin_at + 8 + len as usize).ok_or("truncated GLB BIN")?,
        _ => &[],
    };
    Ok((doc, bin))
}

pub(crate) fn join(doc: &Value, bin: &[u8]) -> Vec<u8> {
    let mut json = serde_json::to_vec(doc).unwrap_or_default();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let mut bin = bin.to_vec();
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    let total = 12 + 8 + json.len() + if bin.is_empty() { 0 } else { 8 + bin.len() };
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&JSON_CHUNK.to_le_bytes());
    out.extend_from_slice(&json);
    if !bin.is_empty() {
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(&BIN_CHUNK.to_le_bytes());
        out.extend_from_slice(&bin);
    }
    out
}
