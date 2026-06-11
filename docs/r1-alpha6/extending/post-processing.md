# Post-Processing Effects

Author full-screen camera effects as a small Rust struct plus a WGSL shader, and let the `#[post_process]` macro wire the whole pipeline for you.

## How the pipeline works

Renzora runs every post-process effect through **one** render-graph node. The framework lives in `renzora::postprocess` (re-exported through the `renzora_postprocess` shim crate), so it ships inside `renzora.dll` and all the effect plugins share a single `PostProcessRegistry` and matching `TypeId`s across the dynamic-plugin boundary.

- A single `UnifiedPostProcessNode` is inserted into the `Core3d` sub-graph **between `Node3d::Tonemapping` and `Node3d::EndMainPassPostProcessing`**.
- Each effect registers a type-erased handler. The handler runs only when its settings component is present on the view (a `Camera3d`), so **inactive effects have zero render-graph overhead** — there is no per-effect node and nothing executes for effects you didn't add.
- Effects are ordinary `bevy_ui`/ECS components. The extract step is filtered with `With<Camera3d>`, so the settings live on (or are routed to) a 3D camera.

### Getting settings onto the camera

You can put a settings component directly on your `Camera3d`, or author it on any entity and let the engine proxy it onto active cameras through the **`EffectRouting`** table (`renzora::EffectRouting`, `crates/renzora/src/core/mod.rs`). The routing table maps each target camera to a list of source entities; it's refreshed every frame (by the viewport crate in the editor, by `renzora_engine` at runtime). `PostProcessPlugin::<T>` adds a system that copies the component from a routed source onto the camera and removes it again when the source goes away. This is how the editor's inspector drives effects on the viewport camera without you touching the camera entity yourself.

## Effect families

Not everything called a "camera effect" goes through the unified node. There are three structural families:

| Family | Crates | How it renders |
|---|---|---|
| **Unified post-process** | ~53 effect crates (`renzora_ascii`, `renzora_vignette`, …) | A fullscreen fragment pass in the single `UnifiedPostProcessNode`. This is what `#[post_process]` / `PostProcessEffect` produce. |
| **Bevy built-in wrappers** | `bloom`, `dof`, `ssao`, `ssr`, `motion_blur`, `auto_exposure`, `atmosphere`, `skybox`, `environment_map`, `forward_decal`, fog, antialiasing (FXAA/SMAA/TAA/CAS) | Author user-facing settings, then route a **stock Bevy component** (`Bloom`, `DepthOfField`, `ScreenSpaceAmbientOcclusion`, …) onto the camera via `EffectRouting`. No custom WGSL pass. |
| **Multi-pass render-graph crates** | `renzora_lumen`, `renzora_rt` (GI), `renzora_oit` (transparency) | Their own render-graph nodes and passes — not part of the unified pipeline. |

This page covers the first family. Of the ~53 unified effects, **45 use the `#[post_process]` macro** and **8 implement `PostProcessEffect` by hand** (`vignette`, `outline`, `gaussian_blur`, `god_rays`, `edge_glow`, `palette_quantization`, `underwater`, `screen_transition` — these need a custom bind-group layout, an extra texture, or a two-image snapshot).

> The unified effects are normal removable plugins. Each is typically its own `cdylib` distribution plugin dropped into the engine's `plugins/` directory, registered with `renzora::add!`.

## The `PostProcessEffect` trait

Every unified effect implements this trait (from `renzora_postprocess`, i.e. `renzora::postprocess`):

```rust
pub trait PostProcessEffect:
    Component + ExtractComponent + Clone + Copy + ShaderType + WriteInto + Default + 'static
{
    fn fragment_shader() -> ShaderRef;

    // Optional, for advanced effects:
    fn has_extra_texture() -> bool { false }       // bind a second texture + sampler
    fn extra_texture_is_snapshot() -> bool { false } // that texture is a frozen previous frame
    fn freeze_snapshot(&self) -> bool { false }    // stop refreshing the snapshot this frame
}
```

