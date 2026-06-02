//! Tab asset cache — keeps GLBs and the assets each tab's live entities
//! actually reference (meshes, materials, textures) resident across tab
//! switches so respawn is near-instant and doesn't show white/missing
//! textures.
//!
//! ## The problem this solves
//!
//! Switching tabs today:
//!   1. Serialize current scene → RON snapshot for the leaving tab.
//!   2. Despawn every scene entity. Every typed handle each entity held
//!      drops; Bevy's per-asset refcount falls, and anything no other
//!      holder is referencing gets evicted from `Assets<...>`.
//!   3. Deserialize target tab's snapshot → new entities flow in,
//!      `rehydrate_mesh_instances` calls `asset_server.load(model_path)`,
//!      `MaterialResolver` re-binds `MaterialRef`s, etc.
//!
//! Step 2 makes step 3 expensive — and worse, **wrong-looking**: a model
//! whose `MeshMaterial3d<StandardMaterial>` points at a material created
//! by `MaterialResolver` (the gltf-bake path) drops the only strong handle
//! to that material when its entity despawns. Even though `MaterialCache`
//! caches the *initial* resolution, anything that bypasses the resolver
//! (terrain / water / foliage materials, direct `Assets<…>.add()` calls)
//! has no other pin → asset evicts → next tab-switch back shows missing
//! textures until Bevy re-resolves the path.
//!
//! ## The fix
//!
//! Two layers:
//!
//!   1. **GLB pin** — for every `MeshInstanceData` that arrives, hold a
//!      strong `Handle<Gltf>` keyed by tab id. The `Gltf` asset carries
//!      strong handles to its scenes / meshes / materials, which keep
//!      *gltf-loaded* assets resident. Cheap, idempotent.
//!
//!   2. **Live-entity pin** — right before a tab's entities are despawned,
//!      walk every live entity and snapshot every typed asset handle it
//!      references: `Mesh3d`, `MeshMaterial3d<StandardMaterial>`,
//!      `MeshMaterial3d<GraphMaterial>`, `SceneRoot`. Also transitively
//!      collect every `Handle<Image>` reachable through those materials.
//!      Store the lot as `UntypedHandle`s under the leaving tab's id.
//!
//!   Layer 2 catches everything layer 1 misses — anything the entity is
//!   *currently* rendering with, regardless of where it came from.
//!
//! ## Lifecycle
//!
//! - **Add (GLB pin)** — `cache_added_mesh_instances` system, on every
//!   `Added<MeshInstanceData>`.
//! - **Pin (live)** — `pin_live_tab_handles(world, tab_id)`, called by
//!   the scene crate's despawn paths immediately before each
//!   `despawn_scene_entities`.
//! - **Hit** — tab switches respawn entities; their `asset_server.load`
//!   calls return the cached handle; `MaterialResolver` finds the
//!   material already in `MaterialCache`; textures already resident.
//! - **Evict** — `evict_closed_tab` observer drops everything for a
//!   closed tab. Cross-tab dedup is automatic via Bevy's refcount.

use std::collections::{HashMap, HashSet};

use bevy::asset::UntypedHandle;
use bevy::pbr::StandardMaterial;
use bevy::prelude::*;
use renzora::core::{MeshInstanceData, TabClosed};
use renzora::{EditorCamera, HideInHierarchy};
use renzora_shader::material::GraphMaterial;
use renzora_ui::DocumentTabState;

/// Per-tab pin set. `gltfs` keeps GLB assets path-deduped and reusable;
/// `live` is a flat untyped catch-all populated at despawn time by
/// walking the entity world.
#[derive(Default)]
struct TabPin {
    /// Strong `Handle<Gltf>` per asset path. Holding these keeps the
    /// glTF asset (and everything it transitively references inside
    /// `Gltf.materials` / `Gltf.meshes` / `Gltf.scenes`) resident.
    gltfs: HashMap<String, Handle<Gltf>>,
    /// Strong untyped handles to every mesh / material / image the
    /// tab's entities reference at despawn time. Belt-and-suspenders
    /// over `gltfs` for entities that have been rebound to materials
    /// outside the original glTF (MaterialRef → MaterialResolver, or
    /// any custom material system that adds its own `MeshMaterial3d`).
    live: Vec<UntypedHandle>,
    /// Per-handle-type counts of what's currently in `live`. Cheap to
    /// stash here at pin time and lets the diagnostics panel render the
    /// breakdown without re-walking the entity world.
    live_breakdown: LivePinBreakdown,
}

