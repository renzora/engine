//! Auto-generated blueprint nodes for component properties.
//!
//! For each component with `script_property_meta_fn`, this module generates:
//! - A **getter** data node (e.g. "Get Sun") with one output pin per property
//! - A **setter** action node (e.g. "Set Sun") with exec pins + one input per property
//!
//! Also provides generic "Get Property" and "Set Property" utility nodes.

use super::{ComponentNodeDef, NodeRegistry, NodeTypeDefinition};
use crate::blueprint::{Pin, PinDirection, PinType, PinValue};
use crate::component_system::{ComponentRegistry, PropertyValueType};

/// Map a PropertyValueType to a PinType
fn property_type_to_pin_type(pvt: PropertyValueType) -> PinType {
    match pvt {
        PropertyValueType::Float => PinType::Float,
        PropertyValueType::Int => PinType::Int,
        PropertyValueType::Bool => PinType::Bool,
        PropertyValueType::String => PinType::String,
        PropertyValueType::Vec2 => PinType::Vec2,
        PropertyValueType::Vec3 => PinType::Vec3,
        PropertyValueType::Color => PinType::Color,
    }
}

/// Map a PropertyValueType to a default PinValue
fn property_type_default_value(pvt: PropertyValueType) -> PinValue {
    match pvt {
        PropertyValueType::Float => PinValue::Float(0.0),
        PropertyValueType::Int => PinValue::Int(0),
        PropertyValueType::Bool => PinValue::Bool(false),
        PropertyValueType::String => PinValue::String(String::new()),
        PropertyValueType::Vec2 => PinValue::Vec2([0.0, 0.0]),
        PropertyValueType::Vec3 => PinValue::Vec3([0.0, 0.0, 0.0]),
        PropertyValueType::Color => PinValue::Color([1.0, 1.0, 1.0, 1.0]),
    }
}

/// Generate and register component nodes for all components that have script property metadata.
pub fn generate_component_nodes(node_reg: &mut NodeRegistry, comp_reg: &ComponentRegistry) {
    for comp_def in comp_reg.all() {
        let Some(meta_fn) = comp_def.script_property_meta_fn else {
            continue;
        };

        let props = meta_fn();
        if props.is_empty() {
            continue;
        }

        let type_id = comp_def.type_id;
        let display_name = comp_def.display_name;

        // ── Getter node ──────────────────────────────────────────────
        let mut getter_pins = Vec::new();

        // Input: entity name (string)
        getter_pins.push(Pin {
            name: "name".to_string(),
            label: "Name".to_string(),
            pin_type: PinType::String,
            direction: PinDirection::Input,
            default_value: Some(PinValue::String(display_name.to_string())),
            required: false,
        });

        // Output: one pin per property
        for &(prop_name, prop_type) in &props {
            getter_pins.push(Pin {
                name: prop_name.to_string(),
                label: pretty_label(prop_name),
                pin_type: property_type_to_pin_type(prop_type),
                direction: PinDirection::Output,
                default_value: None,
                required: false,
            });
        }

        node_reg.register_component_node(ComponentNodeDef {
            type_id: format!("component/get_{}", type_id),
            display_name: format!("Get {}", display_name),
            category: "Components".to_string(),
            description: format!("Read properties from a {} component by entity name", display_name),
            pins: getter_pins,
            color: [100, 180, 220],
        });

        // ── Setter node ──────────────────────────────────────────────
        let mut setter_pins = Vec::new();

        // Exec input
        setter_pins.push(Pin {
            name: "exec".to_string(),
            label: "".to_string(),
            pin_type: PinType::Execution,
            direction: PinDirection::Input,
            default_value: None,
            required: false,
        });

        // Entity name
        setter_pins.push(Pin {
            name: "name".to_string(),
            label: "Name".to_string(),
            pin_type: PinType::String,
            direction: PinDirection::Input,
            default_value: Some(PinValue::String(display_name.to_string())),
            required: false,
        });

        // One input per writable property
        for &(prop_name, prop_type) in &props {
            setter_pins.push(Pin {
                name: prop_name.to_string(),
                label: pretty_label(prop_name),
                pin_type: property_type_to_pin_type(prop_type),
                direction: PinDirection::Input,
                default_value: Some(property_type_default_value(prop_type)),
                required: false,
            });
        }

        // Exec output
        setter_pins.push(Pin {
            name: "exec".to_string(),
            label: "".to_string(),
            pin_type: PinType::Execution,
            direction: PinDirection::Output,
            default_value: None,
            required: false,
        });

        node_reg.register_component_node(ComponentNodeDef {
            type_id: format!("component/set_{}", type_id),
            display_name: format!("Set {}", display_name),
            category: "Components".to_string(),
            description: format!("Write properties on a {} component by entity name (only connected pins are written)", display_name),
            pins: setter_pins,
            color: [220, 140, 100],
        });
    }

    // Register generic Get Property / Set Property nodes
    node_reg.register(&GET_PROPERTY);
    node_reg.register(&SET_PROPERTY);
}

/// Convert snake_case property name to a pretty label.
fn pretty_label(name: &str) -> String {
    name.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ─── Generic Get Property node ───────────────────────────────────────────────

pub static GET_PROPERTY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "component/get_property",
    display_name: "Get Property",
    category: "Components",
    description: "Read a single property from an entity by name and property name",
    create_pins: get_property_pins,
    color: [100, 180, 220],
    is_event: false,
    is_comment: false,
};

fn get_property_pins() -> Vec<Pin> {
    vec![
        Pin::input("name", "Entity Name", PinType::String)
            .with_default(PinValue::String(String::new())),
        Pin::input("property", "Property", PinType::String)
            .with_default(PinValue::String(String::new())),
        Pin::output("value", "Value", PinType::Any),
    ]
}

// ─── Generic Set Property node ───────────────────────────────────────────────

pub static SET_PROPERTY: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "component/set_property",
    display_name: "Set Property",
    category: "Components",
    description: "Write a single property on an entity by name and property name",
    create_pins: set_property_pins,
    color: [220, 140, 100],
    is_event: false,
    is_comment: false,
};

fn set_property_pins() -> Vec<Pin> {
    vec![
        Pin::input("exec", "", PinType::Execution),
        Pin::input("name", "Entity Name", PinType::String)
            .with_default(PinValue::String(String::new())),
        Pin::input("property", "Property", PinType::String)
            .with_default(PinValue::String(String::new())),
        Pin::input("value", "Value", PinType::Any)
            .with_default(PinValue::Float(0.0)),
        Pin::output("exec", "", PinType::Execution),
    ]
}
