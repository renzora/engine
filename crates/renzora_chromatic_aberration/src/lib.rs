use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::prelude::*;
use bevy::render::{
    extract_component::ExtractComponent,
    render_graph::{InternedRenderLabel, InternedRenderSubGraph, RenderLabel, RenderSubGraph},
    render_resource::ShaderType,
};
use bevy::shader::ShaderRef;
use renzora_postprocess::PostProcessEffect;
use egui_phosphor::regular;
use renzora_editor::{FieldDef, FieldType, FieldValue, InspectorEntry, InspectorRegistry};

#[derive(Component, Clone, Copy, ShaderType, ExtractComponent)]
#[extract_component_filter(With<Camera3d>)]
pub struct ChromaticAberrationSettings {
    pub intensity: f32,
    pub samples: f32,
    pub direction_x: f32,
    pub direction_y: f32,
    pub _padding1: f32,
    pub _padding2: f32,
    pub _padding3: f32,
    pub enabled: f32,
}

impl Default for ChromaticAberrationSettings {
    fn default() -> Self {
        Self {
            intensity: 0.005,
            samples: 3.0,
            direction_x: 1.0,
            direction_y: 0.0,
            _padding1: 0.0,
            _padding2: 0.0,
            _padding3: 0.0,
            enabled: 1.0,
        }
    }
}

impl PostProcessEffect for ChromaticAberrationSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/chromatic_aberration.wgsl".into()
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

fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "chromatic_aberration",
        display_name: "Chromatic Aberration",
        icon: regular::RAINBOW,
        category: "post_process",
        has_fn: |world, entity| world.get::<ChromaticAberrationSettings>(entity).is_some(),
        add_fn: Some(|world, entity| { world.entity_mut(entity).insert(ChromaticAberrationSettings::default()); }),
        remove_fn: Some(|world, entity| { world.entity_mut(entity).remove::<ChromaticAberrationSettings>(); }),
        is_enabled_fn: Some(|world, entity| world.get::<ChromaticAberrationSettings>(entity).map(|s| s.enabled > 0.5).unwrap_or(false)),
        set_enabled_fn: Some(|world, entity, val| { if let Some(mut s) = world.get_mut::<ChromaticAberrationSettings>(entity) { s.enabled = if val { 1.0 } else { 0.0 }; } }),
        fields: vec![
            FieldDef {
                name: "Intensity",
                field_type: FieldType::Float { speed: 0.001, min: 0.0, max: 0.1 },
                get_fn: |world, entity| world.get::<ChromaticAberrationSettings>(entity).map(|s| FieldValue::Float(s.intensity)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<ChromaticAberrationSettings>(entity) { s.intensity = v; } } },
            },
            FieldDef {
                name: "Samples",
                field_type: FieldType::Float { speed: 1.0, min: 1.0, max: 16.0 },
                get_fn: |world, entity| world.get::<ChromaticAberrationSettings>(entity).map(|s| FieldValue::Float(s.samples)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<ChromaticAberrationSettings>(entity) { s.samples = v; } } },
            },
        ],
        custom_ui_fn: None,
    }
}

pub struct ChromaticAberrationPlugin;

impl Plugin for ChromaticAberrationPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<ChromaticAberrationSettings>::default());
        app.init_resource::<InspectorRegistry>();
        let world = app.world_mut();
        if let Some(mut registry) = world.get_resource_mut::<InspectorRegistry>() {
            registry.register(inspector_entry());
        }
    }
}
