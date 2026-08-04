//! End-to-end checks against the real interpreter.
//!
//! These run the whole plugin half — build a VM, set the context globals, call
//! the hook, collect the reply — with no host, no GPU and no `dlopen`. That is
//! the point: when a script misbehaves in the editor, this says within seconds
//! whether the interpreter is at fault or the engine side is.

use renzora_plugin::script::*;
use renzora_plugin::sys;

use crate::interp::LuaBackend;

// ── A stub host: every read answers "nothing" ────────────────────────────────

unsafe fn emit(out: *const sys::ByteSink, w: &Writer) {
    let Some(sink) = out.as_ref() else { return };
    let b = w.bytes();
    (sink.write)(sink.ctx, b.as_ptr(), b.len());
}

unsafe fn write_false(out: *const sys::ByteSink) {
    let mut w = Writer::new();
    w.bool(false);
    emit(out, &w);
}

unsafe extern "C" fn no_value(
    _c: *mut std::ffi::c_void,
    _e: sys::StrRef,
    _co: sys::StrRef,
    _f: sys::StrRef,
    out: *const sys::ByteSink,
) {
    write_false(out);
}

unsafe extern "C" fn no_component(
    _c: *mut std::ffi::c_void,
    _e: sys::StrRef,
    _co: sys::StrRef,
    out: *const sys::ByteSink,
) {
    write_false(out);
}

unsafe extern "C" fn no_components(
    _c: *mut std::ffi::c_void,
    _e: sys::StrRef,
    out: *const sys::ByteSink,
) {
    let mut w = Writer::new();
    w.count(0);
    emit(out, &w);
}

unsafe extern "C" fn no_progress(_c: *mut std::ffi::c_void, out: *const sys::ByteSink) {
    write_false(out);
}

unsafe extern "C" fn echo_key(
    _c: *mut std::ffi::c_void,
    key: sys::StrRef,
    out: *const sys::ByteSink,
) {
    let mut w = Writer::new();
    w.str(key.as_str());
    emit(out, &w);
}

fn stub_host() -> sys::ScriptHostCalls {
    sys::ScriptHostCalls {
        ctx: std::ptr::null_mut(),
        get: no_value,
        get_component: no_component,
        get_components: no_components,
        asset_progress: no_progress,
        translate: echo_key,
    }
}

fn frame_at(delta: f32, count: u64) -> FrameContext {
    FrameContext {
        time: ScriptTime {
            elapsed: count as f64 * delta as f64,
            delta,
            fixed_delta: delta,
            frame_count: count,
        },
        ..Default::default()
    }
}

/// Run one hook and return the reply.
fn run(
    backend: &mut LuaBackend,
    source: &str,
    vars: &[(String, ScriptValue)],
    hook: Hook,
    frame: &FrameContext,
) -> Result<ScriptReply, String> {
    let entity = EntityContext {
        entity_id: 1,
        name: "Cube".into(),
        rotation: [0.0, 0.0, 0.0, 1.0],
        scale: [1.0, 1.0, 1.0],
        ..Default::default()
    };
    let raw = stub_host();
    let ctx = Ctx {
        frame,
        entity: &entity,
        host: HostCalls::new(&raw),
    };
    let script = ScriptRef {
        path: "scripts/t.lua",
        source,
        version: 1,
        entity: 1,
        vars,
    };
    let mut reply = ScriptReply::default();
    backend.hook(&script, hook, &ctx, &mut reply)?;
    Ok(reply)
}

/// **The question this file was written to answer.** A script that assigns a
/// declared prop must hand the new value back, or the inspector shows a frozen
/// number all through play mode.
#[test]
fn a_prop_the_script_writes_comes_back_out() {
    let mut backend = LuaBackend::default();
    let src = "function on_update() _time = _time + 1.0 end";
    let vars = vec![("_time".to_string(), ScriptValue::Float(5.0))];

    let reply = run(&mut backend, src, &vars, Hook::Update, &frame_at(0.5, 1)).unwrap();

    assert_eq!(
        reply.vars,
        vec![("_time".to_string(), ScriptValue::Float(6.0))],
        "the script set _time = 6.0; that must reach the host"
    );
}

/// A bool prop is the other half of the day/night pattern (`_init`), and bools
/// take a different arm in both conversions.
#[test]
fn a_bool_prop_round_trips() {
    let mut backend = LuaBackend::default();
    let src = "function on_update() if not _init then _init = true end end";
    let vars = vec![("_init".to_string(), ScriptValue::Bool(false))];

    let reply = run(&mut backend, src, &vars, Hook::Update, &frame_at(0.5, 1)).unwrap();
    assert_eq!(reply.vars, vec![("_init".to_string(), ScriptValue::Bool(true))]);
}

/// Props are read *in* as well as out — a script must see the inspector's value.
#[test]
fn a_prop_the_inspector_set_reaches_the_script() {
    let mut backend = LuaBackend::default();
    let src = "function on_update() set_position(speed, 0.0, 0.0) end";
    let vars = vec![("speed".to_string(), ScriptValue::Float(42.0))];

    let reply = run(&mut backend, src, &vars, Hook::Update, &frame_at(0.5, 1)).unwrap();
    assert_eq!(
        reply.commands,
        vec![ScriptCommand::SetPosition {
            x: 42.0,
            y: 0.0,
            z: 0.0
        }]
    );
}

const ROTATE: &str = r#"
function props()
    return {
        speed  = { value = 90.0 },
        axis_x = { value = 0.0 },
        axis_y = { value = 1.0 },
        axis_z = { value = 0.0 },
    }
end

function on_ready()
    _angle = 0.0
end

