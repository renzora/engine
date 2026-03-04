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
pub struct FilmGrainSettings {
    pub intensity: f32,
    pub grain_size: f32,
    pub time: f32,
    pub _padding0: f32,
    pub _padding1: f32,
    pub _padding2: f32,
    pub _padding3: f32,
    pub enabled: f32,
}

impl Default for FilmGrainSettings {
    fn default() -> Self {
        Self {
            intensity: 0.3,
            grain_size: 1.5,
            time: 0.0,
            _padding0: 0.0,
            _padding1: 0.0,
            _padding2: 0.0,
            _padding3: 0.0,
            enabled: 1.0,
        }
    }
}

impl PostProcessEffect for FilmGrainSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/film_grain.wgsl".into()
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

fn sync_time(time: Res<Time>, mut query: Query<&mut FilmGrainSettings>) {
    for mut s in query.iter_mut() {
        s.time = time.elapsed_secs();
    }
}

fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "film_grain",
        display_name: "Film Grain",
        icon: regular::FILM_STRIP,
        category: "post_process",
        has_fn: |world, entity| world.get::<FilmGrainSettings>(entity).is_some(),
        add_fn: Some(|world, entity| { world.entity_mut(entity).insert(FilmGrainSettings::default()); }),
        remove_fn: Some(|world, entity| { world.entity_mut(entity).remove::<FilmGrainSettings>(); }),
        is_enabled_fn: Some(|world, entity| world.get::<FilmGrainSettings>(entity).map(|s| s.enabled > 0.5).unwrap_or(false)),
        set_enabled_fn: Some(|world, entity, val| { if let Some(mut s) = world.get_mut::<FilmGrainSettings>(entity) { s.enabled = if val { 1.0 } else { 0.0 }; } }),
        fields: vec![
            FieldDef {
                name: "Intensity",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 2.0 },
                get_fn: |world, entity| world.get::<FilmGrainSettings>(entity).map(|s| FieldValue::Float(s.intensity)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<FilmGrainSettings>(entity) { s.intensity = v; } } },
            },
            FieldDef {
                name: "Grain Size",
                field_type: FieldType::Float { speed: 0.1, min: 0.1, max: 10.0 },
                get_fn: |world, entity| world.get::<FilmGrainSettings>(entity).map(|s| FieldValue::Float(s.grain_size)),
                set_fn: |world, entity, val| { if let FieldValue::Float(v) = val { if let Some(mut s) = world.get_mut::<FilmGrainSettings>(entity) { s.grain_size = v; } } },
            },
        ],
        custom_ui_fn: None,
    }
}

pub struct FilmGrainPlugin;

impl Plugin for FilmGrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(
            renzora_postprocess::PostProcessPlugin::<FilmGrainSettings>::default(),
        );
        app.add_systems(Update, sync_time);
        app.init_resource::<InspectorRegistry>();
        let world = app.world_mut();
        if let Some(mut registry) = world.get_resource_mut::<InspectorRegistry>() {
            registry.register(inspector_entry());
        }
    }
}
