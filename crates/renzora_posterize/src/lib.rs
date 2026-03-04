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
pub struct PosterizeSettings {
    pub levels: f32,
    pub _padding1: f32,
    pub _padding2: f32,
    pub _padding3: f32,
    pub _padding4: f32,
    pub _padding5: f32,
    pub _padding6: f32,
    pub enabled: f32,
}

impl Default for PosterizeSettings {
    fn default() -> Self {
        Self {
            levels: 8.0,
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

impl PostProcessEffect for PosterizeSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/posterize.wgsl".into()
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
        type_id: "posterize",
        display_name: "Posterize",
        icon: regular::STAIRS,
        category: "post_process",
        has_fn: |world, entity| world.get::<PosterizeSettings>(entity).is_some(),
        add_fn: Some(|world, entity| { world.entity_mut(entity).insert(PosterizeSettings::default()); }),
        remove_fn: Some(|world, entity| { world.entity_mut(entity).remove::<PosterizeSettings>(); }),
        is_enabled_fn: Some(|world, entity| world.get::<PosterizeSettings>(entity).map(|s| s.enabled > 0.5).unwrap_or(false)),
        set_enabled_fn: Some(|world, entity, val| { if let Some(mut s) = world.get_mut::<PosterizeSettings>(entity) { s.enabled = if val { 1.0 } else { 0.0 }; } }),
        fields: vec![
            FieldDef {
                name: "Levels",
                field_type: FieldType::Float { speed: 1.0, min: 2.0, max: 64.0 },
                get_fn: |world, entity| world.get::<PosterizeSettings>(entity).map(|s| FieldValue::Float(s.levels)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<PosterizeSettings>(entity) { s.levels = v; } } },
            },
        ],
        custom_ui_fn: None,
    }
}

pub struct PosterizePlugin;

impl Plugin for PosterizePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<PosterizeSettings>::default());
        app.init_resource::<InspectorRegistry>();
        let world = app.world_mut();
        if let Some(mut registry) = world.get_resource_mut::<InspectorRegistry>() {
            registry.register(inspector_entry());
        }
    }
}
