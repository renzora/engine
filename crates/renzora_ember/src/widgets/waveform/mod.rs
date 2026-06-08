//! Waveform — a GPU-painted symmetric audio envelope (via [`UiMaterial`]) for
//! audio clips / sound preview.

use bevy::asset::Asset;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial};
use bevy::ui_render::UiMaterialPlugin;

use crate::theme::*;

const MAX_SAMPLES: usize = 32;

pub(crate) struct WaveformPlugin;

impl Plugin for WaveformPlugin {
    fn build(&self, app: &mut App) {
        // Idempotent — both `WidgetsPlugin` and the markup `vector::plugin` add
        // it; re-adding the same `UiMaterialPlugin` panics (see `GaugePlugin`).
        if app.is_plugin_added::<UiMaterialPlugin<WaveMaterial>>() {
            return;
        }
        bevy::asset::embedded_asset!(app, "waveform.wgsl");
        app.add_plugins(UiMaterialPlugin::<WaveMaterial>::default());
        app.add_systems(Update, (waveform_attach, waveform_sync));
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct WaveMaterial {
    #[uniform(0)]
    data: [Vec4; 8],
    #[uniform(0)]
    color: Vec4,
    #[uniform(0)]
    params: Vec4,
}

impl UiMaterial for WaveMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_ember/widgets/waveform/waveform.wgsl".into()
    }
}

#[derive(Component)]
pub(crate) struct WaveData {
    amps: Vec<f32>,
}

impl WaveData {
    /// Build from amplitudes (0..1) — used by the markup `vector="wave"` bridge.
    pub(crate) fn new(amps: Vec<f32>) -> Self {
        Self { amps }
    }

    /// Replace the amplitudes in place (live markup `vector="wave"` sync).
    pub(crate) fn set_amps(&mut self, amps: Vec<f32>) {
        self.amps = amps;
    }
}

/// Pack amplitudes (0..1) into the `WaveMaterial` uniforms, or `None` if there
/// aren't enough samples to draw. The accent color is used.
fn wave_material(amps: &[f32]) -> Option<WaveMaterial> {
    if amps.len() < 2 {
        return None;
    }
    let n = amps.len().min(MAX_SAMPLES);
    let mut flat = [0.0f32; MAX_SAMPLES];
    for (i, v) in amps.iter().take(MAX_SAMPLES).enumerate() {
        flat[i] = v.clamp(0.0, 1.0);
    }
    let mut data = [Vec4::ZERO; 8];
    for (g, slot) in data.iter_mut().enumerate() {
        *slot = Vec4::new(flat[g * 4], flat[g * 4 + 1], flat[g * 4 + 2], flat[g * 4 + 3]);
    }
    let accent = rgb(accent()).to_linear();
    Some(WaveMaterial {
        data,
        color: Vec4::new(accent.red, accent.green, accent.blue, 1.0),
        params: Vec4::new(n as f32, 0.0, 0.0, 0.0),
    })
}

/// A waveform preview of `amps` (amplitudes 0..1).
pub fn waveform(commands: &mut Commands, amps: &[f32]) -> Entity {
    let outer = commands
        .spawn((
            Node {
                width: Val::Px(240.0),
                height: Val::Px(56.0),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            BorderColor::all(rgb(border())),
            Name::new("waveform"),
        ))
        .id();
    let plot = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            WaveData {
                amps: amps.to_vec(),
            },
            Pickable::IGNORE,
            Name::new("waveform-plot"),
        ))
        .id();
    commands.entity(outer).add_child(plot);
    outer
}

fn waveform_attach(
    mut commands: Commands,
    mut materials: ResMut<Assets<WaveMaterial>>,
    waves: Query<(Entity, &WaveData), Without<MaterialNode<WaveMaterial>>>,
) {
    for (e, wd) in &waves {
        if let Some(material) = wave_material(&wd.amps) {
            // try_insert: the waveform entity may be despawned this same frame (panel teardown).
            commands
                .entity(e)
                .try_insert(MaterialNode(materials.add(material)));
        }
    }
}

/// Update an already-attached waveform's material when its `WaveData` changes
/// (the markup `vector="wave"` bridge re-binds `{{ }}` data each frame).
fn waveform_sync(
    mut materials: ResMut<Assets<WaveMaterial>>,
    waves: Query<(&WaveData, &MaterialNode<WaveMaterial>), Changed<WaveData>>,
) {
    for (wd, node) in &waves {
        if let Some(material) = wave_material(&wd.amps) {
            if let Some(slot) = materials.get_mut(&node.0) {
                *slot = material;
            }
        }
    }
}
