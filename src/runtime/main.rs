//! Renzora Runtime - Standalone game executable
//!
//! This is the entry point for exported games. It loads the project's main scene
//! and runs the game without any editor UI.

// Hide console window on Windows (release builds)
// Temporarily disabled for debugging
// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Include shared module using path attribute
#[path = "../shared/mod.rs"]
mod shared;

// Runtime-specific modules (relative to this file's directory)
#[path = "camera.rs"]
mod camera;

#[path = "loader.rs"]
mod loader;

#[path = "pack.rs"]
mod pack;

use bevy::prelude::*;

fn main() {
    // Try to read window title from project.toml
    let window_title = read_project_title().unwrap_or_else(|| "Renzora Game".to_string());

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: window_title,
                        resolution: (1280u32, 720u32).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    // Allow loading assets from current directory
                    ..default()
                }),
        )
        .add_plugins(RuntimePlugin)
        .run();
}

/// Read the project name from project.toml to use as window title
fn read_project_title() -> Option<String> {
    // First try to read from embedded pack
    if let Some(mut pack_reader) = pack::PackReader::from_current_exe() {
        if let Some(content) = pack_reader.read_string("project.toml") {
            let config: toml::Value = toml::from_str(&content).ok()?;
            return config
                .get("project")
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .map(|s| s.to_string());
        }
    }

    // Fall back to filesystem
    let content = std::fs::read_to_string("project.toml").ok()?;
    let config: toml::Value = toml::from_str(&content).ok()?;
    config
        .get("project")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .map(|s| s.to_string())
}

/// Main plugin for the game runtime
pub struct RuntimePlugin;

impl Plugin for RuntimePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((loader::RuntimeLoaderPlugin, camera::RuntimeCameraPlugin));
    }
}