/// Counts the diagnostics panel needs to render the tab-pin section.
/// Stored alongside the flat `live` Vec so the panel doesn't have to
/// classify untyped handles itself.
#[derive(Default, Clone, Copy)]
pub struct LivePinBreakdown {
    pub meshes: usize,
    pub std_mats: usize,
    pub graph_mats: usize,
    pub scenes: usize,
    pub images: usize,
}

/// Strong-handle cache keyed by tab id.
#[derive(Resource, Default)]
pub struct TabAssetCache {
    by_tab: HashMap<u64, TabPin>,
}

impl TabAssetCache {
    /// Drop every handle a tab claimed. Called by the editor's tab-close
    /// path. Other tabs holding the same path keep the asset alive via
    /// Bevy's refcount — we don't need to track sharing ourselves.
    pub fn drop_tab(&mut self, tab_id: u64) {
        if let Some(removed) = self.by_tab.remove(&tab_id) {
            debug!(
                "[tab_asset_cache] dropped tab {} ({} gltfs, {} live handles)",
                tab_id,
                removed.gltfs.len(),
                removed.live.len()
            );
        }
    }

    /// Drop just the live-entity pin set for a tab, keeping its GLB pins
    /// intact. Used when a tab's scene is wiped in place (New Scene on
    /// the same tab id) — the old live handles can't be reached anymore,
    /// but the GLBs we'd want for a future drag-drop are worth keeping.
    pub fn drop_tab_live(&mut self, tab_id: u64) {
        if let Some(pin) = self.by_tab.get_mut(&tab_id) {
            if !pin.live.is_empty() {
                debug!(
                    "[tab_asset_cache] cleared {} live handles for tab {}",
                    pin.live.len(),
                    tab_id
                );
                pin.live.clear();
            }
        }
    }

    /// Record (or refresh) a GLB handle against a tab. Called by
    /// [`cache_added_mesh_instances`] as `MeshInstanceData` entities arrive.
    /// Idempotent: storing the same path twice replaces the handle with
    /// itself.
    fn record_gltf(&mut self, tab_id: u64, path: String, handle: Handle<Gltf>) {
        self.by_tab
            .entry(tab_id)
            .or_default()
            .gltfs
            .insert(path, handle);
    }

    fn store_live_with_breakdown(
        &mut self,
        tab_id: u64,
        handles: Vec<UntypedHandle>,
        breakdown: LivePinBreakdown,
    ) {
        let entry = self.by_tab.entry(tab_id).or_default();
        entry.live = handles;
        entry.live_breakdown = breakdown;
    }

    /// Snapshot every tab's pin counts for the diagnostics panel. Sorted
    /// by tab id for stable display.
    pub fn snapshot_for_diagnostics(&self) -> Vec<TabPinSnapshot> {
        let mut out: Vec<TabPinSnapshot> = self
            .by_tab
            .iter()
            .map(|(&tab_id, pin)| TabPinSnapshot {
                tab_id,
                gltf_count: pin.gltfs.len(),
                live_total: pin.live.len(),
                breakdown: pin.live_breakdown,
            })
            .collect();
        out.sort_by_key(|s| s.tab_id);
        out
    }
}

/// Read-only view of a single tab's pin state for diagnostics rendering.
pub struct TabPinSnapshot {
    pub tab_id: u64,
    pub gltf_count: usize,
    pub live_total: usize,
    pub breakdown: LivePinBreakdown,
}

/// System: snapshot every freshly-arrived `MeshInstanceData` into the
/// active tab's handle set. Uses `Added<MeshInstanceData>` so we only
/// pay for new entities — the query is empty in steady state.
///
/// Runs in both `Loading` and `Editor` states because models can arrive
/// in either: the splash-loaded scene rehydrates entities during
/// `Loading`, and drag-drop / tab-respawn add them during `Editor`.
pub fn cache_added_mesh_instances(
    asset_server: Res<AssetServer>,
    tabs: Option<Res<DocumentTabState>>,
    mut cache: ResMut<TabAssetCache>,
    new_instances: Query<&MeshInstanceData, Added<MeshInstanceData>>,
) {
    let Some(tabs) = tabs else { return };
    let Some(active_id) = tabs.active_tab_id() else {
        return;
    };

    for instance in new_instances.iter() {
        let Some(ref path) = instance.model_path else {
            continue;
        };
        // `asset_server.load` returns the already-cached handle if the
        // asset is loaded; for a fresh model it kicks the load and we
        // get a strong handle to the future asset. Either way, holding
        // it in the cache pins the asset's lifetime to the tab's.
        let handle: Handle<Gltf> = asset_server.load(path);
        cache.record_gltf(active_id, path.clone(), handle);
    }
}

