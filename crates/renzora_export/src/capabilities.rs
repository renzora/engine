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
//! * **Structural subsystems** (audio, navmesh, post-FX, sky, terrain, …):
//!   `renzora_runtime` features. Default ON *unless the project says otherwise*.
//!
//! # The defaults are read off the project
//!
//! [`scan_project`] walks the project once and reads its scenes, scripts and
//! markup; [`defaults_from_scan`] turns that into the tick state the export
//! dialog opens with. A `.bsn` scene names every component by its full Rust path,
//! so "does this game use terrain" is answerable exactly, and the subsystems that
//! are reachable only from a script are matched on their script-API names
//! instead. See [`detection_types`].
//!
//! The structural subsystems used to default ON unconditionally, on the
//! reasoning that a game might reach one from a script the scan could not see.
//! That was the right call while the scan only looked at file extensions — but
//! it meant a 2D game shipped the entire 3D pipeline unless its author walked
//! the whole list by hand, which in practice nobody did. Reading the scenes is
//! not guessing, and where the content genuinely cannot answer (no scenes at
//! all; neither pipeline in evidence; which physics backend) the old default
//! still stands. Those three holes are spelled out on [`defaults_from_scan`].
//!
//! Nothing here is final: every toggle is still a toggle, and the dialog is
//! where the author overrides a call the scan got wrong.

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
        //
        // `editor_tools` is the same story one crate over: `renzora_ember`
        // depended on `arboard` and on the `image` decoder stack (plus `moxcms`)
        // for its code editor and file-browser thumbnails, non-optionally, so a
        // shipped game carried both. Nothing a game renders goes through either.
        runtime_features: &["crash_dialog", "editor_tools"],
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
        // Stripping the bevy features is only half of it: any crate that still
        // declares a `Gizmos` system param then fails to compile on a type that
        // no longer exists. `renzora_runtime`'s `gizmos` forwards to the crates
        // that draw them (`renzora_light2d`'s selection outlines today), so the
        // code goes at the same moment the capability does.
        runtime_features: &["gizmos"],
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
        id: "networking",
        section: "systems",
        label: "Multiplayer (networking)",
        help: "The from-scratch UDP transport, replication and script RPC, plus the \
               `--server` / `--host` startup paths and the `bincode`/`serde_json` wire \
               formats behind them. Off makes the game single-player: `--server` on such a \
               build logs that it has no networking and starts as an ordinary client. \
               Detected from replicated entities in a scene and from `rpc`/`net_*` calls in \
               scripts.",
        bevy_features: &[],
        runtime_features: &["networking"],
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
        id: "parkour",
        section: "simulation",
        label: "Parkour traversal",
        help: "Vault/mantle/ledge-hang/ladder/wall-run/swing character controller. 3D only, and it pulls in 3D physics on its own.",
        bevy_features: &[],
        runtime_features: &["parkour"],
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
            // Raycasting against 3D meshes. Named here as well as under the
            // Picking capability, because a 2D game that keeps picking (sprite
            // and UI hit-testing are the same framework) was still compiling the
            // mesh backend for meshes it does not have.
            "mesh_picking",
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
        help: "The skeletal/property animation subsystem — `renzora_animation` (clips, state \
               machines, the property dopesheet) and Bevy's `bevy_animation` under it, which \
               brings `blake3` for its curve hashing. Off for a game that animates by moving \
               transforms or swapping sprite frames.",
        // `bevy_animation` used to be missing here, so the toggle stripped our
        // half and left bevy's compiled into every export. It comes out cleanly
        // now that `scene_io`'s three `bevy::animation` denies sit behind the
        // matching `renzora_engine/animation` gate.
        // `gltf_animation` is listed by the glTF capability too, and needs to be
        // in both: it expands to `bevy_animation`, so leaving it behind while
        // animation is off would pull the crate straight back in — the same
        // child-re-enables-parent trap the nested capabilities exist to avoid.
        // `collect_disabled` unions the OFF capabilities, so naming it twice
        // means it goes when either one is unticked.
        bevy_features: &["bevy_animation", "gltf_animation"],
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
        id: "loop_vectorization",
        section: "build",
        label: "Loop vectorization (opt-level s)",
        help: "Builds at `opt-level = \"s\"`, which optimizes for size but still lets LLVM \
               vectorize loops. Turning it OFF uses `opt-level = \"z\"`, the same trade with \
               vectorization disabled as well: smaller code, but hot per-vertex/per-pixel CPU \
               loops lose their SIMD widening. Which one wins on size is genuinely \
               project-dependent — `z` is not always smaller, because a scalar loop that has to \
               run more iterations can cost more in unrolling than the vector version saved. \
               Export both and compare before shipping `z`.",
        bevy_features: &[],
        runtime_features: &[],
        default_on: true,
        group: None,
    },
    Capability {
        id: "parallel_codegen",
        section: "build",
        label: "Parallel code generation (16 units)",
        help: "Splits each crate into 16 LLVM modules that compile in parallel — the release \
               default, and what makes a lean export take minutes rather than an hour. Turning \
               it OFF uses `codegen-units = 1`: one module per crate, so thin LTO sees whole \
               crates at once and has fewer duplicated inline copies left to merge. It is the \
               classic size trick, but it interacts with LTO rather than adding to it, so the \
               win here is usually small — pay the build time only if a measured export says \
               it's worth it.",
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
///
/// The coarse half of detection: "there is a `.glb` in here, so the glTF loader
/// is wanted". [`detection_types`] is the fine half, which looks *inside* those
/// files. A capability listed in either is decided by the scan; one listed in
/// neither keeps its `default_on`.
fn detection_extensions(id: &str) -> &'static [&'static str] {
    match id {
        // Markup UI is a `.html` file, and it needs the UI layer under it. Listed
        // as well as the type paths in `detection_types` because a template that
        // no scene references yet is still a template the game means to show.
        "ui" | "game_ui" => &["html"],
        // Sprite/skeletal animation clips and state machines.
        "animation" => &["anim", "animsm"],
        // An authored effect asset is a clear intention even if no scene has
        // placed one yet.
        "particles" => &["particle"],
        // Audio files a script may play without any scene naming an emitter.
        "audio" => &["mp3", "ogg", "wav", "flac"],
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

/// Text fragments that, found inside a scene or a script, mean the project
/// actually uses a capability.
///
/// # Why substrings of type paths
///
/// A `.bsn` scene names every component by its full Rust path —
/// `renzora_terrain::data::TerrainData`, `bevy_firefly::lights::PointLight2d` —
/// so the defining crate is written out beside every single use. Matching
/// `renzora_terrain::` therefore answers "does anything in this project have
/// terrain on it" without this table having to know which of that crate's dozen
/// components a scene happens to hold, and without going stale when one is
/// renamed. Where a subsystem's serialized type lives in the contract crate
/// instead (`SpriteSheet`, `GaussianSplat`), the bare type name is listed too.
///
/// # Script API names, and why they are here
///
/// A scene is not the whole story: a game can spawn its world from a script, and
/// then the only trace of a subsystem is the function the script calls. Lua's
/// `play_sound`, `apply_impulse` and `parkour_jump` name no type at all. So the
/// scan reads scripts as well as scenes, and the entries below include the
/// script-facing names for every subsystem that has one. Getting this wrong in
/// the "keep it" direction costs a few hundred KB; getting it wrong the other
/// way ships a game whose sound does not play, so the lists are deliberately
/// generous and a subsystem with an ambiguous signal stays on.
fn detection_types(id: &str) -> &'static [&'static str] {
    match id {
        // ── The two pipelines ────────────────────────────────────────────────
        //
        // Neither is ever inferred from the *absence* of the other: a project
        // with no evidence for either (a menu-only prototype, say) keeps both,
        // handled in `defaults_from_scan`. `MeshPrimitive` covers a scene built
        // from engine primitives, `spawn_primitive`/`cube`/`sphere` the script
        // that makes them at runtime.
        "render_3d" => &[
            "Mesh3d",
            "Camera3d",
            "MeshMaterial3d",
            "StandardMaterial",
            "MeshPrimitive",
            "MeshInstanceData",
            "PbrAdvanced",
            "bevy_pbr::",
            "bevy_solari::",
            "spawn_primitive",
        ],
        "render_2d" => &[
            "Camera2d",
            "Mesh2d",
            "MeshMaterial2d",
            "Node2d",
            "SpriteImagePath",
            "SpriteCustomSize",
            "YSort",
            "bevy_sprite::",
        ],

        // ── 2D subsystems ────────────────────────────────────────────────────
        "light2d" => &["bevy_firefly::", "renzora_light2d::"],
        "tilemap" => &["renzora_tilemap::"],
        // `renzora_sprite_anim` serializes nothing of its own — a sprite
        // animation is a `.anim` clip (an extension, above) driving the contract
        // crate's atlas components.
        "sprite_anim" => &["SpriteSheet", "SpriteAtlasRegion", "renzora_sprite_anim::"],

        // ── 3D subsystems ────────────────────────────────────────────────────
        "terrain" => &["renzora_terrain::"],
        "water" => &["renzora_water::"],
        "lumen" => &["renzora_lumen::", "LumenLighting"],
        "gaussian_splatting" => &[
            "bevy_gaussian_splatting::",
            "renzora_gaussian_splatting::",
            "GaussianSplat",
        ],
        "forward_decal" => &["renzora_forward_decal::", "DecalSettings"],
        "cloth" => &["renzora_cloth::", "bevy_silk::"],

        // ── Sky ──────────────────────────────────────────────────────────────
        "atmosphere" => &["renzora_atmosphere::", "set_sun_angles"],
        "environment_map" => &["renzora_environment_map::", "ReflectionProbeSource"],
        "skybox" => &["renzora_skybox::"],

        // ── Post-processing ──────────────────────────────────────────────────
        //
        // One crate each, and each crate's settings component is what a camera
        // carries when the effect is on, so the crate prefix is exact.
        "bloom" => &["renzora_bloom_effect::"],
        "ssao" => &["renzora_ssao::"],
        "ssr" => &["renzora_ssr::"],
        "dof" => &["renzora_dof::"],
        "motion_blur" => &["renzora_motion_blur::"],
        "distance_fog" => &["renzora_distance_fog::", "set_fog"],
        "volumetric_fog" => &["renzora_volumetric_fog::"],
        "lens_distortion" => &["renzora_lens_distortion::"],
        "oit" => &["renzora_oit::"],
        "antialiasing" => &["renzora_antialiasing::"],

        // ── Simulation & gameplay ────────────────────────────────────────────
        "particles" => &["renzora_hanabi::", "bevy_hanabi::"],
        "navmesh" => &[
            "renzora_navmesh::",
            "vleue_navigator::",
            "nav_set_destination",
            "nav_clear_destination",
            "nav_stop",
        ],
        "ragdoll" => &["renzora_ragdoll::", "enable_ragdoll"],
        "parkour" => &[
            "renzora_parkour::",
            "parkour_move",
            "parkour_sprint",
            "parkour_jump",
            "parkour_action",
        ],
        "animation" => &[
            "renzora_animation::",
            "bevy_animation::",
            "play_animation",
            "crossfade_animation",
            "set_anim_param",
            "set_anim_bool",
            "set_anim_trigger",
        ],
        "audio" => &[
            "renzora_audio::",
            "AudioLink",
            "AudioEmitting",
            "play_sound",
            "play_music",
            "play_audio",
        ],
        // Physics is deliberately absent here: the two backends share one set of
        // serialized components and one script API, so which dimension a project
        // needs cannot be read off a name. `defaults_from_scan` decides it from
        // the pipeline instead — see `PHYSICS_MARKERS`.

        // ── Systems ──────────────────────────────────────────────────────────
        //
        // `net_` and `rpc` cover the script side in both languages; the component
        // paths cover a scene that marks entities for replication. `on_rpc` and
        // `on_player_joined` are hooks rather than calls, and a script that
        // defines one is a script expecting a network, so they count too.
        "networking" => &[
            "renzora_network::",
            "Networked",
            "NetworkOwner",
            "net_rpc",
            // The call form, not the bare word: `rpc` alone is three letters
            // that turn up inside unrelated identifiers and hex blobs.
            "rpc(",
            "net_is_",
            "net_player_count",
            "on_rpc",
            "on_player_joined",
            "on_player_left",
            "ScriptHook::Rpc",
        ],
        "scripting" => &["renzora_scripting::", "ScriptComponent"],
        // World-space picking only, and the needles are chosen carefully.
        //
        // `Pickable` and `PickingInteraction` are what this used to match, and
        // they are useless as evidence: bevy_ui inserts both on *every* UI node
        // as required components, so any project with a HUD saved hundreds of
        // them into its scenes and the capability was on for everyone. What is
        // actually being asked is "does this game raycast into the world", and
        // the answer to that is an observer on a `Pointer<…>` event or a
        // deliberate settings component. UI hit-testing is unaffected either
        // way — it is `ui_picking`, which rides the UI capability.
        "picking" => &["Pointer<", "MeshPickingSettings", "MeshRayCastSettings"],
        // The three LUT-sampling tone curves, by the names a scene spells them.
        // `TonemappingSettings` counts too: an entity that carries one has
        // chosen a curve deliberately, and five of the eight need no tables —
        // but keeping a few hundred KB of KTX2 is the cheap side of that guess.
        //
        // Under-detecting is survivable here in a way it is not elsewhere:
        // `renzora_tonemapping::force_lutless_tonemapping` rewrites any
        // LUT-sampling curve to a table-free one when the tables are gone, so
        // the worst case is a slightly different picture, not a magenta screen.
        "tonemapping_luts" => &[
            "AgX",
            "TonyMcMapface",
            "BlenderFilmic",
            "TonemappingSettings",
        ],
        // Translation, from the call rather than from the setting.
        //
        // There is no project-level language field to read — the active language
        // is a per-user preference in `~/.renzora/editor.toml` — so the question
        // becomes "does anything in this project ask for a translated string".
        // The needles are the qualified call, never a bare `t(`: that matches
        // `print(`, `expect(` and every other identifier ending in t, which is
        // exactly the kind of needle that keeps a capability on for everyone.
        //
        // `scan_project` adds one more signal that is not a text match: a
        // `languages/` directory of the project's own packs.
        "localization" => &["lang::t(", "lang::t_or(", "renzora::lang"],
        // `Gamepad` covers the components and the Rust script API; `gamepad_`
        // covers Lua's, where every entry point is `gamepad_button`,
        // `gamepad_left_stick` and so on.
        "gamepad" => &["Gamepad", "gamepad_"],

        // ── Interface ────────────────────────────────────────────────────────
        "ui" => &["bevy_ui::", "renzora_ember::", "UiCanvas"],
        "game_ui" => &["renzora_ember::game_ui", "UiCanvas", "HtmlTemplatePath"],

        _ => &[],
    }
}

/// Names that mean "this project uses physics", without saying which dimension.
///
/// `renzora_physics` owns the serialized components for both backends
/// (`PhysicsBodyData`, `CollisionShapeData`) and `auto_init_physics` picks
/// avian2d or avian3d at runtime from whether the entity is a 2D one — so the
/// scene genuinely does not record the answer. The avian paths are listed anyway
/// for a scene that names one directly.
const PHYSICS_MARKERS: &[&str] = &[
    "renzora_physics::",
    "avian2d::",
    "avian3d::",
    "apply_force",
    "apply_impulse",
    "set_linear_velocity",
    "move_controller",
    "is_colliding",
    "set_gravity_scale",
];

/// File types whose *contents* the scan reads.
///
/// Scenes and prefabs first — they are what the detection is really about — plus
/// the places a subsystem can be reached without appearing in a scene at all:
/// scripts in either language, markup templates, and the authored assets
/// (materials, blueprints, particle effects) that name the crate that loads them.
/// Everything else is counted by extension only, so a project full of textures
/// and audio costs one `read_dir` per folder and no file reads.
const SCANNED_EXTENSIONS: &[&str] = &[
    "bsn", "ron", "lua", "rs", "html", "material", "blueprint", "bp", "particle", "anim", "animsm",
];

/// One file may not exceed this when scanned. A scene is text and stays well
/// under it; the cap is only here so a stray multi-megabyte file in a project
/// folder cannot stall the export dialog on open.
const MAX_SCANNED_FILE: u64 = 16 * 1024 * 1024;

/// What one walk of the project learned.
///
/// Built once when the export overlay opens and shared by both consumers — the
/// capability defaults and the plugin pre-selection — because they were each
/// walking the whole project separately to ask almost the same question.
pub struct ProjectScan {
    /// Lowercased extensions present anywhere under the project.
    extensions: std::collections::HashSet<String>,
    /// Which of the needles it was given were found in a readable file.
    found: std::collections::HashSet<String>,
    /// Whether any scene or prefab was read.
    ///
    /// Load-bearing: with no scene there is no evidence of anything, and turning
    /// every undetected subsystem off would strip a working game down to
    /// nothing. The callers fall back to plain defaults when this is false.
    pub saw_scene: bool,
    /// Every file in the project, as a forward-slashed project-relative path.
    ///
    /// Collected here because this walk already visits every one of them, so the
    /// export dialog's Files tab costs no extra traversal. Sorted, so the tree it
    /// builds is stable between openings.
    pub files: Vec<String>,
    /// Lowercased directory names seen anywhere under the project.
    ///
    /// For the signals a file's *contents* cannot carry. A `languages/` folder
    /// of the project's own translation packs is the one that needs it: nothing
    /// inside those `.toml` files says what they are, and the folder name is the
    /// contract the loader itself goes by.
    dirs: std::collections::HashSet<String>,
}

impl ProjectScan {
    /// Was `needle` found anywhere?
    pub fn saw(&self, needle: &str) -> bool {
        self.found.contains(needle)
    }

    fn saw_any(&self, needles: &[&str]) -> bool {
        needles.iter().any(|n| self.found.contains(*n))
    }

    /// Is there a directory of this name (case-insensitively) in the project?
    pub fn saw_dir(&self, name: &str) -> bool {
        self.dirs.contains(&name.to_ascii_lowercase())
    }
}

/// Walk `root` once, recording extensions and which needles appear in its
/// scenes, scripts and markup.
///
/// `extra` is for needles only the caller knows — the plugin picker passes one
/// crate prefix per installed plugin. They are searched in the same pass, so
/// adding a caller costs no extra traversal.
pub fn scan_project(root: &Path, extra: &[String]) -> ProjectScan {
    // The static half of the needle set, gathered once so the per-file loop is a
    // flat scan rather than a walk of the capability table per file.
    let mut needles: Vec<&str> = Vec::new();
    for c in CAPABILITIES {
        needles.extend(detection_types(c.id));
    }
    needles.extend(PHYSICS_MARKERS);
    // The http verbs, which used to be their own full walk of the project.
    needles.extend(["http_get", "http_post"]);
    needles.sort_unstable();
    needles.dedup();

    let mut scan = ProjectScan {
        extensions: Default::default(),
        found: Default::default(),
        saw_scene: false,
        files: Vec::new(),
        dirs: Default::default(),
    };

    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Dot-directories are the editor's own state (`.editor`,
                // `.cache`, `.renzora`) and version control. `.renzora/scripts`
                // in particular holds staged copies of the project's scripts,
                // which would be scanned twice for no gain.
                let dot = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with('.'));
                if !dot {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        scan.dirs.insert(name.to_ascii_lowercase());
                    }
                    stack.push(path);
                }
                continue;
            }
            if let Ok(rel) = path.strip_prefix(root) {
                scan.files.push(rel.to_string_lossy().replace('\\', "/"));
            }
            let Some(ext) = path.extension().and_then(|e| e.to_str()) else { continue };
            let ext = ext.to_ascii_lowercase();
            let scanned = SCANNED_EXTENSIONS.contains(&ext.as_str());
            scan.extensions.insert(ext.clone());
            if !scanned {
                continue;
            }
            if entry.metadata().map(|m| m.len()).unwrap_or(0) > MAX_SCANNED_FILE {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            if ext == "bsn" || ext == "ron" {
                scan.saw_scene = true;
            }
            for n in &needles {
                if !scan.found.contains(*n) && text.contains(*n) {
                    scan.found.insert((*n).to_string());
                }
            }
            for n in extra {
                if !scan.found.contains(n) && text.contains(n.as_str()) {
                    scan.found.insert(n.clone());
                }
            }
        }
    }
    scan.files.sort();
    scan
}

