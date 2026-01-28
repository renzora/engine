//! Blueprint file serialization (.blueprint files)

use serde::{Deserialize, Serialize};
use std::path::Path;
use super::BlueprintGraph;

/// Blueprint file format (JSON)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintFile {
    /// File format version
    pub version: u32,

    /// The blueprint graph
    pub graph: BlueprintGraph,
}

impl BlueprintFile {
    /// Current file format version
    pub const CURRENT_VERSION: u32 = 1;

    /// Create a new blueprint file from a graph
    pub fn new(graph: BlueprintGraph) -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            graph,
        }
    }

    /// Save to a file
    pub fn save(&self, path: &Path) -> Result<(), BlueprintError> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| BlueprintError::SerializeError(e.to_string()))?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| BlueprintError::IoError(e.to_string()))?;
        }

        std::fs::write(path, json)
            .map_err(|e| BlueprintError::IoError(e.to_string()))?;

        Ok(())
    }

    /// Load from a file
    pub fn load(path: &Path) -> Result<Self, BlueprintError> {
        let json = std::fs::read_to_string(path)
            .map_err(|e| BlueprintError::IoError(e.to_string()))?;

        let file: BlueprintFile = serde_json::from_str(&json)
            .map_err(|e| BlueprintError::DeserializeError(e.to_string()))?;

        // Handle version migrations if needed
        if file.version > Self::CURRENT_VERSION {
            return Err(BlueprintError::VersionError(format!(
                "Blueprint version {} is newer than supported version {}",
                file.version, Self::CURRENT_VERSION
            )));
        }

        Ok(file)
    }
}

/// Errors that can occur during blueprint operations
#[derive(Debug, Clone)]
pub enum BlueprintError {
    IoError(String),
    SerializeError(String),
    DeserializeError(String),
    VersionError(String),
}

impl std::fmt::Display for BlueprintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlueprintError::IoError(e) => write!(f, "IO error: {}", e),
            BlueprintError::SerializeError(e) => write!(f, "Serialization error: {}", e),
            BlueprintError::DeserializeError(e) => write!(f, "Deserialization error: {}", e),
            BlueprintError::VersionError(e) => write!(f, "Version error: {}", e),
        }
    }
}

impl std::error::Error for BlueprintError {}

/// List all blueprint files in a directory
pub fn list_blueprints(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut blueprints = Vec::new();

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "blueprint").unwrap_or(false) {
                blueprints.push(path);
            }
        }
    }

    blueprints.sort();
    blueprints
}

/// Export blueprint to Rhai code file
#[allow(dead_code)]
pub fn export_to_rhai(graph: &BlueprintGraph, path: &Path) -> Result<(), BlueprintError> {
    let result = super::generate_rhai_code(graph);

    // Add header comment
    let code = format!(
        "// Generated from blueprint: {}\n// Do not edit directly - changes will be overwritten\n\n{}",
        graph.name,
        result.code
    );

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| BlueprintError::IoError(e.to_string()))?;
    }

    std::fs::write(path, code)
        .map_err(|e| BlueprintError::IoError(e.to_string()))?;

    Ok(())
}
