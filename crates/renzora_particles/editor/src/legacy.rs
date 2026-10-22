//! Old Hanabi effects in the editor: made editable without the user asking.
//!
//! The runtime already plays them (`renzora_particles::legacy`): a `.particle`
//! loads by converting in memory, and a scene's old component becomes a
//! Particle System pointing at its `.particle`. That is enough to play, but
//! not to edit, because the effect editor edits `.vfx` files. So the editor
//! finishes the job:
//!
//! 1. **On project open**, every `.particle` in the project that has no
//!    `<name>.vfx` beside it gets one ([`convert_project`]), written by the
//!    same converter, on a background task so a large project does not stall
//!    the frame. Each file is reported in the Console with what did not carry
//!    over. The `.particle` is never modified, moved or deleted, and an
//!    existing `.vfx` of that name is never replaced, edited or not.
//! 2. **Scene effects follow**: a Particle System whose path is a `.particle`
//!    with a `.vfx` beside it is re-pointed at the `.vfx`
//!    ([`repoint_old_effects`]), on load and once more when the project pass
//!    finishes. Saving the scene then stores the `.vfx`.
//! 3. **Inline effects get a file**: an old component that held its effect
//!    inside the scene plays from memory, which a scene cannot save, so the
//!    conversion is written to `effects/<name>.vfx` and the entity pointed at
//!    it ([`write_inline_effects`]).
//! 4. **Opening a `.particle`** opens its `.vfx` instead, converting first if
//!    the project pass has not reached it ([`open_particle_documents_as_vfx`]).
//!    The tab is retargeted in place, so the open-tabs list a project restores
//!    stops naming the old file too.
//!
//! The one name used throughout is the plain sibling, `fire.particle` to
//! `fire.vfx` (`renzora_particles::convert::sibling_vfx`), so the pass, the
//! re-pointing and the open redirect agree on which file belongs to which with
//! no record kept between them. A `fire.vfx` the user made themselves before
//! any of this is therefore taken as the conversion, which is what they would
//! want: it is the effect they made to replace it.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::tasks::{futures::check_ready, IoTaskPool, Task};
use renzora::core::CurrentProject;
use renzora_editor_framework::{DocTabKind, DocumentTabState, EditorContext, Toasts};
use renzora_particles::legacy::{InlineLegacyEffect, CONSOLE_CATEGORY};
use renzora_particles::ParticleSystem;

use crate::convert::{self, Converted};
use crate::convert_actions;

/// How deep the project walk goes. Far deeper than any real asset tree, and
/// only there so a symlink loop cannot walk forever.
const MAX_DEPTH: usize = 32;

/// Folders the walk never enters: hidden ones (`.git`, the editor's own
/// caches) and a build's `target`, none of which hold project assets, and any
/// of which can be far larger than the project itself.
fn skip_dir(name: &str) -> bool {
    name.starts_with('.') || name.eq_ignore_ascii_case("target")
}

