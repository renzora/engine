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
    AndroidX86_64,
    FireTVArm64,
    #[serde(rename = "ios_arm64")]
    IOSArm64,
    #[serde(rename = "tvos_arm64")]
    TvOSArm64,
    WebWasm32,
}

impl Platform {
    pub const ALL: &'static [Platform] = &[
        Platform::WindowsX64,
        Platform::LinuxX64,
        Platform::MacOSX64,
        Platform::MacOSArm64,
        Platform::AndroidArm64,
        Platform::AndroidX86_64,
        Platform::FireTVArm64,
        Platform::IOSArm64,
        Platform::TvOSArm64,
        Platform::WebWasm32,
    ];

    pub fn display_name(&self) -> &'static str {
        match self {
            Platform::WindowsX64 => "Windows (x64)",
            Platform::LinuxX64 => "Linux (x64)",
            Platform::MacOSX64 => "macOS (x64)",
            Platform::MacOSArm64 => "macOS (ARM64)",
            Platform::AndroidArm64 => "Android (ARM64)",
            Platform::AndroidX86_64 => "Android (x86_64)",
            Platform::FireTVArm64 => "Fire TV",
            Platform::IOSArm64 => "iOS (ARM64)",
            Platform::TvOSArm64 => "Apple TV",
            Platform::WebWasm32 => "Web (WASM)",
        }
    }

    pub fn binary_name(&self, project_name: &str) -> String {
        match self {
            Platform::WindowsX64 => format!("{}.exe", project_name),
            Platform::LinuxX64 => project_name.to_string(),
            Platform::MacOSX64 | Platform::MacOSArm64 => project_name.to_string(),
            Platform::AndroidArm64 | Platform::AndroidX86_64 | Platform::FireTVArm64 => format!("{}.apk", project_name),
            Platform::IOSArm64 | Platform::TvOSArm64 => format!("{}.ipa", project_name),
            Platform::WebWasm32 => format!("{}.wasm", project_name),
        }
    }

    /// Runtime binary name within the runtime/ directory.
    pub fn runtime_binary_name(&self) -> &'static str {
        match self {
            Platform::WindowsX64 => "renzora-runtime.exe",
            Platform::LinuxX64 => "renzora-runtime",
            Platform::MacOSX64 | Platform::MacOSArm64 => "renzora-runtime",
            _ => self.template_filename(),
        }
    }

    /// Server binary name within the server/ directory.
    pub fn server_binary_name_in_dir(&self) -> Option<&'static str> {
        match self {
            Platform::WindowsX64 => Some("renzora-server.exe"),
            Platform::LinuxX64 => Some("renzora-server"),
            Platform::MacOSX64 | Platform::MacOSArm64 => Some("renzora-server"),
            _ => None,
        }
    }

    pub fn template_filename(&self) -> &'static str {
        match self {
            Platform::WindowsX64 => "renzora-runtime-windows-x64.exe",
            Platform::LinuxX64 => "renzora-runtime-linux-x64",
            Platform::MacOSX64 => "renzora-runtime-macos-x64",
            Platform::MacOSArm64 => "renzora-runtime-macos-arm64",
            Platform::AndroidArm64 => "renzora-runtime-android-arm64.apk",
            Platform::AndroidX86_64 => "renzora-runtime-android-x86_64.apk",
            Platform::FireTVArm64 => "renzora-runtime-firetv-arm64.apk",
            Platform::IOSArm64 => "renzora-runtime-ios-arm64.zip",
            Platform::TvOSArm64 => "renzora-runtime-tvos-arm64.zip",
            Platform::WebWasm32 => "renzora-runtime-web-wasm32.zip",
        }
    }

    /// Server template filename for this platform (desktop only).
    pub fn server_template_filename(&self) -> Option<&'static str> {
        match self {
            Platform::WindowsX64 => Some("renzora-server-windows-x64.exe"),
            Platform::LinuxX64 => Some("renzora-server-linux-x64"),
            Platform::MacOSX64 => Some("renzora-server-macos-x64"),
            Platform::MacOSArm64 => Some("renzora-server-macos-arm64"),
            _ => None, // No server for mobile/web
        }
    }

    /// Server binary output name.
    pub fn server_binary_name(&self, project_name: &str) -> Option<String> {
        match self {
            Platform::WindowsX64 => Some(format!("{}-server.exe", project_name)),
            Platform::LinuxX64 | Platform::MacOSX64 | Platform::MacOSArm64 => {
                Some(format!("{}-server", project_name))
            }
            _ => None,
        }
    }

    pub fn supported_devices(&self) -> &'static str {
        match self {
            Platform::WindowsX64 => "Desktop PCs, laptops, PCVR (SteamVR, Oculus Link)",
            Platform::LinuxX64 => "Desktop PCs, laptops, Steam Deck",
            Platform::MacOSX64 => "Intel Macs",
            Platform::MacOSArm64 => "Apple Silicon Macs (M1/M2/M3/M4)",
            Platform::AndroidArm64 => "Phones, tablets, Meta Quest, Pico, HTC Vive Focus",
            Platform::AndroidX86_64 => "Android emulators",
            Platform::FireTVArm64 => "Fire TV Stick 4K Max, Fire TV Cube (3rd gen+)",
            Platform::IOSArm64 => "iPhone, iPad",
            Platform::TvOSArm64 => "Apple TV 4K, Apple TV HD",
            Platform::WebWasm32 => "All modern browsers",
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

/// A downloaded/available runtime or server template.
#[derive(Debug, Clone)]
pub struct ExportTemplate {
    pub platform: Platform,
    pub path: PathBuf,
    pub version: String,
    pub is_server: bool,
}

/// Manages export templates from dist/{platform}/{target}/ directories.
///
/// Each target (runtime, server) is built separately with its own hash.
/// Templates are found as sibling directories to the editor's folder.
#[derive(Resource)]
pub struct TemplateManager {
    /// Parent of the editor dir (e.g. dist/windows-x64/)
    pub dist_dir: PathBuf,
    pub templates: Vec<ExportTemplate>,
}

impl Default for TemplateManager {
    fn default() -> Self {
        // Editor runs from dist/{platform}/editor/ — go up one level to dist/{platform}/
        let dist_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))  // editor/
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))  // {platform}/
            .unwrap_or_else(|| PathBuf::from("."));
        let mut mgr = Self {
            dist_dir,
            templates: Vec::new(),
        };
        mgr.scan();
        mgr
    }
}

