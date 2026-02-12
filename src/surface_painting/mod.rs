//! Splatmap-based surface painting system.
//!
//! Allows painting material layers onto any mesh using a brush.
//! Each mesh gets an RGBA splatmap texture that blends up to 4 layers.

pub mod brush;
pub mod component;
pub mod data;
pub mod material;
pub mod shader_gen;

pub use data::*;

use bevy::prelude::*;
use bevy::pbr::MaterialPlugin;

use crate::core::AppState;

use brush::SplatmapHandles;
use material::{SplatmapMaterial, SPLATMAP_FRAG_SHADER_HANDLE};
use shader_gen::DEFAULT_SPLATMAP_SHADER;

/// Plugin that registers the splatmap material, resources, and systems.
pub struct SurfacePaintingPlugin;

/// Insert the default splatmap shader at the UUID handle on startup.
fn setup_splatmap_shader(mut shaders: ResMut<Assets<Shader>>) {
    let _ = shaders.insert(
        &SPLATMAP_FRAG_SHADER_HANDLE,
        Shader::from_wgsl(DEFAULT_SPLATMAP_SHADER, "splatmap://default"),
    );
}

impl Plugin for SurfacePaintingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<SplatmapMaterial>::default())
            .register_type::<PaintableSurfaceData>()
            .init_resource::<SurfacePaintSettings>()
            .init_resource::<SurfacePaintState>()
            .init_resource::<SplatmapHandles>()
            .add_systems(OnEnter(AppState::Editor), setup_splatmap_shader)
            .add_systems(
                Update,
                (
                    brush::surface_paint_shortcut_system,
                    brush::surface_paint_brush_scroll_system,
                    component::init_splatmap_for_new_surfaces,
                    component::enforce_splatmap_material,
                    brush::surface_paint_hover_system,
                    brush::surface_paint_system,
                    brush::surface_paint_gizmo_system,
                    brush::surface_paint_sync_system,
                    brush::surface_paint_command_system,
                    shader_gen::splatmap_shader_regen_system,
                    brush::surface_paint_layer_preview_system,
                    component::cleanup_splatmap_handles,
                )
                    .chain()
                    .run_if(in_state(AppState::Editor)),
            );
    }
}
