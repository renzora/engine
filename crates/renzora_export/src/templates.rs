//! Runtime template management — download/locate pre-built runtime binaries.
//!
//! An **export template** is the game runtime for one platform: `renzora[.exe]`,
//! its `plugins/`, and nothing else (no editor). Exporting for the platform you
//! are sitting on can use the runtime that shipped beside the editor; exporting
//! for any *other* platform needs a template, and templates come from the release
//! that matches this engine's version (see [`crate::download`]).
//!
//! Templates are found in two places, in this order:
//!
//! 1. `dist/<platform>/` — a from-source checkout that ran `renzora build`.
//!    Always preferred: if you built it, you meant it.
//! 2. `~/.renzora/templates/<version>/<platform>/` — downloaded from the release
//!    matching [`renzora::version::ENGINE_VERSION`]. Scoped by version so an
//!    engine upgrade can never silently reuse the previous version's runtime,
//!    and outside the install dir so it survives a reinstall and works when the
//!    engine lives somewhere unwritable.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Supported export platforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Platform {
    WindowsX64,
    WindowsArm64,
    LinuxX64,
    LinuxArm64,
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
        Platform::WindowsArm64,
        Platform::LinuxX64,
        Platform::LinuxArm64,
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
            Platform::WindowsArm64 => "Windows (ARM64)",
            Platform::LinuxX64 => "Linux (x64)",
            Platform::LinuxArm64 => "Linux (ARM64)",
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
            Platform::WindowsX64 | Platform::WindowsArm64 => format!("{}.exe", project_name),
            Platform::LinuxX64 | Platform::LinuxArm64 => project_name.to_string(),
            Platform::MacOSX64 | Platform::MacOSArm64 => project_name.to_string(),
            Platform::AndroidArm64 | Platform::AndroidX86_64 | Platform::FireTVArm64 => {
                format!("{}.apk", project_name)
            }
            Platform::IOSArm64 | Platform::TvOSArm64 => format!("{}.ipa", project_name),
            Platform::WebWasm32 => format!("{}.wasm", project_name),
        }
    }

    /// Filename a downloaded runtime template is saved as for this platform.
    ///
    /// Desktop templates arrive as a zip that is *extracted*, so the binary
    /// inside keeps the name the build gave it (`renzora[.exe]`) — this name is
    /// only used for the single-file mobile/web templates that are saved as-is.
    pub fn runtime_binary_name(&self) -> &'static str {
        match self {
            Platform::WindowsX64 | Platform::WindowsArm64 => "renzora.exe",
            Platform::LinuxX64 | Platform::LinuxArm64 => "renzora",
            Platform::MacOSX64 | Platform::MacOSArm64 => "renzora",
            _ => self.template_filename(),
        }
    }

    /// The `dist/<name>/` directory `build-all.sh` writes this platform's
    /// output to (the renzora CLI builds straight into `dist/<name>/`).
    ///
    /// Doubles as the platform key in a release: assets are `<name>.zip` (the
    /// engine) and `renzora-runtime-<name>.zip` (the export template), and
    /// `manifest.json` keys its rows by this string. `scripts/package-release.sh`
    /// is the other half of that contract.
    pub fn dist_dir_name(&self) -> &'static str {
        match self {
            Platform::WindowsX64 => "windows-x64",
            Platform::WindowsArm64 => "windows-arm64",
            Platform::LinuxX64 => "linux-x64",
            Platform::LinuxArm64 => "linux-arm64",
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
                | Platform::WindowsArm64
                | Platform::LinuxX64
                | Platform::LinuxArm64
                | Platform::MacOSX64
                | Platform::MacOSArm64
        )
    }

    /// Name of this platform's export template on a GitHub release.
    ///
    /// Derived from [`Self::dist_dir_name`] rather than tabulated separately —
    /// the two used to be independent lists and drifted immediately: the
    /// downloader asked for `renzora-runtime-windows.zip` while everything else
    /// in the codebase called the platform `windows-x64`, so no download could
    /// ever have succeeded. Written by `scripts/package-release.sh`.
    pub fn release_asset_name(&self) -> String {
        format!("renzora-runtime-{}.zip", self.dist_dir_name())
    }

    pub fn template_filename(&self) -> &'static str {
        match self {
            Platform::WindowsX64 => "renzora-runtime-windows-x64.exe",
            Platform::WindowsArm64 => "renzora-runtime-windows-arm64.exe",
            Platform::LinuxX64 => "renzora-runtime-linux-x64",
            Platform::LinuxArm64 => "renzora-runtime-linux-arm64",
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
        self.is_desktop()
    }

    pub fn supported_devices(&self) -> &'static str {
        match self {
            Platform::WindowsX64 => "Desktop PCs, laptops, PCVR (SteamVR, Oculus Link)",
            Platform::WindowsArm64 => "Snapdragon X / ARM64 Windows laptops",
            Platform::LinuxX64 => "Desktop PCs, laptops, Steam Deck",
            Platform::LinuxArm64 => "ARM64 Linux desktops, Raspberry Pi 5, Jetson",
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
        #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
        {
            return Some(Platform::WindowsArm64);
        }
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            return Some(Platform::LinuxX64);
        }
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        {
            return Some(Platform::LinuxArm64);
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
    /// Where it came from: `"local"` for a `dist/` build, otherwise the release
    /// tag it was downloaded from (`r1-alpha7-nightly-16aug26`).
    pub version: String,
}

