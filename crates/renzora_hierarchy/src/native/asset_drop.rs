//! Drag a script, blueprint or material from the asset browser onto a *row* in
//! the hierarchy → attach it to that entity.
//!
//! The sibling [`super::scene_drop`] handles a scene dropped anywhere on the
//! panel (it always spawns at the scene root, so it doesn't care which row it
//! landed on). This one is row-targeted: the entity under the cursor is the
//! thing being modified, so it hit-tests the rows themselves.
//!
//! Same arming model as every other asset drop in the editor, and for the same
//! reason: the asset browser removes the drag payload via a deferred command on
//! mouse-up, so the release frame can't read it. [`arm_hier_asset_drop`] records
//! the candidate every frame *while* a compatible payload hovers a row, and
//! [`commit_hier_asset_drop`] consumes that snapshot on the release edge.
//!
//! What lands where mirrors the badges the row already draws (see
//! `components::BadgeKind`), so a drop makes its own badge appear:
//! scripts *and* blueprints ride `ScriptComponent` (the scripting backend
//! compiles a `.blueprint` to Lua on load), and materials are a `MaterialRef`.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora::core::{CurrentProject, MaterialRef, MaterialResolved};
use renzora_editor_framework::{EditorCommands, EditorSelection};
use renzora_ember::widgets::PointerOverOverlay;
use renzora_scripting::ScriptComponent;
use renzora_ui::asset_drag::AssetDragPayload;
use renzora_ui::Toasts;
use renzora_undo::{record, SnapshotCmd, UndoContext};

use super::components::{HierPinClick, HierRowArea};
use super::scene_drop::HierRoot;

const SCRIPT_EXTENSIONS: &[&str] = &["lua"];
const BLUEPRINT_EXTENSIONS: &[&str] = &["blueprint", "bp"];
const MATERIAL_EXTENSIONS: &[&str] = &["material"];

/// What a hovering payload would do to the row under it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttachKind {
    Script,
    Blueprint,
    Material,
}

impl AttachKind {
    fn extensions(self) -> &'static [&'static str] {
        match self {
            AttachKind::Script => SCRIPT_EXTENSIONS,
            AttachKind::Blueprint => BLUEPRINT_EXTENSIONS,
            AttachKind::Material => MATERIAL_EXTENSIONS,
        }
    }

    /// Undo-stack label for this attachment.
    fn undo_label(self) -> &'static str {
        match self {
            AttachKind::Script => "Attach Script",
            AttachKind::Blueprint => "Attach Blueprint",
            AttachKind::Material => "Assign Material",
        }
    }

    fn toast_key(self) -> &'static str {
        match self {
            AttachKind::Script => "hierarchy.drop.attached_script",
            AttachKind::Blueprint => "hierarchy.drop.attached_blueprint",
            AttachKind::Material => "hierarchy.drop.applied_material",
        }
    }
}

/// The drop candidate last seen hovering a hierarchy row, captured while the
/// drag is in flight so the release frame doesn't have to re-read the (by then
/// removed) payload.
#[derive(PartialEq)]
struct Armed {
    target: Entity,
    kind: AttachKind,
    /// Every path in the drag that matches `kind` (a multi-select drag can carry
    /// several scripts).
    paths: Vec<PathBuf>,
}

#[derive(Resource, Default)]
pub(crate) struct ArmedHierAssetDrop(Option<Armed>);

impl ArmedHierAssetDrop {
    /// The row currently armed for an attach, for the row's drop-target tint.
    pub(crate) fn target(&self) -> Option<Entity> {
        self.0.as_ref().map(|a| a.target)
    }
}

/// Classify a payload by its primary path — the file the user grabbed decides
/// the kind, and a mixed multi-select then contributes only its files of that
/// same kind rather than doing two unrelated things in one gesture.
fn classify(payload: &AssetDragPayload) -> Option<AttachKind> {
    [
        AttachKind::Script,
        AttachKind::Blueprint,
        AttachKind::Material,
    ]
    .into_iter()
    .find(|k| payload.matches_extensions(k.extensions()))
}

fn has_extension(path: &std::path::Path, exts: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| exts.iter().any(|a| e.eq_ignore_ascii_case(a)))
}

