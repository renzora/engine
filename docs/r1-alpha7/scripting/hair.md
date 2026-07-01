# Hair

Procedural hair, provided by the `renzora_hair` distribution plugin. Drop a `Hair` component on any mesh and it **generates actual hair strands** over that surface — scattering roots across the mesh, growing a tapered strand from each, simulating them so they sway, and rendering them as camera-facing ribbons. No hair rig, no separate hair asset, no external DCC groom required.

## Backend

`renzora_hair` ships as a `cdylib` distribution plugin (same model as ragdoll/cloth/lumen). It is not built into the engine by default — drop the built artifact into `<exe>/plugins/` to enable it (see [Building Plugins](/docs/r1-alpha7/extending/plugins)).

It is **not** built on the physics solver. Hair is many light, fast links, so the plugin runs its own lightweight **verlet** integrator on the strands, and rebuilds the strand geometry each frame as a **camera-facing ribbon mesh** (billboarded quad strips) so strands stay visible and lit from any angle without a custom shader.

## How it works

Add a `Hair` component to any entity that has a `Mesh3d` — a head, a scalp cap, or any surface you want covered:

1. **Scatter.** The plugin reads the mesh's triangles and scatters `strands` root points across the whole surface, area-weighted so dense areas get proportionally more hair. (Models load asynchronously, so it waits for the mesh to finish loading before growing.)
2. **Grow.** From each root it grows a tapered strand of `segments` points, stepping along the surface normal and easing toward gravity by `droop`, with per-strand length variation from `length_jitter`.
3. **Simulate + render.** Every frame it verlet-simulates the strands (root pinned to the animated surface, so hair follows a moving head and lags when it turns) and rebuilds the ribbon mesh, camera-facing, lit by a standard PBR material tinted by `color`.

The generated geometry lives on a **hidden render entity** — it is not saved into your scene and does not clutter the outliner. Only the `Hair` component is serialized, so the groom rebuilds deterministically when the scene loads.

> **The sway only runs in Play or Simulate mode.** While editing, the groom is held in its grown rest shape and rides the model rigidly. Use **[Simulate](/docs/r1-alpha7/editor/viewport#simulate-mode)** (the blue flask, beside Play) to watch it move while keeping the editor live.

## Tuning

Editing a **shape** field (`strands`, `length`, `length_jitter`, `segments`, `width`, `droop`) rebuilds the groom. Editing the **look** (`color`) or the **motion** (`stiffness`, `damping`, `gravity`) is applied **live** without rebuilding, so it never interrupts the simulation.

| Field | Default | Effect |
|---|---|---|
| `enabled` | `true` | Master switch — generate and show the hair. Off hides it. |
| `simulate` | `true` | Physically simulate the strands (sway) vs. hold the grown shape. Toggle with `enable_hair()`/`disable_hair()`. |
| `strands` | `2000` | Target strand count, scattered area-weighted over the mesh. Capped at 50 000. |
| `length` | `0.12` | Strand length in world units (before jitter). |
| `length_jitter` | `0.3` | Per-strand length variation, `0` (uniform) to `1` (down to half). Breaks up a flat silhouette. |
| `segments` | `5` | Points per strand — more is smoother but heavier. |
| `width` | `0.0035` | Root half-width of a strand ribbon (world units); tapers to a point at the tip. |
| `droop` | `0.5` | How far a strand bends from the surface normal toward gravity as it grows, `0` (sticks out) to `1` (flops down). This is the rest-shape droop, separate from the dynamic `gravity`. |
| `color` | `(0.16, 0.10, 0.06)` | Base hair color as RGB `0..1` (a `Vec3`), multiplied by a small per-strand shade variation. |
| `stiffness` | `0.12` | Sim spring-back toward the grown shape, `0` (limp) to `1` (barely moves). |
| `damping` | `0.7` | Sim velocity bleed-off, `0` (swings forever) to `1` (dead). Frame-rate normalised. |
| `gravity` | `1.0` | Sim gravity multiplier. `0` floats; lower suits short, stiff hair. |

For a full head of hair, put the component on a **scalp mesh** (a low-poly cap covering just the hair area) rather than the whole head — the strands grow over the entire mesh you attach it to, so a dedicated scalp gives the cleanest hairline.

## Scripting

| Function | Lua | Rhai | Effect |
|---|---|---|---|
| `enable_hair()` | yes | — | Turns the sway on for the script's entity (`Hair.simulate = true`). |
| `disable_hair()` | yes | — | Turns it off; strands settle back to the grown shape. |

```lua
-- character.lua
function on_enter_water()
    -- heavy, clingy hair underwater (live tuning, no regen)
    set("Hair.damping", 0.95)
    set("Hair.gravity", 0.25)
end

function on_exit_water()
    set("Hair.damping", 0.7)
    set("Hair.gravity", 1.0)
end
```

> `enable_hair`/`disable_hair` target the **script's own entity** — call them from a script attached to the same entity the `Hair` component is on. Because the look and motion fields are live, you can also animate them directly with `set("Hair.color", …)`, `set("Hair.stiffness", …)`, etc.

Read the current state from the `Hair` component via the reflect path dispatcher, same as any other component:

```lua
if get("Hair.simulate") then
    -- hair is currently swaying
end
```

## Limitations (v1)

- **No collisions yet.** Strands sway under gravity and inertia but do not collide with the body or the world, so very long hair can pass through shoulders. Opt-in capsule colliders are planned.
- **Camera-facing ribbons only.** No tube geometry or hair-card atlases, and no custom anisotropic strand shader yet (uses the standard PBR material).
- **Uniform groom.** One length/width/droop applies across the whole surface; there is no painting of density, length, or direction maps.
- **Grows over the whole attached mesh** — scope hair by attaching to a dedicated scalp mesh rather than a full body.

## Related

- [Ragdoll Physics](/docs/r1-alpha7/scripting/ragdoll) — solver-based skeletal ragdoll (for pre-rigged bones, not generated strands)
- [Building Plugins](/docs/r1-alpha7/extending/plugins) — how distribution plugins like this one are built and loaded
