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
pub struct RadialBlurSettings {
    pub intensity: f32,
    pub center_x: f32,
    pub center_y: f32,
    pub samples: f32,
    pub _p1: f32,
    pub _p2: f32,
    pub _p3: f32,
    pub enabled: f32,
}

impl Default for RadialBlurSettings {
    fn default() -> Self {
        Self {
            intensity: 0.02,
            center_x: 0.5,
            center_y: 0.5,
            samples: 8.0,
            _p1: 0.0,
            _p2: 0.0,
            _p3: 0.0,
            enabled: 1.0,
        }
    }
}

impl PostProcessEffect for RadialBlurSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/radial_blur.wgsl".into()
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
        type_id: "radial_blur",
        display_name: "Radial Blur",
        icon: regular::CIRCLE_DASHED,
        category: "post_process",
        has_fn: |world, entity| world.get::<RadialBlurSettings>(entity).is_some(),
        add_fn: Some(|world, entity| { world.entity_mut(entity).insert(RadialBlurSettings::default()); }),
        remove_fn: Some(|world, entity| { world.entity_mut(entity).remove::<RadialBlurSettings>(); }),
        is_enabled_fn: Some(|world, entity| world.get::<RadialBlurSettings>(entity).map(|s| s.enabled > 0.5).unwrap_or(false)),
        set_enabled_fn: Some(|world, entity, val| { if let Some(mut s) = world.get_mut::<RadialBlurSettings>(entity) { s.enabled = if val { 1.0 } else { 0.0 }; } }),
        fields: vec![
            FieldDef {
                name: "Intensity",
                field_type: FieldType::Float { speed: 0.001, min: 0.0, max: 0.2 },
                get_fn: |world, entity| world.get::<RadialBlurSettings>(entity).map(|s| FieldValue::Float(s.intensity)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<RadialBlurSettings>(entity) { s.intensity = v; } } },
            },
            FieldDef {
                name: "Center X",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 1.0 },
                get_fn: |world, entity| world.get::<RadialBlurSettings>(entity).map(|s| FieldValue::Float(s.center_x)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<RadialBlurSettings>(entity) { s.center_x = v; } } },
            },
            FieldDef {
                name: "Center Y",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 1.0 },
                get_fn: |world, entity| world.get::<RadialBlurSettings>(entity).map(|s| FieldValue::Float(s.center_y)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<RadialBlurSettings>(entity) { s.center_y = v; } } },
            },
            FieldDef {
                name: "Samples",
                field_type: FieldType::Float { speed: 1.0, min: 4.0, max: 32.0 },
                get_fn: |world, entity| world.get::<RadialBlurSettings>(entity).map(|s| FieldValue::Float(s.samples)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<RadialBlurSettings>(entity) { s.samples = v; } } },
            },
        ],
    }
}

pub struct RadialBlurPlugin;

impl Plugin for RadialBlurPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(
            renzora_postprocess::PostProcessPlugin::<RadialBlurSettings>::default(),
        );
        app.init_resource::<InspectorRegistry>();
        let world = app.world_mut();
        if let Some(mut registry) = world.get_resource_mut::<InspectorRegistry>() {
            registry.register(inspector_entry());
        }
    }
}