/// Every frame: arm the drop when a compatible, detached payload hovers a row;
/// disarm when a payload is present but isn't over a valid target. When no
/// payload exists (the release frame, after the browser removed it) the snapshot
/// is left alone so [`commit_hier_asset_drop`] can still consume it.
pub(crate) fn arm_hier_asset_drop(
    payload: Option<Res<AssetDragPayload>>,
    rows: Query<(&RelativeCursorPosition, &HierRowArea)>,
    pins: Query<(&RelativeCursorPosition, &HierPinClick)>,
    roots: Query<&RelativeCursorPosition, With<HierRoot>>,
    over_overlay: Option<Res<PointerOverOverlay>>,
    mut armed: ResMut<ArmedHierAssetDrop>,
) {
    let Some(payload) = payload else {
        return; // keep the last snapshot for the release frame
    };

    // A floating overlay (menu / popup) over the hierarchy owns the pointer — a
    // drop landing on it shouldn't fall through to the row behind. The panel
    // root has to be hovered too, so a row that is only *geometrically* under
    // the cursor (scrolled behind another panel) can't arm.
    let want = (payload.is_detached
        && !over_overlay.is_some_and(|o| o.0)
        && roots.iter().any(|rcp| rcp.cursor_over))
    .then(|| {
        let kind = classify(&payload)?;
        // A sticky parent-stack header is drawn over the rows it scrolled past,
        // so it wins when both report the cursor — otherwise the drop would
        // land on whichever row is hidden behind it.
        let target = pins
            .iter()
            .find(|(rcp, _)| rcp.cursor_over)
            .map(|(_, pin)| pin.entity)
            .or_else(|| {
                rows.iter()
                    .find(|(rcp, _)| rcp.cursor_over)
                    .map(|(_, row)| row.entity)
            })?;
        // `paths` is empty on older single-file drags, which only fill `path`.
        let all = if payload.paths.is_empty() {
            std::slice::from_ref(&payload.path)
        } else {
            payload.paths.as_slice()
        };
        let paths: Vec<PathBuf> = all
            .iter()
            .filter(|p| has_extension(p, kind.extensions()))
            .cloned()
            .collect();
        (!paths.is_empty()).then_some(Armed {
            target,
            kind,
            paths,
        })
    })
    .flatten();

    // Only write through `ResMut` when the value actually moved: the row
    // background binds to this resource, and marking it changed every frame
    // would re-evaluate that binding for every visible row on every frame.
    if armed.0 != want {
        armed.0 = want;
    }
}

/// On the left-mouse-release edge, apply the armed attachment through
/// [`EditorCommands`] (the queue that owns exclusive-world access).
pub(crate) fn commit_hier_asset_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    mut armed: ResMut<ArmedHierAssetDrop>,
    cmds: Option<Res<EditorCommands>>,
) {
    // Checked before `take()` so an ordinary click somewhere else in the editor
    // doesn't mark this resource changed — the row backgrounds bind to it.
    if !mouse.just_released(MouseButton::Left) || armed.0.is_none() {
        return;
    }
    let Some(Armed {
        target,
        kind,
        paths,
    }) = armed.0.take()
    else {
        return;
    };
    let Some(cmds) = cmds else {
        return;
    };
    cmds.push(move |world: &mut World| apply_attach(world, target, kind, paths));
}

fn apply_attach(world: &mut World, target: Entity, kind: AttachKind, paths: Vec<PathBuf>) {
    if world.get_entity(target).is_err() {
        return; // the row was deleted mid-drag
    }
    let rel: Vec<String> = {
        let project = world.get_resource::<CurrentProject>();
        paths
            .iter()
            .map(|p| match project {
                Some(project) => project.make_asset_relative(p),
                None => p.to_string_lossy().replace('\\', "/"),
            })
            .collect()
    };

    let applied = match kind {
        AttachKind::Script | AttachKind::Blueprint => attach_scripts(world, target, kind, &rel),
        AttachKind::Material => assign_material(world, target, &rel[0]),
    };
    if !applied {
        return;
    }

    // Select what was just changed so the inspector shows the new script /
    // material without a second click.
    if let Some(sel) = world.get_resource::<EditorSelection>() {
        sel.set(Some(target));
    }

    let name = file_name(&paths[0]);
    let entity_name = world
        .get::<Name>(target)
        .map(|n| n.as_str().to_string())
        .unwrap_or_else(|| renzora::lang::t("hierarchy.drop.the_entity"));
    if let Some(mut toasts) = world.get_resource_mut::<Toasts>() {
        toasts.success(
            renzora::lang::t(kind.toast_key())
                .replace("{name}", &name)
                .replace("{entity}", &entity_name),
        );
    }
}

fn file_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Snapshot of the `ScriptComponent` on one entity (`None` = it had none), the
/// payload [`SnapshotCmd`] restores on undo.
type ScriptSnapshot = (Entity, Option<ScriptComponent>);

fn restore_scripts(world: &mut World, snap: &ScriptSnapshot) {
    let (entity, comp) = snap;
    let Ok(mut em) = world.get_entity_mut(*entity) else {
        return;
    };
    match comp {
        Some(c) => {
            em.insert(c.clone());
        }
        None => {
            em.remove::<ScriptComponent>();
        }
    }
}

