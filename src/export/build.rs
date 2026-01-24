//! Build process for exporting standalone games
//!
//! Handles compiling the runtime binary for target platforms.

use std::fs;
use std::path::{Path, PathBuf};

use super::assets::{copy_all_assets, copy_scene_files, create_project_toml, discover_assets, copy_assets_to_folder};
use crate::core::ExportLogger;

/// Target platform for export
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportTarget {
    Windows,
    Linux,
    MacOS,
    MacOSArm,
}

impl ExportTarget {
    /// Get the Rust target triple for this platform
    pub fn target_triple(&self) -> &'static str {
        match self {
            ExportTarget::Windows => "x86_64-pc-windows-msvc",
            ExportTarget::Linux => "x86_64-unknown-linux-gnu",
            ExportTarget::MacOS => "x86_64-apple-darwin",
            ExportTarget::MacOSArm => "aarch64-apple-darwin",
        }
    }

    /// Get the executable file extension for this platform
    pub fn exe_extension(&self) -> &'static str {
        match self {
            ExportTarget::Windows => ".exe",
            ExportTarget::Linux | ExportTarget::MacOS | ExportTarget::MacOSArm => "",
        }
    }

    /// Get a display name for this platform
    pub fn display_name(&self) -> &'static str {
        match self {
            ExportTarget::Windows => "Windows",
            ExportTarget::Linux => "Linux",
            ExportTarget::MacOS => "macOS (Intel)",
            ExportTarget::MacOSArm => "macOS (Apple Silicon)",
        }
    }
}

/// Build type (Debug or Release)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildType {
    Debug,
    Release,
}

impl BuildType {
    pub fn cargo_flag(&self) -> &'static str {
        match self {
            BuildType::Debug => "",
            BuildType::Release => "--release",
        }
    }

    pub fn profile_name(&self) -> &'static str {
        match self {
            BuildType::Debug => "debug",
            BuildType::Release => "release",
        }
    }
}

/// Configuration for the export process
#[derive(Debug, Clone)]
pub struct ExportConfig {
    /// Name of the game (used for window title and executable name)
    pub game_name: String,
    /// Target platforms to build for
    pub targets: Vec<ExportTarget>,
    /// Build type (Debug or Release)
    pub build_type: BuildType,
    /// Output directory for the exported game
    pub output_dir: PathBuf,
    /// Path to the main scene file
    pub main_scene: PathBuf,
    /// Path to the project directory
    pub project_dir: PathBuf,
    /// Whether to copy all assets or just referenced ones
    pub copy_all_assets: bool,
}

/// Result of an export operation
#[derive(Debug)]
pub struct ExportResult {
    pub success: bool,
    pub message: String,
    pub output_paths: Vec<PathBuf>,
    pub errors: Vec<String>,
}

