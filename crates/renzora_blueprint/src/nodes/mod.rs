//! Blueprint node registry — the single source of truth.
//!
//! Each node is **one self-contained unit**: a `BlueprintNodeDef` (pins/meta for
//! the editor) plus its Lua emission (`data` for pure outputs, `exec` for
//! side-effecting statements), registered together in [`REGISTRY`]. The editor
//! palette (`node_def`/`categories`/`nodes_in_category`) and the compiler both
//! read from here. Add a node = add a `NodeEntry` + a `#[cfg(test)]` test that
//! asserts its compiled Lua. Verify with `renzora test -p renzora_blueprint`.
//!
//! This is a from-scratch rebuild: the old 191 hand-written defs + the live
//! interpreter were removed in favour of one compile-to-Lua path. Nodes are
//! re-added here incrementally, each tested.

use crate::compiler::{sanitize_var, strip_quotes, Compiler};
use crate::graph::{BlueprintNodeDef, NodeId, PinTemplate, PinType, PinValue};

/// Emit the Lua expression for a pure node's output `pin`.
pub(crate) type DataFn = for<'a> fn(&mut Compiler<'a>, NodeId, &str) -> String;
/// Emit statements for an exec node and recurse into its exec outputs.
pub(crate) type ExecFn = for<'a> fn(&mut Compiler<'a>, NodeId);

/// A registered node: its editor metadata + how it compiles.
pub(crate) struct NodeEntry {
    pub def: &'static BlueprintNodeDef,
    /// Pure-data emission (`"nil"` default for exec-only nodes).
    pub data: DataFn,
    /// Exec emission (`None` for pure-data nodes).
    pub exec: Option<ExecFn>,
}

/// Default data emission for exec-only nodes.
fn data_none(_c: &mut Compiler, _n: NodeId, _pin: &str) -> String {
    "nil".to_string()
}

// ── colors ───────────────────────────────────────────────────────────────────
const CLR_EVENT: [u8; 3] = [200, 60, 60];
const CLR_MATH: [u8; 3] = [120, 120, 120];
const CLR_TRANSFORM: [u8; 3] = [100, 150, 220];
const CLR_FLOW: [u8; 3] = [140, 140, 160];
const CLR_VARIABLE: [u8; 3] = [60, 180, 120];
const CLR_DEBUG: [u8; 3] = [180, 180, 80];
const CLR_ANIM: [u8; 3] = [80, 200, 180];

// ═════════════════════════════════════════════════════════════════════════════
// Event
// ═════════════════════════════════════════════════════════════════════════════

static ON_READY: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "event/on_ready",
    display_name: "On Ready",
    category: "Event",
    description: "Runs once when the entity initialises",
    pins: || vec![PinTemplate::exec_out("exec", "")],
    color: CLR_EVENT,
};

static ON_UPDATE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "event/on_update",
    display_name: "On Update",
    category: "Event",
    description: "Runs every frame",
    pins: || {
        vec![
            PinTemplate::exec_out("exec", ""),
            PinTemplate::output("delta", "Delta Time", PinType::Float),
            PinTemplate::output("elapsed", "Elapsed", PinType::Float),
        ]
    },
    color: CLR_EVENT,
};

fn on_update_data(_c: &mut Compiler, _n: NodeId, pin: &str) -> String {
    match pin {
        "delta" => "delta".to_string(),
        "elapsed" => "elapsed".to_string(),
        _ => "nil".to_string(),
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Math (pure)
// ═════════════════════════════════════════════════════════════════════════════

static ADD: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/add",
    display_name: "Add",
    category: "Math",
    description: "A + B",
    pins: || {
        vec![
            PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};
fn add_data(c: &mut Compiler, n: NodeId, _pin: &str) -> String {
    format!("({} + {})", c.data(n, "a"), c.data(n, "b"))
}

static MULTIPLY: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/multiply",
    display_name: "Multiply",
    category: "Math",
    description: "A * B",
    pins: || {
        vec![
            PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
            PinTemplate::output("result", "Result", PinType::Float),
        ]
    },
    color: CLR_MATH,
};
fn multiply_data(c: &mut Compiler, n: NodeId, _pin: &str) -> String {
    format!("({} * {})", c.data(n, "a"), c.data(n, "b"))
}

static COMBINE_VEC3: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/combine_vec3",
    display_name: "Combine Vec3",
    category: "Math",
    description: "Create a Vec3 from X, Y, Z",
    pins: || {
        vec![
            PinTemplate::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("y", "Y", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::input("z", "Z", PinType::Float).with_default(PinValue::Float(0.0)),
            PinTemplate::output("result", "Result", PinType::Vec3),
        ]
    },
    color: CLR_MATH,
};
fn combine_vec3_data(c: &mut Compiler, n: NodeId, _pin: &str) -> String {
    format!("vec3({}, {}, {})", c.data(n, "x"), c.data(n, "y"), c.data(n, "z"))
}

// ═════════════════════════════════════════════════════════════════════════════
// Transform (exec)
// ═════════════════════════════════════════════════════════════════════════════

/// Emit `f((v).x or v[1], (v).y or v[2], (v).z or v[3])` for a Vec3 input.
fn emit_vec3_call(c: &mut Compiler, n: NodeId, lua_fn: &str, pin: &str) {
    let v = c.data(n, pin);
    c.emit(&format!(
        "{lua_fn}(({v}).x or {v}[1], ({v}).y or {v}[2], ({v}).z or {v}[3])"
    ));
    c.exec(n, "then");
}

static SET_POSITION: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "transform/set_position",
    display_name: "Set Position",
    category: "Transform",
    description: "Set this entity's position",
    pins: || {
        vec![
            PinTemplate::exec_in("exec", ""),
            PinTemplate::input("position", "Position", PinType::Vec3)
                .with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
            PinTemplate::exec_out("then", ""),
        ]
    },
    color: CLR_TRANSFORM,
};
fn set_position_exec(c: &mut Compiler, n: NodeId) {
    emit_vec3_call(c, n, "set_position", "position");
}

static SET_ROTATION: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "transform/set_rotation",
    display_name: "Set Rotation",
    category: "Transform",
    description: "Set this entity's rotation (euler degrees)",
    pins: || {
        vec![
            PinTemplate::exec_in("exec", ""),
            PinTemplate::input("rotation", "Rotation", PinType::Vec3)
                .with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
            PinTemplate::exec_out("then", ""),
        ]
    },
    color: CLR_TRANSFORM,
};
fn set_rotation_exec(c: &mut Compiler, n: NodeId) {
    emit_vec3_call(c, n, "set_rotation", "rotation");
}