/// Walk every entity that's about to be despawned by the scene-cleanup
/// path and snapshot strong handles to every asset it currently
/// references. Mirrors `despawn_scene_entities`' filter
/// (`Without<EditorCamera>`, `Without<HideInHierarchy>`) so we pin
/// exactly the set that will lose its references at despawn time.
///
/// Collects:
///   - `Mesh3d` → `Handle<Mesh>`
///   - `MeshMaterial3d<StandardMaterial>` → `Handle<StandardMaterial>`
///     plus every `Handle<Image>` field inside the material asset.
///   - `MeshMaterial3d<GraphMaterial>` → `Handle<GraphMaterial>` plus
///     every texture slot (`texture_0..5`, `cube_0`, `array_0`,
///     `volume_0`) inside the asset's `SurfaceGraphExt`. Also pulls
///     image handles out of the wrapped `StandardMaterial` base.
///   - `SceneRoot` → `Handle<Scene>` (bevy_scene still owns dynamic
///     scenes for entities the user hasn't dropped through the flatten
///     path).
///
/// Stores everything as `UntypedHandle` in the per-tab pin set so the
/// tab can re-spawn instantly without re-decoding textures from disk.
pub fn pin_live_tab_handles(world: &mut World, tab_id: u64) {
    // Phase 1: collect typed handles directly off entities.
    let mut mesh_handles: Vec<Handle<Mesh>> = Vec::new();
    let mut std_mat_handles: Vec<Handle<StandardMaterial>> = Vec::new();
    let mut graph_mat_handles: Vec<Handle<GraphMaterial>> = Vec::new();
    let mut scene_handles: Vec<Handle<Scene>> = Vec::new();

    {
        let mut q = world.query_filtered::<&Mesh3d, (Without<EditorCamera>, Without<HideInHierarchy>)>();
        for m in q.iter(world) {
            mesh_handles.push(m.0.clone());
        }
    }
    {
        let mut q = world.query_filtered::<
            &MeshMaterial3d<StandardMaterial>,
            (Without<EditorCamera>, Without<HideInHierarchy>),
        >();
        for m in q.iter(world) {
            std_mat_handles.push(m.0.clone());
        }
    }
    {
        let mut q = world.query_filtered::<
            &MeshMaterial3d<GraphMaterial>,
            (Without<EditorCamera>, Without<HideInHierarchy>),
        >();
        for m in q.iter(world) {
            graph_mat_handles.push(m.0.clone());
        }
    }
    {
        let mut q = world.query_filtered::<&SceneRoot, (Without<EditorCamera>, Without<HideInHierarchy>)>();
        for s in q.iter(world) {
            scene_handles.push(s.0.clone());
        }
    }

    // Phase 2: walk material assets to fish out their image handles.
    // Done in two separate borrows so we don't hold any `Assets<…>`
    // borrow across collection.
    let mut image_handles: Vec<Handle<Image>> = Vec::new();

    if let Some(mats) = world.get_resource::<Assets<StandardMaterial>>() {
        for h in &std_mat_handles {
            let Some(mat) = mats.get(h) else { continue };
            collect_standard_material_images(mat, &mut image_handles);
        }
    }
    if let Some(mats) = world.get_resource::<Assets<GraphMaterial>>() {
        for h in &graph_mat_handles {
            let Some(mat) = mats.get(h) else { continue };
            // Wrapped StandardMaterial base also owns image fields.
            collect_standard_material_images(&mat.base, &mut image_handles);
            // Custom extension texture slots.
            for slot in [
                mat.extension.texture_0.as_ref(),
                mat.extension.texture_1.as_ref(),
                mat.extension.texture_2.as_ref(),
                mat.extension.texture_3.as_ref(),
                mat.extension.texture_4.as_ref(),
                mat.extension.texture_5.as_ref(),
                mat.extension.cube_0.as_ref(),
                mat.extension.array_0.as_ref(),
                mat.extension.volume_0.as_ref(),
            ]
            .into_iter()
            .flatten()
            {
                image_handles.push(slot.clone());
            }
        }
    }

    // Phase 3: merge into a single untyped pin set. `HashSet<AssetId>`
    // would dedupe but UntypedHandle isn't `Hash`-stable across types;
    // since the cost is a `Vec<Handle>` per tab (small) we accept the
    // duplicates — the per-asset refcount only cares about distinct
    // strong holders, not the count.
    let n_meshes = mesh_handles.len();
    let n_std_mats = std_mat_handles.len();
    let n_graph_mats = graph_mat_handles.len();
    let n_scenes = scene_handles.len();
    let n_images = image_handles.len();
    let mut live: Vec<UntypedHandle> =
        Vec::with_capacity(n_meshes + n_std_mats + n_graph_mats + n_scenes + n_images);
    live.extend(mesh_handles.into_iter().map(|h| h.untyped()));
    live.extend(std_mat_handles.into_iter().map(|h| h.untyped()));
    live.extend(graph_mat_handles.into_iter().map(|h| h.untyped()));
    live.extend(scene_handles.into_iter().map(|h| h.untyped()));
    live.extend(image_handles.into_iter().map(|h| h.untyped()));

    let live_total = live.len();
    if let Some(mut cache) = world.get_resource_mut::<TabAssetCache>() {
        cache.store_live_with_breakdown(
            tab_id,
            live,
            LivePinBreakdown {
                meshes: n_meshes,
                std_mats: n_std_mats,
                graph_mats: n_graph_mats,
                scenes: n_scenes,
                images: n_images,
            },
        );
        debug!(
            "[tab_asset_cache] tab {} pinned {} handles ({} meshes, {} std-mats, {} graph-mats, {} scenes, {} images)",
            tab_id, live_total, n_meshes, n_std_mats, n_graph_mats, n_scenes, n_images,
        );
    }
}

