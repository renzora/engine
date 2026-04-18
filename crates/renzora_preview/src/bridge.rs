//! JavaScript ↔ WASM bridge via wasm-bindgen.
//!
//! Communication uses a Resource-based command queue (same pattern as the engine's
//! AnimationCommandQueue / ParticleCommandQueue). JS pushes commands into a global
//! static queue, and a Bevy system drains it each frame into a Bevy Resource.

use bevy::prelude::*;
use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};

// ── Command types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetModeCmd {
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadShaderCmd {
    pub source: String,
    pub shader_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetParamCmd {
    pub name: String,
    pub value: ParamValueJs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadModelCmd {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadAnimationCmd {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadParticleCmd {
    pub definition: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadTextureCmd {
    pub url: String,
    pub texture_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetMeshCmd {
    pub shape: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraOrbitCmd {
    pub azimuth: f32,
    pub elevation: f32,
    pub distance: f32,
}

/// A parameter value from JS — mirrors renzora_shader's ParamValue.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum ParamValueJs {
    Float(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    Color([f32; 4]),
    Int(i32),
    Bool(bool),
}

// ── Unified command enum ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PreviewCommand {
    SetMode(SetModeCmd),
    LoadShader(LoadShaderCmd),
    SetParam(SetParamCmd),
    LoadModel(LoadModelCmd),
    LoadAnimation(LoadAnimationCmd),
    LoadParticle(LoadParticleCmd),
    LoadTexture(LoadTextureCmd),
    SetMesh(SetMeshCmd),
    CameraOrbit(CameraOrbitCmd),
}

// ── Bevy Resource queue (drained by systems each frame) ─────────────────────

#[derive(Resource, Default)]
pub struct PreviewCommandQueue {
    pub commands: Vec<PreviewCommand>,
}

// ── Global static queue (JS → Bevy bridge) ──────────────────────────────────

use std::sync::Mutex;
use std::sync::LazyLock;

static JS_QUEUE: LazyLock<Mutex<Vec<PreviewCommand>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

fn push_command(cmd: PreviewCommand) {
    if let Ok(mut queue) = JS_QUEUE.lock() {
        queue.push(cmd);
    }
}

/// Bevy system: drain the JS static queue into the Bevy Resource queue.
pub fn drain_js_commands(mut queue: ResMut<PreviewCommandQueue>) {
    let commands: Vec<PreviewCommand> = {
        let Ok(mut js_queue) = JS_QUEUE.lock() else { return };
        std::mem::take(&mut *js_queue)
    };
    queue.commands.extend(commands);
}

// ── wasm-bindgen exports ────────────────────────────────────────────────────

#[wasm_bindgen]
pub fn preview_set_mode(mode: &str) {
    push_command(PreviewCommand::SetMode(SetModeCmd {
        mode: mode.to_string(),
    }));
}

#[wasm_bindgen]
pub fn preview_load_shader(source: &str, shader_type: &str) {
    push_command(PreviewCommand::LoadShader(LoadShaderCmd {
        source: source.to_string(),
        shader_type: shader_type.to_string(),
    }));
}

#[wasm_bindgen]
pub fn preview_set_param(name: &str, json_value: &str) {
    if let Ok(value) = serde_json::from_str::<ParamValueJs>(json_value) {
        push_command(PreviewCommand::SetParam(SetParamCmd {
            name: name.to_string(),
            value,
        }));
    }
}

#[wasm_bindgen]
pub fn preview_load_model(url: &str) {
    push_command(PreviewCommand::LoadModel(LoadModelCmd {
        url: url.to_string(),
    }));
}

#[wasm_bindgen]
pub fn preview_load_animation(url: &str) {
    push_command(PreviewCommand::LoadAnimation(LoadAnimationCmd {
        url: url.to_string(),
    }));
}

#[wasm_bindgen]
pub fn preview_load_particle(definition_json: &str) {
    push_command(PreviewCommand::LoadParticle(LoadParticleCmd {
        definition: definition_json.to_string(),
    }));
}

#[wasm_bindgen]
pub fn preview_load_texture(url: &str, texture_type: &str) {
    push_command(PreviewCommand::LoadTexture(LoadTextureCmd {
        url: url.to_string(),
        texture_type: texture_type.to_string(),
    }));
}

#[wasm_bindgen]
pub fn preview_set_mesh(shape: &str) {
    push_command(PreviewCommand::SetMesh(SetMeshCmd {
        shape: shape.to_string(),
    }));
}

#[wasm_bindgen]
pub fn preview_camera_orbit(azimuth: f32, elevation: f32, distance: f32) {
    push_command(PreviewCommand::CameraOrbit(CameraOrbitCmd {
        azimuth,
        elevation,
        distance,
    }));
}

/// Returns the shader params as JSON after loading a shader (for UI generation).
#[wasm_bindgen]
pub fn preview_extract_params(source: &str) -> String {
    let params = renzora_shader::file::extract_params(source);
    serde_json::to_string(&params).unwrap_or_default()
}

// ── Plugin ──────────────────────────────────────────────────────────────────

pub struct BridgePlugin;

impl Plugin for BridgePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PreviewCommandQueue>()
            .add_systems(First, drain_js_commands);
    }
}
