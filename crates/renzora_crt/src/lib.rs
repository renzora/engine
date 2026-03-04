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
pub struct CrtSettings {
    pub scanline_intensity: f32,
    pub curvature: f32,
    pub chromatic_amount: f32,
    pub vignette_amount: f32,
    pub _padding0: f32,
    pub _padding1: f32,
    pub _padding2: f32,
    pub enabled: f32,
}

impl Default for CrtSettings {
    fn default() -> Self {
        Self {
            scanline_intensity: 0.3,
            curvature: 0.02,
            chromatic_amount: 0.003,
            vignette_amount: 0.5,
            _padding0: 0.0,
            _padding1: 0.0,
            _padding2: 0.0,
            enabled: 1.0,
        }
    }
}

impl PostProcessEffect for CrtSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/crt.wgsl".into()
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
        type_id: "crt",
        display_name: "CRT",
        icon: regular::MONITOR,
        category: "post_process",
        has_fn: |world, entity| world.get::<CrtSettings>(entity).is_some(),
        add_fn: Some(|world, entity| { world.entity_mut(entity).insert(CrtSettings::default()); }),
        remove_fn: Some(|world, entity| { world.entity_mut(entity).remove::<CrtSettings>(); }),
        is_enabled_fn: Some(|world, entity| world.get::<CrtSettings>(entity).map(|s| s.enabled > 0.5).unwrap_or(false)),
        set_enabled_fn: Some(|world, entity, val| { if let Some(mut s) = world.get_mut::<CrtSettings>(entity) { s.enabled = if val { 1.0 } else { 0.0 }; } }),
        fields: vec![
            FieldDef {
                name: "Scanline Intensity",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 2.0 },
                get_fn: |world, entity| world.get::<CrtSettings>(entity).map(|s| FieldValue::Float(s.scanline_intensity)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<CrtSettings>(entity) { s.scanline_intensity = v; } } },
            },
            FieldDef {
                name: "Curvature",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 1.0 },
                get_fn: |world, entity| world.get::<CrtSettings>(entity).map(|s| FieldValue::Float(s.curvature)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<CrtSettings>(entity) { s.curvature = v; } } },
            },
            FieldDef {
                name: "Chromatic Amount",
                field_type: FieldType::Float { speed: 0.001, min: 0.0, max: 0.1 },
                get_fn: |world, entity| world.get::<CrtSettings>(entity).map(|s| FieldValue::Float(s.chromatic_amount)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<CrtSettings>(entity) { s.chromatic_amount = v; } } },
            },
            FieldDef {
                name: "Vignette Amount",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 2.0 },
                get_fn: |world, entity| world.get::<CrtSettings>(entity).map(|s| FieldValue::Float(s.vignette_amount)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<CrtSettings>(entity) { s.vignette_amount = v; } } },
            },
        ],
    }
}

pub struct CrtPlugin;

impl Plugin for CrtPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(
            renzora_postprocess::PostProcessPlugin::<CrtSettings>::default(),
        );
        app.init_resource::<InspectorRegistry>();
        let world = app.world_mut();
        if let Some(mut registry) = world.get_resource_mut::<InspectorRegistry>() {
            registry.register(inspector_entry());
        }
    }
}
