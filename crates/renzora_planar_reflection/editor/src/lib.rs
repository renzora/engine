//! Editor-only half of `renzora_planar_reflection`.
//!
//! - **The Planar Reflection inspector** (Rendering page) with every field of
//!   [`PlanarReflection`]. Its `add_fn` is what puts it in Add Component, so any
//!   flat mesh already in a scene can become a mirror.
//! - **Add Entity > Mirror**: an upright plane facing +Z with a full-strength,
//!   angle-independent reflection.
//!
//! Plain registry fields rather than a custom drawer, so this crate needs the
//! editor contract and nothing else, plus the viewport gizmo (`gizmo`).

mod gizmo;

use bevy::prelude::*;
use renzora::{
    AppEditorExt, EntityPreset, FieldDef, FieldType, FieldValue, InspectorEntry, MeshColor,
    MeshPrimitive, ShapeRegistry,
};
use renzora_planar_reflection::{NormalAxis, PlanarReflection};

fn with_reflection(world: &mut World, entity: Entity, apply: impl FnOnce(&mut PlanarReflection)) {
    if let Some(mut reflection) = world.get_mut::<PlanarReflection>(entity) {
        apply(&mut reflection);
    }
}

fn normal_axis_field() -> FieldDef {
    FieldDef {
        name: "Normal Axis",
        field_type: FieldType::Enum {
            options: &NormalAxis::LABELS,
        },
        get_fn: |world, entity| {
            world
                .get::<PlanarReflection>(entity)
                .map(|r| FieldValue::Enum(r.normal_axis.label().to_string()))
        },
        set_fn: |world, entity, value| {
            if let FieldValue::Enum(label) = value {
                with_reflection(world, entity, |r| r.normal_axis = NormalAxis::from_label(&label));
            }
        },
    }
}

/// The tint as an sRGB swatch. The component stores a `Color`, so the swatch
/// round-trips through sRGB, the space the colour picker edits in.
fn tint_field() -> FieldDef {
    FieldDef {
        name: "Tint",
        field_type: FieldType::Color,
        get_fn: |world, entity| {
            world.get::<PlanarReflection>(entity).map(|r| {
                let c = r.tint.to_srgba();
                FieldValue::Color([c.red, c.green, c.blue])
            })
        },
        set_fn: |world, entity, value| {
            if let FieldValue::Color([red, green, blue]) = value {
                with_reflection(world, entity, |r| r.tint = Color::srgb(red, green, blue));
            }
        },
    }
}

/// `update_every` as a whole number. `FieldType::Int`, not a float drag cast
/// on write, or the fractional drag and the rounded re-read fight mid-drag.
fn update_every_field() -> FieldDef {
    FieldDef {
        name: "Update Every (frames)",
        field_type: FieldType::Int {
            min: 1.0,
            max: 60.0,
        },
        get_fn: |world, entity| {
            world
                .get::<PlanarReflection>(entity)
                .map(|r| FieldValue::Float(r.update_every as f32))
        },
        set_fn: |world, entity, value| {
            if let FieldValue::Float(v) = value {
                with_reflection(world, entity, |r| r.update_every = v.round().max(1.0) as u32);
            }
        },
    }
}

fn fit_to_mesh_field() -> FieldDef {
    FieldDef {
        name: "Fit to Mesh",
        field_type: FieldType::Bool,
        get_fn: |world, entity| world.get::<PlanarReflection>(entity).map(|r| FieldValue::Bool(r.fit_to_mesh)),
        set_fn: |world, entity, value| {
            if let FieldValue::Bool(v) = value {
                with_reflection(world, entity, |r| r.fit_to_mesh = v);
            }
        },
    }
}

fn flip_field() -> FieldDef {
    FieldDef {
        name: "Flip",
        field_type: FieldType::Bool,
        get_fn: |world, entity| world.get::<PlanarReflection>(entity).map(|r| FieldValue::Bool(r.flip)),
        set_fn: |world, entity, value| {
            if let FieldValue::Bool(v) = value {
                with_reflection(world, entity, |r| r.flip = v);
            }
        },
    }
}

