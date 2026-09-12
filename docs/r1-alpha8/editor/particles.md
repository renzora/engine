# Particles and VFX

Renzora's particle system runs on the GPU, so you can have hundreds of thousands of particles. Effects play in the editor and in exported games alike.

Effects are `.particle` files, and the engine ships over 120 ready-made ones across fire, smoke, magic, weather, explosions, sci-fi, liquids and ambient.

## Dropping in a prebuilt effect

1. Open the **Assets** panel and browse to `particles/`.
2. Drag a `.particle` file into the viewport. It spawns an entity already pointing at that file and starts playing.
3. Move and rotate it like anything else.

That is the whole loop for using the library.

## The Particle Effect component

The component that turns an entity into an emitter. Add it from **Add Component**, or get it for free by dragging a `.particle` in.

| Field | What it does |
|---|---|
| File | Which `.particle` file drives it. Drag one onto the slot. |
| Playing | Whether the emitter is currently spawning |
| Rate Multiplier | Scales the spawn rate at runtime |
| Scale Multiplier | Scales particle size |
| Color Tint | An RGBA multiplier over the whole effect |
| Time Scale | Simulation speed. `0.5` is slow motion. |
| Variable Overrides | Per-instance overrides for the effect's custom variables |

These let one shared file drive many entities that each look slightly different, a bigger fire or a tinted aura, without duplicating the file.

The **pencil** button opens that effect in the Particle Editor. So does double-clicking a `.particle` in the Assets panel.

Files hot-reload. Save in the editor and every entity using that file updates live.

## The Particle Editor

<!-- screenshot: particle_editor.png - the Particle Editor in Simple mode with its property sections -->

The **Particles** workspace has three panels: the **Editor** for the active effect, a live **Preview** with an orbit camera, and the **Inspector** for a selected emitter.

<!-- screenshot: particle_preview.png - the Particle Preview panel showing an effect rendering live -->

**New Effect**, **Open**, **Save** and **Save As** are in the editor header. You can also drag a `.particle` into the Preview to load it.

Two modes:

- **Simple** is collapsible sections of properties. This is where you will spend most of your time.
- **Advanced** is a node graph, for effects that do not fit the property sliders.

<!-- screenshot: particle_graph.png - the Particle Editor in Advanced mode showing the node graph -->

## Spawning

| Setting | What it does |
|---|---|
| Capacity | Hard cap on live particles for this effect |
| Spawn Mode | **Rate** for continuous, **Burst** for one shot, **Burst Rate** for repeated bursts |
| Spawn Rate | Particles per second |
| Spawn Count | Particles per burst |
| Spawn Duration | How long a spawn cycle lasts |
| Spawn Cycles | How many cycles. Zero loops forever. |
| Starts Active | Whether it emits as soon as it spawns |

Use **Rate** for fire and smoke, **Burst** for an explosion or a pickup pop, and **Burst Rate** for rhythmic puffs like a chimney.

## Lifetime

Each particle lives a random duration between the minimum and maximum, in seconds. Most over-lifetime curves are normalized to this span.

## Emission shape

Where particles are born.

| Shape | Parameters | Typical use |
|---|---|---|
| Point | | Torch, sparkler, focused source |
| Circle | Radius, volume or surface | Rain disk, ground ring, fountain mouth |
| Sphere | Radius, volume or surface | Explosions, auras |
| Cone | Base and top radius, height | Flamethrower, spray, breath |
| Rect | Half extents | Wall fire, window frost |
| Box | Half extents | Volumetric fog, room dust |

**Volume** emits from inside the shape, **Surface** from its boundary.

## Velocity

How fast and which way particles launch.

| Mode | What it does |
|---|---|
| Directional | A direction plus a cone spread |
| Radial | Outward from the centre |
| Tangent | Perpendicular to an axis, for vortices |
| Random | Any direction |

**Magnitude** and **Spread** set the base speed and cone angle. A **speed range** gives varied, more natural motion than a single magnitude.

## Forces

Applied every frame after birth.

| Setting | What it does |
|---|---|
| Acceleration | A constant force. Negative Y is gravity, positive Y is buoyant smoke. |
| Linear Drag | Damping. Higher makes particles slow and settle. |
| Radial Acceleration | Push away from the emitter centre, or toward it when negative |
| Tangent Acceleration | Swirl around an axis, for twisting plumes |
| Velocity Limit | Clamp the maximum speed |

### Attractors

Point force fields that bend particle paths. Each has a position, a radius, a falloff distance, a strength (negative repels) and a maximum speed.

Use one above a fire to curl smoke upward, or several to weave particles between points.

**Conform to Sphere** pulls particles onto a sphere shell, with a half-thickness and a stickiness for damping. Good for magnetic halos, planet-surface motes and shields.

### Noise turbulence

Organic, winding motion.

**Frequency** is the scale of the swirls, low for big blobs and high for fine detail. **Amplitude** is how strongly it pushes. **Octaves** layer detail, and 2 to 4 is typical.

This is what makes smoke roil and flames flicker instead of moving in straight lines.

### Orbit

Spirals particles around an axis, with a centre, an axis, a speed, an inward pull and a radius. Swirling magic motes, energy rings, vortices.

