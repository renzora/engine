pub mod enums;
pub mod settings;
pub mod errors;

pub mod meshgen;

use bevy::{ecs::{lifecycle::HookContext, world::DeferredWorld}, prelude::*};
use fastrand::Rng;
use serde::{Deserialize, Serialize};

use crate::meshgen::generate_tree_meshes;

pub use enums::{LeafBillboard, TreeType};
pub use settings::TreeMeshSettings;


pub struct TreeProceduralGenerationPlugin;

impl Plugin for TreeProceduralGenerationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TreeMeshSettings>();
        app.register_type::<TreeMeshSettings>();
        app.init_resource::<TreeDefaultMaterials>();
        app.register_type::<TreeDefaultMaterials>();
        app.register_type::<Tree>();
        // Fork note: `Leaves` is intentionally NOT registered for reflection.
        // It holds an `Entity` link to the generated leaf child that does not
        // remap across scene loads; keeping it out of the registry means
        // renzora's scene serializer never persists it. The leaf child is
        // regenerated from `Tree` by the `on_add` hook on load.

        app.add_systems(PostUpdate, update_all_tree_meshes_with_global_settings.run_if(resource_changed::<TreeMeshSettings>));
        app.add_systems(PostUpdate, update_all_tree_meshes_with_local_settings);
    }
}



#[derive(Component, Reflect, Clone, Debug, Serialize, Deserialize)]
#[reflect(Component, Default, Serialize, Deserialize)]
#[component(on_add = new_tree_component_added)]
pub struct Tree {
    /// the seed for the rng (same seed and TreeMeshSettings = same tree mesh)
    /// the seed is always local to each tree instance (regardless if the tree is using global TreeMeshSettings)
    pub seed: u64,
    /// the settings to use for this tree; if set to none the settings from the global TreeMeshSettings resource are used
    pub tree_mesh_settings_override: Option<TreeMeshSettings>,
    /// defaults to Color::WHITE
    ///
    /// Fork note: skipped during (de)serialization — asset handles don't remap
    /// across scene loads, so the material falls back to the default on reload.
    #[serde(skip)]
    #[reflect(ignore)]
    pub bark_material_override: Option<MeshMaterial3d<StandardMaterial>>,
    /// defaults to green -> Color::LinearRgba(LinearRgba { red: 0.0, green: 1.0, blue: 0.0, alpha: 1.0 }
    /// recommendation: AlphaMode::Mask(0.x) is recommend to be set for the leaves (depending on the texture used)
    #[serde(skip)]
    #[reflect(ignore)]
    pub leaf_material_override: Option<MeshMaterial3d<StandardMaterial>>,
}

impl Default for Tree {
    fn default() -> Self {
        Self {
            seed: 0,
            tree_mesh_settings_override: Some(TreeMeshSettings::default()),
            bark_material_override: None,
            leaf_material_override: None,
        }
    }
}


#[derive(Resource, Reflect)]
struct TreeDefaultMaterials {
    /// defaults to Color::WHITE
    pub bark_material: MeshMaterial3d<StandardMaterial>,
    /// defaults to green -> Color::LinearRgba(LinearRgba { red: 0.0, green: 1.0, blue: 0.0, alpha: 1.0 }
    /// recommendation: AlphaMode::Mask(0.x) is recommend to be set for the leaves (depending on the texture used)
    pub leaf_material: MeshMaterial3d<StandardMaterial>,
}

// Default leaf textures (ambientCG LeafSet005, CC0 — see textures/SOURCE.txt),
// embedded so the default leaf material works without loose asset files (this
// ships as a dlopen plugin, so there's no reliable asset path at runtime).
const LEAF_COLOR_PNG: &[u8] = include_bytes!("../textures/deciduous_leaf_color.png");
const LEAF_NORMAL_PNG: &[u8] = include_bytes!("../textures/deciduous_leaf_normal.png");
const LEAF_ROUGHNESS_PNG: &[u8] = include_bytes!("../textures/deciduous_leaf_roughness.png");

