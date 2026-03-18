//! Load/save lifecycle.json from project root.

use std::path::Path;

use crate::graph::LifecycleGraph;

/// Load a lifecycle graph from a JSON file.
pub fn load_lifecycle(path: &Path) -> Option<LifecycleGraph> {
    let contents = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

/// Save a lifecycle graph to a JSON file.
pub fn save_lifecycle(path: &Path, graph: &LifecycleGraph) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string_pretty(graph)?;
    std::fs::write(path, json)?;
    Ok(())
}
