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
pub struct GlitchSettings {
    pub intensity: f32,
    pub block_size: f32,
    pub color_drift: f32,
    pub speed: f32,
    pub _p1: f32,
    pub _p2: f32,
    pub time: f32,
    pub enabled: f32,
}

impl Default for GlitchSettings {
    fn default() -> Self {
        Self {
            intensity: 0.3,
            block_size: 16.0,
            color_drift: 0.01,
            speed: 5.0,
            _p1: 0.0,
            _p2: 0.0,
            time: 0.0,
            enabled: 1.0,
        }
    }
}

impl PostProcessEffect for GlitchSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/glitch.wgsl".into()
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

fn sync_time(time: Res<Time>, mut query: Query<&mut GlitchSettings>) {
    for mut s in query.iter_mut() {
        s.time = time.elapsed_secs();
    }
}

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "glitch",
        display_name: "Glitch",
        icon: regular::LIGHTNING,
        category: "post_process",
        has_fn: |world, entity| world.get::<GlitchSettings>(entity).is_some(),
        add_fn: Some(|world, entity| { world.entity_mut(entity).insert(GlitchSettings::default()); }),
        remove_fn: Some(|world, entity| { world.entity_mut(entity).remove::<GlitchSettings>(); }),
        is_enabled_fn: Some(|world, entity| world.get::<GlitchSettings>(entity).map(|s| s.enabled > 0.5).unwrap_or(false)),
        set_enabled_fn: Some(|world, entity, val| { if let Some(mut s) = world.get_mut::<GlitchSettings>(entity) { s.enabled = if val { 1.0 } else { 0.0 }; } }),
        fields: vec![
            FieldDef {
                name: "Intensity",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 1.0 },
                get_fn: |world, entity| world.get::<GlitchSettings>(entity).map(|s| FieldValue::Float(s.intensity)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<GlitchSettings>(entity) { s.intensity = v; } } },
            },
            FieldDef {
                name: "Block Size",
                field_type: FieldType::Float { speed: 1.0, min: 4.0, max: 64.0 },
                get_fn: |world, entity| world.get::<GlitchSettings>(entity).map(|s| FieldValue::Float(s.block_size)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<GlitchSettings>(entity) { s.block_size = v; } } },
            },
            FieldDef {
                name: "Color Drift",
                field_type: FieldType::Float { speed: 0.001, min: 0.0, max: 0.1 },
                get_fn: |world, entity| world.get::<GlitchSettings>(entity).map(|s| FieldValue::Float(s.color_drift)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<GlitchSettings>(entity) { s.color_drift = v; } } },
            },
            FieldDef {
                name: "Speed",
                field_type: FieldType::Float { speed: 0.1, min: 0.1, max: 20.0 },
                get_fn: |world, entity| world.get::<GlitchSettings>(entity).map(|s| FieldValue::Float(s.speed)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<GlitchSettings>(entity) { s.speed = v; } } },
            },
        ],
        custom_ui_fn: None,
    }
}

pub struct GlitchPlugin;

impl Plugin for GlitchPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<GlitchSettings>();
        app.add_plugins(
            renzora_postprocess::PostProcessPlugin::<GlitchSettings>::default(),
        );
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
