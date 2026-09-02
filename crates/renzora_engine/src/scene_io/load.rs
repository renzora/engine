//! Reading a scene in.
//!
//! Loads are *lossy by design*: a component whose type is not registered (a
//! plugin that is not present in this build) is skipped rather than aborting
//! the whole scene, and the skipped type paths are reported so the editor can
//! surface a toast. The load also declines to re-enter a scene already being
//! loaded further up the stack — see [`LOADING_STACK`].

use bevy::prelude::*;
use renzora::console_log::*;
use renzora::{DefaultCamera, SceneCamera};
use renzora_bsn::bsn::{BsnSerializer, SceneSerializer};
use renzora_bsn::DynamicScene;
use std::path::Path;

use super::events::{SceneLoadFailed, SceneLoadPhase, SceneLoadState, SceneLoaded, SceneLoadedWithSkippedTypes};
use super::instances::expand_scene_instances;
use super::prune::{prune_leaked_ui, prune_orphaned_entities};

/// Try to deserialize a scene RON, transparently skipping any
/// component/resource entries whose type isn't registered.
///
/// Bevy's `SceneDeserializer` aborts on the first unknown type, so
/// we loop: parse the offending type out of the error message, strip
/// that entry from the RON, retry. Each pass either makes progress
/// (one type stripped) or returns an error we can't massage away.
///
/// Returns the parsed scene plus the list of skipped type paths so the
/// caller can surface a warning.
pub(crate) fn deserialize_scene_lossy(
    world: &World,
    text: &str,
) -> Result<(DynamicScene, Vec<String>), String> {
    let type_registry = world.resource::<AppTypeRegistry>().clone();
    let registry = type_registry.read();
    // The interim BSN parser skips unregistered / un-deserializable components
    // itself (returning their type paths), so the old RON strip-and-retry loop
    // is unnecessary — a scene authored with a now-absent plugin still loads.
    BsnSerializer
        .deserialize_lossy(text, &registry)
        .map_err(|e| e.to_string())
}

/// Pull the offending type path out of a Bevy/serde error message of the
/// form "no registration found for `some::type::path`". Returns `None`
/// if the error isn't of that shape — caller should surface the original
/// error verbatim in that case.
///
/// Obsolete: the interim BSN parser skips unregistered components itself (see
/// [`deserialize_scene_lossy`]). Retained with its tests pending removal.
#[allow(dead_code)]
pub(crate) fn extract_unregistered_type(error_message: &str) -> Option<String> {
    let needle = "no registration found for ";
    let pos = error_message.find(needle)?;
    let rest = &error_message[pos + needle.len()..];
    // Tolerate both `... for \`T\`` and `... for type \`T\``.
    let rest = rest.strip_prefix("type ").unwrap_or(rest);
    let rest = rest.strip_prefix('`')?;
    let close = rest.find('`')?;
    Some(rest[..close].to_string())
}

/// Remove the entry `"<type_path>": ( ... )` (and its trailing comma /
/// own line) from a RON scene string, walking balanced parens to find
/// the closing `)` while respecting string literals. Returns `None` if
/// the key isn't present or paren-matching fails.
///
/// Removing the whole line keeps the surrounding map well-formed
/// regardless of whether the entry is first, last, or middle: leftover
/// commas are RON-tolerated (trailing commas allowed), and we never
/// leave back-to-back commas because we consume one trailing comma when
/// it's there.
///
/// Obsolete under the interim BSN format (no RON text surgery). Retained with
/// its tests pending removal.
#[allow(dead_code)]
pub(crate) fn strip_component_entry(ron: &str, type_path: &str) -> Option<String> {
    let key = format!("\"{}\"", type_path);
    let key_pos = ron.find(&key)?;
    let key_end = key_pos + key.len();
    let bytes = ron.as_bytes();

    // Find the opening `(` after the key (skipping the `:` and whitespace).
    let mut i = key_end;
    while i < bytes.len() && bytes[i] != b'(' {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }
    let open_pos = i;

    // Walk balanced parens from after the open. String literals don't
    // count toward depth — track escapes so an escaped quote inside a
    // string doesn't terminate it prematurely.
    let mut depth: i32 = 1;
    let mut in_string = false;
    let mut prev_escape = false;
    let mut close_pos: Option<usize> = None;
    for (j, &c) in bytes.iter().enumerate().skip(open_pos + 1) {
        if in_string {
            if c == b'"' && !prev_escape {
                in_string = false;
            }
            prev_escape = c == b'\\' && !prev_escape;
            continue;
        }
        prev_escape = false;
        match c {
            b'"' => in_string = true,
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    close_pos = Some(j);
                    break;
                }
            }
            _ => {}
        }
    }
    let close_pos = close_pos?;

    // Extend forward to consume the trailing comma + the rest of the
    // line (including the line break). Keeps the surrounding indentation
    // pristine.
    let mut end = close_pos + 1;
    while end < bytes.len() && (bytes[end] == b' ' || bytes[end] == b'\t') {
        end += 1;
    }
    if end < bytes.len() && bytes[end] == b',' {
        end += 1;
    }
    while end < bytes.len() && (bytes[end] == b' ' || bytes[end] == b'\t') {
        end += 1;
    }
    if end < bytes.len() && bytes[end] == b'\n' {
        end += 1;
    }

    // Extend backward to the start of the key's own line so we don't
    // leave a blank, indented line behind.
    let mut start = key_pos;
    while start > 0 {
        let c = bytes[start - 1];
        if c == b'\n' {
            break;
        }
        if !c.is_ascii_whitespace() {
            break;
        }
        start -= 1;
    }

    let mut out = String::with_capacity(ron.len());
    out.push_str(&ron[..start]);
    out.push_str(&ron[end..]);
    Some(out)
}

