//! Entity spawning system for creating new entities in the editor.
//!
//! This module provides templates and spawn functions for creating various
//! entity types (meshes, lights, cameras, etc.) from the editor UI.

#![allow(dead_code)]

pub mod meshes;
pub mod lights;
pub mod cameras;
pub mod physics;
pub mod nodes2d;
pub mod ui;
pub mod environment;
pub mod scenes;
pub mod terrain;
pub mod layouts;
pub mod procedural_meshes;

use bevy::prelude::*;

pub use scenes::*;

/// Category for organizing entity templates in the UI
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    /// Basic 3D nodes (Node3D empty)
    Nodes3D,
    /// 3D mesh primitives and model instances
    Mesh,
    /// Light sources (point, directional, spot)
    Light,
    /// Camera types (3D, 2D, camera rigs)
    Camera,
    /// Physics bodies and collision shapes
    Physics,
    /// 2D nodes (sprites, 2D cameras)
    TwoD,
    /// UI elements (panels, labels, buttons)
    UI,
    /// Environment settings
    Environment,
    /// Terrain entities
    Terrain,
    /// Pre-built scene layouts
    Layout,
}

impl Category {
    /// Display name for the category
    pub fn display_name(&self) -> &'static str {
        match self {
            Category::Nodes3D => "3D Nodes",
            Category::Mesh => "Meshes",
            Category::Light => "Lights",
            Category::Camera => "Cameras",
            Category::Physics => "Physics",
            Category::TwoD => "2D",
            Category::UI => "UI",
            Category::Environment => "Environment",
            Category::Terrain => "Terrain",
            Category::Layout => "Layouts",
        }
    }

    /// All categories in display order
    pub fn all() -> &'static [Category] {
        &[
            Category::Nodes3D,
            Category::Mesh,
            Category::Light,
            Category::Camera,
            Category::Physics,
            Category::TwoD,
            Category::UI,
            Category::Environment,
            Category::Terrain,
            Category::Layout,
        ]
    }
}

/// Function signature for spawn functions
pub type SpawnFn = fn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Option<Entity>,
) -> Entity;

/// Template for spawning an entity type
pub struct EntityTemplate {
    /// Display name shown in menus
    pub name: &'static str,
    /// Category for menu organization
    pub category: Category,
    /// Function to spawn the entity
    pub spawn: SpawnFn,
}

/// Get all entity templates
pub fn all_templates() -> Vec<&'static EntityTemplate> {
    let mut templates = Vec::new();
    templates.extend(meshes::TEMPLATES.iter());
    templates.extend(lights::TEMPLATES.iter());
    templates.extend(cameras::TEMPLATES.iter());
    templates.extend(physics::TEMPLATES.iter());
    templates.extend(nodes2d::TEMPLATES.iter());
    templates.extend(ui::TEMPLATES.iter());
    templates.extend(environment::TEMPLATES.iter());
    templates.extend(terrain::TEMPLATES.iter());
    templates.extend(layouts::TEMPLATES.iter());
    templates
}

/// Get templates filtered by category
pub fn templates_by_category(category: Category) -> Vec<&'static EntityTemplate> {
    all_templates()
        .into_iter()
        .filter(|t| t.category == category)
        .collect()
}
