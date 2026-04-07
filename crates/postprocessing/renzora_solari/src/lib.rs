use bevy::prelude::*;
use bevy::render::render_resource::{TextureFormat, TextureUsages};
use bevy::render::view::Hdr;
use bevy::camera::CameraMainTextureUsages;
use bevy::solari::prelude::*;
use bevy::anti_alias::dlss::{Dlss, DlssRayReconstructionFeature, DlssRayReconstructionSupported, DlssPerfQualityMode};
use std::marker::PhantomData;
use serde::{Serialize, Deserialize};

#[cfg(feature = "editor")]
use renzora::editor::{AppEditorExt, InspectorEntry, FieldDef, FieldType, FieldValue};

use renzora::core::ViewportRenderTarget;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Serialize, Deserialize, Default)]
pub enum DlssQualityMode {
    #[default]
    Auto,
    Dlaa,
    Quality,
    Balanced,
    Performance,
    UltraPerformance,
}

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct SolariLightingData {
    pub enabled: bool,
    pub dlss_enabled: bool,
    pub dlss_quality: DlssQualityMode,
}

impl Default for SolariLightingData {
    fn default() -> Self {
        Self {
            enabled: true,
            dlss_enabled: false,
            dlss_quality: DlssQualityMode::Auto,
        }
    }
}

#[derive(Resource, Default)]
struct SolariState {
    active: bool,
    viewport_upgraded: bool,
    pending_mesh_add: bool,
}

/// Marker for cameras that need CameraMainTextureUsages added next frame
#[derive(Component)]
struct PendingSolariCamera;

#[derive(Default)]
pub struct RenzoraSolariPlugin;

impl Plugin for RenzoraSolariPlugin {
    fn build(&self, app: &mut App) {
        info!("[solari] RenzoraSolariPlugin — raytraced lighting available");
        app.add_plugins(SolariPlugins);
        // MeshletPlugin disabled for now — may cause render crashes
        // app.add_plugins(bevy::pbr::experimental::meshlet::MeshletPlugin { cluster_buffer_slots: 8192 });
        // Override Solari's forced deferred back to forward for non-Solari rendering
        app.insert_resource(bevy::pbr::DefaultOpaqueRendererMethod::forward());
        app.register_type::<SolariLightingData>();
        app.register_type::<DlssQualityMode>();
        app.init_resource::<SolariState>();
        app.add_systems(Update, (sync_solari_state, apply_pending_solari_cameras, apply_pending_meshes, prepare_new_meshes));

        #[cfg(feature = "editor")]
        app.register_inspector(inspector_entry());
    }
}

/// Prepare meshes for Solari when RaytracingMesh3d is added: add UVs, generate tangents, remove UV_1
fn prepare_new_meshes(
    new_rt_meshes: Query<&Mesh3d, Added<RaytracingMesh3d>>,
    mut mesh_assets: ResMut<Assets<Mesh>>,
) {
    let count = new_rt_meshes.iter().count();
    if count > 0 {
        info!("[solari] Preparing {} meshes for ray tracing (UVs, tangents)", count);
    }
    for mesh3d in &new_rt_meshes {
        if let Some(mesh) = mesh_assets.get_mut(&mesh3d.0) {
            if !mesh.contains_attribute(Mesh::ATTRIBUTE_UV_0) {
                let vertex_count = mesh.count_vertices();
                mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; vertex_count]);
            }
            if !mesh.contains_attribute(Mesh::ATTRIBUTE_TANGENT)
                && mesh.contains_attribute(Mesh::ATTRIBUTE_NORMAL)
                && mesh.contains_attribute(Mesh::ATTRIBUTE_UV_0)
            {
                let _ = mesh.generate_tangents();
            }
            if mesh.contains_attribute(Mesh::ATTRIBUTE_UV_1) {
                mesh.remove_attribute(Mesh::ATTRIBUTE_UV_1);
            }
        }
    }
}

