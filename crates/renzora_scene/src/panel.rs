//! Scenes panel helpers — scene-file listing and open/create utilities shared by
//! the native (ember) `scenes` panel in [`crate::native_scenes`].

use std::path::PathBuf;

use bevy::prelude::*;
use renzora::core::CurrentProject;
use renzora_camera::OrbitCameraState;
use renzora_engine::scene_io;

pub(crate) const EMPTY_SCENE_RON: &str = "(\n    resources: {},\n    entities: {},\n)\n";

pub(crate) fn list_scenes(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(read) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in read.flatten() {
        let p = entry.path();
        if p.extension().and_then(|s| s.to_str()) == Some("bsn") {
            out.push(p);
        }
    }
    out.sort_by(|a, b| {
        a.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase()
            .cmp(
                &b.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase(),
            )
    });
    out
}

pub(crate) fn unique_scene_path(dir: &std::path::Path, base: &str) -> PathBuf {
    let mut i = 0u32;
    loop {
        let name = if i == 0 {
            format!("{base}.bsn")
        } else {
            format!("{base}_{i}.bsn")
        };
        let p = dir.join(&name);
        if !p.exists() {
            return p;
        }
        i += 1;
    }
}

pub(crate) fn paths_equal(a: &std::path::Path, b: &std::path::Path) -> bool {
    std::fs::canonicalize(a)
        .ok()
        .zip(std::fs::canonicalize(b).ok())
        .map(|(a, b)| a == b)
        .unwrap_or_else(|| a == b)
}

pub(crate) fn open_scene(world: &mut World, path: &std::path::Path) {
    let relative = world
        .get_resource::<CurrentProject>()
        .and_then(|p| p.make_relative(path));

    // If a tab already has this scene open, just activate it.
    if let Some(ref rel) = relative {
        let existing_idx = world
            .get_resource::<renzora_ui::DocumentTabState>()
            .and_then(|ts| {
                ts.tabs
                    .iter()
                    .position(|t| t.scene_path.as_deref() == Some(rel.as_str()))
            });
        if let Some(idx) = existing_idx {
            if let Some(mut project) = world.get_resource_mut::<CurrentProject>() {
                if project.config.editor_last_scene.as_deref() != Some(rel.as_str()) {
                    project.config.editor_last_scene = Some(rel.clone());
                    let _ = project.save_config();
                }
            }
            let ids = world
                .get_resource_mut::<renzora_ui::DocumentTabState>()
                .and_then(|mut ts| ts.activate_tab(idx));
            if let Some((old_id, new_id)) = ids {
                world.insert_resource(renzora::core::TabSwitchRequest {
                    old_tab_id: old_id,
                    new_tab_id: new_id,
                });
            }
            return;
        }
    }

    // Otherwise: save the current tab's scene into its buffer, create a new
    // tab for this scene, and load it from disk. We perform the work inline
    // rather than firing a `TabSwitchRequest` because the target tab has no
    // buffer yet — the standard switch handler would reset to an empty scene.
    let old_tab_id = world
        .get_resource::<renzora_ui::DocumentTabState>()
        .and_then(|ts| ts.active_tab_id());

    if let Some(old_id) = old_tab_id {
        let scene_ron = scene_io::serialize_scene_to_string(world)
            .unwrap_or_else(|_| "(entities: {}, resources: {})".to_string());
        let (focus, distance, yaw, pitch) =
            if let Some(orbit) = world.get_resource::<OrbitCameraState>() {
                (
                    orbit.focus.to_array(),
                    orbit.distance,
                    orbit.yaw,
                    orbit.pitch,
                )
            } else {
                let def = OrbitCameraState::default();
                (def.focus.to_array(), def.distance, def.yaw, def.pitch)
            };
        let snapshot = renzora::core::TabSceneSnapshot {
            scene_ron,
            camera_focus: focus,
            camera_distance: distance,
            camera_yaw: yaw,
            camera_pitch: pitch,
        };
        if let Some(mut buffers) = world.get_resource_mut::<renzora::core::SceneTabBuffers>() {
            buffers.buffers.insert(old_id, snapshot);
        }
        // Pin assets the leaving tab's entities reference before they
        // despawn — see `tab_asset_cache` module doc.
        crate::tab_asset_cache::pin_live_tab_handles(world, old_id);
    }

    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Scene".into());
    if let Some(mut tabs) = world.get_resource_mut::<renzora_ui::DocumentTabState>() {
        let idx = tabs.add_tab(name, relative.clone());
        tabs.active_tab = idx;
    }

    crate::despawn_scene_entities(world);
    if let Some(mut orbit) = world.get_resource_mut::<OrbitCameraState>() {
        *orbit = OrbitCameraState::default();
    }
    scene_io::load_scene(world, path);
    crate::extract_orbit_from_scene_camera(world);

    if let (Some(rel), Some(mut project)) = (relative, world.get_resource_mut::<CurrentProject>()) {
        if project.config.editor_last_scene.as_deref() != Some(rel.as_str()) {
            project.config.editor_last_scene = Some(rel);
            let _ = project.save_config();
        }
    }
}
