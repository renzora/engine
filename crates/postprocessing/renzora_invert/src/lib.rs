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
pub struct InvertSettings {
    pub intensity: f32,
    pub _p1: f32,
    pub _p2: f32,
    pub _p3: f32,
    pub _p4: f32,
    pub _p5: f32,
    pub _p6: f32,
    pub enabled: f32,
}

impl Default for InvertSettings {
    fn default() -> Self {
        Self {
            intensity: 1.0,
            _p1: 0.0,
            _p2: 0.0,
            _p3: 0.0,
            _p4: 0.0,
            _p5: 0.0,
            _p6: 0.0,
            enabled: 1.0,
        }
    }
}

impl PostProcessEffect for InvertSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/invert.wgsl".into()
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
        type_id: "invert",
        display_name: "Invert",
        icon: regular::SWAP,
        category: "post_process",
        has_fn: |world, entity| world.get::<InvertSettings>(entity).is_some(),
        add_fn: Some(|world, entity| { world.entity_mut(entity).insert(InvertSettings::default()); }),
        remove_fn: Some(|world, entity| { world.entity_mut(entity).remove::<InvertSettings>(); }),
        is_enabled_fn: Some(|world, entity| world.get::<InvertSettings>(entity).map(|s| s.enabled > 0.5).unwrap_or(false)),
        set_enabled_fn: Some(|world, entity, val| { if let Some(mut s) = world.get_mut::<InvertSettings>(entity) { s.enabled = if val { 1.0 } else { 0.0 }; } }),
        fields: vec![
            FieldDef {
                name: "Intensity",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 1.0 },
                get_fn: |world, entity| world.get::<InvertSettings>(entity).map(|s| FieldValue::Float(s.intensity)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<InvertSettings>(entity) { s.intensity = v; } } },
            },
        ],
        custom_ui_fn: None,
    }
}

pub struct InvertPlugin;

impl Plugin for InvertPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<InvertSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<InvertSettings>::default());
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
