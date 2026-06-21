//! Editor-only half of `renzora_lighting` — the Sun inspector entry
//! (azimuth/elevation/color/etc. declarative fields).
//!
//! `renzora_lighting` compiles lean (no `editor` feature, no egui-phosphor).
//! This crate holds the inspector (renzora editor contract + Phosphor icon),
//! registered `renzora::add!(LightingEditorPlugin, Editor)` and linked only by
//! the editor bundle.

use bevy::prelude::*;
use renzora::{AppEditorExt, InspectorEntry};
use renzora_lighting::Sun;

fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "sun",
        display_name: "Sun",
        icon: "sun-horizon",
        category: "lighting",
        has_fn: |world, entity| world.get::<Sun>(entity).is_some(),
        add_fn: Some(|world, entity| {
            let data = Sun::default();
            let dir = data.direction();
            world.entity_mut(entity).insert((
                DirectionalLight {
                    color: Color::srgb(data.color.x, data.color.y, data.color.z),
                    illuminance: data.illuminance,
                    shadow_maps_enabled: data.shadows_enabled,
                    ..default()
                },
                Transform::from_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, dir)),
                data,
            ));
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(Sun, DirectionalLight)>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            renzora::float_field!("Azimuth", Sun, azimuth, 1.0, 0.0, 360.0),
            renzora::float_field!("Elevation", Sun, elevation, 1.0, -90.0, 90.0),
            renzora::vec3_color_field!("Color", Sun, color),
            renzora::float_field!("Illuminance", Sun, illuminance, 100.0, 0.0, f32::MAX),
            renzora::float_field!("Angular Diameter", Sun, angular_diameter, 0.01, 0.0, 10.0),
            renzora::float_field!("Disk Intensity", Sun, sun_disk_intensity, 0.01, 0.0, 10.0),
            renzora::bool_field!("Shadows", Sun, shadows_enabled),
            renzora::bool_field!("Contact Shadows", Sun, contact_shadows),
        ],
    }
}

/// Editor-scope companion to `renzora_lighting::LightingPlugin`.
#[derive(Default)]
pub struct LightingEditorPlugin;

impl Plugin for LightingEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] LightingEditorPlugin");
        app.register_inspector(inspector_entry());
    }
}

renzora::add!(LightingEditorPlugin, Editor);
