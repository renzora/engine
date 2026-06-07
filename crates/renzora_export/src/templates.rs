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

    /// The `dist/<name>/` directory `build-all.sh` writes this platform's
    /// output to (the renzora CLI builds straight into `dist/<name>/`).
    pub fn dist_dir_name(&self) -> &'static str {
        match self {
            Platform::WindowsX64 => "windows-x64",
            Platform::LinuxX64 => "linux-x64",
            Platform::MacOSX64 => "macos-x64",
            Platform::MacOSArm64 => "macos-arm64",
            Platform::AndroidArm64 => "android-arm64",
            Platform::AndroidX86_64 => "android-x86",
            Platform::FireTVArm64 => "firetv-arm64",
            Platform::IOSArm64 => "ios-arm64",
            Platform::TvOSArm64 => "tvos-arm64",
            Platform::WebWasm32 => "web-wasm32",
        }
    }

    /// True for the desktop platforms, whose game template is just the already-
    /// built `renzora`/`renzora.exe` binary sitting in `dist/<name>/`.
    pub fn is_desktop(&self) -> bool {
        matches!(
            self,
            Platform::WindowsX64
                | Platform::LinuxX64
                | Platform::MacOSX64
                | Platform::MacOSArm64
        )
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

/// Locates the already-built game binary for each platform under `dist/`.
///
/// Operation Merge: the editor's own binary IS the game (remove the editor
/// bundle dll and it runs as the game), so there is no download / separate
/// runtime template — export just copies the binary that's already in
/// `dist/<platform>/`. The dedicated server reuses the same binary (launched
/// with `--server`).
#[derive(Resource)]
pub struct TemplateManager {
    /// The `dist/` root — parent of the per-platform output dirs.
    pub dist_dir: PathBuf,
    pub templates: Vec<ExportTemplate>,
}

impl Default for TemplateManager {
    fn default() -> Self {
        // The editor runs from dist/<platform>/renzora.exe (one flat folder, no
        // editor/ subdir). The dist root is two levels up.
        let dist_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf())) // dist/<platform>/
            .and_then(|p| p.parent().map(|p| p.to_path_buf())) // dist/
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
    /// Scan `dist/<platform>/` for an already-built game binary per platform.
    pub fn scan(&mut self) {
        self.templates.clear();

        for platform in Platform::ALL {
            let pdir = self.dist_dir.join(platform.dist_dir_name());
            // Desktop: the single `renzora`/`renzora.exe` binary IS the game
            // template. Mobile/web: their lane's packaged artifact (apk/zip).
            let path = if platform.is_desktop() {
                pdir.join(platform.binary_name("renzora"))
            } else {
                pdir.join("runtime").join(platform.template_filename())
            };
            if path.exists() {
                self.templates.push(ExportTemplate {
                    platform: *platform,
                    path,
                    version: "local".to_string(),
                });
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