fn collect_standard_material_images(mat: &StandardMaterial, out: &mut Vec<Handle<Image>>) {
    // Only the unconditionally-present texture slots — the rest
    // (`specular_*`, `anisotropy_*`, `clearcoat_*`, `*_transmission_*`,
    // `thickness_*`) sit behind `pbr_specular_textures` /
    // `pbr_anisotropy_texture` / `pbr_multi_layer_material_textures` /
    // `pbr_transmission_textures` cargo features that renzora doesn't
    // enable. Reach for `cfg!(feature = …)` here if any of those get
    // turned on in the workspace later.
    for slot in [
        mat.base_color_texture.as_ref(),
        mat.normal_map_texture.as_ref(),
        mat.metallic_roughness_texture.as_ref(),
        mat.emissive_texture.as_ref(),
        mat.occlusion_texture.as_ref(),
        mat.depth_map.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        out.push(slot.clone());
    }
}

/// Observer: drop the closed tab's handle set when the editor fires
/// `TabClosed`. Bevy's per-handle refcount handles cross-tab sharing —
/// if another tab still references the same path, its own handle keeps
/// the asset alive and only the closed tab's claim is released.
pub fn evict_closed_tab(trigger: On<TabClosed>, mut cache: ResMut<TabAssetCache>) {
    cache.drop_tab(trigger.event().tab_id);
}

// ============================================================================
// Scene Diagnostics — snapshot for the panel
// ============================================================================

/// Material / texture asset health. `images_missing > 0` is the red
/// flag — it means a texture handle is pointing at an evicted asset
/// (the textures-vanish-on-tab-switch bug).
#[derive(Default, Clone)]
pub struct MaterialDiagnostics {
    pub entities_with_std_mat: usize,
    pub unique_std_mats: usize,
    pub mats_loaded: usize,
    pub mats_with_no_textures: usize,
    pub image_handles_seen: usize,
    pub images_alive: usize,
    pub images_missing: usize,
    pub missing_sample_paths: Vec<String>,
}

/// Per-asset-type counts from each `Assets<T>` resource. Cheap — each
/// is a single `len()` call. `None` means the resource isn't
/// registered in this build (e.g. CodeShaderMaterial when
/// renzora_shader's code-material feature is off).
#[derive(Default, Clone)]
pub struct AssetInventory {
    pub images: usize,
    pub meshes: usize,
    pub standard_materials: usize,
    pub graph_materials: Option<usize>,
    pub code_shader_materials: Option<usize>,
    pub scenes: usize,
    pub gltfs: usize,
    pub shaders: usize,
    pub animation_clips: usize,
    pub audio_sources: Option<usize>,
}

