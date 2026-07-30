# Post-Processing Effects

A full-screen camera effect is a small Rust struct plus a WGSL shader. Since alpha7 it is also a [standalone plugin](./standalone-plugins.md): it links no Bevy, builds with any toolchain in about a second, and hot-reloads — shader included — while the editor runs.

Every effect the engine ships lives in `plugins/`. There are 53 of them and they are all the same three files.

## The three files

Taking `plugins/ascii` verbatim.

### 1. `Cargo.toml`

```toml
[package]
name = "ascii"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
renzora_plugin = "0.1"
```

One dependency. No Bevy, no workspace, no engine checkout.

### 2. `src/lib.rs`

```rust
use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("ascii.wgsl");

#[derive(Component)]
#[repr(C)]
pub struct Ascii {
    #[field(min = 2.0, max = 32.0, speed = 0.5)]
    pub char_size: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub color_mix: f32,
    #[field(min = 0.5, max = 3.0, speed = 0.01)]
    pub contrast: f32,
}

impl Default for Ascii {
    fn default() -> Self {
        Self { char_size: 8.0, color_mix: 0.5, contrast: 1.2 }
    }
}

pub struct AsciiPlugin;

impl Plugin for AsciiPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<Ascii>("ascii", WGSL, RenderPhase::LdrPost, 0.0);
    }
}

renzora_plugin::add!(AsciiPlugin);
```

`add_post_process::<T>(id, wgsl, phase, order)` is the whole registration:

| Argument | Meaning |
|---|---|
| `id` | Stable name, used for the pass and for shader hot-reload |
| `wgsl` | The shader source. `include_str!` rather than an embedded asset, so editing the `.wgsl` is a source change the [watcher](./standalone-plugins.md#hot-reload) already sees |
| `phase` | Where in the frame it runs — see below |
| `order` | Sort key within the phase; lower runs first |

The struct is registered as an ordinary plugin component, so it appears in the inspector's add-component list with one control per field, laid out from the `#[field]` ranges. `#[field(skip)]` keeps a value in the struct and out of the inspector — `plugins/sepia` uses it for tone weights the shader reads every pixel but nobody should be dragging.

### 3. `src/ascii.wgsl`

```wgsl
@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct AsciiSettings {
    char_size: f32,
    color_mix: f32,
    contrast: f32,
};
@group(0) @binding(2) var<uniform> settings: AsciiSettings;

@fragment
fn fragment(@builtin(position) pos: vec4<f32>, @location(0) in_uv: vec2<f32>) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(screen_texture));
    let cell = vec2(settings.char_size) / dims;
    let cell_center = (floor(in_uv / cell) + 0.5) * cell;
    let cell_color = textureSample(screen_texture, texture_sampler, cell_center);

    let lum = clamp(dot(cell_color.rgb, vec3(0.299, 0.587, 0.114)) * settings.contrast, 0.0, 1.0);
    let result = mix(vec3(lum), cell_color.rgb, settings.color_mix);
    return vec4(result, 1.0);
}
```

## The shader contract

- **It must be self-contained.** A plugin's WGSL is compiled without naga_oil, so `#import` is not available — it is not valid WGSL, and the import path a Bevy-linked effect used doesn't exist here. Declare the fullscreen inputs yourself, as above. Anything you would have imported, paste in.
- **Write the `@fragment` entry point only**, named `fragment`. The vertex stage is the engine's fullscreen triangle. Its output arrives as `@builtin(position)` plus `@location(0)` UV — name that second parameter something other than `uv` if the body wants `uv` as a local, which is why the shipped effects call it `in_uv`.
- **All bindings are `@group(0)`**: 0 is the source colour texture, 1 its sampler, 2 the settings uniform. Binding 2 is required — a shader that declares no uniform there is rejected at registration with that message rather than failing later in the pipeline cache.
- **The WGSL struct mirrors the Rust struct in order and size.** Field names are arbitrary. There is no padding to count and no trailing `enabled` flag: write exactly the fields the effect needs, in any number, and the host rounds the uniform buffer up to a 16-byte multiple on its own. Switching an effect off is removing the component, which is what `enabled` used to stand in for.

## Render phases

`RenderPhase` says where in the frame the pass runs; `order` sorts within it.

| Phase | Image | Typical use |
|---|---|---|
| `Gi` | HDR, after the main 3D pass, before temporal AA | global illumination, reflections |
| `HdrPost` | HDR, after temporal AA | bloom, depth of field, motion blur |
| `LdrPost` | LDR, after tonemapping | colour grading, stylisation, vignette |
| `Overlay` | final, after AA | letterboxing, screen transitions, UI-adjacent effects |

Most stylistic effects want `LdrPost`. Anything that needs values above 1.0 wants `HdrPost` or earlier.

This is the same [render composition](../rendering/pipeline.md) registry engine passes use, so a plugin effect and an engine pass sort against each other in one list rather than living in separate systems.

## Where the settings component goes

Put it on any entity — the effect is global, and the bridge finds the first instance in the world each frame and uploads its bytes. It does **not** have to be on the camera, and there is no routing table to configure. In the editor, adding the component from the inspector is all there is to it.

The consequence to know: a second entity carrying the same effect component does nothing. One effect, one set of settings.

## Hot reload

Both halves reload. Rebuild the plugin and:

- **Editing a field's value or the shader body** takes effect without a restart. The WGSL is overwritten at the handle the pipeline was built against, so the pipeline cache invalidates and recompiles.
- **Adding or removing a struct field** is refused, with the reason — entities already carrying the component were allocated for the old layout. Restart to pick it up.
- **Adding a whole new effect** to a plugin that already registered one also needs a restart; the render pass is installed once.

A shader that fails to compile leaves the previous one running.

## Performance

- **Inactive effects cost nothing.** A pass returns immediately when no entity carries its component — no pipeline bind, no pass.
- A pipeline variant is built per target format and chosen at render time, so an effect works in both HDR and LDR views without you writing two shaders.
- Each active effect is one fullscreen pass. Fewer, fatter shaders beat many thin ones — fold related work into a single effect where you can.

## Effects that are not plugins

Two families deliberately stay in-tree, compiled against real Bevy:

| Family | Examples | Why |
|---|---|---|
| **Bevy built-in wrappers** | bloom, DOF, SSAO, SSR, motion blur, auto-exposure, atmosphere, skybox, vignette, fog, FXAA/SMAA/TAA/CAS | They author user-facing settings and route a **stock Bevy component** onto the camera. There is no custom WGSL pass to move, and the ABI cannot express a Bevy component. |
| **Multi-pass render-graph crates** | `renzora_lumen`, `renzora_rt`, `renzora_oit`, `renzora_solari` | Their own graph nodes, multiple passes, custom bind groups and extra textures. |

The line is a single fullscreen fragment pass over the current image. If that is what your effect is, it belongs in `plugins/`. If it needs a second input texture, a previous-frame snapshot or its own bind-group layout, it needs a Bevy-linked crate — the ABI has no way to hand you those yet.

`renzora_macros::post_process` and the `PostProcessEffect` trait still exist for that path, but nothing in the tree uses them any more: all 53 unified effects moved to the C ABI, and the remaining in-tree effects are wrappers or graph crates that never used the macro.
