//! Tests for shared data types
//!
//! Covers MeshPrimitiveType, SunData, DisabledComponents, SpriteSheetData,
//! serde roundtrips, and default implementations.

use super::*;
use super::core_components::*;
use super::components::{SpriteSheetData, SpriteAnimation};

// =============================================================================
// A. MeshPrimitiveType conversions
// =============================================================================

#[test]
fn mesh_type_id_cube() {
    assert_eq!(MeshPrimitiveType::Cube.type_id(), "mesh.cube");
}

#[test]
fn mesh_type_id_sphere() {
    assert_eq!(MeshPrimitiveType::Sphere.type_id(), "mesh.sphere");
}

#[test]
fn mesh_type_id_cylinder() {
    assert_eq!(MeshPrimitiveType::Cylinder.type_id(), "mesh.cylinder");
}

#[test]
fn mesh_type_id_plane() {
    assert_eq!(MeshPrimitiveType::Plane.type_id(), "mesh.plane");
}

#[test]
fn mesh_from_type_id_roundtrip() {
    let types = [
        MeshPrimitiveType::Cube,
        MeshPrimitiveType::Sphere,
        MeshPrimitiveType::Cylinder,
        MeshPrimitiveType::Plane,
    ];
    for t in &types {
        let id = t.type_id();
        let restored = MeshPrimitiveType::from_type_id(id);
        assert_eq!(restored, Some(*t), "Roundtrip failed for {:?}", t);
    }
}

#[test]
fn mesh_from_type_id_invalid() {
    assert!(MeshPrimitiveType::from_type_id("mesh.invalid").is_none());
    assert!(MeshPrimitiveType::from_type_id("").is_none());
    assert!(MeshPrimitiveType::from_type_id("cube").is_none());
}

// =============================================================================
// B. DisabledComponents
// =============================================================================

#[test]
fn disabled_components_initially_not_disabled() {
    let dc = DisabledComponents::default();
    assert!(!dc.is_disabled("test_component"));
}

#[test]
fn disabled_components_toggle_adds() {
    let mut dc = DisabledComponents::default();
    dc.toggle("test_component");
    assert!(dc.is_disabled("test_component"));
}

#[test]
fn disabled_components_double_toggle_removes() {
    let mut dc = DisabledComponents::default();
    dc.toggle("test_component");
    dc.toggle("test_component");
    assert!(!dc.is_disabled("test_component"));
}

// =============================================================================
// C. SpriteSheetData
// =============================================================================

