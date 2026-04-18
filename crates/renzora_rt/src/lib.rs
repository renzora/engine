//! renzora_rt — Lumen-style screen-space ray-traced lighting pipeline.
//!
//! Provides screen-space GI, reflections, and contact shadows that work with
//! ALL materials automatically. Reads from the rendered HDR color buffer + depth
//! buffer, so no special mesh components are needed.

mod extract;
mod node;
mod prepare;
pub mod settings;

use bevy::asset::embedded_asset;
use bevy::shader::load_shader_library;
use bevy::camera::CameraMainTextureUsages;
use bevy::core_pipeline::{
    core_3d::graph::{Core3d, Node3d},
    prepass::{DepthPrepass, DepthPrepassDoubleBuffer, MotionVectorPrepass},
};
use bevy::prelude::*;
use bevy::render::{
    render_graph::{RenderGraphExt, ViewNodeRunner},
    render_resource::TextureUsages,
    ExtractSchedule, Render, RenderApp, RenderSystems,
};

pub use settings::{RtLighting, RtQuality};

#[cfg(feature = "editor")]
use {
    bevy_egui::egui,
    egui_phosphor::regular,
    renzora_editor_framework::{inline_property, AppEditorExt, EditorCommands, InspectorEntry},
    renzora_theme::Theme,
};

// ============================================================================
// EffectRouting sync (mirrors renzora_ssao pattern)
// ============================================================================

fn sync_rt_lighting(
    mut commands: Commands,
    sources: Query<(Entity, Ref<RtLighting>)>,
    routing: Res<renzora::EffectRouting>,
) {
    let routing_changed = routing.is_changed();
    for (target, source_list) in routing.iter() {
        let mut found = false;
        for &src in source_list {
            if let Ok((_, settings)) = sources.get(src) {
                if !routing_changed && !settings.is_changed() {
                    found = true;
                    break;
                }
                if settings.enabled {
                    commands.entity(*target).try_insert((
                        settings.clone(),
                        DepthPrepass,
                        MotionVectorPrepass,
                        DepthPrepassDoubleBuffer,
                        CameraMainTextureUsages(
                            TextureUsages::COPY_SRC
                                | TextureUsages::RENDER_ATTACHMENT
                                | TextureUsages::TEXTURE_BINDING
                                | TextureUsages::STORAGE_BINDING,
                        ),
                    ));
                } else {
                    commands.entity(*target).remove::<RtLighting>();
                }
                found = true;
                break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<RtLighting>();
            }
        }
    }
}

fn cleanup_rt_lighting(
    mut commands: Commands,
    mut removed: RemovedComponents<RtLighting>,
    routing: Res<renzora::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<RtLighting>();
            }
        }
    }
}

// ============================================================================
// Inspector UI (editor only)
// ============================================================================

