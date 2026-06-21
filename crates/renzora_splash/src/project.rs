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
