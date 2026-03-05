pub use renzora_core::{CurrentProject, ProjectConfig, WindowConfig, open_project};

use std::path::Path;

/// Create a new project at the specified path
pub fn create_project(path: &Path, name: &str) -> Result<CurrentProject, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(path)?;
    std::fs::create_dir_all(path.join("scenes"))?;
    std::fs::create_dir_all(path.join("assets"))?;
    std::fs::create_dir_all(path.join("plugins"))?;

    let config = ProjectConfig {
        name: name.to_string(),
        version: "0.1.0".to_string(),
        main_scene: "scenes/main.ron".to_string(),
        icon: None,
        window: WindowConfig::default(),
    };

    let config_path = path.join("project.toml");
    let config_content = toml::to_string_pretty(&config)?;
    std::fs::write(&config_path, config_content)?;

    let scene_content = r#"(
  resources: {},
  entities: {},
)
"#;
    let scene_path = path.join("scenes").join("main.ron");
    std::fs::write(&scene_path, scene_content)?;

    Ok(CurrentProject {
        path: path.to_path_buf(),
        config,
    })
}
