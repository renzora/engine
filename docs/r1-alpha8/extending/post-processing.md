# Post-Processing Effects

A full-screen camera effect is a small Rust struct plus a WGSL shader, shipped as a [native plugin](native-plugins.md). It hot-reloads — shader included — while the editor runs.

Every effect the engine ships is installed rather than built in, and they are all the same three files.

## The three files

Taking the `ascii` effect verbatim.

### 1. `Cargo.toml`

```toml
[workspace]

[package]
name = "ascii"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["dylib"]

[dependencies]
bevy = "0.19"
renzora = "0.1"
```

> **Do not run `cargo build` here.** `plugins/` sits outside the engine workspace, so cargo would resolve a second Bevy from crates.io and the plugin that came out would have different `TypeId`s from the engine — it would load, run, and corrupt the World. The editor compiles it against the staged SDK; see [Native Plugins](native-plugins.md).

### 2. `src/lib.rs`

```rust
use bevy::prelude::*;
use renzora::{post_process, AppEditorExt};

#[post_process(shader = "ascii.wgsl", name = "ASCII", icon = "text-aa")]
pub struct Ascii {
    #[field(min = 2.0, max = 32.0, speed = 0.5, default = 8.0)]
    pub char_size: f32,
    #[field(min = 0.0, max = 1.0, speed = 0.01, default = 0.5)]
    pub color_mix: f32,
    #[field(min = 0.5, max = 3.0, speed = 0.01, default = 1.2)]
    pub contrast: f32,
}

#[derive(Default)]
pub struct AsciiPlugin;

impl Plugin for AsciiPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "ascii.wgsl");
        app.add_plugins(renzora::postprocess::PostProcessPlugin::<Ascii>::default());
        app.register_inspectable::<Ascii>();
    }
}

renzora::plugin!(AsciiPlugin, Runtime);
```

`#[post_process]` is the whole of it. From that one attribute the macro writes:

| It generates | Why you do not |
|---|---|
| `Component`, `Reflect` and the derives around them | every effect needs the same set |
| a `Default` impl from the `default =` values | so the inspector's "add component" gives a usable effect, not zeros |
| an `enabled` field, and padding before it | the uniform has to reach a 16-byte multiple, and the arithmetic is easy to get wrong by hand |
| the `PostProcessEffect` impl | it is the same four lines every time |
| the inspector section, laid out from the `#[field]` ranges | one control per field, with the right limits |

| Attribute argument | Meaning |
|---|---|
| `shader` | The `.wgsl` beside `lib.rs`, loaded through `embedded_asset!` |
| `name` | What the inspector calls the effect |
| `icon` | Its glyph in the add-component list |
| `order` | Optional sort key within the render phase; lower runs first |

`#[field(skip)]` keeps a value in the struct and out of the inspector — useful for a tone weight the shader reads every pixel but nobody should be dragging.

### 3. `src/ascii.wgsl`

