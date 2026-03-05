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
pub struct PixelationSettings {
    pub pixel_size: f32,
    pub _padding0: f32,
    pub _padding1: f32,
    pub _padding2: f32,
    pub _padding3: f32,
    pub _padding4: f32,
    pub _padding5: f32,
    pub enabled: f32,
}

impl Default for PixelationSettings {
    fn default() -> Self {
        Self {
            pixel_size: 4.0,
            _padding0: 0.0,
            _padding1: 0.0,
            _padding2: 0.0,
            _padding3: 0.0,
            _padding4: 0.0,
            _padding5: 0.0,
            enabled: 1.0,
        }
    }
}

impl PostProcessEffect for PixelationSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/pixelation.wgsl".into()
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
        type_id: "pixelation",
        display_name: "Pixelation",
        icon: regular::GRID_FOUR,
        category: "post_process",
        has_fn: |world, entity| world.get::<PixelationSettings>(entity).is_some(),
        add_fn: Some(|world, entity| { world.entity_mut(entity).insert(PixelationSettings::default()); }),
        remove_fn: Some(|world, entity| { world.entity_mut(entity).remove::<PixelationSettings>(); }),
        is_enabled_fn: Some(|world, entity| world.get::<PixelationSettings>(entity).map(|s| s.enabled > 0.5).unwrap_or(false)),
        set_enabled_fn: Some(|world, entity, val| { if let Some(mut s) = world.get_mut::<PixelationSettings>(entity) { s.enabled = if val { 1.0 } else { 0.0 }; } }),
        fields: vec![
            FieldDef {
                name: "Pixel Size",
                field_type: FieldType::Float { speed: 0.5, min: 1.0, max: 64.0 },
                get_fn: |world, entity| world.get::<PixelationSettings>(entity).map(|s| FieldValue::Float(s.pixel_size)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<PixelationSettings>(entity) { s.pixel_size = v; } } },
            },
        ],
        custom_ui_fn: None,
    }
}

pub struct PixelationPlugin;

impl Plugin for PixelationPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<PixelationSettings>();
        app.add_plugins(
            renzora_postprocess::PostProcessPlugin::<PixelationSettings>::default(),
        );
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
