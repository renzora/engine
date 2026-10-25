//! Keep the paths stored inside project files pointing at an asset after it
//! moves.
//!
//! [`renzora::core::AssetPathChanged`] already had subscribers, but every one
//! of them patches something live: a component on a scene entity, the
//! project's scene list, an open tab. Nothing followed the paths written into
//! files on disk, and that is where most references live. An imported model's
//! `.material` files name each texture by its full project path, so importing
//! into the wrong folder and dragging the model out left every material
//! pointing at textures in the folder it had just left. The model then loaded
//! untextured, from a fresh drop or from any scene, with nothing on screen to
//! say why, only a wall of "Path not found" in the log.
//!
//! This observer rewrites those files, whoever fired the event: the asset
//! browser's drag-to-folder and rename, or a scene renamed from its tab.
//!
//! # The open scene
//!
//! The open scene's `.bsn` is one of those files, and rewriting it on disk is
//! not enough: the scene in memory still holds the old paths, and the next save
//! writes them straight back over the fixed file. Only a model's path and an
//! animator's clips used to be patched in memory (each by its own subscriber),
//! so a renamed texture, script, sound or particle effect survived the rename
//! and was lost on the next Ctrl+S, which is the opposite of what anyone
//! watching would conclude (GH #80).
//!
//! So the open scene is patched by reflection: every string or path field of
//! every component on a scene entity, held to the same rule as the text
//! rewrite (the whole value is the moved path, or for a folder begins with it).
//! One rule for the file and for the memory is what keeps the two agreeing,
//! and a component type added later is covered without anyone remembering to
//! add a subscriber for it.

use std::any::TypeId;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::reflect::{PartialReflect, ReflectMut, ReflectRef};
use renzora::core::project_files::{is_ignored, SelfWrites};
use renzora::core::{AssetPathChanged, CurrentProject, HideInHierarchy};
use renzora_editor_framework::Toasts;

/// How deep the walk goes. Far past any real project, and a stop for a
/// symlink loop.
const MAX_DEPTH: usize = 32;

/// Extensions of the data formats the editor writes that store project paths.
///
/// Scripts are added to these at run time, from whatever languages are
/// installed (see [`rewrite_project_references`]).
const REFERENCING_EXTENSIONS: &[&str] = &[
    "material",
    "material_bp",
    "material_function",
    "bsn",
    "particle",
    // The particle format the effect library writes, which names its texture.
    "vfx",
    "html",
    "anim",
];

/// Rewrite every file in the project that refers to the moved asset.
///
/// Scripts are included. A script that plays `"audio/click.mp3"` went silent
/// when the sound moved, with nothing to say why. The rewrite is the same one
/// every other file gets: only a string literal that is exactly the moved path
/// (or, for a folder, starts with it) changes, so a path in a bare comment is
/// left as written, and a path built from pieces (`"audio/" .. name`) cannot be
/// matched at all. Every script changed is named in the Console, because it is
/// code somebody wrote.
///
/// The script extensions come from the installed language backends rather
/// than a list here, so a language added by a plugin is covered with it.
pub(crate) fn rewrite_project_references(
    trigger: On<AssetPathChanged>,
    project: Option<Res<CurrentProject>>,
    engine: Option<Res<renzora_scripting::ScriptEngine>>,
    mut commands: Commands,
) {
    let Some(project) = project else {
        return;
    };
    let ev = trigger.event().clone();
    let script_exts: Vec<String> = engine
        .map(|e| e.script_extensions().iter().map(|x| x.to_ascii_lowercase()).collect())
        .unwrap_or_default();
    let mut rewritten: Vec<(PathBuf, String)> = Vec::new();
    let mut scripts: Vec<String> = Vec::new();
    let mut failed: Vec<String> = Vec::new();

    for path in referencing_files(&project.path, &script_exts) {
        // Not valid UTF-8 is a binary file wearing a text extension (an `.anim`
        // can be either), and nothing in it is ours to rewrite.
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some(updated) = ev.rewrite_text(&text) else {
            continue;
        };
        if let Err(e) = std::fs::write(&path, &updated) {
            warn!("[asset-move] could not update {}: {e}", path.display());
            failed.push(format!("{} ({e})", project.make_asset_relative(&path)));
            continue;
        }
        if has_extension(&path, &script_exts) {
            scripts.push(project.make_asset_relative(&path));
        }
        rewritten.push((path, updated));
    }

    // Queued rather than done here, so it runs after every other subscriber to
    // this event. Some of them do more than rewrite a path (the animator one
    // also resets the animator so the new clips load), and they only do it when
    // they find the old path still there. Patching first would leave them
    // nothing to find.
    commands.queue(move |world: &mut World| {
        // Claim the scene writes. The open scene is patched in memory just
        // below, so the watcher event this write causes is our own echo and
        // must not reload the scene (or, with unsaved changes, raise a conflict
        // over it). Everything else is left to look external on purpose: a
        // rewritten `.material` or `.particle` should reload.
        {
            let mut self_writes = world.get_resource_or_insert_with(SelfWrites::default);
            for (path, text) in &rewritten {
                if path.extension().and_then(|e| e.to_str()) == Some("bsn") {
                    self_writes.record(path, text.as_bytes());
                }
            }
        }
        let live = rewrite_open_scene(world, &ev);
        failed.extend(live.failed);
        report(world, &ev, rewritten.len(), live.values, &failed);
        if !scripts.is_empty() {
            renzora::console_log::console_info(
                "Assets",
                format!("Updated the path to {} in: {}", ev.new, scripts.join(", ")),
            );
        }
    });
}

