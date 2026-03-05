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
pub struct GrayscaleSettings {
    pub intensity: f32,
    pub luminance_r: f32,
    pub luminance_g: f32,
    pub luminance_b: f32,
    pub _padding1: f32,
    pub _padding2: f32,
    pub _padding3: f32,
    pub enabled: f32,
}

impl Default for GrayscaleSettings {
    fn default() -> Self {
        Self {
            intensity: 1.0,
            luminance_r: 0.2126,
            luminance_g: 0.7152,
            luminance_b: 0.0722,
            _padding1: 0.0,
            _padding2: 0.0,
            _padding3: 0.0,
            enabled: 1.0,
        }
    }
}

impl PostProcessEffect for GrayscaleSettings {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_grayscale/grayscale.wgsl".into()
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
        type_id: "grayscale",
        display_name: "Grayscale",
        icon: regular::DROP_HALF_BOTTOM,
        category: "post_process",
        has_fn: |world, entity| world.get::<GrayscaleSettings>(entity).is_some(),
        add_fn: Some(|world, entity| { world.entity_mut(entity).insert(GrayscaleSettings::default()); }),
        remove_fn: Some(|world, entity| { world.entity_mut(entity).remove::<GrayscaleSettings>(); }),
        is_enabled_fn: Some(|world, entity| world.get::<GrayscaleSettings>(entity).map(|s| s.enabled > 0.5).unwrap_or(false)),
        set_enabled_fn: Some(|world, entity, val| { if let Some(mut s) = world.get_mut::<GrayscaleSettings>(entity) { s.enabled = if val { 1.0 } else { 0.0 }; } }),
        fields: vec![
            FieldDef {
                name: "Intensity",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 1.0 },
                get_fn: |world, entity| world.get::<GrayscaleSettings>(entity).map(|s| FieldValue::Float(s.intensity)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<GrayscaleSettings>(entity) { s.intensity = v; } } },
            },
        ],
        custom_ui_fn: None,
    }
}

pub struct GrayscalePlugin;

impl Plugin for GrayscalePlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "grayscale.wgsl");
        app.register_type::<GrayscaleSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<GrayscaleSettings>::default());
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
