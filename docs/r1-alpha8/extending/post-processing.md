# Post-Processing Effects

A full-screen camera effect is **one shader file**. The file declares its own sliders in comments, so there is no Rust to write, nothing to compile and no plugin to install: put the `.wesl` in your project and drop it on a camera.

The effects published on the marketplace (CRT, film grain, pixelation, toon and about fifty more) are all files of this shape. Install one from the **Shaders** category and it lands in your project's `shaders/` folder.

## Using an effect

1. Select the camera, or the World Environment if the effect should apply to every camera.
2. **Add Component ▸ Post Process**.
3. Drag the effect's `.wesl` from the Assets panel onto the drop zone at the top of the component.

Two shortcuts do the same: drop the `.wesl` on the **viewport** and it joins the scene's Post Process list (an existing one first, else the World Environment's, else the default camera's), or drop it on a **Hierarchy** row to add it to that entity's list. Either way the entity is selected so the inspector shows the effect. An effect file never goes on a mesh as a material: it is a different kind of shader, and the drop targets refuse it as one.

Each effect becomes a section with its sliders, an on switch, move up and move down, and remove. Effects run in list order, each one reading what the one above it wrote, so a blur above a pixelation is a different picture from the same two the other way round.

A file that is not a usable effect still appears in the list, with a red message saying why, so a typo in a shader you are writing shows up where you are looking. The same message goes to the Console.

## Writing one

The whole of a working effect:

```wgsl
// @effect(name = "Invert", icon = "circle-half")

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;

struct Settings {
    amount: f32,    // @slider(0, 1, default = 1)
};
@group(0) @binding(2) var<uniform> settings: Settings;

@fragment
fn fragment(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let c = textureSample(screen_texture, screen_sampler, uv);
    return vec4(mix(c.rgb, 1.0 - c.rgb, settings.amount), c.a);
}
```

Save it as `shaders/invert.wesl` and it can be dropped on a Post Process component straight away. Edit it while it is in use and the effect, and its sliders, update when you save.

### The header

The `// @effect(...)` comment is what makes a shader an effect; the drop zone refuses a shader without one. It takes:

| Setting | Meaning |
|---|---|
| `name` | What the inspector calls the effect. Defaults to the file name. |
| `icon` | Its glyph, a [Phosphor](https://phosphoricons.com) name such as `television-simple`. Defaults to `sparkle`. |
| `freeze_while` | For effects that read a frozen frame; see [below](#effects-that-need-the-previous-frame). |

`// @effect` on its own, with no parentheses, is a valid header too.

### Settings and sliders

The settings are an ordinary struct at `@group(0) @binding(2)`. A comment after each field says how the inspector shows it:

| Comment | The field is |
|---|---|
| `// @slider(min, max)` | a slider over that range |
| `// @slider(min, max, default = 0.5, step = 0.01, label = "Amount")` | the same, with a starting value, drag step and row label |
| `// @hidden(default = 1)` | fixed at that value and not shown |
| none | a slider with no range, starting at 0 |

Two field names are filled in by the engine, and get no comment:

- **`time`**: seconds since startup. It wraps at 1024 seconds, because a float second count loses precision after a few hours of uptime and a noise pattern hashed from it starts to visibly step.
- **`enabled`**: always 1. Effects carried over from the old plugin format read it; a new one has no need to, since a switched-off effect is not drawn at all.

A field whose name starts with `_` is hidden at 0.

Fields must be `f32`, `i32` or `u32`. An integer field's slider snaps to whole numbers. You never write padding: the engine takes each field's offset from the shader compiler and packs the buffer to match, so add, remove or reorder fields freely. Values are stored by field name, so a value you have set survives the shader being edited around it.

### The shader contract

- **Write the `@fragment` entry point only.** The vertex stage is the engine's fullscreen triangle; its output arrives as `@builtin(position)` plus a `@location(0)` UV.
- **Bindings are all in `@group(0)`.** 0 is the frame so far, 1 its sampler, 2 the settings. 3 and 4 are for a frozen frame (below). Binding anything else is refused with a message naming the binding, because the pass does not provide it.
- **One file, no imports.** An effect is compiled on its own, so it cannot `import` another module. Plain WGSL is valid WESL, which is why the `.wesl` extension works for both.
- **Effects run after tonemapping**, on the picture as it will be shown. Values are 0 to 1; an effect that needs light above 1 belongs earlier in the frame, as a render pass (below).

### Animated effects

Read `settings.time`. Two habits worth copying from the shipped effects:

- **Scale time in the shader** when a slider sets the speed: `settings.time * settings.speed`.
- **Feed time in as its own hash input** rather than adding it to the sample position. Offsetting the position slides one fixed noise pattern across the picture, which reads as dirt on the lens instead of grain regenerating in place.

## Effects that need the previous frame

A transition fades *from* what was on screen. Declare bindings 3 and 4 and the pass gives the effect a copy of the frame as it arrived, alongside the live one:

```wgsl
// @effect(name = "Screen Transition", icon = "arrows-left-right", freeze_while = "progress < 1")

@group(0) @binding(3) var frozen_texture: texture_2d<f32>;
@group(0) @binding(4) var frozen_sampler: sampler;
```

`freeze_while` names a settings field and a comparison (`<`, `<=`, `>`, `>=`, `==`, `!=`). While it holds the copy stops updating, so the effect blends from a fixed picture. Here, setting `progress` to 0 freezes the outgoing frame, and animating it back up to 1 wipes to the live one.

## Checking effects in bulk

```sh
cargo run --profile dist -p renzora_postprocess --example check_effects -- <folder>
```

parses every `.wesl` and `.wgsl` under a folder exactly as the engine does and lists each effect's sliders, or why it would not load. It is how the marketplace set is checked before publishing.

## Performance

- **Nothing is compiled per effect** beyond its own shader pipeline, which is built the first time the effect is used.
- **A camera with no effects costs nothing.** The pass returns immediately.
- Each effect is one fullscreen pass. Fewer, fatter shaders beat many thin ones, so fold related work into one effect where you can.

## When an effect is not one shader

An effect file is one fullscreen fragment pass over the current picture. Anything else is a **render pass** in a plugin: it registers into one of the [render phases](../rendering/pipeline.md) with `add_render_pass` and does whatever drawing it likes there.

| Phase | Image | Typical use |
|---|---|---|
| `Gi` | HDR, after the main 3D pass, before temporal AA | global illumination, reflections |
| `HdrPost` | HDR, after temporal AA | bloom, depth of field, motion blur |
| `LdrPost` | LDR, after tonemapping | colour grading, stylisation (where effect files run) |
| `Overlay` | final, after AA | letterboxing, UI-adjacent effects |

The engine's own bloom, depth of field, SSAO, SSR, motion blur, fog and anti-aliasing route Bevy's built-in effects onto the camera rather than running a shader of their own, and Solari and order-independent transparency are multi-pass renderers.
