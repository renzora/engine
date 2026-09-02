//! Dropping files on the material slot, and the graph edits every slot change
//! goes through.
//!
//! A `.material` binds the material. Images are routed to texture slots by
//! filename, so dragging a whole downloaded texture set onto the row fills the
//! channels in one gesture — which is the drawer's whole reason to exist.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora::core::CurrentProject;
use renzora_editor_framework::{open_asset_tab, AssetDragPayload, DocTabKind};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::{accent, rgb};

use renzora_shader::material::graph::{MaterialDomain, MaterialGraph};
use renzora_shader::material::material_ref::MaterialRef;
use renzora_shader::material::resolver::MaterialResolved;
use renzora_shader::material::texture_slots::{self, TextureSlot};

use crate::material_inspector::IMAGE_EXTENSIONS;

use super::create::MATERIALS_DIR;
use super::{material_path, MatCache, MatClearBtn, MatDropZone, MatEditBtn};

pub(super) fn mat_slot_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    payload: Option<Res<AssetDragPayload>>,
    zones: Query<(&RelativeCursorPosition, &MatDropZone)>,
    mut commands: Commands,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let Some(payload) = payload else { return };
    if !payload.is_detached {
        return;
    }
    let mut exts: Vec<&str> = vec!["material"];
    exts.extend_from_slice(IMAGE_EXTENSIONS);
    if !payload.matches_extensions(&exts) {
        return;
    }
    for (rcp, zone) in &zones {
        if !rcp.cursor_over {
            continue;
        }
        let dropped = dropped_paths(&payload);
        let entity = zone.entity;
        commands.queue(move |w: &mut World| apply_drop(w, entity, dropped));
        break;
    }
}

/// Every path in the drag. A multi-select drag fills `paths`; older single
/// drags only set `path`.
fn dropped_paths(payload: &AssetDragPayload) -> Vec<PathBuf> {
    if payload.paths.is_empty() {
        vec![payload.path.clone()]
    } else {
        payload.paths.clone()
    }
}

fn is_image(path: &std::path::Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    IMAGE_EXTENSIONS.contains(&ext.as_str())
}

/// Handle a drop on the material slot itself.
///
/// A single image whose name says nothing goes to base color — that is what one
/// unlabelled texture nearly always is — but in a multi-file drop the
/// unrecognised ones are left alone rather than fighting over the same slot in
/// whatever order the drag happened to list them.
fn apply_drop(world: &mut World, entity: Entity, dropped: Vec<PathBuf>) {
    if let Some(mat) = dropped.iter().find(|p| {
        p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("material"))
    }) {
        let mat_path = world
            .get_resource::<CurrentProject>()
            .map(|p| p.make_asset_relative(mat))
            .unwrap_or_else(|| mat.to_string_lossy().to_string());
        bind_material(world, entity, mat_path);
        return;
    }

    let images: Vec<PathBuf> = dropped.into_iter().filter(|p| is_image(p)).collect();
    if images.is_empty() {
        return;
    }
    let single = images.len() == 1;
    let routed: Vec<(Vec<&'static TextureSlot>, String)> = images
        .iter()
        .filter_map(|img| {
            let mut slots = texture_slots::guess_slots(img);
            if slots.is_empty() && single {
                slots = texture_slots::slot("base_color").into_iter().collect();
            }
            if slots.is_empty() {
                return None;
            }
            Some((slots, asset_relative(world, img)))
        })
        .collect();
    if routed.is_empty() {
        warn!("[material] dropped images don't name a texture channel; drop them on a slot row instead");
        return;
    }

    slot_edit(world, entity, move |graph| {
        let mut changed = false;
        for (slots, rel) in &routed {
            for slot in slots {
                changed |= texture_slots::set_slot_texture(graph, slot, rel);
            }
        }
        changed
    });
}

