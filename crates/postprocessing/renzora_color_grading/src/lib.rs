use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::prelude::*;
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

#[derive(Component, Clone, Copy, ShaderType, ExtractComponent)]
#[extract_component_filter(With<Camera3d>)]
pub struct ColorGradingSettings {
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub gamma: f32,
    pub temperature: f32,
    pub tint: f32,
    pub _padding1: f32,
    pub enabled: f32,
}

impl Default for ColorGradingSettings {
    fn default() -> Self {
        Self {
            brightness: 1.0,
            contrast: 1.0,
            saturation: 1.0,
            gamma: 1.0,
            temperature: 0.0,
            tint: 0.0,
            _padding1: 0.0,
            enabled: 1.0,
        }
    }
}

impl PostProcessEffect for ColorGradingSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/color_grading.wgsl".into()
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
        type_id: "color_grading",
        display_name: "Color Grading",
        icon: regular::PALETTE,
        category: "post_process",
        has_fn: |world, entity| world.get::<ColorGradingSettings>(entity).is_some(),
        add_fn: Some(|world, entity| { world.entity_mut(entity).insert(ColorGradingSettings::default()); }),
        remove_fn: Some(|world, entity| { world.entity_mut(entity).remove::<ColorGradingSettings>(); }),
        is_enabled_fn: Some(|world, entity| world.get::<ColorGradingSettings>(entity).map(|s| s.enabled > 0.5).unwrap_or(false)),
        set_enabled_fn: Some(|world, entity, val| { if let Some(mut s) = world.get_mut::<ColorGradingSettings>(entity) { s.enabled = if val { 1.0 } else { 0.0 }; } }),
        fields: vec![
            FieldDef {
                name: "Brightness",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 3.0 },
                get_fn: |world, entity| world.get::<ColorGradingSettings>(entity).map(|s| FieldValue::Float(s.brightness)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<ColorGradingSettings>(entity) { s.brightness = v; } } },
            },
            FieldDef {
                name: "Contrast",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 3.0 },
                get_fn: |world, entity| world.get::<ColorGradingSettings>(entity).map(|s| FieldValue::Float(s.contrast)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<ColorGradingSettings>(entity) { s.contrast = v; } } },
            },
            FieldDef {
                name: "Saturation",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 3.0 },
                get_fn: |world, entity| world.get::<ColorGradingSettings>(entity).map(|s| FieldValue::Float(s.saturation)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<ColorGradingSettings>(entity) { s.saturation = v; } } },
            },
            FieldDef {
                name: "Gamma",
                field_type: FieldType::Float { speed: 0.01, min: 0.1, max: 3.0 },
                get_fn: |world, entity| world.get::<ColorGradingSettings>(entity).map(|s| FieldValue::Float(s.gamma)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<ColorGradingSettings>(entity) { s.gamma = v; } } },
            },
            FieldDef {
                name: "Temperature",
                field_type: FieldType::Float { speed: 0.01, min: -1.0, max: 1.0 },
                get_fn: |world, entity| world.get::<ColorGradingSettings>(entity).map(|s| FieldValue::Float(s.temperature)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<ColorGradingSettings>(entity) { s.temperature = v; } } },
            },
            FieldDef {
                name: "Tint",
                field_type: FieldType::Float { speed: 0.01, min: -1.0, max: 1.0 },
                get_fn: |world, entity| world.get::<ColorGradingSettings>(entity).map(|s| FieldValue::Float(s.tint)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<ColorGradingSettings>(entity) { s.tint = v; } } },
            },
        ],
        custom_ui_fn: None,
    }
}

pub struct ColorGradingPlugin;

impl Plugin for ColorGradingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<ColorGradingSettings>::default());
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
