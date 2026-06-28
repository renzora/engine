//! Engine capabilities the lean export can strip to shrink the binary.
//!
//! Each capability maps to Bevy features (removed from the export copy's root
//! `Cargo.toml`) and/or `renzora_runtime` subsystem features (removed from the
//! copy's `renzora_runtime/Cargo.toml` `default`). The dev source is never
//! touched — only the disposable copy — so this is safe.
//!
//! Two kinds:
//! * **Safe-leaf** (Solari, meshlets, BRP, Feathers, uncommon codecs): Bevy
//!   features no core crate hard-depends on. Default OFF (auto-stripped), since
//!   they're confidently unneeded.
//! * **Structural subsystems** (audio, navmesh, networking, post-FX, sky, …):
//!   `renzora_runtime` features, made optional in Wave 2. Default ON (kept) — a
//!   game might use them via scripts the scan can't see, so the dev unchecks the
//!   ones they know are unused rather than risk auto-stripping something needed.

use std::collections::HashMap;
use std::path::Path;

/// A toggleable engine capability shown in the export UI.
pub struct Capability {
    pub id: &'static str,
    pub label: &'static str,
    pub help: &'static str,
    /// Bevy features removed from the export copy's root manifest when OFF.
    pub bevy_features: &'static [&'static str],
    /// `renzora_runtime` `default` features removed from the copy when OFF.
    pub runtime_features: &'static [&'static str],
    /// Default state when no plugin/asset detection overrides it.
    pub default_on: bool,
}

