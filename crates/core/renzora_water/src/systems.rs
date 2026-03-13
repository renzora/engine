use bevy::prelude::*;

use crate::buoyancy::Buoyant;
use crate::component::WaterSurface;
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
        commands.entity(entity).insert((
            Mesh3d(mesh),
            MeshMaterial3d(material),
        ));
    }
}

/// Update water material uniforms each frame (time, sun, params, object interactions).
pub fn update_water_uniforms(
    time: Res<Time>,
    water_query: Query<(&WaterSurface, &MeshMaterial3d<WaterMaterial>)>,
    mut materials: ResMut<Assets<WaterMaterial>>,
    sun_query: Query<&Transform, With<DirectionalLight>>,
    buoyant_query: Query<&GlobalTransform, With<Buoyant>>,
) {
    let t = time.elapsed_secs();

    let sun_dir = sun_query.iter().next()
        .map(|tr| tr.forward().as_vec3())
        .unwrap_or(Vec3::new(-0.3, -0.7, -0.4).normalize());

    // Collect buoyant object positions (up to 8)
    let mut obj_slots = [Vec4::ZERO; 8];
    let mut obj_count = 0u32;
    for transform in buoyant_query.iter() {
        if obj_count >= 8 {
            break;
        }
        let pos = transform.translation();
        // Pack as (x, z, radius, submerge_intensity)
        // Radius is approximate — could read from collider later
        obj_slots[obj_count as usize] = Vec4::new(pos.x, pos.z, 2.0, 1.0);
        obj_count += 1;
    }

    for (surface, mat_handle) in water_query.iter() {
        if let Some(mat) = materials.get_mut(&mat_handle.0) {
            mat.uniforms.time = t;
            mat.uniforms.sun_direction = Vec4::new(sun_dir.x, sun_dir.y, sun_dir.z, 0.0);
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
