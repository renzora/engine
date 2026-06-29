//! Systems that apply viewport render toggles and visualization modes.
//!
//! Two mechanisms:
//!   - Textures / lighting / wireframe / shadows: mutate [`StandardMaterial`]
//!     directly. Only covers StandardMaterial — other material types (terrain,
//!     foliage) ignore these toggles.
//!   - Visualization modes: swap each mesh's material for a
//!     [`ViewportDebugMaterial`] driven by `viewport_debug.wgsl`. Works with
//!     any source material type through the generic
//!     [`apply_visualization_mode_for<M>`] system.

use bevy::pbr::{wireframe::WireframeConfig, Material, MeshMaterial3d};
use bevy::prelude::*;
use renzora::core::EditorCamera;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;

use crate::debug_material::{DebugParams, ViewportDebugMaterial};
use crate::settings::{RenderToggles, ViewportSettings, VisualizationMode};

// ── Standard-material toggle state (textures/lighting) ──────────────────────

#[derive(Resource, Default)]
pub struct OriginalMaterialStates {
    states: HashMap<AssetId<StandardMaterial>, MaterialState>,
}

#[derive(Clone)]
struct MaterialState {
    unlit: bool,
    base_color: Color,
    base_color_texture: Option<Handle<Image>>,
    emissive: LinearRgba,
    emissive_texture: Option<Handle<Image>>,
    normal_map_texture: Option<Handle<Image>>,
    metallic_roughness_texture: Option<Handle<Image>>,
    occlusion_texture: Option<Handle<Image>>,
    alpha_mode: AlphaMode,
    metallic: f32,
    perceptual_roughness: f32,
}

impl MaterialState {
    fn capture(m: &StandardMaterial) -> Self {
        Self {
            unlit: m.unlit,
            base_color: m.base_color,
            base_color_texture: m.base_color_texture.clone(),
            emissive: m.emissive,
            emissive_texture: m.emissive_texture.clone(),
            normal_map_texture: m.normal_map_texture.clone(),
            metallic_roughness_texture: m.metallic_roughness_texture.clone(),
            occlusion_texture: m.occlusion_texture.clone(),
            alpha_mode: m.alpha_mode,
            metallic: m.metallic,
            perceptual_roughness: m.perceptual_roughness,
        }
    }

    fn apply_to(&self, m: &mut StandardMaterial) {
        m.unlit = self.unlit;
        m.base_color = self.base_color;
        m.base_color_texture = self.base_color_texture.clone();
        m.emissive = self.emissive;
        m.emissive_texture = self.emissive_texture.clone();
        m.normal_map_texture = self.normal_map_texture.clone();
        m.metallic_roughness_texture = self.metallic_roughness_texture.clone();
        m.occlusion_texture = self.occlusion_texture.clone();
        m.alpha_mode = self.alpha_mode;
        m.metallic = self.metallic;
        m.perceptual_roughness = self.perceptual_roughness;
    }
}

#[derive(Resource, Default)]
pub struct LastToggleState {
    toggles: Option<RenderToggles>,
}

/// Generated grey checkerboard used as the base-color texture when the Textures
/// toggle is off. It mirrors the dark/light grey checker terrain shows when a
/// chunk has no material ([`TerrainCheckerboardMaterial::default`]), so an
/// untextured `StandardMaterial` reads the same way — a recognizable "no
/// material" default rather than a flat grey fill. Built once at startup.
#[derive(Resource)]
pub struct DefaultCheckerTexture(pub Handle<Image>);

impl FromWorld for DefaultCheckerTexture {
    fn from_world(world: &mut World) -> Self {
        let image = build_checker_image();
        let mut images = world.resource_mut::<Assets<Image>>();
        Self(images.add(image))
    }
}

/// Encode a linear grey value to the sRGB bytes an `Rgba8UnormSrgb` texture
/// decodes back to that same linear value on sample (so lighting matches the
/// terrain checker, whose colors are specified in linear space).
fn checker_srgb_bytes(linear: f32) -> [u8; 4] {
    let s = Srgba::from(LinearRgba::new(linear, linear, linear, 1.0));
    [
        (s.red * 255.0).round() as u8,
        (s.green * 255.0).round() as u8,
        (s.blue * 255.0).round() as u8,
        255,
    ]
}

