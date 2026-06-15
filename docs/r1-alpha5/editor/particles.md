# Particles & VFX

Renzora's particle system is a GPU-driven VFX engine built on a vendored `bevy_hanabi` fork (wrapped by `renzora_hanabi`), with a dedicated node-and-property editor (`renzora_particle_editor`). Effects are authored as **`.particle`** files and attached to entities with a single component, and the engine ships **120+ ready-made effects** across fire, smoke, magic, weather, explosions, sci-fi, liquids, and ambient categories.

Everything runs on the GPU, so you can have hundreds of thousands of particles. Effects play in the editor *and* in exported games.

## Quick start: drop in a prebuilt effect

1. Open the **Assets** panel and browse to `particles/`.
2. **Drag a `.particle` file into the viewport** (e.g. `fire.particle`). It spawns an entity with a `Hanabi Effect` component already pointing at that file and starts playing immediately.
3. Move/rotate the entity like any other — by default the effect simulates in **local space**, so it follows the entity.

That's the whole loop for using the library. To build or tweak an effect, read on.

## The Hanabi Effect component

`HanabiEffect` is the component that turns an entity into a particle emitter. Add it from the Inspector (**Add Component → "Hanabi Effect"**) or get it for free by dragging a `.particle` into the scene.

| Field | Purpose |
|-------|---------|
| `source` | Where the definition comes from: `Asset { path }` (a `.particle` file — the usual case) or `Inline { definition }` (embedded in the scene). |
| `playing` | Whether the emitter is currently spawning. |
| `rate_multiplier` | Scales spawn rate at runtime (`1.0` = as authored). |
| `scale_multiplier` | Scales particle size at runtime. |
| `color_tint` | RGBA multiplier over the whole effect. |
| `time_scale` | Simulation speed (`1.0` = normal, `0.5` = slow-mo). |
| `variable_overrides` | Per-instance overrides for the effect's [custom variables](#custom-variables). |

These let one shared `.particle` file drive many entities that each look slightly different (a bigger fire, a tinted aura) without duplicating the file.

### Inspector workflow

The `Hanabi Effect` Inspector drawer has two controls:

- **File** — an asset drop field. Drag a `.particle` here to point the component at it.
- **Edit** (pencil) — opens that effect in the **Particle Editor** (below). Double-clicking a `.particle` in the Assets panel does the same.

`.particle` files **hot-reload**: when you save in the editor, every entity referencing that file updates live.

## The Particle Editor

Open it by editing an effect (above), or create a new one with **New Effect** — it switches to the **Particles** workspace, which has three panels:

- **Editor** — the property/graph editor for the active effect.
- **Preview** — a live 512×512 render with an orbit camera (auto-rotate + checkerboard floor toggles), updating as you edit.
- **Inspector** — the `Hanabi Effect` component when an emitter entity is selected in a scene.

File operations live in the editor header: **New Effect**, **Open**, **Save**, **Save As**. You can also **drag a `.particle` into the Preview** to load it.

The editor has two modes, toggled in the header:

- **Simple** — collapsible sections of properties (covered feature-by-feature below). This is where you'll spend most of your time.
- **Advanced** — a visual **node graph** (emitter → spawn → init → update → render nodes, plus math/input nodes) for wiring effects that don't fit the property sliders.

---

## Features in depth

Every section below maps to a group in the Simple editor and a field in the `.particle` file.

### Spawning

Controls how many particles are born and when.

| Property | Meaning |
|----------|---------|
| `capacity` | Hard cap on live particles for this effect (GPU buffer size). |
| `spawn_mode` | **Rate** (continuous, `spawn_rate`/sec), **Burst** (one shot of `spawn_count`), or **BurstRate** (repeated bursts). |
| `spawn_rate` | Particles per second (Rate / BurstRate). |
| `spawn_count` | Particles per burst (Burst / BurstRate). |
| `spawn_duration` | How long a spawn cycle lasts (`0` = default). |
| `spawn_cycle_count` | Number of cycles (`0` = infinite loop). |
| `spawn_starts_active` | Whether it begins emitting on spawn. |

Use **Rate** for sustained effects (fire, smoke), **Burst** for one-offs (an explosion, a pickup pop), and **BurstRate** for rhythmic puffs (a chimney, a signal flare).

### Lifetime

`lifetime_min` / `lifetime_max` — each particle lives a random duration in this range (seconds). Most "over lifetime" curves (size, color) are normalized to this span.

### Emission shape

Where particles are born, set by `emit_shape`:

| Shape | Parameters | Typical use |
|-------|-----------|-------------|
| **Point** | — | Torch, sparkler, focused source |
| **Circle** | radius, Volume/Surface | Rain disk, ground ring, fountain mouth |
| **Sphere** | radius, Volume/Surface | Explosions, auras |
| **Cone** | base/top radius, height | Flamethrower, spray, breath |
| **Rect** | half-extents (x, y) | Wall fire, window frost |
| **Box** | half-extents (x, y, z) | Volumetric fog, room dust |

"Volume" emits from inside the shape; "Surface" emits from its boundary.

### Velocity (initial motion)