/// Load a scene from a RON string into the world (same logic as [`load_scene`] but from string).
pub fn load_scene_from_string(world: &mut World, ron: &str) {
    let trimmed = ron.trim();
    if trimmed.is_empty() || trimmed == "(entities: {}, resources: {})" {
        return;
    }

    let (mut scene, skipped_types) = match deserialize_scene_lossy(world, ron) {
        Ok(pair) => pair,
        Err(e) => {
            error!("Failed to deserialize scene from string: {}", e);
            return;
        }
    };
    if !skipped_types.is_empty() {
        for type_path in &skipped_types {
            warn!(
                "[scene] string scene skipped `{}` — type is unregistered, or \
                 its value did not match the type's reflection encoding",
                type_path
            );
        }
        // No path to report for string scenes — pass an empty marker.
        world.trigger(SceneLoadedWithSkippedTypes {
            path: String::new(),
            skipped: skipped_types.clone(),
        });
    }

    let pruned = prune_orphaned_entities(&mut scene);
    if pruned > 0 {
        warn!(
            "[scene] pruned {} orphaned entities (leaked editor-chrome / missing parent) from string scene",
            pruned
        );
    }
    let ui_pruned = prune_leaked_ui(&mut scene);
    if ui_pruned > 0 {
        warn!(
            "[scene] pruned {} leaked editor-UI entities (no UiCanvas ancestor) from string scene",
            ui_pruned
        );
    }

    crate::plugin_scene_bridge::refresh_raw_component_registry(world);
    let mut entity_map = bevy::ecs::entity::EntityHashMap::default();
    // Globals, not per-entity data — so this is a whole-scene load only. The
    // undo-restore path deliberately does not call it: restoring a deleted
    // subtree must not reach out and reset the world's plugin settings.
    scene.write_raw_resources(world);
    match scene.write_to_world(world, &mut entity_map) {
        Ok(()) => {
            // Re-insert ChildOf to trigger hierarchy hooks
            let children_with_parents: Vec<(Entity, Entity)> = entity_map
                .values()
                .filter_map(|&entity| {
                    world
                        .get_entity(entity)
                        .ok()?
                        .get::<ChildOf>()
                        .map(|c| (entity, c.parent()))
                })
                .collect();

            for (child, parent) in children_with_parents {
                world.entity_mut(child).remove::<ChildOf>();
                world.entity_mut(child).insert(ChildOf(parent));
            }
        }
        Err(e) => {
            error!("Failed to write scene from string to world: {}", e);
        }
    }
}

