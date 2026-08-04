//! The engine side of the scripting boundary.
//!
//! [`PluginScriptBackend`] implements the same [`ScriptBackend`] trait the
//! in-tree interpreter does, so [`ScriptEngine`](crate::engine::ScriptEngine)
//! routes to a language plugin exactly the way it routes to anything else —
//! by file extension, with no idea that a `dlopen` happened.
//!
//! ## What stays on this side
//!
//! File I/O, and that is deliberate. The plugin is handed source text and a
//! version number; it never opens a path. Exported and Android builds read
//! scripts out of an rpak archive through a closure this crate owns, and a
//! plugin doing its own `std::fs` would work in the editor and fail in every
//! shipped game — the worst possible place for that difference to show up.
//! Hot-reload detection stays here too, for the same reason.
//!
//! ## Frame encoding happens once
//!
//! The context splits in two: the half that is the same for every scripted
//! entity and the half that is not. The frame half is re-encoded only when
//! `time.frame_count` moves, and every call in a frame carries the same
//! `frame_seq` so the plugin can skip re-decoding it as well.
//!
//! ## Host calls need no context pointer
//!
//! `get`, `get_component` and the rest read thread-locals that
//! [`crate::get_handler`] already sets around script execution — the mechanism
//! the in-tree backend uses. So the callbacks below are plain functions and the
//! `ctx` field is null: there is nothing for it to carry that is not already
//! reachable.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use renzora_plugin::script::{
    encode_bindings, ChildNode, EntityContext, FrameContext, GamepadSnapshot, HookArgs,
    RaycastHit, ScriptReply, ScriptTime, ScriptValue as WireValue, VarDef, Writer,
};
use renzora_plugin::sys;

use crate::backend::{FileReader, ScriptBackend};
use crate::command::{to_wire_action, to_wire_prop, ScriptCommand};
use crate::component::{ScriptValue, ScriptVariableDefinition, ScriptVariables};
use crate::context::ScriptContext;

/// A scripting language provided by a loaded plugin.
pub struct PluginScriptBackend {
    name: String,
    extensions: Vec<String>,
    /// Borrowed extension list as `&str`, because [`ScriptBackend::extensions`]
    /// returns `&[&str]` and there is nowhere to build one per call.
    extension_refs: Vec<&'static str>,
    entry: sys::ScriptEntry,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    scripts_folder: Option<PathBuf>,
    file_reader: Option<FileReader>,
    sources: HashMap<PathBuf, Source>,
    /// Encoded [`FrameContext`], rebuilt when the frame moves on.
    frame: Vec<u8>,
    frame_seq: u64,
    /// `false` until the first frame is encoded — frame 0 is a real count.
    has_frame: bool,
    /// Which `ScriptExtensions` generation the plugin has been told about.
    bindings_generation: Option<u64>,
}

struct Source {
    text: String,
    /// Bumped on every reload, so the plugin drops its compiled VM.
    version: u64,
    modified: Option<std::time::SystemTime>,
}

/// Compile a `.blueprint`/`.bp` graph to Lua source; pass anything else through.
///
/// Host-side rather than in the language plugin, for a hard reason and a soft
/// one. The hard one: `renzora_blueprint` links Bevy, so it cannot cross into a
/// standalone plugin at all. The soft one: the host already decides *what text*
/// a backend receives, having just read the file, so this is the same decision
/// and belongs in the same place.
///
/// A parse failure becomes a top-level `error(...)` so it surfaces in the
/// console rather than failing silently.
#[cfg(feature = "blueprint")]
fn compile_blueprint(path: &Path, source: String) -> String {
    if !is_blueprint(path) {
        return source;
    }
    match serde_json::from_str::<renzora_blueprint::graph::BlueprintGraph>(&source) {
        Ok(graph) => renzora_blueprint::compiler::compile_to_lua(&graph),
        Err(e) => {
            let msg = e.to_string().replace(['\'', '\n'], " ");
            log::warn!(
                "[scripting] blueprint '{}' failed to parse: {}",
                path.display(),
                msg
            );
            format!("error('blueprint parse failed: {msg}')")
        }
    }
}

/// Blueprint support stripped from this build (lean export, `blueprint` off).
/// A graph cannot be compiled, so say so rather than feeding JSON to an
/// interpreter that will report a confusing syntax error.
#[cfg(not(feature = "blueprint"))]
fn compile_blueprint(path: &Path, source: String) -> String {
    if is_blueprint(path) {
        "error('blueprint support is not included in this build')".to_string()
    } else {
        source
    }
}

