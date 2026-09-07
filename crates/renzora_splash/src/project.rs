pub use renzora::{open_project, CurrentProject, ProjectConfig, WindowConfig};

use std::path::Path;

/// Create a new project at the specified path
#[cfg(not(target_arch = "wasm32"))]
pub fn create_project(
    path: &Path,
    name: &str,
) -> Result<CurrentProject, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(path)?;
    std::fs::create_dir_all(path.join("scenes"))?;
    std::fs::create_dir_all(path.join("plugins"))?;

    let config = ProjectConfig {
        name: name.to_string(),
        version: "0.1.0".to_string(),
        // Stamped at creation and never touched again, so a project made three
        // versions ago still says so after today's editor has saved it.
        created_with: Some(renzora::version::ENGINE_VERSION.to_string()),
        main_scene: "scenes/main.bsn".to_string(),
        ..Default::default()
    };

    let config_path = path.join("project.toml");
    let config_content = toml::to_string_pretty(&config)?;
    std::fs::write(&config_path, config_content)?;

    // An empty scene in the interim BSN format (a header comment + no `entity`
    // blocks). The old `(resources:{},entities:{})` was 0.18 RON, which the BSN
    // loader rejects — see `renzora_bsn`.
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
            Some(renzora::version::ENGINE_VERSION)
        );

        let written = std::fs::read_to_string(dir.join("project.toml")).expect("read back");
        let parsed: ProjectConfig = toml::from_str(&written).expect("parse");
        assert_eq!(
            parsed.created_with.as_deref(),
            Some(renzora::version::ENGINE_VERSION)
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
