//! Source file unit detection.
//!
//! Probes 3D model files for embedded unit metadata and returns a scale
//! factor that converts the file's native units to meters (Bevy's convention).
//!
//! Returns `None` if no unit information is found (assume 1:1 / meters).

use std::path::Path;

/// Probe a file for unit metadata and return meters-per-unit.
///
/// Returns `Some(scale)` where `scale` converts file units to meters:
/// - `0.01` = source is in centimeters
/// - `0.0254` = source is in inches
/// - `1.0` = source is already in meters
/// - `None` = unknown / assume meters
pub fn detect_unit_scale(path: &Path) -> Option<f32> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())?
        .to_lowercase();

    match ext.as_str() {
        "fbx" => detect_fbx_units(path),
        "usd" | "usda" | "usdc" => detect_usd_units_file(path),
        "usdz" => detect_usdz_units(path),
        "dae" => detect_dae_units(path),
        "blend" => Some(1.0), // Blender uses meters natively
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// FBX
// ---------------------------------------------------------------------------

/// Read FBX `UnitScaleFactor` from GlobalSettings.
///
/// UnitScaleFactor is the number of centimeters per file unit.
/// - 1.0 = centimeters → 0.01 meters
/// - 100.0 = meters → 1.0 meters
/// - 2.54 = inches → 0.0254 meters
fn detect_fbx_units(path: &Path) -> Option<f32> {
    let data = std::fs::read(path).ok()?;

    // Try binary FBX (both modern and legacy) via fbx_legacy parser
    // which handles all binary versions
    if data.len() >= 27 && &data[0..20] == b"Kaydara FBX Binary  " {
        return detect_fbx_units_binary(&data);
    }

    // ASCII FBX
    if let Ok(text) = std::str::from_utf8(&data) {
        return detect_fbx_units_ascii(text);
    }

    None
}

fn detect_fbx_units_binary(data: &[u8]) -> Option<f32> {
    let (_, nodes) = crate::fbx_legacy::parse_document(data).ok()?;

    for node in &nodes {
        if node.name != "GlobalSettings" {
            continue;
        }
        for child in &node.children {
            if child.name != "Properties70" && child.name != "Properties60" {
                continue;
            }
            for prop in &child.children {
                if prop.name != "P" && prop.name != "Property" {
                    continue;
                }
                if let Some(crate::fbx_legacy::FbxProp::String(ref name)) = prop.properties.first()
                {
                    if name == "UnitScaleFactor" {
                        for p in prop.properties.iter().skip(4) {
                            match p {
                                crate::fbx_legacy::FbxProp::F64(v) => {
                                    return Some((*v / 100.0) as f32)
                                }
                                crate::fbx_legacy::FbxProp::F32(v) => {
                                    return Some(*v / 100.0)
                                }
                                crate::fbx_legacy::FbxProp::I32(v) => {
                                    return Some(*v as f32 / 100.0)
                                }
                                crate::fbx_legacy::FbxProp::I64(v) => {
                                    return Some(*v as f32 / 100.0)
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

fn detect_fbx_units_ascii(text: &str) -> Option<f32> {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.contains("UnitScaleFactor") && trimmed.contains(',') {
            // Format: P: "UnitScaleFactor", "double", "Number", "",1
            if let Some(val_str) = trimmed.rsplit(',').next() {
                if let Ok(val) = val_str.trim().parse::<f64>() {
                    return Some((val / 100.0) as f32);
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// USD
// ---------------------------------------------------------------------------

fn detect_usd_units_file(path: &Path) -> Option<f32> {
    // Read file and try text probe first (fast), then binary
    let data = std::fs::read(path).ok()?;

    if data.starts_with(b"PXR-USDC") {
        let stage = crate::usd::crate_format::parse(&data).ok()?;
        return Some(stage.meters_per_unit);
    }

    if let Ok(text) = std::str::from_utf8(&data) {
        return detect_usd_units_text(text);
    }

    None
}

fn detect_usdz_units(path: &Path) -> Option<f32> {
    // Parse the full USDZ — renzora_usd handles zip extraction
    let stage = crate::usd::parse(path).ok()?;
    Some(stage.meters_per_unit)
}

fn detect_usd_units_text(text: &str) -> Option<f32> {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.contains("metersPerUnit") && trimmed.contains('=') {
            let val_str = trimmed.split('=').nth(1)?.trim();
            // Remove trailing parens or other chars
            let val_str = val_str.trim_end_matches(')').trim();
            if let Ok(val) = val_str.parse::<f64>() {
                return Some(val as f32);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Collada
// ---------------------------------------------------------------------------

fn detect_dae_units(path: &Path) -> Option<f32> {
    // Read just the first 8KB — <asset> block is always near the top
    let data = std::fs::read(path).ok()?;
    let header = if data.len() > 8192 {
        String::from_utf8_lossy(&data[..8192]).to_string()
    } else {
        String::from_utf8_lossy(&data).to_string()
    };

    // Look for <unit meter="0.01"/> or <unit name="centimeter" meter="0.01"/>
    let unit_pos = header.find("<unit")?;
    let rest = &header[unit_pos..];
    let end = rest.find("/>").or_else(|| rest.find(">"))?;
    let tag = &rest[..end];

    let meter_pos = tag.find("meter=\"")?;
    let val_start = meter_pos + 7;
    let val_rest = &tag[val_start..];
    let val_end = val_rest.find('"')?;
    let val_str = &val_rest[..val_end];

    val_str.parse::<f64>().ok().map(|v| v as f32)
}
