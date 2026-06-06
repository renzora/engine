//! Script attachment helper shared by the bevy-native Scripts panel
//! (`native_scripts`): create a fresh `.lua` under `scripts/`, attach it to the
//! entity's `ScriptComponent`, and open it in the code editor.

use bevy::prelude::*;
use renzora_scripting::ScriptComponent;

use crate::state::CodeEditorState;

const NEW_SCRIPT_TEMPLATE: &str = r#"-- New Script

function on_ready(ctx, vars)
    -- Called once when the script is first attached
end

function on_update(ctx, vars)
    -- Called every frame
end
"#;

/// Create a new `.lua` file under `<project>/scripts/` with a unique name,
/// attach it to `entity` via its `ScriptComponent` (creating one if absent),
/// and open it in the code editor.
pub(crate) fn create_and_attach_new_script(
    world: &mut World,
    entity: Entity,
    project_root: std::path::PathBuf,
) {
    let scripts_dir = project_root.join("scripts");
    if let Err(e) = std::fs::create_dir_all(&scripts_dir) {
        log::error!("Failed to create scripts dir: {}", e);
        return;
    }

    // Pick a unique name.
    let mut idx = 1usize;
    let (abs_path, rel_path) = loop {
        let name = if idx == 1 {
            "new_script.lua".to_string()
        } else {
            format!("new_script_{}.lua", idx)
        };
        let abs = scripts_dir.join(&name);
        let rel = std::path::PathBuf::from("scripts").join(&name);
        if !abs.exists() {
            break (abs, rel);
        }
        idx += 1;
        if idx > 1000 {
            log::error!("Couldn't find a free new_script name");
            return;
        }
    };

    if let Err(e) = std::fs::write(&abs_path, NEW_SCRIPT_TEMPLATE) {
        log::error!("Failed to write new script {}: {}", abs_path.display(), e);
        return;
    }

    // Attach (create ScriptComponent if needed).
    if let Some(mut sc) = world.get_mut::<ScriptComponent>(entity) {
        sc.add_file_script(rel_path);
    } else {
        let mut sc = ScriptComponent::new();
        sc.add_file_script(rel_path);
        world.entity_mut(entity).insert(sc);
    }

    // Open in editor.
    if let Some(mut state) = world.get_resource_mut::<CodeEditorState>() {
        state.open_file(abs_path);
    }
}