/// Say how far a move reached, and what it could not fix.
///
/// A rename that silently did nothing and one that silently fixed forty
/// references look the same, so the count goes to the Console and a toast
/// every time there is one. A failure gets a warning toast instead: a
/// reference left on the old path is a broken texture or a script that no
/// longer runs, and it should not be found later by accident.
fn report(world: &mut World, ev: &AssetPathChanged, files: usize, values: usize, failed: &[String]) {
    if files > 0 || values > 0 {
        let message = match (files, values) {
            (f, 0) => format!("Moved {}: updated {f} file(s) that referenced it", ev.new),
            (0, v) => format!("Moved {}: updated {v} reference(s) in the open scene", ev.new),
            (f, v) => format!(
                "Moved {}: updated {f} file(s) and {v} reference(s) in the open scene",
                ev.new
            ),
        };
        info!("[asset-move] '{}' -> '{}': {message}", ev.old, ev.new);
        renzora::console_log::console_info("Assets", message.clone());
        // Unless it went wrong, when the warning below says the same thing and
        // more. Two toasts for one move would be noise.
        if failed.is_empty() {
            if let Some(mut toasts) = world.get_resource_mut::<Toasts>() {
                toasts.success(message);
            }
        }
    }
    if failed.is_empty() {
        return;
    }
    renzora::console_log::console_warn(
        "Assets",
        format!(
            "Moved {}, but {} reference(s) still point at {}: {}",
            ev.new,
            failed.len(),
            ev.old,
            failed.join(", ")
        ),
    );
    if let Some(mut toasts) = world.get_resource_mut::<Toasts>() {
        toasts.warning(format!(
            "{} reference(s) to {} could not be updated. See the Console.",
            failed.len(),
            ev.old
        ));
    }
}

/// What patching the open scene in memory did.
#[derive(Default)]
struct LiveRewrite {
    /// String and path values rewritten.
    values: usize,
    /// `entity: Component` for each component that held the old path and could
    /// not be rewritten.
    failed: Vec<String>,
}

/// Rewrite every reference to the moved asset held by a component on a scene
/// entity.
///
/// A scene entity is one the scene save would write: named, not marked
/// [`HideInHierarchy`] and not under anything that is. That is the line between
/// the scene and the editor's own entities, whose stored paths (a preview's
/// model, a thumbnail's source) are not references the user made.
///
/// Each component is read first and only cloned and written back when it holds
/// the old path, because writing goes through `Mut` and marks the component
/// changed. That is wanted for the ones that did change (a `Changed<T>` system
/// reloads from the new path), and would be a needless reload of everything
/// else.
fn rewrite_open_scene(world: &mut World, ev: &AssetPathChanged) -> LiveRewrite {
    let mut out = LiveRewrite::default();
    let entities: Vec<Entity> = world
        .query_filtered::<Entity, (With<Name>, Without<HideInHierarchy>)>()
        .iter(world)
        .filter(|&e| !has_hidden_ancestor(world, e))
        .collect();

    let registry = world.resource::<AppTypeRegistry>().clone();
    let registry = registry.read();
    for entity in entities {
        let type_ids: Vec<TypeId> = world
            .inspect_entity(entity)
            .map(|it| it.filter_map(|(_, info)| info.type_id()).collect())
            .unwrap_or_default();
        for type_id in type_ids {
            let Some(registration) = registry.get(type_id) else {
                continue;
            };
            let Some(reflect_component) = registration.data::<ReflectComponent>() else {
                continue;
            };
            let Ok(entity_ref) = world.get_entity(entity) else {
                continue;
            };
            let Some(current) = reflect_component.reflect(entity_ref) else {
                continue;
            };
            if !holds_reference(current.as_partial_reflect(), ev) {
                continue;
            }
            let label = || {
                let name = world.get::<Name>(entity).map(|n| n.as_str()).unwrap_or("?");
                format!("{name}: {}", registration.type_info().type_path_table().short_path())
            };
            let Ok(mut cloned) = current.reflect_clone() else {
                out.failed.push(label());
                continue;
            };
            let changed = rewrite_references(cloned.as_partial_reflect_mut(), ev);
            let Ok(mut entity_mut) = world.get_entity_mut(entity) else {
                continue;
            };
            reflect_component.apply(&mut entity_mut, cloned.as_partial_reflect());
            out.values += changed;
        }
    }
    out
}