// Scenes currently being loaded on this thread, outermost first.
//
// Recorded by `load_scene` rather than by the instance expander, which is the
// whole point: the expander only ever knew about *nested* loads, so a scene
// holding a `SceneInstance` of **itself** was never on the stack when its own
// instance was checked. It expanded once, and the inner pass — by then on the
// stack — was correctly skipped. Not an infinite loop, so nothing crashed: the
// scene simply loaded twice, every entity in it duplicated, and the second
// camera made the clouds plugin flip between two sources every frame.
thread_local! {
    static LOADING_STACK: std::cell::RefCell<Vec<std::path::PathBuf>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Whether `path` is already being loaded further up the stack.
pub(crate) fn loading_stack_contains(path: &Path) -> bool {
    let abs = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    LOADING_STACK.with(|s| s.borrow().iter().any(|p| p == &abs))
}

/// Pops the loading stack however [`load_scene`] returns — it has several early
/// exits, and a stack that leaks an entry makes every later load of that scene
/// look like a cycle.
struct LoadingStackGuard;

impl Drop for LoadingStackGuard {
    fn drop(&mut self) {
        LOADING_STACK.with(|s| {
            s.borrow_mut().pop();
        });
    }
}

/// Load a scene from a RON file into the world.
///
/// Tries the Vfs (rpak archive) first, then falls back to disk.
///
/// Declines to load a scene that is already being loaded further up the stack,
/// which is what stops a scene instancing itself from loading twice.
pub fn load_scene(world: &mut World, path: &Path) {
    if loading_stack_contains(path) {
        warn!(
            "[scene] {} instances itself (directly or through a cycle) — skipping the inner load",
            path.display()
        );
        return;
    }
    LOADING_STACK.with(|s| {
        s.borrow_mut()
            .push(path.canonicalize().unwrap_or_else(|_| path.to_path_buf()))
    });
    let _stack_guard = LoadingStackGuard;

    console_info(
        "Scene",
        format!("=== Loading scene from {} ===", path.display()),
    );

    let path_str = path.to_string_lossy().to_string();
    if let Some(mut state) = world.get_resource_mut::<SceneLoadState>() {
        state.phase = SceneLoadPhase::Loading;
        state.current_path = Some(path_str.clone());
        state.progress = 0.0;
    }

    // Try reading from Vfs (rpak archive) first.
    let content = if let Some(vfs) = world.get_resource::<crate::Vfs>() {
        // Normalize to forward-slash archive-relative path, stripping leading "./" or ".\"
        let path_str = path.to_string_lossy().replace('\\', "/");
        let archive_key = path_str.strip_prefix("./").unwrap_or(&path_str);
        if let Some(s) = vfs.read_string(archive_key) {
            console_info(
                "Scene",
                format!("Read {} bytes from rpak: {}", s.len(), archive_key),
            );
            Some(s)
        } else {
            None
        }
    } else {
        None
    };

    // Fall back to disk if Vfs didn't have it.
    // Web: the project is behind a browser directory handle, so `path.exists()`
    // and `read_to_string` below both answer "no" regardless of what is there.
    // Scene files are pre-read when the project is adopted (`webfs::prewarm`),
    // precisely because this load is a one-shot `OnEnter` system with no second
    // attempt — a cache miss here would mean the scene never loads at all.
    #[cfg(target_arch = "wasm32")]
    let content = content.or_else(|| renzora_webfs::read_text_cached(path));

    let content = match content {
        Some(c) => c,
        None => {
            if !path.exists() {
                console_warn(
                    "Scene",
                    format!("Scene file does not exist: {}", path.display()),
                );
                info!("Scene file does not exist yet: {}", path.display());
                return;
            }
            match std::fs::read_to_string(path) {
                Ok(c) => {
                    console_info(
                        "Scene",
                        format!("Read {} bytes from {}", c.len(), path.display()),
                    );
                    c
                }
                Err(e) => {
                    console_error(
                        "Scene",
                        format!("Failed to read scene file {}: {}", path.display(), e),
                    );
                    error!("Failed to read scene file {}: {}", path.display(), e);
                    return;
                }
            }
        }
    };

    let trimmed = content.trim();
    if trimmed.is_empty() || trimmed == "(entities: {}, resources: {})" {
        console_info("Scene", format!("Scene is empty: {}", path.display()));
        info!("Scene is empty: {}", path.display());
        if let Some(mut state) = world.get_resource_mut::<SceneLoadState>() {
            state.phase = SceneLoadPhase::Ready;
            state.progress = 1.0;
        }
        world.trigger(SceneLoaded {
            path: path_str.clone(),
        });
        return;
    }

    let (mut scene, skipped_types) = match deserialize_scene_lossy(world, &content) {
        Ok(pair) => pair,
        Err(e) => {
            error!("Failed to deserialize scene {}: {}", path.display(), e);
            return;
        }
    };
    if !skipped_types.is_empty() {
        for type_path in &skipped_types {
            // "skipped" covers two causes, and saying only the first sends
            // anyone hand-editing a scene hunting for a registration that is
            // already there: `deserialize_lossy` drops a component whose type
            // is unregistered *or* whose value failed to decode. A wrong RON
            // encoding — `Name: ("x")` for a newtype, `(x:..,y:..)` for a Vec3 —
            // lands here looking exactly like a missing type.
            warn!(
                "[scene] {} skipped `{}` — type is unregistered, or its value \
                 did not match the type's reflection encoding",
                path.display(),
                type_path
            );
        }
        world.trigger(SceneLoadedWithSkippedTypes {
            path: path_str.clone(),
            skipped: skipped_types.clone(),
        });
    }

    let pruned = prune_orphaned_entities(&mut scene);
    if pruned > 0 {
        console_info(
            "Scene",
            format!("Pruned {pruned} orphaned editor-chrome entities on load"),
        );
        warn!(
            "[scene] {} pruned {} orphaned entities (leaked editor-chrome / missing parent)",
            path.display(),
            pruned
        );
    }
    let ui_pruned = prune_leaked_ui(&mut scene);
    if ui_pruned > 0 {
        console_info(
            "Scene",
            format!("Pruned {ui_pruned} leaked editor-UI entities (no UiCanvas ancestor) on load"),
        );
        warn!(
            "[scene] {} pruned {} leaked editor-UI entities (no UiCanvas ancestor)",
            path.display(),
            ui_pruned
        );
    }

    crate::plugin_scene_bridge::refresh_raw_component_registry(world);
    let mut entity_map = bevy::ecs::entity::EntityHashMap::default();
    scene.write_raw_resources(world);
    match scene.write_to_world(world, &mut entity_map) {
        Ok(()) => {
            console_info(
                "Scene",
                format!(
                    "Scene written to world: {} entities mapped from {}",
                    entity_map.len(),
                    path.display()
                ),
            );

            // Log each mapped entity
            for (&scene_entity, &world_entity) in &entity_map {
                let name = world
                    .get::<Name>(world_entity)
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "unnamed".into());
                let has_scene_cam = world.get::<SceneCamera>(world_entity).is_some();
                let has_default = world.get::<DefaultCamera>(world_entity).is_some();
                let mut tags = Vec::new();
                if has_scene_cam {
                    tags.push("SceneCamera");
                }
                if has_default {
                    tags.push("DefaultCamera");
                }
                let tag_str = if tags.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", tags.join(", "))
                };
                console_info(
                    "Scene",
                    format!(
                        "  scene:{:?} -> world:{:?} \"{}\"{}",
                        scene_entity, world_entity, name, tag_str
                    ),
                );
            }

            info!(
                "Loaded scene from {} ({} entities mapped)",
                path.display(),
                entity_map.len()
            );

            // Bevy's write_to_world inserts ChildOf via reflection, which may not
            // trigger the on_insert hooks that maintain the parent's Children component.
            // Re-insert ChildOf on each child to force the hooks to fire.
            let children_with_parents: Vec<(Entity, Entity)> = entity_map
                .values()
                .filter_map(|&entity| {
                    world
                        .get_entity(entity)
                        .ok()?
                        .get::<ChildOf>()
                        .map(|c| (entity, c.parent()))
                })
                .collect();

            console_info(
                "Scene",
                format!(
                    "Re-inserting ChildOf on {} entities to trigger hierarchy hooks",
                    children_with_parents.len()
                ),
            );

            for (child, parent) in children_with_parents {
                // Remove and re-insert ChildOf to trigger hooks
                world.entity_mut(child).remove::<ChildOf>();
                world.entity_mut(child).insert(ChildOf(parent));
            }

            // Expand nested scene instances referenced from the host scene.
            expand_scene_instances(world);

            console_success(
                "Scene",
                format!("=== Scene load complete: {} ===", path.display()),
            );

            if let Some(mut state) = world.get_resource_mut::<SceneLoadState>() {
                state.phase = SceneLoadPhase::Ready;
                state.progress = 1.0;
            }
            world.trigger(SceneLoaded {
                path: path_str.clone(),
            });
        }
        Err(e) => {
            console_error(
                "Scene",
                format!("Failed to write scene to world {}: {}", path.display(), e),
            );
            error!("Failed to write scene to world {}: {}", path.display(), e);

            if let Some(mut state) = world.get_resource_mut::<SceneLoadState>() {
                state.phase = SceneLoadPhase::Failed;
            }
            let err_str = e.to_string();
            world.trigger(SceneLoadFailed {
                path: path_str.clone(),
                error: err_str,
            });
        }
    }
}

