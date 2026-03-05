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
pub struct MatrixSettings {
    pub speed: f32,
    pub density: f32,
    pub glow: f32,
    pub trail_length: f32,
    pub color_r: f32,
    pub color_g: f32,
    pub time: f32,
    pub enabled: f32,
}

impl Default for MatrixSettings {
    fn default() -> Self {
        Self {
            speed: 2.0,
            density: 20.0,
            glow: 0.5,
            trail_length: 0.8,
            color_r: 0.0,
            color_g: 1.0,
            time: 0.0,
            enabled: 1.0,
        }
    }
}

impl PostProcessEffect for MatrixSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/matrix.wgsl".into()
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

fn sync_time(time: Res<Time>, mut query: Query<&mut MatrixSettings>) {
    for mut s in query.iter_mut() {
        s.time = time.elapsed_secs();
    }
}

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "matrix",
        display_name: "Matrix Rain",
        icon: regular::CODE,
        category: "post_process",
        has_fn: |world, entity| world.get::<MatrixSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(MatrixSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<MatrixSettings>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<MatrixSettings>(entity).map(|s| s.enabled > 0.5).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<MatrixSettings>(entity) {
                s.enabled = if val { 1.0 } else { 0.0 };
            }
        }),
        fields: vec![
            FieldDef {
                name: "Speed",
                field_type: FieldType::Float { speed: 0.05, min: 0.1, max: 10.0 },
                get_fn: |world, entity| {
                    world.get::<MatrixSettings>(entity).map(|s| FieldValue::Float(s.speed))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<MatrixSettings>(entity) {
                            s.speed = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Density",
                field_type: FieldType::Float { speed: 0.5, min: 5.0, max: 50.0 },
                get_fn: |world, entity| {
                    world.get::<MatrixSettings>(entity).map(|s| FieldValue::Float(s.density))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<MatrixSettings>(entity) {
                            s.density = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Glow",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 1.0 },
                get_fn: |world, entity| {
                    world.get::<MatrixSettings>(entity).map(|s| FieldValue::Float(s.glow))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<MatrixSettings>(entity) {
                            s.glow = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Trail Length",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 1.0 },
                get_fn: |world, entity| {
                    world.get::<MatrixSettings>(entity).map(|s| FieldValue::Float(s.trail_length))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<MatrixSettings>(entity) {
                            s.trail_length = v;
                        }
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}

pub struct MatrixPlugin;

impl Plugin for MatrixPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<MatrixSettings>::default());
        app.add_systems(Update, sync_time);
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
