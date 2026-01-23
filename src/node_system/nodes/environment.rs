use bevy::prelude::*;
use std::collections::HashMap;

use crate::core::{AudioListenerMarker, EditorEntity, SceneNode, WorldEnvironmentMarker};
use crate::node_system::components::NodeTypeMarker;
use crate::node_system::definition::{NodeCategory, NodeDefinition};
use crate::scene_file::WorldEnvironmentData;

/// World environment node
pub static WORLD_ENVIRONMENT: NodeDefinition = NodeDefinition {
    type_id: "environment.world",
    display_name: "World Environment",
    category: NodeCategory::Environment,
    default_name: "World Environment",
    spawn_fn: spawn_world_environment,
    serialize_fn: Some(serialize_world_environment),
    deserialize_fn: Some(deserialize_world_environment),
    priority: 0,
};

/// Audio listener node
pub static AUDIO_LISTENER: NodeDefinition = NodeDefinition {
    type_id: "environment.audio_listener",
    display_name: "Audio Listener",
    category: NodeCategory::Environment,
    default_name: "Audio Listener",
    spawn_fn: spawn_audio_listener,
    serialize_fn: None,
    deserialize_fn: Some(deserialize_audio_listener),
    priority: 1,
};

// --- World Environment ---

fn spawn_world_environment(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: WORLD_ENVIRONMENT.default_name.to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        NodeTypeMarker::new(WORLD_ENVIRONMENT.type_id),
        WorldEnvironmentMarker {
            data: WorldEnvironmentData::default(),
        },
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

fn serialize_world_environment(entity: Entity, world: &World) -> Option<HashMap<String, serde_json::Value>> {
    let env_marker = world.get::<WorldEnvironmentMarker>(entity)?;
    let data = &env_marker.data;

    let mut map = HashMap::new();
    // Ambient light
    map.insert("ambient_color".to_string(), serde_json::json!([data.ambient_color.0, data.ambient_color.1, data.ambient_color.2]));
    map.insert("ambient_brightness".to_string(), serde_json::json!(data.ambient_brightness));

    // Sky mode
    map.insert("sky_mode".to_string(), serde_json::json!(format!("{:?}", data.sky_mode)));
    map.insert("clear_color".to_string(), serde_json::json!([data.clear_color.0, data.clear_color.1, data.clear_color.2]));

    // Procedural sky
    let sky = &data.procedural_sky;
    map.insert("procedural_sky".to_string(), serde_json::json!({
        "sky_top_color": [sky.sky_top_color.0, sky.sky_top_color.1, sky.sky_top_color.2],
        "sky_horizon_color": [sky.sky_horizon_color.0, sky.sky_horizon_color.1, sky.sky_horizon_color.2],
        "ground_bottom_color": [sky.ground_bottom_color.0, sky.ground_bottom_color.1, sky.ground_bottom_color.2],
        "ground_horizon_color": [sky.ground_horizon_color.0, sky.ground_horizon_color.1, sky.ground_horizon_color.2],
        "sun_angle_azimuth": sky.sun_angle_azimuth,
        "sun_angle_elevation": sky.sun_angle_elevation,
        "sun_disk_scale": sky.sun_disk_scale,
        "sun_color": [sky.sun_color.0, sky.sun_color.1, sky.sun_color.2],
        "sun_energy": sky.sun_energy,
        "sky_curve": sky.sky_curve,
        "ground_curve": sky.ground_curve,
    }));

    // Panorama sky
    let pano = &data.panorama_sky;
    map.insert("panorama_sky".to_string(), serde_json::json!({
        "panorama_path": pano.panorama_path,
        "rotation": pano.rotation,
        "energy": pano.energy,
    }));
    // Fog
    map.insert("fog_enabled".to_string(), serde_json::json!(data.fog_enabled));
    map.insert("fog_color".to_string(), serde_json::json!([data.fog_color.0, data.fog_color.1, data.fog_color.2]));
    map.insert("fog_start".to_string(), serde_json::json!(data.fog_start));
    map.insert("fog_end".to_string(), serde_json::json!(data.fog_end));
    // Anti-aliasing
    map.insert("msaa_samples".to_string(), serde_json::json!(data.msaa_samples));
    map.insert("fxaa_enabled".to_string(), serde_json::json!(data.fxaa_enabled));
    // SSAO
    map.insert("ssao_enabled".to_string(), serde_json::json!(data.ssao_enabled));
    map.insert("ssao_intensity".to_string(), serde_json::json!(data.ssao_intensity));
    map.insert("ssao_radius".to_string(), serde_json::json!(data.ssao_radius));
    // SSR
    map.insert("ssr_enabled".to_string(), serde_json::json!(data.ssr_enabled));
    map.insert("ssr_intensity".to_string(), serde_json::json!(data.ssr_intensity));
    map.insert("ssr_max_steps".to_string(), serde_json::json!(data.ssr_max_steps));
    // Bloom
    map.insert("bloom_enabled".to_string(), serde_json::json!(data.bloom_enabled));
    map.insert("bloom_intensity".to_string(), serde_json::json!(data.bloom_intensity));
    map.insert("bloom_threshold".to_string(), serde_json::json!(data.bloom_threshold));
    // Tonemapping
    map.insert("tonemapping".to_string(), serde_json::json!(format!("{:?}", data.tonemapping)));
    map.insert("exposure".to_string(), serde_json::json!(data.exposure));
    // Depth of Field
    map.insert("dof_enabled".to_string(), serde_json::json!(data.dof_enabled));
    map.insert("dof_focal_distance".to_string(), serde_json::json!(data.dof_focal_distance));
    map.insert("dof_aperture".to_string(), serde_json::json!(data.dof_aperture));
    // Motion Blur
    map.insert("motion_blur_enabled".to_string(), serde_json::json!(data.motion_blur_enabled));
    map.insert("motion_blur_intensity".to_string(), serde_json::json!(data.motion_blur_intensity));

    Some(map)
}

fn deserialize_world_environment(
    entity_commands: &mut EntityCommands,
    data: &HashMap<String, serde_json::Value>,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    use crate::scene_file::{PanoramaSkyData, ProceduralSkyData, SkyMode, TonemappingMode};

    // Helper to parse color arrays
    let parse_color = |key: &str, default: (f32, f32, f32)| -> (f32, f32, f32) {
        data.get(key)
            .and_then(|v| v.as_array())
            .map(|arr| {
                (
                    arr.first().and_then(|v| v.as_f64()).unwrap_or(default.0 as f64) as f32,
                    arr.get(1).and_then(|v| v.as_f64()).unwrap_or(default.1 as f64) as f32,
                    arr.get(2).and_then(|v| v.as_f64()).unwrap_or(default.2 as f64) as f32,
                )
            })
            .unwrap_or(default)
    };

    let ambient_color = parse_color("ambient_color", (1.0, 1.0, 1.0));
    let ambient_brightness = data.get("ambient_brightness").and_then(|v| v.as_f64()).unwrap_or(300.0) as f32;
    let clear_color = parse_color("clear_color", (0.4, 0.6, 0.9));

    // Sky mode
    let sky_mode = data.get("sky_mode")
        .and_then(|v| v.as_str())
        .map(|s| match s {
            "Procedural" => SkyMode::Procedural,
            "Panorama" => SkyMode::Panorama,
            _ => SkyMode::Color,
        })
        .unwrap_or(SkyMode::Color);

    // Procedural sky
    let procedural_sky = if let Some(sky_data) = data.get("procedural_sky").and_then(|v| v.as_object()) {
        let parse_sky_color = |key: &str, default: (f32, f32, f32)| -> (f32, f32, f32) {
            sky_data.get(key)
                .and_then(|v| v.as_array())
                .map(|arr| {
                    (
                        arr.first().and_then(|v| v.as_f64()).unwrap_or(default.0 as f64) as f32,
                        arr.get(1).and_then(|v| v.as_f64()).unwrap_or(default.1 as f64) as f32,
                        arr.get(2).and_then(|v| v.as_f64()).unwrap_or(default.2 as f64) as f32,
                    )
                })
                .unwrap_or(default)
        };

        ProceduralSkyData {
            sky_top_color: parse_sky_color("sky_top_color", (0.15, 0.35, 0.65)),
            sky_horizon_color: parse_sky_color("sky_horizon_color", (0.55, 0.70, 0.85)),
            ground_bottom_color: parse_sky_color("ground_bottom_color", (0.2, 0.17, 0.13)),
            ground_horizon_color: parse_sky_color("ground_horizon_color", (0.55, 0.55, 0.52)),
            sun_angle_azimuth: sky_data.get("sun_angle_azimuth").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
            sun_angle_elevation: sky_data.get("sun_angle_elevation").and_then(|v| v.as_f64()).unwrap_or(45.0) as f32,
            sun_disk_scale: sky_data.get("sun_disk_scale").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            sun_color: parse_sky_color("sun_color", (1.0, 0.95, 0.85)),
            sun_energy: sky_data.get("sun_energy").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            sky_curve: sky_data.get("sky_curve").and_then(|v| v.as_f64()).unwrap_or(0.15) as f32,
            ground_curve: sky_data.get("ground_curve").and_then(|v| v.as_f64()).unwrap_or(0.02) as f32,
        }
    } else {
        ProceduralSkyData::default()
    };

    // Panorama sky
    let panorama_sky = if let Some(pano_data) = data.get("panorama_sky").and_then(|v| v.as_object()) {
        PanoramaSkyData {
            panorama_path: pano_data.get("panorama_path").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            rotation: pano_data.get("rotation").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
            energy: pano_data.get("energy").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        }
    } else {
        PanoramaSkyData::default()
    };

    // Fog
    let fog_enabled = data.get("fog_enabled").and_then(|v| v.as_bool()).unwrap_or(false);
    let fog_color = parse_color("fog_color", (0.5, 0.5, 0.5));
    let fog_start = data.get("fog_start").and_then(|v| v.as_f64()).unwrap_or(10.0) as f32;
    let fog_end = data.get("fog_end").and_then(|v| v.as_f64()).unwrap_or(100.0) as f32;

    // Anti-aliasing
    let msaa_samples = data.get("msaa_samples").and_then(|v| v.as_u64()).unwrap_or(4) as u8;
    let fxaa_enabled = data.get("fxaa_enabled").and_then(|v| v.as_bool()).unwrap_or(false);

    // SSAO
    let ssao_enabled = data.get("ssao_enabled").and_then(|v| v.as_bool()).unwrap_or(false);
    let ssao_intensity = data.get("ssao_intensity").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
    let ssao_radius = data.get("ssao_radius").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;

    // SSR
    let ssr_enabled = data.get("ssr_enabled").and_then(|v| v.as_bool()).unwrap_or(false);
    let ssr_intensity = data.get("ssr_intensity").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;
    let ssr_max_steps = data.get("ssr_max_steps").and_then(|v| v.as_u64()).unwrap_or(64) as u32;

    // Bloom
    let bloom_enabled = data.get("bloom_enabled").and_then(|v| v.as_bool()).unwrap_or(false);
    let bloom_intensity = data.get("bloom_intensity").and_then(|v| v.as_f64()).unwrap_or(0.15) as f32;
    let bloom_threshold = data.get("bloom_threshold").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;

    // Tonemapping
    let tonemapping = data.get("tonemapping")
        .and_then(|v| v.as_str())
        .map(|s| match s {
            "None" => TonemappingMode::None,
            "Reinhard" => TonemappingMode::Reinhard,
            "ReinhardLuminance" => TonemappingMode::ReinhardLuminance,
            "AcesFitted" => TonemappingMode::AcesFitted,
            "AgX" => TonemappingMode::AgX,
            "SomewhatBoringDisplayTransform" => TonemappingMode::SomewhatBoringDisplayTransform,
            "TonyMcMapface" => TonemappingMode::TonyMcMapface,
            "BlenderFilmic" => TonemappingMode::BlenderFilmic,
            _ => TonemappingMode::Reinhard,
        })
        .unwrap_or(TonemappingMode::Reinhard);
    let exposure = data.get("exposure").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;

    // Depth of Field
    let dof_enabled = data.get("dof_enabled").and_then(|v| v.as_bool()).unwrap_or(false);
    let dof_focal_distance = data.get("dof_focal_distance").and_then(|v| v.as_f64()).unwrap_or(10.0) as f32;
    let dof_aperture = data.get("dof_aperture").and_then(|v| v.as_f64()).unwrap_or(0.05) as f32;

    // Motion Blur
    let motion_blur_enabled = data.get("motion_blur_enabled").and_then(|v| v.as_bool()).unwrap_or(false);
    let motion_blur_intensity = data.get("motion_blur_intensity").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;

    entity_commands.insert(WorldEnvironmentMarker {
        data: WorldEnvironmentData {
            ambient_color,
            ambient_brightness,
            sky_mode,
            clear_color,
            procedural_sky,
            panorama_sky,
            fog_enabled,
            fog_color,
            fog_start,
            fog_end,
            msaa_samples,
            fxaa_enabled,
            ssao_enabled,
            ssao_intensity,
            ssao_radius,
            ssr_enabled,
            ssr_intensity,
            ssr_max_steps,
            bloom_enabled,
            bloom_intensity,
            bloom_threshold,
            tonemapping,
            exposure,
            dof_enabled,
            dof_focal_distance,
            dof_aperture,
            motion_blur_enabled,
            motion_blur_intensity,
        },
    });
}

// --- Audio Listener ---

fn spawn_audio_listener(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: AUDIO_LISTENER.default_name.to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        NodeTypeMarker::new(AUDIO_LISTENER.type_id),
        AudioListenerMarker,
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

fn deserialize_audio_listener(
    entity_commands: &mut EntityCommands,
    _data: &HashMap<String, serde_json::Value>,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    entity_commands.insert(AudioListenerMarker);
}