/// Apply SolariLighting + CameraMainTextureUsages one frame after Hdr is added
fn apply_pending_solari_cameras(
    mut commands: Commands,
    pending: Query<(Entity, Option<&Name>), With<PendingSolariCamera>>,
) {
    for (entity, name) in &pending {
        let cam_name = name.map(|n| n.as_str()).unwrap_or("unnamed");
        info!("[solari] Applying SolariLighting + STORAGE_BINDING to camera {:?} '{}'", entity, cam_name);
        commands.entity(entity).insert((
            SolariLighting { reset: true },
            CameraMainTextureUsages(
                TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::TEXTURE_BINDING
                | TextureUsages::STORAGE_BINDING
            ),
        )).remove::<PendingSolariCamera>();
    }
}

/// Add RaytracingMesh3d to existing meshes one frame after Solari enables
fn apply_pending_meshes(
    mut commands: Commands,
    mut state: ResMut<SolariState>,
    meshes_without_rt: Query<(Entity, &Mesh3d), Without<RaytracingMesh3d>>,
) {
    if !state.pending_mesh_add {
        return;
    }
    let count = meshes_without_rt.iter().count();
    if count > 0 {
        info!("[solari] Adding RaytracingMesh3d to {} meshes (deferred from enable)", count);
    }
    for (entity, mesh3d) in &meshes_without_rt {
        commands.entity(entity).insert(RaytracingMesh3d(mesh3d.0.clone()));
    }
    state.pending_mesh_add = false;
}

