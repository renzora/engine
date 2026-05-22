//! Runtime template management — download/locate pre-built runtime binaries.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
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
            Platform::AndroidArm64 | Platform::AndroidX86_64 | Platform::FireTVArm64 => {
                format!("{}.apk", project_name)
            }
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

    /// Whether this platform can run a dedicated server. Desktop only — the
    /// server is the runtime binary launched with `--server`, so there's no
    /// separate template; mobile/web don't ship a headless server.
    pub fn supports_dedicated_server(&self) -> bool {
        matches!(
            self,
            Platform::WindowsX64
                | Platform::LinuxX64
                | Platform::MacOSX64
                | Platform::MacOSArm64
        )
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
        {
            return Some(Platform::WindowsX64);
        }
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            return Some(Platform::LinuxX64);
        }
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        {
            return Some(Platform::MacOSX64);
        }
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            return Some(Platform::MacOSArm64);
        }
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

/// Manages export templates from dist/{platform}/{target}/ directories.
///
/// One runtime template per platform; the dedicated server reuses it (launched
/// with `--server`), so there's no separate server template.
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
            .and_then(|p| p.parent().map(|p| p.to_path_buf())) // editor/
            .and_then(|p| p.parent().map(|p| p.to_path_buf())) // {platform}/
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
    /// Scan for templates in the runtime/ sibling directory.
    pub fn scan(&mut self) {
        self.templates.clear();

        // Check runtime/ directory. The dedicated server isn't a separate
        // build — it's the runtime binary launched with `--server` — so the
        // runtime template is all that's needed for both client and server.
        let runtime_dir = self.dist_dir.join("runtime");
        if runtime_dir.exists() {
            for platform in Platform::ALL {
                let path = runtime_dir.join(platform.runtime_binary_name());
                if path.exists() {
                    self.templates.push(ExportTemplate {
                        platform: *platform,
                        path,
                        version: "local".to_string(),
                    });
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
        self.templates.iter().find(|t| t.platform == platform)
    }

    /// Check if a template is installed for the given platform.
    pub fn is_installed(&self, platform: Platform) -> bool {
        self.get(platform).is_some()
    }
}
