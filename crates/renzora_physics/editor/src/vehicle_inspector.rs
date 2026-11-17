//! The Vehicle inspector: which axles are the wheels, and how the car drives.

use bevy::prelude::*;
use renzora::vehicle::{Vehicle, VehicleDrive};
use renzora::{FieldDef, FieldType, FieldValue, InspectorEntry};

/// The axles a vehicle's model offers: every part with wheels separated out
/// of it (children named `<part>_part<n>`), which is what Separate Parts
/// leaves. Listed by the name the Hierarchy shows.
fn axles(world: &World, root: Entity) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        let Some(children) = world.get::<Children>(e) else { continue };
        stack.extend(children.iter());
        let Some(name) = world.get::<Name>(e).map(|n| n.as_str().to_string()) else { continue };
        let prefix = format!("{}_part", name.to_lowercase().replace('.', "_"));
        let separated = children.iter().any(|c| {
            world
                .get::<Name>(c)
                .is_some_and(|n| n.as_str().to_lowercase().replace('.', "_").starts_with(&prefix))
        });
        if separated && !out.contains(&name) {
            out.push(name);
        }
    }
    out
}


pub(crate) fn vehicle_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "vehicle",
        display_name: "Vehicle",
        icon: "car",
        category: "physics",
        has_fn: |world, entity| world.get::<Vehicle>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(Vehicle::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<Vehicle>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Front Axle",
                field_type: FieldType::DynamicEnum { options: axles },
                get_fn: |w, e| w.get::<Vehicle>(e).map(|v| FieldValue::Enum(v.front_axle.clone())),
                set_fn: |w, e, v| {
                    if let (FieldValue::Enum(name), Some(mut veh)) = (v, w.get_mut::<Vehicle>(e)) {
                        veh.front_axle = name;
                    }
                },
            },
            FieldDef {
                name: "Rear Axle",
                field_type: FieldType::DynamicEnum { options: axles },
                get_fn: |w, e| w.get::<Vehicle>(e).map(|v| FieldValue::Enum(v.rear_axle.clone())),
                set_fn: |w, e, v| {
                    if let (FieldValue::Enum(name), Some(mut veh)) = (v, w.get_mut::<Vehicle>(e)) {
                        veh.rear_axle = name;
                    }
                },
            },
            FieldDef {
                name: "Drive",
                field_type: FieldType::Enum { options: &["Front", "Rear", "All"] },
                get_fn: |w, e| {
                    w.get::<Vehicle>(e).map(|v| {
                        FieldValue::Enum(
                            match v.drive {
                                VehicleDrive::Front => "Front",
                                VehicleDrive::Rear => "Rear",
                                VehicleDrive::All => "All",
                            }
                            .to_string(),
                        )
                    })
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Enum(label), Some(mut veh)) = (v, w.get_mut::<Vehicle>(e)) {
                        veh.drive = match label.as_str() {
                            "Front" => VehicleDrive::Front,
                            "All" => VehicleDrive::All,
                            _ => VehicleDrive::Rear,
                        };
                    }
                },
            },
            renzora::float_field!("Engine Force (N)", Vehicle, engine_force, 100.0, 0.0, 1_000_000.0),
            renzora::float_field!("Brake Force (N)", Vehicle, brake_force, 100.0, 0.0, 1_000_000.0),
            renzora::float_field!("Top Speed (m/s)", Vehicle, max_speed, 0.5, 0.0, 500.0),
            renzora::float_field!("Reverse Speed (m/s)", Vehicle, max_reverse_speed, 0.5, 0.0, 100.0),
            renzora::float_field!("Steering (deg)", Vehicle, max_steer_degrees, 0.5, 0.0, 80.0),
            renzora::float_field!("Suspension Travel (m)", Vehicle, suspension_travel, 0.01, 0.01, 2.0),
            renzora::float_field!("Stiffness (N/m, 0 auto)", Vehicle, suspension_stiffness, 100.0, 0.0, 10_000_000.0),
            renzora::float_field!("Damping (0 to 1)", Vehicle, suspension_damping, 0.01, 0.0, 1.0),
            renzora::float_field!("Grip", Vehicle, grip, 0.01, 0.0, 5.0),
            renzora::float_field!("Wheel Radius (m, 0 auto)", Vehicle, wheel_radius, 0.01, 0.0, 5.0),
            renzora::bool_field!("Drive With Input", Vehicle, use_input),
        ],
    }
}