fn sync_solari_state(
    mut commands: Commands,
    mut state: ResMut<SolariState>,
    solari_query: Query<&SolariLightingData>,
    camera_query: Query<(Entity, Option<&SolariLighting>, Option<&Name>, Option<&PendingSolariCamera>, Option<&Dlss<DlssRayReconstructionFeature>>, Option<&Hdr>), With<Camera3d>>,
    dlss_rr_supported: Option<Res<DlssRayReconstructionSupported>>,
    meshes_without_rt: Query<(Entity, &Mesh3d), Without<RaytracingMesh3d>>,
    meshes_with_rt: Query<Entity, With<RaytracingMesh3d>>,
    mut mesh_assets: ResMut<Assets<Mesh>>,
    mut default_render_method: ResMut<bevy::pbr::DefaultOpaqueRendererMethod>,
    mut dir_lights: Query<&mut DirectionalLight>,
    mut point_lights: Query<&mut PointLight>,
    mut spot_lights: Query<&mut SpotLight>,
    viewport: Res<ViewportRenderTarget>,
    mut images: ResMut<Assets<Image>>,
) {
    let active = solari_query.iter().find(|d| d.enabled);
    let should_enable = active.is_some();

    if should_enable && !state.active {
        // === ENABLE SOLARI ===
        info!("[solari] Enabling raytraced lighting");
        *default_render_method = bevy::pbr::DefaultOpaqueRendererMethod::deferred();

        // Disable shadows — Solari replaces shadow mapping
        for mut light in &mut dir_lights { light.shadows_enabled = false; }
        for mut light in &mut point_lights { light.shadows_enabled = false; }
        for mut light in &mut spot_lights { light.shadows_enabled = false; }

        // Add SolariLighting + Hdr + camera settings to scene cameras
        for (entity, has_solari, name, pending, _has_dlss, has_hdr) in &camera_query {
            let camera_name = name.map(|n| n.as_str()).unwrap_or("");
            let is_scene_camera = camera_name == "Editor Camera"
                || camera_name == "Camera 3D"
                || camera_name.is_empty();
            if is_scene_camera && has_solari.is_none() && pending.is_none() {
                if has_hdr.is_some() {
                    // Camera already has HDR — safe to add everything immediately
                    info!("[solari] Adding SolariLighting + STORAGE_BINDING to HDR camera {:?} '{}'", entity, camera_name);
                    commands.entity(entity).insert((
                        SolariLighting { reset: true },
                        CameraMainTextureUsages(
                            TextureUsages::RENDER_ATTACHMENT
                            | TextureUsages::TEXTURE_BINDING
                            | TextureUsages::STORAGE_BINDING
                        ),
                        Msaa::Off,
                    ));
                } else {
                    // Camera needs HDR first — defer SolariLighting to next frame
                    info!("[solari] Adding Hdr + Msaa::Off to camera {:?} '{}' (SolariLighting deferred)", entity, camera_name);
                    commands.entity(entity).insert((
                        Hdr,
                        Msaa::Off,
                        PendingSolariCamera,
                    ));
                }
            }
        }

        // Prep and add RaytracingMesh3d to all existing meshes immediately
        let mesh_count = meshes_without_rt.iter().count();
        if mesh_count > 0 {
            info!("[solari] Preparing and adding RaytracingMesh3d to {} existing meshes", mesh_count);
        }
        for (entity, mesh3d) in &meshes_without_rt {
            if let Some(mesh) = mesh_assets.get_mut(&mesh3d.0) {
                if !mesh.contains_attribute(Mesh::ATTRIBUTE_UV_0) {
                    let vertex_count = mesh.count_vertices();
                    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; vertex_count]);
                }
                if !mesh.contains_attribute(Mesh::ATTRIBUTE_TANGENT)
                    && mesh.contains_attribute(Mesh::ATTRIBUTE_NORMAL)
                    && mesh.contains_attribute(Mesh::ATTRIBUTE_UV_0)
                {
                    let _ = mesh.generate_tangents();
                }
                if mesh.contains_attribute(Mesh::ATTRIBUTE_UV_1) {
                    mesh.remove_attribute(Mesh::ATTRIBUTE_UV_1);
                }
            }
            commands.entity(entity).insert(RaytracingMesh3d(mesh3d.0.clone()));
        }

        // Upgrade viewport texture to HDR + STORAGE_BINDING
        if !state.viewport_upgraded {
            if let Some(ref handle) = viewport.image {
                if let Some(image) = images.get_mut(handle) {
                    if image.texture_descriptor.format != TextureFormat::Rgba16Float {
                        info!("[solari] Upgrading viewport texture to Rgba16Float + STORAGE_BINDING");
                        let size = image.texture_descriptor.size;
                        image.texture_descriptor.format = TextureFormat::Rgba16Float;
                        image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
                            | TextureUsages::COPY_DST
                            | TextureUsages::RENDER_ATTACHMENT
                            | TextureUsages::STORAGE_BINDING;
                        image.data = Some(vec![0u8; (size.width * size.height * 8) as usize]);
                        state.viewport_upgraded = true;
                    }
                }
            }
        }

        state.active = true;

    } else if !should_enable && state.active {
        // === DISABLE SOLARI ===
        info!("[solari] Disabling raytraced lighting");
        *default_render_method = bevy::pbr::DefaultOpaqueRendererMethod::forward();

        for (entity, has_solari, _, _, _has_dlss, _) in &camera_query {
            if has_solari.is_some() {
                // Keep Hdr + CameraMainTextureUsages — render pipelines are cached for HDR format.
                commands.entity(entity)
                    .remove::<SolariLighting>()
                    .remove::<Dlss<DlssRayReconstructionFeature>>();
            }
        }

        for entity in &meshes_with_rt {
            commands.entity(entity).remove::<RaytracingMesh3d>();
        }

        // Re-enable shadows
        for mut light in &mut dir_lights { light.shadows_enabled = true; }
        for mut light in &mut point_lights { light.shadows_enabled = true; }
        for mut light in &mut spot_lights { light.shadows_enabled = true; }

        state.active = false;

    } else if should_enable {
        // === SOLARI ACTIVE — maintain state ===
        // Re-add to cameras that lost components (play mode teardown/rebuild)
        for (entity, has_solari, name, pending, _has_dlss, has_hdr) in &camera_query {
            let camera_name = name.map(|n| n.as_str()).unwrap_or("");
            let is_scene_camera = camera_name == "Editor Camera"
                || camera_name == "Camera 3D"
                || camera_name.is_empty();
            if is_scene_camera && has_solari.is_none() && pending.is_none() {
                if has_hdr.is_some() {
                    // Camera already has HDR — safe to add everything immediately
                    info!("[solari] Adding SolariLighting + STORAGE_BINDING to HDR camera {:?} '{}'", entity, camera_name);
                    commands.entity(entity).insert((
                        SolariLighting { reset: true },
                        CameraMainTextureUsages(
                            TextureUsages::RENDER_ATTACHMENT
                            | TextureUsages::TEXTURE_BINDING
                            | TextureUsages::STORAGE_BINDING
                        ),
                        Msaa::Off,
                    ));
                } else {
                    // Camera needs HDR first — defer SolariLighting to next frame
                    info!("[solari] Adding Hdr + Msaa::Off to camera {:?} '{}' (SolariLighting deferred)", entity, camera_name);
                    commands.entity(entity).insert((
                        Hdr,
                        Msaa::Off,
                        PendingSolariCamera,
                    ));
                }
            }
        }
        // Add RaytracingMesh3d to new meshes
        for (entity, mesh3d) in &meshes_without_rt {
            commands.entity(entity).insert(RaytracingMesh3d(mesh3d.0.clone()));
        }
        // Keep shadows off
        for mut light in &mut dir_lights { light.shadows_enabled = false; }
        for mut light in &mut point_lights { light.shadows_enabled = false; }
        for mut light in &mut spot_lights { light.shadows_enabled = false; }

        // Handle DLSS toggle
        if let Some(settings) = active {
            for (entity, has_solari, _, _, has_dlss, _) in &camera_query {
                if has_solari.is_none() { continue; }

                if settings.dlss_enabled && dlss_rr_supported.is_some() {
                    if has_dlss.is_none() {
                        let dlss_quality = match settings.dlss_quality {
                            DlssQualityMode::Auto => DlssPerfQualityMode::Auto,
                            DlssQualityMode::Dlaa => DlssPerfQualityMode::Dlaa,
                            DlssQualityMode::Quality => DlssPerfQualityMode::Quality,
                            DlssQualityMode::Balanced => DlssPerfQualityMode::Balanced,
                            DlssQualityMode::Performance => DlssPerfQualityMode::Performance,
                            DlssQualityMode::UltraPerformance => DlssPerfQualityMode::UltraPerformance,
                        };
                        info!("[solari] Enabling DLSS Ray Reconstruction: {:?}", dlss_quality);
                        commands.entity(entity).insert(Dlss::<DlssRayReconstructionFeature> {
                            perf_quality_mode: dlss_quality,
                            reset: false,
                            _phantom_data: PhantomData,
                        });
                    }
                } else if has_dlss.is_some() {
                    info!("[solari] Disabling DLSS Ray Reconstruction");
                    commands.entity(entity).remove::<Dlss<DlssRayReconstructionFeature>>();
                }
            }
        }
    }
}

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "solari_lighting",
        display_name: "Solari Lighting",
        icon: egui_phosphor::regular::SUN,
        category: "rendering",
        has_fn: |world, entity| world.get::<SolariLightingData>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(SolariLightingData::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<SolariLightingData>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<SolariLightingData>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<SolariLightingData>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![
            FieldDef {
                name: "DLSS Ray Reconstruction",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world.get::<SolariLightingData>(entity).map(|s| FieldValue::Bool(s.dlss_enabled))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(v) = val {
                        if let Some(mut s) = world.get_mut::<SolariLightingData>(entity) {
                            s.dlss_enabled = v;
                        }
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}

renzora::add!(RenzoraSolariPlugin);