/// Bake the terrain-default grey checkerboard into a small point-sampled,
/// repeating texture. Tiles crisply across a mesh's UVs (and wraps for UVs
/// outside 0..1), the closest `StandardMaterial` equivalent of terrain's
/// world-space procedural checker.
fn build_checker_image() -> Image {
    use bevy::asset::RenderAssetUsages;
    use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    // TerrainCheckerboardMaterial::default() colors (linear grey).
    let a = checker_srgb_bytes(0.32);
    let b = checker_srgb_bytes(0.22);
    const CELLS: usize = 2; // checker squares per axis across one UV tile
    const SIZE: usize = CELLS * 2; // 2 px/cell — point-sampled, stays sharp
    let mut data = Vec::with_capacity(SIZE * SIZE * 4);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let on = ((x / 2) + (y / 2)) % 2 == 0;
            data.extend_from_slice(if on { &a } else { &b });
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: SIZE as u32,
            height: SIZE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Nearest,
        min_filter: ImageFilterMode::Nearest,
        ..default()
    });
    image
}

// ── Debug material swap state (visualization modes) ─────────────────────────

/// Per-entity backup of the original `MeshMaterial3d<M>` handle, parameterized
/// by the source material type so many types can coexist.
#[derive(Component)]
pub struct ViewportDebugBackup<M: Asset>(pub Handle<M>);

/// Per-material-type cache of ViewportDebugMaterial handles.
#[derive(Resource)]
pub struct DebugMaterialCache<M: Asset> {
    map: HashMap<AssetId<M>, Handle<ViewportDebugMaterial>>,
    _p: PhantomData<M>,
}
impl<M: Asset> Default for DebugMaterialCache<M> {
    fn default() -> Self {
        Self {
            map: HashMap::new(),
            _p: PhantomData,
        }
    }
}

/// Per-material-type last applied debug mode. `None` = not swapped.
#[derive(Resource)]
pub struct LastVizState<M: Asset> {
    mode: Option<f32>,
    _p: PhantomData<M>,
}
impl<M: Asset> Default for LastVizState<M> {
    fn default() -> Self {
        Self {
            mode: None,
            _p: PhantomData,
        }
    }
}

const MODE_FLAT_CLAY: f32 = 5.0;

fn viz_to_mode_index(v: VisualizationMode) -> Option<f32> {
    match v {
        VisualizationMode::None => None,
        VisualizationMode::Normals => Some(0.0),
        VisualizationMode::Roughness => Some(1.0),
        VisualizationMode::Metallic => Some(2.0),
        VisualizationMode::Depth => Some(3.0),
        VisualizationMode::UvChecker => Some(4.0),
    }
}

/// Pick the debug-shader mode for the current settings, or None to unswap.
/// `is_custom`: custom materials (terrain, foliage, …) can't have their texture
/// maps stripped or be flipped to `unlit` the way `StandardMaterial` can, so for
/// them `!textures` / `!lighting` fall back to the flat-clay debug swap. For
/// `StandardMaterial` (`is_custom = false`) both are handled in
/// `update_render_toggles` by mutating the material in place — stripping its
/// texture maps while leaving it lit — so real scene lighting + shadows survive a
/// textures-off view instead of being replaced by the unlit, shadowless clay
/// shader. (Textures-off used to swap StandardMaterial to flat clay too, which is
/// why turning textures off looked like it also turned lighting + shadows off.)
fn desired_mode_inner(settings: &ViewportSettings, is_custom: bool) -> Option<f32> {
    // Mesh hidden — let the standard-material pass discard its pixels instead
    // of swapping in the (still-solid) debug shader.
    if !settings.render_toggles.mesh {
        return None;
    }
    if let Some(m) = viz_to_mode_index(settings.visualization_mode) {
        return Some(m);
    }
    if is_custom && (!settings.render_toggles.textures || !settings.render_toggles.lighting) {
        return Some(MODE_FLAT_CLAY);
    }
    None
}

fn desired_mode(settings: &ViewportSettings) -> Option<f32> {
    desired_mode_inner(settings, false)
}

fn desired_mode_custom(settings: &ViewportSettings) -> Option<f32> {
    desired_mode_inner(settings, true)
}

fn default_debug_params(mode: f32) -> DebugParams {
    DebugParams {
        config: Vec4::new(mode, 0.5, 0.0, 0.0),
        extra: Vec4::new(0.1, 200.0, 16.0, 0.0),
    }
}

