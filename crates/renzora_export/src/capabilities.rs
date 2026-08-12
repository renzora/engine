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
    /// Which section of the Features tab this appears under.
    ///
    /// Purely presentational — nothing in the strip logic reads it. Must be one
    /// of [`SECTIONS`]; a capability naming anything else would silently never
    /// render, which [`every_capability_has_a_known_section`] catches.
    pub section: &'static str,
}

/// The Features-tab sections, in display order, as `(id, heading)`.
///
/// Ordered so the two rendering pipelines sit next to each other and can be
/// compared at a glance — a game is usually one or the other, and the whole
/// point of the tab is deciding which half to drop.
pub const SECTIONS: &[(&str, &str)] = &[
    ("render_3d", "3D rendering"),
    ("render_2d", "2D rendering"),
    ("postfx", "Post-processing"),
    ("sky", "Sky & environment"),
    ("simulation", "Simulation"),
    ("systems", "Systems & gameplay"),
    ("ui", "Interface"),
    ("assets", "Assets"),
    ("build", "Build & diagnostics"),
];

/// The capabilities offered for the lean export.
pub const CAPABILITIES: &[Capability] = &[
    // ── Safe-leaf Bevy features (default off = auto-stripped) ───────────────
    Capability {
        id: "solari",
        section: "render_3d",
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
    //
    // `remote_assets` (bevy `http`/`https`) went the same way in the 2026-08
    // pass, and is worth a line because it looked load-bearing: it claimed to
    // strip "the whole rustls/ring/ureq TLS stack", and it did. But the engine
    // no longer HAS a TLS stack to strip — HTTP is `plugins/http` behind the
    // `renzora_net` boundary, and a game drops the whole thing by not shipping
    // that plugin. Bevy's URL asset source turned out to have no callers in the
    // engine at all, so the features it needed left the manifest and this
    // toggle went with them.
    Capability {
        id: "dev_extras",
        section: "build",
        label: "Editor/dev conveniences",
        help: "Hot-reload file watching, reflection doc-strings (inspector tooltips), clipboard \
               access, OS font discovery, and the native crash dialog — all editor/dev only, with \
               zero usage in a shipped game. Takes the `arboard` clipboard backend and `rfd` (a \
               full native file-dialog stack) with it.",
        bevy_features: &[
            "file_watcher",
            "reflect_documentation",
            // All three must go together, and `bevy_clipboard` is the one that
            // matters: the other two only switch on its `system_clipboard`
            // feature, i.e. the `arboard` backend. Dropping them while leaving
            // the crate itself named here would still compile bevy_clipboard.
            "system_clipboard",
            "clipboard_image",
            "bevy_clipboard",
            "system_font_discovery",
        ],
        // `rfd` + `arboard`, pulled directly by `renzora_engine` for the crash
        // message box (crash.rs). Its call site is already gated on
        // `is_editor_process()`, so a game could never show it — it was simply
        // linked in regardless.
        runtime_features: &["crash_dialog"],
        default_on: false,
        group: None,
    },
    Capability {
        id: "diagnostics",
        section: "build",
        label: "System diagnostics (CPU/RAM)",
        help: "Bevy's process CPU and memory diagnostics, which pull in `sysinfo` — a per-OS \
               system-information crate the engine only reads for the editor's debug overlay. \
               Frame-time diagnostics are unaffected.",
        // Arrived through `default_platform` behind the `2d`/`3d`/`ui` metas, so
        // the root manifest's note calling it "unused — DROPPED" was wrong: it
        // shipped in every export. Named explicitly there now, hence strippable.
        bevy_features: &["sysinfo_plugin"],
        runtime_features: &[],
        default_on: false,
        group: None,
    },
    Capability {
        id: "gamepad",
        section: "systems",
        label: "Gamepad input",
        help: "Controller support — `bevy_gilrs` and the `gilrs`/`gilrs-core` backend behind it. \
               Off leaves keyboard and mouse working; every gamepad becomes invisible to the \
               engine, so leave it on for anything a controller can play.",
        // Same story as `diagnostics`: reached via `default_platform`, never named.
        bevy_features: &["bevy_gilrs"],
        runtime_features: &[],
        default_on: true,
        group: None,
    },
    Capability {
        id: "gizmos",
        section: "build",
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
        section: "assets",
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
        section: "sky",
        label: "Atmosphere",
        help: "Physically-based sky scattering.",
        bevy_features: &[],
        runtime_features: &["atmosphere"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "environment_map",
        section: "sky",
        label: "Environment maps",
        help: "Image-based lighting from an HDRI or baked cubemap.",
        bevy_features: &[],
        runtime_features: &["environment_map"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "skybox",
        section: "sky",
        label: "Skybox",
        help: "Cubemap / procedural skybox background.",
        bevy_features: &[],
        runtime_features: &["skybox"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "clouds",
        section: "sky",
        label: "Clouds",
        help: "Volumetric cloud rendering.",
        bevy_features: &[],
        runtime_features: &["clouds"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "night_stars",
        section: "sky",
        label: "Night stars",
        help: "The star field for night skies.",
        bevy_features: &[],
        runtime_features: &["night_stars"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "localization",
        section: "systems",
        label: "Translation packs",
        help: "The twenty embedded `languages/*.toml` packs — about 2.4 MiB of TOML compiled straight into the binary. Off leaves every string at its English fallback, since `t()` returns the key's own text when no pack is loaded. Drop it for a single-language game.",
        bevy_features: &[],
        runtime_features: &["localization"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "particles",
        section: "simulation",
        label: "Particles",
        help: "The GPU particle system (bevy_hanabi). ~5 MB — drop if your game has no particle effects.",
        bevy_features: &[],
        runtime_features: &["particles"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "postfx",
        section: "postfx",
        label: "Post-processing",
        help: "The post-process stack as a whole. Off takes every effect below with it AND \
               bevy own built-in post-process pipeline (~420 KiB), which survived having each \
               effect individually unticked because nothing named it. The framework itself \
               stays: C-ABI plugins register their render passes through it, so a \
               plugin-provided effect still works. Tonemapping is separate — it lives in \
               bevy_core_pipeline, not here.",
        bevy_features: &["bevy_post_process"],
        runtime_features: &[],
        default_on: true,
        group: None,
    },
    Capability {
        id: "bloom",
        section: "postfx",
        label: "Bloom",
        help: "Bright-pass glow.",
        bevy_features: &[],
        runtime_features: &["bloom"],
        default_on: true,
        group: Some("postfx"),
    },
    Capability {
        id: "ssao",
        section: "postfx",
        label: "SSAO",
        help: "Screen-space ambient occlusion.",
        bevy_features: &[],
        runtime_features: &["ssao"],
        default_on: true,
        group: Some("postfx"),
    },
    Capability {
        id: "ssr",
        section: "postfx",
        label: "SSR",
        help: "Screen-space reflections.",
        bevy_features: &[],
        runtime_features: &["ssr"],
        default_on: true,
        group: Some("postfx"),
    },
    Capability {
        id: "dof",
        section: "postfx",
        label: "Depth of field",
        help: "Camera focus blur.",
        bevy_features: &[],
        runtime_features: &["dof"],
        default_on: true,
        group: Some("postfx"),
    },
    Capability {
        id: "motion_blur",
        section: "postfx",
        label: "Motion blur",
        help: "Per-object and camera motion blur.",
        bevy_features: &[],
        runtime_features: &["motion_blur"],
        default_on: true,
        group: Some("postfx"),
    },
    Capability {
        id: "distance_fog",
        section: "postfx",
        label: "Distance fog",
        help: "Depth-based fog.",
        bevy_features: &[],
        runtime_features: &["distance_fog"],
        default_on: true,
        group: Some("postfx"),
    },
    Capability {
        id: "volumetric_fog",
        section: "postfx",
        label: "Volumetric fog",
        help: "Light-scattering fog volumes.",
        bevy_features: &[],
        runtime_features: &["volumetric_fog"],
        default_on: true,
        group: Some("postfx"),
    },
    Capability {
        id: "lens_distortion",
        section: "postfx",
        label: "Lens distortion",
        help: "Barrel / chromatic lens warp.",
        bevy_features: &[],
        runtime_features: &["lens_distortion"],
        default_on: true,
        group: Some("postfx"),
    },
    Capability {
        id: "auto_exposure",
        section: "postfx",
        label: "Auto exposure",
        help: "Adaptive eye-adjustment exposure.",
        bevy_features: &[],
        runtime_features: &["auto_exposure"],
        default_on: true,
        group: Some("postfx"),
    },
    Capability {
        id: "oit",
        section: "postfx",
        label: "Order-independent transparency",
        help: "Correct blending for overlapping transparent surfaces.",
        bevy_features: &[],
        runtime_features: &["oit"],
        default_on: true,
        group: Some("postfx"),
    },
    Capability {
        id: "antialiasing",
        section: "postfx",
        label: "Anti-aliasing",
        help: "TAA / FXAA / SMAA. Off leaves MSAA only, and drops the SMAA lookup textures — \
               KTX2 blobs embedded in the binary as data.",
        // `bevy_anti_alias` and its LUTs came in via the `3d` meta and so could
        // not be stripped before the manifest named them.
        bevy_features: &["bevy_anti_alias", "smaa_luts"],
        runtime_features: &["antialiasing"],
        default_on: true,
        group: Some("postfx"),
    },
    Capability {
        id: "lumen",
        section: "render_3d",
        label: "Lumen global illumination",
        help: "Software-traced GI with its own render graph and compute passes.",
        bevy_features: &[],
        runtime_features: &["lumen"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "cloth",
        section: "simulation",
        label: "Cloth simulation",
        help: "Verlet cloth (bevy_silk).",
        bevy_features: &[],
        runtime_features: &["cloth"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "ragdoll",
        section: "simulation",
        label: "Ragdolls",
        help: "Physics bodies per bone. Needs 3D physics.",
        bevy_features: &[],
        runtime_features: &["ragdoll"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "gaussian_splatting",
        section: "render_3d",
        label: "Gaussian splatting",
        help: "The .ply/.sog splat renderer — sizeable; drop unless a scene uses one.",
        bevy_features: &[],
        runtime_features: &["gaussian_splatting"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "light2d",
        section: "render_2d",
        label: "2D lighting",
        help: "The bevy_firefly 2D light and shadow renderer.",
        bevy_features: &[],
        runtime_features: &["light2d"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "text3d",
        section: "render_3d",
        label: "3D text",
        help: "World-space extruded text meshes.",
        bevy_features: &[],
        runtime_features: &["text3d"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "vignette",
        section: "postfx",
        label: "Vignette",
        help: "Screen-edge darkening.",
        bevy_features: &[],
        runtime_features: &["vignette"],
        default_on: true,
        group: Some("postfx"),
    },
    Capability {
        id: "forward_decal",
        section: "render_3d",
        label: "Forward decals",
        help: "Projected decals on forward-rendered surfaces.",
        bevy_features: &[],
        runtime_features: &["forward_decal"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "pool_water",
        section: "render_3d",
        label: "Pool water",
        help: "The animated pool-water material.",
        bevy_features: &[],
        runtime_features: &["pool_water"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "procedural_tree",
        section: "render_3d",
        label: "Procedural trees",
        help: "Runtime tree mesh generation.",
        bevy_features: &[],
        runtime_features: &["procedural_tree"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "sprite_anim",
        section: "render_2d",
        label: "2D sprite animation",
        help: "Named AnimatedSprite clips and their scripting API.",
        bevy_features: &[],
        runtime_features: &["sprite_anim"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "water",
        section: "simulation",
        label: "Water",
        help: "FFT ocean water: wave cascades, foam and buoyancy.",
        bevy_features: &[],
        runtime_features: &["water"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "terrain",
        section: "simulation",
        label: "Terrain",
        help: "The terrain subsystem.",
        bevy_features: &[],
        runtime_features: &["terrain"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "spline",
        section: "simulation",
        label: "Splines",
        help: "The spline subsystem.",
        bevy_features: &[],
        runtime_features: &["spline"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "navmesh",
        section: "simulation",
        label: "Navmesh pathfinding",
        help: "Navigation-mesh generation and pathfinding (polyanya/vleue).",
        bevy_features: &[],
        runtime_features: &["navmesh"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "tilemap",
        section: "render_2d",
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
        section: "simulation",
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
        section: "simulation",
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
        section: "render_3d",
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
        section: "render_3d",
        label: "glTF model loading",
        help: "The .gltf/.glb loader and its animation support. A scene built only from                engine primitives (cube, sphere, plane) never touches it.",
        bevy_features: &["bevy_gltf", "gltf_animation"],
        // Without this the toggle did not compile: `renzora_engine` uses
        // `bevy::gltf::Gltf` directly for the LOD-variant probe and the
        // mesh-instance rehydrate, so stripping the Bevy feature alone left
        // those referring to a module that no longer existed.
        runtime_features: &["gltf"],
        default_on: true,
        group: Some("render_3d"),
    },
    Capability {
        id: "shader_graph",
        section: "render_3d",
        label: "Graph materials (node-based shaders)",
        help: "The `renzora_shader` node-graph material system — roughly 1 MiB of code — which \
               compiles `.material` assets into custom PBR shaders at runtime. A game whose \
               meshes all use plain StandardMaterial never touches it. Auto-enabled when the \
               project contains `.material` files.",
        bevy_features: &[],
        runtime_features: &["shader_graph"],
        default_on: true,
        group: Some("render_3d"),
    },
    Capability {
        id: "morph_targets",
        section: "render_3d",
        label: "Morph targets (blend shapes)",
        help: "Per-vertex blend-shape deformation and its animation sampling. Used by                face rigs and shape keys; nothing else needs it.",
        bevy_features: &["morph", "morph_animation"],
        runtime_features: &[],
        default_on: true,
        group: Some("render_3d"),
    },
    Capability {
        id: "pbr_textures",
        section: "render_3d",
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
        section: "render_3d",
        label: "Lighting lookup tables",
        help: "Precomputed tables baked into the binary as data, not code: the blue-noise                texture, the DFG environment-BRDF table and the area-light LTC tables.                Dropping them costs quality in specular/area-light shading, not                correctness elsewhere.",
        bevy_features: &["bluenoise_texture", "dfg_lut", "area_light_luts"],
        runtime_features: &[],
        default_on: true,
        group: Some("render_3d"),
    },
    Capability {
        id: "render_2d",
        section: "render_2d",
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
        section: "systems",
        label: "Audio",
        help: "The audio subsystem. Drop for a silent game.",
        bevy_features: &[],
        runtime_features: &["audio"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "animation",
        section: "systems",
        label: "Skeletal animation",
        help: "The skeletal/property animation subsystem.",
        bevy_features: &[],
        runtime_features: &["animation"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "panic_unwind",
        section: "build",
        label: "Panic unwinding (fault isolation)",
        help: "Keeps Rust's unwinding panic strategy. Turning it OFF builds with `panic = \"abort\"`, \
               which measured ~24% smaller (60.9 MB → 46.7 MB on a cube-and-light project) because \
               the unwind tables, landing pads and panic message/location strings all go. THE COST: \
               the engine guards every call into a C-ABI plugin with `catch_unwind`, including each \
               script call — with abort, a panicking plugin or script takes the whole game down \
               instead of being caught and logged. Crash reports still work (the panic hook runs \
               before the abort). Leave it on unless you've tested your game's scripts.",
        bevy_features: &[],
        runtime_features: &[],
        default_on: true,
        group: None,
    },
    Capability {
        id: "picking",
        section: "systems",
        label: "Picking (mesh & sprite raycasts)",
        help: "Bevy's pointer-picking framework and its mesh/sprite backends — the machinery behind \
               `Pointer<Click>` hit-testing in the world. Nothing in the runtime uses it unless your \
               game does; the editor's viewport picking is separate and never ships. Note the UI \
               keeps its own picking backend, so the full saving only lands when the UI layer is \
               off too.",
        bevy_features: &["bevy_picking", "mesh_picking", "sprite_picking"],
        runtime_features: &[],
        default_on: true,
        group: None,
    },
    Capability {
        id: "tonemapping_luts",
        section: "postfx",
        label: "Tonemapping lookup tables",
        help: "The colour-grading tables Bevy embeds as data for the AgX, TonyMcMapface and \
               Blender-Filmic tone curves — about 680 KiB of KTX2 baked into the binary. Off, \
               those three are substituted with ACES-fitted, which is also filmic but needs no \
               table; the simpler curves (Reinhard, None) are unaffected. Applies to 2D as well \
               as 3D, so it isn't nested under 3D rendering.",
        bevy_features: &["tonemapping_luts"],
        // The runtime half is not optional here: Bevy does NOT fall back when a
        // LUT curve has no table — it logs an error and renders the screen
        // magenta — and `Tonemapping`'s own `Default` is TonyMcMapface, which
        // every camera gets as a required component. Stripping the Bevy feature
        // alone therefore broke the picture outright.
        runtime_features: &["tonemapping_luts"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "scripting",
        section: "systems",
        label: "Scripting",
        help: "The scripting layer: hooks (on_ready/on_update/…), the command queue, and the \
               script vocabularies every subsystem declares. Auto-enabled when the project \
               contains .lua files. Off strips it from the engine, ember, animation, physics, \
               navmesh, ragdoll and localization at once. Blueprints and game UI both need it, \
               so keeping either brings it back automatically.",
        bevy_features: &[],
        runtime_features: &["scripting"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "blueprint",
        section: "systems",
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
        section: "systems",
        label: "Script HTTP (http_get / http_post)",
        help: "The script HTTP verbs — pull in the ureq + rustls/ring TLS stack (~1 MiB). \
               Auto-enabled when a script calls http_get/http_post.",
        bevy_features: &[],
        runtime_features: &["script_http"],
        default_on: false,
        group: None,
    },
    Capability {
        id: "ui",
        section: "ui",
        label: "User interface",
        help: "The whole UI layer: bevy_ui, bevy_ui_render, bevy_ui_widgets, bevy_text and the \
               `renzora_ember` widget framework. Turning it off also drops the text-shaping stack \
               behind bevy_text — parley, swash, harfrust, fontique, skrifa, read-fonts — which is \
               several MB of pure dead weight for a game that draws no text. Only for a game with \
               no on-screen UI at all; keeping Game UI or 3D text brings it back.",
        bevy_features: &[
            "bevy_ui",
            "bevy_ui_render",
            "bevy_ui_widgets",
            "bevy_text",
            "ui_picking",
            "bevy_input_focus",
        ],
        runtime_features: &["ui"],
        default_on: true,
        group: None,
    },
    Capability {
        id: "default_font",
        section: "ui",
        label: "Bevy's built-in font",
        help: "The fallback typeface Bevy embeds in every binary as data. Off is safe for a game \
               that ships its own fonts — text with no explicit font simply doesn't render.",
        bevy_features: &["default_font"],
        runtime_features: &[],
        default_on: true,
        group: Some("ui"),
    },
    Capability {
        id: "game_ui",
        section: "ui",
        label: "Game UI (markup)",
        help: "The in-game `.html` markup UI runtime. Needs the UI layer and scripting, both of \
               which come back automatically if this is kept.",
        bevy_features: &[],
        runtime_features: &["game_ui"],
        default_on: true,
        group: Some("ui"),
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
        // The scripting layer only when the project ships scripts. Safe to decide
        // from files alone: the two other things that need it — blueprints and
        // ember's markup UI — enable it through Cargo (`blueprint`/`game_ui` both
        // list `scripting`), so a project with graphs or .html UI but no .lua
        // still gets it.
        "scripting" => &["lua"],
        // The glTF loader only when the project ships models. A scene built from
        // engine primitives never touches it, and it isn't cheap: `bevy_gltf`
        // plus the `gltf`/`gltf-json`/`gltf-derive` crates and their serde
        // derives. Detection is on files present anywhere under the project, so
        // a model that ships in the rpak is found whether or not it was
        // converted on import.
        "gltf" => &["gltf", "glb"],
        // The graph-material system only when the project authors materials with
        // it. A `.material` asset IS a shader graph, so its presence is exactly
        // the condition — meshes using plain StandardMaterial produce none.
        "shader_graph" => &["material"],
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

/// Whether the export copy's `dist-lean` profile should be patched to
/// `panic = "abort"`.
///
/// Not a Cargo feature, so it can't ride the normal strip path — but it is by
/// far the largest single saving available (measured 60.9 MB → 46.7 MB, with
/// `.text` down 6.9 MB and `.rdata` down 6.9 MB, not just the unwind tables).
///
/// The root manifest says `dist-lean` can't use `abort` because the `renzora`
/// crate is `dylib`+`rlib` and the dylib links the precompiled std's
/// `panic_unwind`. That is true of the dev build and NOT of this one: the export
/// copy is patched to build `renzora` as `rlib` only (to dodge the Windows PE
/// export cap), so there is no dylib in a lean binary and the objection doesn't
/// apply.
pub fn use_panic_abort(state: &HashMap<String, bool>) -> bool {
    !state.get("panic_unwind").copied().unwrap_or(true)
}

/// Bevy features to strip from the export copy (union of OFF capabilities).
///
/// Mirrors [`disabled_runtime_features`]'s render_3d rule on the Bevy side: the
/// anti-aliasing crate is part of the 3D pipeline, so dropping 3D must take it
/// too. Without this, a 2D export strips `renzora_antialiasing` but still
/// compiles `bevy_anti_alias` and embeds the SMAA lookup textures.
pub fn disabled_bevy_features(state: &HashMap<String, bool>) -> Vec<String> {
    let mut out = collect_disabled(state, |c| c.bevy_features);
    if !state.get("render_3d").copied().unwrap_or(true) {
        // `bevy_post_process` too: the six effect crates that use it are all
        // force-stripped with 3D (see `disabled_runtime_features`), so nothing is
        // left to need bevy's built-in stack.
        for f in ["bevy_anti_alias", "smaa_luts", "bevy_post_process"] {
            if !out.iter().any(|x| x == f) {
                out.push(f.to_string());
            }
        }
    }
    out
}

/// `renzora_runtime` `default` features to strip from the export copy.
///
/// Enforces the one hard dependency between capabilities: the 3D subsystems
/// (terrain/water/sky/post-FX/spline) build on bevy_pbr, so when `render_3d` is
/// off they MUST be stripped too — otherwise the 2D build fails to compile. We do
/// it here (not via a Cargo feature dep, which would force render_3d back ON).
pub fn disabled_runtime_features(state: &HashMap<String, bool>) -> Vec<String> {
    let mut out = collect_disabled(state, |c| c.runtime_features);
    // 3D text is the one subsystem outside the UI tree that still needs
    // `bevy_text`: `renzora_text_mesh` builds its glyph outlines from
    // `bevy::text`'s font data. Verified by compiling a UI-stripped runtime —
    // this was the only error. Not expressed as `group: Some("ui")` because
    // extruded world-space text isn't a kind of UI, and nesting it there would
    // read as though turning UI on were what enabled it.
    if !state.get("ui").copied().unwrap_or(true) && !out.iter().any(|x| x == "text3d") {
        out.push("text3d".to_string());
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A capability whose `section` isn't in [`SECTIONS`] renders nowhere — the
    /// Features tab iterates sections and picks up members, so a typo silently
    /// drops the toggle out of the UI while the strip logic still honours it.
    /// That failure is invisible by inspection, hence the test.
    #[test]
    fn every_capability_has_a_known_section() {
        for c in CAPABILITIES {
            assert!(
                SECTIONS.iter().any(|(id, _)| *id == c.section),
                "capability `{}` has section `{}`, which is not in SECTIONS",
                c.id,
                c.section,
            );
        }
    }

    /// Every capability must be reachable in the rendered list exactly once.
    /// Guards the derived ordering in `native.rs`: children are emitted under
    /// their parent, so a child whose `group` names a missing (or itself
    /// grouped) parent would never be drawn.
    #[test]
    fn every_capability_renders_exactly_once() {
        for c in CAPABILITIES {
            let Some(parent_id) = c.group else { continue };
            let parent = CAPABILITIES
                .iter()
                .find(|p| p.id == parent_id)
                .unwrap_or_else(|| panic!("`{}` groups under unknown `{parent_id}`", c.id));
            assert!(
                parent.group.is_none(),
                "`{}` groups under `{parent_id}`, which is itself a child — the \
                 Features tab only nests one level, so it would never render",
                c.id,
            );
        }
    }

    /// Ids are used as map keys in the saved per-project capability state, so a
    /// duplicate would make two toggles share one checkbox.
    #[test]
    fn capability_ids_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for c in CAPABILITIES {
            assert!(seen.insert(c.id), "duplicate capability id `{}`", c.id);
        }
    }
}