You almost never write this trait yourself — the `#[post_process]` macro generates the component, the `Default` impl, and `fragment_shader()` for you. The optional methods exist for the hand-written effects (see *Hand-written effects* below).

## Authoring an effect with `#[post_process]`

A complete effect is a `cdylib` crate with three things: a `Cargo.toml`, an annotated struct + plugin (`lib.rs`), and a WGSL shader. The real `renzora_ascii` crate is used verbatim below.

### 1. `Cargo.toml`

```toml
[package]
name = "renzora_ascii"
version = "0.1.0"
edition = "2021"

# Distribution plugin: ships as a standalone .dll/.so/.dylib that
# dynamic_plugin_loader picks up from the engine's plugins/ directory.
[lib]
crate-type = ["cdylib"]

[features]
default = ["editor", "dlopen"]
dlopen = []
editor = ["renzora/editor"]

[dependencies]
renzora_postprocess = { path = "../renzora_postprocess" }
renzora_editor_framework = { path = "../renzora_editor_framework" }
bevy = { workspace = true }
serde = { version = "1", features = ["derive"] }
renzora = { path = "../renzora", default-features = false }
renzora_macros = { path = "../renzora_macros" }
```

The `editor` feature turns on `renzora/editor`, which is what makes the inspector registration (`register_inspectable`, `FieldDef`, …) available. Without it the effect still renders — it just won't appear in the editor inspector.

### 2. The struct and plugin (`src/lib.rs`)

```rust
use bevy::prelude::*;
#[cfg(feature = "editor")]
use renzora_editor_framework::AppEditorExt;

#[renzora_macros::post_process(shader = "ascii.wgsl", name = "ASCII", icon = "TEXT_AA")]
pub struct AsciiSettings {
    #[field(speed = 0.5, min = 2.0, max = 32.0, default = 8.0)]
    pub char_size: f32,
    #[field(speed = 0.01, min = 0.0, max = 1.0, default = 0.5)]
    pub color_mix: f32,
    #[field(speed = 0.01, min = 0.5, max = 3.0, default = 1.2)]
    pub contrast: f32,
}

#[derive(Default)]
pub struct AsciiPlugin;

impl Plugin for AsciiPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "ascii.wgsl");
        app.register_type::<AsciiSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<AsciiSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspectable::<AsciiSettings>();
    }
}

renzora::add!(AsciiPlugin);
```

The four lines in `build()` are the whole contract:

1. `embedded_asset!(app, "ascii.wgsl")` embeds the shader (resolved to `embedded://renzora_ascii/ascii.wgsl`). Put the `.wgsl` next to `lib.rs` in `src/`.
2. `register_type::<AsciiSettings>()` registers it for Bevy reflection (scene save/load + script reflection).
3. `PostProcessPlugin::<AsciiSettings>::default()` builds the per-effect GPU pipeline and registers the handler with the unified node. It's idempotent about the shared core node — only the first effect installs it.
4. `register_inspectable::<AsciiSettings>()` (editor only) adds the auto-generated inspector card.

Then `renzora::add!(AsciiPlugin)` self-registers the plugin. (No scope argument means `Runtime` — the effect runs in both the editor and exported games.)

> Import the trait directly: `use renzora_editor_framework::AppEditorExt;`. There is **no `renzora::prelude`** — use `use renzora::*;` or import individual items.

### 3. The WGSL shader (`src/ascii.wgsl`)

```wgsl
#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct AsciiSettings {
    char_size: f32,
    color_mix: f32,
    contrast: f32,
    _padding0: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    enabled: f32,
};
@group(0) @binding(2) var<uniform> settings: AsciiSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    if settings.enabled < 0.5 {
        return color;
    }

    let dims = vec2<f32>(textureDimensions(screen_texture));
    let cell = vec2(settings.char_size) / dims;
    let cell_center = (floor(in.uv / cell) + 0.5) * cell;
    let cell_color = textureSample(screen_texture, texture_sampler, cell_center);

    let lum = clamp(dot(cell_color.rgb, vec3(0.299, 0.587, 0.114)) * settings.contrast, 0.0, 1.0);
    let mono = vec3(lum);
    let result = mix(mono, cell_color.rgb, settings.color_mix);
    return vec4(result, color.a);
}
```

