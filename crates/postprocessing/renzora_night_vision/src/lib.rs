use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::prelude::*;
use serde::{Serialize, Deserialize};
use bevy::render::{
    extract_component::ExtractComponent,
    render_graph::{InternedRenderLabel, InternedRenderSubGraph, RenderLabel, RenderSubGraph},
    render_resource::ShaderType,
};
use bevy::shader::ShaderRef;
use renzora_postprocess::PostProcessEffect;
#[cfg(feature = "editor")]
use egui_phosphor::regular;
#[cfg(feature = "editor")]
use renzora_editor::{FieldDef, FieldType, FieldValue, InspectorEntry, InspectorRegistry};

#[derive(Component, Clone, Copy, Reflect, Serialize, Deserialize, ShaderType, ExtractComponent)]
#[reflect(Component, Serialize, Deserialize)]
#[extract_component_filter(With<Camera3d>)]
pub struct NightVisionSettings {
    pub intensity: f32,
    pub noise_amount: f32,
    pub scanline_amount: f32,
    pub color_amplification: f32,
    pub _p1: f32,
    pub _p2: f32,
    pub time: f32,
    pub enabled: f32,
}

impl Default for NightVisionSettings {
    fn default() -> Self {
        Self {
            intensity: 1.0,
            noise_amount: 0.15,
            scanline_amount: 0.3,
            color_amplification: 3.0,
            _p1: 0.0,
            _p2: 0.0,
            time: 0.0,
            enabled: 1.0,
        }
    }
}

impl PostProcessEffect for NightVisionSettings {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_night_vision/night_vision.wgsl".into()
    }
    fn sub_graph() -> Option<InternedRenderSubGraph> {
        Some(Core3d.intern())
    }
    fn node_edges() -> Vec<InternedRenderLabel> {
        vec![
            Node3d::Tonemapping.intern(),
            Self::node_label().intern(),
            Node3d::EndMainPassPostProcessing.intern(),
        ]
    }
}

fn sync_time(time: Res<Time>, mut query: Query<&mut NightVisionSettings>) {
    for mut s in query.iter_mut() {
        s.time = time.elapsed_secs();
    }
}

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "night_vision",
        display_name: "Night Vision",
        icon: regular::BINOCULARS,
        category: "post_process",
        has_fn: |world, entity| world.get::<NightVisionSettings>(entity).is_some(),
        add_fn: Some(|world, entity| { world.entity_mut(entity).insert(NightVisionSettings::default()); }),
        remove_fn: Some(|world, entity| { world.entity_mut(entity).remove::<NightVisionSettings>(); }),
        is_enabled_fn: Some(|world, entity| world.get::<NightVisionSettings>(entity).map(|s| s.enabled > 0.5).unwrap_or(false)),
        set_enabled_fn: Some(|world, entity, val| { if let Some(mut s) = world.get_mut::<NightVisionSettings>(entity) { s.enabled = if val { 1.0 } else { 0.0 }; } }),
        fields: vec![
            FieldDef {
                name: "Intensity",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 1.0 },
                get_fn: |world, entity| world.get::<NightVisionSettings>(entity).map(|s| FieldValue::Float(s.intensity)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<NightVisionSettings>(entity) { s.intensity = v; } } },
            },
            FieldDef {
                name: "Noise",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 1.0 },
                get_fn: |world, entity| world.get::<NightVisionSettings>(entity).map(|s| FieldValue::Float(s.noise_amount)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<NightVisionSettings>(entity) { s.noise_amount = v; } } },
            },
            FieldDef {
                name: "Scanlines",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 1.0 },
                get_fn: |world, entity| world.get::<NightVisionSettings>(entity).map(|s| FieldValue::Float(s.scanline_amount)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<NightVisionSettings>(entity) { s.scanline_amount = v; } } },
            },
            FieldDef {
                name: "Color Amplification",
                field_type: FieldType::Float { speed: 0.05, min: 1.0, max: 10.0 },
                get_fn: |world, entity| world.get::<NightVisionSettings>(entity).map(|s| FieldValue::Float(s.color_amplification)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<NightVisionSettings>(entity) { s.color_amplification = v; } } },
            },
        ],
        custom_ui_fn: None,
    }
}

pub struct NightVisionPlugin;

impl Plugin for NightVisionPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "night_vision.wgsl");
        app.register_type::<NightVisionSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<NightVisionSettings>::default());
        app.add_systems(Update, sync_time);
        #[cfg(feature = "editor")]
        {
            app.init_resource::<InspectorRegistry>();
        let world = app.world_mut();
        if let Some(mut registry) = world.get_resource_mut::<InspectorRegistry>() {
            registry.register(inspector_entry());
        }
        }
    }
}