/// Default on/off per capability for a fresh export, from an existing scan.
///
/// # What the scan is allowed to decide
///
/// Every capability with a [`detection_extensions`] or [`detection_types`] entry
/// is answered by the project's own content: terrain is on because something in
/// a scene carries a terrain component, audio is on because a scene has an
/// emitter or a script calls `play_sound`. Everything else keeps its
/// `default_on`, which is the honest answer when nothing in the project can
/// distinguish the two cases.
///
/// This used to be extensions only, so the structural subsystems — the ones
/// worth megabytes — all defaulted ON and a 2D game shipped the whole 3D
/// pipeline unless its author went through the list by hand. The comment at the
/// top of this file still describes that policy as deliberate, and it was, for
/// as long as the alternative was guessing. Reading the scenes is not guessing.
///
/// # Where it refuses to decide
///
/// Three deliberate holes, each one a case where "no evidence" is not the same
/// as "not used":
///
/// * **No scene at all** ([`ProjectScan::saw_scene`]) — an empty or
///   just-created project says nothing about anything, so everything falls back
///   to `default_on`.
/// * **Neither pipeline detected** — a project whose scenes are pure UI has
///   evidence for no renderer, and turning both off would ship a game that
///   cannot draw. Both stay on.
/// * **Physics** — the two backends share their serialized components and their
///   script API, so the scan can see *that* physics is used and never *which*.
///   It follows the pipeline instead.
pub fn defaults_from_scan(
    selected_plugins: &[String],
    scan: Option<&ProjectScan>,
) -> HashMap<String, bool> {
    let Some(scan) = scan.filter(|s| s.saw_scene) else {
        // Nothing to go on: reproduce the pre-scan behaviour exactly.
        return CAPABILITIES
            .iter()
            .map(|c| {
                let on = match c.id {
                    "solari" => selected_plugins.iter().any(|p| p == "renzora_solari"),
                    _ => c.default_on,
                };
                (c.id.to_string(), on)
            })
            .collect();
    };

    let detected = |id: &str| -> Option<bool> {
        let exts = detection_extensions(id);
        let types = detection_types(id);
        if exts.is_empty() && types.is_empty() {
            return None;
        }
        Some(
            exts.iter().any(|e| scan.extensions.contains(*e))
                || types.iter().any(|t| scan.saw(t)),
        )
    };

    // Resolved first: physics and several others are answered in terms of them.
    let mut three_d = detected("render_3d").unwrap_or(true);
    let mut two_d = detected("render_2d").unwrap_or(true);
    if !three_d && !two_d {
        three_d = true;
        two_d = true;
    }
    let physics = scan.saw_any(PHYSICS_MARKERS);
    // The post-process stack as a whole is exactly "any of its effects", so it is
    // answered by its children rather than by markers of its own — there is no
    // such thing as a scene that uses post-processing but no effect.
    let any_postfx = CAPABILITIES
        .iter()
        .filter(|c| c.group == Some("postfx"))
        .any(|c| detected(c.id).unwrap_or(c.default_on));

    let mut state: HashMap<String, bool> = CAPABILITIES
        .iter()
        .map(|c| {
            let on = match c.id {
                // Follows its plugin, not the content: Solari is hardware
                // ray-tracing, and a scene that would use it looks like any
                // other lit scene.
                "solari" => selected_plugins.iter().any(|p| p == "renzora_solari"),
                "render_3d" => three_d,
                "render_2d" => two_d,
                "postfx" => any_postfx,
                // A named backend wins outright; otherwise physics rides the
                // pipeline, because that is what `auto_init_physics` does at
                // runtime when it picks a backend for an entity.
                "physics_3d" => scan.saw("avian3d::") || (physics && three_d),
                "physics_2d" => scan.saw("avian2d::") || (physics && two_d),
                "script_http" => scan.saw("http_get") || scan.saw("http_post"),
                // A project shipping its own packs wants the runtime that loads
                // them, whether or not its own code calls `t()`.
                "localization" => {
                    detected("localization").unwrap_or(true) || scan.saw_dir("languages")
                }
                _ => detected(c.id).unwrap_or(c.default_on),
            };
            (c.id.to_string(), on)
        })
        .collect();
    enforce_dependencies(&mut state);
    state
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

/// The three `[profile.dist-lean]` knobs, read off their capability toggles.
///
/// None of them is a Cargo *feature*, so they can't ride the normal strip path —
/// they are build-profile edits applied to the export copy's manifest by
/// [`crate::build::build_lean`]. They share the Features tab because the question
/// they answer is the same one ("what will you give up for a smaller binary?"),
/// and because each is stated as the thing you KEEP: on = the engine's default,
/// off = the smaller, worse-in-some-way build.
///
/// Missing keys default to on, so an older saved export config — or a state map
/// built before these existed — reproduces exactly the previous behaviour.
pub fn lean_profile(state: &HashMap<String, bool>) -> crate::build::LeanProfile {
    crate::build::LeanProfile {
        panic_abort: use_panic_abort(state),
        opt_level_z: !state.get("loop_vectorization").copied().unwrap_or(true),
        codegen_units_one: !state.get("parallel_codegen").copied().unwrap_or(true),
    }
}

/// Bevy features to strip from the export copy (union of OFF capabilities).
///
/// The render_3d cascade used to be re-listed here as three literal Bevy feature
/// names. It isn't any more: [`enforce_dependencies`] turns `antialiasing` and
/// `postfx` off with `render_3d`, and those two capabilities already name
/// `bevy_anti_alias` + `smaa_luts` and `bevy_post_process`, so the same set falls
/// out of the union. One rule, one place, and the Features tab agrees with it.
pub fn disabled_bevy_features(state: &HashMap<String, bool>) -> Vec<String> {
    let mut state = state.clone();
    enforce_dependencies(&mut state);
    collect_disabled(&state, |c| c.bevy_features)
}

/// `renzora_runtime` `default` features to strip from the export copy.
///
/// Enforces the one hard dependency between capabilities: the 3D subsystems
/// (terrain/water/sky/post-FX/spline) build on bevy_pbr, so when `render_3d` is
/// off they MUST be stripped too — otherwise the 2D build fails to compile. We do
/// it here (not via a Cargo feature dep, which would force render_3d back ON).
pub fn disabled_runtime_features(state: &HashMap<String, bool>) -> Vec<String> {
    // 3D text used to need a special case here: it was the one subsystem outside
    // the UI tree that still pulled `bevy_text`, so a UI-stripped runtime had to
    // drop it too. It is a native plugin now, and the glyph machinery it shares
    // with the UI emitter sits behind `renzora`'s `text_mesh` feature, which only
    // `renzora_ember` turns on — and ember is already gone when UI is off. The
    // dependency is expressed in the manifests now, so there is nothing to
    // enforce here.
    //
    // The render_3d cascade is no longer applied here: [`enforce_dependencies`]
    // has already written it into `state`, so it is one rule with one
    // implementation, and — the point of moving it — the Features tab shows the
    // same answer the build will use.
    let mut state = state.clone();
    enforce_dependencies(&mut state);
    collect_disabled(&state, |c| c.runtime_features)
}

/// Capabilities that cannot survive `render_3d` being off, because what they are
/// built out of is `bevy_pbr`.
///
/// This is a real build constraint, not a suggestion: keeping one of these while
/// 3D rendering is stripped does not produce a bigger binary, it produces one
/// that does not compile. [`enforce_dependencies`] therefore writes it into the
/// state rather than applying it silently at strip time, so the Features tab
/// cannot show a green toggle for something the build is about to drop — which
/// is exactly what it used to do for all twenty-odd of these.
///
/// `particles` is deliberately NOT here. It was, on the grounds that bevy_hanabi
/// reached for bevy_pbr in its asset path; it no longer does — the only mention
/// left in the vendored crate is a doc comment, and the whole 2D-particle path
/// (`plane_2d` effects, the 2D emitters) landed since. Verified by compiling a 2D
/// lean binary with `particles` kept, which is the only way to be sure of a claim
/// like that. Leaving it here silently dropped a 2D game's particle effects.
pub const RENDER_3D_DEPENDENTS: &[&str] = &[
    "terrain",
    "water",
    // the sky set
    "atmosphere",
    "environment_map",
    "skybox",
    // the post-process stack, parent included: bevy's own post-process pipeline
    // is part of what goes.
    "postfx",
    "bloom",
    "ssao",
    "ssr",
    "dof",
    "motion_blur",
    "distance_fog",
    "volumetric_fog",
    "lens_distortion",
    "oit",
    "antialiasing",
    // 3D-only extras that build on bevy_pbr
    "lumen",
    "cloth",
    "ragdoll",
    "parkour",
    "gaussian_splatting",
    "forward_decal",
    // Hardware ray tracing is bevy_pbr's, so it cannot outlive it either.
    "solari",
];

/// Apply the rules a capability state must obey, in place.
///
/// Two of them, and both only ever turn things **off**:
///
/// 1. A child cannot outlive its parent — see [`Capability::group`].
/// 2. Nothing in [`RENDER_3D_DEPENDENTS`] can outlive `render_3d`.
///
/// Off-only is deliberate. Both rules describe what the *build* will do, so
/// enforcing them keeps the dialog honest; the reverse ("3D is back on, so have
/// terrain again") would be the dialog inventing an intention, and turning a
/// subsystem on behind the user's back is the one direction that can make a
/// binary bigger than they asked for.
///
/// Called on the freshly detected defaults and again after every toggle, so what
/// the Features tab shows is what the export will build.
pub fn enforce_dependencies(state: &mut HashMap<String, bool>) {
    if !state.get("render_3d").copied().unwrap_or(true) {
        for id in RENDER_3D_DEPENDENTS {
            state.insert((*id).to_string(), false);
        }
    }
    // After the cascade above, so a child of a parent this just turned off goes
    // with it (`bloom` under `postfx`, for one).
    for c in CAPABILITIES {
        let Some(parent) = c.group else { continue };
        if !state.get(parent).copied().unwrap_or(true) {
            state.insert(c.id.to_string(), false);
        }
    }
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

    /// The three profile knobs are the only capabilities that change a build
    /// flag instead of a feature list, so nothing else in the strip path would
    /// catch a typo'd id — [`lean_profile`] would just read a missing key and
    /// silently default it to "keep".
    #[test]
    fn profile_knobs_have_matching_capabilities() {
        for id in ["panic_unwind", "loop_vectorization", "parallel_codegen"] {
            let cap = CAPABILITIES
                .iter()
                .find(|c| c.id == id)
                .unwrap_or_else(|| panic!("`{id}` is read by lean_profile but has no capability"));
            assert!(
                cap.bevy_features.is_empty() && cap.runtime_features.is_empty(),
                "`{id}` is a build-profile knob, not a feature strip",
            );
            assert!(cap.default_on, "`{id}` must default to the engine's setting");
        }
    }

    /// Defaults must reproduce the checked-in profile exactly: a fresh export,
    /// or one whose saved state predates these toggles, has to build the same
    /// binary it always did.
    #[test]
    fn a_default_state_asks_for_no_profile_changes() {
        let state = defaults_from_scan(&[], None);
        assert_eq!(lean_profile(&state), crate::build::LeanProfile::default());

        let mut smaller = state.clone();
        smaller.insert("panic_unwind".into(), false);
        smaller.insert("loop_vectorization".into(), false);
        smaller.insert("parallel_codegen".into(), false);
        let p = lean_profile(&smaller);
        assert!(p.panic_abort && p.opt_level_z && p.codegen_units_one);
    }

    // ── Content detection ───────────────────────────────────────────────────

    /// A throwaway project directory holding `files` as `(relative path,
    /// contents)`. Returned as a guard so the directory is removed even when an
    /// assertion fails partway through.
    struct TempProject(std::path::PathBuf);

    impl TempProject {
        fn new(tag: &str, files: &[(&str, &str)]) -> Self {
            let root = std::env::temp_dir().join(format!(
                "renzora_scan_{tag}_{}_{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&root);
            for (rel, body) in files {
                let path = root.join(rel);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, body).unwrap();
            }
            Self(root)
        }

        fn state(&self) -> HashMap<String, bool> {
            defaults_from_scan(&[], Some(&scan_project(&self.0, &[])))
        }
    }

    impl Drop for TempProject {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A 2D scene must not drag the 3D pipeline — or terrain, water, or the sky
    /// set — into the export. This is the case the whole scan exists for: it is
    /// most of the binary, and before the scan read scene contents every one of
    /// these defaulted ON.
    #[test]
    fn a_2d_project_strips_the_3d_pipeline() {
        let p = TempProject::new(
            "2d",
            &[(
                "scenes/level.bsn",
                "entity 1 {\n\
                 bevy_camera::components::Camera2d: (),\n\
                 renzora::core::components::Node2d: (),\n\
                 renzora::core::components::SpriteImagePath: (\"a.png\"),\n\
                 }\n",
            )],
        );
        let s = p.state();
        assert!(s["render_2d"], "the scene is plainly 2D");
        assert!(!s["render_3d"]);
        for id in ["terrain", "water", "skybox", "atmosphere", "lumen", "gltf"] {
            assert!(!s[id], "`{id}` has nothing in this project");
        }
    }

    /// The mirror image, so the scan cannot pass by answering "off" to
    /// everything.
    #[test]
    fn a_3d_project_keeps_the_3d_pipeline() {
        let p = TempProject::new(
            "3d",
            &[(
                "scenes/level.bsn",
                "entity 1 {\n\
                 bevy_camera::components::Camera3d: (),\n\
                 bevy_mesh::components::Mesh3d: (),\n\
                 renzora_terrain::data::TerrainData: (),\n\
                 }\n",
            )],
        );
        let s = p.state();
        assert!(s["render_3d"]);
        assert!(s["terrain"]);
        assert!(!s["water"], "one 3D subsystem must not imply the rest");
    }

    /// A subsystem reached only from a script survives, because the scan reads
    /// scripts too. Without this a game whose audio is entirely script-driven
    /// would ship with no audio backend and no error to explain it.
    #[test]
    fn a_subsystem_only_a_script_uses_is_still_detected() {
        let p = TempProject::new(
            "script",
            &[
                ("scenes/level.bsn", "entity 1 {\n bevy_ecs::name::Name: \"a\",\n}\n"),
                ("scripts/boom.lua", "function on_ready()\n  play_sound(\"bang\")\nend\n"),
            ],
        );
        let s = p.state();
        assert!(s["audio"], "`play_sound` is the only trace audio leaves");
        assert!(!s["navmesh"], "and it must not turn on everything else");
    }

    /// Physics has one set of components and one script API for two backends, so
    /// the dimension comes from the pipeline — which is what `auto_init_physics`
    /// does at runtime.
    #[test]
    fn physics_follows_the_pipeline() {
        let p = TempProject::new(
            "phys2d",
            &[(
                "scenes/level.bsn",
                "entity 1 {\n\
                 renzora::core::components::Node2d: (),\n\
                 renzora_physics::data::PhysicsBodyData: (),\n\
                 }\n",
            )],
        );
        let s = p.state();
        assert!(s["physics_2d"]);
        assert!(!s["physics_3d"], "a 2D game must not carry parry3d");
    }

    /// Physics nowhere in the project means neither backend, whatever the
    /// pipeline says.
    #[test]
    fn no_physics_means_neither_backend() {
        let p = TempProject::new(
            "nophys",
            &[("scenes/level.bsn", "entity 1 {\n bevy_mesh::components::Mesh3d: (),\n}\n")],
        );
        let s = p.state();
        assert!(!s["physics_2d"] && !s["physics_3d"]);
    }

    /// A scene with no renderer in evidence — a pure-UI menu project — keeps
    /// both pipelines. "No evidence" is not "not used", and a game that cannot
    /// draw is a worse outcome than a game that is a few MB larger.
    #[test]
    fn neither_pipeline_in_evidence_keeps_both() {
        let p = TempProject::new(
            "menu",
            &[(
                "scenes/menu.bsn",
                "entity 1 {\n bevy_ui::ui_node::Node: (),\n bevy_ecs::name::Name: \"root\",\n}\n",
            )],
        );
        let s = p.state();
        assert!(s["render_2d"] && s["render_3d"]);
        assert!(s["ui"]);
    }

    /// A project with no scenes at all tells us nothing, so every capability
    /// falls back to its engine default. A freshly created project must not
    /// export as an empty shell.
    #[test]
    fn a_project_with_no_scenes_keeps_the_engine_defaults() {
        let p = TempProject::new("empty", &[("readme.txt", "nothing here")]);
        let scan = scan_project(&p.0, &[]);
        assert!(!scan.saw_scene);
        let s = defaults_from_scan(&[], Some(&scan));
        for c in CAPABILITIES {
            if c.id == "solari" {
                continue; // follows its plugin, never the content
            }
            assert_eq!(s[c.id], c.default_on, "`{}` must fall back", c.id);
        }
    }

    /// The caller-supplied needles the plugin picker uses are found in the same
    /// pass, in scripts as well as scenes.
    #[test]
    fn extra_needles_come_back_from_scenes_and_scripts() {
        let p = TempProject::new(
            "plugins",
            &[
                ("scenes/level.bsn", "entity 1 {\n renzora_matrix::MatrixSettings: (),\n}\n"),
                ("scripts/go.lua", "local x = renzora_ripple::something\n"),
            ],
        );
        let extra = ["renzora_matrix::", "renzora_ripple::", "renzora_absent::"]
            .map(String::from)
            .to_vec();
        let scan = scan_project(&p.0, &extra);
        assert!(scan.saw("renzora_matrix::"));
        assert!(scan.saw("renzora_ripple::"), "scripts count too");
        assert!(!scan.saw("renzora_absent::"));
    }

    /// The editor's own directories hold staged copies of the project's scripts
    /// and caches of its scenes. Reading them would double the work and could
    /// resurrect a subsystem from a stale copy of a file the user has since
    /// changed.
    #[test]
    fn dot_directories_are_not_scanned() {
        let p = TempProject::new(
            "dotdirs",
            &[
                ("scenes/level.bsn", "entity 1 {\n renzora::core::components::Node2d: (),\n}\n"),
                (".renzora/scripts/old.lua", "play_sound(\"gone\")\n"),
            ],
        );
        assert!(!p.state()["audio"], "a staged copy under `.renzora` must not count");
    }

    /// Turning 3D rendering off must visibly take everything built on bevy_pbr
    /// with it. The build has always stripped these; the Features tab used to go
    /// on showing twenty-odd green toggles for things it was about to drop,
    /// which is the single most misleading thing the dialog did.
    #[test]
    fn render_3d_off_takes_its_dependents_with_it() {
        let mut state: HashMap<String, bool> =
            CAPABILITIES.iter().map(|c| (c.id.to_string(), true)).collect();
        state.insert("render_3d".into(), false);
        enforce_dependencies(&mut state);
        for id in RENDER_3D_DEPENDENTS {
            assert!(!state[*id], "`{id}` cannot outlive render_3d");
        }
        // …and only those: 2D and the systems layer are untouched.
        assert!(state["render_2d"] && state["light2d"] && state["audio"]);
    }

    /// A child follows its parent, including a parent the render_3d cascade just
    /// turned off — `bloom` under `postfx` is exactly that chain.
    #[test]
    fn a_child_cannot_outlive_its_parent() {
        let mut state: HashMap<String, bool> =
            CAPABILITIES.iter().map(|c| (c.id.to_string(), true)).collect();
        state.insert("ui".into(), false);
        enforce_dependencies(&mut state);
        assert!(!state["default_font"] && !state["game_ui"]);
        assert!(state["audio"], "an unrelated capability must be left alone");
    }

    /// Multiplayer is detected from the script API as well as from replicated
    /// entities: a game whose networking is entirely `rpc()` calls has nothing
    /// in its scenes to find.
    #[test]
    fn networking_is_detected_from_a_script_rpc_call() {
        let p = TempProject::new(
            "net",
            &[
                ("scenes/level.bsn", "entity 1 {\n bevy_ecs::name::Name: \"a\",\n}\n"),
                ("scripts/net.lua", "function on_ready()\n  rpc(\"hello\", {})\nend\n"),
            ],
        );
        assert!(p.state()["networking"]);

        let q = TempProject::new(
            "nonet",
            &[("scenes/level.bsn", "entity 1 {\n bevy_ecs::name::Name: \"a\",\n}\n")],
        );
        assert!(
            !q.state()["networking"],
            "a single-player game must not ship the UDP stack"
        );
    }

    /// A HUD must not turn world picking on.
    ///
    /// `bevy_ui` inserts `Pickable` and `PickingInteraction` on every node as
    /// required components, so any project with a UI saved hundreds of them into
    /// its scenes — matching on those kept the mesh and sprite raycast backends
    /// in every export ever made. UI hit-testing is a different bevy feature and
    /// rides the UI capability, so it is unaffected.
    #[test]
    fn a_ui_scene_does_not_turn_world_picking_on() {
        let p = TempProject::new(
            "picking",
            &[(
                "scenes/hud.bsn",
                "entity 1 {\n\
                 bevy_ui::ui_node::Node: (),\n\
                 bevy_picking::Pickable: (),\n\
                 bevy_picking::hover::PickingInteraction: None,\n\
                 renzora::core::components::Node2d: (),\n\
                 }\n",
            )],
        );
        assert!(!p.state()["picking"], "auto-inserted UI markers are not evidence");
        assert!(p.state()["ui"]);

        // A script that observes a pointer event is.
        let q = TempProject::new(
            "picking_yes",
            &[
                ("scenes/level.bsn", "entity 1 {\n renzora::core::components::Node2d: (),\n}\n"),
                ("scripts/click.rs", "fn on(_t: Trigger<Pointer<Click>>) {}\n"),
            ],
        );
        assert!(q.state()["picking"]);
    }

    /// The tonemapping lookup textures follow the curve the scene actually asks
    /// for. Five of the eight curves need no tables at all.
    #[test]
    fn tonemapping_luts_follow_the_curve_the_scene_names() {
        let plain = TempProject::new(
            "tm_none",
            &[(
                "scenes/level.bsn",
                "entity 1 {\n bevy_core_pipeline::tonemapping::Tonemapping: None,\n}\n",
            )],
        );
        assert!(!plain.state()["tonemapping_luts"]);

        let lut = TempProject::new(
            "tm_lut",
            &[(
                "scenes/level.bsn",
                "entity 1 {\n bevy_core_pipeline::tonemapping::Tonemapping: TonyMcMapface,\n}\n",
            )],
        );
        assert!(lut.state()["tonemapping_luts"]);
    }

    /// Translation follows a real call or the project's own packs — never a bare
    /// `t(`, which matches `print(`, `expect(` and half of every Rust file.
    #[test]
    fn localization_needs_a_real_signal() {
        let quiet = TempProject::new(
            "loc_no",
            &[
                ("scenes/level.bsn", "entity 1 {\n bevy_ecs::name::Name: \"a\",\n}\n"),
                // Full of `t(` and meaning none of it.
                ("scripts/a.rs", "fn go(w: &mut World) { let x = w.get_mut::<T>(e).expect(\"x\"); }\n"),
            ],
        );
        assert!(!quiet.state()["localization"], "`expect(` is not a translation");

        let calls = TempProject::new(
            "loc_call",
            &[
                ("scenes/level.bsn", "entity 1 {\n bevy_ecs::name::Name: \"a\",\n}\n"),
                ("scripts/a.rs", "let s = renzora::lang::t(\"hud.score\");\n"),
            ],
        );
        assert!(calls.state()["localization"]);

        // A project shipping its own packs wants the runtime that loads them,
        // whatever its own code does.
        let packs = TempProject::new(
            "loc_packs",
            &[
                ("scenes/level.bsn", "entity 1 {\n bevy_ecs::name::Name: \"a\",\n}\n"),
                ("languages/fr.toml", "\"hud.score\" = \"Score\"\n"),
            ],
        );
        assert!(packs.state()["localization"]);
    }

    /// Every id named by the detection tables must be a real capability, or the
    /// entry is a rule that can never fire.
    #[test]
    fn detection_tables_only_name_real_capabilities() {
        for c in CAPABILITIES {
            let _ = detection_extensions(c.id);
            let _ = detection_types(c.id);
        }
        // The reverse direction is what actually goes stale: a capability
        // renamed, with its detection entry left behind under the old id.
        for id in ["render_3d", "render_2d", "audio", "terrain", "ui", "game_ui", "networking"] {
            assert!(
                CAPABILITIES.iter().any(|c| c.id == id),
                "`{id}` is named by a detection table but is not a capability"
            );
            assert!(
                !detection_types(id).is_empty(),
                "`{id}` lost its detection entry"
            );
        }
    }
}