/// Project-relative form of a dropped file, which is what a graph stores.
pub(super) fn asset_relative(world: &World, path: &std::path::Path) -> String {
    world
        .get_resource::<CurrentProject>()
        .map(|p| p.make_asset_relative(path))
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

pub(super) fn bind_material(world: &mut World, entity: Entity, mat_path: String) {
    world.entity_mut(entity).remove::<MaterialResolved>();
    if let Some(mut mr) = world.get_mut::<MaterialRef>(entity) {
        mr.0 = mat_path;
    } else {
        world.entity_mut(entity).insert(MaterialRef(mat_path));
    }
}

/// Edit the entity's material graph, creating a material first if it has none,
/// and re-read the drawer afterwards.
///
/// Every texture-slot change goes through here so a drop behaves the same
/// whether the mesh already had a material or not — an entity with no
/// `MaterialRef` gets a fresh empty graph rather than refusing the drop.
pub(super) fn slot_edit(world: &mut World, entity: Entity, edit: impl FnOnce(&mut MaterialGraph) -> bool) {
    let Some(path) = ensure_material(world, entity) else { return };
    if crate::edit_material_graph(world, &path, edit) {
        // The drawer re-reads the file on a `rev` change; nothing else would
        // tell it the graph on disk moved under it.
        if let Some(mut cache) = world.get_resource_mut::<MatCache>() {
            cache.rev = cache.rev.wrapping_add(1);
        }
    }
}

/// The entity's material path, creating and binding an empty one if needed.
/// Returns `None` only when there is no project to write into.
pub(super) fn ensure_material(world: &mut World, entity: Entity) -> Option<String> {
    let existing = material_path(&Rx::new(&*world), entity);
    if !existing.is_empty() {
        return Some(existing);
    }
    create_material(world, entity)
}

/// Write a fresh empty `.material` under `<project>/materials/` and bind it to
/// `entity`, whatever it pointed at before.
fn create_material(world: &mut World, entity: Entity) -> Option<String> {
    let project_root = world.get_resource::<CurrentProject>().map(|p| p.path.clone())?;
    let stem = default_material_stem(world, entity);
    create_material_at(world, entity, &project_root.join(MATERIALS_DIR), &stem)
}

/// Name a new material after the mesh so the file is findable later; a generic
/// name for an unnamed entity. Sanitised, because this becomes a filename.
pub(super) fn default_material_stem(world: &World, entity: Entity) -> String {
    let base = world
        .get::<Name>(entity)
        .map(|n| n.as_str().to_string())
        .unwrap_or_else(|| "Material".to_string());
    sanitize_stem(&base)
}

pub(super) fn sanitize_stem(base: &str) -> String {
    base.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect()
}

/// Write `<dir>/<stem>.material` (uniquified) and bind it to `entity`.
///
/// Returns `None` when there's no project to write into, or the save failed —
/// in which case nothing is bound, so the mesh keeps whatever it had rather than
/// losing it to a file that isn't there.
pub(super) fn create_material_at(world: &mut World, entity: Entity, dir: &Path, stem: &str) -> Option<String> {
    let project_root = world.get_resource::<CurrentProject>().map(|p| p.path.clone())?;
    let stem = if stem.trim().is_empty() { "Material" } else { stem };
    let _ = std::fs::create_dir_all(dir);
    // Never write over a material that already exists — two meshes called
    // "Cube" must not end up silently sharing (and overwriting) one file.
    let mut fs_path = dir.join(format!("{stem}.material"));
    let mut n = 1;
    while fs_path.exists() {
        fs_path = dir.join(format!("{stem}_{n}.material"));
        n += 1;
    }

    let asset_path = renzora_shader::material::precompiled::project_relative(&project_root, &fs_path);
    let mut graph = MaterialGraph::new(stem, MaterialDomain::Surface);
    if !crate::save_material_graph(world, &asset_path, &mut graph) {
        return None;
    }
    bind_material(world, entity, asset_path.clone());
    Some(asset_path)
}

pub(super) fn mat_slot_drop_highlight(
    payload: Option<Res<AssetDragPayload>>,
    mut zones: Query<(&RelativeCursorPosition, &mut BorderColor), With<MatDropZone>>,
) {
    let mut exts: Vec<&str> = vec!["material"];
    exts.extend_from_slice(IMAGE_EXTENSIONS);
    for (rcp, mut bc) in &mut zones {
        let active = payload.as_ref().is_some_and(|p| p.is_detached && rcp.cursor_over && p.matches_extensions(&exts));
        let want = BorderColor::all(if active { rgb(accent()) } else { Color::NONE });
        if *bc != want {
            *bc = want;
        }
    }
}

pub(super) fn mat_edit_click(q: Query<(&Interaction, &MatEditBtn), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, b) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let e = b.entity;
        commands.queue(move |w: &mut World| {
            let path = material_path(&Rx::new(&*w), e);
            if path.is_empty() {
                return;
            }
            let abs = w.get_resource::<CurrentProject>().map(|p| p.resolve_path(&path)).unwrap_or_else(|| PathBuf::from(&path));
            open_asset_tab(w, &abs, DocTabKind::Material);
        });
    }
}

pub(super) fn mat_clear_click(q: Query<(&Interaction, &MatClearBtn), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, b) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let e = b.entity;
        commands.queue(move |w: &mut World| {
            w.entity_mut(e).remove::<MaterialRef>();
            w.entity_mut(e).remove::<MaterialResolved>();
            w.entity_mut(e).remove::<bevy::pbr::MeshMaterial3d<renzora_shader::material::runtime::GraphMaterial>>();
            let default_mat = w.resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial::default());
            w.entity_mut(e).insert(bevy::pbr::MeshMaterial3d(default_mat));
        });
    }
}