fn standard_debug_params(mode: f32, src: &StandardMaterial) -> DebugParams {
    let has_mr = src.metallic_roughness_texture.is_some();
    DebugParams {
        config: Vec4::new(
            mode,
            src.perceptual_roughness,
            src.metallic,
            if has_mr { 1.0 } else { 0.0 },
        ),
        extra: Vec4::new(0.1, 200.0, 16.0, 0.0),
    }
}

// ── System: toggle standard-material render flags (textures/lighting) ──────

pub fn update_render_toggles(
    settings: Res<ViewportSettings>,
    mut wireframe_config: ResMut<WireframeConfig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut original_states: ResMut<OriginalMaterialStates>,
    mut last_state: ResMut<LastToggleState>,
    checker: Res<DefaultCheckerTexture>,
    mut material_events: MessageReader<AssetEvent<StandardMaterial>>,
) {
    let toggles = settings.render_toggles;
    let toggles_changed = last_state.toggles != Some(toggles);

    let new_materials = material_events
        .read()
        .any(|e| matches!(e, AssetEvent::Added { .. }));

    // When the viz-swap path is active, StandardMaterial is handled by the debug
    // shader — don't mutate it here. Only the visualization modes swap now;
    // textures-off is handled below by stripping the material's texture maps in
    // place so it stays lit + shadowed. Mesh-off bypasses the swap so we can
    // discard pixels via the standard pipeline.
    let swap_active = toggles.mesh && settings.visualization_mode != VisualizationMode::None;
    let is_default = toggles.mesh && toggles.textures && toggles.lighting && !toggles.wireframe;

    // Mesh-hidden path: discard every StandardMaterial fragment via alpha mask.
    // Wireframe is rendered by a separate pipeline that picks up Mesh3d, so it
    // remains visible when mesh is off.
    if !toggles.mesh {
        if !toggles_changed && !new_materials {
            return;
        }
        last_state.toggles = Some(toggles);
        wireframe_config.global = toggles.wireframe;

        let ids: Vec<AssetId<StandardMaterial>> = materials.iter().map(|(id, _)| id).collect();
        for id in &ids {
            if !original_states.states.contains_key(id) {
                if let Some(m) = materials.get(*id) {
                    original_states
                        .states
                        .insert(*id, MaterialState::capture(m));
                }
            }
        }
        for (_, m) in materials.iter_mut() {
            m.base_color = Color::NONE;
            m.alpha_mode = AlphaMode::Mask(0.5);
        }
        return;
    }

    if (swap_active || is_default || !new_materials) && !toggles_changed {
        return;
    }

    if toggles_changed {
        last_state.toggles = Some(toggles);
        wireframe_config.global = toggles.wireframe;
    }

    if swap_active {
        return;
    }

    let ids: Vec<AssetId<StandardMaterial>> = materials.iter().map(|(id, _)| id).collect();
    for id in &ids {
        if !original_states.states.contains_key(id) {
            if let Some(m) = materials.get(*id) {
                original_states
                    .states
                    .insert(*id, MaterialState::capture(m));
            }
        }
    }

    if is_default {
        for (id, state) in &original_states.states {
            if let Some(mut m) = materials.get_mut(*id) {
                state.apply_to(&mut m);
            }
        }
        return;
    }

    for id in ids {
        let original = original_states.states.get(&id).cloned();
        let Some(mut material) = materials.get_mut(id) else {
            continue;
        };
        if let Some(ref orig) = original {
            orig.apply_to(&mut material);
        }
        if !toggles.lighting {
            material.unlit = true;
        }
        if !toggles.textures {
            // Textures-off = the default "no material" checker, STILL lit by the
            // real scene lights and receiving shadows. Swap the base-color map for
            // the generated grey checker (matching terrain's untextured look) and
            // drop the other maps; leave `unlit` driven solely by the lighting
            // toggle above so shading + shadows are unaffected. (Without this,
            // textures-off swapped the whole mesh to the unlit flat-clay shader,
            // which read as lighting + shadows being turned off too.) Mirror the
            // terrain checker's matte PBR (non-metallic, fairly rough) so every
            // untextured surface reads consistently regardless of its material.
            material.base_color = Color::WHITE;
            material.base_color_texture = Some(checker.0.clone());
            material.emissive = LinearRgba::BLACK;
            material.emissive_texture = None;
            material.normal_map_texture = None;
            material.metallic = 0.0;
            material.metallic_roughness_texture = None;
            material.perceptual_roughness = 0.8;
            material.occlusion_texture = None;
        }
    }
}

// ── Generic viz-mode swap system (works for any Material type) ─────────────

