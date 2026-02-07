//! Export module for building standalone game executables
//!
//! This module handles:
//! - Asset discovery and bundling
//! - Build process orchestration
//! - Cross-platform compilation

pub mod assets;
pub mod build;
pub mod pack;

pub use build::{ExportConfig, ExportTarget, BuildType, run_export, is_target_installed};

#[cfg(test)]
mod tests;
