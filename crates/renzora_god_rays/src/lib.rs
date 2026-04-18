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
use renzora_editor_framework::{AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry};

#[derive(Component, Clone, Copy, Reflect, Serialize, Deserialize, ShaderType, ExtractComponent)]
#[reflect(Component, Serialize, Deserialize)]
#[extract_component_filter(With<Camera3d>)]
pub struct GodRaysSettings {
    pub intensity: f32,
    pub decay: f32,
    pub density: f32,
    pub num_samples: u32,
    pub light_pos_x: f32,
    pub light_pos_y: f32,
    pub _padding1: f32,
    pub enabled: f32,
}

impl Default for GodRaysSettings {
    fn default() -> Self {
        Self {
            intensity: 0.5,
            decay: 0.97,
            density: 1.0,
            num_samples: 64,
            light_pos_x: 0.5,
            light_pos_y: 0.3,
            _padding1: 0.0,
            enabled: 1.0,
        }
    }
}

impl PostProcessEffect for GodRaysSettings {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_god_rays/god_rays.wgsl".into()
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

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "god_rays",
        display_name: "God Rays",
        icon: regular::SUN_HORIZON,
        category: "post_process",
        has_fn: |world, entity| world.get::<GodRaysSettings>(entity).is_some(),
        add_fn: Some(|world, entity| { world.entity_mut(entity).insert(GodRaysSettings::default()); }),
        remove_fn: Some(|world, entity| { world.entity_mut(entity).remove::<GodRaysSettings>(); }),
        is_enabled_fn: Some(|world, entity| world.get::<GodRaysSettings>(entity).map(|s| s.enabled > 0.5).unwrap_or(false)),
        set_enabled_fn: Some(|world, entity, val| { if let Some(mut s) = world.get_mut::<GodRaysSettings>(entity) { s.enabled = if val { 1.0 } else { 0.0 }; } }),
        fields: vec![
            FieldDef {
                name: "Intensity",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 2.0 },
                get_fn: |world, entity| world.get::<GodRaysSettings>(entity).map(|s| FieldValue::Float(s.intensity)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<GodRaysSettings>(entity) { s.intensity = v; } } },
            },
            FieldDef {
                name: "Decay",
                field_type: FieldType::Float { speed: 0.001, min: 0.9, max: 1.0 },
                get_fn: |world, entity| world.get::<GodRaysSettings>(entity).map(|s| FieldValue::Float(s.decay)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<GodRaysSettings>(entity) { s.decay = v; } } },
            },
            FieldDef {
                name: "Density",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 2.0 },
                get_fn: |world, entity| world.get::<GodRaysSettings>(entity).map(|s| FieldValue::Float(s.density)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<GodRaysSettings>(entity) { s.density = v; } } },
            },
            FieldDef {
                name: "Num Samples",
                field_type: FieldType::Float { speed: 1.0, min: 1.0, max: 256.0 },
                get_fn: |world, entity| world.get::<GodRaysSettings>(entity).map(|s| FieldValue::Float(s.num_samples as f32)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<GodRaysSettings>(entity) { s.num_samples = v as u32; } } },
            },
            FieldDef {
                name: "Light Pos X",
                field_type: FieldType::Float { speed: 0.01, min: -1.0, max: 2.0 },
                get_fn: |world, entity| world.get::<GodRaysSettings>(entity).map(|s| FieldValue::Float(s.light_pos_x)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<GodRaysSettings>(entity) { s.light_pos_x = v; } } },
            },
            FieldDef {
                name: "Light Pos Y",
                field_type: FieldType::Float { speed: 0.01, min: -1.0, max: 2.0 },
                get_fn: |world, entity| world.get::<GodRaysSettings>(entity).map(|s| FieldValue::Float(s.light_pos_y)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<GodRaysSettings>(entity) { s.light_pos_y = v; } } },
            },
        ],
        custom_ui_fn: None,
    }
}

#[derive(Default)]
pub struct GodRaysPlugin;

impl Plugin for GodRaysPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] GodRaysPlugin");
        bevy::asset::embedded_asset!(app, "god_rays.wgsl");
        app.register_type::<GodRaysSettings>();
        app.add_plugins(
            renzora_postprocess::PostProcessPlugin::<GodRaysSettings>::default(),
        );
        #[cfg(feature = "editor")]
        app.register_inspector(inspector_entry());
    }
}

renzora::add!(GodRaysPlugin);
