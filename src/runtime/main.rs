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

#[path = "pack_asset_reader.rs"]
mod pack_asset_reader;

use bevy::asset::io::{AssetSourceBuilder, AssetSourceId};
use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use pack_asset_reader::{PackAssetReader, PackIndex};
use std::io::Write;

fn main() {
    // Set up panic hook to show errors before exiting
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("\n==== RUNTIME CRASH ====");
        eprintln!("{}", panic_info);
        eprintln!("========================\n");
        wait_for_enter();
    }));

    // Run the actual main logic, catching any errors
    if let Err(e) = run_game() {
        eprintln!("\n==== RUNTIME ERROR ====");
        eprintln!("{}", e);
        eprintln!("========================\n");
        wait_for_enter();
    }
}

/// Wait for user to press Enter (keeps console open on error)
fn wait_for_enter() {
    eprint!("Press Enter to exit...");
    let _ = std::io::stderr().flush();
    let mut input = String::new();
    let _ = std::io::stdin().read_line(&mut input);
}

/// Main game logic wrapped in Result for error handling
fn run_game() -> Result<(), String> {
    println!("Renzora Runtime starting...");

    // Try to load pack index from embedded binary (lazy loading - no data loaded yet)
    let pack_index = PackIndex::from_current_exe();

    if pack_index.is_none() {
        println!("Warning: No embedded pack found, will try filesystem");
    }

    // Try to read window title from project.toml (pack or filesystem)
    let window_title = read_project_title(&pack_index).unwrap_or_else(|| "Renzora Game".to_string());
    println!("Window title: {}", window_title);

    let mut app = App::new();

    // Configure plugins based on whether we have pack data
    if let Some(ref pack) = pack_index {
        println!("Running from packed executable (lazy loading enabled)");
        let pack_clone = pack.clone();

        // Register our pack-based asset source BEFORE adding plugins
        app.register_asset_source(
            AssetSourceId::Default,
            AssetSourceBuilder::new(move || {
                Box::new(PackAssetReader::new(pack_clone.clone()))
            }),
        );

        // Now add DefaultPlugins - AssetPlugin will use our registered source
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: window_title,
                        resolution: (1280u32, 720u32).into(),
                        ..default()
                    }),
                    ..default()
                }),
        );

        // Insert pack index as resource
        app.insert_resource(pack.clone());
    } else {
        println!("Running from filesystem");
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: window_title,
                        resolution: (1280u32, 720u32).into(),
                        ..default()
                    }),
                    ..default()
                }),
        );
    }

    println!("Starting Bevy app...");
    app.add_plugins(RuntimePlugin).run();

    Ok(())
}

/// Read the project name from project.toml to use as window title
fn read_project_title(pack_index: &Option<PackIndex>) -> Option<String> {
    // First try to read from pack index if available
    if let Some(ref pack) = pack_index {
        if let Some(content) = pack.read_string("project.toml") {
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
        // Register shared component types for scene deserialization
        app
            // Light components
            .register_type::<shared::PointLightData>()
            .register_type::<shared::DirectionalLightData>()
            .register_type::<shared::SpotLightData>()
            // Physics components
            .register_type::<shared::PhysicsBodyData>()
            .register_type::<shared::PhysicsBodyType>()
            .register_type::<shared::CollisionShapeData>()
            .register_type::<shared::CollisionShapeType>()
            // Camera components
            .register_type::<shared::CameraNodeData>()
            .register_type::<shared::CameraRigData>()
            .register_type::<shared::Camera2DData>()
            // Mesh components
            .register_type::<shared::MeshNodeData>()
            .register_type::<shared::MeshPrimitiveType>()
            // Sprite components
            .register_type::<shared::Sprite2DData>()
            // Instance components
            .register_type::<shared::MeshInstanceData>()
            .register_type::<shared::SceneInstanceData>()
            // UI components
            .register_type::<shared::UIPanelData>()
            .register_type::<shared::UILabelData>()
            .register_type::<shared::UIButtonData>()
            .register_type::<shared::UIImageData>()
            // Environment components
            .register_type::<shared::WorldEnvironmentData>()
            .register_type::<shared::SkyMode>()
            .register_type::<shared::ProceduralSkyData>()
            .register_type::<shared::PanoramaSkyData>()
            .register_type::<shared::TonemappingMode>()
            // Add plugins
            .add_plugins((
                loader::RuntimeLoaderPlugin,
                camera::RuntimeCameraPlugin,
                // Physics plugin (runs immediately in runtime, not paused)
                shared::RenzoraPhysicsPlugin::new(false),
            ));
    }
}
