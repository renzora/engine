//! The scene vocabulary: the ECS components and entity tags that more than one
//! crate reads or writes, canonical entity ids, the built-in shape registry, a
//! camera's render-resolution scale, and the project byte loader for assets the
//! `AssetServer` does not own.
//!
//! A layer of the contract hierarchy, moved out of `renzora::core`. Plugins
//! reach every item at the path it always had (`renzora::SceneCamera`,
//! `renzora::core::HideInHierarchy`, ...) through the re-exports; engine crates
//! depend on this crate directly.
//!
//! **Every reflected type keeps the type path it had in `renzora`**, through
//! `#[type_path = ".."]`. Saved scenes name each component by its full type
//! path (`renzora::core::components::SceneEntityId`), so dropping the pin from a
//! type stops it loading from every scene already saved with it. A new type
//! added here takes this crate's own path and needs no pin.

pub mod asset_bytes; // the project/VFS byte loader for non-AssetServer assets
pub mod components; // shared ECS components + entity-tag markers
pub mod entity_id; // canonical unique snake_case entity ids (Name)
pub mod shapes; // built-in shape registry (mesh factories by id)
mod render_resolution; // a camera's render-resolution scale

pub use asset_bytes::*;
pub use components::*;
pub use entity_id::*;
pub use render_resolution::*;
pub use shapes::*;

#[cfg(test)]
mod type_path_tests {
    use bevy::reflect::TypePath;

    /// Every type that moved here from `renzora` still reflects the path saved
    /// scenes were written with. A failure here is a scene that stops loading.
    #[test]
    fn moved_types_keep_their_saved_type_paths() {
        macro_rules! at {
            ($module:literal: $($ty:ident),* $(,)?) => {
                $(assert_eq!(
                    <crate::$ty as TypePath>::type_path(),
                    concat!($module, "::", stringify!($ty)),
                );)*
            };
        }
        at!("renzora::core::components":
            EntityGroup, Node2d, SpriteImagePath, SpriteCustomSize, YSort, SpriteSheet,
            SpriteAtlasRegion, ReflectionProbeSource, Persistent, SceneEntityId, EntityIcon,
            EntityLabelColor, HierarchyOrder, PickAsUnit, SceneInstance, WorldPartition,
            PartitionCell, GaussianSplat, SceneCamera, DefaultCamera, FollowNode,
            CameraRenderResolution, CameraPreset, CameraPresets, MeshPrimitive, EditedMesh,
            MeshInstanceData, MeshLod, StandardGeometry,
        );
        at!("renzora::core::shapes": MeshColor);
        at!("renzora::core::viewport_types": RenderResolution);
    }
}