impl ExportTemplate {
    /// The directory the runtime binary sits in — the export copies its sibling
    /// shared libraries and `plugins/` from here.
    pub fn dir(&self) -> PathBuf {
        self.path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// `true` when this template was downloaded rather than built locally.
    pub fn is_downloaded(&self) -> bool {
        self.version != "local"
    }
}

/// Root of the per-user template store: `~/.renzora/templates/<version>/`.
///
/// Version-scoped on purpose. Templates are the *runtime half* of the engine, so
/// a `r1-alpha7` editor pairing with a `r1-alpha6` runtime is an ABI mismatch
/// that surfaces as a game which won't load the scene the editor just saved —
/// keeping each version's downloads in its own directory makes that impossible
/// rather than merely unlikely.
///
/// Resolves the home dir from `HOME`, falling back to Windows' `USERPROFILE`,
/// mirroring `renzora::core::project_config`.
pub fn user_templates_root() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)?;
    Some(
        home.join(".renzora")
            .join("templates")
            .join(renzora::version::ENGINE_VERSION),
    )
}

/// Where a downloaded template for `platform` is installed.
pub fn user_template_dir(platform: Platform) -> Option<PathBuf> {
    Some(user_templates_root()?.join(platform.dist_dir_name()))
}

/// Where a downloaded engine source checkout is installed.
///
/// Version-scoped like the templates, and for the same reason: a lean build
/// recompiles the engine, so the source has to be the source this editor was
/// built from. Two engine versions on one machine keep separate trees rather
/// than one overwriting the other.
///
/// This exists so a canonical editor download — which ships binaries and no
/// source — can still do a lean export. Someone with a checkout never touches
/// it; `find_engine_source` prefers the checkout the editor is running from.
pub fn user_source_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)?;
    Some(home.join(".renzora").join("src").join(renzora::version::ENGINE_VERSION))
}

/// The release asset holding the engine source, published beside the runtime
/// templates. Not platform-scoped — source is source.
pub const SOURCE_ASSET: &str = "engine-source.zip";

/// Sidecar written beside a downloaded template recording which release it came
/// from, so the UI can say "r1-alpha7-nightly-16aug26" rather than "installed".
pub const TEMPLATE_STAMP: &str = "template.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateStamp {
    /// The release tag the template was downloaded from.
    pub tag: String,
    /// SHA-256 of the downloaded zip, as published in the release manifest.
    #[serde(default)]
    pub sha256: String,
}

/// Locates the game runtime for each platform — from a local `dist/` build first,
/// then from the per-user download store.
///
/// The editor's own runtime covers exporting for the platform you are running
/// on; every other platform needs a template that was either cross-built here
/// (`renzora build <platform>`) or downloaded from this version's release.
#[derive(Resource)]
pub struct TemplateManager {
    /// The `dist/` root — parent of the per-platform output dirs.
    pub dist_dir: PathBuf,
    /// Root of the per-user download store, or `None` when there is no home
    /// directory to resolve. A field rather than a call so tests can point it at
    /// a temp dir instead of picking up whatever the developer has installed.
    pub user_dir: Option<PathBuf>,
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
            user_dir: user_templates_root(),
            templates: Vec::new(),
        };
        mgr.scan();
        mgr
    }
}