/// Run the export process with logging
pub fn run_export(config: &ExportConfig, logger: &ExportLogger) -> ExportResult {
    let mut result = ExportResult {
        success: true,
        message: String::new(),
        output_paths: Vec::new(),
        errors: Vec::new(),
    };

    let total_steps = 5 + config.targets.len(); // validate, output dir, discover, copy, project.toml, + per-target
    let mut current_step = 0;

    // Helper to update progress
    let update_progress = |step: usize, msg: &str| {
        let progress = step as f32 / total_steps as f32;
        logger.set_progress(progress, msg);
    };

    // Step 1: Validate configuration
    update_progress(current_step, "Validating configuration...");
    logger.info("Starting export process...");
    logger.info(format!("Game: {}", config.game_name));
    logger.info(format!("Targets: {}", config.targets.iter().map(|t| t.display_name()).collect::<Vec<_>>().join(", ")));

    if !config.main_scene.exists() {
        logger.error(format!("Main scene not found: {:?}", config.main_scene));
        result.success = false;
        result.message = format!("Main scene not found: {:?}", config.main_scene);
        return result;
    }
    logger.success(format!("Main scene: {:?}", config.main_scene));

    if config.targets.is_empty() {
        logger.error("No target platforms selected");
        result.success = false;
        result.message = "No target platforms selected".to_string();
        return result;
    }
    current_step += 1;

    // Step 2: Create output directory
    update_progress(current_step, "Creating output directory...");
    logger.info(format!("Output directory: {:?}", config.output_dir));
    if let Err(e) = fs::create_dir_all(&config.output_dir) {
        logger.error(format!("Failed to create output directory: {}", e));
        result.success = false;
        result.message = format!("Failed to create output directory: {}", e);
        return result;
    }

    // Create a staging directory for assets
    let staging_dir = config.output_dir.join("_staging");
    if let Err(e) = fs::create_dir_all(&staging_dir) {
        logger.error(format!("Failed to create staging directory: {}", e));
        result.success = false;
        result.message = format!("Failed to create staging directory: {}", e);
        return result;
    }
    logger.success("Created staging directory");
    current_step += 1;

    // Step 3: Discover and copy assets
    update_progress(current_step, "Discovering assets...");

    if config.copy_all_assets {
        logger.info("Copying all project assets...");
        if let Err(e) = copy_all_assets(&config.project_dir, &staging_dir) {
            logger.warning(format!("Failed to copy assets: {}", e));
            result.errors.push(format!("Failed to copy assets: {}", e));
        } else {
            logger.success("Copied all assets");
        }
    } else {
        // Discover only referenced assets
        logger.info("Discovering referenced assets...");
        match discover_assets(&config.main_scene, &config.project_dir) {
            Ok(assets) => {
                logger.success(format!("Found {} referenced assets", assets.len()));
                for asset in &assets {
                    logger.info(format!("  Asset: {}", asset.display()));
                }

                // Copy discovered assets
                if let Err(e) = copy_assets_to_folder(&assets, &config.project_dir, &staging_dir) {
                    logger.warning(format!("Failed to copy discovered assets: {}", e));
                    result.errors.push(format!("Failed to copy assets: {}", e));
                } else {
                    logger.success("Copied discovered assets");
                }
            }
            Err(e) => {
                logger.warning(format!("Failed to discover assets: {}", e));
                result.errors.push(format!("Failed to discover assets: {}", e));
            }
        }
    }
    current_step += 1;

    // Step 4: Copy scene files
    update_progress(current_step, "Copying scene files...");
    logger.info("Copying scene files...");
    if let Err(e) = copy_scene_files(&config.main_scene, &config.project_dir, &staging_dir) {
        logger.warning(format!("Failed to copy scene files: {}", e));
        result.errors.push(format!("Failed to copy scene files: {}", e));
    } else {
        logger.success("Copied scene files");
    }
    current_step += 1;

    // Step 5: Create project.toml
    update_progress(current_step, "Creating project manifest...");
    let main_scene_rel = config
        .main_scene
        .strip_prefix(&config.project_dir)
        .unwrap_or(&config.main_scene)
        .to_string_lossy()
        .replace('\\', "/");

    logger.info(format!("Creating project.toml (main_scene: {})", main_scene_rel));
    if let Err(e) = create_project_toml(&config.game_name, &main_scene_rel, &staging_dir) {
        logger.warning(format!("Failed to create project.toml: {}", e));
        result.errors.push(format!("Failed to create project.toml: {}", e));
    } else {
        logger.success("Created project.toml");
    }
    current_step += 1;

    // Build for each target platform
    for target in &config.targets {
        update_progress(current_step, &format!("Building for {}...", target.display_name()));
        logger.info(format!("Building for {}...", target.display_name()));

        // Get the pre-built runtime binary
        let runtime_binary = match get_runtime_binary(target) {
            Ok(path) => {
                logger.info(format!("  Runtime: {:?}", path));
                path
            }
            Err(e) => {
                logger.error(format!("Failed to find runtime for {}: {}", target.display_name(), e));
                result.errors.push(format!(
                    "Failed to find runtime for {}: {}",
                    target.display_name(),
                    e
                ));
                current_step += 1;
                continue;
            }
        };

        // Create the output exe name
        let exe_name = format!("{}{}", config.game_name, target.exe_extension());
        let output_exe = config.output_dir.join(&exe_name);
        logger.info(format!("  Output: {:?}", output_exe));

        // Create packed executable (runtime + assets in single file)
        logger.info("  Packing assets with zstd compression...");
        match super::pack::create_packed_exe(&runtime_binary, &staging_dir, &output_exe, logger) {
            Ok(()) => {
                logger.success(format!("Created {}", exe_name));
                result.output_paths.push(output_exe);
            }
            Err(e) => {
                logger.error(format!("Failed to create packed exe for {}: {}", target.display_name(), e));
                result.errors.push(format!(
                    "Failed to create packed exe for {}: {}",
                    target.display_name(),
                    e
                ));
            }
        }
        current_step += 1;
    }

    // Clean up staging directory
    logger.info("Cleaning up staging directory...");
    let _ = fs::remove_dir_all(&staging_dir);
    logger.success("Cleanup complete");

    // Set final progress
    logger.set_progress(1.0, "Export complete");

    // Set result message
    if result.errors.is_empty() {
        let msg = format!(
            "Export completed successfully for {} platform(s)",
            config.targets.len()
        );
        logger.success(&msg);
        result.message = msg;
    } else if result.output_paths.is_empty() {
        result.success = false;
        let msg = format!("Export failed with {} error(s)", result.errors.len());
        logger.error(&msg);
        result.message = msg;
    } else {
        let msg = format!(
            "Export completed with {} warning(s)",
            result.errors.len()
        );
        logger.warning(&msg);
        result.message = msg;
    }

    result
}

