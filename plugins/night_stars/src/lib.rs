//! Night Stars — a procedural starfield on a sky dome, as a native plugin.
//!
//! `NightStarsData` and `Sun` both live in the contract crate: `level_presets`
//! constructs the former when building a night sky, and this plugin reads the
//! latter to fade the field by sun elevation. Both are named by a binary and by
//! this runtime-loaded library, so both need one definition.

pub mod inspector;

use bevy::pbr::Material;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

/// Re-exported so `renzora_night_stars::NightStarsData` still resolves.
pub use renzora::NightStarsData;

// ============================================================================
// Data types
// ============================================================================


// ============================================================================
// Star Material
// ============================================================================

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct NightStarsMaterial {
    /// density, brightness, star_size, twinkle_speed
    #[uniform(0)]
    pub params_a: Vec4,
    /// twinkle_amount, horizon_fade, unused, unused
    #[uniform(1)]
    pub params_b: Vec4,
    /// Star color tint (r, g, b, unused)
    #[uniform(2)]
    pub star_color: LinearRgba,
}

impl Material for NightStarsMaterial {
    fn fragment_shader() -> ShaderRef {
        // Crate name is part of the path — `night_stars`, not
        // `renzora_night_stars`. Wrong name resolves to nothing at runtime.
        ShaderRef::Path("embedded://night_stars/night_stars.wgsl".into())
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

// ============================================================================
// Marker & State
// ============================================================================

#[derive(Component)]
pub struct NightStarsDomeMarker;

#[derive(Resource, Default)]
pub struct NightStarsState {
    pub entity: Option<Entity>,
    pub material_handle: Option<Handle<NightStarsMaterial>>,
    pub mesh_handle: Option<Handle<Mesh>>,
}

// ============================================================================
// Sync System
// ============================================================================

fn sync_night_stars(
    mut commands: Commands,
    mut state: ResMut<NightStarsState>,
    stars_query: Query<&NightStarsData>,
    camera_query: Query<&Transform, (With<Camera3d>, Without<renzora::IsolatedCamera>)>,
    // `Sun` moved to the contract crate; a plugin cannot link renzora_lighting.
    sun_query: Query<&renzora::Sun>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut star_materials: ResMut<Assets<NightStarsMaterial>>,
    has_data: Query<(), With<NightStarsData>>,
    mut removed: RemovedComponents<NightStarsData>,
) {
    let had_removals = removed.read().count() > 0;
    if had_removals && has_data.is_empty() {
        if let Some(ent) = state.entity.take() {
            commands.entity(ent).despawn();
            state.material_handle = None;
            state.mesh_handle = None;
        }
        return;
    }

    let Some(data) = stars_query.iter().next() else {
        return;
    };

    // Toggle off → tear down the dome but keep the settings component so
    // the user can re-enable in one click.
    if !data.enabled {
        if let Some(ent) = state.entity.take() {
            commands.entity(ent).despawn();
            state.material_handle = None;
            state.mesh_handle = None;
        }
        return;
    }

    let Some(camera_transform) = camera_query.iter().next() else {
        return;
    };

    let camera_pos = camera_transform.translation;

    // Sun elevation in radians for day/night fading (positive = above horizon)
    let sun_elevation = sun_query
        .iter()
        .next()
        .map(|s| s.elevation.to_radians())
        .unwrap_or(1.0); // default to daytime if no Sun component

    let params_a = Vec4::new(
        data.density,
        data.brightness,
        data.star_size,
        data.twinkle_speed,
    );
    let params_b = Vec4::new(data.twinkle_amount, data.horizon_fade, sun_elevation, 0.0);
    let star_color = LinearRgba::new(data.color.0, data.color.1, data.color.2, 1.0);

    if let Some(dome_entity) = state.entity {
        if commands.get_entity(dome_entity).is_ok() {
            if let Some(ref mat_handle) = state.material_handle {
                if let Some(mut mat) = star_materials.get_mut(mat_handle) {
                    mat.params_a = params_a;
                    mat.params_b = params_b;
                    mat.star_color = star_color;
                }
            }
            let transform = Transform::from_translation(camera_pos).with_scale(Vec3::splat(800.0));
            commands.entity(dome_entity).insert(transform);
        } else {
            state.entity = None;
            state.material_handle = None;
            state.mesh_handle = None;
        }
    }

    if state.entity.is_none() {
        let mesh_handle = meshes.add(Sphere::new(1.0).mesh().uv(64, 32));
        let material_handle = star_materials.add(NightStarsMaterial {
            params_a,
            params_b,
            star_color,
        });
        let transform = Transform::from_translation(camera_pos).with_scale(Vec3::splat(800.0));

        let dome_entity = commands
            .spawn((
                Mesh3d(mesh_handle.clone()),
                MeshMaterial3d(material_handle.clone()),
                transform,
                NightStarsDomeMarker,
                bevy::light::NotShadowCaster,
                bevy::light::NotShadowReceiver,
            ))
            .id();

        state.entity = Some(dome_entity);
        state.mesh_handle = Some(mesh_handle);
        state.material_handle = Some(material_handle);
    }
}

// ============================================================================
// Plugin
// ============================================================================

#[derive(Default)]
pub struct NightStarsPlugin;

impl Plugin for NightStarsPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] NightStarsPlugin");
        bevy::asset::embedded_asset!(app, "night_stars.wgsl");

        app.register_type::<NightStarsData>()
            .init_resource::<NightStarsState>()
            .add_plugins(MaterialPlugin::<NightStarsMaterial>::default())
            .add_systems(Update, sync_night_stars);

        inspector::register(app);
    }
}

// `Runtime`, explicitly: `plugin!` defaults to `Editor` where `add!` defaulted
// to `Runtime`, so omitting it would stop shipping the starfield to games.
renzora::plugin!(NightStarsPlugin, Runtime);