/// Load the current project's main scene.
pub fn load_current_scene(world: &mut World) {
    let Some(project) = world.get_resource::<renzora::CurrentProject>() else {
        warn!("load_current_scene: no CurrentProject resource");
        return;
    };
    let path = project.main_scene_path();
    info!("load_current_scene: loading from {}", path.display());
    load_scene(world, &path);
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // extract_unregistered_type
    // ------------------------------------------------------------------

    #[test]
    fn extract_type_basic_backtick_form() {
        let msg = "no registration found for `my::crate::Foo`";
        assert_eq!(
            extract_unregistered_type(msg),
            Some("my::crate::Foo".to_string())
        );
    }

    #[test]
    fn extract_type_with_type_keyword() {
        // Tolerate the `... for type \`T\`` variant.
        let msg = "no registration found for type `bevy::pbr::StandardMaterial`";
        assert_eq!(
            extract_unregistered_type(msg),
            Some("bevy::pbr::StandardMaterial".to_string())
        );
    }

    #[test]
    fn extract_type_embedded_in_larger_message() {
        let msg = "deserialization error at line 5: no registration found for `a::B`, aborting";
        assert_eq!(extract_unregistered_type(msg), Some("a::B".to_string()));
    }

    #[test]
    fn extract_type_returns_none_when_pattern_absent() {
        assert_eq!(extract_unregistered_type("some unrelated error"), None);
    }

    #[test]
    fn extract_type_returns_none_without_closing_backtick() {
        let msg = "no registration found for `unterminated";
        assert_eq!(extract_unregistered_type(msg), None);
    }

    #[test]
    fn extract_type_returns_none_without_opening_backtick() {
        let msg = "no registration found for plainname";
        assert_eq!(extract_unregistered_type(msg), None);
    }

    // ------------------------------------------------------------------
    // strip_component_entry
    // ------------------------------------------------------------------

    #[test]
    fn strip_middle_entry_keeps_neighbors() {
        let ron = "(\n  \"a::A\": (x: 1),\n  \"b::B\": (y: 2),\n  \"c::C\": (z: 3),\n)";
        let out = strip_component_entry(ron, "b::B").expect("entry should be found");
        assert!(!out.contains("b::B"), "stripped key must be gone");
        assert!(out.contains("a::A"), "preceding entry must remain");
        assert!(out.contains("c::C"), "following entry must remain");
        // No back-to-back commas left behind.
        assert!(!out.contains(",,"));
    }

    #[test]
    fn strip_entry_with_nested_parens() {
        // The closing paren of the target must be found via balanced-paren
        // walking, not the first ')' encountered.
        let ron = "(\n  \"t::T\": (inner: (a: 1, b: (2)), tail: 9),\n  \"u::U\": (k: 0),\n)";
        let out = strip_component_entry(ron, "t::T").expect("entry should be found");
        assert!(!out.contains("t::T"));
        assert!(out.contains("u::U"));
        // The inner data of u::U must survive intact.
        assert!(out.contains("k: 0"));
    }

    #[test]
    fn strip_entry_with_paren_inside_string_literal() {
        // A ')' inside a quoted string must not be treated as the closing
        // paren of the entry.
        let ron = "(\n  \"s::S\": (label: \"a ) b ( c\"),\n  \"v::V\": (n: 1),\n)";
        let out = strip_component_entry(ron, "s::S").expect("entry should be found");
        assert!(!out.contains("s::S"));
        assert!(out.contains("v::V"));
        assert!(out.contains("n: 1"));
    }

    #[test]
    fn strip_entry_with_escaped_quote_in_string() {
        // An escaped quote must not prematurely end the string scan.
        let ron = "(\n  \"e::E\": (txt: \"x \\\" ) still in string\"),\n  \"w::W\": (m: 2),\n)";
        let out = strip_component_entry(ron, "e::E").expect("entry should be found");
        assert!(!out.contains("e::E"));
        assert!(out.contains("w::W"));
        assert!(out.contains("m: 2"));
    }

    #[test]
    fn strip_returns_none_for_missing_key() {
        let ron = "(\n  \"a::A\": (x: 1),\n)";
        assert_eq!(strip_component_entry(ron, "z::Z"), None);
    }

    #[test]
    fn strip_returns_none_on_unbalanced_parens() {
        // Opening paren but never closed -> paren matching fails -> None.
        let ron = "(\n  \"a::A\": (x: 1,\n";
        assert_eq!(strip_component_entry(ron, "a::A"), None);
    }

    #[test]
    fn strip_last_entry_is_removed() {
        let ron = "(\n  \"a::A\": (x: 1),\n  \"b::B\": (y: 2),\n)";
        let out = strip_component_entry(ron, "b::B").expect("entry should be found");
        assert!(!out.contains("b::B"));
        assert!(out.contains("a::A"));
    }
}