```wgsl
@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

// Must match the struct `#[post_process]` generates, field for field: the user
// fields in declaration order, then padding out to a 16-byte multiple, then
// `enabled` last. Nothing checks this at run time.
struct AsciiSettings {
    char_size: f32,
    color_mix: f32,
    contrast: f32,
    _padding1: f32,
    _padding2: f32,
    _padding3: f32,
    _padding4: f32,
    enabled: f32,
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

- **Write the `@fragment` entry point only**, named `fragment`. The vertex stage is the engine's fullscreen triangle. Its output arrives as `@builtin(position)` plus `@location(0)` UV — name that second parameter something other than `uv` if the body wants `uv` as a local, which is why the shipped effects call it `in_uv`.
- **All bindings are `@group(0)`**: 0 is the source colour texture, 1 its sampler, 2 the settings uniform. Bindings 3 and 4 are an optional extra texture and its sampler; a shader that declares either without the effect asking for one is a wgpu validation crash at the moment it first renders, not a compile error.
- **The WGSL struct mirrors the generated Rust struct exactly**: your fields in declaration order, then padding, then `enabled` last. The padding count is `max(8, ceil((fields + 1) / 4) * 4) - fields - 1`. Field names are arbitrary; order and size are not.
- **Respect `enabled`.** It is `0.0` when the effect is switched off in the inspector. Return the sampled colour unchanged rather than running the effect.

## Render phases

The phase says where in the frame the pass runs; `order` sorts within it.

| Phase | Image | Typical use |
|---|---|---|
| `Gi` | HDR, after the main 3D pass, before temporal AA | global illumination, reflections |
| `HdrPost` | HDR, after temporal AA | bloom, depth of field, motion blur |
| `LdrPost` | LDR, after tonemapping | colour grading, stylisation, vignette |
| `Overlay` | final, after AA | letterboxing, screen transitions, UI-adjacent effects |

Most stylistic effects want `LdrPost`. Anything that needs values above 1.0 wants `HdrPost` or earlier.

This is the same [render composition](../rendering/pipeline.md) registry engine passes use, so a plugin effect and an engine pass sort against each other in one list rather than living in separate systems.

## Where the settings component goes

Put it on any entity — the effect is global, and the pass finds the first instance in the world each frame. It does **not** have to be on the camera, and there is no routing table to configure. In the editor, adding the component from the inspector is all there is to it.

The consequence to know: a second entity carrying the same effect component does nothing. One effect, one set of settings.

## Animated effects tick their own clock

There is no `time` binding and no globals uniform. The pass uploads the settings component's bytes verbatim — it never interprets a field, so a field called `time` is not special and nothing in the engine writes to it. An effect that animates needs an `f32` in its struct and a three-line system to advance it:

```rust
#[post_process(shader = "film_grain.wgsl", name = "Film Grain", icon = "film")]
pub struct FilmGrain {
    #[field(min = 0.0, max = 2.0, speed = 0.01, default = 0.5)]
    pub intensity: f32,
    #[field(skip)]
    pub time: f32,
}

fn sync_time(mut q: Query<&mut FilmGrain>, time: Res<Time>) {
    for mut g in &mut q {
        g.time += time.delta_secs();
        if g.time > 1024.0 {
            g.time -= 1024.0;   // keep the f32 seed small
        }
    }
}

// ...
app.add_systems(Update, sync_time);
```

`#[field(skip)]` only hides the value from the inspector. It does not mean "engine-driven" — reading it that way is what left several effects frozen at `0.0`, animating nothing while their shaders faithfully sampled a clock that never moved.

Two details worth copying:

- **Wrap the accumulator** instead of assigning `time.elapsed_secs()`. An f32 second count stops resolving small steps after a few hours of uptime.
- **Feed time as its own hash axis**, not as an offset added to the sample coordinate. Offsetting the coordinate translates one fixed noise field, which looks like a sheet of dirt sliding over the image rather than something regenerating in place.

## Effects that need the previous frame

An effect that fades *from* what was on screen — a screen transition, a dissolve — needs a snapshot as a second input. Ask for one in the attribute:

```rust
#[post_process(
    shader = "screen_transition.wgsl",
    name = "Screen Transition",
    icon = "arrows-left-right",
    snapshot = true,
    frozen_when = "self.progress < 1.0",
)]
pub struct ScreenTransition {
    #[field(min = 0.0, max = 1.0, speed = 0.01, default = 0.0)]
    pub progress: f32,
}
```

`snapshot = true` binds the captured texture at `@binding(3)` and its sampler at `@binding(4)`. `frozen_when` is an expression on `&self` that says when the snapshot should stop updating, so the effect fades from a fixed image rather than from a moving one.

Declaring the bindings in WGSL **without** `snapshot = true` is the failure mode to know: the pipeline layout has no binding 3, and wgpu rejects the pass with "binding 3 is not available in the pipeline layout" the first time that effect renders.

## Hot reload

Both halves reload. Save and:

- **Editing a field's value or the shader body** takes effect without a restart. The plugin's stamp stops matching, the editor rebuilds it, and the pipeline cache recompiles.
- **Adding or removing a struct field** changes the uniform layout, so the WGSL struct has to change with it. Get the padding wrong and the effect reads its own fields at the wrong offsets.

A shader that fails to compile leaves the previous one running.

## Performance

- **Inactive effects cost nothing.** A pass returns immediately when no entity carries its component — no pipeline bind, no pass.
- A pipeline variant is built per target format and chosen at render time, so an effect works in both HDR and LDR views without you writing two shaders.
- Each active effect is one fullscreen pass. Fewer, fatter shaders beat many thin ones — fold related work into a single effect where you can.

## Effects that are not `#[post_process]`

Two families stay in the engine, and the line between them and a plugin effect is whether the effect is **one fullscreen fragment pass over the current image**:

| Family | Examples | Why |
|---|---|---|
| **Bevy built-in wrappers** | bloom, DOF, SSAO, SSR, motion blur, auto-exposure, atmosphere, skybox, vignette, fog, FXAA/SMAA/TAA/CAS | They author user-facing settings and route a **stock Bevy component** onto the camera. There is no custom WGSL pass to write. |
| **Multi-pass render-graph crates** | `renzora_lumen`, `renzora_rt`, `renzora_oit`, `renzora_solari` | Their own graph nodes, multiple passes, custom bind groups and extra textures. |

If your effect needs its own bind-group layout or more than one pass, it is the second kind and wants a crate rather than the macro.
