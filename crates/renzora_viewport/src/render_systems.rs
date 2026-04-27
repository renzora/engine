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

use bevy::prelude::*;
use bevy::pbr::{Material, MeshMaterial3d, wireframe::WireframeConfig};
use std::collections::HashMap;
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
        Self { map: HashMap::new(), _p: PhantomData }
    }
}

/// Per-material-type last applied debug mode. `None` = not swapped.
#[derive(Resource)]
pub struct LastVizState<M: Asset> {
    mode: Option<f32>,
    _p: PhantomData<M>,
}
impl<M: Asset> Default for LastVizState<M> {
    fn default() -> Self { Self { mode: None, _p: PhantomData } }
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
/// `include_lighting`: when true, `!lighting` also triggers the flat-clay swap.
/// Pass `false` for `StandardMaterial` (lighting is handled via `unlit` mutation)
/// and `true` for custom materials whose shaders don't expose an unlit flag.
fn desired_mode_inner(settings: &ViewportSettings, include_lighting: bool) -> Option<f32> {
    // Mesh hidden — let the standard-material pass discard its pixels instead
    // of swapping in the (still-solid) debug shader.
    if !settings.render_toggles.mesh {
        return None;
    }
    if let Some(m) = viz_to_mode_index(settings.visualization_mode) {
        return Some(m);
    }
    if !settings.render_toggles.textures {
        return Some(MODE_FLAT_CLAY);
    }
    if include_lighting && !settings.render_toggles.lighting {
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
    mut material_events: MessageReader<AssetEvent<StandardMaterial>>,
) {
    let toggles = settings.render_toggles;
    let toggles_changed = last_state.toggles != Some(toggles);

    let new_materials = material_events
        .read()
        .any(|e| matches!(e, AssetEvent::Added { .. }));

    // When swap path is active, StandardMaterial is handled by the debug
    // shader — don't mutate it here. Swap is active if viz is set OR
    // textures toggle is off. Mesh-off bypasses the swap so we can discard
    // pixels via the standard pipeline.
    let swap_active = toggles.mesh
        && (settings.visualization_mode != VisualizationMode::None || !toggles.textures);
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
                    original_states.states.insert(*id, MaterialState::capture(m));
                }
            }
        }
        for (_, m) in materials.iter_mut() {
            m.base_color = Color::NONE;
            m.alpha_mode = AlphaMode::Mask(0.5);
        }
        return;
    }

    if !toggles_changed && !(new_materials && !is_default && !swap_active) {
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
                original_states.states.insert(*id, MaterialState::capture(m));
            }
        }
    }

    if is_default {
        for (id, state) in &original_states.states {
            if let Some(m) = materials.get_mut(*id) {
                state.apply_to(m);
            }
        }
        return;
    }

    for id in ids {
        let original = original_states.states.get(&id).cloned();
        let Some(material) = materials.get_mut(id) else { continue };
        if let Some(ref orig) = original {
            orig.apply_to(material);
        }
        if !toggles.lighting {
            material.unlit = true;
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
    q_swapped: &Query<(Entity, &ViewportDebugBackup<M>), With<MeshMaterial3d<ViewportDebugMaterial>>>,
    desired_fn: fn(&ViewportSettings) -> Option<f32>,
) {
    let desired = desired_fn(settings);
    let changed = last_viz.mode != desired;

    let Some(mode_idx) = desired else {
        // Unswap everything if we were swapped.
        if changed {
            last_viz.mode = None;
            for (entity, backup) in q_swapped.iter() {
                commands.entity(entity)
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
        for (_src_id, dbg_handle) in &cache.map {
            if let Some(dbg) = debug_materials.get_mut(dbg_handle) {
                dbg.params.config.x = mode_idx;
            }
        }
    }

    for (entity, mm) in q_src.iter() {
        let src_handle = mm.0.clone();
        let src_id = src_handle.id();

        let dbg_handle = cache.map.entry(src_id).or_insert_with(|| {
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
        }).clone();

        if let Some(dbg) = debug_materials.get_mut(&dbg_handle) {
            dbg.params.config.x = mode_idx;
        }

        commands.entity(entity)
            .insert(ViewportDebugBackup::<M>(src_handle))
            .insert(MeshMaterial3d(dbg_handle))
            .remove::<MeshMaterial3d<M>>();
    }
}

/// Viz-swap system for `StandardMaterial` — lighting is handled via the
/// `unlit` flag mutation in `update_render_toggles`, so the swap only fires
/// for viz modes and `!textures`.
pub fn apply_visualization_mode_for<M: Material>(
    mut commands: Commands,
    settings: Res<ViewportSettings>,
    mut last_viz: ResMut<LastVizState<M>>,
    mut cache: ResMut<DebugMaterialCache<M>>,
    mut debug_materials: ResMut<Assets<ViewportDebugMaterial>>,
    source_materials: Res<Assets<M>>,
    q_src: Query<(Entity, &MeshMaterial3d<M>), Without<ViewportDebugBackup<M>>>,
    q_swapped: Query<(Entity, &ViewportDebugBackup<M>), With<MeshMaterial3d<ViewportDebugMaterial>>>,
) {
    apply_swap_generic(&mut commands, &settings, &mut last_viz, &mut cache,
        &mut debug_materials, &source_materials, &q_src, &q_swapped, desired_mode);
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
    q_swapped: Query<(Entity, &ViewportDebugBackup<M>), With<MeshMaterial3d<ViewportDebugMaterial>>>,
) {
    apply_swap_generic(&mut commands, &settings, &mut last_viz, &mut cache,
        &mut debug_materials, &source_materials, &q_src, &q_swapped, desired_mode_custom);
}

// ── System: shadow toggle ───────────────────────────────────────────────────

pub fn update_shadow_settings(
    settings: Res<ViewportSettings>,
    mut directional_lights: Query<&mut DirectionalLight>,
    mut point_lights: Query<&mut PointLight>,
    mut spot_lights: Query<&mut SpotLight>,
) {
    if !settings.is_changed() {
        return;
    }
    let enabled = settings.render_toggles.shadows;
    for mut light in directional_lights.iter_mut() {
        light.shadows_enabled = enabled;
    }
    for mut light in point_lights.iter_mut() {
        light.shadows_enabled = enabled;
    }
    for mut light in spot_lights.iter_mut() {
        light.shadows_enabled = enabled;
    }
}