## Size over lifetime

**Size Start** and **Size End** grow or shrink linearly across the particle's life. A **size curve** with several keys overrides them, for shapes like puff out then fade.

**Non-uniform size** stretches X and Y independently, for tall flames or flat shockwaves.

**Screen space size** keeps a constant on-screen size regardless of distance, which suits markers.

**Roundness** softens the particle's edge, from hard to very soft.

## Colour over lifetime

Three ways, in increasing physical accuracy.

1. **Flat colour** for a single RGBA.
2. **Gradient**, a list of stops the particle blends through as it ages. The editor has add, remove and preset controls.
3. **Blackbody**, which drives colour from physically accurate black-body radiation. A start of 6500K and an end of 1200K is white-hot fading to ember red. This is the most realistic fire colour and overrides the gradient when set.

For particles that glow through the bloom pass, enable **HDR colour** and push the intensity above 1. Muzzle flashes and magic want this.

## Rendering

| Setting | Options |
|---|---|
| Blend Mode | **Blend** for normal, **Additive** to brighten (fire, energy), **Multiply** to darken |
| Alpha Mode | Blend, Premultiply, Add, Multiply, Mask, Opaque |
| Billboard | **Face Camera**, **Face Camera Y** for upright effects, **Velocity** for streaks, **Fixed** |
| Rotation Speed | Spin each particle around its facing axis |
| Render Layer | Which layers see the effect |

Every effect uses a built-in soft sprite, so overlapping particles blend smoothly instead of reading as hard squares. There is nothing to configure.

**Erosion** dissolves particles in organic wisps driven by noise rather than fading uniformly. This is what gives smoke and gas their tattered, dissipating edges.

**Ribbons** connect sequential particles into a continuous strip of geometry, for beams, energy trails and slashes. Pair it with velocity billboarding for clean trails.

**Particle lights** attach a real point light to the emitter so the effect actually illuminates its surroundings. **Flicker** animates the intensity for natural fire light, and **shadows** is available but expensive.

## Simulation space and performance

**Local** means particles move with the entity. **World** means they stay where they spawned, so a moving torch leaves a trail.

**When Visible** skips simulation while the effect is off screen, which is free performance for distant or ambient effects.

**Kill zones** cull particles that enter or leave a region, either a sphere or a box, with an inside-or-outside toggle. A thin flat box at ground level makes a fountain splash out when droplets hit the floor.

## Custom variables

A named set of tweakable values baked into the effect: floats with a range, colours, or vectors.

They are the effect's public interface. Author against them in the graph, then override them per entity through the component so one file can drive many distinct instances.

## Particles in 2D

The same `.particle` files work in 2D. With the viewport in 2D, drag one in and the emitter spawns on the 2D plane and renders alongside your sprites.

Three things to know:

- **Set the 2D plane flag** on effects meant for 2D scenes. The simulation itself is dimension-agnostic, but several authoring conventions assume a 3D ground plane: circle emitters lie flat, radial velocities burst as a sphere, and noise pushes along Z. The flag flips all of those into the plane the 2D camera actually sees. The bundled `*_2d` effects use it.
- **Sorting** follows the emitter's Z, exactly like sprites. Raise or lower it to put the effect in front of or behind other 2D content.
- **Scale.** Effects are authored in world units and 2D scenes are pixel-scaled, so a stock 3D effect looks tiny. Author 2D effects with pixel-sized values, or scale the emitter up.

## The bundled library

`assets/particles/` ships over 120 effects to drop in directly or use as starting points.

| Category | Examples |
|---|---|
| Fire | `fire`, `fire_torch`, `fire_bonfire`, `fire_flamethrower`, `fire_dragon_breath` |
| Smoke | `smoke_chimney`, `smoke_steam_vent`, `smoke_poison_gas`, `smoke_volcanic` |
| Explosion | `explosion_grenade`, `explosion_shockwave`, `explosion_firework_peony`, `explosion_muzzle_flash` |
| Magic | `magic_aura`, `magic_portal`, `magic_frost_nova`, `magic_lightning_orb` |
| Weather | `weather_rain`, `weather_snow`, `weather_blizzard`, `weather_sandstorm` |
| Sci-Fi | `scifi_ion_trail`, `scifi_plasma_ball`, `scifi_force_field`, `scifi_teleport` |
| Liquid | `liquid_fountain`, `liquid_geyser`, `liquid_waterfall_mist`, `liquid_slime` |
| Ambient | `dust_motes`, `fireflies`, `ember_float`, `glow_spores` |
| Pickup and UI | `pickup_collect`, `pickup_levelup`, `ui_confetti_pop`, `ui_sparkle_trail` |

Opening one of these and reading how its sections are set is a good way to learn the system.

## Tips

- Start from a library effect close to what you want. It is far faster than building from defaults.
- Additive blending, HDR colour and a particle light together is the recipe for fire and magic that glows and lights the scene.
- For trails that follow a moving object, switch the simulation space to World.
- Turn on When Visible for ambient or distant effects.
- Use blackbody rather than a hand-tuned gradient whenever you are making fire. It is both easier and more accurate.