fn is_blueprint(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e == "blueprint" || e == "bp")
        .unwrap_or(false)
}

/// Collects the plugin's reply.
///
/// # Safety
/// `ctx` must be a live `*mut Vec<u8>`.
unsafe extern "C" fn collect(ctx: *mut std::ffi::c_void, bytes: *const u8, len: usize) {
    let buf = &mut *(ctx as *mut Vec<u8>);
    buf.extend_from_slice(std::slice::from_raw_parts(bytes, len));
}

/// Write an encoded value back to the plugin.
///
/// # Safety
/// `sink` must be the plugin-provided sink for this call.
unsafe fn reply(sink: *const sys::ByteSink, w: &Writer) {
    let Some(sink) = sink.as_ref() else { return };
    let bytes = w.bytes();
    (sink.write)(sink.ctx, bytes.as_ptr(), bytes.len());
}

/// A null pointer means "the script's own entity".
///
/// # Safety
/// `s` must be a valid `StrRef` or have a null pointer.
unsafe fn opt_str<'a>(s: sys::StrRef) -> Option<&'a str> {
    if s.ptr.is_null() {
        None
    } else {
        Some(s.as_str())
    }
}

/// # Safety
/// Called only by a plugin, during a call this crate made.
unsafe extern "C" fn host_get(
    _ctx: *mut std::ffi::c_void,
    entity: sys::StrRef,
    component: sys::StrRef,
    field: sys::StrRef,
    out: *const sys::ByteSink,
) {
    let value = crate::get_handler::call_get(
        opt_str(entity),
        component.as_str(),
        field.as_str(),
    );
    let mut w = Writer::new();
    match value {
        Some(v) => {
            w.bool(true);
            to_wire_prop(v).encode(&mut w);
        }
        None => w.bool(false),
    }
    reply(out, &w);
}

/// # Safety
/// Called only by a plugin, during a call this crate made.
unsafe extern "C" fn host_get_component(
    _ctx: *mut std::ffi::c_void,
    entity: sys::StrRef,
    component: sys::StrRef,
    out: *const sys::ByteSink,
) {
    let fields = crate::get_handler::call_get_component(opt_str(entity), component.as_str());
    let mut w = Writer::new();
    match fields {
        Some(map) => {
            w.bool(true);
            w.count(map.len());
            for (k, v) in map {
                w.str(&k);
                to_wire_prop(v).encode(&mut w);
            }
        }
        None => w.bool(false),
    }
    reply(out, &w);
}

/// # Safety
/// Called only by a plugin, during a call this crate made.
unsafe extern "C" fn host_get_components(
    _ctx: *mut std::ffi::c_void,
    entity: sys::StrRef,
    out: *const sys::ByteSink,
) {
    let names = crate::get_handler::call_get_components(opt_str(entity));
    let mut w = Writer::new();
    w.count(names.len());
    for n in names {
        w.str(&n);
    }
    reply(out, &w);
}

/// # Safety
/// Called only by a plugin, during a call this crate made.
unsafe extern "C" fn host_asset_progress(
    _ctx: *mut std::ffi::c_void,
    out: *const sys::ByteSink,
) {
    let mut w = Writer::new();
    match crate::get_handler::call_asset_progress() {
        Some(p) => {
            w.bool(true);
            renzora_plugin::script::AssetProgress {
                state: p.state.to_string(),
                total_files: p.total_files,
                loaded_files: p.loaded_files,
                total_bytes: p.total_bytes,
                loaded_bytes: p.loaded_bytes,
                current_path: p.current_path,
                elapsed_secs: p.elapsed_secs,
                fraction: p.fraction,
            }
            .encode(&mut w);
        }
        None => w.bool(false),
    }
    reply(out, &w);
}

/// # Safety
/// Called only by a plugin, during a call this crate made.
unsafe extern "C" fn host_translate(
    _ctx: *mut std::ffi::c_void,
    key: sys::StrRef,
    out: *const sys::ByteSink,
) {
    let mut w = Writer::new();
    w.str(&renzora::lang::t(key.as_str()));
    reply(out, &w);
}

