# Particles and VFX

Particle effects in Renzora are `.vfx` files, made and edited in the
**effect editor** in the Particles workspace. Each emitter's behaviour is a
node graph; effects work in 2D and 3D, play in the editor and in exported
games alike, and large emitters run on the GPU. The full reference is
[Particle System (.vfx)](../rendering/particle-system.md); this page is the
short tour.

## Placing an effect

1. **Assets panel > New (or right-click) > Particle Effect**, then pick a
   template. The new `.vfx` opens in the effect editor.
2. Drag the `.vfx` from the Assets panel into the viewport. It spawns an
   entity already playing it, on the ground in 3D or on the plane in 2D.
3. Move and rotate it like anything else.

**Add Entity > Effects > Particle System** gives you an entity playing the
built-in campfire; drop any `.vfx` on its **Effect** slot to change it.

## The Particle System component

| Field | What it does |
|---|---|
| Effect | Which `.vfx` drives it. Drag one onto the slot. |
| Edit Effect | Opens the file in the effect editor. |
| Play / Pause / Stop / Restart | Playback. |
| Time Scale | Simulation speed. `0.5` is slow motion. |
| When Finished | What a one-shot effect does when it ends: keep, deactivate, or despawn the entity. |

Files hot-reload: save in the effect editor and every entity using that file
updates live. Scripts drive effects with the `vfx_*` functions; see
[Particle Effects](../scripting/particles.md).

## The effect editor

Double-click a `.vfx` in the Assets panel, or press **Edit Effect**. The
Particles workspace shows the **Effect Preview**, the **Emitter Settings** and
the **Effect Graph**. See
[The effect editor](../rendering/particle-system.md#the-effect-editor).

## Old .particle effects

Before the `.vfx` system, Renzora's particles were `.particle` files played by
bevy_hanabi, with their own editor. That system has been removed, and its
effects keep working:

- **Opening a project** converts each `.particle` that has no `.vfx` beside it
  into one with the same name (`fire.particle` gets `fire.vfx`). The Console
  lists every file it converted and anything that did not carry over.
- **Scenes** that used old effects load with a Particle System in their place,
  pointing at the converted `.vfx`. Save the scene to keep it that way.
- **Double-clicking a `.particle`** opens its `.vfx` in the effect editor.
- **An exported game** plays `.particle` files as they are: the engine
  converts them in memory when they load.

Your `.particle` files are never changed or deleted, and a `.vfx` that is
already there is never overwritten. See
[Old .particle effects](../rendering/particle-system.md#old-particle-effects)
for exactly what converts and how to convert by hand.

## The bundled library

`assets/particles/` holds over 120 effects in the old `.particle` format
(fire, smoke, explosions, magic, weather, sci-fi, liquids, ambient, pickups
and UI, with `*_2d` versions for 2D scenes). They play as they are, and in a
project they convert to `.vfx` like any other `.particle`. For new work, the
template gallery's built-in effects are the better starting point.