/// Every `.particle` under `root`, sorted, so the Console lists them in a
/// stable order.
pub fn find_particle_files(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        if depth > MAX_DEPTH {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_dir() {
                if !path.file_name().and_then(|n| n.to_str()).is_some_and(skip_dir) {
                    walk(&path, depth + 1, out);
                }
            } else if kind.is_file() && path.to_str().is_some_and(convert::is_particle_path) {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, 0, &mut out);
    out.sort();
    out
}

/// One file's outcome in the project pass.
#[derive(Debug)]
pub struct FileResult {
    pub particle: PathBuf,
    /// `Ok(None)`: a `.vfx` was already there, so nothing was written.
    pub result: Result<Option<Converted>, String>,
}

/// Write the missing `.vfx` for every `.particle` under `root`. Blocking file
/// work; the editor runs it on the IO pool.
pub fn convert_project(root: &Path) -> Vec<FileResult> {
    find_particle_files(root)
        .into_iter()
        .map(|particle| {
            let result = convert::convert_if_missing(&particle);
            FileResult { particle, result }
        })
        .collect()
}

/// The project pass in flight, and which project it was started for.
#[derive(Resource, Default)]
pub struct LegacyScan {
    project: Option<PathBuf>,
    task: Option<Task<Vec<FileResult>>>,
    /// Set when a pass finishes, so [`repoint_old_effects`] looks at every
    /// Particle System once rather than only the changed ones: the scene was
    /// usually loaded before the files it now points at existed.
    sweep: bool,
}

/// Start the project pass when a project opens (or a different one does).
pub fn start_scan(project: Option<Res<CurrentProject>>, mut scan: ResMut<LegacyScan>) {
    let root = project.map(|p| p.path.clone());
    if scan.project == root {
        return;
    }
    scan.project = root.clone();
    scan.task = root.map(|root| IoTaskPool::get().spawn(async move { convert_project(&root) }));
}

/// Report a finished pass in the Console.
pub fn finish_scan(mut scan: ResMut<LegacyScan>, project: Option<Res<CurrentProject>>) {
    let Some(task) = scan.task.as_mut() else { return };
    let Some(results) = check_ready(task) else { return };
    scan.task = None;
    scan.sweep = true;
    let rel = |p: &Path| -> String {
        project
            .as_ref()
            .and_then(|pr| p.strip_prefix(&pr.path).ok())
            .map(|r| r.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|| p.display().to_string())
    };
    let (mut written, mut failed) = (0usize, 0usize);
    for r in &results {
        match &r.result {
            Ok(Some(converted)) => {
                written += 1;
                convert_actions::log(&rel(&r.particle), converted);
            }
            Ok(None) => {}
            Err(e) => {
                failed += 1;
                renzora::console_log::console_warn(
                    CONSOLE_CATEGORY,
                    format!("{}: {e}. It was left as it is and will not play until it is fixed.", rel(&r.particle)),
                );
            }
        }
    }
    if written > 0 || failed > 0 {
        let mut line = format!(
            "Old particle effects: converted {written} .particle file(s) to .vfx beside the originals, which are unchanged"
        );
        if failed > 0 {
            line.push_str(&format!("; {failed} could not be read"));
        }
        renzora::console_log::console_info(CONSOLE_CATEGORY, line);
    }
}

/// The `.vfx` a scene path should use instead, if `path` is a project
/// `.particle` with its conversion beside it.
fn converted_path(project: &CurrentProject, path: &str) -> Option<String> {
    // `embedded://` and other sources are not project files to look beside.
    if !convert::is_particle_path(path) || path.contains("://") {
        return None;
    }
    let vfx = convert::sibling_vfx_str(path);
    project.resolve_path(&vfx).is_file().then_some(vfx)
}

/// Point Particle Systems at the `.vfx` beside their `.particle` (see the
/// module docs). Changed ones every frame (a scene load, a paste, a script),
/// every one after a project pass.
pub fn repoint_old_effects(
    project: Option<Res<CurrentProject>>,
    mut scan: ResMut<LegacyScan>,
    mut systems: Query<&mut ParticleSystem>,
) {
    let Some(project) = project else { return };
    let sweep = std::mem::take(&mut scan.sweep);
    for mut system in &mut systems {
        if !(sweep || system.is_changed()) {
            continue;
        }
        if let Some(vfx) = converted_path(&project, &system.path) {
            system.path = vfx;
        }
    }
}

/// A file name from an effect's display name: what a filesystem accepts on
/// every platform, never empty.
fn file_stem_for(name: &str) -> String {
    let stem: String = name.chars().filter(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | ' ')).collect();
    let stem = stem.trim();
    if stem.is_empty() {
        "effect".to_string()
    } else {
        stem.to_string()
    }
}

/// Give each inline old effect a `.vfx` in `effects/` and point its entity at
/// it (see the module docs). Never overwrites: the name is the first free one,
/// and a file holding exactly this conversion is reused, so loading the same
/// unsaved scene twice does not make a second copy.
pub fn write_inline_effects(
    mut commands: Commands,
    project: Option<Res<CurrentProject>>,
    mut effects: Query<(Entity, &InlineLegacyEffect, &mut ParticleSystem, Option<&Name>)>,
    mut toasts: Option<ResMut<Toasts>>,
) {
    let Some(project) = project else { return };
    for (entity, inline, mut system, name) in &mut effects {
        commands.entity(entity).remove::<InlineLegacyEffect>();
        let display = if inline.conversion.asset.name.trim().is_empty() {
            name.map(|n| n.as_str().to_string()).unwrap_or_default()
        } else {
            inline.conversion.asset.name.clone()
        };
        let dir = project.path.join("effects");
        let derived_from = dir.join(format!("{}.particle", file_stem_for(&display)));
        let written = std::fs::create_dir_all(&dir)
            .map_err(|e| format!("could not create {}: {e}", dir.display()))
            .and_then(|_| convert::write_beside_as(&derived_from, "an effect stored in the scene", &inline.conversion));
        match written {
            Ok(converted) => {
                let rel = project
                    .make_relative(&converted.path)
                    .unwrap_or_else(|| converted.path.to_string_lossy().replace('\\', "/"));
                convert_actions::log(&format!("The inline effect \"{display}\""), &converted);
                system.path = rel;
            }
            Err(e) => {
                renzora::console_log::console_error(
                    CONSOLE_CATEGORY,
                    format!("The inline effect \"{display}\" plays, but could not be written to a .vfx, so saving the scene will lose it: {e}"),
                );
                if let Some(toasts) = toasts.as_mut() {
                    toasts.warning(format!("Could not save the old effect \"{display}\" as a .vfx (see the Console)"));
                }
            }
        }
    }
}