/// The read-back table handed to every call.
///
/// A `const` bound to a local rather than a `static`, because the struct holds
/// a raw pointer and so is not `Sync`. Blanket-impl'ing `Sync` on it would be a
/// claim about every use of the type, including a plugin that puts real state
/// behind `ctx`; six pointers built per call costs nothing and claims nothing.
const HOST_CALLS: sys::ScriptHostCalls = sys::ScriptHostCalls {
    ctx: std::ptr::null_mut(),
    get: host_get,
    get_component: host_get_component,
    get_components: host_get_components,
    asset_progress: host_asset_progress,
    translate: host_translate,
};

fn str_ref(s: &str) -> sys::StrRef {
    sys::StrRef {
        ptr: s.as_ptr(),
        len: s.len(),
    }
}

impl PluginScriptBackend {
    /// Adopt a backend a plugin registered.
    ///
    /// Extensions are leaked into `&'static str` because
    /// [`ScriptBackend::extensions`] returns a borrowed slice of borrowed
    /// strings and there is nowhere to hang a shorter lifetime. A handful of
    /// four-byte strings, once per loaded language plugin, for the life of the
    /// process — set against changing a trait every other backend implements.
    pub fn new(name: String, extensions: Vec<String>, entry: sys::ScriptEntry) -> Self {
        let extension_refs = extensions
            .iter()
            .map(|e| &*Box::leak(e.clone().into_boxed_str()))
            .collect();
        Self {
            name,
            extensions,
            extension_refs,
            entry,
            state: Mutex::new(State::default()),
        }
    }

    /// Read a script, preferring the VFS reader an exported build installs.
    fn read_source(state: &State, path: &Path) -> Option<String> {
        let text = match &state.file_reader {
            Some(reader) => reader(path).or_else(|| std::fs::read_to_string(path).ok()),
            None => std::fs::read_to_string(path).ok(),
        }?;
        Some(compile_blueprint(path, text))
    }

    /// Make sure `path` is cached, returning its source and version.
    fn ensure_source(state: &mut State, path: &Path) -> Result<(String, u64), String> {
        if let Some(s) = state.sources.get(path) {
            return Ok((s.text.clone(), s.version));
        }
        let text = Self::read_source(state, path)
            .ok_or_else(|| format!("could not read {}", path.display()))?;
        let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        state.sources.insert(
            path.to_path_buf(),
            Source {
                text: text.clone(),
                version: 1,
                modified,
            },
        );
        Ok((text, 1))
    }

    /// Encode the frame half if the frame moved on, and return its sequence.
    fn frame_bytes(state: &mut State, ctx: &ScriptContext) -> u64 {
        let seq = ctx.time.frame_count;
        if state.has_frame && state.frame_seq == seq {
            return seq;
        }
        let mut w = Writer::new();
        frame_context(ctx).encode(&mut w);
        state.frame = w.into_bytes();
        state.frame_seq = seq;
        state.has_frame = true;
        seq
    }

    /// Tell the plugin about the declared bindings, if it has not been told.
    fn sync_bindings(&self, state: &mut State, ctx: &ScriptContext) {
        let Some(exts) = ctx.extensions() else { return };
        let generation = exts.generation();
        if state.bindings_generation == Some(generation) {
            return;
        }
        let mut w = Writer::new();
        encode_bindings(&mut w, exts.bindings());
        let args = w.into_bytes();
        let host_calls = HOST_CALLS;
        let call = sys::ScriptCall {
            op: sys::ScriptOp::Bindings,
            _pad: 0,
            path: str_ref(""),
            source: str_ref(""),
            version: 0,
            entity: 0,
            frame: sys::BlobRef::EMPTY,
            frame_seq: 0,
            entity_ctx: sys::BlobRef::EMPTY,
            args: sys::BlobRef::new(&args),
            vars: sys::BlobRef::EMPTY,
            out: std::ptr::null(),
            host: &host_calls,
        };
        // SAFETY: every blob outlives the call; `out` is null, which the
        // dispatcher checks before writing.
        let status = unsafe { (self.entry)(&call) };
        if status == sys::ScriptStatus::Ok || status == sys::ScriptStatus::UnknownOp {
            state.bindings_generation = Some(generation);
        } else {
            log::warn!(
                "[scripting] `{}` refused the binding list (status {})",
                self.name,
                status.0
            );
        }
    }

