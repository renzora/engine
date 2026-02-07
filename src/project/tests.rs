//! Tests for project configuration and management
//!
//! Covers ProjectConfig, CurrentProject path resolution, and AppConfig recent projects.

use super::*;
use std::path::PathBuf;

// =============================================================================
// A. ProjectConfig
// =============================================================================

#[test]
fn project_config_default_has_sensible_name() {
    let config = project::ProjectConfig::default();
    assert!(!config.name.is_empty());
    assert!(!config.version.is_empty());
    assert!(!config.main_scene.is_empty());
}

#[test]
fn project_config_serde_toml_roundtrip() {
    let config = project::ProjectConfig {
        name: "Test Game".into(),
        version: "1.0.0".into(),
        main_scene: "scenes/main.ron".into(),
    };
    let toml_str = toml::to_string_pretty(&config).unwrap();
    let restored: project::ProjectConfig = toml::from_str(&toml_str).unwrap();
    assert_eq!(config.name, restored.name);
    assert_eq!(config.version, restored.version);
    assert_eq!(config.main_scene, restored.main_scene);
}

#[test]
fn project_config_custom_values_preserved() {
    let config = project::ProjectConfig {
        name: "My Custom Game".into(),
        version: "2.5.3".into(),
        main_scene: "levels/level1.ron".into(),
    };
    let json = serde_json::to_string(&config).unwrap();
    let restored: project::ProjectConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(config.name, restored.name);
    assert_eq!(config.version, restored.version);
    assert_eq!(config.main_scene, restored.main_scene);
}

// =============================================================================
// B. CurrentProject path resolution
// =============================================================================

#[test]
fn current_project_resolve_path() {
    let project = project::CurrentProject {
        path: PathBuf::from("/projects/my_game"),
        config: project::ProjectConfig::default(),
    };
    let resolved = project.resolve_path("scenes/main.ron");
    assert_eq!(resolved, PathBuf::from("/projects/my_game/scenes/main.ron"));
}

#[test]
fn current_project_main_scene_path() {
    let project = project::CurrentProject {
        path: PathBuf::from("/projects/my_game"),
        config: project::ProjectConfig {
            main_scene: "scenes/test.ron".into(),
            ..Default::default()
        },
    };
    let scene_path = project.main_scene_path();
    assert_eq!(scene_path, PathBuf::from("/projects/my_game/scenes/test.ron"));
}

#[test]
fn current_project_resolve_relative_path() {
    let project = project::CurrentProject {
        path: PathBuf::from("/projects/game"),
        config: project::ProjectConfig::default(),
    };
    let resolved = project.resolve_path("assets/models/player.glb");
    assert!(resolved.to_str().unwrap().contains("assets"));
    assert!(resolved.to_str().unwrap().contains("player.glb"));
}

// =============================================================================
// C. AppConfig recent projects
// =============================================================================

#[test]
fn app_config_add_first_project() {
    let mut config = config::AppConfig::default();
    config.add_recent_project(PathBuf::from("/projects/game1"));
    assert_eq!(config.recent_projects.len(), 1);
    assert_eq!(config.recent_projects[0], PathBuf::from("/projects/game1"));
}

#[test]
fn app_config_duplicate_moves_to_front() {
    let mut config = config::AppConfig::default();
    config.add_recent_project(PathBuf::from("/projects/game1"));
    config.add_recent_project(PathBuf::from("/projects/game2"));
    config.add_recent_project(PathBuf::from("/projects/game1")); // duplicate

    assert_eq!(config.recent_projects.len(), 2);
    assert_eq!(config.recent_projects[0], PathBuf::from("/projects/game1"));
    assert_eq!(config.recent_projects[1], PathBuf::from("/projects/game2"));
}

#[test]
fn app_config_truncates_to_10() {
    let mut config = config::AppConfig::default();
    for i in 0..15 {
        config.add_recent_project(PathBuf::from(format!("/projects/game{}", i)));
    }
    assert_eq!(config.recent_projects.len(), 10);
    // Most recent should be first
    assert_eq!(config.recent_projects[0], PathBuf::from("/projects/game14"));
}

#[test]
fn app_config_empty_add_works() {
    let mut config = config::AppConfig::default();
    assert!(config.recent_projects.is_empty());
    config.add_recent_project(PathBuf::from("/test"));
    assert_eq!(config.recent_projects.len(), 1);
}
