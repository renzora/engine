//! Virtual geometry: meshes cut into clusters with a level of detail chosen
//! per cluster, culled and rasterized on the GPU.
//!
//! A model converted to virtual geometry has an `.rvg` file beside its GLB
//! (see `renzora_rvg`). Its meshes then skip Bevy's camera passes and are drawn
//! here instead: a compute pass picks, per view, the clusters whose detail
//! matches their distance and that are inside the frustum; a hardware raster
//! pass draws them into a visibility buffer (which cluster and triangle covers
//! each pixel) and into the view's ordinary depth buffer; and each material's
//! own Bevy shader then shades the covered pixels, once per material, from a
//! fullscreen triangle. Because that last step writes the same targets an
//! ordinary mesh would (lit colour, normals, motion vectors, the deferred
//! G-buffer), lighting, shadows received, SSAO, SSR, TAA, DLSS and Solari need
//! no knowledge of virtual geometry, and converted and ordinary meshes mix
//! freely in one scene.
//!
//! What is not converted keeps rendering exactly as before: models without an
//! `.rvg`, materials this version cannot shade (see `attach.rs`), meshes with
//! blend shapes, and any camera with MSAA on. Skinned meshes are converted: a
//! compute pass deforms their clusters' vertices every frame (`skin.wesl`). Shadow maps still draw the ordinary
//! mesh, which every converted entity keeps.

mod attach;
#[cfg(test)]
mod gpu_tests;
mod material;
mod render;

pub use attach::{RvgUpdated, StandardGeometry, VirtualGeometry, VirtualGeometrySettings};
pub use material::{classify_standard, VgMaterial, VgSurface, VgSystems, VirtualGeometryAppExt};
pub use renzora_rvg::RvgFile;

use bevy::asset::embedded_asset;
use bevy::prelude::*;

/// Path Bevy's PBR shaders import their visibility buffer resolve from when
/// compiled for a visibility buffer pass. See `shaders/resolve.wesl`.
const RESOLVE_MODULE_PATH: &str = "embedded://bevy_pbr/meshlet/visibility_buffer_resolve.wesl";

/// Keeps the resolve module loaded for as long as the app runs.
#[derive(Resource)]
struct ResolveModule(#[allow(dead_code)] Handle<Shader>);

/// Report what the loaded `.rvg` files hold to the editor's asset memory
/// breakdown, twice a second. Each file is the whole of a model's geometry at
/// every level of detail, so on a large scene it is one of the bigger rows.
fn report_memory(
    files: Res<Assets<RvgFile>>,
    memory: Option<ResMut<renzora::load_timeline::AssetMemory>>,
    mut frames: Local<u32>,
) {
    *frames = frames.wrapping_add(1);
    let Some(mut memory) = memory.filter(|_| *frames % 30 == 1) else {
        return;
    };
    memory.set(renzora::load_timeline::AssetMemoryRow {
        kind: "virtual geometry (.rvg files)",
        count: files.len(),
        bytes: files.iter().map(|(_, file)| file.heap_bytes()).sum(),
        in_process: true,
        note: String::new(),
    });
}

#[derive(Default)]
pub struct VirtualGeometryPlugin;

impl Plugin for VirtualGeometryPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] VirtualGeometryPlugin");
        // No loader here: an `.rvg` loads as a model, with its clusters as a
        // sub-asset, through `renzora_rvg::model::RvgModelPlugin`, which the
        // engine installs with glTF support.
        app.init_asset::<RvgFile>()
            // Before any early return below: wherever the renderer is not
            // installed (the web, a headless app), a model stored as `.rvg`
            // still needs its full detail drawn the ordinary way.
            .init_resource::<attach::FullDetailMeshes>()
            .add_systems(PostUpdate, attach::full_detail.after(VgSystems::Attach))
            .register_type::<StandardGeometry>()
            .init_resource::<VirtualGeometrySettings>()
            .init_resource::<attach::VgFiles>()
            .add_message::<RvgUpdated>()
            .add_systems(Update, report_memory);

        // Below this point the renderer is installed, and only then are meshes
        // ever converted: a mesh taken out of Bevy's passes with nothing to
        // draw it would simply vanish.
        //
        // The renderer needs storage buffers in every stage it uses, which the
        // web's WebGL2 fallback does not have. On the web every mesh renders
        // as ordinary geometry; the `.rvg` files simply go unused.
        if cfg!(target_arch = "wasm32") {
            app.insert_resource(VirtualGeometrySettings {
                enabled: false,
                ..default()
            });
            return;
        }

        bevy::shader::load_shader_library!(app, "shaders/types.wesl");
        bevy::shader::load_shader_library!(app, "shaders/bindings.wesl");
        embedded_asset!(app, "shaders/cull.wesl");
        embedded_asset!(app, "shaders/skin.wesl");
        embedded_asset!(app, "shaders/transparent.wesl");
        embedded_asset!(app, "shaders/raster.wesl");
        embedded_asset!(app, "shaders/raster_masked.wesl");
        embedded_asset!(app, "shaders/material_depth.wesl");
        embedded_asset!(app, "shaders/material.wesl");
        embedded_asset!(app, "shaders/standard_prepass.wesl");
        embedded_asset!(app, "shaders/debug.wesl");

        match app.world_mut().get_resource_mut::<Assets<Shader>>() {
            Some(mut shaders) => {
                let handle = shaders.add(Shader::from_wesl(
                    include_str!("shaders/resolve.wesl"),
                    RESOLVE_MODULE_PATH,
                ));
                app.insert_resource(ResolveModule(handle));
            }
            None => {
                warn!("[virtual geometry] no shader assets; converted meshes render as ordinary geometry");
                app.insert_resource(VirtualGeometrySettings {
                    enabled: false,
                    ..default()
                });
                return;
            }
        }

        if !render::build(app) {
            // A headless server or test app: nothing renders, so nothing
            // needs converting.
            return;
        }
        app.insert_resource(attach::RendererActive);
        app.init_resource::<attach::TracedMeshes>()
            .add_systems(PostUpdate, attach::traced_geometry.after(VgSystems::Attach));
        app.configure_sets(PostUpdate, VgSystems::Classify.before(VgSystems::Attach))
            .add_systems(
                PostUpdate,
                (
                    attach::follow_project,
                    attach::react_to_changes,
                    attach::follow_opt_out,
                    attach::poll_files,
                    attach::detach,
                    attach::attach,
                    attach::disable_msaa,
                )
                    .chain()
                    .in_set(VgSystems::Attach),
            )
            .add_virtual_geometry_material::<StandardMaterial>(classify_standard);
    }
}

renzora::add!(VirtualGeometryPlugin);