fn decode_embedded_png(images: &mut Assets<Image>, bytes: &[u8], is_srgb: bool) -> Handle<Image> {
    let image = Image::from_buffer(
        bytes,
        bevy::image::ImageType::Extension("png"),
        bevy::image::CompressedImageFormats::NONE,
        is_srgb,
        bevy::image::ImageSampler::Default,
        bevy::asset::RenderAssetUsages::MAIN_WORLD | bevy::asset::RenderAssetUsages::RENDER_WORLD,
    )
    .expect("embedded leaf PNG should decode (bevy `png` feature)");
    images.add(image)
}

impl FromWorld for TreeDefaultMaterials {
    fn from_world(world: &mut World) -> Self {
        // Decode the embedded leaf textures first (scoped borrow), then build
        // the materials.
        let (leaf_color, leaf_normal, leaf_roughness) = {
            let mut images = world.get_resource_mut::<Assets<Image>>().unwrap();
            (
                decode_embedded_png(&mut images, LEAF_COLOR_PNG, true),
                decode_embedded_png(&mut images, LEAF_NORMAL_PNG, false),
                decode_embedded_png(&mut images, LEAF_ROUGHNESS_PNG, false),
            )
        };

        let mut materials = world.get_resource_mut::<Assets<StandardMaterial>>().unwrap();
        Self {
            // Default bark: a neutral brown so an untextured tree still reads as
            // wood (override via `Tree::bark_material_override` for a real bark
            // texture).
            bark_material: MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.45, 0.32, 0.22),
                perceptual_roughness: 0.9,
                ..default()
            })),
            // Default leaves: the alpha-cutout leaf texture, double-sided and
            // alpha-masked. Without the mask each billboard quad renders as a
            // solid rectangle (and the perpendicular "Double" pair reads as an
            // "X"); the cutout is what gives leaves their shape.
            leaf_material: MeshMaterial3d(materials.add(StandardMaterial {
                base_color_texture: Some(leaf_color),
                normal_map_texture: Some(leaf_normal),
                metallic_roughness_texture: Some(leaf_roughness),
                perceptual_roughness: 1.0,
                cull_mode: None,
                double_sided: true,
                alpha_mode: AlphaMode::Mask(0.5),
                ..default()
            })),
        }
    }
}

/// Marks the parent tree entity, linking it to its generated leaf-mesh child.
///
/// Fork note: made `pub` (was private) so the renzora wrapper can find the
/// generated child and tag it (`HideInHierarchy`) without name-matching.
#[derive(Component, Reflect)]
pub struct Leaves(pub Entity);

fn new_tree_component_added(mut world: DeferredWorld, context: HookContext) {
    let tree_entity = context.entity;

    // Generate meshes
    // TODO: remove unwrap
    let tree: Tree = (*world.entity(tree_entity).components::<&Tree>()).clone();
    let tree_mesh_settings = tree.tree_mesh_settings_override.or_else(
      || {
            world.get_resource::<TreeMeshSettings>().cloned()
      }
    ).unwrap();

    let mut rng: Rng = Rng::with_seed(tree.seed);

    match generate_tree_meshes(&tree_mesh_settings, &mut rng) {
        Ok((branches_mesh, leaves_mesh)) => {
            // retrieve AssetServer
            let mut meshes = world.get_resource_mut::<Assets<Mesh>>().unwrap();

            // meshes
            let branches_mesh = Mesh3d(meshes.add(branches_mesh));
            let leaves_mesh = Mesh3d(meshes.add(leaves_mesh));

            let default_materials = world.get_resource::<TreeDefaultMaterials>().unwrap();
            // bark material
            let branch_material = tree.bark_material_override.clone().unwrap_or_else(|| default_materials.bark_material.clone());
            // leaf material
            let leaf_material = tree.leaf_material_override.clone().unwrap_or_else(|| default_materials.leaf_material.clone());

            // Spawn/Insert
            let mut commands = world.commands();
            let leaves_id = commands.spawn((
                Name::new("ProcGenTreeLeaves"),
                leaves_mesh,
                leaf_material,
            )).id();

            let mut tree_commands = commands.entity(tree_entity);

            // Fork note: upstream also overwrote the parent's `Name` with
            // "ProcGenTreeBranches". We don't — the parent keeps the name the
            // user/preset gave it, which is what shows in the hierarchy.
            tree_commands.insert((
                Leaves(leaves_id),
                branches_mesh,
                branch_material
            )).add_child(leaves_id);
        },
        Err(err) => error!("Error during tree mesh generation: {}", err),
    }
}


