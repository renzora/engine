//! GLTF/GLB passthrough — reads and re-exports (or copies) the file.

use std::path::Path;

use crate::convert::{ExtractedTexture, ImportError, ImportResult};
use crate::settings::ImportSettings;

/// GLB files: read the binary directly, then extract any embedded images to
/// sit alongside the GLB in `<model_dir>/textures/`. Embedded image entries
/// are rewritten in the GLB's JSON to external URIs so the GLB and the
/// loose texture files agree on the layout.
pub fn convert_glb(path: &Path, settings: &ImportSettings) -> Result<ImportResult, ImportError> {
    let bytes = std::fs::read(path)?;

    if bytes.len() < 12 {
        return Err(ImportError::ParseError("file too small for GLB".into()));
    }
    let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if magic != 0x46546C67 {
        return Err(ImportError::ParseError("invalid GLB magic number".into()));
    }

    if !settings.extract_textures {
        // Passthrough — keep the GLB exactly as-is (embedded textures
        // included). The user can re-enable extraction later and re-import.
        return Ok(ImportResult {
            glb_bytes: crate::glb_compat::strip_unsupported_extensions(&bytes),
            warnings: Vec::new(),
            extracted_textures: Vec::new(), extracted_materials: Vec::new(),
        });
    }

    let (rewritten, extracted_textures, warnings) =
        extract_glb_textures(&bytes).unwrap_or_else(|e| {
            (bytes.clone(), Vec::new(), vec![format!("texture extraction: {}", e)])
        });

    Ok(ImportResult {
        glb_bytes: crate::glb_compat::strip_unsupported_extensions(&rewritten),
        warnings,
        extracted_textures, extracted_materials: Vec::new(),
    })
}

/// Parse a GLB, pull every `bufferView`-backed image out of the BIN chunk,
/// and rewrite those image entries to reference external URIs instead.
/// Returns the (possibly rewritten) GLB bytes, the extracted texture list,
/// and any non-fatal warnings. On fatal parse failure returns an error and
/// the caller falls back to passthrough.
fn extract_glb_textures(
    glb_bytes: &[u8],
) -> Result<(Vec<u8>, Vec<ExtractedTexture>, Vec<String>), String> {
    let glb = gltf::Glb::from_slice(glb_bytes)
        .map_err(|e| format!("parse GLB: {}", e))?;

    let json_slice = glb.json.as_ref();
    let bin_slice: Option<&[u8]> = glb.bin.as_deref();

    let mut root: gltf_json::Root = serde_json::from_slice(json_slice)
        .map_err(|e| format!("parse GLB JSON: {}", e))?;

    if root.images.is_empty() {
        return Ok((glb_bytes.to_vec(), Vec::new(), Vec::new()));
    }

    let mut warnings = Vec::new();
    let mut extracted: Vec<ExtractedTexture> = Vec::new();
    let mut used_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (i, image) in root.images.iter_mut().enumerate() {
        // Skip images that already live as external files; nothing to do.
        if image.uri.is_some() {
            continue;
        }
        let Some(buffer_view_idx) = image.buffer_view.take() else {
            continue;
        };
        let Some(bin) = bin_slice else {
            warnings.push(format!(
                "image {}: bufferView {} but GLB has no BIN chunk",
                i,
                buffer_view_idx.value()
            ));
            continue;
        };

        let view = match root.buffer_views.get(buffer_view_idx.value()) {
            Some(v) => v,
            None => {
                warnings.push(format!(
                    "image {}: bufferView {} out of range",
                    i,
                    buffer_view_idx.value()
                ));
                continue;
            }
        };
        let byte_offset = view
            .byte_offset
            .map(|o| o.0 as usize)
            .unwrap_or(0);
        let byte_length = view.byte_length.0 as usize;
        let end = byte_offset + byte_length;
        if end > bin.len() {
            warnings.push(format!(
                "image {}: bufferView range {}..{} exceeds BIN size {}",
                i,
                byte_offset,
                end,
                bin.len()
            ));
            continue;
        }
        let data = bin[byte_offset..end].to_vec();

        let extension = match image.mime_type.as_ref().map(|m| m.0.as_str()) {
            Some("image/png") => "png",
            Some("image/jpeg") => "jpg",
            Some("image/webp") => "webp",
            _ => sniff_image_extension(&data),
        };
        let mut name = format!("image_{}", i);
        let mut n = 1;
        while used_names.contains(&name) {
            n += 1;
            name = format!("image_{}_{}", i, n);
        }
        used_names.insert(name.clone());

        let uri = format!("textures/{}.{}", name, extension);
        image.uri = Some(uri);
        image.mime_type = None;

        extracted.push(ExtractedTexture {
            name,
            extension: extension.to_string(),
            data,
        });
    }

    if extracted.is_empty() {
        return Ok((glb_bytes.to_vec(), Vec::new(), warnings));
    }

    let new_json = root
        .to_vec()
        .map_err(|e| format!("re-serialize GLB JSON: {}", e))?;
    let new_glb = pack_glb(&new_json, bin_slice);
    Ok((new_glb, extracted, warnings))
}

/// Very small magic-byte sniff — mirrors the ufbx path so GLB and FBX
/// extractors agree on extensions.
fn sniff_image_extension(data: &[u8]) -> &'static str {
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) { "png" }
    else if data.starts_with(&[0xFF, 0xD8, 0xFF]) { "jpg" }
    else if data.starts_with(b"DDS ") { "dds" }
    else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") { "gif" }
    else if data.starts_with(b"BM") { "bmp" }
    else if data.starts_with(&[0x52, 0x49, 0x46, 0x46]) && data.get(8..12) == Some(b"WEBP") { "webp" }
    else { "bin" }
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
        glb_bytes: crate::glb_compat::strip_unsupported_extensions(&glb_bytes),
        warnings, extracted_textures: Vec::new(), extracted_materials: Vec::new(),
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
