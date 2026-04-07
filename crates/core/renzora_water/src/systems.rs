use bevy::prelude::*;
use bevy::core_pipeline::prepass::DepthPrepass;

use crate::buoyancy::Buoyant;
use crate::component::{WaterSurface, WaterInteractor};
use crate::material::{WaterMaterial, sync_uniforms};
use crate::mesh::generate_water_mesh;

/// Auto-setup: when a `WaterSurface` is added without a mesh, generate the mesh + material.
pub fn setup_water_entities(
    mut commands: Commands,
    query: Query<(Entity, &WaterSurface), (Added<WaterSurface>, Without<Mesh3d>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
) {
    for (entity, surface) in query.iter() {
        let mesh = meshes.add(generate_water_mesh(surface.mesh_size, surface.subdivisions));
        let material = materials.add(WaterMaterial::default());
        commands.entity(entity).try_insert((
            Mesh3d(mesh),
            MeshMaterial3d(material),
        ));
    }
}

/// Ensure cameras have DepthPrepass for depth-based water effects.
pub fn ensure_depth_prepass(
    mut commands: Commands,
    cameras: Query<Entity, (With<Camera3d>, Without<DepthPrepass>)>,
    water_exists: Query<(), With<WaterSurface>>,
) {
    if water_exists.is_empty() { return; }
    for entity in cameras.iter() {
        commands.entity(entity).insert(DepthPrepass);
    }
}

/// Update water material uniforms each frame (time, sun, params, object interactions).
pub fn update_water_uniforms(
    time: Res<Time>,
    water_query: Query<(&WaterSurface, &MeshMaterial3d<WaterMaterial>, &GlobalTransform)>,
    mut materials: ResMut<Assets<WaterMaterial>>,
    sun_query: Query<(&Transform, &DirectionalLight)>,
    buoyant_query: Query<&GlobalTransform, With<Buoyant>>,
    interactor_query: Query<(&GlobalTransform, &WaterInteractor), Without<Buoyant>>,
) {
    let t = time.elapsed_secs();

    let (sun_dir, sun_intensity) = sun_query.iter().next()
        .map(|(tr, light)| {
            let illum = light.illuminance / 10000.0;
            (tr.forward().as_vec3(), illum.clamp(0.0, 1.0))
        })
        .unwrap_or((Vec3::new(-0.3, -0.7, -0.4).normalize(), 1.0));

    // Collect buoyant objects (up to 8 slots)
    let mut obj_slots = [Vec4::ZERO; 8];
    let mut obj_count = 0u32;

    for transform in buoyant_query.iter() {
        if obj_count >= 8 { break; }
        let pos = transform.translation();
        obj_slots[obj_count as usize] = Vec4::new(pos.x, pos.z, 2.0, 1.0);
        obj_count += 1;
    }

    // Collect WaterInteractor entities (fill remaining slots)
    for (transform, interactor) in interactor_query.iter() {
        if obj_count >= 8 { break; }
        let pos = transform.translation();
        let r = if interactor.radius > 0.0 { interactor.radius } else { 2.0 };
        let i = interactor.intensity.clamp(0.0, 2.0);
        obj_slots[obj_count as usize] = Vec4::new(pos.x, pos.z, r, i);
        obj_count += 1;
    }

    for (surface, mat_handle, _water_transform) in water_query.iter() {
        if let Some(mat) = materials.get_mut(&mat_handle.0) {
            mat.uniforms.time = t;
            mat.uniforms.sun_direction = Vec4::new(sun_dir.x, sun_dir.y, sun_dir.z, sun_intensity);
            sync_uniforms(surface, &mut mat.uniforms);

            // Write object interaction slots
            mat.uniforms.obj_0 = obj_slots[0];
            mat.uniforms.obj_1 = obj_slots[1];
            mat.uniforms.obj_2 = obj_slots[2];
            mat.uniforms.obj_3 = obj_slots[3];
            mat.uniforms.obj_4 = obj_slots[4];
            mat.uniforms.obj_5 = obj_slots[5];
            mat.uniforms.obj_6 = obj_slots[6];
            mat.uniforms.obj_7 = obj_slots[7];
            mat.uniforms.obj_count = obj_count;
        }
    }
}

/// Spawn a water surface entity with the given configuration.
pub fn spawn_water(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<WaterMaterial>,
    config: WaterSurface,
    transform: Transform,
) -> Entity {
    let mesh = meshes.add(generate_water_mesh(config.mesh_size, config.subdivisions));
    let material = materials.add(WaterMaterial::default());

    commands.spawn((
        Name::new("Water Surface"),
        Mesh3d(mesh),
        MeshMaterial3d(material),
        transform,
        config,
    )).id()
}
