//! One mesh and one material per *kind* of primitive, instead of one per entity.
//!
//! Every path that puts a built-in shape in the world used to call the shape
//! registry's factory and `Assets::add` a fresh `StandardMaterial`, so a hundred
//! identical grey cubes were a hundred `Mesh` assets and a hundred materials.
//! The memory is the small part: Bevy batches draws by `(mesh id, material id)`,
//! so identical-looking shapes that do not literally share both handles are one
//! draw call each.
//!
//! This is the cache all four of those paths go through — scene rehydration, the
//! spawn undo command, the delete-undo that rebuilds them, and the ghost that
//! follows the cursor during a shape drag. Handing out the same two handles is
//! what lets the renderer see them as one batch.
//!
//! # Sharing a mutable asset, and how the mesh half stays correct
//!
//! A shared handle is only safe while nothing writes through it for one entity's
//! benefit, and two systems did exactly that:
//!
//! * [`blockout::apply_mesh_color`](crate::blockout::apply_mesh_color) wrote a
//!   changed `MeshColor` into the material. It now asks this cache for the
//!   material matching the new colour and swaps the handle instead, which shares
//!   by construction: every shape at that colour ends up on one material.
//! * [`blockout::project_blockout_uvs`](crate::blockout::project_blockout_uvs)
//!   rewrites a mesh's UVs from the entity's world scale, which genuinely is
//!   per-entity data. There the cached mesh is **copy-on-write**: the shared
//!   copy is projected at scale 1 when it is created, and an entity whose scale
//!   wants different UVs gets a private fork ([`PrimitiveAssets::is_shared_mesh`]
//!   is the test) rather than editing the copy its siblings are rendering.
//!
//! So an unscaled primitive shares its mesh forever, and so does one wearing a
//! real `MaterialRef` (the projection skips those entirely, at any scale). A
//! scaled blockout shape forks, which is exactly the one-mesh-per-entity cost
//! it already paid. Two shapes forked to the *same* scale still get a fork each;
//! keying the forks by `(shape, scale)` would collapse those too, but it needs
//! an eviction pass to survive a gizmo drag generating an entry per frame, so it
//! is deliberately not here.
//!
//! The material half has no such caveat — a colour is not derived from the
//! entity's transform, so materials are shared unconditionally.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use renzora::core::{GridTexture, ShapeRegistry};

/// What makes two blockout materials the same material: the tint, and which
/// grid image (if any) they wear.
///
/// The grid is part of the key rather than assumed constant because it is not
/// resident at boot — a material built before the image is generated is a plain
/// matte fill, and keying on the image's id means the textured one that follows
/// is a separate entry instead of silently reusing the flat one.
type BlockoutKey = ([u32; 4], Option<AssetId<Image>>);

/// Shared meshes and materials for the built-in shapes. See the module docs.
#[derive(Resource, Default)]
pub struct PrimitiveAssets {
    /// One mesh per shape id, projected at scale 1.
    meshes: HashMap<String, Handle<Mesh>>,
    /// The ids in `meshes`, for the copy-on-write test. A set rather than a
    /// scan of the map: the UV projection asks this once per primitive per
    /// frame.
    shared: HashSet<AssetId<Mesh>>,
    /// One blockout material per `(tint, grid)`.
    materials: HashMap<BlockoutKey, Handle<StandardMaterial>>,
}

impl PrimitiveAssets {
    /// The shared mesh for a registered shape, building it on first use.
    ///
    /// The stored copy is UV-projected at scale 1 rather than left on the
    /// registry's authored unwrap, so an unscaled primitive is already at the
    /// UVs `project_blockout_uvs` would compute for it and never forks. (The
    /// projection is idempotent, so re-projecting it later writes nothing.)
    ///
    /// `None` for an unregistered id, which is the caller's cue to warn: it
    /// means a scene names a shape this build does not have.
    pub fn mesh(
        &mut self,
        id: &str,
        registry: &ShapeRegistry,
        meshes: &mut Assets<Mesh>,
    ) -> Option<Handle<Mesh>> {
        if let Some(handle) = self.meshes.get(id) {
            return Some(handle.clone());
        }
        let handle = registry.create_mesh(id, meshes)?;
        if let Some(mut mesh) = meshes.get_mut(&handle) {
            crate::blockout::project_mesh_uvs(&mut mesh, Vec3::ONE);
        }
        self.shared.insert(handle.id());
        self.meshes.insert(id.to_string(), handle.clone());
        Some(handle)
    }

