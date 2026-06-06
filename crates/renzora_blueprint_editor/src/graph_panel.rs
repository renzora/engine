//! Blueprint file I/O + Lua-compile helpers shared by the native (ember)
//! blueprint graph view in [`crate::native_graph`].

use bevy::prelude::*;

use renzora::core::CurrentProject;
use renzora_blueprint::BlueprintGraph;

/// Load a `.blueprint` file (JSON-serialised `BlueprintGraph`). Resolves the
/// project-relative path via `CurrentProject` if available. Returns `None`
/// if the file is missing or unparseable — caller should fall back to an
/// empty graph.
pub(crate) fn load_blueprint_file(
    project: Option<&CurrentProject>,
    rel_path: &str,
) -> Option<BlueprintGraph> {
    let abs = project
        .map(|p| p.resolve_path(rel_path))
        .unwrap_or_else(|| std::path::PathBuf::from(rel_path));
    let json = std::fs::read_to_string(&abs).ok()?;
    // `.blueprint` files are JSON-serialised BlueprintGraph; new files
    // created from the asset browser start out as `{}` which deserialises
    // to a default graph.
    serde_json::from_str(&json).ok()
}

/// Persist `graph` back to the `.blueprint` file at `rel_path`. Errors are
/// logged but not propagated.
pub(crate) fn save_blueprint_file(
    project: Option<&CurrentProject>,
    rel_path: &str,
    graph: &BlueprintGraph,
) {
    let abs = project
        .map(|p| p.resolve_path(rel_path))
        .unwrap_or_else(|| std::path::PathBuf::from(rel_path));
    if let Some(parent) = abs.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_string_pretty(graph) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&abs, json) {
                warn!("[blueprint_editor] save failed for {}: {}", rel_path, e);
            }
        }
        Err(e) => warn!(
            "[blueprint_editor] serialise failed for {}: {}",
            rel_path, e
        ),
    }
}

/// Compile a blueprint graph to Lua, save it to the project scripts folder,
/// and attach a ScriptComponent pointing to the generated file.
pub(crate) fn apply_blueprint_to_lua(
    world: &mut World,
    entity: Entity,
    graph: &BlueprintGraph,
    project_path: &std::path::Path,
    entity_name: &str,
) {
    use renzora_scripting::ScriptComponent;

    // Compile
    let lua_source = renzora_blueprint::compiler::compile_to_lua(graph);

    // Sanitize entity name for filename
    let safe_name: String = entity_name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let file_name = format!("bp_{}.lua", safe_name.to_lowercase());

    // Ensure scripts directory exists
    let scripts_dir = project_path.join("scripts");
    if let Err(e) = std::fs::create_dir_all(&scripts_dir) {
        warn!("Failed to create scripts dir: {}", e);
        return;
    }

    // Write the file
    let file_path = scripts_dir.join(&file_name);
    if let Err(e) = std::fs::write(&file_path, &lua_source) {
        warn!("Failed to write compiled blueprint: {}", e);
        return;
    }

    info!(
        "Blueprint compiled to Lua: {} ({} bytes)",
        file_path.display(),
        lua_source.len()
    );

    // Attach or update ScriptComponent
    let script_rel_path = std::path::PathBuf::from(format!("scripts/{}", file_name));
    if let Some(mut sc) = world.get_mut::<ScriptComponent>(entity) {
        // Check if this blueprint script is already attached
        let existing = sc.scripts.iter().position(|e| {
            e.script_path
                .as_ref()
                .map(|p| p.ends_with(&file_name))
                .unwrap_or(false)
        });
        if let Some(idx) = existing {
            // Update path (forces reload) and reset runtime state
            sc.scripts[idx].script_path = Some(script_rel_path);
            sc.scripts[idx].runtime_state = Default::default();
        } else {
            sc.add_file_script(script_rel_path);
        }
    } else {
        world
            .entity_mut(entity)
            .insert(ScriptComponent::from_file(script_rel_path));
    }
}
