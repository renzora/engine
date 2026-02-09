//! Particle system control nodes

use super::{NodeTypeDefinition, Pin, PinType, PinValue};

/// Particle burst emission
pub static BURST: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "particles/burst",
    display_name: "Particle Burst",
    category: "Particles",
    description: "Emit a burst of particles",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("count", "Count", PinType::Int).with_default(PinValue::Int(10)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [200, 150, 100],
    is_event: false,
    is_comment: false,
};

/// Set particle emission rate
pub static SET_RATE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "particles/set_rate",
    display_name: "Set Emission Rate",
    category: "Particles",
    description: "Set the particle emission rate",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("rate", "Rate", PinType::Float).with_default(PinValue::Float(10.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [200, 150, 100],
    is_event: false,
    is_comment: false,
};

/// Set particle scale
pub static SET_SCALE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "particles/set_scale",
    display_name: "Set Particle Scale",
    category: "Particles",
    description: "Set the particle system scale",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("scale", "Scale", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [200, 150, 100],
    is_event: false,
    is_comment: false,
};

/// Set particle time scale
pub static SET_TIME_SCALE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "particles/set_time_scale",
    display_name: "Set Particle Time Scale",
    category: "Particles",
    description: "Set the particle system time scale",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("time_scale", "Time Scale", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [200, 150, 100],
    is_event: false,
    is_comment: false,
};

/// Set particle tint color
pub static SET_TINT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "particles/set_tint",
    display_name: "Set Particle Tint",
    category: "Particles",
    description: "Set the particle tint color",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("r", "R", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("g", "G", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("a", "A", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [200, 150, 100],
    is_event: false,
    is_comment: false,
};

/// Reset particle system
pub static RESET: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "particles/reset",
    display_name: "Reset Particles",
    category: "Particles",
    description: "Reset a particle system to initial state",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [200, 150, 100],
    is_event: false,
    is_comment: false,
};

/// Set particle variable (float)
pub static SET_VARIABLE_FLOAT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "particles/set_variable_float",
    display_name: "Set Particle Float",
    category: "Particles",
    description: "Set a float variable on a particle system",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("name", "Name", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [200, 150, 100],
    is_event: false,
    is_comment: false,
};

/// Set particle variable (color)
pub static SET_VARIABLE_COLOR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "particles/set_variable_color",
    display_name: "Set Particle Color",
    category: "Particles",
    description: "Set a color variable on a particle system",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("name", "Name", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("r", "R", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("g", "G", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("a", "A", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [200, 150, 100],
    is_event: false,
    is_comment: false,
};

/// Set particle variable (vec3)
pub static SET_VARIABLE_VEC3: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "particles/set_variable_vec3",
    display_name: "Set Particle Vec3",
    category: "Particles",
    description: "Set a vec3 variable on a particle system",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("name", "Name", PinType::String).with_default(PinValue::String("".into())),
        Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("y", "Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("z", "Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [200, 150, 100],
    is_event: false,
    is_comment: false,
};

/// Emit particles at a specific position
pub static EMIT_AT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "particles/emit_at",
    display_name: "Emit At Position",
    category: "Particles",
    description: "Emit a single particle at a specific world position",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("y", "Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("z", "Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [200, 150, 100],
    is_event: false,
    is_comment: false,
};

/// Emit multiple particles at a specific position
pub static EMIT_AT_COUNT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "particles/emit_at_count",
    display_name: "Emit Multiple At Position",
    category: "Particles",
    description: "Emit multiple particles at a specific world position",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("y", "Y", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("z", "Z", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("count", "Count", PinType::Int).with_default(PinValue::Int(10)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [200, 150, 100],
    is_event: false,
    is_comment: false,
};
