# Wind

Renzora has **one wind**. Set a speed and a bearing on the World Environment and
every system that should respond to weather does: grass lays over, tree canopies
sway and rustle, cloth and flags pull downwind, the cloud deck drifts on the
same heading, and the ocean builds to a rougher sea state.

That sounds obvious, and it is the point. Before this existed each of those
systems had its own private wind knob, so a scene could easily end up with grass
leaning east under clouds sailing west — the kind of mismatch nobody consciously
notices and everybody reads as "this looks fake".

## Quick start

1. Select the **World Environment** entity in the outliner.
2. Open the **Wind** section in the inspector (Rendering category).
3. Drag **Speed** up from the default 4 m/s and watch the whole scene respond.

There is nothing to add or enable per-scene — every scene has a wind, calm by
default is still a light breeze, and the toggle in the section header turns it
off entirely.

## The controls

| Field | What it does |
|---|---|
| **Direction** | Compass bearing the wind blows *toward*, in degrees. 0° is +X, 90° is +Z. |
| **Speed** | Sustained speed in m/s. 4 is a gentle breeze, 12 is a fresh-to-strong breeze (the reference point everything is tuned against), 25+ is a storm. |
| **Gust Strength** | How far gusts lift the speed above sustained, 0–1. At 0.5 a gust peaks at 1.5× Speed. 0 is a perfectly steady wind, which reads as artificial. |
| **Gusts / sec** | Gust rate. 0.15 (one gust every ~7 s) is natural outdoor wind; above 1 it stops being gusting and becomes buffeting. |
| **Turbulence** | Cross-wind chaos, 0–1. This is what stops foliage from all swinging along one axis like a row of metronomes. |

> **Direction is "toward", not "from".** Weather forecasts report the direction
> wind comes *from*, which is the opposite of what a shader wants. Renzora
> stores travel direction, so a bearing of 90° pushes things toward +Z.

## What responds to it

### Grass and terrain foliage

Automatic. Each foliage layer keeps its own **Wind Strength** in the terrain
foliage settings, which is now a *response multiplier* rather than a wind: stiff
sedge at 0.4, tall meadow grass at 1.0. Turning the world wind down calms every
layer regardless of what they are set to.

Gusts travel spatially, so a gust front visibly sweeps across a field rather
than every blade surging at once.

### Trees and plants

Procedural trees sway out of the box — the generator writes per-vertex stiffness
weights into the mesh, and the wrapper tags both the trunk and the leaf canopy
with a **Wind Sway** component tuned for each.

For anything else — an imported bush, a hand-modelled palm, a fern — add **Wind
Sway** from the inspector:

| Field | What it does |
|---|---|
| **Response** | Multiplies the world wind for this mesh. Stiff shrub ≈ 0.4, willow ≈ 1.6. |
| **Flutter** | High-frequency leaf rustle, independent of the branch sway. Set to 0 for anything woody that should only bend. |
| **Amplitude** | How far the floppiest vertex travels at reference wind, in metres. |
| **Pivot Height** | *Fallback only.* Height at which a mesh with no authored wind weights counts as fully flexible. |

Adding Wind Sway swaps the mesh's material for a wind-animated variant of the
same material — you keep alpha cutouts, lighting, shadows and everything else.
Removing the component puts the original back. Meshes that share a material and
the same Wind Sway settings share one animated material too, so a forest of a
hundred identical trees costs one.

**Authored weights vs. the fallback.** Motion quality depends on where the
per-vertex stiffness comes from. A procedural tree carries real weights in its
`UV_1` attribute — 0 at the trunk base, 1 at a twig tip — so it bends the way a
tree bends. A mesh without them falls back to a height ramp: rigid at the object
origin, flexible at Pivot Height. That looks right on a bush and wrong on
anything with a long horizontal branch, which will swing as a rigid arm.

### Cloth

Automatic. Cloth follows the instantaneous wind including gusts, so a flag snaps
in the same gust that flattens the grass beneath it. The world wind occupies the
first slot in the cloth wind list; any extra forces a scene pushes (a scripted
downdraft, a fan) add on top rather than being overwritten.

### Clouds

The cloud deck takes its heading from the world wind and scales its drift with
wind strength, controlled by **Follow World Wind** on the Clouds component (on
by default). The authored **Speed** stays meaningful — it is the drift the deck
reaches at reference wind.

Two deliberate differences from ground-level foliage:

- **The deck ignores gusts entirely.** Cloud features are kilometres across; a
  two-second gust does not move them.
- **It never fully stops.** Air aloft is always moving, so even a dead-calm
  scene keeps a slow drift. A frozen sky looks more wrong than a drifting one.

Turn Follow World Wind off for a deliberately decoupled sky — a stylised level,
or a cutscene where the ground wind is scripted and the sky must not change.

### Oceans

The FFT ocean scales its whole sea state with the world wind, via **Follow World
Wind** on the Water Surface component (on by default) and **Wind Response** (how
much of the wind reaches this surface — a sheltered bay is ~0.4, open ocean 1.0).

The authored per-cascade wind speeds and bearings become the sea's *shape* — the
swell-to-wind-sea balance, the relative headings — and the world wind scales and
rotates that set as a whole. Turn the wind up and the same ocean gets harsher
without becoming a different ocean. Turning the flag off restores exactly the sea
you authored.

> **The ocean lags, on purpose.** It follows a heavily smoothed wind, so the sea
> takes tens of seconds to build after you turn the dial. This is not a
> shortcut. Wind speed is an input to the JONSWAP wave spectrum, so every change
> re-bakes the cascade textures — and a real sea takes *hours* to build to a new
> wind, so a sea that snapped instantly would look wrong as well as cost more.
> Watch the swell arrive after the gust; that delay is the effect working.

## Scripting

```lua
-- Speed in m/s, direction in degrees the wind travels toward.
set_wind(14.0, 90.0)

-- Gust depth (0-1), gusts per second, cross-wind turbulence (0-1).
set_wind_gusts(0.7, 0.3, 0.6)

-- Reading goes through reflection.
local speed = get("WindState.speed")
```

A storm rolling in is a script that ramps `set_wind` over time — the ocean's
natural lag will make the sea build behind it without any extra work.

## Notes and limits

- **Shading normals do not bend with the geometry.** A swaying branch keeps the
  normals it was built with. At sane amplitudes the error is invisible; at very
  high Amplitude on large woody geometry it can show as lighting that does not
  quite track the motion.
- **Wind is uniform across the level.** There are no local wind zones yet — no
  explosion gusts, no rotor wash, no shelter behind a cliff. Gusts do travel
  spatially, so wind is not *static*, but every point of the world sees the same
  wind field.
- **Wind Sway needs a StandardMaterial.** A mesh already using a procedural
  graph material is not swapped; author the sway into the graph with the
  **Wind** node in the material editor instead.
