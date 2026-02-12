//! Mesh entity spawning (cubes, spheres, etc.)

use bevy::prelude::*;

use crate::core::{EditorEntity, SceneNode};
use crate::shared::{MaterialData, MeshNodeData, MeshPrimitiveType, MeshInstanceData, MeshletMeshData};
use super::{Category, EntityTemplate};
use super::procedural_meshes;

/// Path to the default checkerboard material blueprint (relative to engine root)
const DEFAULT_MATERIAL_PATH: &str = "assets/materials/checkerboard_default.material_bp";

pub static TEMPLATES: &[EntityTemplate] = &[
    EntityTemplate { name: "Cube", category: Category::Mesh, spawn: spawn_cube },
    EntityTemplate { name: "Sphere", category: Category::Mesh, spawn: spawn_sphere },
    EntityTemplate { name: "Cylinder", category: Category::Mesh, spawn: spawn_cylinder },
    EntityTemplate { name: "Plane", category: Category::Mesh, spawn: spawn_plane },
    EntityTemplate { name: "Cone", category: Category::Mesh, spawn: spawn_cone },
    EntityTemplate { name: "Torus", category: Category::Mesh, spawn: spawn_torus },
    EntityTemplate { name: "Capsule", category: Category::Mesh, spawn: spawn_capsule },
    EntityTemplate { name: "Wedge", category: Category::Mesh, spawn: spawn_wedge },
    EntityTemplate { name: "Stairs", category: Category::Mesh, spawn: spawn_stairs },
    EntityTemplate { name: "Arch", category: Category::Mesh, spawn: spawn_arch },
    EntityTemplate { name: "Half Cylinder", category: Category::Mesh, spawn: spawn_half_cylinder },
    EntityTemplate { name: "Quarter Pipe", category: Category::Mesh, spawn: spawn_quarter_pipe },
    EntityTemplate { name: "Corner", category: Category::Mesh, spawn: spawn_corner },
    EntityTemplate { name: "Prism", category: Category::Mesh, spawn: spawn_prism },
    EntityTemplate { name: "Pyramid", category: Category::Mesh, spawn: spawn_pyramid },
    EntityTemplate { name: "Pipe", category: Category::Mesh, spawn: spawn_pipe },
    EntityTemplate { name: "Ring", category: Category::Mesh, spawn: spawn_ring },
    EntityTemplate { name: "Wall", category: Category::Mesh, spawn: spawn_wall },
    EntityTemplate { name: "Ramp", category: Category::Mesh, spawn: spawn_ramp },
    EntityTemplate { name: "Hemisphere", category: Category::Mesh, spawn: spawn_hemisphere },
    EntityTemplate { name: "Curved Wall", category: Category::Mesh, spawn: spawn_curved_wall },
    EntityTemplate { name: "Doorway", category: Category::Mesh, spawn: spawn_doorway },
    EntityTemplate { name: "Window Wall", category: Category::Mesh, spawn: spawn_window_wall },
    EntityTemplate { name: "L-Shape", category: Category::Mesh, spawn: spawn_l_shape },
    EntityTemplate { name: "T-Shape", category: Category::Mesh, spawn: spawn_t_shape },
    EntityTemplate { name: "Cross", category: Category::Mesh, spawn: spawn_cross_shape },
    EntityTemplate { name: "Funnel", category: Category::Mesh, spawn: spawn_funnel },
    EntityTemplate { name: "Gutter", category: Category::Mesh, spawn: spawn_gutter },
    EntityTemplate { name: "Spiral Stairs", category: Category::Mesh, spawn: spawn_spiral_stairs },
    EntityTemplate { name: "Pillar", category: Category::Mesh, spawn: spawn_pillar },
    EntityTemplate { name: "MeshInstance", category: Category::Mesh, spawn: spawn_mesh_instance },
    EntityTemplate { name: "MeshletMesh", category: Category::Mesh, spawn: spawn_meshlet_mesh },
    EntityTemplate { name: "Node3D (Empty)", category: Category::Nodes3D, spawn: spawn_node3d },
];

fn create_standard_material(materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::srgb(0.7, 0.7, 0.7),
        perceptual_roughness: 0.9,
        ..default()
    })
}