#[cfg(feature = "editor")]
fn rt_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(rt) = world.get::<RtLighting>(entity) else {
        return;
    };

    let mut data = rt.clone();
    let mut changed = false;
    let mut row = 0;

    // Quality preset dropdown
    changed |= inline_property(ui, row, "Quality", theme, |ui| {
        let mut current = data.quality;
        let resp = egui::ComboBox::from_id_salt("rt_quality")
            .selected_text(current.label())
            .show_ui(ui, |ui| {
                for q in RtQuality::ALL {
                    if ui.selectable_value(&mut current, q, q.label()).changed() {
                        data.apply_quality(q);
                        return true;
                    }
                }
                false
            });
        let combo_changed = resp.inner.unwrap_or(false);
        if combo_changed {
            data.quality = current;
        }
        combo_changed
    });
    row += 1;

    ui.add_space(4.0);
    ui.label(egui::RichText::new("Global Illumination").strong());

    changed |= inline_property(ui, row, "GI Enabled", theme, |ui| {
        ui.checkbox(&mut data.gi_enabled, "").changed()
    });
    row += 1;

    if data.gi_enabled {
        changed |= inline_property(ui, row, "GI Intensity", theme, |ui| {
            ui.add(
                egui::DragValue::new(&mut data.gi_intensity)
                    .speed(0.01)
                    .range(0.0..=2.0),
            )
            .changed()
        });
        row += 1;

        changed |= inline_property(ui, row, "Max Ray Steps", theme, |ui| {
            ui.add(
                egui::DragValue::new(&mut data.gi_max_ray_steps)
                    .speed(1.0)
                    .range(8..=256),
            )
            .changed()
        });
        row += 1;

        changed |= inline_property(ui, row, "Max Distance", theme, |ui| {
            ui.add(
                egui::DragValue::new(&mut data.gi_max_distance)
                    .speed(0.5)
                    .range(1.0..=200.0),
            )
            .changed()
        });
        row += 1;

        changed |= inline_property(ui, row, "Thickness", theme, |ui| {
            ui.add(
                egui::DragValue::new(&mut data.gi_thickness)
                    .speed(0.01)
                    .range(0.01..=5.0),
            )
            .changed()
        });
        row += 1;
    }

    ui.add_space(4.0);
    ui.label(egui::RichText::new("Reflections").strong());

    changed |= inline_property(ui, row, "Reflections", theme, |ui| {
        ui.checkbox(&mut data.reflections_enabled, "").changed()
    });
    row += 1;

    if data.reflections_enabled {
        changed |= inline_property(ui, row, "Refl Intensity", theme, |ui| {
            ui.add(
                egui::DragValue::new(&mut data.reflections_intensity)
                    .speed(0.01)
                    .range(0.0..=2.0),
            )
            .changed()
        });
        row += 1;
    }

    ui.add_space(4.0);
    ui.label(egui::RichText::new("Contact Shadows").strong());

    changed |= inline_property(ui, row, "Shadows", theme, |ui| {
        ui.checkbox(&mut data.shadows_enabled, "").changed()
    });
    row += 1;

    if data.shadows_enabled {
        changed |= inline_property(ui, row, "Shadow Steps", theme, |ui| {
            ui.add(
                egui::DragValue::new(&mut data.shadow_max_steps)
                    .speed(1.0)
                    .range(4..=64),
            )
            .changed()
        });
        row += 1;
    }

    ui.add_space(4.0);
    ui.label(egui::RichText::new("Denoise").strong());

    changed |= inline_property(ui, row, "Temporal", theme, |ui| {
        ui.checkbox(&mut data.denoise_temporal, "").changed()
    });
    row += 1;

    changed |= inline_property(ui, row, "Spatial Iters", theme, |ui| {
        ui.add(
            egui::DragValue::new(&mut data.denoise_spatial_iterations)
                .speed(1.0)
                .range(0..=5),
        )
        .changed()
    });

    // Reset button
    ui.add_space(4.0);
    if ui.button("Reset Temporal History").clicked() {
        data.reset = true;
        changed = true;
    }

    if changed {
        let new_data = data;
        cmds.push(move |world: &mut World| {
            if let Some(mut rt) = world.get_mut::<RtLighting>(entity) {
                *rt = new_data;
            }
        });
    }
}

#[cfg(feature = "editor")]
fn rt_inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "rt_lighting",
        display_name: "RT Lighting",
        icon: regular::SUN,
        category: "rendering",
        has_fn: |world, entity| world.get::<RtLighting>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(RtLighting::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<RtLighting>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<RtLighting>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<RtLighting>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![],
        custom_ui_fn: Some(rt_custom_ui),
    }
}

// ============================================================================
// Plugin
// ============================================================================

pub struct RtPlugin;

impl Plugin for RtPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] RtPlugin");

        // Common utilities as importable shader library
        load_shader_library!(app, "shaders/common.wgsl");

        // Compute shaders as embedded assets (loaded by node for pipeline creation)
        embedded_asset!(app, "shaders/hi_z_generate.wgsl");
        embedded_asset!(app, "shaders/ssgi_trace.wgsl");
        embedded_asset!(app, "shaders/radiance_cache.wgsl");
        embedded_asset!(app, "shaders/ss_reflections.wgsl");
        embedded_asset!(app, "shaders/ss_shadows.wgsl");
        embedded_asset!(app, "shaders/temporal_denoise.wgsl");
        embedded_asset!(app, "shaders/spatial_denoise.wgsl");
        embedded_asset!(app, "shaders/composite.wgsl");

        app.register_type::<RtLighting>();
        app.register_type::<RtQuality>();
        app.add_systems(Update, (sync_rt_lighting, cleanup_rt_lighting));

        #[cfg(feature = "editor")]
        app.register_inspector(rt_inspector_entry());
    }

    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_resource::<extract::ExtractedLightDirection>()
            .add_systems(ExtractSchedule, extract::extract_rt_lighting)
            .add_systems(
                Render,
                prepare::prepare_rt_lighting_resources.in_set(RenderSystems::PrepareResources),
            )
            .add_render_graph_node::<ViewNodeRunner<node::RtLightingNode>>(
                Core3d,
                node::graph::RtLightingNode,
            )
            .add_render_graph_edges(
                Core3d,
                (
                    Node3d::EndMainPass,
                    node::graph::RtLightingNode,
                    Node3d::Tonemapping,
                ),
            );
    }
}