/// Find a `*<suffix>` bundle dir directly under `pdir` and join `inner` onto it
/// (e.g. the `renzora` binary inside a `.app` / `.AppDir`). Returns a path that
/// won't exist when there's no such bundle, so the caller's `.exists()` skips it.
fn bundle_inner(pdir: &std::path::Path, suffix: &str, inner: &[&str]) -> PathBuf {
    let bundle = std::fs::read_dir(pdir).ok().and_then(|rd| {
        rd.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.ends_with(suffix))
                .unwrap_or(false)
        })
    });
    match bundle {
        Some(b) => inner.iter().fold(b, |acc, c| acc.join(c)),
        None => pdir.join(format!("__missing{suffix}")),
    }
}

/// Look for a downloaded template for `platform` under
/// `~/.renzora/templates/<version>/<platform>/`.
///
/// Desktop templates are extracted trees, so we look for the runtime binary
/// inside; mobile/web templates are the single downloaded file, kept under the
/// name the release published it as.
fn scan_user_template(root: Option<&Path>, platform: Platform) -> Option<ExportTemplate> {
    let dir = root?.join(platform.dist_dir_name());
    let path = if platform.is_desktop() {
        dir.join(platform.runtime_binary_name())
    } else {
        dir.join(platform.template_filename())
    };
    if !path.exists() {
        return None;
    }
    Some(ExportTemplate {
        platform,
        path,
        version: read_stamp(&dir)
            .map(|s| s.tag)
            // A template with no readable stamp still works — it is just a tree
            // of files — so report it rather than hiding it. "downloaded" is
            // honest about what we know.
            .unwrap_or_else(|| "downloaded".to_string()),
    })
}

/// Read the `template.json` the downloader wrote beside an installed template.
pub fn read_stamp(dir: &Path) -> Option<TemplateStamp> {
    let raw = std::fs::read_to_string(dir.join(TEMPLATE_STAMP)).ok()?;
    serde_json::from_str(&raw).ok()
}

impl TemplateManager {
    /// Scan `dist/<platform>/` for an already-built game binary per platform.
    ///
    /// `build-all.sh` nests each platform's runtime differently, so we resolve
    /// to where the file actually lives — not a uniform flat path:
    /// * Windows — flat `dist/windows-x64/renzora.exe`.
    /// * macOS — the editor is wrapped in a `.app`, so the binary is at
    ///   `dist/macos-*/<name>.app/Contents/MacOS/renzora`.
    /// * Linux — wrapped in the AppImage's `.AppDir`, so the binary is at
    ///   `dist/linux-x64/<name>.AppDir/renzora`.
    /// * Mobile/web — the lane drops its artifact flat in `dist/<platform>/`.
    ///
    /// A downloaded template (`~/.renzora/templates/<version>/<platform>/`) is
    /// always flat — `scripts/package-release.sh` unwraps the bundle when it
    /// builds the asset, precisely so the install side needs no layout knowledge.
    /// A local `dist/` build wins over a download for the same platform: if you
    /// built it, you meant to use it.
    pub fn scan(&mut self) {
        self.templates.clear();

        for platform in Platform::ALL {
            let pdir = self.dist_dir.join(platform.dist_dir_name());
            let local = match platform {
                Platform::WindowsX64 | Platform::WindowsArm64 => {
                    pdir.join(platform.binary_name("renzora"))
                }
                Platform::LinuxX64 | Platform::LinuxArm64 => {
                    bundle_inner(&pdir, ".AppDir", &["renzora"])
                }
                Platform::MacOSX64 | Platform::MacOSArm64 => {
                    bundle_inner(&pdir, ".app", &["Contents", "MacOS", "renzora"])
                }
                _ => pdir.join(platform.template_filename()),
            };
            if local.exists() {
                self.templates.push(ExportTemplate {
                    platform: *platform,
                    path: local,
                    version: "local".to_string(),
                });
                continue;
            }
            if let Some(t) = scan_user_template(self.user_dir.as_deref(), *platform) {
                self.templates.push(t);
            }
        }
    }