fn planar_reflection_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "planar_reflection",
        display_name: "Planar Reflection",
        icon: "flip-horizontal",
        category: "rendering",
        has_fn: |world, entity| world.get::<PlanarReflection>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(PlanarReflection::default());
        }),
        remove_fn: Some(|world, entity| {
            // The runtime notices the component is gone and tears down the
            // capture camera and the overlay itself.
            world.entity_mut(entity).remove::<PlanarReflection>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<PlanarReflection>(entity)
                .is_some_and(|r| r.enabled)
        }),
        set_enabled_fn: Some(|world, entity, on| {
            with_reflection(world, entity, |r| r.enabled = on);
        }),
        fields: vec![
            normal_axis_field(),
            flip_field(),
            fit_to_mesh_field(),
            renzora::float_field!("Plane Offset", PlanarReflection, plane_offset, 0.005, -100.0, 100.0),
            renzora::float_field!("Resolution Scale", PlanarReflection, resolution_scale, 0.01, 0.25, 1.0),
            renzora::float_field!("Strength", PlanarReflection, strength, 0.01, 0.0, 1.0),
            tint_field(),
            renzora::float_field!("Fresnel", PlanarReflection, fresnel, 0.01, 0.0, 1.0),
            renzora::float_field!("Blur", PlanarReflection, blur, 0.01, 0.0, 1.0),
            renzora::float_field!("Clip Offset", PlanarReflection, clip_offset, 0.001, 0.0, 1.0),
            renzora::float_field!("Max Distance", PlanarReflection, max_distance, 1.0, 0.0, 10_000.0),
            update_every_field(),
        ],
    }
}

/// An upright 2x2 mirror standing on the ground, facing +Z.
///
/// Built on the stock Plane primitive so it saves and reloads like any other
/// shape (a scene stores the `MeshPrimitive` id, not the mesh). That plane
/// faces local +Y, so it is turned a quarter about X to face +Z, and the
/// reflection normal is the mesh's own face normal, local Y.
fn mirror_preset() -> EntityPreset {
    EntityPreset {
        id: "planar_mirror",
        display_name: "Mirror",
        icon: "flip-horizontal",
        category: "rendering",
        spawn_fn: |world| {
            // Without a registry the mesh comes from scene rehydration, which
            // builds any `MeshPrimitive` that has no `Mesh3d` yet.
            let mesh = world
                .contains_resource::<ShapeRegistry>()
                .then(|| {
                    world.resource_scope(|world, registry: Mut<ShapeRegistry>| {
                        let mut meshes = world.resource_mut::<Assets<Mesh>>();
                        registry.create_mesh("plane", &mut meshes)
                    })
                })
                .flatten();
            // Dark and glossy: at full strength the reflection covers it, and
            // where the capture is off (out of range, over the cap) a dark
            // mirror reads as a mirror rather than as a grey card.
            let color = Color::srgb(0.05, 0.05, 0.06);
            let material = world.resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial {
                base_color: color,
                perceptual_roughness: 0.1,
                metallic: 1.0,
                ..default()
            });
            let mut entity = world.spawn((
                Name::new("Mirror"),
                Transform::from_xyz(0.0, 1.0, 0.0)
                    .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                Visibility::default(),
                MeshPrimitive("plane".to_string()),
                MeshColor(color),
                MeshMaterial3d(material),
                PlanarReflection::mirror(NormalAxis::Y),
            ));
            if let Some(mesh) = mesh {
                entity.insert(Mesh3d(mesh));
            }
            entity.id()
        },
    }
}

/// Editor-scope companion to `renzora_planar_reflection::PlanarReflectionPlugin`.
#[derive(Default)]
pub struct PlanarReflectionEditorPlugin;

impl Plugin for PlanarReflectionEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] PlanarReflectionEditorPlugin");
        app.register_inspector(planar_reflection_entry());
        app.add_systems(Update, gizmo::draw_reflection_gizmos);
        app.register_entity_preset(mirror_preset());
    }
}

renzora::add!(PlanarReflectionEditorPlugin, Editor);