/// Get the path to pre-built runtime binaries
fn get_runtimes_dir() -> Result<PathBuf, String> {
    // Try multiple locations for the runtimes folder:
    // 1. Next to the editor executable
    // 2. In the current working directory
    // 3. In a "runtimes" subdirectory of the exe's parent

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            // Check next to executable
            let runtimes_dir = exe_dir.join("runtimes");
            if runtimes_dir.exists() {
                return Ok(runtimes_dir);
            }

            // Check parent directory (for development: target/debug/../runtimes)
            if let Some(parent) = exe_dir.parent() {
                let runtimes_dir = parent.join("runtimes");
                if runtimes_dir.exists() {
                    return Ok(runtimes_dir);
                }

                // Check two levels up (target/../runtimes)
                if let Some(grandparent) = parent.parent() {
                    let runtimes_dir = grandparent.join("runtimes");
                    if runtimes_dir.exists() {
                        return Ok(runtimes_dir);
                    }
                }
            }
        }
    }

    // Check current working directory
    if let Ok(cwd) = std::env::current_dir() {
        let runtimes_dir = cwd.join("runtimes");
        if runtimes_dir.exists() {
            return Ok(runtimes_dir);
        }
    }

    Err("Runtime binaries not found. Please ensure the 'runtimes' folder exists with pre-built binaries.".to_string())
}

/// Get the path to the pre-built runtime binary for a specific target
fn get_runtime_binary(target: &ExportTarget) -> Result<PathBuf, String> {
    let runtimes_dir = get_runtimes_dir()?;

    let target_folder = match target {
        ExportTarget::Windows => "windows",
        ExportTarget::Linux => "linux",
        ExportTarget::MacOS => "macos-intel",
        ExportTarget::MacOSArm => "macos-arm",
    };

    let binary_name = format!("renzora_runtime{}", target.exe_extension());
    let binary_path = runtimes_dir.join(target_folder).join(&binary_name);

    if binary_path.exists() {
        Ok(binary_path)
    } else {
        Err(format!(
            "Pre-built runtime for {} not found at {:?}",
            target.display_name(),
            binary_path
        ))
    }
}

/// Check if a pre-built runtime binary exists for this target
pub fn is_target_installed(target: &ExportTarget) -> bool {
    get_runtime_binary(target).is_ok()
}
