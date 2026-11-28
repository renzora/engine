//! Making a new project on disk: the folder, `project.toml` and an empty main
//! scene.
//!
//! In the contract rather than in the launcher because more than the launcher
//! makes projects: the MCP plugin creates one from a tool call, with no folder
//! picker for an agent to get stuck behind. Both must write the same thing, or
//! a project made one way opens differently from one made the other. Here and
//! not in `renzora_project_api` because the stamp needs [`ENGINE_VERSION`],
//! which lives in this layer, above that one.
//!
//! [`ENGINE_VERSION`]: crate::version::ENGINE_VERSION

use renzora_project_api::{CurrentProject, ProjectConfig};
use std::path::Path;

/// Create a new project at `path`, named `name`, and return it ready to open.
///
/// The folder is created if missing. The caller decides whether an existing
/// folder is acceptable; this only writes the three things a project is.
#[cfg(not(target_arch = "wasm32"))]
pub fn create_project(
    path: &Path,
    name: &str,
) -> Result<CurrentProject, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(path)?;
    // `scenes/` only. A new project used to get an empty `plugins/` beside it,
    // from when a plugin was a thing a project carried. Plugins install beside
    // the *executable* now (see `renzora_marketplace`'s `engine_plugins_dir`),
    // so nothing has read that folder for some time, and an empty directory in
    // a new project is a question the Assets panel asks the user and cannot
    // answer.
    std::fs::create_dir_all(path.join("scenes"))?;

    let config = ProjectConfig {
        name: name.to_string(),
        version: "0.1.0".to_string(),
        // Stamped at creation and never touched again, so a project made three
        // versions ago still says so after today's editor has saved it.
        created_with: Some(crate::version::ENGINE_VERSION.to_string()),
        main_scene: "scenes/main.bsn".to_string(),
        ..Default::default()
    };

    let config_path = path.join("project.toml");
    let config_content = toml::to_string_pretty(&config)?;
    std::fs::write(&config_path, config_content)?;

    // An empty scene in the interim BSN format (a header comment and no
    // `entity` blocks). The old `(resources:{},entities:{})` was 0.18 RON,
    // which the BSN loader rejects; see `renzora_bsn`.
    let scene_content = "// renzora interim bsn v1\n";
    let scene_path = path.join("scenes").join("main.bsn");
    std::fs::write(&scene_path, scene_content)?;

    Ok(CurrentProject {
        path: path.to_path_buf(),
        config,
    })
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    /// The stamp has to survive to disk, not just to the returned config: the
    /// value's whole job is to still be there in a `project.toml` someone opens
    /// three versions from now.
    #[test]
    fn a_new_project_records_the_engine_version_that_made_it() {
        let dir = std::env::temp_dir().join(format!(
            "renzora-new-project-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let project = create_project(&dir, "Demo").expect("create");
        assert_eq!(
            project.config.created_with.as_deref(),
            Some(crate::version::ENGINE_VERSION)
        );

        let written = std::fs::read_to_string(dir.join("project.toml")).expect("read back");
        let parsed: ProjectConfig = toml::from_str(&written).expect("parse");
        assert_eq!(
            parsed.created_with.as_deref(),
            Some(crate::version::ENGINE_VERSION)
        );
        assert!(dir.join("scenes").join("main.bsn").is_file());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
