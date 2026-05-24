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
    let ext = path.extension().and_then(|e| e.to_str())?.to_lowercase();

    match ext.as_str() {
        // FBX is handled by ufbx at load time: `load_scene` sets
        // `target_unit_meters = 1.0`, and ufbx normalizes the file's source
        // units (read from its own `original_unit_meters`) to meters reliably
        // across every FBX version. The previous bespoke probe was both
        // inverted (it returned meters-per-unit, which `load_scene` then used
        // as `target_unit_meters` — preserving cm instead of normalizing) and
        // unreliable (it failed to parse FBX 6100, so old rigs imported as
        // meters while modern Mixamo clip FBX 7700s imported as cm — the same
        // character ending up with a meter skeleton and centimeter clips, which
        // explodes the rig when a clip plays). Returning `None` keeps the scale
        // at its 1.0 default so ufbx does the (correct, consistent) conversion.
        "fbx" => None,
        "usd" | "usda" | "usdc" => detect_usd_units_file(path),
        "usdz" => detect_usdz_units(path),
        "dae" => detect_dae_units(path),
        "blend" => Some(1.0), // Blender uses meters natively
        _ => None,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    // ─── FBX is normalized by ufbx at load (no bespoke probe) ───────────

    #[test]
    fn detect_unit_scale_fbx_defers_to_ufbx() {
        // FBX returns None so the import scale stays at its 1.0 default and
        // `load_scene`'s `target_unit_meters = 1.0` lets ufbx normalize units.
        assert_eq!(detect_unit_scale(std::path::Path::new("a.fbx")), None);
    }

    // ─── USD metersPerUnit text probe ───────────────────────────────────

    #[test]
    fn usd_text_meters_per_unit() {
        let text = "#usda 1.0\n(\n    metersPerUnit = 0.01\n    upAxis = \"Y\"\n)\n";
        assert_eq!(detect_usd_units_text(text), Some(0.01));
    }

    #[test]
    fn usd_text_meters_per_unit_trailing_paren() {
        // Value may have a trailing ')' stripped.
        let text = "metersPerUnit = 1.0)\n";
        assert_eq!(detect_usd_units_text(text), Some(1.0));
    }

    #[test]
    fn usd_text_missing_returns_none() {
        let text = "#usda 1.0\n(\n    upAxis = \"Z\"\n)\n";
        assert_eq!(detect_usd_units_text(text), None);
    }

    // ─── detect_unit_scale dispatch ─────────────────────────────────────

    #[test]
    fn detect_unit_scale_blend_is_meters() {
        assert_eq!(detect_unit_scale(std::path::Path::new("a.blend")), Some(1.0));
    }

    #[test]
    fn detect_unit_scale_unknown_extension_is_none() {
        assert_eq!(detect_unit_scale(std::path::Path::new("a.txt")), None);
        assert_eq!(detect_unit_scale(std::path::Path::new("noext")), None);
    }
}
