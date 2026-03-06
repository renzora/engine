//! Runtime template management — download/locate pre-built runtime binaries.

use serde::{Deserialize, Serialize};
use bevy::prelude::*;
use std::path::PathBuf;

/// Supported export platforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Platform {
    WindowsX64,
    LinuxX64,
    MacOSX64,
    MacOSArm64,
    AndroidArm64,
    FireTVArm64,
    #[serde(rename = "ios_arm64")]
    IOSArm64,
    WebWasm32,
}

impl Platform {
    pub const ALL: &'static [Platform] = &[
        Platform::WindowsX64,
        Platform::LinuxX64,
        Platform::MacOSX64,
        Platform::MacOSArm64,
        Platform::AndroidArm64,
        Platform::FireTVArm64,
        Platform::IOSArm64,
        Platform::WebWasm32,
    ];

    pub fn display_name(&self) -> &'static str {
        match self {
            Platform::WindowsX64 => "Windows (x64)",
            Platform::LinuxX64 => "Linux (x64)",
            Platform::MacOSX64 => "macOS (x64)",
            Platform::MacOSArm64 => "macOS (ARM64)",
            Platform::AndroidArm64 => "Android (ARM64)",
            Platform::FireTVArm64 => "Fire TV (ARM64)",
            Platform::IOSArm64 => "iOS (ARM64)",
            Platform::WebWasm32 => "Web (WASM)",
        }
    }

    pub fn binary_name(&self, project_name: &str) -> String {
        match self {
            Platform::WindowsX64 => format!("{}.exe", project_name),
            Platform::LinuxX64 => project_name.to_string(),
            Platform::MacOSX64 | Platform::MacOSArm64 => project_name.to_string(),
            Platform::AndroidArm64 | Platform::FireTVArm64 => format!("{}.apk", project_name),
            Platform::IOSArm64 => format!("{}.app", project_name),
            Platform::WebWasm32 => format!("{}.wasm", project_name),
        }
    }

    pub fn template_filename(&self) -> &'static str {
        match self {
            Platform::WindowsX64 => "renzora-runtime-windows-x64.exe",
            Platform::LinuxX64 => "renzora-runtime-linux-x64",
            Platform::MacOSX64 => "renzora-runtime-macos-x64",
            Platform::MacOSArm64 => "renzora-runtime-macos-arm64",
            Platform::AndroidArm64 => "renzora-runtime-android-arm64.apk",
            Platform::FireTVArm64 => "renzora-runtime-firetv-arm64.apk",
            Platform::IOSArm64 => "renzora-runtime-ios-arm64",
            Platform::WebWasm32 => "renzora-runtime-web-wasm32",
        }
    }

    /// Detect the current host platform.
    pub fn current() -> Option<Platform> {
        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        { return Some(Platform::WindowsX64); }
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        { return Some(Platform::LinuxX64); }
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        { return Some(Platform::MacOSX64); }
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        { return Some(Platform::MacOSArm64); }
        #[allow(unreachable_code)]
        None
    }
}

/// A downloaded/available runtime template.
#[derive(Debug, Clone)]
pub struct ExportTemplate {
    pub platform: Platform,
    pub path: PathBuf,
    pub version: String,
}

/// Manages the template cache directory and available templates.
#[derive(Resource)]
pub struct TemplateManager {
    pub cache_dir: PathBuf,
    pub templates: Vec<ExportTemplate>,
    pub download_url_base: String,
}

impl Default for TemplateManager {
    fn default() -> Self {
        let cache_dir = dirs_cache_dir().join("templates");
        let mut mgr = Self {
            cache_dir,
            templates: Vec::new(),
            download_url_base: String::new(),
        };
        mgr.scan();
        mgr
    }
}

impl TemplateManager {
    /// Scan the cache directory for available templates.
    pub fn scan(&mut self) {
        self.templates.clear();

        if !self.cache_dir.exists() {
            return;
        }

        for platform in Platform::ALL {
            let path = self.cache_dir.join(platform.template_filename());
            if path.exists() {
                self.templates.push(ExportTemplate {
                    platform: *platform,
                    path,
                    version: "local".to_string(),
                });
            }
        }
    }

    /// Check if a template is available for the given platform.
    pub fn get(&self, platform: Platform) -> Option<&ExportTemplate> {
        self.templates.iter().find(|t| t.platform == platform)
    }

    /// Check if a template is installed for the given platform.
    pub fn is_installed(&self, platform: Platform) -> bool {
        self.get(platform).is_some()
    }

    /// Install a template from a file path (copy into cache).
    pub fn install_from_file(&mut self, platform: Platform, source: &std::path::Path) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.cache_dir)?;
        let dest = self.cache_dir.join(platform.template_filename());
        std::fs::copy(source, &dest)?;
        self.scan();
        Ok(())
    }
}

fn dirs_cache_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("renzora");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join("Library/Application Support/renzora");
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".config/renzora");
        }
    }
    PathBuf::from(".renzora")
}