/// Open a `.particle` document as its `.vfx` (see the module docs).
///
/// Exclusive because opening a tab is a `World` operation. `failed` holds the
/// last path that could not be converted, so a broken file reports once rather
/// than every frame it stays the active document.
pub fn open_particle_documents_as_vfx(world: &mut World, mut failed: Local<Option<String>>) {
    let Some(EditorContext::Asset { path, kind: DocTabKind::Particle }) = world.get_resource::<EditorContext>().cloned() else {
        return;
    };
    if !convert::is_particle_path(&path) {
        *failed = None;
        return;
    }
    if failed.as_deref() == Some(path.as_str()) {
        return;
    }
    let Some(project) = world.get_resource::<CurrentProject>() else { return };
    let abs = project.resolve_path(&path);
    let vfx_abs = convert::sibling_vfx(&abs);
    let vfx_rel = convert::sibling_vfx_str(&path);

    if !vfx_abs.exists() {
        match convert::convert_if_missing(&abs) {
            Ok(Some(converted)) => convert_actions::log(&path, &converted),
            Ok(None) => {}
            Err(e) => {
                renzora::console_log::console_error(CONSOLE_CATEGORY, format!("{path}: {e}"));
                if let Some(mut toasts) = world.get_resource_mut::<Toasts>() {
                    toasts.warning(format!("Could not open {path}: it is not a valid effect"));
                }
                *failed = Some(path);
                return;
            }
        }
    }
    *failed = None;

    let retargeted = {
        let Some(mut tabs) = world.get_resource_mut::<DocumentTabState>() else { return };
        let old = tabs.find_by_path(&path, DocTabKind::Particle);
        let existing = tabs.find_by_path(&vfx_rel, DocTabKind::Particle);
        match (old, existing) {
            // Retarget the tab in place, which keeps its position in the strip
            // and needs no tab switch.
            (Some(old), None) => {
                let tab = &mut tabs.tabs[old];
                tab.scene_path = Some(vfx_rel.clone());
                tab.name = vfx_abs.file_stem().and_then(|s| s.to_str()).unwrap_or("effect").to_string();
                true
            }
            // The `.vfx` is already open: close the `.particle` tab and show
            // that one instead.
            (Some(old), Some(_)) => {
                tabs.close_tab(old);
                false
            }
            (None, _) => false,
        }
    };
    if retargeted {
        world.insert_resource(EditorContext::Asset { path: vfx_rel, kind: DocTabKind::Particle });
    } else {
        renzora_editor_framework::open_asset_tab(world, &vfx_abs, DocTabKind::Particle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_particles::convert::format::HanabiEffectDefinition;

    fn temp_project(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("renzora_legacy_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn particle(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, ron::to_string(&HanabiEffectDefinition::default()).unwrap()).unwrap();
    }

    #[test]
    fn the_project_pass_converts_what_is_missing_and_touches_nothing_else() {
        let root = temp_project("pass");
        particle(&root.join("particles/fire/torch.particle"));
        particle(&root.join("particles/smoke.particle"));
        // Already converted (and since edited): left exactly as it is.
        std::fs::write(root.join("particles/smoke.vfx"), "(name: \"tuned\")").unwrap();
        // Hidden folders are never entered.
        particle(&root.join(".cache/stale.particle"));
        std::fs::write(root.join("particles/broken.particle"), "(not an effect").unwrap();
        let before: Vec<Vec<u8>> = ["particles/fire/torch.particle", "particles/smoke.particle", "particles/broken.particle"]
            .iter()
            .map(|p| std::fs::read(root.join(p)).unwrap())
            .collect();

        let results = convert_project(&root);
        assert_eq!(results.len(), 3, "{results:?}");
        let by_name = |n: &str| results.iter().find(|r| r.particle.ends_with(n)).unwrap();
        assert!(matches!(by_name("torch.particle").result, Ok(Some(_))));
        assert!(matches!(by_name("smoke.particle").result, Ok(None)));
        assert!(by_name("broken.particle").result.is_err());

        assert!(root.join("particles/fire/torch.vfx").is_file());
        assert_eq!(std::fs::read_to_string(root.join("particles/smoke.vfx")).unwrap(), "(name: \"tuned\")");
        assert!(!root.join("particles/broken.vfx").exists());
        assert!(!root.join(".cache/stale.vfx").exists());
        let after: Vec<Vec<u8>> = ["particles/fire/torch.particle", "particles/smoke.particle", "particles/broken.particle"]
            .iter()
            .map(|p| std::fs::read(root.join(p)).unwrap())
            .collect();
        assert_eq!(before, after, "no .particle is ever modified");

        // A second pass has nothing left to do.
        assert!(convert_project(&root).iter().all(|r| !matches!(r.result, Ok(Some(_)))));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_scene_path_moves_to_the_vfx_only_when_it_exists() {
        let root = temp_project("repoint");
        particle(&root.join("particles/fire.particle"));
        let project = CurrentProject { path: root.clone(), config: Default::default() };
        assert_eq!(converted_path(&project, "particles/fire.particle"), None, "nothing to point at yet");
        std::fs::write(root.join("particles/fire.vfx"), "()").unwrap();
        assert_eq!(converted_path(&project, "particles/fire.particle").as_deref(), Some("particles/fire.vfx"));
        assert_eq!(converted_path(&project, "particles/fire.vfx"), None);
        assert_eq!(converted_path(&project, "embedded://x/fire.particle"), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_effect_name_becomes_a_safe_file_stem() {
        assert_eq!(file_stem_for("Camp Fire"), "Camp Fire");
        assert_eq!(file_stem_for("a/b:c"), "abc");
        assert_eq!(file_stem_for("  "), "effect");
    }
}
