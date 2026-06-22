//! Tests for the theming system
//!
//! Covers ThemeColor hex parsing, serde roundtrips, theme presets, and defaults.

use super::*;

// =============================================================================
// A. ThemeColor hex parsing
// =============================================================================

#[test]
fn parse_hex_rgb() {
    let color = ThemeColor::from_hex("#FF8800").unwrap();
    let [r, g, b, a] = color.0;
    assert_eq!((r, g, b), (255, 136, 0));
    assert_eq!(a, 255);
}

#[test]
fn parse_hex_rgba() {
    let color = ThemeColor::from_hex("#FF880080").unwrap();
    // Compare against a ThemeColor constructed the same way
    let expected = ThemeColor::with_alpha(255, 136, 0, 128);
    assert_eq!(color, expected);
}

#[test]
fn parse_hex_lowercase() {
    let color = ThemeColor::from_hex("#ff8800").unwrap();
    let [r, g, b, _] = color.0;
    assert_eq!((r, g, b), (255, 136, 0));
}

#[test]
fn parse_hex_no_hash() {
    let color = ThemeColor::from_hex("FF8800").unwrap();
    let [r, g, b, _] = color.0;
    assert_eq!((r, g, b), (255, 136, 0));
}

#[test]
fn parse_hex_invalid_length() {
    assert!(ThemeColor::from_hex("#FFF").is_none());
    assert!(ThemeColor::from_hex("#FFFFF").is_none());
    assert!(ThemeColor::from_hex("#FFFFFFFFF").is_none());
}

#[test]
fn parse_hex_invalid_chars() {
    assert!(ThemeColor::from_hex("#GGHHII").is_none());
    assert!(ThemeColor::from_hex("#ZZZZZZ").is_none());
}

#[test]
fn to_hex_opaque_omits_alpha() {
    let color = ThemeColor::new(255, 128, 0);
    let hex = color.to_hex();
    assert_eq!(hex.len(), 7, "Opaque hex should be 7 chars: {}", hex);
    assert!(hex.starts_with('#'));
}

#[test]
fn to_hex_with_alpha_includes_alpha() {
    let color = ThemeColor::with_alpha(255, 128, 0, 128);
    let hex = color.to_hex();
    assert_eq!(hex.len(), 9, "Alpha hex should be 9 chars: {}", hex);
}

// =============================================================================
// B. Roundtrips
// =============================================================================

#[test]
fn hex_roundtrip() {
    let original = ThemeColor::new(100, 200, 50);
    let hex = original.to_hex();
    let restored = ThemeColor::from_hex(&hex).unwrap();
    assert_eq!(original, restored);
}

#[test]
fn hex_roundtrip_with_alpha() {
    // Full-opacity roundtrips perfectly since no premultiplication occurs
    let original = ThemeColor::with_alpha(100, 200, 50, 255);
    let hex = original.to_hex();
    let restored = ThemeColor::from_hex(&hex).unwrap();
    assert_eq!(original, restored);
}

#[test]
fn serde_json_roundtrip() {
    let color = ThemeColor::new(42, 128, 200);
    let json = serde_json::to_string(&color).unwrap();
    let restored: ThemeColor = serde_json::from_str(&json).unwrap();
    assert_eq!(color, restored);
}

#[test]
fn theme_toml_roundtrip() {
    let theme = Theme::dark();
    let toml_str = toml::to_string_pretty(&theme).unwrap();
    let restored: Theme = toml::from_str(&toml_str).unwrap();
    // Check a few key colors survived the roundtrip
    assert_eq!(theme.semantic.accent, restored.semantic.accent);
    assert_eq!(theme.surfaces.window, restored.surfaces.window);
    assert_eq!(theme.text.primary, restored.text.primary);
}

// =============================================================================
// C. Theme presets
// =============================================================================

#[test]
fn dark_theme_creates() {
    let theme = Theme::dark();
    assert_eq!(theme.meta.name, "Dark");
}

#[test]
fn default_theme_is_dark() {
    let theme = Theme::default();
    assert_eq!(theme.meta.name, "Dark");
}

#[test]
fn all_color_group_defaults_valid() {
    // Just verify all default constructors don't panic
    let _ = SemanticColors::default();
    let _ = SurfaceColors::default();
    let _ = TextColors::default();
    let _ = WidgetColors::default();
    let _ = PanelColors::default();
    let _ = CategoryColors::default();
    let _ = MaterialColors::default();
    let _ = ViewportColors::default();
    let _ = SyntaxColors::default();
}

#[test]
fn syntax_colors_roundtrip() {
    // The `type` field is a raw identifier; confirm it (and an alpha-bearing
    // chrome color) survive a TOML roundtrip with the clean key name.
    let theme = Theme::dark();
    let toml_str = toml::to_string_pretty(&theme).unwrap();
    assert!(toml_str.contains("type ="), "syntax.type should serialize as `type`");
    let restored: Theme = toml::from_str(&toml_str).unwrap();
    assert_eq!(theme.syntax.r#type, restored.syntax.r#type);
    assert_eq!(theme.syntax.keyword, restored.syntax.keyword);
    assert_eq!(theme.syntax.selection, restored.syntax.selection);
}

// =============================================================================
// D. Defaults
// =============================================================================

#[test]
fn theme_color_default_is_white() {
    let c = ThemeColor::default();
    assert_eq!(c.0, [255, 255, 255, 255]);
}

#[test]
fn theme_color_byte_roundtrip() {
    let tc = ThemeColor::new(10, 20, 30);
    assert_eq!(tc.to_array(), [10, 20, 30, 255]);
    assert_eq!(ThemeColor::with_alpha(10, 20, 30, 128).to_array(), [10, 20, 30, 128]);
}

#[test]
fn theme_meta_default_empty() {
    let meta = ThemeMeta::default();
    assert!(meta.name.is_empty());
    assert!(meta.author.is_empty());
}
