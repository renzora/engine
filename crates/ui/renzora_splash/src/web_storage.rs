//! Browser localStorage backend for project storage on WASM.
//!
//! Keys used:
//! - `renzora:config`          — JSON-serialized AppConfig
//! - `renzora:projects`        — JSON array of project slugs
//! - `renzora:project:{slug}`  — JSON-serialized project data (config + scene files)

use std::path::PathBuf;

use renzora_core::{CurrentProject, ProjectConfig, WindowConfig};
use serde::{Deserialize, Serialize};

use crate::config::AppConfig;

const CONFIG_KEY: &str = "renzora:config";
const PROJECTS_KEY: &str = "renzora:projects";

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

/// A project stored in localStorage.
#[derive(Serialize, Deserialize, Clone)]
pub struct WebProject {
    pub slug: String,
    pub config: ProjectConfig,
    /// Map of relative path -> file content (e.g. "scenes/main.ron" -> "(...)")
    pub files: std::collections::HashMap<String, String>,
}

/// Load AppConfig from localStorage.
pub fn load_config() -> AppConfig {
    let s = storage();
    s.and_then(|s| s.get_item(CONFIG_KEY).ok().flatten())
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

/// Save AppConfig to localStorage.
pub fn save_config(config: &AppConfig) {
    if let Some(s) = storage() {
        if let Ok(json) = serde_json::to_string(config) {
            let _ = s.set_item(CONFIG_KEY, &json);
        }
    }
}

/// List all stored project slugs.
pub fn list_projects() -> Vec<String> {
    storage()
        .and_then(|s| s.get_item(PROJECTS_KEY).ok().flatten())
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn save_project_list(slugs: &[String]) {
    if let Some(s) = storage() {
        if let Ok(json) = serde_json::to_string(slugs) {
            let _ = s.set_item(PROJECTS_KEY, &json);
        }
    }
}

/// Create a new project in localStorage.
pub fn create_web_project(name: &str) -> Result<CurrentProject, String> {
    let slug = name
        .replace(' ', "_")
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect::<String>();

    if slug.is_empty() {
        return Err("Invalid project name".to_string());
    }

    let config = ProjectConfig {
        name: name.to_string(),
        version: "0.1.0".to_string(),
        main_scene: "scenes/main.ron".to_string(),
        icon: None,
        window: WindowConfig::default(),
    };

    let scene_content = "(
  resources: {},
  entities: {},
)
";

    let mut files = std::collections::HashMap::new();
    files.insert(
        "project.toml".to_string(),
        toml::to_string_pretty(&config).map_err(|e| e.to_string())?,
    );
    files.insert("scenes/main.ron".to_string(), scene_content.to_string());

    let web_project = WebProject {
        slug: slug.clone(),
        config: config.clone(),
        files,
    };

    // Save project data
    let key = format!("renzora:project:{}", slug);
    let json = serde_json::to_string(&web_project).map_err(|e| e.to_string())?;
    storage()
        .ok_or("No localStorage available")?
        .set_item(&key, &json)
        .map_err(|_| "Failed to write to localStorage")?;

    // Add to project list
    let mut slugs = list_projects();
    if !slugs.contains(&slug) {
        slugs.insert(0, slug.clone());
        save_project_list(&slugs);
    }

    // Use the slug as the virtual path
    let path = PathBuf::from(format!("web:/{}", slug));

    Ok(CurrentProject { path, config })
}

/// Load a project from localStorage by slug.
pub fn load_web_project(slug: &str) -> Option<CurrentProject> {
    let key = format!("renzora:project:{}", slug);
    let json = storage()?.get_item(&key).ok()??;
    let web_project: WebProject = serde_json::from_str(&json).ok()?;

    let path = PathBuf::from(format!("web:/{}", slug));
    Some(CurrentProject {
        path,
        config: web_project.config,
    })
}

/// Delete a project from localStorage.
pub fn delete_web_project(slug: &str) {
    if let Some(s) = storage() {
        let key = format!("renzora:project:{}", slug);
        let _ = s.remove_item(&key);

        let mut slugs = list_projects();
        slugs.retain(|s| s != slug);
        save_project_list(&slugs);
    }
}
