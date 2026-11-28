pub use renzora_project_api::{open_project, CurrentProject, ProjectConfig, WindowConfig};

/// Making a project lives in the contract (`renzora_platform_api::new_project`),
/// so the MCP plugin writes exactly what the launcher's New Project does.
#[cfg(not(target_arch = "wasm32"))]
pub use renzora_platform_api::create_project;
