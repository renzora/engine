//! Runtime registration for the graph material.
//!
//! The actual asset type lives in `surface_ext.rs` as
//! `GraphMaterial = ExtendedMaterial<StandardMaterial, SurfaceGraphExt>`.
//! This module only owns the plugin that registers it and the fallback-texture
//! resource used for unused texture slots.

use bevy::prelude::*;
use bevy::pbr::MaterialPlugin as BevyMaterialPlugin;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use super::surface_ext::{DEFAULT_EXT_FRAG_SRC, SURFACE_GRAPH_EXT_DEFAULT_FRAG};
pub use super::surface_ext::{GraphMaterial, new_graph_material};

// ── Fallback texture resource ───────────────────────────────────────────────

/// 1x1 white image handle, always valid. Points at every texture slot on a
/// graph material that the user hasn't bound explicitly — so the pipeline
/// layout is always complete even when a material only uses, say, texture_0.
#[derive(Resource, Clone)]
pub struct FallbackTexture(pub Handle<Image>);

// ── Shader state (kept for API compatibility with the resolver) ─────────────

/// Tracks the last compiled WGSL's hash per shader handle. Used to avoid
/// redundant `Shader::from_wgsl` allocations when nothing changed.
#[derive(Resource, Default)]
pub struct GraphMaterialShaderState {
    pub last_wgsl_hash: Option<u64>,
}

// ── Plugin ──────────────────────────────────────────────────────────────────

pub struct GraphMaterialPlugin;

impl Plugin for GraphMaterialPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] GraphMaterialPlugin (ExtendedMaterial<StandardMaterial, SurfaceGraphExt>)");

        // Create the fallback image up-front so it's available before any
        // material is created. 1x1 white is benign in a multiply chain and
        // makes alpha=1.0 fill naturally.
        let fallback_image = Image::new(
            Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            TextureDimension::D2,
            vec![255, 255, 255, 255],
            TextureFormat::Rgba8UnormSrgb,
            default(),
        );
        let fallback_handle = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(fallback_image);
        app.insert_resource(FallbackTexture(fallback_handle));

        app.add_plugins(BevyMaterialPlugin::<GraphMaterial>::default())
            .init_resource::<GraphMaterialShaderState>()
            .add_systems(PostStartup, setup_default_shader);
    }
}

/// Register the default extension-hook fragment shader at its well-known handle.
/// Materials that haven't been compiled yet fall back to this, which just runs
/// StandardMaterial unchanged.
fn setup_default_shader(mut shaders: ResMut<Assets<Shader>>) {
    let shader = Shader::from_wgsl(
        DEFAULT_EXT_FRAG_SRC.to_string(),
        "graph_material://default_ext",
    );
    let _ = shaders.insert(&SURFACE_GRAPH_EXT_DEFAULT_FRAG, shader);
}

