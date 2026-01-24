//! Export module for building standalone game executables
//!
//! This module handles:
//! - Asset discovery and bundling
//! - Build process orchestration
//! - Cross-platform compilation

pub mod assets;
pub mod build;
pub mod pack;

pub use assets::{discover_assets, copy_assets_to_folder};
pub use build::{ExportConfig, ExportTarget, BuildType, run_export, is_target_installed};
pub use pack::create_packed_exe;