Three things make this work:

- The vertex stage comes from Bevy's `FullscreenVertexOutput` — you only write the `@fragment` entry point named `fragment`.
- All bindings are in **`@group(0)`**: binding 0 is the source color texture, binding 1 its sampler, binding 2 the uniform. (The old "uniform in `@group(1)`" convention is wrong.)
- The uniform struct must match the component's GPU layout **byte for byte**, including the macro-added padding and the trailing `enabled` field (see below). Field *names* in WGSL are arbitrary — only the order and sizes matter. Check `settings.enabled < 0.5` and early-return the untouched color so the inspector's enable/disable toggle works.

## What the macro generates

`#[post_process(...)]` rewrites your struct into the full component. For `AsciiSettings` above it emits roughly:

```rust
#[derive(Component, Clone, Copy, Reflect, serde::Serialize, serde::Deserialize,
         bevy::render::render_resource::ShaderType,
         bevy::render::extract_component::ExtractComponent)]
#[reflect(Component, Serialize, Deserialize)]
#[extract_component_filter(With<Camera3d>)]
pub struct AsciiSettings {
    pub char_size: f32,
    pub color_mix: f32,
    pub contrast: f32,
    #[serde(skip, default)] pub _padding1: f32,
    #[serde(skip, default)] pub _padding2: f32,
    #[serde(skip, default)] pub _padding3: f32,
    #[serde(skip, default)] pub _padding4: f32,
    pub enabled: f32,
}
// + Default (honoring #[field(default = ...)], enabled = 1.0)
// + impl PostProcessEffect { fragment_shader() -> "embedded://renzora_ascii/ascii.wgsl" }
// + (feature = "editor") impl InspectableComponent -> InspectorEntry
```

Key points:

- It appends an **`enabled: f32`** field at the end (1.0 = on). Your shader reads this to gate the effect.
- It pads the struct with serde-skipped `f32`s up to a **minimum of 8 floats (two `vec4`s)**, rounding up to a multiple of four beyond that. Padding is `#[serde(skip)]` so scene files don't break when the layout changes. This is why the WGSL `struct` has the extra `_padding*` members before `enabled`.
- It derives `ShaderType` + `ExtractComponent` (filtered `With<Camera3d>`) and `Reflect`/`Serialize`/`Deserialize`.
- Under `feature = "editor"` it generates the entire `InspectorEntry`: display name, icon, add/remove, the enable toggle, and one field control per `#[field]`.

> The padding math counts **one float per field**. Keep `#[post_process]` fields as scalar `f32` (the overwhelming common case) so the auto-padding stays correct; if you need `Vec3`/`Vec4` uniforms, implement `PostProcessEffect` by hand and lay out the struct yourself.

### Macro attributes

`#[post_process(...)]`:

| Key | Required | Default | Meaning |
|---|---|---|---|
| `shader` | yes | — | Embedded shader path, relative to `embedded_asset!` (e.g. `"ascii.wgsl"`). |
| `name` | no | title-cased struct name | Display name in the inspector. |
| `icon` | no | `"SPARKLE"` | Phosphor icon name (e.g. `"TEXT_AA"`); converted to kebab-case for the native inspector. |
| `category` | no | `"post_process"` | Inspector grouping/category. |
| `type_id` | no | snake_case of the struct name, with a trailing `_settings` stripped | Stable id used by the inspector and reflection (`AsciiSettings` → `ascii`). |

`#[field(...)]` on each user field:

| Key | Applies to | Meaning |
|---|---|---|
| `default` | f32 | Initial value in the generated `Default` impl. |
| `speed` | Float / Vec3 | Drag sensitivity in the inspector. |
| `min` / `max` | Float | Slider/drag bounds. |
| `name` | any | Override the field's display label. |
| `skip` | any | Omit this field from the inspector. |
| `readonly` | any | Show as a read-only debug value. |