/// The capabilities offered for the lean export.
pub const CAPABILITIES: &[Capability] = &[
    // ── Safe-leaf Bevy features (default off = auto-stripped) ───────────────
    Capability {
        id: "solari",
        label: "Raytraced GI (Solari)",
        help: "Bevy Solari hardware ray-traced lighting. On only when the Solari plugin is used.",
        bevy_features: &["bevy_solari"],
        runtime_features: &["solari"],
        default_on: false,
    },
    Capability {
        id: "meshlets",
        label: "Virtual geometry (meshlets)",
        help: "Experimental meshlet rendering + its metis-based processor. Rarely used; large.",
        bevy_features: &["meshlet", "meshlet_processor"],
        runtime_features: &[],
        default_on: false,
    },
    Capability {
        id: "remote",
        label: "Remote debugging protocol",
        help: "Bevy Remote Protocol server. Not needed in a shipped game.",
        bevy_features: &["bevy_remote"],
        runtime_features: &[],
        default_on: false,
    },
    Capability {
        id: "feathers",
        label: "Bevy Feathers widgets",
        help: "Bevy's widget toolkit — unused by Renzora's own UI.",
        bevy_features: &["bevy_feathers"],
        runtime_features: &[],
        default_on: false,
    },
    Capability {
        id: "remote_assets",
        label: "Remote asset loading (HTTP)",
        help: "Loading assets over http/https at runtime — pulls in the whole rustls/ring/ureq \
               TLS stack (several MB). Off for a game shipping local (rpak) assets.",
        bevy_features: &["http", "https", "web_asset_cache"],
        runtime_features: &[],
        default_on: false,
    },
    Capability {
        id: "asset_pipeline",
        label: "Asset processor / compressed-image saver",
        help: "Offline asset processing and compressed-image saving — editor/dev tools, not \
               needed in a shipped game.",
        bevy_features: &["asset_processor", "compressed_image_saver"],
        runtime_features: &[],
        default_on: false,
    },
    Capability {
        id: "extra_shader_langs",
        label: "Extra shader languages (GLSL/SPIR-V)",
        help: "GLSL and SPIR-V shader support. Renzora's shaders are WGSL/WESL, so these are unused.",
        bevy_features: &["shader_format_glsl", "shader_format_spirv"],
        runtime_features: &[],
        default_on: false,
    },
    Capability {
        id: "editor_helpers",
        label: "Editor camera & diagnostics",
        help: "Free/pan camera controllers and system-info diagnostics — editor/dev only.",
        bevy_features: &["bevy_camera_controller", "free_camera", "pan_camera", "sysinfo_plugin"],
        runtime_features: &[],
        default_on: false,
    },
    Capability {
        id: "dev_extras",
        label: "Editor/dev conveniences",
        help: "Hot-reload file watching, reflection doc-strings (inspector tooltips), clipboard \
               access, OS font discovery, and bevy's settings system — all editor/dev only, with \
               zero usage in a shipped game. (Clipboard's `arboard` backend is pulled separately \
               by the engine and needs its own gate for the full saving.)",
        bevy_features: &[
            "file_watcher",
            "reflect_documentation",
            "system_clipboard",
            "clipboard_image",
            "system_font_discovery",
            "bevy_settings",
        ],
        runtime_features: &[],
        default_on: false,
    },
    Capability {
        id: "gizmos",
        label: "Debug gizmos (immediate-mode draw)",
        help: "bevy_gizmos + bevy_gizmos_render — immediate-mode debug-line drawing (~1.3 MiB). \
               Editor/debug only; a shipped game rarely uses it. (Now strippable because we own \
               the explicit bevy manifest — it used to be welded into the 2d/3d metas.) The \
               editor's transform gizmo is a separate crate, renzora_gizmo, and is unaffected.",
        bevy_features: &["bevy_gizmos", "bevy_gizmos_render"],
        runtime_features: &[],
        default_on: false,
    },
    Capability {
        id: "image_extra",
        label: "Image-format decoders",
        help: "Every optional texture decoder — DDS, JPEG, WebP, basis-universal, EXR/HDR, TIFF, \
               GIF, BMP, TGA, PNM, QOI. PNG (+ its zlib) and KTX2 are always kept (window icon / \
               compressed textures). Auto-enabled per the image files actually in the project, so \
               a textureless or single-format game drops the decoders it never uses.",
        bevy_features: &[
            "dds", "jpeg", "webp", "basis-universal",
            "exr", "tiff", "gif", "bmp", "tga", "pnm", "qoi", "ff", "ico",
        ],
        runtime_features: &[],
        default_on: false,
    },
    // ── Structural subsystems (default on = kept; uncheck to strip) ─────────
    Capability {
        id: "sky",
        label: "Sky & atmosphere",
        help: "Atmosphere, skybox, clouds, night stars, environment maps. Drop for a 2D game.",
        bevy_features: &[],
        runtime_features: &["sky"],
        default_on: true,
    },
    Capability {
        id: "particles",
        label: "Particles",
        help: "The GPU particle system (bevy_hanabi). ~5 MB — drop if your game has no particle effects.",
        bevy_features: &[],
        runtime_features: &["particles"],
        default_on: true,
    },
    Capability {
        id: "postfx",
        label: "Post-process effects",
        help: "Bloom, SSAO, SSR, DoF, motion blur, fog, OIT, etc. (Tonemapping stays.)",
        bevy_features: &[],
        runtime_features: &["postfx"],
        default_on: true,
    },
    Capability {
        id: "water",
        label: "Water",
        help: "The water surface subsystem.",
        bevy_features: &[],
        runtime_features: &["water"],
        default_on: true,
    },
    Capability {
        id: "terrain",
        label: "Terrain",
        help: "The terrain subsystem.",
        bevy_features: &[],
        runtime_features: &["terrain"],
        default_on: true,
    },
    Capability {
        id: "spline",
        label: "Splines",
        help: "The spline subsystem.",
        bevy_features: &[],
        runtime_features: &["spline"],
        default_on: true,
    },
    Capability {
        id: "navmesh",
        label: "Navmesh pathfinding",
        help: "Navigation-mesh generation and pathfinding (polyanya/vleue).",
        bevy_features: &[],
        runtime_features: &["navmesh"],
        default_on: true,
    },
    Capability {
        id: "physics",
        label: "Physics (rigid bodies & collisions)",
        help: "The avian rigid-body physics engine (~6.5 MiB). Also powers water buoyancy \
               and navmesh collider-obstacles, which strip with it. Drop for a game with no \
               physics simulation.",
        bevy_features: &[],
        runtime_features: &["physics"],
        default_on: true,
    },
    Capability {
        id: "render_3d",
        label: "3D rendering (PBR pipeline)",
        help: "The whole 3D pipeline: bevy_pbr (StandardMaterial, shadows, deferred/forward \
               renderer), glTF model loading, and the renzora_shader graph-material system \
               (~10 MiB). OFF = a 2D-only game (sprites + UI). Lights/atmosphere still work \
               (bevy_light is kept). Requires the 3D subsystems (Terrain/Water/Sky/Post-FX) \
               to also be off — they build on bevy_pbr.",
        // Dropping the `3d` meta also requires dropping every pbr_* sub-feature: each
        // would otherwise re-enable bevy_pbr on its own. (bevy_solari/meshlet need pbr
        // too — they're separate caps but also stripped here.)
        bevy_features: &[
            // The explicit 3D-render features (we own the manifest now — no `3d` meta).
            "bevy_pbr",
            "bevy_gltf",
            "gltf_animation",
            "bevy_mikktspace",
            "pbr_transmission_textures",
            "pbr_clustered_decals",
            "pbr_light_textures",
            "pbr_multi_layer_material_textures",
            "pbr_anisotropy_texture",
            "pbr_specular_textures",
            "experimental_pbr_pcss",
            "bluenoise_texture",
            "dfg_lut",
            "area_light_luts",
            "bevy_solari",
            "meshlet",
            "meshlet_processor",
            // 3D mesh morph targets (skinned/blend-shape) — irrelevant to 2D.
            "morph",
            "morph_animation",
        ],
        runtime_features: &["render_3d"],
        default_on: true,
    },
    Capability {
        id: "audio",
        label: "Audio",
        help: "The audio subsystem. Drop for a silent game.",
        bevy_features: &[],
        runtime_features: &["audio"],
        default_on: true,
    },
    Capability {
        id: "animation",
        label: "Skeletal animation",
        help: "The skeletal/property animation subsystem.",
        bevy_features: &[],
        runtime_features: &["animation"],
        default_on: true,
    },
    Capability {
        id: "blueprint",
        label: "Blueprints (visual scripting)",
        help: "The node-graph visual scripting runtime (~0.2 MiB). Auto-enabled when the \
               project contains .blueprint/.bp graphs.",
        bevy_features: &[],
        runtime_features: &["blueprint"],
        default_on: false,
    },
    Capability {
        id: "rhai",
        label: "Rhai scripting backend",
        help: "The Rhai script backend (~2.3 MiB). Lua is always included; Rhai is \
               auto-enabled only when the project contains .rhai scripts.",
        bevy_features: &[],
        runtime_features: &["rhai"],
        default_on: false,
    },
    Capability {
        id: "script_http",
        label: "Script HTTP (http_get / http_post)",
        help: "The script HTTP verbs — pull in the ureq + rustls/ring TLS stack (~1 MiB). \
               Auto-enabled when a script calls http_get/http_post.",
        bevy_features: &[],
        runtime_features: &["script_http"],
        default_on: false,
    },
    Capability {
        id: "game_ui",
        label: "Game UI",
        help: "The in-game UI subsystem.",
        bevy_features: &[],
        runtime_features: &["game_ui"],
        default_on: true,
    },
];