/// Create a mesh handle for a given MeshPrimitiveType
pub fn create_mesh_for_type(meshes: &mut Assets<Mesh>, mesh_type: MeshPrimitiveType) -> Handle<Mesh> {
    match mesh_type {
        MeshPrimitiveType::Cube => meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        MeshPrimitiveType::Sphere => meshes.add(Sphere::new(0.5).mesh().ico(5).unwrap()),
        MeshPrimitiveType::Cylinder => meshes.add(Cylinder::new(0.5, 1.0)),
        MeshPrimitiveType::Plane => meshes.add(Plane3d::default().mesh().size(2.0, 2.0)),
        MeshPrimitiveType::Cone => meshes.add(Cone { radius: 0.5, height: 1.0 }),
        MeshPrimitiveType::Torus => meshes.add(Torus { minor_radius: 0.15, major_radius: 0.35 }),
        MeshPrimitiveType::Capsule => meshes.add(Capsule3d::new(0.25, 0.5)),
        MeshPrimitiveType::Wedge => meshes.add(procedural_meshes::create_wedge_mesh()),
        MeshPrimitiveType::Stairs => meshes.add(procedural_meshes::create_stairs_mesh(6)),
        MeshPrimitiveType::Arch => meshes.add(procedural_meshes::create_arch_mesh(16)),
        MeshPrimitiveType::HalfCylinder => meshes.add(procedural_meshes::create_half_cylinder_mesh(16)),
        MeshPrimitiveType::QuarterPipe => meshes.add(procedural_meshes::create_quarter_pipe_mesh(16)),
        MeshPrimitiveType::Corner => meshes.add(procedural_meshes::create_corner_mesh()),
        MeshPrimitiveType::Prism => meshes.add(procedural_meshes::create_prism_mesh()),
        MeshPrimitiveType::Pyramid => meshes.add(procedural_meshes::create_pyramid_mesh()),
        MeshPrimitiveType::Pipe => meshes.add(procedural_meshes::create_pipe_mesh(24)),
        MeshPrimitiveType::Ring => meshes.add(procedural_meshes::create_ring_mesh(24)),
        MeshPrimitiveType::Wall => meshes.add(Cuboid::new(1.0, 2.0, 0.1)),
        MeshPrimitiveType::Ramp => meshes.add(procedural_meshes::create_ramp_mesh()),
        MeshPrimitiveType::Hemisphere => meshes.add(procedural_meshes::create_hemisphere_mesh(16)),
        MeshPrimitiveType::CurvedWall => meshes.add(procedural_meshes::create_curved_wall_mesh(16)),
        MeshPrimitiveType::Doorway => meshes.add(procedural_meshes::create_doorway_mesh()),
        MeshPrimitiveType::WindowWall => meshes.add(procedural_meshes::create_window_wall_mesh()),
        MeshPrimitiveType::LShape => meshes.add(procedural_meshes::create_l_shape_mesh()),
        MeshPrimitiveType::TShape => meshes.add(procedural_meshes::create_t_shape_mesh()),
        MeshPrimitiveType::CrossShape => meshes.add(procedural_meshes::create_cross_shape_mesh()),
        MeshPrimitiveType::Funnel => meshes.add(procedural_meshes::create_funnel_mesh(24)),
        MeshPrimitiveType::Gutter => meshes.add(procedural_meshes::create_gutter_mesh(16)),
        MeshPrimitiveType::SpiralStairs => meshes.add(procedural_meshes::create_spiral_stairs_mesh(16)),
        MeshPrimitiveType::Pillar => meshes.add(procedural_meshes::create_pillar_mesh()),
    }
}

pub fn spawn_cube(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::Cube);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Cube, "Cube", parent)
}

pub fn spawn_sphere(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::Sphere);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Sphere, "Sphere", parent)
}

pub fn spawn_cylinder(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::Cylinder);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Cylinder, "Cylinder", parent)
}

pub fn spawn_plane(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::Plane);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Plane, "Plane", parent)
}

pub fn spawn_cone(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::Cone);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Cone, "Cone", parent)
}

pub fn spawn_torus(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::Torus);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Torus, "Torus", parent)
}

pub fn spawn_capsule(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::Capsule);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Capsule, "Capsule", parent)
}

pub fn spawn_wedge(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::Wedge);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Wedge, "Wedge", parent)
}

pub fn spawn_stairs(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::Stairs);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Stairs, "Stairs", parent)
}

pub fn spawn_arch(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::Arch);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Arch, "Arch", parent)
}

pub fn spawn_half_cylinder(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::HalfCylinder);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::HalfCylinder, "Half Cylinder", parent)
}