Supported field types are inferred from the Rust type: `f32` → Float, `bool` → Bool, `Vec3` → Vec3, plus String and Color.

## Hand-written effects

When you need a custom bind-group layout, a second input texture, or a previous-frame snapshot, implement `PostProcessEffect` directly instead of using the macro. `renzora_vignette` is the canonical example — it lays out the struct (with its own padding + trailing `enabled`) and builds the `InspectorEntry` by hand:

```rust
use bevy::prelude::*;
use bevy::render::{extract_component::ExtractComponent, render_resource::ShaderType};
use bevy::shader::ShaderRef;
use serde::{Deserialize, Serialize};
#[cfg(feature = "editor")]
use renzora_editor_framework::{AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry};
use renzora_postprocess::PostProcessEffect;

#[derive(Component, Clone, Copy, Reflect, Serialize, Deserialize, ShaderType, ExtractComponent)]
#[reflect(Component, Serialize, Deserialize)]
#[extract_component_filter(With<Camera3d>)]
pub struct VignetteSettings {
    pub intensity: f32,
    pub radius: f32,
    pub smoothness: f32,
    pub color_r: f32,
    pub color_g: f32,
    pub color_b: f32,
    pub _padding1: f32,
    pub enabled: f32,
}

impl PostProcessEffect for VignetteSettings {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_vignette/vignette.wgsl".into()
    }
}

#[derive(Default)]
pub struct VignettePlugin;

impl Plugin for VignettePlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "vignette.wgsl");
        app.register_type::<VignetteSettings>();
        app.add_plugins(renzora_postprocess::PostProcessPlugin::<VignetteSettings>::default());
        #[cfg(feature = "editor")]
        app.register_inspector(inspector_entry()); // build an InspectorEntry by hand
    }
}

renzora::add!(VignettePlugin);
```

Two registration calls exist on `AppEditorExt`:

- `register_inspectable::<T>()` — for macro-generated effects (the macro emits an `InspectableComponent` impl).
- `register_inspector(entry)` — for hand-written effects, where you supply the `InspectorEntry` (with `FieldDef`/`FieldType`/`FieldValue` controls) yourself.

### Extra texture / snapshot effects

For effects that sample a second image, override `has_extra_texture()` to return `true`. The pipeline then adds two more bindings after the uniform:

```wgsl
@group(0) @binding(3) var extra_texture: texture_2d<f32>;
@group(0) @binding(4) var extra_sampler: sampler;
```

Set `ExtraTextureSource::<T>` (a main-world resource) to the `Handle<Image>` you want bound. If you also override `extra_texture_is_snapshot()` to `true`, the framework maintains a per-view snapshot of the previous fully-composited frame and binds it as the extra texture — and `freeze_snapshot(&self)` lets you stop refreshing it (return `true`) while a transition blends the frozen outgoing frame against the live one. This is how `screen_transition` works.

## Effect ordering

There is **no per-effect priority API** for the fullscreen passes (the old `add_post_process_with_priority` does not exist, and `node_edges()`/`node_label()` on the trait are dead methods). All effects run inside the one unified node, in the order their `PostProcessPlugin::<T>` registered its handler — i.e. **plugin registration order**. You influence that with the `add!` priority on the plugin:

```rust
renzora::add!(AsciiPlugin, Runtime, priority = 100); // higher priority installs later -> runs later
```

The whole unified node is fixed between tonemapping and the end of main-pass post-processing, so all your effects see the already-tonemapped HDR/LDR image.

## Performance

- **Inactive effects cost nothing.** A handler returns immediately when the camera lacks the component — there is no node, pipeline bind, or pass for effects you didn't add.
- Both an LDR and an HDR pipeline variant are built per effect; the correct one is chosen from the view's HDR state at render time.
- Each active effect is one fullscreen pass (`post_process_write` ping-pong). Fewer, fatter shaders beat many thin passes — fold related work into a single effect where you can.
- Always honor `enabled` in the shader so the inspector toggle is a cheap in-shader early-out rather than requiring the component to be removed.
