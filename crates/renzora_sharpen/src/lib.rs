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
pub struct SharpenSettings {
    pub strength: f32,
    pub _padding1: f32,
    pub _padding2: f32,
    pub _padding3: f32,
    pub _padding4: f32,
    pub _padding5: f32,
    pub _padding6: f32,
    pub enabled: f32,
}

impl Default for SharpenSettings {
    fn default() -> Self {
        Self {
            strength: 0.5,
            _padding1: 0.0,
            _padding2: 0.0,
            _padding3: 0.0,
            _padding4: 0.0,
            _padding5: 0.0,
            _padding6: 0.0,
            enabled: 1.0,
        }
    }
}

impl PostProcessEffect for SharpenSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/sharpen.wgsl".into()
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
        type_id: "sharpen",
        display_name: "Sharpen",
        icon: regular::DIAMOND,
        category: "post_process",
        has_fn: |world, entity| world.get::<SharpenSettings>(entity).is_some(),
        add_fn: Some(|world, entity| { world.entity_mut(entity).insert(SharpenSettings::default()); }),
        remove_fn: Some(|world, entity| { world.entity_mut(entity).remove::<SharpenSettings>(); }),
        is_enabled_fn: Some(|world, entity| world.get::<SharpenSettings>(entity).map(|s| s.enabled > 0.5).unwrap_or(false)),
        set_enabled_fn: Some(|world, entity, val| { if let Some(mut s) = world.get_mut::<SharpenSettings>(entity) { s.enabled = if val { 1.0 } else { 0.0 }; } }),
        fields: vec![
            FieldDef {
                name: "Strength",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 3.0 },
                get_fn: |world, entity| world.get::<SharpenSettings>(entity).map(|s| FieldValue::Float(s.strength)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<SharpenSettings>(entity) { s.strength = v; } } },
            },
        ],
        custom_ui_fn: None,
    }
}

pub struct SharpenPlugin;

impl Plugin for SharpenPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<SharpenSettings>::default());
        app.init_resource::<InspectorRegistry>();
        let world = app.world_mut();
        if let Some(mut registry) = world.get_resource_mut::<InspectorRegistry>() {
            registry.register(inspector_entry());
        }
    }
}