/// Whether any ancestor of `e` is marked [`HideInHierarchy`]. The marker goes
/// on the root of an editor subtree, and its children do not carry it.
fn has_hidden_ancestor(world: &World, mut e: Entity) -> bool {
    while let Some(parent) = world.get::<ChildOf>(e).map(|c| c.parent()) {
        if world.get::<HideInHierarchy>(parent).is_some() {
            return true;
        }
        e = parent;
    }
    false
}

/// The moved path's replacement, if `value` is a string or path that refers
/// to it. A `PathBuf` is compared with forward slashes, the form every
/// project-relative path is stored in.
fn rewritten_leaf(value: &dyn PartialReflect, ev: &AssetPathChanged) -> Option<String> {
    if let Some(s) = value.try_downcast_ref::<String>() {
        return ev.rewrite(s);
    }
    if let Some(p) = value.try_downcast_ref::<PathBuf>() {
        return ev.rewrite(&p.to_string_lossy().replace('\\', "/"));
    }
    None
}

/// Whether a list is worth walking element by element. A list of numbers (a
/// heightmap, a mesh's weights) can run to millions of entries and can never
/// hold a path, so it is judged by its first element instead.
fn may_hold_paths(first: Option<&dyn PartialReflect>) -> bool {
    match first {
        None => false,
        Some(v) => match v.reflect_ref() {
            ReflectRef::Opaque(_) => {
                v.try_downcast_ref::<String>().is_some() || v.try_downcast_ref::<PathBuf>().is_some()
            }
            _ => true,
        },
    }
}

/// Whether anything inside `value` refers to the moved asset.
fn holds_reference(value: &dyn PartialReflect, ev: &AssetPathChanged) -> bool {
    match value.reflect_ref() {
        ReflectRef::Struct(s) => s.iter_fields().any(|(_, f)| holds_reference(f, ev)),
        ReflectRef::TupleStruct(t) => t.iter_fields().any(|f| holds_reference(f, ev)),
        ReflectRef::Tuple(t) => t.iter_fields().any(|f| holds_reference(f, ev)),
        ReflectRef::List(l) => may_hold_paths(l.get(0)) && l.iter().any(|v| holds_reference(v, ev)),
        ReflectRef::Array(a) => may_hold_paths(a.get(0)) && a.iter().any(|v| holds_reference(v, ev)),
        ReflectRef::Map(m) => m.iter().any(|(_, v)| holds_reference(v, ev)),
        ReflectRef::Enum(e) => e.iter_fields().any(|f| holds_reference(f.value(), ev)),
        ReflectRef::Opaque(_) => rewritten_leaf(value, ev).is_some(),
        // A set's members cannot be changed in place, and nothing stores an
        // asset path in one.
        _ => false,
    }
}

