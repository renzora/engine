//! Editor-only half of `renzora_volumetric_fog`: the Volumetric Fog inspector,
//! and the Fog Volume (its Add Entity entry and its card).
//!
//! `renzora_volumetric_fog` compiles lean (no `editor` feature). This crate
//! holds the editor contract, registered `renzora::add!(VolumetricFogEditorPlugin,
//! Editor)` and linked only by the editor bundle.

use bevy::light::{FogVolume, VolumetricFog, VolumetricLight};
use bevy::prelude::*;
use renzora::{AppEditorExt, EntityPreset, FieldDef, FieldType, FieldValue, InspectorEntry};
use renzora_volumetric_fog::VolumetricFogSettings;

fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "volumetric_fog",
        display_name: "Volumetric Fog",
        icon: "cloud-fog",
        category: "environment",
        // The settings now live on the Fog Volume and show on its card. This
        // card stays only for a World Environment or camera that carries them
        // from a scene saved before that, and is no longer offered to add:
        // three cards (Fog Volume, Volumetric Fog, Volumetric Light) for one
        // effect was the confusing part.
        has_fn: |world, entity| {
            world.get::<VolumetricFogSettings>(entity).is_some() && world.get::<FogVolume>(entity).is_none()
        },
        add_fn: None,
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<(VolumetricFogSettings, VolumetricFog)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<VolumetricFogSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<VolumetricFogSettings>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![
            renzora::tuple_color_field!("Color", VolumetricFogSettings, ambient_color),
            renzora::float_field!("Ambient Intensity", VolumetricFogSettings, ambient_intensity, 0.01, 0.0, 4.0),
            renzora::int_field!("Step Count", VolumetricFogSettings, step_count, u32, 1.0, 8.0, 256.0),
            renzora::float_field!("Jitter", VolumetricFogSettings, jitter, 0.01, 0.0, 1.0),
        ],
    }
}

/// The fog a new Fog Volume starts with. Bevy's own default (density 0.1) is
/// sized for a box a few metres across: over a street-sized box it absorbed
/// most of the light and darkened the whole frame. These are thinner, and lean
/// on scattering over absorption, so the fog lights up where the sun reaches it
/// (the shafts) instead of only dimming what is behind it.
pub fn outdoor_fog_volume() -> FogVolume {
    FogVolume {
        density_factor: 0.02,
        absorption: 0.1,
        scattering: 0.3,
        scattering_asymmetry: 0.6,
        ..default()
    }
}

/// How big a new Fog Volume is, in metres: a street's width and a few storeys
/// tall, so it reads at once. Resize it with the Transform's scale.
const NEW_VOLUME_SIZE: Vec3 = Vec3::new(40.0, 20.0, 40.0);

fn fog_volume_preset() -> EntityPreset {
    EntityPreset {
        id: "fog_volume",
        display_name: "Fog Volume",
        icon: "cloud-fog",
        category: "environment",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("Fog Volume"),
                    // Sat on the ground rather than half under it.
                    Transform::from_translation(Vec3::Y * NEW_VOLUME_SIZE.y * 0.5)
                        .with_scale(NEW_VOLUME_SIZE),
                    outdoor_fog_volume(),
                    // How the camera draws it, shown on the same card.
                    VolumetricFogSettings::default(),
                ))
                .id()
        },
    }
}

/// A `Color` field on the Fog Volume, read and written as sRGB.
macro_rules! fog_color_field {
    ($name:expr, $field:ident) => {
        FieldDef {
            name: $name,
            field_type: FieldType::Color,
            get_fn: |w, e| {
                w.get::<FogVolume>(e).map(|v| {
                    let c = v.$field.to_srgba();
                    FieldValue::Color([c.red, c.green, c.blue])
                })
            },
            set_fn: |w, e, val| {
                if let (FieldValue::Color([r, g, b]), Some(mut v)) = (val, w.get_mut::<FogVolume>(e)) {
                    v.$field = Color::srgb(r, g, b);
                }
            },
        }
    };
}

/// A float field on the Fog Volume.
macro_rules! fog_float_field {
    ($name:expr, $field:ident, $speed:expr, $min:expr, $max:expr) => {
        FieldDef {
            name: $name,
            field_type: FieldType::Float { speed: $speed, min: $min, max: $max },
            get_fn: |w, e| w.get::<FogVolume>(e).map(|v| FieldValue::Float(v.$field)),
            set_fn: |w, e, val| {
                if let (FieldValue::Float(f), Some(mut v)) = (val, w.get_mut::<FogVolume>(e)) {
                    v.$field = f;
                }
            },
        }
    };
}

fn fog_volume_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "fog_volume",
        display_name: "Fog Volume",
        icon: "cloud-fog",
        category: "environment",
        has_fn: |world, entity| world.get::<FogVolume>(entity).is_some(),
        // A Fog Volume is an entity, like a light: its box is its Transform.
        // So it comes from Add Entity (sized, see `fog_volume_preset`) and is
        // removed by deleting it. An `add_fn` here also listed it a second
        // time in Add Entity, as a 1 m box nobody could see.
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            // The one setup step the fog cannot do for itself: shafts come
            // only from lights marked volumetric. Shown only while none is.
            FieldDef {
                name: "Light Shafts",
                field_type: FieldType::ReadOnly,
                get_fn: |w, _| {
                    let any_lit = w
                        .try_query::<&VolumetricLight>()
                        .is_some_and(|mut q| q.iter(w).next().is_some());
                    (!any_lit).then(|| {
                        FieldValue::ReadOnly(
                            "No light casts shafts yet. Add Volumetric Light to the sun or a lamp."
                                .to_string(),
                        )
                    })
                },
                set_fn: |_, _, _| {},
            },
            fog_float_field!("Density", density_factor, 0.001, 0.0, 2.0),
            fog_color_field!("Color", fog_color),
            FieldDef::group(
                "Light",
                vec![
                    fog_float_field!("Scattering", scattering, 0.01, 0.0, 1.0),
                    fog_float_field!("Absorption", absorption, 0.01, 0.0, 1.0),
                    fog_float_field!("Forward Scatter", scattering_asymmetry, 0.01, -0.99, 0.99),
                    fog_color_field!("Light Tint", light_tint),
                    fog_float_field!("Light Intensity", light_intensity, 0.05, 0.0, 20.0),
                ],
            ),
            // How the camera draws the fog: the settings that used to be a
            // separate Volumetric Fog card. A Fog Volume from before they
            // moved here has none, and these rows simply do not show.
            FieldDef::group(
                "Sky Light",
                vec![
                    renzora::tuple_color_field!("Sky Color", VolumetricFogSettings, ambient_color),
                    renzora::float_field!("Sky Brightness", VolumetricFogSettings, ambient_intensity, 0.01, 0.0, 4.0),
                ],
            ),
            FieldDef::group(
                "Quality",
                vec![
                    renzora::int_field!("Step Count", VolumetricFogSettings, step_count, u32, 1.0, 8.0, 256.0),
                    renzora::float_field!("Jitter", VolumetricFogSettings, jitter, 0.01, 0.0, 1.0),
                ],
            ),
        ],
    }
}

/// Editor-scope companion to `renzora_volumetric_fog::VolumetricFogPlugin`.
#[derive(Default)]
pub struct VolumetricFogEditorPlugin;

impl Plugin for VolumetricFogEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] VolumetricFogEditorPlugin");
        app.register_inspector(inspector_entry());
        app.register_inspector(fog_volume_entry());
        app.register_entity_preset(fog_volume_preset());
    }
}

renzora::add!(VolumetricFogEditorPlugin, Editor);