    /// The distribution-plugin directory the editor is running from
    /// (`dist/<platform>/plugins`). The editor and the game it exports share one
    /// flat per-platform folder — the old `dist/runtime/plugins` lane was
    /// flattened away, so deriving this from the live exe keeps the export's
    /// plugin scan pointed at the dlls that actually exist (otherwise the export
    /// ships zero plugins and the game drops every effect's components).
    pub fn runtime_plugins_dir(&self) -> PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("plugins")))
            .unwrap_or_else(|| self.dist_dir.join("plugins"))
    }

    /// The plugin directory to ship from when exporting for `platform`.
    ///
    /// Cross-platform export made this a real question: the editor's own
    /// `plugins/` holds *host* libraries, so a Windows editor exporting a Linux
    /// game would copy `.dll`s into it — plugins the game cannot load, and the
    /// failure is silent because the host just finds nothing it can open. A
    /// template that brought its own `plugins/` is authoritative for its
    /// platform; without one we fall back to the editor's, which is correct for
    /// the same-platform case and no worse than before for any other.
    pub fn plugins_dir_for(&self, platform: Platform) -> PathBuf {
        if let Some(t) = self.get(platform) {
            let dir = t.dir().join("plugins");
            if dir.is_dir() {
                return dir;
            }
        }
        self.runtime_plugins_dir()
    }

    /// The shared-lib directory the editor is running from (`dist/<platform>/`).
    pub fn runtime_dir(&self) -> PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| self.dist_dir.clone())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Unique-per-test temp dir, recreated empty on each run.
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "renzora_export_templates_{}_{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn binary_name_appends_platform_extension() {
        assert_eq!(Platform::WindowsX64.binary_name("MyGame"), "MyGame.exe");
        assert_eq!(Platform::LinuxX64.binary_name("MyGame"), "MyGame");
        assert_eq!(Platform::MacOSX64.binary_name("MyGame"), "MyGame");
        assert_eq!(Platform::MacOSArm64.binary_name("MyGame"), "MyGame");
        assert_eq!(Platform::AndroidArm64.binary_name("MyGame"), "MyGame.apk");
        assert_eq!(Platform::AndroidX86_64.binary_name("MyGame"), "MyGame.apk");
        assert_eq!(Platform::FireTVArm64.binary_name("MyGame"), "MyGame.apk");
        assert_eq!(Platform::IOSArm64.binary_name("MyGame"), "MyGame.ipa");
        assert_eq!(Platform::TvOSArm64.binary_name("MyGame"), "MyGame.ipa");
        assert_eq!(Platform::WebWasm32.binary_name("MyGame"), "MyGame.wasm");
    }

    #[test]
    fn desktop_platforms_match_dedicated_server_support() {
        // The dedicated server reuses the desktop game binary, so the two
        // predicates must describe the same platform set.
        for &p in Platform::ALL {
            assert_eq!(p.is_desktop(), p.supports_dedicated_server(), "{p:?}");
        }
        let desktops = Platform::ALL.iter().filter(|p| p.is_desktop()).count();
        assert_eq!(desktops, 6);
    }

    #[test]
    fn dist_dir_names_are_unique() {
        let names: std::collections::HashSet<&str> =
            Platform::ALL.iter().map(|p| p.dist_dir_name()).collect();
        assert_eq!(names.len(), Platform::ALL.len());
    }

    #[test]
    fn template_filenames_are_unique_runtime_artifacts() {
        let names: std::collections::HashSet<&str> =
            Platform::ALL.iter().map(|p| p.template_filename()).collect();
        assert_eq!(names.len(), Platform::ALL.len());
        for &p in Platform::ALL {
            assert!(p.template_filename().starts_with("renzora-runtime-"));
        }
    }

    #[test]
    fn runtime_binary_name_per_platform_kind() {
        // Desktop templates are extracted trees, so the binary inside keeps the
        // name the build gave it — which is also what the export copies.
        assert_eq!(Platform::WindowsX64.runtime_binary_name(), "renzora.exe");
        assert_eq!(Platform::WindowsArm64.runtime_binary_name(), "renzora.exe");
        assert_eq!(Platform::LinuxX64.runtime_binary_name(), "renzora");
        assert_eq!(Platform::LinuxArm64.runtime_binary_name(), "renzora");
        assert_eq!(Platform::MacOSX64.runtime_binary_name(), "renzora");
        assert_eq!(Platform::MacOSArm64.runtime_binary_name(), "renzora");
        // Non-desktop platforms install the release artifact as-is.
        for &p in Platform::ALL {
            if !p.is_desktop() {
                assert_eq!(p.runtime_binary_name(), p.template_filename(), "{p:?}");
            }
        }
    }

    #[test]
    fn platform_serde_roundtrips_with_apple_renames() {
        assert_eq!(
            serde_json::to_string(&Platform::IOSArm64).unwrap(),
            "\"ios_arm64\""
        );
        assert_eq!(
            serde_json::to_string(&Platform::TvOSArm64).unwrap(),
            "\"tvos_arm64\""
        );
        for &p in Platform::ALL {
            let json = serde_json::to_string(&p).unwrap();
            let back: Platform = serde_json::from_str(&json).unwrap();
            assert_eq!(back, p);
        }
    }

    #[test]
    fn scan_locates_artifacts_per_platform_layout() {
        let dist = temp_dir("scan_layout");
        // Windows: flat exe at dist/windows-x64/renzora.exe.
        let win_dir = dist.join(Platform::WindowsX64.dist_dir_name());
        fs::create_dir_all(&win_dir).unwrap();
        fs::write(win_dir.join("renzora.exe"), b"bin").unwrap();
        // macOS: binary INSIDE the .app bundle.
        let mac_macos = dist
            .join(Platform::MacOSArm64.dist_dir_name())
            .join("Renzora Engine.app")
            .join("Contents")
            .join("MacOS");
        fs::create_dir_all(&mac_macos).unwrap();
        fs::write(mac_macos.join("renzora"), b"bin").unwrap();
        // Linux: binary INSIDE the AppImage's AppDir.
        let lin_appdir = dist
            .join(Platform::LinuxX64.dist_dir_name())
            .join("Renzora Engine.AppDir");
        fs::create_dir_all(&lin_appdir).unwrap();
        fs::write(lin_appdir.join("renzora"), b"bin").unwrap();
        // Mobile: packaged artifact FLAT in dist/<platform>/ (no runtime/ subdir).
        let apk_dir = dist.join(Platform::AndroidArm64.dist_dir_name());
        fs::create_dir_all(&apk_dir).unwrap();
        fs::write(apk_dir.join(Platform::AndroidArm64.template_filename()), b"apk").unwrap();

        let mut mgr = TemplateManager {
            dist_dir: dist.clone(),
            user_dir: None,
            templates: Vec::new(),
        };
        mgr.scan();

        assert!(mgr.is_installed(Platform::WindowsX64));
        assert!(mgr.is_installed(Platform::MacOSArm64));
        assert!(mgr.is_installed(Platform::LinuxX64));
        assert!(mgr.is_installed(Platform::AndroidArm64));
        assert_eq!(mgr.templates.len(), 4);

        // The macOS template resolves to the binary inside the .app bundle.
        let t = mgr.get(Platform::MacOSArm64).unwrap();
        assert_eq!(t.path, mac_macos.join("renzora"));
        assert_eq!(t.version, "local");
        assert!(mgr.get(Platform::WebWasm32).is_none());

        fs::remove_dir_all(&dist).unwrap();
    }

    #[test]
    fn rescan_is_idempotent_and_drops_stale_entries() {
        let dist = temp_dir("scan_stale");
        let win_dir = dist.join(Platform::WindowsX64.dist_dir_name());
        fs::create_dir_all(&win_dir).unwrap();
        let bin = win_dir.join("renzora.exe");
        fs::write(&bin, b"bin").unwrap();

        let mut mgr = TemplateManager {
            dist_dir: dist.clone(),
            user_dir: None,
            templates: Vec::new(),
        };
        mgr.scan();
        mgr.scan();
        assert_eq!(mgr.templates.len(), 1);

        fs::remove_file(&bin).unwrap();
        mgr.scan();
        assert!(mgr.templates.is_empty());

        fs::remove_dir_all(&dist).unwrap();
    }

    /// A downloaded template is a FLAT tree — the packaging script unwraps the
    /// AppImage/.app bundle so the install side needs no layout knowledge.
    #[test]
    fn scan_finds_downloaded_templates_in_the_user_store() {
        let dist = temp_dir("scan_user_dist");
        let user = temp_dir("scan_user_store");

        let lin = user.join(Platform::LinuxX64.dist_dir_name());
        fs::create_dir_all(lin.join("plugins")).unwrap();
        fs::write(lin.join("renzora"), b"bin").unwrap();
        fs::write(lin.join("plugins").join("liblua.so"), b"so").unwrap();
        fs::write(
            lin.join(TEMPLATE_STAMP),
            br#"{"tag":"r1-alpha7-nightly-16aug26","sha256":"abc"}"#,
        )
        .unwrap();

        let mut mgr = TemplateManager {
            dist_dir: dist.clone(),
            user_dir: Some(user.clone()),
            templates: Vec::new(),
        };
        mgr.scan();

        let t = mgr.get(Platform::LinuxX64).expect("downloaded template");
        assert_eq!(t.path, lin.join("renzora"));
        assert_eq!(t.version, "r1-alpha7-nightly-16aug26");
        assert!(t.is_downloaded());
        // Its own plugins win over the editor's — the whole point of shipping
        // `plugins/` inside the template.
        assert_eq!(mgr.plugins_dir_for(Platform::LinuxX64), lin.join("plugins"));

        fs::remove_dir_all(&dist).unwrap();
        fs::remove_dir_all(&user).unwrap();
    }

    /// If you built it locally, you meant to use it.
    #[test]
    fn a_local_build_wins_over_a_download() {
        let dist = temp_dir("precedence_dist");
        let user = temp_dir("precedence_user");

        let win_dist = dist.join(Platform::WindowsX64.dist_dir_name());
        fs::create_dir_all(&win_dist).unwrap();
        fs::write(win_dist.join("renzora.exe"), b"local").unwrap();

        let win_user = user.join(Platform::WindowsX64.dist_dir_name());
        fs::create_dir_all(&win_user).unwrap();
        fs::write(win_user.join("renzora.exe"), b"downloaded").unwrap();

        let mut mgr = TemplateManager {
            dist_dir: dist.clone(),
            user_dir: Some(user.clone()),
            templates: Vec::new(),
        };
        mgr.scan();

        let t = mgr.get(Platform::WindowsX64).unwrap();
        assert_eq!(t.path, win_dist.join("renzora.exe"));
        assert_eq!(t.version, "local");
        // Exactly one entry per platform — never both sources at once.
        assert_eq!(mgr.templates.len(), 1);

        fs::remove_dir_all(&dist).unwrap();
        fs::remove_dir_all(&user).unwrap();
    }

    /// A template installed by hand (no `template.json`) is still usable; we
    /// just can't name the release it came from.
    #[test]
    fn a_stampless_template_still_registers() {
        let dist = temp_dir("stampless_dist");
        let user = temp_dir("stampless_user");
        let mac = user.join(Platform::MacOSArm64.dist_dir_name());
        fs::create_dir_all(&mac).unwrap();
        fs::write(mac.join("renzora"), b"bin").unwrap();

        let mut mgr = TemplateManager {
            dist_dir: dist.clone(),
            user_dir: Some(user.clone()),
            templates: Vec::new(),
        };
        mgr.scan();
        assert_eq!(mgr.get(Platform::MacOSArm64).unwrap().version, "downloaded");

        fs::remove_dir_all(&dist).unwrap();
        fs::remove_dir_all(&user).unwrap();
    }

    #[test]
    fn user_store_is_scoped_to_this_engine_version() {
        // A r1-alpha7 editor must never see a r1-alpha6 download — same runtime
        // filename, different ABI.
        if let Some(root) = user_templates_root() {
            assert!(root.ends_with(renzora::version::ENGINE_VERSION));
        }
    }

    #[test]
    fn runtime_dirs_derive_from_current_exe() {
        let mgr = TemplateManager {
            dist_dir: PathBuf::from("unused-fallback"),
            user_dir: None,
            templates: Vec::new(),
        };
        let exe_dir = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        assert_eq!(mgr.runtime_dir(), exe_dir);
        assert_eq!(mgr.runtime_plugins_dir(), exe_dir.join("plugins"));
    }
}