/// Counts of entity-level problems that wouldn't crash but suggest
/// something's off. All defaults are 0; anything > 0 is worth a look.
#[derive(Default, Clone)]
pub struct EntityHealth {
    pub total_entities: usize,
    /// Entities with `Mesh3d` but no `MeshMaterial3d` of any tracked
    /// type (Standard / Graph / CodeShader). They render with the
    /// default white fallback.
    pub mesh3d_without_material: usize,
    /// Entities with `MaterialRef` but no `MaterialResolved` marker —
    /// MaterialResolver hasn't finished binding them yet (or failed).
    /// Transient at scene-load time; lingering means the resolver
    /// gave up.
    pub materialref_unresolved: usize,
    /// Entities still carrying `PendingMeshInstanceRehydrate` — their
    /// GLTF asset hasn't landed yet. Transient on scene-load;
    /// persistent means the GLTF failed to load and nothing flipped
    /// them to `MeshInstanceLoadFailed`.
    pub pending_rehydrate: usize,
    /// `SceneRoot` entities with no `Children` — bevy_scene didn't
    /// populate them, usually because the underlying `Scene` asset
    /// hasn't loaded yet OR the load failed silently.
    pub empty_scene_roots: usize,
    /// B0004: child entities with `GlobalTransform` whose parent has
    /// no `GlobalTransform`. Bevy logs this per-frame; we just count.
    pub b0004_violations: usize,
}

/// One row per `Camera` entity in the world. Surfaces every wiring
/// surface that's bitten us before: prepass attachments, atmosphere
/// bind-group slots, render target redirection.
#[derive(Clone)]
pub struct CameraEntry {
    pub entity: Entity,
    pub name: String,
    pub is_active: bool,
    pub is_3d: bool,
    pub is_2d: bool,
    pub render_target: CameraRenderTarget,
    pub hdr: bool,
    pub normal_prepass: bool,
    pub depth_prepass: bool,
    pub motion_prepass: bool,
    pub atmosphere: bool,
    pub atmosphere_env_light: bool,
    pub env_map_light: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub enum CameraRenderTarget {
    Window,
    Image,
    TextureView,
    /// `RenderTarget::None` or any future variant we don't recognise —
    /// camera is wired up but explicitly draws nowhere. Surfacing it
    /// because a misconfigured camera that silently renders nothing is
    /// a real footgun.
    NoTarget,
}

impl CameraRenderTarget {
    pub fn label(self) -> &'static str {
        match self {
            CameraRenderTarget::Window => "window",
            CameraRenderTarget::Image => "image",
            CameraRenderTarget::TextureView => "texture-view",
            CameraRenderTarget::NoTarget => "none",
        }
    }
}

/// The whole panel's input data, refreshed every Update by
/// [`update_scene_diag_snapshot`].
#[derive(Resource, Default)]
pub struct SceneDiagSnapshot {
    pub material: MaterialDiagnostics,
    pub assets: AssetInventory,
    pub entities: EntityHealth,
    pub cameras: Vec<CameraEntry>,
}

