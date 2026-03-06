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
use renzora_editor::{AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry};

#[derive(Component, Clone, Copy, Reflect, Serialize, Deserialize, ShaderType, ExtractComponent)]
#[reflect(Component, Serialize, Deserialize)]
#[extract_component_filter(With<Camera3d>)]
pub struct GaussianBlurSettings {
    pub sigma: f32,
    pub kernel_size: u32,
    pub _padding0: f32,
    pub _padding1: f32,
    pub _padding2: f32,
    pub _padding3: f32,
    pub _padding4: f32,
    pub enabled: f32,
}

impl Default for GaussianBlurSettings {
    fn default() -> Self {
        Self {
            sigma: 2.0,
            kernel_size: 9,
            _padding0: 0.0,
            _padding1: 0.0,
            _padding2: 0.0,
            _padding3: 0.0,
            _padding4: 0.0,
            enabled: 1.0,
        }
    }
}

impl PostProcessEffect for GaussianBlurSettings {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_gaussian_blur/gaussian_blur.wgsl".into()
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
        type_id: "gaussian_blur",
        display_name: "Gaussian Blur",
        icon: regular::DROP_HALF_BOTTOM,
        category: "post_process",
        has_fn: |world, entity| world.get::<GaussianBlurSettings>(entity).is_some(),
        add_fn: Some(|world, entity| { world.entity_mut(entity).insert(GaussianBlurSettings::default()); }),
        remove_fn: Some(|world, entity| { world.entity_mut(entity).remove::<GaussianBlurSettings>(); }),
        is_enabled_fn: Some(|world, entity| world.get::<GaussianBlurSettings>(entity).map(|s| s.enabled > 0.5).unwrap_or(false)),
        set_enabled_fn: Some(|world, entity, val| { if let Some(mut s) = world.get_mut::<GaussianBlurSettings>(entity) { s.enabled = if val { 1.0 } else { 0.0 }; } }),
        fields: vec![
            FieldDef {
                name: "Sigma",
                field_type: FieldType::Float { speed: 0.1, min: 0.1, max: 20.0 },
                get_fn: |world, entity| world.get::<GaussianBlurSettings>(entity).map(|s| FieldValue::Float(s.sigma)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<GaussianBlurSettings>(entity) { s.sigma = v; } } },
            },
            FieldDef {
                name: "Kernel Size",
                field_type: FieldType::Float { speed: 1.0, min: 1.0, max: 64.0 },
                get_fn: |world, entity| world.get::<GaussianBlurSettings>(entity).map(|s| FieldValue::Float(s.kernel_size as f32)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<GaussianBlurSettings>(entity) { s.kernel_size = v as u32; } } },
            },
        ],
        custom_ui_fn: None,
    }
}

pub struct GaussianBlurPlugin;

impl Plugin for GaussianBlurPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "gaussian_blur.wgsl");
        app.register_type::<GaussianBlurSettings>();
        app.add_plugins(
            renzora_postprocess::PostProcessPlugin::<GaussianBlurSettings>::default(),
        );
        #[cfg(feature = "editor")]
        app.register_inspector(inspector_entry());
    }
}