/// Append each dropped file to the entity's `ScriptComponent`, creating one if
/// the entity has no scripts yet. Returns whether anything changed.
fn attach_scripts(world: &mut World, target: Entity, kind: AttachKind, rel: &[String]) -> bool {
    let before: ScriptSnapshot = (target, world.get::<ScriptComponent>(target).cloned());
    let mut comp = before.1.clone().unwrap_or_default();
    for r in rel {
        // Re-attaching a file the entity already runs would silently run it
        // twice, which reads as a bug rather than as an attach.
        let already = comp.scripts.iter().any(|e| {
            e.script_path
                .as_ref()
                .is_some_and(|p| p.as_path() == std::path::Path::new(r))
        });
        if !already {
            comp.add_file_script(PathBuf::from(r));
        }
    }
    if comp.scripts.len() == before.1.as_ref().map_or(0, |c| c.scripts.len()) {
        // Every dropped file was already attached. Say so rather than letting
        // the drop look like it was swallowed.
        let name = rel.first().map(|r| file_name(std::path::Path::new(r)));
        if let (Some(name), Some(mut toasts)) = (name, world.get_resource_mut::<Toasts>()) {
            toasts.info(renzora::lang::t("hierarchy.drop.already_attached").replace("{name}", &name));
        }
        return false;
    }
    world.entity_mut(target).insert(comp.clone());
    record(
        world,
        UndoContext::Scene,
        Box::new(SnapshotCmd {
            label: kind.undo_label().to_string(),
            before,
            after: (target, Some(comp)),
            restore: restore_scripts,
            merge_key: None,
        }),
    );
    true
}

/// Snapshot of `MaterialRef` across the entities a material drop touched.
type MaterialSnapshot = Vec<(Entity, Option<String>)>;

/// Write a snapshot into the world. [`SnapshotCmd`] calls it to undo and redo;
/// [`assign_material`] also calls it directly to make the edit in the first
/// place, so all three paths are the same code.
fn write_materials(world: &mut World, snap: &MaterialSnapshot) {
    for (entity, path) in snap {
        let Ok(mut em) = world.get_entity_mut(*entity) else {
            continue;
        };
        // Always drop the resolved marker — it's what makes the resolver pick
        // the entity up again on the next frame.
        em.remove::<MaterialResolved>();
        match path {
            Some(p) => {
                em.insert(MaterialRef(p.clone()));
            }
            None => {
                em.remove::<MaterialRef>();
            }
        }
    }
}

/// Bind a `.material` to the dropped-on entity — or, when that entity is a
/// model root with no mesh of its own, to every mesh beneath it.
///
/// The viewport's material drop raycasts, so it always lands on a real mesh. A
/// hierarchy row is usually the *logical* entity instead: an imported model's
/// root carries the name you dropped on but no `Mesh3d`, so binding only there
/// would write a `MaterialRef` nothing ever renders and look like the drop did
/// nothing. Returns whether anything was bound.
fn assign_material(world: &mut World, target: Entity, rel: &str) -> bool {
    let targets = mesh_targets(world, target);
    if targets.is_empty() {
        let entity_name = world
            .get::<Name>(target)
            .map(|n| n.as_str().to_string())
            .unwrap_or_default();
        if let Some(mut toasts) = world.get_resource_mut::<Toasts>() {
            toasts.warning(
                renzora::lang::t("hierarchy.drop.no_mesh").replace("{entity}", &entity_name),
            );
        }
        return false;
    }

    let before: MaterialSnapshot = targets
        .iter()
        .map(|e| (*e, world.get::<MaterialRef>(*e).map(|m| m.0.clone())))
        .collect();
    let after: MaterialSnapshot = targets.iter().map(|e| (*e, Some(rel.to_string()))).collect();
    write_materials(world, &after);
    record(
        world,
        UndoContext::Scene,
        Box::new(SnapshotCmd {
            label: AttachKind::Material.undo_label().to_string(),
            before,
            after,
            restore: write_materials,
            merge_key: None,
        }),
    );
    true
}

/// The entity itself if it has a mesh, otherwise every mesh in its subtree.
fn mesh_targets(world: &World, root: Entity) -> Vec<Entity> {
    if world.get::<Mesh3d>(root).is_some() {
        return vec![root];
    }
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if let Some(children) = world.get::<Children>(e) {
            stack.extend(children.iter());
        }
        if e != root && world.get::<Mesh3d>(e).is_some() {
            out.push(e);
        }
    }
    out
}