/// One-shot exclusive-world walk that fills every section of
/// [`SceneDiagSnapshot`]. Exclusive because we need ~a dozen
/// independent queries + several `Assets<T>` lookups; cheaper to do
/// it all here than to fan out into a dozen parallel systems whose
/// scheduling overhead would dwarf the work.
///
/// Runs every frame in editor state. Costs scale with entity count
/// and material count — measured at ~hundreds of µs even on Sponza,
/// well inside budget for a debug-only panel that the user can hide.
pub fn update_scene_diag_snapshot(world: &mut World) {
    use bevy::ecs::system::SystemState;
    use bevy::pbr::Atmosphere;
    use bevy::light::AtmosphereEnvironmentMapLight;
    use bevy::core_pipeline::prepass::{DepthPrepass, MotionVectorPrepass, NormalPrepass};
    use bevy::render::view::Hdr;
    use bevy::camera::RenderTarget;
    use renzora_shader::material::GraphMaterial;
    use renzora_shader::material::resolver::MaterialResolved;
    use renzora_shader::runtime::CodeShaderMaterial;

    // ── Material section: pull the typed handles via a query, then
    // dereference Assets<>. Need StandardMaterial and Image *after*
    // the query borrow drops, so collect first.
    let std_handles: Vec<Handle<StandardMaterial>> = {
        let mut state = SystemState::<Query<
            &MeshMaterial3d<StandardMaterial>,
            (Without<EditorCamera>, Without<HideInHierarchy>),
        >>::new(world);
        let q = state.get(world);
        q.iter().map(|m| m.0.clone()).collect()
    };

    let mut material = MaterialDiagnostics {
        entities_with_std_mat: std_handles.len(),
        ..Default::default()
    };
    let unique_std: std::collections::HashMap<
        bevy::asset::AssetId<StandardMaterial>,
        Handle<StandardMaterial>,
    > = std_handles.iter().map(|h| (h.id(), h.clone())).collect();
    material.unique_std_mats = unique_std.len();

    if let (Some(mats), Some(images)) = (
        world.get_resource::<Assets<StandardMaterial>>(),
        world.get_resource::<Assets<Image>>(),
    ) {
        let mut sample_missing: Vec<bevy::asset::AssetId<Image>> = Vec::new();
        for h in unique_std.values() {
            let Some(mat) = mats.get(h) else { continue };
            material.mats_loaded += 1;
            let mut had_handle = false;
            for img_h in [
                mat.base_color_texture.as_ref(),
                mat.normal_map_texture.as_ref(),
                mat.metallic_roughness_texture.as_ref(),
                mat.emissive_texture.as_ref(),
                mat.occlusion_texture.as_ref(),
                mat.depth_map.as_ref(),
            ]
            .into_iter()
            .flatten()
            {
                had_handle = true;
                material.image_handles_seen += 1;
                if images.get(img_h).is_some() {
                    material.images_alive += 1;
                } else {
                    material.images_missing += 1;
                    if sample_missing.len() < 5 {
                        sample_missing.push(img_h.id());
                    }
                }
            }
            if !had_handle {
                material.mats_with_no_textures += 1;
            }
        }
        if !sample_missing.is_empty() {
            if let Some(server) = world.get_resource::<AssetServer>() {
                material.missing_sample_paths = sample_missing
                    .iter()
                    .map(|id| {
                        server
                            .get_path(*id)
                            .map(|p| p.to_string())
                            .unwrap_or_else(|| format!("<unknown id {:?}>", id))
                    })
                    .collect();
            }
        }
    }

    // ── Asset inventory: each `Assets<T>::len()` call.
    let assets = AssetInventory {
        images: world.get_resource::<Assets<Image>>().map_or(0, Assets::len),
        meshes: world.get_resource::<Assets<Mesh>>().map_or(0, Assets::len),
        standard_materials: world
            .get_resource::<Assets<StandardMaterial>>()
            .map_or(0, Assets::len),
        graph_materials: world.get_resource::<Assets<GraphMaterial>>().map(Assets::len),
        code_shader_materials: world
            .get_resource::<Assets<CodeShaderMaterial>>()
            .map(Assets::len),
        scenes: world.get_resource::<Assets<Scene>>().map_or(0, Assets::len),
        gltfs: world.get_resource::<Assets<Gltf>>().map_or(0, Assets::len),
        shaders: world.get_resource::<Assets<Shader>>().map_or(0, Assets::len),
        animation_clips: world
            .get_resource::<Assets<AnimationClip>>()
            .map_or(0, Assets::len),
        // Audio source asset type is plugin-dependent; expose as Option so
        // it shows "n/a" when the audio plugin isn't loaded yet.
        audio_sources: world
            .get_resource::<Assets<bevy::audio::AudioSource>>()
            .map(Assets::len),
    };

    // ── Entity health: a batch of small queries via SystemState.
    let entities = {
        type EntityHealthParams<'w, 's> = (
            // Total entities (no filter).
            Query<'w, 's, Entity>,
            // Mesh3d with no material binding of any tracked type.
            Query<
                'w,
                's,
                Entity,
                (
                    With<Mesh3d>,
                    Without<MeshMaterial3d<StandardMaterial>>,
                    Without<MeshMaterial3d<GraphMaterial>>,
                    Without<MeshMaterial3d<CodeShaderMaterial>>,
                ),
            >,
            // MaterialRef without resolver marker.
            Query<
                'w,
                's,
                Entity,
                (
                    With<renzora::core::MaterialRef>,
                    Without<MaterialResolved>,
                ),
            >,
            // Pending GLTF rehydrate still in flight.
            Query<
                'w,
                's,
                Entity,
                With<renzora_engine::scene_io::PendingMeshInstanceRehydrate>,
            >,
            // Empty SceneRoots (bevy_scene didn't fill them).
            Query<'w, 's, Entity, (With<SceneRoot>, Without<Children>)>,
            // For B0004: every child with GlobalTransform → check parent
            // also has GlobalTransform.
            Query<'w, 's, (Entity, &'static ChildOf), With<GlobalTransform>>,
            // Lookup query to validate the parent has GlobalTransform.
            Query<'w, 's, &'static GlobalTransform>,
        );

        let mut state = SystemState::<EntityHealthParams>::new(world);
        let (all_q, mesh_no_mat_q, unresolved_q, pending_q, empty_root_q, gt_children_q, gt_lookup_q) =
            state.get(world);

        let mut h = EntityHealth {
            total_entities: all_q.iter().count(),
            mesh3d_without_material: mesh_no_mat_q.iter().count(),
            materialref_unresolved: unresolved_q.iter().count(),
            pending_rehydrate: pending_q.iter().count(),
            empty_scene_roots: empty_root_q.iter().count(),
            b0004_violations: 0,
        };
        for (_e, child_of) in gt_children_q.iter() {
            if gt_lookup_q.get(child_of.parent()).is_err() {
                h.b0004_violations += 1;
            }
        }
        h
    };

    // ── Cameras: one row per Camera entity in the world. `RenderTarget`
    // is its own component in Bevy 0.18 (not a `Camera` field) — query
    // it as an `Option<&RenderTarget>` so cameras rendering to the
    // default window (no explicit RT component) still show up.
    let cameras = {
        type CameraParams<'w, 's> = Query<
            'w,
            's,
            (
                Entity,
                Option<&'static Name>,
                &'static Camera,
                Option<&'static RenderTarget>,
                Has<Camera3d>,
                Has<Camera2d>,
                Has<Hdr>,
                Has<NormalPrepass>,
                Has<DepthPrepass>,
                Has<MotionVectorPrepass>,
                Has<Atmosphere>,
                Has<AtmosphereEnvironmentMapLight>,
                Has<bevy::light::EnvironmentMapLight>,
            ),
        >;
        let mut state = SystemState::<CameraParams>::new(world);
        let q = state.get(world);
        q.iter()
            .map(
                |(
                    entity,
                    name,
                    cam,
                    rt,
                    is_3d,
                    is_2d,
                    hdr,
                    nprep,
                    dprep,
                    mprep,
                    atmo,
                    atmo_env,
                    env_map,
                )| {
                    let render_target = match rt {
                        None => CameraRenderTarget::Window,
                        Some(RenderTarget::Window(_)) => CameraRenderTarget::Window,
                        Some(RenderTarget::Image(_)) => CameraRenderTarget::Image,
                        Some(RenderTarget::TextureView(_)) => CameraRenderTarget::TextureView,
                        Some(_) => CameraRenderTarget::NoTarget,
                    };
                    CameraEntry {
                        entity,
                        name: name
                            .map(|n| n.as_str().to_string())
                            .unwrap_or_else(|| format!("<unnamed {:?}>", entity)),
                        is_active: cam.is_active,
                        is_3d,
                        is_2d,
                        render_target,
                        hdr,
                        normal_prepass: nprep,
                        depth_prepass: dprep,
                        motion_prepass: mprep,
                        atmosphere: atmo,
                        atmosphere_env_light: atmo_env,
                        env_map_light: env_map,
                    }
                },
            )
            .collect()
    };

    let mut snap = world.resource_mut::<SceneDiagSnapshot>();
    snap.material = material;
    snap.assets = assets;
    snap.entities = entities;
    snap.cameras = cameras;
}

/// Convenience accessor used by diagnostic/debug tooling — reports how
/// many distinct paths the cache currently pins, and how many tabs.
#[allow(dead_code)]
pub fn cache_stats(cache: &TabAssetCache) -> (usize, usize) {
    let tab_count = cache.by_tab.len();
    let mut paths: HashSet<&str> = HashSet::new();
    for tab in cache.by_tab.values() {
        for path in tab.gltfs.keys() {
            paths.insert(path.as_str());
        }
    }
    (tab_count, paths.len())
}