#[test]
fn sprite_sheet_get_animation_found() {
    let sheet = SpriteSheetData {
        animations: vec![
            SpriteAnimation {
                name: "walk".into(),
                ..Default::default()
            },
            SpriteAnimation {
                name: "idle".into(),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    assert!(sheet.get_animation("walk").is_some());
    assert_eq!(sheet.get_animation("walk").unwrap().name, "walk");
}

#[test]
fn sprite_sheet_get_animation_not_found() {
    let sheet = SpriteSheetData::default();
    assert!(sheet.get_animation("nonexistent").is_none());
}

#[test]
fn sprite_sheet_empty_animations() {
    let sheet = SpriteSheetData {
        animations: Vec::new(),
        ..Default::default()
    };
    assert!(sheet.get_animation("any").is_none());
}

// =============================================================================
// D. Serde roundtrips
// =============================================================================

#[test]
fn procedural_sky_data_serde_roundtrip() {
    let data = ProceduralSkyData::default();
    let json = serde_json::to_string(&data).unwrap();
    let restored: ProceduralSkyData = serde_json::from_str(&json).unwrap();
    assert_eq!(data.sky_curve, restored.sky_curve);
    assert_eq!(data.ground_curve, restored.ground_curve);
}

#[test]
fn clouds_data_serde_roundtrip() {
    let data = CloudsData::default();
    let json = serde_json::to_string(&data).unwrap();
    let restored: CloudsData = serde_json::from_str(&json).unwrap();
    assert_eq!(data.coverage, restored.coverage);
    assert_eq!(data.density, restored.density);
}

#[test]
fn world_environment_data_serde_roundtrip() {
    let data = WorldEnvironmentData::default();
    let json = serde_json::to_string(&data).unwrap();
    let restored: WorldEnvironmentData = serde_json::from_str(&json).unwrap();
    assert_eq!(data.ambient_brightness, restored.ambient_brightness);
}

#[test]
fn health_data_serde_roundtrip() {
    let data = HealthData::default();
    let json = serde_json::to_string(&data).unwrap();
    let restored: HealthData = serde_json::from_str(&json).unwrap();
    assert_eq!(data.max_health, restored.max_health);
    assert_eq!(data.current_health, restored.current_health);
}

#[test]
fn sky_mode_serde_roundtrip() {
    for mode in &[SkyMode::Color, SkyMode::Procedural, SkyMode::Panorama] {
        let json = serde_json::to_string(mode).unwrap();
        let restored: SkyMode = serde_json::from_str(&json).unwrap();
        assert_eq!(*mode, restored);
    }
}

#[test]
fn tonemapping_mode_serde_roundtrip() {
    let modes = [
        TonemappingMode::None,
        TonemappingMode::Reinhard,
        TonemappingMode::ReinhardLuminance,
        TonemappingMode::AcesFitted,
        TonemappingMode::AgX,
        TonemappingMode::TonyMcMapface,
        TonemappingMode::BlenderFilmic,
    ];
    for mode in &modes {
        let json = serde_json::to_string(mode).unwrap();
        let restored: TonemappingMode = serde_json::from_str(&json).unwrap();
        assert_eq!(*mode, restored);
    }
}

#[test]
fn sprite_2d_data_serde_roundtrip() {
    let data = Sprite2DData::default();
    let json = serde_json::to_string(&data).unwrap();
    let restored: Sprite2DData = serde_json::from_str(&json).unwrap();
    assert_eq!(data.texture_path, restored.texture_path);
    assert_eq!(data.flip_x, restored.flip_x);
}

#[test]
fn sprite_animation_serde_roundtrip() {
    let anim = SpriteAnimation {
        name: "attack".into(),
        first_frame: 0,
        last_frame: 5,
        frame_duration: 0.08,
        looping: false,
    };
    let json = serde_json::to_string(&anim).unwrap();
    let restored: SpriteAnimation = serde_json::from_str(&json).unwrap();
    assert_eq!(anim.name, restored.name);
    assert_eq!(anim.last_frame, restored.last_frame);
    assert_eq!(anim.looping, restored.looping);
}

// =============================================================================
// E. Default implementations
// =============================================================================

#[test]
fn procedural_sky_data_default_has_sensible_colors() {
    let data = ProceduralSkyData::default();
    // Sky top should be blue-ish (higher blue component)
    assert!(data.sky_top_color.2 > data.sky_top_color.0, "Sky top should be blue-ish");
}

#[test]
fn health_data_default_has_positive_max() {
    let data = HealthData::default();
    assert!(data.max_health > 0.0);
    assert_eq!(data.current_health, data.max_health);
}

#[test]
fn clouds_data_default_has_reasonable_coverage() {
    let data = CloudsData::default();
    assert!(data.coverage >= 0.0 && data.coverage <= 1.0);
    assert!(data.density >= 0.0 && data.density <= 1.0);
}

// =============================================================================
// F. Additional type tests
// =============================================================================

#[test]
fn editor_entity_default() {
    let e = EditorEntity::default();
    assert!(e.name.is_empty());
    assert!(e.visible);
    assert!(!e.locked);
}

#[test]
fn script_variable_value_default() {
    let v = ScriptVariableValue::default();
    matches!(v, ScriptVariableValue::Float(0.0));
}

#[test]
fn disabled_components_multiple_types() {
    let mut dc = DisabledComponents::default();
    dc.toggle("light");
    dc.toggle("mesh");
    assert!(dc.is_disabled("light"));
    assert!(dc.is_disabled("mesh"));
    assert!(!dc.is_disabled("camera"));
}