impl TemplateManager {
    /// Scan for templates in runtime/ and server/ sibling directories.
    pub fn scan(&mut self) {
        self.templates.clear();

        // Check runtime/ directory
        let runtime_dir = self.dist_dir.join("runtime");
        if runtime_dir.exists() {
            for platform in Platform::ALL {
                let path = runtime_dir.join(platform.runtime_binary_name());
                if path.exists() {
                    self.templates.push(ExportTemplate {
                        platform: *platform,
                        path: path.clone(),
                        version: "local".to_string(),
                        is_server: false,
                    });
                }
            }
        }

        // Check server/ directory
        let server_dir = self.dist_dir.join("server");
        if server_dir.exists() {
            for platform in Platform::ALL {
                if let Some(name) = platform.server_binary_name_in_dir() {
                    let path = server_dir.join(name);
                    if path.exists() {
                        self.templates.push(ExportTemplate {
                            platform: *platform,
                            path,
                            version: "local".to_string(),
                            is_server: true,
                        });
                    }
                }
            }
        }
    }

    /// Get the runtime plugins directory for a platform.
    pub fn runtime_plugins_dir(&self) -> PathBuf {
        self.dist_dir.join("runtime").join("plugins")
    }

    /// Get the runtime shared libs directory for a platform.
    pub fn runtime_dir(&self) -> PathBuf {
        self.dist_dir.join("runtime")
    }

    /// Check if a template is available for the given platform.
    pub fn get(&self, platform: Platform) -> Option<&ExportTemplate> {
        self.templates.iter().find(|t| t.platform == platform && !t.is_server)
    }

    /// Check if a server template is available for the given platform.
    pub fn get_server(&self, platform: Platform) -> Option<&ExportTemplate> {
        self.templates.iter().find(|t| t.platform == platform && t.is_server)
    }

    /// Check if a template is installed for the given platform.
    pub fn is_installed(&self, platform: Platform) -> bool {
        self.get(platform).is_some()
    }

    /// Check if a server template is installed for the given platform.
    pub fn is_server_installed(&self, platform: Platform) -> bool {
        self.get_server(platform).is_some()
    }

}