fn apply_swap_generic<M: Material>(
    commands: &mut Commands,
    settings: &ViewportSettings,
    last_viz: &mut LastVizState<M>,
    cache: &mut DebugMaterialCache<M>,
    debug_materials: &mut Assets<ViewportDebugMaterial>,
    source_materials: &Assets<M>,
    q_src: &Query<(Entity, &MeshMaterial3d<M>), Without<ViewportDebugBackup<M>>>,
    q_swapped: &Query<
        (Entity, &ViewportDebugBackup<M>),
        With<MeshMaterial3d<ViewportDebugMaterial>>,
    >,
    desired_fn: fn(&ViewportSettings) -> Option<f32>,
) {
    let desired = desired_fn(settings);
    let changed = last_viz.mode != desired;

    let Some(mode_idx) = desired else {
        // Unswap everything if we were swapped.
        if changed {
            last_viz.mode = None;
            for (entity, backup) in q_swapped.iter() {
                commands
                    .entity(entity)
                    .insert(MeshMaterial3d(backup.0.clone()))
                    .remove::<ViewportDebugBackup<M>>()
                    .remove::<MeshMaterial3d<ViewportDebugMaterial>>();
            }
        }
        return;
    };

    // Mode changed — update cached debug-material uniforms.
    if changed {
        last_viz.mode = Some(mode_idx);
        for dbg_handle in cache.map.values() {
            if let Some(mut dbg) = debug_materials.get_mut(dbg_handle) {
                dbg.params.config.x = mode_idx;
            }
        }
    }

    for (entity, mm) in q_src.iter() {
        let src_handle = mm.0.clone();
        let src_id = src_handle.id();

        let dbg_handle = cache
            .map
            .entry(src_id)
            .or_insert_with(|| {
                let mut dbg = ViewportDebugMaterial::default();
                // If this is a StandardMaterial we can fill scalar roughness/metallic
                // and the MR texture; for other materials those stay defaults and the
                // shader falls back to scalar values.
                let src_any = source_materials.get(src_id);
                let any_mat: Option<&dyn std::any::Any> = src_any.map(|m| m as &dyn std::any::Any);
                if let Some(std_mat) = any_mat.and_then(|a| a.downcast_ref::<StandardMaterial>()) {
                    dbg.params = standard_debug_params(mode_idx, std_mat);
                    dbg.mr_texture = std_mat.metallic_roughness_texture.clone();
                } else {
                    dbg.params = default_debug_params(mode_idx);
                }
                debug_materials.add(dbg)
            })
            .clone();

        if let Some(mut dbg) = debug_materials.get_mut(&dbg_handle) {
            dbg.params.config.x = mode_idx;
        }

        commands
            .entity(entity)
            .insert(ViewportDebugBackup::<M>(src_handle))
            .insert(MeshMaterial3d(dbg_handle))
            .remove::<MeshMaterial3d<M>>();
    }
}

/// Viz-swap system for `StandardMaterial` — textures-off and lighting are both
/// handled by mutating the material in place in `update_render_toggles` (strip
/// texture maps / set `unlit`), so this swap only fires for visualization modes.
pub fn apply_visualization_mode_for<M: Material>(
    mut commands: Commands,
    settings: Res<ViewportSettings>,
    mut last_viz: ResMut<LastVizState<M>>,
    mut cache: ResMut<DebugMaterialCache<M>>,
    mut debug_materials: ResMut<Assets<ViewportDebugMaterial>>,
    source_materials: Res<Assets<M>>,
    q_src: Query<(Entity, &MeshMaterial3d<M>), Without<ViewportDebugBackup<M>>>,
    q_swapped: Query<
        (Entity, &ViewportDebugBackup<M>),
        With<MeshMaterial3d<ViewportDebugMaterial>>,
    >,
) {
    apply_swap_generic(
        &mut commands,
        &settings,
        &mut last_viz,
        &mut cache,
        &mut debug_materials,
        &source_materials,
        &q_src,
        &q_swapped,
        desired_mode,
    );
}