How fast and which way particles launch.

- **Mode** (`velocity_mode`): **Directional** (a direction + cone spread), **Radial** (outward from center), **Tangent** (perpendicular to an axis — vortices), **Random**.
- `velocity_magnitude` + `velocity_spread` — base speed and cone angle.
- `velocity_speed_min` / `velocity_speed_max` — use a *range* of speeds instead of a single magnitude (varied, natural motion).
- `velocity_direction` — the aim for Directional mode; `velocity_axis` — the spin axis for Tangent.

### Forces, acceleration & drag

Applied every frame after birth:

- `acceleration` — constant force (set Y negative for gravity, positive for buoyant smoke).
- `linear_drag` — velocity damping (higher = particles slow and settle).
- `radial_acceleration` — push away from (or toward, if negative) the emitter center.
- `tangent_acceleration` + `tangent_accel_axis` — swirl around an axis (twisting plumes).
- `velocity_limit` — clamp maximum speed.

### Attractors & conform-to-sphere

Point force-fields that bend particle paths:

- **Attractors** (`attractors`) — a list, each with a world `position`, `radius`, `influence_dist` (falloff), `strength` (negative repels), and `max_speed`. Use one above a fire to curl smoke upward, or several to weave particles between points.
- **Conform to Sphere** (`conform_to_sphere`) — pulls particles onto a sphere shell (`shell_half_thickness`, `sticky_factor` for damping). Good for magnetic halos, planet-surface motes, shields.

### Noise turbulence

Organic, winding motion via Perlin-style noise:

- `noise_frequency` — scale of the swirls (low = big blobs, high = fine detail).
- `noise_amplitude` — how strongly it pushes.
- `noise_octaves` / `noise_lacunarity` — detail layering (2–4 octaves typical; more costs more).

This is what makes smoke roil and flames flicker instead of moving in straight lines.

### Orbit

`orbit` makes particles spiral around an axis: `center`, `axis`, `speed` (rad/sec, negative reverses), `radial_pull` (inward spiral), and `orbit_radius`. Used for swirling magic motes, energy rings, vortices.

### Size over lifetime

- `size_start` / `size_end` — linear grow/shrink across life.
- `size_curve` — a **multi-key curve** (`[{ time: 0..1, value }, …]`) that overrides start/end for shapes like "puff out then fade". The Simple editor has a curve widget for this.
- `size_non_uniform` + `size_start_x/y` / `size_end_x/y` — stretch X and Y independently (tall flames, flat shockwaves).
- `screen_space_size` — keep a constant on-screen size regardless of distance (UI/markers).
- `roundness` — softens the particle's edge (0 = hard, 1 = very soft).

### Color over lifetime

Three ways to color particles, in increasing physical accuracy:

1. **Flat color** — `use_flat_color` + `flat_color` for a single RGBA.
2. **Gradient** — `color_gradient`, a list of `{ position: 0..1, color: [r,g,b,a] }` stops the particle blends through as it ages. The editor has add/remove/preset controls.
3. **Blackbody** — `blackbody: Some([start_kelvin, end_kelvin])` drives color from physically-accurate **black-body radiation** (e.g. `[6500, 1200]` = white-hot → ember red). This is the most realistic fire color and overrides the gradient when set.

**HDR / bloom**: enable `use_hdr_color` and push `hdr_intensity > 1.0` so bright particles glow through the bloom pass (muzzle flashes, magic). `color_blend_mode` (**Modulate / Overwrite / Add**) controls how the lifetime color combines with the texture.

### Rendering & blending

How particles composite into the scene:

- `blend_mode` — **Blend** (normal), **Additive** (brightens — fire, energy), **Multiply** (darkens).
- `alpha_mode` — Blend / Premultiply / Add / Multiply / **Mask** (alpha-tested at `alpha_mask_threshold`) / Opaque.
- `billboard_mode` — **FaceCamera** (default), **FaceCameraY** (upright effects), **Velocity** (aligned to motion — streaks), **Fixed**.
- `orient_mode` — **ParallelCameraDepthPlane**, **FaceCameraPosition**, or **AlongVelocity** (stretch along travel).
- `rotation_speed` — spin each particle around its facing axis.
- `render_layer` — which render layers see the effect.

### Soft particles

Every effect automatically uses a built-in **soft radial sprite** (a procedural Gaussian falloff), so overlapping particles blend smoothly instead of reading as hard squares. There's nothing to configure — it's on by default and works with both additive and alpha-blended effects.

### Erosion / dissolve

Set `erosion: true` to dissolve particles in organic **wisps** driven by an fbm noise texture, rather than fading uniformly. This is what gives smoke and gas their tattered, dissipating edges (`smoke_volcanic`, `smoke_poison_gas`, `fire_bonfire`).

### Ribbons & trails

`ribbon: Some({ groups })` connects sequential particles into a continuous **ribbon** of geometry — beams, energy trails, slashes. Width follows the size curve. For clean trails, pair it with `billboard_mode: Velocity` and `orient_mode: AlongVelocity` (see `scifi_ion_trail`).

### Particle lights