fn update_all_tree_meshes_with_local_settings(
    trees: Query<(Entity, &Tree, &MeshMaterial3d<StandardMaterial>, &Leaves), Changed<Tree>>,
    mesh_materials: Query<&MeshMaterial3d<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    global_tree_settings: Res<TreeMeshSettings>,
    default_materials: Res<TreeDefaultMaterials>,
    mut commands: Commands,
)
{
    // For now we are regenerating the whole tree mesh each time
    // TODO: Try to modify in place (or at least only branch/leaf levels or textures that need modification)
    for (tree_entity, tree, current_bark_material, leaves_entity) in trees.iter() {
        let tree_settings: &TreeMeshSettings = match tree.tree_mesh_settings_override {
            Some(ref tree_settings) => tree_settings,
            None => global_tree_settings.as_ref(),
        };

        let mut rng: Rng = Rng::with_seed(tree.seed);

        match generate_tree_meshes(tree_settings, &mut rng) {
            Ok((branches_mesh, leaves_mesh)) => {
                let branches_mesh = Mesh3d(meshes.add(branches_mesh));
                let leaves_mesh = Mesh3d(meshes.add(leaves_mesh));

                commands.entity(tree_entity).insert(branches_mesh);
                commands.entity(leaves_entity.0).insert(leaves_mesh);

                // check if the textures changed
                match tree.bark_material_override { // what is the target state of the bark material
                    Some(ref bark_material_from_local_settings) => {
                        if !current_bark_material.eq(bark_material_from_local_settings) {
                            commands.entity(tree_entity).insert(bark_material_from_local_settings.clone());
                        }
                    },
                    None => {
                        if !current_bark_material.eq(&default_materials.bark_material) {
                            commands.entity(tree_entity).insert(default_materials.bark_material.clone());
                        }
                    },
                }

                if let Ok(current_leaf_material) = mesh_materials.get(leaves_entity.0) {
                    match tree.leaf_material_override { // what is the target state of the leaf material
                        Some(ref leaf_material_from_local_settings) => {
                            if !current_leaf_material.eq(leaf_material_from_local_settings) {
                                commands.entity(leaves_entity.0).insert(leaf_material_from_local_settings.clone());
                            }
                        },
                        None => {
                            if !current_leaf_material.eq(&default_materials.leaf_material) {
                                commands.entity(leaves_entity.0).insert(default_materials.leaf_material.clone());
                            }
                        },
                    }
                }
            },
            Err(err) => error!("Error during tree mesh generation: {}", err),
        }
    }
}

fn update_all_tree_meshes_with_global_settings(
    trees: Query<(Entity, &Tree, &Leaves)>,
    tree_settings: Res<TreeMeshSettings>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
) {
    // For now we are regenerating the whole tree mesh each time
    // TODO: Try to modify in place (or at least only branch/leaf levels or textures that need modification)

    for (tree_entity, tree, leaves_entity) in trees.iter() {
        if tree.tree_mesh_settings_override.is_none() {
            let mut rng: Rng = Rng::with_seed(tree.seed);

            match generate_tree_meshes(&tree_settings, &mut rng) {
                Ok((branches_mesh, leaves_mesh)) => {
                    let branches_mesh = Mesh3d(meshes.add(branches_mesh));
                    let leaves_mesh = Mesh3d(meshes.add(leaves_mesh));

                    commands.entity(tree_entity).insert(branches_mesh);
                    commands.entity(leaves_entity.0).insert(leaves_mesh);
                },
                Err(err) => error!("Error during tree mesh generation: {}", err),
            }
        }
    }

}
