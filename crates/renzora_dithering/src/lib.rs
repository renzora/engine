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
pub struct DitheringSettings {
    pub color_depth: f32,
    pub intensity: f32,
    pub _p1: f32,
    pub _p2: f32,
    pub _p3: f32,
    pub _p4: f32,
    pub _p5: f32,
    pub enabled: f32,
}

impl Default for DitheringSettings {
    fn default() -> Self {
        Self {
            color_depth: 8.0,
            intensity: 1.0,
            _p1: 0.0,
            _p2: 0.0,
            _p3: 0.0,
            _p4: 0.0,
            _p5: 0.0,
            enabled: 1.0,
        }
    }
}

impl PostProcessEffect for DitheringSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/dithering.wgsl".into()
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
        type_id: "dithering",
        display_name: "Dithering",
        icon: regular::GRID_FOUR,
        category: "post_process",
        has_fn: |world, entity| world.get::<DitheringSettings>(entity).is_some(),
        add_fn: Some(|world, entity| { world.entity_mut(entity).insert(DitheringSettings::default()); }),
        remove_fn: Some(|world, entity| { world.entity_mut(entity).remove::<DitheringSettings>(); }),
        is_enabled_fn: Some(|world, entity| world.get::<DitheringSettings>(entity).map(|s| s.enabled > 0.5).unwrap_or(false)),
        set_enabled_fn: Some(|world, entity, val| { if let Some(mut s) = world.get_mut::<DitheringSettings>(entity) { s.enabled = if val { 1.0 } else { 0.0 }; } }),
        fields: vec![
            FieldDef {
                name: "Color Depth",
                field_type: FieldType::Float { speed: 0.5, min: 2.0, max: 32.0 },
                get_fn: |world, entity| world.get::<DitheringSettings>(entity).map(|s| FieldValue::Float(s.color_depth)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<DitheringSettings>(entity) { s.color_depth = v; } } },
            },
            FieldDef {
                name: "Intensity",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 1.0 },
                get_fn: |world, entity| world.get::<DitheringSettings>(entity).map(|s| FieldValue::Float(s.intensity)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<DitheringSettings>(entity) { s.intensity = v; } } },
            },
        ],
    }
}

pub struct DitheringPlugin;

impl Plugin for DitheringPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<DitheringSettings>::default());
        app.init_resource::<InspectorRegistry>();
        let world = app.world_mut();
        if let Some(mut registry) = world.get_resource_mut::<InspectorRegistry>() {
            registry.register(inspector_entry());
        }
    }
}