    /// Invoke one hook and decode the reply.
    fn call(
        &self,
        op: sys::ScriptOp,
        path: &Path,
        args: &HookArgs,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<ScriptReply, String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        self.sync_bindings(&mut state, ctx);
        // Evicting a script the plugin never loaded is a no-op, not an error —
        // the entity may have been despawned before its script ever ran.
        let (source, version) = if op == sys::ScriptOp::Evict {
            Self::ensure_source(&mut state, path).unwrap_or_default()
        } else {
            Self::ensure_source(&mut state, path)?
        };
        let frame_seq = Self::frame_bytes(&mut state, ctx);

        let mut w = Writer::new();
        entity_context(ctx).encode(&mut w);
        let entity_bytes = w.into_bytes();

        let mut w = Writer::new();
        args.encode(&mut w);
        let arg_bytes = w.into_bytes();

        let mut w = Writer::new();
        w.count(vars.iter_all().count());
        for (k, v) in vars.iter_all() {
            w.str(k);
            to_wire_value(v).encode(&mut w);
        }
        let var_bytes = w.into_bytes();

        let path_str = path.to_string_lossy().into_owned();
        let mut out: Vec<u8> = Vec::new();
        let sink = sys::ByteSink {
            ctx: &mut out as *mut Vec<u8> as *mut std::ffi::c_void,
            write: collect,
        };

        let host_calls = HOST_CALLS;
        let call = sys::ScriptCall {
            op,
            _pad: 0,
            path: str_ref(&path_str),
            source: str_ref(&source),
            version,
            entity: ctx.self_entity_id,
            frame: sys::BlobRef::new(&state.frame),
            frame_seq,
            entity_ctx: sys::BlobRef::new(&entity_bytes),
            args: sys::BlobRef::new(&arg_bytes),
            vars: sys::BlobRef::new(&var_bytes),
            out: &sink,
            host: &host_calls,
        };

        // SAFETY: every blob and string above outlives this call, and the sink
        // writes into `out`, which does too.
        let status = unsafe { (self.entry)(&call) };
        // Nothing else may touch `state` while the plugin holds pointers into
        // it, so release only now.
        drop(state);

        if !status.is_known() {
            return Err(format!(
                "{} returned status {} — it was built against a newer engine",
                self.name, status.0
            ));
        }
        // A hook the script does not define, or an op this backend has never
        // heard of, are both "nothing to do" rather than failures.
        if status == sys::ScriptStatus::NoHook || status == sys::ScriptStatus::UnknownOp {
            return Ok(ScriptReply::default());
        }

        let mut r = renzora_plugin::script::Reader::new(&out);
        let decoded = ScriptReply::decode(&mut r)
            .map_err(|e| format!("{} sent a reply that would not decode: {e}", self.name))?;

        if let Some(err) = decoded.error.clone() {
            // Props still come back on an error reply, so apply what did arrive
            // before reporting — a script with a syntax error should still show
            // whatever the inspector managed to read.
            write_back(vars, &decoded);
            return Err(err);
        }
        write_back(vars, &decoded);
        Ok(decoded)
    }
}

/// Copy the script's prop values back out of a reply.
fn write_back(vars: &mut ScriptVariables, reply: &ScriptReply) {
    for (k, v) in &reply.vars {
        vars.set(k.clone(), from_wire_value(v));
    }
}

fn to_wire_value(v: &ScriptValue) -> WireValue {
    match v {
        ScriptValue::Float(f) => WireValue::Float(*f),
        ScriptValue::Int(i) => WireValue::Int(*i),
        ScriptValue::Bool(b) => WireValue::Bool(*b),
        ScriptValue::String(s) => WireValue::String(s.clone()),
        ScriptValue::Entity(s) => WireValue::Entity(s.clone()),
        ScriptValue::Vec2(v) => WireValue::Vec2(v.to_array()),
        ScriptValue::Vec3(v) => WireValue::Vec3(v.to_array()),
        ScriptValue::Color(v) => WireValue::Color(v.to_array()),
    }
}

fn from_wire_value(v: &WireValue) -> ScriptValue {
    use bevy::prelude::{Vec2, Vec3, Vec4};
    match v {
        WireValue::Float(f) => ScriptValue::Float(*f),
        WireValue::Int(i) => ScriptValue::Int(*i),
        WireValue::Bool(b) => ScriptValue::Bool(*b),
        WireValue::String(s) => ScriptValue::String(s.clone()),
        WireValue::Entity(s) => ScriptValue::Entity(s.clone()),
        WireValue::Vec2(v) => ScriptValue::Vec2(Vec2::from(*v)),
        WireValue::Vec3(v) => ScriptValue::Vec3(Vec3::from(*v)),
        WireValue::Color(v) => ScriptValue::Color(Vec4::from(*v)),
    }
}

