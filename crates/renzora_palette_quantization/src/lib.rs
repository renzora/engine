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
pub struct PaletteQuantizationSettings {
    pub num_colors: u32,
    pub dithering: f32,
    pub _padding0: f32,
    pub _padding1: f32,
    pub _padding2: f32,
    pub _padding3: f32,
    pub _padding4: f32,
    pub enabled: f32,
}

impl Default for PaletteQuantizationSettings {
    fn default() -> Self {
        Self {
            num_colors: 8,
            dithering: 0.5,
            _padding0: 0.0,
            _padding1: 0.0,
            _padding2: 0.0,
            _padding3: 0.0,
            _padding4: 0.0,
            enabled: 1.0,
        }
    }
}

impl PostProcessEffect for PaletteQuantizationSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/palette_quantization.wgsl".into()
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
        type_id: "palette_quantization",
        display_name: "Palette Quantization",
        icon: regular::PALETTE,
        category: "post_process",
        has_fn: |world, entity| world.get::<PaletteQuantizationSettings>(entity).is_some(),
        add_fn: Some(|world, entity| { world.entity_mut(entity).insert(PaletteQuantizationSettings::default()); }),
        remove_fn: Some(|world, entity| { world.entity_mut(entity).remove::<PaletteQuantizationSettings>(); }),
        is_enabled_fn: Some(|world, entity| world.get::<PaletteQuantizationSettings>(entity).map(|s| s.enabled > 0.5).unwrap_or(false)),
        set_enabled_fn: Some(|world, entity, val| { if let Some(mut s) = world.get_mut::<PaletteQuantizationSettings>(entity) { s.enabled = if val { 1.0 } else { 0.0 }; } }),
        fields: vec![
            FieldDef {
                name: "Num Colors",
                field_type: FieldType::Float { speed: 1.0, min: 2.0, max: 256.0 },
                get_fn: |world, entity| world.get::<PaletteQuantizationSettings>(entity).map(|s| FieldValue::Float(s.num_colors as f32)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<PaletteQuantizationSettings>(entity) { s.num_colors = v as u32; } } },
            },
            FieldDef {
                name: "Dithering",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 1.0 },
                get_fn: |world, entity| world.get::<PaletteQuantizationSettings>(entity).map(|s| FieldValue::Float(s.dithering)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<PaletteQuantizationSettings>(entity) { s.dithering = v; } } },
            },
        ],
        custom_ui_fn: None,
    }
}

pub struct PaletteQuantizationPlugin;

impl Plugin for PaletteQuantizationPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(
            renzora_postprocess::PostProcessPlugin::<PaletteQuantizationSettings>::default(),
        );
        app.init_resource::<InspectorRegistry>();
        let world = app.world_mut();
        if let Some(mut registry) = world.get_resource_mut::<InspectorRegistry>() {
            registry.register(inspector_entry());
        }
    }
}
