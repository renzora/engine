//! GLTF/GLB passthrough — reads and re-exports (or copies) the file.

use std::path::Path;

use crate::convert::{ImportError, ImportResult};
use crate::settings::ImportSettings;

/// GLB files: read the binary directly (passthrough).
pub fn convert_glb(path: &Path, _settings: &ImportSettings) -> Result<ImportResult, ImportError> {
    let bytes = std::fs::read(path)?;

    // Basic validation: GLB magic number is 0x46546C67 ("glTF")
    if bytes.len() < 12 {
        return Err(ImportError::ParseError("file too small for GLB".into()));
    }
    let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if magic != 0x46546C67 {
        return Err(ImportError::ParseError("invalid GLB magic number".into()));
    }

    Ok(ImportResult {
        glb_bytes: bytes,
        warnings: vec![], extracted_textures: Vec::new(),
    })
}

/// GLTF files: read the JSON and all referenced buffers/images, pack into GLB.
///
/// For now, we embed the JSON GLTF as a GLB by reading all external resources
/// and packing them into a single binary buffer.
pub fn convert_gltf(path: &Path, _settings: &ImportSettings) -> Result<ImportResult, ImportError> {
    let parent = path.parent().unwrap_or(Path::new("."));
    let json_str = std::fs::read_to_string(path)
        .map_err(|e| ImportError::ParseError(format!("failed to read GLTF: {}", e)))?;

    let root: gltf_json::Root = serde_json::from_str(&json_str)
        .map_err(|e| ImportError::ParseError(format!("invalid GLTF JSON: {}", e)))?;

    // Collect all external buffer data
    let mut bin_data = Vec::new();
    let mut warnings = Vec::new();

    for buffer in &root.buffers {
        if let Some(ref uri) = buffer.uri {
            if uri.starts_with("data:") {
                // Data URI — decode base64
                if let Some(base64_start) = uri.find(";base64,") {
                    let encoded = &uri[base64_start + 8..];
                    let decoded = base64_decode(encoded).map_err(|e| {
                        ImportError::ParseError(format!("invalid base64 in buffer URI: {}", e))
                    })?;
                    bin_data.extend_from_slice(&decoded);
                } else {
                    warnings.push(format!("unsupported data URI scheme in buffer"));
                }
            } else {
                // External file reference
                let buf_path = parent.join(uri);
                let data = std::fs::read(&buf_path).map_err(|e| {
                    ImportError::ParseError(format!(
                        "failed to read buffer '{}': {}",
                        buf_path.display(),
                        e
                    ))
                })?;
                bin_data.extend_from_slice(&data);
            }
        }
    }

    // Build GLB from JSON + binary chunk
    let json_bytes = root.to_vec()
        .map_err(|e| ImportError::ConversionError(format!("failed to serialize GLTF JSON: {}", e)))?;

    let glb_bytes = pack_glb(&json_bytes, if bin_data.is_empty() { None } else { Some(&bin_data) });

    Ok(ImportResult {
        glb_bytes,
        warnings, extracted_textures: Vec::new(),
    })
}

/// Simple base64 decoder (no external dep needed).
fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    fn val(c: u8) -> Result<u8, String> {
        match c {
            b'A'..=b'Z' => Ok(c - b'A'),
            b'a'..=b'z' => Ok(c - b'a' + 26),
            b'0'..=b'9' => Ok(c - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err(format!("invalid base64 char: {}", c as char)),
        }
    }

    let input: Vec<u8> = input.bytes().filter(|&b| b != b'\n' && b != b'\r' && b != b' ').collect();
    let mut out = Vec::with_capacity(input.len() * 3 / 4);

    for chunk in input.chunks(4) {
        let len = chunk.iter().filter(|&&b| b != b'=').count();
        if len < 2 {
            break;
        }
        let a = val(chunk[0])?;
        let b = val(chunk[1])?;
        out.push((a << 2) | (b >> 4));
        if len > 2 {
            let c = val(chunk[2])?;
            out.push((b << 4) | (c >> 2));
            if len > 3 {
                let d = val(chunk[3])?;
                out.push((c << 6) | d);
            }
        }
    }

    Ok(out)
}

/// Pack JSON and optional binary data into a GLB container.
pub(crate) fn pack_glb(json: &[u8], bin: Option<&[u8]>) -> Vec<u8> {
    // Pad JSON to 4-byte boundary with spaces
    let json_pad = (4 - (json.len() % 4)) % 4;
    let json_chunk_len = json.len() + json_pad;

    let bin_chunk_len = if let Some(b) = bin {
        let pad = (4 - (b.len() % 4)) % 4;
        b.len() + pad
    } else {
        0
    };

    let total_len = 12 // header
        + 8 + json_chunk_len // JSON chunk header + data
        + if bin.is_some() { 8 + bin_chunk_len } else { 0 }; // BIN chunk

    let mut out = Vec::with_capacity(total_len);

    // GLB header
    out.extend_from_slice(&0x46546C67u32.to_le_bytes()); // magic "glTF"
    out.extend_from_slice(&2u32.to_le_bytes()); // version 2
    out.extend_from_slice(&(total_len as u32).to_le_bytes()); // total length

    // JSON chunk
    out.extend_from_slice(&(json_chunk_len as u32).to_le_bytes());
    out.extend_from_slice(&0x4E4F534Au32.to_le_bytes()); // "JSON"
    out.extend_from_slice(json);
    for _ in 0..json_pad {
        out.push(b' ');
    }

    // BIN chunk
    if let Some(b) = bin {
        let bin_pad = (4 - (b.len() % 4)) % 4;
        out.extend_from_slice(&(bin_chunk_len as u32).to_le_bytes());
        out.extend_from_slice(&0x004E4942u32.to_le_bytes()); // "BIN\0"
        out.extend_from_slice(b);
        for _ in 0..bin_pad {
            out.push(0);
        }
    }

    out
}
