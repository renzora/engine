#![allow(unused_mut, dead_code, unused_variables)]

//! Shared scene load/save and rehydration — used by both editor and runtime.
//!
//! The shape of this module follows one asymmetry: a save deliberately writes
//! *less* than the world contains, and a load has to put the rest back. Every
//! component that is derived, ephemeral, or owned by another file is denied at
//! save time (see [`deny`]) and rebuilt at load time (see [`meshes`],
//! [`sprites`], [`cameras`], [`lights`]). A scene file is therefore a
//! description, not a dump — which is what lets a scene authored with a plugin
//! still load in a build without it.
//!
//! - [`save`] / [`load`] — the two main pipelines
//! - [`snapshot`] — the faithful, keeps-hierarchy variant used to undo a delete
//! - [`instances`] — nested scenes, prefab write-back, cycle detection
//! - [`prune`] — load-time repair of scenes an earlier save polluted
//! - [`events`] — load state + the events a load emits

mod cameras;
mod deny;
mod events;
mod instances;
mod lights;
mod load;
mod meshes;
mod prune;
mod save;
mod snapshot;
mod sprites;

// One flat public seam, unchanged by the split — every name here was a
// top-level item of this file before.
pub use cameras::{
    enforce_single_active_camera, rehydrate_cameras, rehydrate_visibility, sync_play_mode_camera,
    sync_scene_camera_to_editor_camera,
};
pub use events::{
    SceneLoadFailed, SceneLoadPhase, SceneLoadState, SceneLoaded, SceneLoadedWithSkippedTypes,
};
pub use instances::{
    expand_scene_instances, is_self_reference, paths_equal, save_all_scene_instances,
    save_prefab_source, spawn_scene_instance, would_create_reference_cycle, SceneReferenceCache,
};
pub use lights::{rehydrate_lights, rehydrate_suns};
pub use load::{load_current_scene, load_scene, load_scene_from_string};
pub use meshes::MeshInstanceLoadFailed;
pub use save::{save_current_scene, save_scene, serialize_scene_to_string};
pub use snapshot::{snapshot_entity_subtrees, spawn_entities_from_snapshot};

#[cfg(feature = "render_3d")]
pub use meshes::{apply_edited_meshes, rehydrate_meshes};

#[cfg(all(feature = "render_3d", feature = "gltf"))]
pub use meshes::{
    finish_mesh_instance_rehydrate, rehydrate_mesh_instances, PendingMeshInstanceRehydrate,
};

#[cfg(feature = "render_2d")]
pub use sprites::{
    apply_sprite_atlas_region, apply_sprite_sheet_crop, apply_y_sort, mirror_sprite_custom_size,
    on_sprite_custom_size_inserted, on_sprite_image_path_inserted,
    on_sprite_inserted_apply_image_path, on_sprite_sheet_removed,
};

pub(crate) use prune::{prune_leaked_ui, prune_orphaned_entities};