static ROTATE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "transform/rotate",
    display_name: "Rotate",
    category: "Transform",
    // Rate-based + delta-scaled internally so `on_update -> rotate` is a complete,
    // frame-rate-independent spin with no Multiply/Combine nodes. Set the rate
    // right on the node's `degrees` pin (e.g. (0, 90, 0) = 90°/s around Y).
    description: "Rotate continuously at degrees-per-second (delta applied automatically)",
    pins: || {
        vec![
            PinTemplate::exec_in("exec", ""),
            PinTemplate::input("degrees", "Degrees/sec", PinType::Vec3)
                .with_default(PinValue::Vec3([0.0, 90.0, 0.0])),
            PinTemplate::exec_out("then", ""),
        ]
    },
    color: CLR_TRANSFORM,
};
fn rotate_exec(c: &mut Compiler, n: NodeId) {
    // `degrees` is a per-second rate — multiply each axis by `delta` so the
    // result is frame-rate independent without the user wiring a Multiply node.
    let v = c.data(n, "degrees");
    c.emit(&format!(
        "rotate((({v}).x or {v}[1]) * delta, (({v}).y or {v}[2]) * delta, (({v}).z or {v}[3]) * delta)"
    ));
    c.exec(n, "then");
}

// ═════════════════════════════════════════════════════════════════════════════
// Flow
// ═════════════════════════════════════════════════════════════════════════════

static BRANCH: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "flow/branch",
    display_name: "Branch",
    category: "Flow",
    description: "If/else on a boolean condition",
    pins: || {
        vec![
            PinTemplate::exec_in("exec", ""),
            PinTemplate::input("condition", "Condition", PinType::Bool)
                .with_default(PinValue::Bool(true)),
            PinTemplate::exec_out("true", "True"),
            PinTemplate::exec_out("false", "False"),
        ]
    },
    color: CLR_FLOW,
};
fn branch_exec(c: &mut Compiler, n: NodeId) {
    let cond = c.data(n, "condition");
    c.emit(&format!("if {cond} then"));
    c.indent_inc();
    c.exec(n, "true");
    c.indent_dec();
    if c.has_exec(n, "false") {
        c.emit("else");
        c.indent_inc();
        c.exec(n, "false");
        c.indent_dec();
    }
    c.emit("end");
}

// ═════════════════════════════════════════════════════════════════════════════
// Variable
// ═════════════════════════════════════════════════════════════════════════════

static GET_VARIABLE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "variable/get",
    display_name: "Get Variable",
    category: "Variable",
    description: "Read a blueprint variable",
    pins: || {
        vec![
            PinTemplate::input("name", "Name", PinType::String)
                .with_default(PinValue::String("my_var".into())),
            PinTemplate::output("value", "Value", PinType::Any),
        ]
    },
    color: CLR_VARIABLE,
};
fn get_variable_data(c: &mut Compiler, n: NodeId, _pin: &str) -> String {
    sanitize_var(&strip_quotes(&c.inline(n, "name")))
}