fn to_var_def(v: &VarDef) -> ScriptVariableDefinition {
    ScriptVariableDefinition {
        name: v.name.clone(),
        display_name: v.display_name.clone(),
        default_value: from_wire_value(&v.default_value),
        hint: v.hint.clone(),
        tab: v.tab.clone(),
    }
}

/// The pressed entries of a sparse `name -> bool` table.
fn set_of(map: &HashMap<String, bool>) -> Vec<String> {
    map.iter()
        .filter(|(_, v)| **v)
        .map(|(k, _)| k.clone())
        .collect()
}

fn frame_context(ctx: &ScriptContext) -> FrameContext {
    FrameContext {
        time: ScriptTime {
            elapsed: ctx.time.elapsed,
            delta: ctx.time.delta,
            fixed_delta: ctx.time.fixed_delta,
            frame_count: ctx.time.frame_count,
        },
        input_movement: ctx.input_movement.to_array(),
        mouse_position: ctx.mouse_position.to_array(),
        mouse_delta: ctx.mouse_delta.to_array(),
        mouse_scroll: ctx.mouse_scroll,
        camera_yaw: ctx.camera_yaw,
        keys_pressed: set_of(&ctx.keys_pressed),
        keys_just_pressed: set_of(&ctx.keys_just_pressed),
        keys_just_released: set_of(&ctx.keys_just_released),
        mouse_buttons_pressed: ctx.mouse_buttons_pressed,
        mouse_buttons_just_pressed: ctx.mouse_buttons_just_pressed,
        camera_ev: ctx.camera_ev,
        project_width: ctx.project_width,
        project_height: ctx.project_height,
        net_is_server: ctx.net_is_server,
        net_is_connected: ctx.net_is_connected,
        net_player_count: ctx.net_player_count,
        gamepads: ctx
            .gamepads
            .iter()
            .map(|g| GamepadSnapshot {
                id: g.id,
                left_stick: g.left_stick.to_array(),
                right_stick: g.right_stick.to_array(),
                left_trigger: g.left_trigger,
                right_trigger: g.right_trigger,
                buttons: g.buttons,
                buttons_just_pressed: g.buttons_just_pressed,
            })
            .collect(),
        actions_pressed: set_of(&ctx.action_pressed),
        actions_just_pressed: set_of(&ctx.action_just_pressed),
        actions_just_released: set_of(&ctx.action_just_released),
        action_axis_1d: ctx
            .action_axis_1d
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect(),
        action_axis_2d: ctx
            .action_axis_2d
            .iter()
            .map(|(k, v)| (k.clone(), v.to_array()))
            .collect(),
        named_entities: ctx
            .found_entities
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect(),
        timers_just_finished: ctx.timers_just_finished.clone(),
    }
}

fn entity_context(ctx: &ScriptContext) -> EntityContext {
    EntityContext {
        entity_id: ctx.self_entity_id,
        name: ctx.self_entity_name.clone(),
        position: ctx.transform.position.to_array(),
        rotation: ctx.transform.rotation.to_array(),
        rotation_euler: ctx.transform.euler_degrees().to_array(),
        scale: ctx.transform.scale.to_array(),
        has_parent: ctx.has_parent,
        parent_entity: ctx.parent_entity.map(|e| e.to_bits()),
        parent_position: ctx.parent_position.to_array(),
        parent_rotation: ctx.parent_rotation.to_array(),
        parent_scale: ctx.parent_scale.to_array(),
        children: ctx
            .children
            .iter()
            .map(|c| ChildNode {
                entity_id: c.entity.to_bits(),
                name: c.name.clone(),
                position: c.position.to_array(),
                rotation: c.rotation.to_array(),
                scale: c.scale.to_array(),
            })
            .collect(),
        collisions_entered: ctx.collisions_entered.clone(),
        collisions_exited: ctx.collisions_exited.clone(),
        active_collisions: ctx.active_collisions.clone(),
        raycast_results: ctx
            .raycast_results
            .iter()
            .map(|(k, h)| {
                (
                    k.clone(),
                    RaycastHit {
                        hit: h.hit,
                        entity_id: h.entity.map(|e| e.to_bits()),
                        point: h.point.to_array(),
                        normal: h.normal.to_array(),
                        distance: h.distance,
                    },
                )
            })
            .collect(),
        health: ctx.self_health,
        max_health: ctx.self_max_health,
        health_percent: ctx.self_health_percent,
        is_invincible: ctx.self_is_invincible,
        light_intensity: ctx.self_light_intensity,
        light_color: ctx.self_light_color,
        material_color: ctx.self_material_color,
    }
}