    /// The shared blockout material at `color`, building it on first use.
    pub fn material(
        &mut self,
        color: Color,
        grid: Option<&GridTexture>,
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        let c = color.to_linear();
        let key: BlockoutKey = (
            [
                c.red.to_bits(),
                c.green.to_bits(),
                c.blue.to_bits(),
                c.alpha.to_bits(),
            ],
            grid.map(|g| g.0.id()),
        );
        if let Some(handle) = self.materials.get(&key) {
            return handle.clone();
        }
        let handle = materials.add(crate::blockout::blockout_material(color, grid));
        self.materials.insert(key, handle.clone());
        handle
    }

    /// Is this mesh one of the shared copies, and therefore not ours to write
    /// to for one entity's benefit? See the module docs on copy-on-write.
    pub fn is_shared_mesh(&self, id: AssetId<Mesh>) -> bool {
        self.shared.contains(&id)
    }
}

/// Mesh + material for a fresh primitive, from a `&mut World`.
///
/// For the exclusive-world callers (the undo commands), which would otherwise
/// each need the same three-deep `resource_scope` dance to hold the cache, the
/// shape registry and two asset collections at once.
///
/// `None` when the shape id is not registered.
pub fn primitive_assets(
    world: &mut World,
    shape_id: &str,
    color: Color,
) -> Option<(Handle<Mesh>, Handle<StandardMaterial>)> {
    world.resource_scope(|world, mut cache: Mut<PrimitiveAssets>| {
        world.resource_scope(|world, registry: Mut<ShapeRegistry>| {
            let grid = world.get_resource::<GridTexture>().cloned();
            let mesh = {
                let mut meshes = world.resource_mut::<Assets<Mesh>>();
                cache.mesh(shape_id, &registry, &mut meshes)?
            };
            let material = {
                let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
                cache.material(color, grid.as_ref(), &mut materials)
            };
            Some((mesh, material))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> ShapeRegistry {
        let mut reg = ShapeRegistry::default();
        reg.register(renzora::core::ShapeEntry {
            id: "cube",
            name: "Cube",
            icon: "",
            category: "Shapes",
            create_mesh: |m| m.add(Cuboid::new(1.0, 1.0, 1.0)),
            default_color: Color::WHITE,
        });
        reg
    }

    /// The whole point: two shapes of the same kind get one mesh, so the
    /// renderer can batch them.
    #[test]
    fn the_same_shape_twice_is_one_mesh() {
        let mut cache = PrimitiveAssets::default();
        let reg = registry();
        let mut meshes = Assets::<Mesh>::default();

        let a = cache.mesh("cube", &reg, &mut meshes).expect("registered");
        let b = cache.mesh("cube", &reg, &mut meshes).expect("registered");

        assert_eq!(a.id(), b.id());
        assert_eq!(meshes.len(), 1, "the factory ran twice");
    }

    /// Same for materials, and a different colour has to stay a different
    /// material or one shape's tint would repaint every other.
    #[test]
    fn a_colour_is_one_material_and_two_colours_are_two() {
        let mut cache = PrimitiveAssets::default();
        let mut materials = Assets::<StandardMaterial>::default();

        let red = cache.material(Color::srgb(1.0, 0.0, 0.0), None, &mut materials);
        let red_again = cache.material(Color::srgb(1.0, 0.0, 0.0), None, &mut materials);
        let blue = cache.material(Color::srgb(0.0, 0.0, 1.0), None, &mut materials);

        assert_eq!(red.id(), red_again.id());
        assert_ne!(red.id(), blue.id());
        assert_eq!(materials.len(), 2);
    }

    /// A shared mesh must announce itself, or the UV projection would write an
    /// entity's scale into the copy every other entity is rendering.
    #[test]
    fn a_cached_mesh_is_marked_shared() {
        let mut cache = PrimitiveAssets::default();
        let reg = registry();
        let mut meshes = Assets::<Mesh>::default();

        let shared = cache.mesh("cube", &reg, &mut meshes).expect("registered");
        let private = meshes.add(Cuboid::new(1.0, 1.0, 1.0));

        assert!(cache.is_shared_mesh(shared.id()));
        assert!(!cache.is_shared_mesh(private.id()));
    }

    /// An unregistered id is `None`, not a panic and not an empty mesh: it means
    /// a scene names a shape this build does not have.
    #[test]
    fn an_unknown_shape_has_no_mesh() {
        let mut cache = PrimitiveAssets::default();
        let reg = registry();
        let mut meshes = Assets::<Mesh>::default();

        assert!(cache.mesh("dodecahedron", &reg, &mut meshes).is_none());
        assert_eq!(meshes.len(), 0);
    }
}