function on_update()
    _angle = (_angle or 0.0) + speed * delta
    _angle = _angle % 360.0
    set_rotation(_angle * axis_x, _angle * axis_y, _angle * axis_z)
end
"#;

fn rotate_vars() -> Vec<(String, ScriptValue)> {
    vec![
        ("speed".into(), ScriptValue::Float(90.0)),
        ("axis_x".into(), ScriptValue::Float(0.0)),
        ("axis_y".into(), ScriptValue::Float(1.0)),
        ("axis_z".into(), ScriptValue::Float(0.0)),
    ]
}

#[test]
fn props_are_parsed_from_the_script() {
    let mut backend = LuaBackend::default();
    let script = ScriptRef {
        path: "scripts/rotate.lua",
        source: ROTATE,
        version: 1,
        entity: 1,
        vars: &[],
    };
    let names: Vec<String> = backend.props(&script).iter().map(|p| p.name.clone()).collect();
    assert_eq!(names, ["axis_x", "axis_y", "axis_z", "speed"]);
}

#[test]
fn rotate_emits_a_rotation() {
    let mut backend = LuaBackend::default();
    let reply = run(
        &mut backend,
        ROTATE,
        &rotate_vars(),
        Hook::Update,
        &frame_at(0.5, 1),
    )
    .expect("hook failed");

    // 90 deg/s for half a second = 45 degrees, about Y only.
    assert_eq!(
        reply.commands,
        vec![ScriptCommand::SetRotation {
            x: 0.0,
            y: 45.0,
            z: 0.0
        }],
        "got {:?}",
        reply.commands
    );
}

/// `_angle` is a plain VM global, not a prop, so it survives only if the VM is
/// reused between frames. If this fails the cube sits at a fixed small angle
/// instead of spinning — which looks exactly like "not moving".
#[test]
fn the_angle_accumulates_across_frames() {
    let mut backend = LuaBackend::default();
    let vars = rotate_vars();
    for (i, expected) in [(1u64, 45.0f32), (2, 90.0), (3, 135.0)] {
        let reply = run(&mut backend, ROTATE, &vars, Hook::Update, &frame_at(0.5, i)).unwrap();
        match reply.commands.as_slice() {
            [ScriptCommand::SetRotation { y, .. }] => assert!(
                (*y - expected).abs() < 0.001,
                "frame {i}: expected {expected}, got {y}"
            ),
            other => panic!("frame {i}: unexpected {other:?}"),
        }
    }
}

#[test]
fn context_globals_reach_the_script() {
    let mut backend = LuaBackend::default();
    let src = "function on_update() set_position(delta, elapsed, position_x) end";
    let reply = run(&mut backend, src, &[], Hook::Update, &frame_at(0.25, 4)).unwrap();
    match reply.commands.as_slice() {
        [ScriptCommand::SetPosition { x, y, z }] => {
            assert!((*x - 0.25).abs() < 1e-6, "delta was {x}");
            assert!((*y - 1.0).abs() < 1e-6, "elapsed was {y}");
            assert!((*z - 0.0).abs() < 1e-6, "position_x was {z}");
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn a_missing_hook_is_not_an_error() {
    let mut backend = LuaBackend::default();
    let reply = run(
        &mut backend,
        ROTATE,
        &rotate_vars(),
        Hook::Draw {
            width: 800.0,
            height: 600.0,
        },
        &frame_at(0.016, 1),
    )
    .expect("a missing hook must not be an error");
    assert!(reply.draws.is_empty());
}

#[test]
fn a_script_error_is_reported_rather_than_swallowed() {
    let mut backend = LuaBackend::default();
    let err = run(
        &mut backend,
        "function on_update() error('boom') end",
        &[],
        Hook::Update,
        &frame_at(0.016, 1),
    )
    .unwrap_err();
    assert!(err.contains("boom"), "unhelpful error: {err}");
}

/// `set(...)` must target the script's own entity — no name, no lookup. This is
/// what lets a script drive a component on the entity it is attached to.
#[test]
fn set_targets_the_scripts_own_entity() {
    let mut backend = LuaBackend::default();
    let src = r#"function on_update() set("Sun.elevation", 12.5) end"#;
    let reply = run(&mut backend, src, &[], Hook::Update, &frame_at(0.016, 1)).unwrap();
    assert_eq!(
        reply.commands,
        vec![ScriptCommand::SetComponentField {
            entity_id: None,
            entity_name: None,
            component_type: "Sun".into(),
            field_path: "elevation".into(),
            value: PropValue::Float(12.5),
        }],
        "an empty entity_name/entity_id is what the engine reads as 'self'"
    );
}

#[test]
fn a_declared_action_binding_becomes_a_function() {
    let mut backend = LuaBackend::default();
    backend.set_bindings(&[Binding {
        name: "apply_force".into(),
        kind: BindingKind::Action {
            action: "apply_force".into(),
        },
        params: vec![
            Param { name: "x".into(), kind: ParamKind::Float },
            Param { name: "y".into(), kind: ParamKind::Float },
            Param { name: "z".into(), kind: ParamKind::Float },
        ],
        doc: String::new(),
    }]);

    let reply = run(
        &mut backend,
        "function on_update() apply_force(1.0, 2.0, 3.0) end",
        &[],
        Hook::Update,
        &frame_at(0.016, 1),
    )
    .unwrap();

    assert_eq!(
        reply.commands,
        vec![ScriptCommand::Action {
            name: "apply_force".into(),
            target_entity: None,
            args: vec![
                ("x".into(), ActionValue::Float(1.0)),
                ("y".into(), ActionValue::Float(2.0)),
                ("z".into(), ActionValue::Float(3.0)),
            ],
        }]
    );
}