/// File extensions that imply a detectable capability is needed.
fn detection_extensions(id: &str) -> &'static [&'static str] {
    match id {
        "image_extra" => &[
            "dds", "jpg", "jpeg", "jpe", "webp", "basis",
            "exr", "hdr", "tif", "tiff", "gif", "bmp", "tga", "pnm", "qoi", "ico",
        ],
        // Rhai backend only when the project ships .rhai scripts (Lua is always in).
        "rhai" => &["rhai"],
        // Visual scripting only when the project ships blueprint graphs.
        "blueprint" => &["blueprint", "bp"],
        _ => &[],
    }
}

/// Default on/off per capability for a fresh export. Solari follows its plugin;
/// codecs follow the project's asset files; everything else uses `default_on`.
pub fn defaults(selected_plugins: &[String], project_root: Option<&Path>) -> HashMap<String, bool> {
    let used_exts = project_root.map(used_extensions).unwrap_or_default();
    let uses_http = project_root.map(project_uses_script_http).unwrap_or(false);
    CAPABILITIES
        .iter()
        .map(|c| {
            let on = match c.id {
                "solari" => selected_plugins.iter().any(|p| p == "renzora_solari"),
                // Content scan (not an extension): the http verbs in any script file.
                "script_http" => uses_http,
                _ if !detection_extensions(c.id).is_empty() => detection_extensions(c.id)
                    .iter()
                    .any(|e| used_exts.contains(*e)),
                _ => c.default_on,
            };
            (c.id.to_string(), on)
        })
        .collect()
}

/// Whether any `.lua`/`.rhai` script in the project calls `http_get`/`http_post`.
/// Drives the `script_http` capability so the TLS stack is only built for games
/// that actually make script HTTP requests.
fn project_uses_script_http(root: &Path) -> bool {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let dot = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with('.'));
                if !dot {
                    stack.push(path);
                }
            } else if matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("lua") | Some("rhai")
            ) {
                if let Ok(src) = std::fs::read_to_string(&path) {
                    if src.contains("http_get") || src.contains("http_post") {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Bevy features to strip from the export copy (union of OFF capabilities).
pub fn disabled_bevy_features(state: &HashMap<String, bool>) -> Vec<String> {
    collect_disabled(state, |c| c.bevy_features)
}

/// `renzora_runtime` `default` features to strip from the export copy.
///
/// Enforces the one hard dependency between capabilities: the 3D subsystems
/// (terrain/water/sky/post-FX/spline) build on bevy_pbr, so when `render_3d` is
/// off they MUST be stripped too — otherwise the 2D build fails to compile. We do
/// it here (not via a Cargo feature dep, which would force render_3d back ON).
pub fn disabled_runtime_features(state: &HashMap<String, bool>) -> Vec<String> {
    let mut out = collect_disabled(state, |c| c.runtime_features);
    let render_3d_on = state.get("render_3d").copied().unwrap_or(true);
    if !render_3d_on {
        // particles (bevy_hanabi) references bevy_pbr in its asset path — drop it
        // too in 2D (a dedicated 2D-particle path can re-add it later).
        for f in ["terrain", "water", "sky", "postfx", "spline", "particles"] {
            if !out.iter().any(|x| x == f) {
                out.push(f.to_string());
            }
        }
    }
    out
}

fn collect_disabled(
    state: &HashMap<String, bool>,
    pick: impl Fn(&Capability) -> &'static [&'static str],
) -> Vec<String> {
    let mut out = Vec::new();
    for c in CAPABILITIES {
        if !state.get(c.id).copied().unwrap_or(c.default_on) {
            out.extend(pick(c).iter().map(|f| f.to_string()));
        }
    }
    out
}

/// Lowercased file extensions present anywhere under `root` (skipping dot-dirs).
fn used_extensions(root: &Path) -> std::collections::HashSet<String> {
    let mut exts = std::collections::HashSet::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let dot = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with('.'));
                if !dot {
                    stack.push(path);
                }
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                exts.insert(ext.to_ascii_lowercase());
            }
        }
    }
    exts
}