/// Rewrite every reference to the moved asset inside `value`, returning how
/// many there were. Walks exactly what [`holds_reference`] reads.
fn rewrite_references(value: &mut dyn PartialReflect, ev: &AssetPathChanged) -> usize {
    if let Some(new) = rewritten_leaf(value, ev) {
        if let Some(s) = value.try_downcast_mut::<String>() {
            *s = new;
        } else if let Some(p) = value.try_downcast_mut::<PathBuf>() {
            *p = PathBuf::from(new);
        }
        return 1;
    }
    match value.reflect_mut() {
        ReflectMut::Struct(s) => (0..s.field_len())
            .filter_map(|i| s.field_at_mut(i).map(|f| rewrite_references(f, ev)))
            .sum(),
        ReflectMut::TupleStruct(t) => (0..t.field_len())
            .filter_map(|i| t.field_mut(i).map(|f| rewrite_references(f, ev)))
            .sum(),
        ReflectMut::Tuple(t) => (0..t.field_len())
            .filter_map(|i| t.field_mut(i).map(|f| rewrite_references(f, ev)))
            .sum(),
        ReflectMut::List(l) => {
            if !may_hold_paths(l.get(0)) {
                return 0;
            }
            (0..l.len())
                .filter_map(|i| l.get_mut(i).map(|v| rewrite_references(v, ev)))
                .sum()
        }
        ReflectMut::Array(a) => {
            if !may_hold_paths(a.get(0)) {
                return 0;
            }
            (0..a.len())
                .filter_map(|i| a.get_mut(i).map(|v| rewrite_references(v, ev)))
                .sum()
        }
        ReflectMut::Map(m) => {
            let keys: Vec<Box<dyn PartialReflect>> =
                m.iter().filter_map(|(k, _)| k.to_dynamic().ok()).collect();
            keys.iter()
                .filter_map(|k| m.get_mut(k.as_ref()).map(|v| rewrite_references(v, ev)))
                .sum()
        }
        ReflectMut::Enum(e) => (0..e.field_len())
            .filter_map(|i| e.field_at_mut(i).map(|f| rewrite_references(f, ev)))
            .sum(),
        _ => 0,
    }
}

/// Every file under `root` in one of [`REFERENCING_EXTENSIONS`] or
/// `script_exts`, skipping the directories the project watcher ignores (which
/// is also what keeps a script's build output out of it).
fn referencing_files(root: &Path, script_exts: &[String]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if is_ignored(&relative) {
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                if depth + 1 < MAX_DEPTH {
                    stack.push((path, depth + 1));
                }
            } else if file_type.is_file()
                && (has_referencing_extension(&path) || has_extension(&path, script_exts))
            {
                out.push(path);
            }
        }
    }
    out
}