/// Convert an inbound hook's engine-typed arguments to the boundary's.
fn wire_args(
    args: &HashMap<String, renzora::ScriptActionValue>,
) -> Vec<(String, renzora_plugin::script::ActionValue)> {
    args.iter()
        .map(|(k, v)| (k.clone(), to_wire_action(v.clone())))
        .collect()
}

impl ScriptBackend for PluginScriptBackend {
    fn name(&self) -> &str {
        &self.name
    }

    fn extensions(&self) -> &[&str] {
        &self.extension_refs
    }

    fn set_scripts_folder(&mut self, path: PathBuf) {
        if let Ok(mut s) = self.state.lock() {
            s.scripts_folder = Some(path);
        }
    }

    fn set_file_reader(&mut self, reader: FileReader) {
        if let Ok(mut s) = self.state.lock() {
            s.file_reader = Some(reader);
        }
    }

    fn get_available_scripts(&self) -> Vec<(String, PathBuf)> {
        let Ok(state) = self.state.lock() else {
            return Vec::new();
        };
        let Some(folder) = &state.scripts_folder else {
            return Vec::new();
        };
        let Ok(entries) = std::fs::read_dir(folder) else {
            return Vec::new();
        };
        let mut out: Vec<(String, PathBuf)> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| self.extensions.iter().any(|x| x.eq_ignore_ascii_case(e)))
                    .unwrap_or(false)
            })
            .map(|p| {
                let name = p
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("script")
                    .to_string();
                (name, p)
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    fn get_script_props(&self, path: &Path) -> Vec<ScriptVariableDefinition> {
        // Props are read outside the per-entity loop, where there is no context
        // to hand over — a bare one is enough, since parsing a declaration
        // cannot depend on the world.
        let mut ctx = ScriptContext::new(Default::default(), Default::default());
        let mut vars = ScriptVariables::default();
        match self.call(
            sys::ScriptOp::Props,
            path,
            &HookArgs::None,
            &mut ctx,
            &mut vars,
        ) {
            Ok(reply) => reply.props.iter().map(to_var_def).collect(),
            Err(e) => {
                log::warn!("[scripting] props for {}: {e}", path.display());
                Vec::new()
            }
        }
    }

    fn call_on_ready(
        &self,
        path: &Path,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<Vec<ScriptCommand>, String> {
        Ok(self
            .call(sys::ScriptOp::OnReady, path, &HookArgs::None, ctx, vars)?
            .commands)
    }

    fn call_on_update(
        &self,
        path: &Path,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<Vec<ScriptCommand>, String> {
        Ok(self
            .call(sys::ScriptOp::OnUpdate, path, &HookArgs::None, ctx, vars)?
            .commands)
    }

    fn call_on_rpc(
        &self,
        path: &Path,
        rpc_name: &str,
        args: &HashMap<String, renzora::ScriptActionValue>,
        from: u64,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<Vec<ScriptCommand>, String> {
        let hook = HookArgs::Rpc {
            name: rpc_name.to_string(),
            from,
            args: wire_args(args),
        };
        Ok(self
            .call(sys::ScriptOp::OnRpc, path, &hook, ctx, vars)?
            .commands)
    }

    fn call_on_ui(
        &self,
        path: &Path,
        name: &str,
        args: &HashMap<String, renzora::ScriptActionValue>,
        entity_bits: u64,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<Vec<ScriptCommand>, String> {
        let hook = HookArgs::Ui {
            name: name.to_string(),
            entity_bits,
            args: wire_args(args),
        };
        Ok(self
            .call(sys::ScriptOp::OnUi, path, &hook, ctx, vars)?
            .commands)
    }

    fn call_on_draw(
        &self,
        path: &Path,
        width: f32,
        height: f32,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<Vec<renzora::DrawCmd>, String> {
        let hook = HookArgs::Draw { width, height };
        let reply = self.call(sys::ScriptOp::OnDraw, path, &hook, ctx, vars)?;
        Ok(reply.draws.iter().map(to_engine_draw).collect())
    }

    fn call_on_animation_event(
        &self,
        path: &Path,
        name: &str,
        entity_bits: u64,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<Vec<ScriptCommand>, String> {
        let hook = HookArgs::AnimationEvent {
            name: name.to_string(),
            entity_bits,
        };
        Ok(self
            .call(sys::ScriptOp::OnAnimationEvent, path, &hook, ctx, vars)?
            .commands)
    }

    fn call_on_http(
        &self,
        path: &Path,
        callback: &str,
        status: u16,
        body: &str,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<Vec<ScriptCommand>, String> {
        let hook = HookArgs::Http {
            callback: callback.to_string(),
            status,
            body: body.to_string(),
        };
        Ok(self
            .call(sys::ScriptOp::OnHttp, path, &hook, ctx, vars)?
            .commands)
    }

    fn call_on_player_event(
        &self,
        path: &Path,
        id: u64,
        joined: bool,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<Vec<ScriptCommand>, String> {
        let hook = HookArgs::PlayerEvent { id, joined };
        Ok(self
            .call(sys::ScriptOp::OnPlayerEvent, path, &hook, ctx, vars)?
            .commands)
    }

    fn needs_reload(&self, path: &Path) -> bool {
        let Ok(state) = self.state.lock() else {
            return false;
        };
        // Unknown means never loaded, which the next call handles anyway —
        // reporting `true` here would make the editor announce a hot reload for
        // a script that simply had not run yet.
        let Some(cached) = state.sources.get(path) else {
            return false;
        };
        let Some(known) = cached.modified else {
            return false;
        };
        std::fs::metadata(path)
            .and_then(|m| m.modified())
            .map(|now| now > known)
            .unwrap_or(false)
    }

    fn reload(&self, path: &Path) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        let text = Self::read_source(&state, path)
            .ok_or_else(|| format!("could not read {}", path.display()))?;
        let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        let version = state.sources.get(path).map(|s| s.version + 1).unwrap_or(1);
        state.sources.insert(
            path.to_path_buf(),
            Source {
                text,
                version,
                modified,
            },
        );
        Ok(())
    }

    fn evict(&self, path: &Path, entity: u64) {
        let mut ctx = ScriptContext::new(Default::default(), Default::default());
        ctx.self_entity_id = entity;
        let mut vars = ScriptVariables::default();
        // Eviction is best-effort housekeeping: if the plugin does not
        // implement the op, or the script was never loaded, there is nothing to
        // report and nothing the caller could do about it.
        let _ = self.call(sys::ScriptOp::Evict, path, &HookArgs::None, &mut ctx, &mut vars);
        if let Ok(mut state) = self.state.lock() {
            if !path.as_os_str().is_empty() {
                state.sources.remove(path);
            }
        }
    }

    fn eval_expression(&self, expr: &str) -> Result<String, String> {
        let mut ctx = ScriptContext::new(Default::default(), Default::default());
        let mut vars = ScriptVariables::default();
        let hook = HookArgs::Eval {
            expr: expr.to_string(),
        };
        // `Eval` needs no script, but `call` reads a source file for every op.
        // Rather than special-case the path, hand it an empty in-memory entry.
        {
            let mut state = self.state.lock().map_err(|e| e.to_string())?;
            state.sources.entry(PathBuf::from("<eval>")).or_insert(Source {
                text: String::new(),
                version: 1,
                modified: None,
            });
        }
        let reply = self.call(
            sys::ScriptOp::Eval,
            Path::new("<eval>"),
            &hook,
            &mut ctx,
            &mut vars,
        )?;
        Ok(reply.text.unwrap_or_default())
    }
}

fn to_engine_draw(d: &renzora_plugin::script::DrawCmd) -> renzora::DrawCmd {
    use renzora_plugin::script::DrawCmd as W;
    match d {
        W::Line { x1, y1, x2, y2, color, thickness } => renzora::DrawCmd::Line {
            x1: *x1,
            y1: *y1,
            x2: *x2,
            y2: *y2,
            color: *color,
            thickness: *thickness,
        },
        W::Arc { cx, cy, r, start, end, color, thickness } => renzora::DrawCmd::Arc {
            cx: *cx,
            cy: *cy,
            r: *r,
            start: *start,
            end: *end,
            color: *color,
            thickness: *thickness,
        },
        W::Circle { cx, cy, r, color } => renzora::DrawCmd::Circle {
            cx: *cx,
            cy: *cy,
            r: *r,
            color: *color,
        },
        W::Rect { x, y, w, h, color } => renzora::DrawCmd::Rect {
            x: *x,
            y: *y,
            w: *w,
            h: *h,
            color: *color,
        },
        W::Triangle { x1, y1, x2, y2, x3, y3, color } => renzora::DrawCmd::Triangle {
            x1: *x1,
            y1: *y1,
            x2: *x2,
            y2: *y2,
            x3: *x3,
            y3: *y3,
            color: *color,
        },
        W::Text { x, y, text, size, color } => renzora::DrawCmd::Text {
            x: *x,
            y: *y,
            text: text.clone(),
            size: *size,
            color: *color,
        },
    }
}

/// Adopt every backend registered by a loaded plugin.
///
/// Runs once, after plugin loading — `PluginScriptBackends` is filled during
/// `renzora_plugin_init`, which happens well before the first frame.
pub fn adopt_plugin_backends(
    mut registered: bevy::prelude::ResMut<renzora_plugin::host::PluginScriptBackends>,
    mut engine: bevy::prelude::ResMut<crate::engine::ScriptEngine>,
) {
    if registered.0.is_empty() {
        return;
    }
    for b in registered.0.drain(..) {
        bevy::log::info!(
            "[scripting] adopting `{}` for .{}",
            b.name,
            b.extensions.join(", .")
        );
        engine.add_backend(Box::new(PluginScriptBackend::new(
            b.name,
            b.extensions,
            b.entry,
        )));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sparse_key_table_encodes_only_the_pressed_keys() {
        let mut map = HashMap::new();
        map.insert("w".to_string(), true);
        map.insert("a".to_string(), false);
        map.insert("s".to_string(), true);

        let mut set = set_of(&map);
        set.sort();
        assert_eq!(set, ["s", "w"]);
    }

    #[test]
    fn script_values_survive_both_conversions() {
        use bevy::prelude::{Vec2, Vec3, Vec4};
        for v in [
            ScriptValue::Float(1.5),
            ScriptValue::Int(-3),
            ScriptValue::Bool(true),
            ScriptValue::String("hi".into()),
            ScriptValue::Entity("Player".into()),
            ScriptValue::Vec2(Vec2::new(1.0, 2.0)),
            ScriptValue::Vec3(Vec3::new(1.0, 2.0, 3.0)),
            ScriptValue::Color(Vec4::new(1.0, 2.0, 3.0, 4.0)),
        ] {
            let back = from_wire_value(&to_wire_value(&v));
            assert_eq!(
                std::mem::discriminant(&back),
                std::mem::discriminant(&v),
                "{v:?} changed variant crossing the boundary"
            );
        }
    }

    #[test]
    fn a_reply_writes_prop_values_back_into_the_component() {
        let mut vars = ScriptVariables::default();
        let reply = ScriptReply {
            vars: vec![
                ("speed".into(), WireValue::Float(9.0)),
                ("name".into(), WireValue::String("bob".into())),
            ],
            ..Default::default()
        };
        write_back(&mut vars, &reply);
        assert_eq!(vars.get_float("speed"), Some(9.0));
        assert_eq!(vars.get_string("name"), Some("bob"));
    }

    #[test]
    fn every_draw_command_maps_to_its_engine_twin() {
        use renzora_plugin::script::DrawCmd as W;
        let c = [1.0, 0.5, 0.25, 1.0];
        let cases = [
            W::Line { x1: 1.0, y1: 2.0, x2: 3.0, y2: 4.0, color: c, thickness: 2.0 },
            W::Arc { cx: 1.0, cy: 2.0, r: 3.0, start: 0.0, end: 90.0, color: c, thickness: 1.0 },
            W::Circle { cx: 1.0, cy: 2.0, r: 3.0, color: c },
            W::Rect { x: 1.0, y: 2.0, w: 3.0, h: 4.0, color: c },
            W::Triangle { x1: 1.0, y1: 2.0, x2: 3.0, y2: 4.0, x3: 5.0, y3: 6.0, color: c },
            W::Text { x: 1.0, y: 2.0, text: "hi".into(), size: 16.0, color: c },
        ];
        // Discriminant order matches by construction; check they all convert and
        // keep their colour, which is the field most easily transposed.
        for d in &cases {
            let e = to_engine_draw(d);
            let colour = match e {
                renzora::DrawCmd::Line { color, .. }
                | renzora::DrawCmd::Arc { color, .. }
                | renzora::DrawCmd::Circle { color, .. }
                | renzora::DrawCmd::Rect { color, .. }
                | renzora::DrawCmd::Triangle { color, .. }
                | renzora::DrawCmd::Text { color, .. } => color,
            };
            assert_eq!(colour, c);
        }
    }
}
