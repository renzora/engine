//! Drag a scene asset (`.bsn` / `.ron`) from the asset browser onto the
//! hierarchy panel → spawn it as a nested `SceneInstance` at the scene root.
//!
//! This mirrors the viewport's `native_drop` arming model: the asset browser
//! removes the drag payload via a deferred command on mouse-up, so the release
//! frame can't read it directly. Instead [`arm_hier_scene_drop`] records the
//! candidate every frame *while* a compatible payload hovers the hierarchy, and
//! [`commit_hier_scene_drop`] consumes that armed snapshot on the release edge.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora::core::CurrentProject;
use renzora_editor_framework::{EditorCommands, EditorSelection};
use renzora_ember::widgets::PointerOverOverlay;
use renzora_ui::asset_drag::AssetDragPayload;
use renzora_ui::{DocumentTabState, Toasts};

/// Scene file extensions accepted by a hierarchy drop — kept in sync with the
/// viewport's `scene_drop::SCENE_EXTENSIONS`.
const SCENE_EXTENSIONS: &[&str] = &["bsn", "ron"];

/// Marks the hierarchy panel's root node as a scene-drop target (carries a
/// `RelativeCursorPosition` so the arm system can hit-test the cursor).
#[derive(Component)]
pub(crate) struct HierRoot;

/// The scene path last seen hovering the hierarchy. Captured while the drag is in
/// flight so the release frame doesn't have to re-read the (by-then-removed)
/// payload.
#[derive(Resource, Default)]
pub(crate) struct ArmedHierSceneDrop(Option<PathBuf>);

/// Every frame: arm the drop when a detached scene payload hovers the hierarchy
/// root; disarm if a payload is present but isn't a valid scene hover. When no
/// payload is present (e.g. the release frame, after the browser removed it) the
/// snapshot is left untouched so [`commit_hier_scene_drop`] can still consume it.
pub(crate) fn arm_hier_scene_drop(
    payload: Option<Res<AssetDragPayload>>,
    roots: Query<&RelativeCursorPosition, With<HierRoot>>,
    over_overlay: Option<Res<PointerOverOverlay>>,
    mut armed: ResMut<ArmedHierSceneDrop>,
) {
    let Some(payload) = payload else {
        return; // keep the last snapshot for the release frame
    };
    // A floating overlay (menu / popup) over the hierarchy owns the pointer — a
    // drop landing on it shouldn't fall through to the panel behind.
    let over = roots.iter().any(|rcp| rcp.cursor_over)
        && !over_overlay.is_some_and(|o| o.0);
    if payload.is_detached && over && payload.matches_extensions(SCENE_EXTENSIONS) {
        armed.0 = Some(payload.path.clone());
    } else {
        armed.0 = None;
    }
}

/// On the left-mouse-release edge, spawn the armed scene (if any) as a nested
/// instance at the scene root via [`EditorCommands`].
pub(crate) fn commit_hier_scene_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    mut armed: ResMut<ArmedHierSceneDrop>,
    cmds: Option<Res<EditorCommands>>,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let Some(path) = armed.0.take() else {
        return;
    };
    let Some(cmds) = cmds else {
        return;
    };
    cmds.push(move |world: &mut World| spawn_dropped_scene(world, path));
}

/// Spawn `path` as a `SceneInstance` at the scene root, rejecting a drop that
/// would make the active scene reference itself (a cycle). Mirrors the
/// viewport's `commit_scene_drop` self-reference guard.
fn spawn_dropped_scene(world: &mut World, path: PathBuf) {
    let host_abs = world.get_resource::<CurrentProject>().and_then(|p| {
        world
            .get_resource::<DocumentTabState>()
            .and_then(|t| t.tabs.get(t.active_tab).and_then(|tab| tab.scene_path.clone()))
            .map(|rel| p.resolve_path(&rel))
    });
    if let (Some(host_abs), Some(project_root)) = (
        host_abs,
        world.get_resource::<CurrentProject>().map(|p| p.path.clone()),
    ) {
        let mut cache = world
            .remove_resource::<renzora_engine::scene_io::SceneReferenceCache>()
            .unwrap_or_default();
        let cycle = renzora_engine::scene_io::would_create_reference_cycle(
            &mut cache,
            &project_root,
            &host_abs,
            &path,
        );
        world.insert_resource(cache);
        if cycle {
            if let Some(mut toasts) = world.get_resource_mut::<Toasts>() {
                toasts.warning(renzora::lang::t("hierarchy.toast.cannot_add_self"));
            }
            return;
        }
    }

    if let Some(entity) =
        renzora_engine::scene_io::spawn_scene_instance(world, &path, None, Transform::default())
    {
        if let Some(sel) = world.get_resource::<EditorSelection>() {
            sel.set(Some(entity));
        }
    }
}