/// Viz-swap system for custom materials (terrain, foliage, etc.). Also fires
/// on `!lighting`, because those shaders don't expose an unlit flag.
pub fn apply_visualization_mode_for_custom<M: Material>(
    mut commands: Commands,
    settings: Res<ViewportSettings>,
    mut last_viz: ResMut<LastVizState<M>>,
    mut cache: ResMut<DebugMaterialCache<M>>,
    mut debug_materials: ResMut<Assets<ViewportDebugMaterial>>,
    source_materials: Res<Assets<M>>,
    q_src: Query<(Entity, &MeshMaterial3d<M>), Without<ViewportDebugBackup<M>>>,
    q_swapped: Query<
        (Entity, &ViewportDebugBackup<M>),
        With<MeshMaterial3d<ViewportDebugMaterial>>,
    >,
) {
    apply_swap_generic(
        &mut commands,
        &settings,
        &mut last_viz,
        &mut cache,
        &mut debug_materials,
        &source_materials,
        &q_src,
        &q_swapped,
        desired_mode_custom,
    );
}

// ── System: shadow toggle + point-light shadow budget ───────────────────────

/// Maximum number of point lights allowed to cast real-time shadows at once.
///
/// Every shadow-casting point light re-renders the scene to six cube-map faces
/// each frame, so scenes that ship 20+ punctual lights (e.g. Sponza's lamps)
/// spend most of the GPU frame on point-light shadow maps. Like Unity/Unreal/
/// Godot, we cap the count: only the nearest `N` point lights to the editor
/// camera cast shadows; the rest stay fully lit but shadowless. Raising this
/// trades frame time for more shadowing lights.
const MAX_SHADOW_CASTING_POINT_LIGHTS: usize = 4;

/// Maximum number of directional lights allowed to cast shadows at once.
///
/// Each directional light renders the full cascade set (typically 4 splits)
/// every frame. Scenes frequently end up with two "suns" — the editor's own
/// `Sun` plus a directional light imported from a glTF — which doubles cascade
/// cost for no visual benefit. Only the brightest casts shadows.
const MAX_SHADOW_CASTING_DIRECTIONAL_LIGHTS: usize = 1;

/// Apply the viewport shadow toggle, and — when shadows are on — enforce a
/// budget so only the nearest [`MAX_SHADOW_CASTING_POINT_LIGHTS`] point lights
/// cast shadows. Runs every frame (cheap for a few dozen lights) so the budget
/// follows the camera; the per-light guard avoids re-marking lights whose
/// shadow state didn't actually change, which would thrash shadow-map alloc.
pub fn update_shadow_settings(
    camera: Query<&GlobalTransform, With<EditorCamera>>,
    settings: Res<ViewportSettings>,
    mut directional_lights: Query<(Entity, &mut DirectionalLight)>,
    mut point_lights: Query<(Entity, &GlobalTransform, &mut PointLight)>,
    mut spot_lights: Query<&mut SpotLight>,
) {
    let shadows_on = settings.render_toggles.shadows;

    // Spot lights are few; they just follow the global toggle.
    for mut light in spot_lights.iter_mut() {
        if light.shadow_maps_enabled != shadows_on {
            light.shadow_maps_enabled = shadows_on;
        }
    }

    // Directional lights: only the brightest casts shadows (avoids paying for
    // a redundant second sun's full cascade set).
    let dir_casters: HashSet<Entity> = if shadows_on {
        let mut scored: Vec<(Entity, f32)> = directional_lights
            .iter()
            .map(|(e, l)| (e, l.illuminance))
            .collect();
        // Brightest first.
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored
            .into_iter()
            .take(MAX_SHADOW_CASTING_DIRECTIONAL_LIGHTS)
            .map(|(e, _)| e)
            .collect()
    } else {
        HashSet::new()
    };
    for (entity, mut light) in directional_lights.iter_mut() {
        let want = dir_casters.contains(&entity);
        if light.shadow_maps_enabled != want {
            light.shadow_maps_enabled = want;
        }
    }

    // Point lights: pick the nearest N to the camera as shadow casters.
    let casters: HashSet<Entity> = if shadows_on {
        let cam = camera.iter().next().map(|t| t.translation());
        let mut scored: Vec<(Entity, f32)> = point_lights
            .iter()
            .map(|(e, xf, _)| {
                let d = cam
                    .map(|c| c.distance_squared(xf.translation()))
                    .unwrap_or(0.0);
                (e, d)
            })
            .collect();
        scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        scored
            .into_iter()
            .take(MAX_SHADOW_CASTING_POINT_LIGHTS)
            .map(|(e, _)| e)
            .collect()
    } else {
        HashSet::new()
    };

    for (entity, _, mut light) in point_lights.iter_mut() {
        let want = casters.contains(&entity);
        if light.shadow_maps_enabled != want {
            light.shadow_maps_enabled = want;
        }
    }
}