pub fn spawn_quarter_pipe(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::QuarterPipe);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::QuarterPipe, "Quarter Pipe", parent)
}

pub fn spawn_corner(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::Corner);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Corner, "Corner", parent)
}

pub fn spawn_prism(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::Prism);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Prism, "Prism", parent)
}

pub fn spawn_pyramid(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::Pyramid);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Pyramid, "Pyramid", parent)
}

pub fn spawn_pipe(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::Pipe);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Pipe, "Pipe", parent)
}

pub fn spawn_ring(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::Ring);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Ring, "Ring", parent)
}

pub fn spawn_wall(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::Wall);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Wall, "Wall", parent)
}

fn spawn_mesh_entity(
    commands: &mut Commands,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    mesh_type: MeshPrimitiveType,
    name: &str,
    parent: Option<Entity>,
) -> Entity {
    // Note: RaytracingMesh3d is managed by sync_rendering_settings based on Solari state
    let mut entity_commands = commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_xyz(0.0, 0.5, 0.0),
        Visibility::default(),
        EditorEntity {
            name: name.to_string(),
            tag: String::new(),
            visible: true,
            locked: false,
        },
        SceneNode,
        MeshNodeData { mesh_type },
        MaterialData {
            material_path: Some(DEFAULT_MATERIAL_PATH.to_string()),
        },
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

pub fn spawn_ramp(
    commands: &mut Commands, meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>, parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::Ramp);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Ramp, "Ramp", parent)
}

pub fn spawn_hemisphere(
    commands: &mut Commands, meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>, parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::Hemisphere);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Hemisphere, "Hemisphere", parent)
}

pub fn spawn_curved_wall(
    commands: &mut Commands, meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>, parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::CurvedWall);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::CurvedWall, "Curved Wall", parent)
}

pub fn spawn_doorway(
    commands: &mut Commands, meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>, parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::Doorway);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Doorway, "Doorway", parent)
}

pub fn spawn_window_wall(
    commands: &mut Commands, meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>, parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::WindowWall);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::WindowWall, "Window Wall", parent)
}

pub fn spawn_l_shape(
    commands: &mut Commands, meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>, parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::LShape);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::LShape, "L-Shape", parent)
}

pub fn spawn_t_shape(
    commands: &mut Commands, meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>, parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::TShape);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::TShape, "T-Shape", parent)
}

pub fn spawn_cross_shape(
    commands: &mut Commands, meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>, parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::CrossShape);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::CrossShape, "Cross", parent)
}

pub fn spawn_funnel(
    commands: &mut Commands, meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>, parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::Funnel);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Funnel, "Funnel", parent)
}

pub fn spawn_gutter(
    commands: &mut Commands, meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>, parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::Gutter);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Gutter, "Gutter", parent)
}

pub fn spawn_spiral_stairs(
    commands: &mut Commands, meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>, parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::SpiralStairs);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::SpiralStairs, "Spiral Stairs", parent)
}

pub fn spawn_pillar(
    commands: &mut Commands, meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>, parent: Option<Entity>,
) -> Entity {
    let mesh = create_mesh_for_type(meshes, MeshPrimitiveType::Pillar);
    let material = create_standard_material(materials);
    spawn_mesh_entity(commands, mesh, material, MeshPrimitiveType::Pillar, "Pillar", parent)
}

pub fn spawn_mesh_instance(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: "MeshInstance".to_string(),
            tag: String::new(),
            visible: true,
            locked: false,
        },
        SceneNode,
        MeshInstanceData { model_path: None },
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

/// Spawn an entity configured for meshlet (virtual geometry) rendering.
/// The entity starts with no mesh - assign a .meshlet asset path via the inspector.
/// Meshlet rendering provides GPU-driven LOD and occlusion culling for high-poly meshes.
pub fn spawn_meshlet_mesh(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: "MeshletMesh".to_string(),
            tag: String::new(),
            visible: true,
            locked: false,
        },
        SceneNode,
        MeshletMeshData {
            meshlet_path: String::new(),
            enabled: true,
            lod_bias: 0.0,
        },
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}

pub fn spawn_node3d(
    commands: &mut Commands,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        Transform::default(),
        Visibility::default(),
        EditorEntity {
            name: "Node3D".to_string(),
            tag: String::new(),
            visible: true,
            locked: false,
        },
        SceneNode,
    ));

    if let Some(parent_entity) = parent {
        entity_commands.insert(ChildOf(parent_entity));
    }

    entity_commands.id()
}
