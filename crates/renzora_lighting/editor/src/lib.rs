//! Editor-only half of `renzora_lighting` — the Sun inspector entry
//! (azimuth/elevation/color/etc. declarative fields).
//!
//! `renzora_lighting` compiles lean (no `editor` feature, no egui-phosphor).
//! This crate holds the inspector (renzora editor contract + Phosphor icon),
//! registered `renzora::add!(LightingEditorPlugin, Editor)` and linked only by
//! the editor bundle.

use bevy::prelude::*;
use renzora_editor_api::{AppEditorExt, InspectorEntry};
use renzora_lighting::Sun;

fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "sun",
        display_name: "Sun",
        icon: "sun-horizon",
        category: "lighting",
        has_fn: |world, entity| world.get::<Sun>(entity).is_some(),
        // A light is an ENTITY, not a component you bolt onto something else —
        // it needs its own transform. Add one via "Add Entity → Sun" (the
        // `world_environment` / `sun` entity presets), and delete it by deleting
        // its entity. So no component add/remove here (and no trash button).
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            renzora_editor_api::float_field!("Azimuth", Sun, azimuth, 1.0, 0.0, 360.0),
            renzora_editor_api::float_field!("Elevation", Sun, elevation, 1.0, -90.0, 90.0),
            renzora_editor_api::vec3_color_field!("Color", Sun, color),
            renzora_editor_api::float_field!("Illuminance", Sun, illuminance, 100.0, 0.0, f32::MAX),
            renzora_editor_api::float_field!("Angular Diameter", Sun, angular_diameter, 0.01, 0.0, 10.0),
            renzora_editor_api::float_field!("Disk Intensity", Sun, sun_disk_intensity, 0.01, 0.0, 10.0),
            renzora_editor_api::float_field!("Glare", Sun, glare_intensity, 0.01, 0.0, 10.0),
            renzora_editor_api::float_field!("Glare Size", Sun, glare_size, 0.1, 0.1, 45.0),
            renzora_editor_api::bool_field!("Shadows", Sun, shadows_enabled),
            // The switch is the group's header; its settings fold under it.
            renzora_editor_api::FieldDef::toggle_group(
                renzora_editor_api::bool_field!("Contact Shadows", Sun, contact_shadows),
                vec![
                    renzora_editor_api::float_field!("Length", Sun, contact_shadow_length, 0.01, 0.01, 5.0),
                    renzora_editor_api::float_field!("Thickness", Sun, contact_shadow_thickness, 0.005, 0.001, 1.0),
                    renzora_editor_api::int_field!(
                        "Steps",
                        Sun,
                        contact_shadow_steps,
                        u32,
                        1.0,
                        *renzora_world_api::sun::CONTACT_SHADOW_STEPS.start() as f32,
                        *renzora_world_api::sun::CONTACT_SHADOW_STEPS.end() as f32
                    ),
                ],
            ),
            renzora_editor_api::FieldDef::group(
                "Moon",
                vec![
                    renzora_editor_api::float_field!("Moonlight", Sun, moon_illuminance, 1.0, 0.0, 1000.0),
                    renzora_editor_api::vec3_color_field!("Moon Color", Sun, moon_color),
                    renzora_editor_api::bool_field!("Follows Calendar", Sun, moon_follows_calendar),
                    renzora_editor_api::float_field!("Moon Day", Sun, moon_day, 0.1, 0.0, 29.5),
                    renzora_editor_api::vec3_color_field!("Moon Tint", Sun, moon_tint),
                    renzora_editor_api::float_field!("Eclipse", Sun, moon_eclipse, 0.01, 0.0, 1.0),
                    renzora_editor_api::float_field!("Moon Glow", Sun, moon_glow, 0.05, 0.0, 10.0),
                    renzora_editor_api::float_field!("Moon Size", Sun, moon_size, 0.05, 0.1, 20.0),
                ],
            ),
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