`light: Some({ color, intensity, range, flicker, shadows })` attaches a real-time **PointLight** to the emitter so the effect illuminates its surroundings. `flicker` (0 = steady, ~0.35 = lively) animates the intensity with detuned sines for natural fire light; `shadows` enables shadow casting (expensive). See `fire_blackbody` for a fire that actually lights the scene.

### Simulation space & performance

- `simulation_space` — **Local** (particles move with the entity) or **World** (particles stay where they spawned — a moving torch leaves a trail).
- `simulation_condition` — **Always** or **WhenVisible** (skip simulation when off-screen; a free optimization for distant effects).
- `motion_integration` — PostUpdate (default), PreUpdate, or None.

### Kill zones

`kill_zones` cull particles that enter/leave a region — a **Sphere** or **Aabb** with a `kill_inside` toggle. Use a thin flat AABB at ground level to make a geyser or fountain "splash out" when droplets hit the floor (`liquid_geyser`).

### Custom variables

`variables` is a named map of tweakable values (`Float { value, min, max }`, `Color`, or `Vec3`) baked into the effect. They're the effect's "public API": you author against them in the graph, then override them per-entity through `HanabiEffect.variable_overrides` so one file can drive many distinct instances.

---

## The `.particle` file format

`.particle` files are human-readable RON. You rarely edit them by hand — the editor writes them — but here's an abbreviated `fire.particle` so you can read and diff them:

```ron
(
    name: "fire",
    capacity: 4096,
    spawn_mode: Rate,
    spawn_rate: 80.0,
    lifetime_min: 0.6,
    lifetime_max: 1.2,
    emit_shape: Circle(radius: 0.2, dimension: Volume),
    velocity_mode: Directional,
    velocity_direction: (0.0, 1.0, 0.0),
    velocity_magnitude: 1.5,
    velocity_spread: 0.3,
    acceleration: (0.0, 2.0, 0.0),
    size_start: 0.4,
    size_end: 0.0,
    blackbody: Some((6500.0, 1200.0)),
    blend_mode: Additive,
    erosion: false,
    simulation_space: Local,
    // …many more fields, all optional with sensible defaults
)
```

Every field has a default, and unknown/missing fields are tolerated, so older files keep loading as the format grows.

## The bundled effect library

`assets/particles/` ships 120+ effects you can drop in directly or use as starting points. Categories:

| Category | A few examples |
|----------|----------------|
| **Fire** | `fire`, `fire_torch`, `fire_bonfire`, `fire_flamethrower`, `fire_dragon_breath`, `fire_blackbody` |
| **Smoke** | `smoke`, `smoke_chimney`, `smoke_steam_vent`, `smoke_poison_gas`, `smoke_volcanic` |
| **Explosion** | `explosion`, `explosion_grenade`, `explosion_shockwave`, `explosion_firework_peony`, `explosion_muzzle_flash` |
| **Magic** | `magic_aura`, `magic_portal`, `magic_frost_nova`, `magic_healing_aura`, `magic_lightning_orb` |
| **Weather** | `weather_rain`, `weather_snow`, `weather_blizzard`, `weather_falling_leaves`, `weather_sandstorm` |
| **Sci-Fi** | `scifi_ion_trail`, `scifi_plasma_ball`, `scifi_force_field`, `scifi_teleport`, `scifi_reactor_core` |
| **Liquid** | `liquid_fountain`, `liquid_geyser`, `liquid_waterfall_mist`, `liquid_blood_spray`, `liquid_slime` |
| **Ambient** | `dust_motes`, `fireflies`, `ember_float`, `glow_spores`, `light_rays_dust` |
| **Pickup / UI** | `pickup_collect`, `pickup_levelup`, `ui_confetti_pop`, `ui_sparkle_trail` |

A good way to learn the system is to open one of these in the editor and read how its sections are set.

## Controlling effects at runtime

You drive a live effect through its `HanabiEffect` component fields — toggle `playing`, scale `rate_multiplier` / `scale_multiplier`, retint with `color_tint`, slow time with `time_scale`, or push `variable_overrides`. The engine also has an internal **command queue** (play / pause / stop / reset / burst / set-rate / set-scale / set-tint / set-variable) that the editor and scene systems use.

> **In progress:** direct Lua/Rhai functions for particles (e.g. a `play_effect()` verb) are not wired up yet — control today is via the component fields above. Likewise, **custom particle textures** (`texture_path`) and **flipbook** sprite-sheet animation are supported in the file format but the runtime currently renders with the built-in soft-sprite and erosion-noise textures rather than loading custom sprites. These are format-ready for when the texture pipeline lands.

## Tips

- Start from a library effect close to what you want, then tweak — it's far faster than building from `Point`/`Rate` defaults.
- **Additive + HDR + a particle light** is the recipe for convincing fire and magic that glows *and* lights the scene.
- For trails that follow a moving object, switch `simulation_space` to **World**.
- Turn on **WhenVisible** simulation for ambient or distant effects to save GPU time.
- Use **blackbody** instead of a hand-tuned gradient whenever you're making fire — it's both easier and more accurate.