fn has_referencing_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| REFERENCING_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Whether `path` has one of `exts`, which are lowercase.
fn has_extension(path: &Path, exts: &[String]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| exts.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

#[cfg(test)]
mod open_scene_tests {
    use super::*;

    #[derive(Reflect, Clone, Default)]
    struct Slot {
        path: String,
    }

    /// Every shape a component stores a path in.
    #[derive(Component, Reflect, Clone, Default)]
    #[reflect(Component)]
    struct Holder {
        texture: String,
        script: Option<String>,
        file: PathBuf,
        clips: Vec<String>,
        nested: Slot,
        heights: Vec<f32>,
        /// Starts with the moved path but is a different file.
        lookalike: String,
    }

    #[derive(Component, Reflect, Clone, Default)]
    #[reflect(Component)]
    struct Untouched {
        path: String,
    }

    fn world() -> World {
        let mut world = World::new();
        let registry = AppTypeRegistry::default();
        {
            let mut r = registry.write();
            r.register::<Holder>();
            r.register::<Untouched>();
        }
        world.insert_resource(registry);
        world
    }

    fn holder(old: &str) -> Holder {
        Holder {
            texture: old.into(),
            script: Some(old.into()),
            file: PathBuf::from(old),
            clips: vec!["other.anim".into(), old.into()],
            nested: Slot { path: old.into() },
            heights: vec![0.0; 1000],
            lookalike: format!("{old}_large.png"),
        }
    }

    fn moved(old: &str, new: &str, is_dir: bool) -> AssetPathChanged {
        AssetPathChanged { old: old.into(), new: new.into(), is_dir }
    }

    /// The bug: a renamed texture survived only as far as the next save,
    /// because nothing patched it in memory.
    #[test]
    fn every_kind_of_field_follows_a_rename() {
        let mut world = world();
        let e = world.spawn((Name::new("rock"), holder("textures/rock.png"))).id();

        let live = rewrite_open_scene(&mut world, &moved("textures/rock.png", "textures/stone.png", false));

        let h = world.get::<Holder>(e).unwrap();
        assert_eq!(h.texture, "textures/stone.png");
        assert_eq!(h.script.as_deref(), Some("textures/stone.png"));
        assert_eq!(h.file, PathBuf::from("textures/stone.png"));
        assert_eq!(h.clips, ["other.anim", "textures/stone.png"]);
        assert_eq!(h.nested.path, "textures/stone.png");
        assert_eq!(h.lookalike, "textures/rock.png_large.png", "a different file must not be touched");
        assert_eq!(live.values, 5);
        assert!(live.failed.is_empty());
    }

    #[test]
    fn a_folder_move_rewrites_what_is_inside_it() {
        let mut world = world();
        let e = world
            .spawn((Name::new("a"), Untouched { path: "props/rock/mesh.glb".into() }))
            .id();
        let sibling = world
            .spawn((Name::new("b"), Untouched { path: "props/rock_large/mesh.glb".into() }))
            .id();

        rewrite_open_scene(&mut world, &moved("props/rock", "env/rock", true));

        assert_eq!(world.get::<Untouched>(e).unwrap().path, "env/rock/mesh.glb");
        assert_eq!(
            world.get::<Untouched>(sibling).unwrap().path,
            "props/rock_large/mesh.glb",
            "a folder whose name merely starts the same is a different folder"
        );
    }

    /// The editor's own entities keep their paths: a preview's model is not a
    /// reference the user made.
    #[test]
    fn editor_entities_are_left_alone() {
        let mut world = world();
        let root = world.spawn((Name::new("preview"), HideInHierarchy)).id();
        let child = world
            .spawn((Name::new("preview_model"), Untouched { path: "a.glb".into() }, ChildOf(root)))
            .id();
        let unnamed = world.spawn(Untouched { path: "a.glb".into() }).id();

        let live = rewrite_open_scene(&mut world, &moved("a.glb", "b.glb", false));

        assert_eq!(live.values, 0);
        assert_eq!(world.get::<Untouched>(child).unwrap().path, "a.glb");
        assert_eq!(world.get::<Untouched>(unnamed).unwrap().path, "a.glb");
    }

    /// Writing back marks a component changed, which reloads it. Only the ones
    /// that held the path should pay for that.
    #[test]
    fn components_without_the_path_are_not_marked_changed() {
        let mut world = world();
        let e = world
            .spawn((Name::new("rock"), holder("a.png"), Untouched { path: "b.png".into() }))
            .id();
        world.clear_trackers();

        rewrite_open_scene(&mut world, &moved("a.png", "c.png", false));

        let entity = world.entity(e);
        assert!(entity.get_ref::<Holder>().unwrap().is_changed());
        assert!(!entity.get_ref::<Untouched>().unwrap().is_changed());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walk_finds_nested_materials_and_skips_ignored_dirs() {
        let root = std::env::temp_dir().join(format!("renzora_refs_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("models/bistro/materials")).unwrap();
        std::fs::create_dir_all(root.join(".renzora/cache")).unwrap();
        std::fs::write(root.join("models/bistro/materials/a.material"), "{}").unwrap();
        std::fs::write(root.join("models/bistro/bistro.glb"), "").unwrap();
        std::fs::write(root.join(".renzora/cache/b.material"), "{}").unwrap();
        std::fs::write(root.join("main.BSN"), "").unwrap();
        std::fs::create_dir_all(root.join("scripts")).unwrap();
        std::fs::create_dir_all(root.join(".renzora/scripts/spin")).unwrap();
        std::fs::write(root.join("scripts/menu.lua"), "").unwrap();
        std::fs::write(root.join("scripts/sparks.vfx"), "").unwrap();
        std::fs::write(root.join("scripts/notes.txt"), "").unwrap();
        // A script's build output, which must never be rewritten.
        std::fs::write(root.join(".renzora/scripts/spin/spin.rs"), "").unwrap();

        let scripts = vec!["lua".to_string(), "rs".to_string()];
        let mut found: Vec<String> = referencing_files(&root, &scripts)
            .iter()
            .map(|p| p.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/"))
            .collect();
        found.sort();
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(
            found,
            [
                "main.BSN",
                "models/bistro/materials/a.material",
                "scripts/menu.lua",
                "scripts/sparks.vfx"
            ]
        );
    }

    /// The rule a script gets: the literal path changes, a path in a bare
    /// comment and one built from pieces do not.
    #[test]
    fn a_script_keeps_everything_but_the_literal_path() {
        let ev = AssetPathChanged {
            old: "audio/menu_button.mp3".into(),
            new: "audio/ui/menu_button.mp3".into(),
            is_dir: false,
        };
        let script = r#"-- TODO: replace audio/menu_button.mp3 with the new click
function on_ready()
    play_sound("audio/menu_button.mp3", 0.5)
    play_sound("audio/" .. name .. ".mp3", 0.5)
end
"#;
        let updated = ev.rewrite_text(script).unwrap();
        assert!(updated.contains(r#"play_sound("audio/ui/menu_button.mp3", 0.5)"#));
        assert!(updated.contains("-- TODO: replace audio/menu_button.mp3"));
        assert!(updated.contains(r#"play_sound("audio/" .. name .. ".mp3", 0.5)"#));
    }
}
