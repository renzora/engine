//! Engine capabilities the lean export can strip to shrink the binary.
//!
//! Each capability maps to Bevy features (removed from the export copy's root
//! `Cargo.toml`) and/or `renzora_runtime` subsystem features (removed from the
//! copy's `renzora_runtime/Cargo.toml` `default`). The dev source is never
//! touched — only the disposable copy — so this is safe.
//!
//! Two kinds:
//! * **Safe-leaf** (Solari, gizmos, remote assets, optional codecs): Bevy
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
    /// Parent capability id, for the nested list in the export UI.
    ///
    /// A child is a strict subset of its parent, so the parent going off takes
    /// every child with it — see [`collect_disabled`]. That is not cosmetic:
    /// dropping bevy's 3D stack while leaving, say, `pbr_specular_textures`
    /// enabled would pull `bevy_pbr` straight back in.
    pub group: Option<&'static str>,
}

/// The capabilities offered for the lean export.
pub const CAPABILITIES: &[Capability] = &[
    // ── Safe-leaf Bevy features (default off = auto-stripped) ───────────────
    Capability {
        id: "solari",
        label: "Raytraced Lighting (Solari)",
        help: "Bevy Solari hardware ray-traced direct + indirect lighting (DI + GI). On only when the Solari plugin is used.",
        bevy_features: &["bevy_solari"],
        runtime_features: &["solari"],
        default_on: false,
        group: None,
    },
    // NOTE (2026-07 slim pass): the `meshlets`, `feathers`, `asset_pipeline`,
    // `extra_shader_langs` and `editor_helpers` capabilities were REMOVED from this
    // list. They existed to strip Bevy features the base build no longer enables at
    // all — meshlet/meshlet_processor, bevy_feathers, asset_processor/
    // compressed_image_saver, shader_format_glsl/spirv, and the camera-controller +
    // sysinfo_plugin set. See the root `Cargo.toml` bevy feature list for why each
    // went. A capability whose features aren't in the manifest is a no-op toggle
    // that still costs the user a decision, so it doesn't belong here.
    Capability {
        id: "remote_assets",
        label: "Remote asset loading (HTTP)",
        help: "Loading assets over http/https at runtime — pulls in the whole rustls/ring/ureq \
               TLS stack (several MB). Off for a game shipping local (rpak) assets.",
        bevy_features: &["http", "https"],
        runtime_features: &[],
        default_on: false,
        group: None,
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
        ],
        runtime_features: &[],
        default_on: false,
        group: None,
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
        group: None,
    },
    Capability {
        id: "image_extra",
        label: "Image-format decoders",
        help: "Every optional texture decoder — DDS, JPEG, WebP, basis-universal, EXR, HDR, GIF, \
               BMP, TGA. PNG (+ its zlib) and KTX2 are always kept (window icon / compressed \
               textures). Auto-enabled per the image files actually in the project, so a \
               textureless or single-format game drops the decoders it never uses.",
        // Kept in lockstep with the root manifest's image list: TIFF/PNM/QOI/FF/ICO
        // were dropped there (no `renzora_import` sniffer recognises them), so they
        // can't be stripped from an export that never enabled them.
        bevy_features: &[
            "dds", "jpeg", "webp", "basis-universal",
            "exr", "hdr", "gif", "bmp", "tga",
        ],
        runtime_features: &[],
        default_on: false,
        group: None,
    },
    // ── Structural subsystems (default on = kept; uncheck to strip) ─────────
    Capability {
        id: "atmosphere",
        label: "Atmosphere",
        help: "Physically-based sky scattering.",
        bevy_features: &[],
        runtime_features: &["atmosphere"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "environment_map",
        label: "Environment maps",
        help: "Image-based lighting from an HDRI or baked cubemap.",
        bevy_features: &[],
        runtime_features: &["environment_map"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "skybox",
        label: "Skybox",
        help: "Cubemap / procedural skybox background.",
        bevy_features: &[],
        runtime_features: &["skybox"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "clouds",
        label: "Clouds",
        help: "Volumetric cloud rendering.",
        bevy_features: &[],
        runtime_features: &["clouds"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "night_stars",
        label: "Night stars",
        help: "The star field for night skies.",
        bevy_features: &[],
        runtime_features: &["night_stars"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "localization",
        label: "Translation packs",
        help: "The twenty embedded `languages/*.toml` packs — about 2.4 MiB of TOML compiled straight into the binary. Off leaves every string at its English fallback, since `t()` returns the key's own text when no pack is loaded. Drop it for a single-language game.",
        bevy_features: &[],
        runtime_features: &["localization"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "particles",
        label: "Particles",
        help: "The GPU particle system (bevy_hanabi). ~5 MB — drop if your game has no particle effects.",
        bevy_features: &[],
        runtime_features: &["particles"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "bloom",
        label: "Bloom",
        help: "Bright-pass glow.",
        bevy_features: &[],
        runtime_features: &["bloom"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "ssao",
        label: "SSAO",
        help: "Screen-space ambient occlusion.",
        bevy_features: &[],
        runtime_features: &["ssao"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "ssr",
        label: "SSR",
        help: "Screen-space reflections.",
        bevy_features: &[],
        runtime_features: &["ssr"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "dof",
        label: "Depth of field",
        help: "Camera focus blur.",
        bevy_features: &[],
        runtime_features: &["dof"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "motion_blur",
        label: "Motion blur",
        help: "Per-object and camera motion blur.",
        bevy_features: &[],
        runtime_features: &["motion_blur"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "distance_fog",
        label: "Distance fog",
        help: "Depth-based fog.",
        bevy_features: &[],
        runtime_features: &["distance_fog"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "volumetric_fog",
        label: "Volumetric fog",
        help: "Light-scattering fog volumes.",
        bevy_features: &[],
        runtime_features: &["volumetric_fog"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "lens_distortion",
        label: "Lens distortion",
        help: "Barrel / chromatic lens warp.",
        bevy_features: &[],
        runtime_features: &["lens_distortion"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "auto_exposure",
        label: "Auto exposure",
        help: "Adaptive eye-adjustment exposure.",
        bevy_features: &[],
        runtime_features: &["auto_exposure"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "oit",
        label: "Order-independent transparency",
        help: "Correct blending for overlapping transparent surfaces.",
        bevy_features: &[],
        runtime_features: &["oit"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "antialiasing",
        label: "Anti-aliasing",
        help: "TAA / FXAA / SMAA. Off leaves MSAA only.",
        bevy_features: &[],
        runtime_features: &["antialiasing"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "lumen",
        label: "Lumen global illumination",
        help: "Software-traced GI with its own render graph and compute passes.",
        bevy_features: &[],
        runtime_features: &["lumen"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "cloth",
        label: "Cloth simulation",
        help: "Verlet cloth (bevy_silk).",
        bevy_features: &[],
        runtime_features: &["cloth"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "ragdoll",
        label: "Ragdolls",
        help: "Physics bodies per bone. Needs 3D physics.",
        bevy_features: &[],
        runtime_features: &["ragdoll"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "gaussian_splatting",
        label: "Gaussian splatting",
        help: "The .ply/.sog splat renderer — sizeable; drop unless a scene uses one.",
        bevy_features: &[],
        runtime_features: &["gaussian_splatting"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "light2d",
        label: "2D lighting",
        help: "The bevy_firefly 2D light and shadow renderer.",
        bevy_features: &[],
        runtime_features: &["light2d"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "text3d",
        label: "3D text",
        help: "World-space extruded text meshes.",
        bevy_features: &[],
        runtime_features: &["text3d"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "vignette",
        label: "Vignette",
        help: "Screen-edge darkening.",
        bevy_features: &[],
        runtime_features: &["vignette"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "forward_decal",
        label: "Forward decals",
        help: "Projected decals on forward-rendered surfaces.",
        bevy_features: &[],
        runtime_features: &["forward_decal"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "pool_water",
        label: "Pool water",
        help: "The animated pool-water material.",
        bevy_features: &[],
        runtime_features: &["pool_water"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "procedural_tree",
        label: "Procedural trees",
        help: "Runtime tree mesh generation.",
        bevy_features: &[],
        runtime_features: &["procedural_tree"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "sprite_anim",
        label: "2D sprite animation",
        help: "Named AnimatedSprite clips and their scripting API.",
        bevy_features: &[],
        runtime_features: &["sprite_anim"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "water",
        label: "Water",
        help: "The water surface subsystem.",
        bevy_features: &[],
        runtime_features: &["water"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "terrain",
        label: "Terrain",
        help: "The terrain subsystem.",
        bevy_features: &[],
        runtime_features: &["terrain"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "spline",
        label: "Splines",
        help: "The spline subsystem.",
        bevy_features: &[],
        runtime_features: &["spline"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "navmesh",
        label: "Navmesh pathfinding",
        help: "Navigation-mesh generation and pathfinding (polyanya/vleue).",
        bevy_features: &[],
        runtime_features: &["navmesh"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "tilemap",
        label: "2D tilemap",
        help: "The 2D tilemap runtime (chunked quad-mesh renderer). Drop for a \
               game with no tilemaps — e.g. a pure-3D game.",
        bevy_features: &[],
        runtime_features: &["tilemap"],
        default_on: true,
        group: None,
    },
    // Physics is two capabilities because avian ships as two crates, each with
    // its own parry. Turning both off strips `renzora_physics` outright — the
    // shared `physics` feature is enabled only by these, never listed in
    // `default` on its own.
    Capability {
        id: "physics_3d",
        label: "3D physics (rigid bodies & collisions)",
        help: "The avian3d rigid-body engine and parry3d (~4.5 MiB). Also powers water \
               buoyancy and navmesh collider-obstacles, which strip with it. Drop for a 2D \
               game, or one with no physics simulation.",
        bevy_features: &[],
        runtime_features: &["physics_3d"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "physics_2d",
        label: "2D physics (rigid bodies & collisions)",
        help: "The avian2d rigid-body engine and parry2d. A separate simulation from the 3D \
               one — sprites, Node2d entities and tilemap colliders route here. Drop for a \
               pure-3D game.",
        bevy_features: &[],
        runtime_features: &["physics_2d"],
        default_on: true,
        group: None,
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
        // would otherwise re-enable bevy_pbr on its own. (bevy_solari needs pbr too —
        // it's a separate cap but also stripped here.)
        bevy_features: &[
            // The explicit 3D-render features (we own the manifest now — no `3d` meta).
            "bevy_pbr",
            "bevy_mikktspace",
            "bevy_solari",
        ],
        runtime_features: &["render_3d"],
        default_on: true,
        group: None,
    },
    // ── 3D sub-features ────────────────────────────────────────────────────
    // Subsets of the pipeline above, separated so a scene of primitives and a
    // light stops compiling the parts it cannot be using. Each re-enables
    // `bevy_pbr` on its own, which is why the parent going off forces them off.
    Capability {
        id: "gltf",
        label: "glTF model loading",
        help: "The .gltf/.glb loader and its animation support. A scene built only from                engine primitives (cube, sphere, plane) never touches it.",
        bevy_features: &["bevy_gltf", "gltf_animation"],
        runtime_features: &[],
        default_on: true,
        group: Some("render_3d"),
    },
    Capability {
        id: "morph_targets",
        label: "Morph targets (blend shapes)",
        help: "Per-vertex blend-shape deformation and its animation sampling. Used by                face rigs and shape keys; nothing else needs it.",
        bevy_features: &["morph", "morph_animation"],
        runtime_features: &[],
        default_on: true,
        group: Some("render_3d"),
    },
    Capability {
        id: "pbr_textures",
        label: "Advanced PBR texture maps",
        help: "Transmission, multi-layer (clearcoat), anisotropy and specular texture                maps on StandardMaterial. The base PBR set — colour, normal, metallic,                roughness, emissive, occlusion — is unaffected.",
        bevy_features: &[
            "pbr_transmission_textures",
            "pbr_multi_layer_material_textures",
            "pbr_anisotropy_texture",
            "pbr_specular_textures",
        ],
        runtime_features: &[],
        default_on: true,
        group: Some("render_3d"),
    },
    Capability {
        id: "lighting_luts",
        label: "Lighting lookup tables",
        help: "Precomputed tables baked into the binary as data, not code: the blue-noise                texture, the DFG environment-BRDF table and the area-light LTC tables.                Dropping them costs quality in specular/area-light shading, not                correctness elsewhere.",
        bevy_features: &["bluenoise_texture", "dfg_lut", "area_light_luts"],
        runtime_features: &[],
        default_on: true,
        group: Some("render_3d"),
    },
    Capability {
        id: "render_2d",
        label: "2D sprites",
        help: "The sprite scene systems: texture binding, size persistence, and \
               sprite-sheet frame cropping/animation. Drop for a pure-3D game. \
               (Tilemaps are a separate capability.)",
        // No bevy features yet: bevy's `2d` meta can't be stripped while the
        // window-blit present path (renzora_runtime::viewport_stretch) draws the
        // offscreen viewport image with a bevy `Sprite` in every export — 3D
        // included. When that blit moves off bevy_sprite, add "2d" here.
        bevy_features: &[],
        runtime_features: &["render_2d"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "audio",
        label: "Audio",
        help: "The audio subsystem. Drop for a silent game.",
        bevy_features: &[],
        runtime_features: &["audio"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "animation",
        label: "Skeletal animation",
        help: "The skeletal/property animation subsystem.",
        bevy_features: &[],
        runtime_features: &["animation"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "blueprint",
        label: "Blueprints (visual scripting)",
        help: "The node-graph visual scripting runtime (~0.2 MiB). Auto-enabled when the \
               project contains .blueprint/.bp graphs.",
        bevy_features: &[],
        runtime_features: &["blueprint"],
        default_on: false,
        group: None,
    },
    Capability {
        id: "script_http",
        label: "Script HTTP (http_get / http_post)",
        help: "The script HTTP verbs — pull in the ureq + rustls/ring TLS stack (~1 MiB). \
               Auto-enabled when a script calls http_get/http_post.",
        bevy_features: &[],
        runtime_features: &["script_http"],
        default_on: false,
        group: None,
    },
    Capability {
        id: "game_ui",
        label: "Game UI",
        help: "The in-game UI subsystem.",
        bevy_features: &[],
        runtime_features: &["game_ui"],
        default_on: true,
        group: None,
    },
];

/// File extensions that imply a detectable capability is needed.
fn detection_extensions(id: &str) -> &'static [&'static str] {
    match id {
        // Mirrors `image_extra`'s `bevy_features` — an extension listed here that has
        // no decoder behind it would flip the capability on for nothing.
        "image_extra" => &[
            "dds", "jpg", "jpeg", "jpe", "webp", "basis",
            "exr", "hdr", "gif", "bmp", "tga",
        ],
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
                Some("lua")
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
        for f in [
            "terrain", "water", "spline", "particles",
            // former `sky` bundle
            "atmosphere", "environment_map", "skybox", "clouds", "night_stars",
            // former `postfx` bundle
            "bloom", "ssao", "ssr", "dof", "motion_blur", "distance_fog",
            "volumetric_fog", "lens_distortion", "auto_exposure", "oit", "antialiasing",
            // 3D-only extras that build on bevy_pbr
            "lumen", "cloth", "ragdoll", "gaussian_splatting", "text3d",
            "forward_decal", "pool_water", "procedural_tree",
        ] {
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
        if !is_on(state, c) {
            out.extend(pick(c).iter().map(|f| f.to_string()));
        }
    }
    out
}

/// Whether a capability is enabled, honouring its parent.
///
/// A child is a subset of its parent, so leaving one on while the parent is off
/// would strip most of a subsystem and then re-enable it through the remainder
/// — turning off 3D rendering but keeping "advanced PBR texture maps" pulls
/// `bevy_pbr` back in, and the build either grows again or fails outright. The
/// UI could enforce this, but the answer has to be right even when the state map
/// comes from a saved project or an older config that predates the child.
fn is_on(state: &HashMap<String, bool>, c: &Capability) -> bool {
    if let Some(parent) = c.group {
        if let Some(p) = CAPABILITIES.iter().find(|x| x.id == parent) {
            if !is_on(state, p) {
                return false;
            }
        }
    }
    state.get(c.id).copied().unwrap_or(c.default_on)
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