static SET_VARIABLE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "variable/set",
    display_name: "Set Variable",
    category: "Variable",
    description: "Write a blueprint variable",
    pins: || {
        vec![
            PinTemplate::exec_in("exec", ""),
            PinTemplate::input("name", "Name", PinType::String)
                .with_default(PinValue::String("my_var".into())),
            PinTemplate::input("value", "Value", PinType::Any),
            PinTemplate::exec_out("then", ""),
        ]
    },
    color: CLR_VARIABLE,
};
fn set_variable_exec(c: &mut Compiler, n: NodeId) {
    let name = sanitize_var(&strip_quotes(&c.inline(n, "name")));
    let value = c.data(n, "value");
    c.emit(&format!("{name} = {value}"));
    c.exec(n, "then");
}

// ═════════════════════════════════════════════════════════════════════════════
// Debug
// ═════════════════════════════════════════════════════════════════════════════

static LOG: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "debug/log",
    display_name: "Log",
    category: "Debug",
    description: "Print a message to the console",
    pins: || {
        vec![
            PinTemplate::exec_in("exec", ""),
            PinTemplate::input("message", "Message", PinType::String)
                .with_default(PinValue::String("Hello!".into())),
            PinTemplate::exec_out("then", ""),
        ]
    },
    color: CLR_DEBUG,
};
fn log_exec(c: &mut Compiler, n: NodeId) {
    let msg = c.data(n, "message");
    c.emit(&format!("print_log(tostring({msg}))"));
    c.exec(n, "then");
}

// ═════════════════════════════════════════════════════════════════════════════
// Animation
// ═════════════════════════════════════════════════════════════════════════════

static CROSSFADE_ANIMATION: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "animation/crossfade",
    display_name: "Crossfade Animation",
    category: "Animation",
    description: "Crossfade to a different animation clip",
    pins: || {
        vec![
            PinTemplate::exec_in("exec", ""),
            PinTemplate::input("name", "Name", PinType::String),
            PinTemplate::input("duration", "Duration", PinType::Float)
                .with_default(PinValue::Float(0.3)),
            PinTemplate::input("looping", "Loop", PinType::Bool).with_default(PinValue::Bool(true)),
            PinTemplate::exec_out("then", ""),
        ]
    },
    color: CLR_ANIM,
};
fn crossfade_exec(c: &mut Compiler, n: NodeId) {
    let name = c.data(n, "name");
    let dur = c.data(n, "duration");
    let looping = c.data(n, "looping");
    c.emit(&format!("crossfade_animation({name}, {dur}, {looping})"));
    c.exec(n, "then");
}

// ═════════════════════════════════════════════════════════════════════════════
// Registry
// ═════════════════════════════════════════════════════════════════════════════

pub(crate) static REGISTRY: &[NodeEntry] = &[
    NodeEntry { def: &ON_READY, data: data_none, exec: None },
    NodeEntry { def: &ON_UPDATE, data: on_update_data, exec: None },
    NodeEntry { def: &ADD, data: add_data, exec: None },
    NodeEntry { def: &MULTIPLY, data: multiply_data, exec: None },
    NodeEntry { def: &COMBINE_VEC3, data: combine_vec3_data, exec: None },
    NodeEntry { def: &SET_POSITION, data: data_none, exec: Some(set_position_exec) },
    NodeEntry { def: &SET_ROTATION, data: data_none, exec: Some(set_rotation_exec) },
    NodeEntry { def: &ROTATE, data: data_none, exec: Some(rotate_exec) },
    NodeEntry { def: &BRANCH, data: data_none, exec: Some(branch_exec) },
    NodeEntry { def: &GET_VARIABLE, data: get_variable_data, exec: None },
    NodeEntry { def: &SET_VARIABLE, data: data_none, exec: Some(set_variable_exec) },
    NodeEntry { def: &LOG, data: data_none, exec: Some(log_exec) },
    NodeEntry { def: &CROSSFADE_ANIMATION, data: data_none, exec: Some(crossfade_exec) },
];

/// Registry entry for a node type.
pub(crate) fn entry(node_type: &str) -> Option<&'static NodeEntry> {
    REGISTRY.iter().find(|e| e.def.node_type == node_type)
}

/// Editor: metadata for a node type.
pub fn node_def(node_type: &str) -> Option<&'static BlueprintNodeDef> {
    entry(node_type).map(|e| e.def)
}

/// Editor: all categories, in first-seen order.
pub fn categories() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for e in REGISTRY {
        if !out.contains(&e.def.category) {
            out.push(e.def.category);
        }
    }
    out
}

/// Editor: node defs in a category.
pub fn nodes_in_category(category: &str) -> Vec<&'static BlueprintNodeDef> {
    REGISTRY
        .iter()
        .filter(|e| e.def.category == category)
        .map(|e| e.def)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_consistent() {
        let mut seen = std::collections::HashSet::new();
        for e in REGISTRY {
            assert!(!e.def.node_type.is_empty());
            assert!(seen.insert(e.def.node_type), "duplicate {}", e.def.node_type);
            // Pins build without panicking.
            let _ = (e.def.pins)();
        }
        assert!(node_def("transform/rotate").is_some());
        assert!(node_def("nope/nope").is_none());
        assert!(categories().contains(&"Transform"));
    }
}
