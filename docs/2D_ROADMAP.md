# 2D Roadmap

A working list of 2D features that would take Renzora from "can make a 2D game"
to a first-class 2D engine. Grouped by system; roughly ordered by impact within
each group. ✅ = shipped, 🔶 = partial, ⬜ = not started.

## Tilemap

- ✅ Tileset import via the Tilemap panel, multiple tilemaps, tile palette with
  zoom/pan/multi-tile brush, paint/erase/rect-fill, ghost preview, tiles as
  real entities
- ⬜ **Atlas margin/spacing support** — many published tilesets pad 1–2px
  between cells; the slicer assumes a tight grid today
- ⬜ **Autotiling / terrain brushes** — paint "grass" and let bitmask rules
  pick edge/corner tiles automatically (the single biggest tilemap
  quality-of-life feature in any modern editor)
- ⬜ **Tile collision authoring** — per-tile collision shapes in the tileset
  editor, auto-merged into optimized colliders per layer (greedy rectangle
  merge, not one collider per tile)
- ⬜ Bucket fill (flood fill contiguous region), line tool, eyedropper
  (pick the tile/brush under the cursor from the map)
- ⬜ Tile transforms while painting: rotate/flip the brush (e.g. X/Y/Z keys)
- ⬜ Random/scatter brush — paint from a weighted set of tile variants
- ⬜ Animated tiles (per-tile frame sequences defined in the tileset)
- ⬜ Layer ordering UI + per-layer opacity/tint/lock/visibility toggles
- ⬜ Per-tile custom data (string/number tags readable from scripts —
  "is_water", "damage=2")
- ⬜ Chunked mesh render path *as an optimization behind the entity API* for
  huge maps (10k+ tiles), keeping entities as the authoring model
- ⬜ Isometric and hex grid modes
- ⬜ Tilemap import from the Tiled `.tmx`/`.tsx` format

## Sprites & animation

- ✅ Sprite sheets (hframes/vframes/frame), frame animation via the property
  timeline, sprite resize/rotate handles, pixel-perfect nearest filtering
- ⬜ **9-slice sprites** (corner-preserving stretch — needed for panels,
  speech bubbles, platforms)
- ✅ Named sprite animations: built on the property timeline — the **Sprite Anim**
  panel is a frame picker that writes `.anim` clips (`SpriteSheet.frame` track +
  a `SpriteImages.active` track for multi-sheet characters) onto the entity's
  `AnimatorComponent`, so each movement is a first-class timeline clip.
  `play_animation("run")` drives them (unified with skeletal); `SpriteImages`
  allows several sheets per character. See `docs/r1-alpha7/editor/sprite-animation.md`
- ✅ Animated-sprite preview — via the timeline scrub / play (clips are timeline clips)
- ⬜ Aseprite import (`.ase`/`.aseprite` → sheet + tagged clips)
- ⬜ 2D skeletal animation (bones + mesh deform), or at minimum cutout
  animation with pivots — plus Spine/spritesheet-rig import
- ⬜ Sprite z-sorting controls: sort by Y ("2.5D top-down"), explicit sorting
  layers with names, per-layer parallax factors
- ⬜ Sprite flip_x/flip_y in the inspector + scripting
- ⬜ Sprite modulate/tint + blend modes (additive, multiply) in the inspector
- ⬜ Pixel-perfect camera mode (integer zoom + integer sprite snapping to
  eliminate shimmer at any resolution)

## 2D physics (the missing pillar)

- ⬜ **avian2d backend** — `physics_2d` feature: rigid bodies, colliders
  (rect/circle/capsule/polygon), auto-init via `Node2d` (plan already in
  the 2D pipeline notes)
- ⬜ **Character controller 2D** — the make-or-break for platformers:
  move-and-slide, floor/wall/ceiling detection, slope handling, coyote time
  helpers
- ⬜ Collision shape editing gizmos in the 2D viewport
- ⬜ Area/trigger zones with `on_body_entered` script hooks
- ⬜ One-way platforms, moving platforms (kinematic bodies carrying riders)
- ⬜ Raycast/shapecast 2D scripting API
- ⬜ 2D joints (pin, spring, wheel) — swings, ropes, vehicles

## 2D lighting & VFX

- 🔶 2D lighting plugin (point lights, occluders) — vendored bevy_firefly
- ⬜ Normal-mapped sprites (lighting responds to sprite normal maps)
- ⬜ Soft shadows + light textures/cookies ("light sprites")
- ⬜ Global illumination-ish ambient + day/night tinting workflow
- ⬜ 2D particles presets (dust, rain, snow, torch fire) sized for pixel art
- ⬜ Sprite shader/material support: custom WGSL on a sprite (dissolve,
  outline, flash-on-hit) with inspector-editable uniforms
- ⬜ Screen-space 2D post FX profile: CRT, chromatic aberration, palette
  swap/limit, dithering
- ⬜ Trail/ribbon renderer 2D (sword slashes, projectile trails)

## Camera & feel

- ⬜ **Camera 2D follow behaviors** — follow target with smoothing/deadzone,
  look-ahead, camera limits (min/max world bounds), per-room snapping
- ⬜ Screen shake (trauma-based) + hit-stop helpers as built-ins
- ⬜ Camera zones/triggers (entering an area retargets or re-bounds the camera)
- ⬜ Parallax background layers as a first-class component (auto-scroll +
  factor per layer, infinite repeat)

## World & scene tooling

- ⬜ **Prefabs/blueprints for 2D**: author a "coin"/"enemy" once, stamp
  instances in the viewport (with per-instance overrides) — pairs naturally
  with the tile painter for object layers
- ⬜ Room/level workflow: multiple named 2D "rooms" per project with
  transitions (LDtk-style world map view)
- ⬜ Grid-snapped object placement mode for non-tile entities (snap toggle
  already exists; needs stamp-to-place UX)
- ⬜ 2D pathfinding: grid A* over tilemap collision + nav regions, and/or
  navmesh-2d, exposed to scripts (`find_path_2d`)
- ⬜ Y-sorted "props" workflow for top-down games (sort origin/pivot editing)

## UI & text (2D games lean on these hard)

- 🔶 Game UI canvas (ember markup) — needs 2D-game-oriented widgets:
  health bars, hearts, inventory grids, dialogue boxes
- ⬜ Bitmap font support + text effects (typewriter reveal, wave, shake)
- ⬜ Dialogue system primitive (or a scripting-friendly dialogue queue API)
- ⬜ World-space UI anchored to 2D entities (damage numbers, name plates)

## Scripting API (2D-specific)

- ⬜ Tilemap API: `get_tile(x,y)`, `set_tile(x,y,id)`, `erase_tile`,
  `tile_data(x,y)` (custom data), `world_to_cell`/`cell_to_world`
- 🟡 Sprite API: `play_animation` ✅, `set_sprite_frame` ✅; `flip`, `tint` ⬜
- ⬜ Physics 2D API: `move_and_slide`, `is_on_floor`, `raycast_2d`
- ⬜ Camera 2D API: `shake`, `follow`, `set_limits`, `zoom_to`
- ⬜ Input helpers: virtual axes/actions with buffering (jump buffering,
  input replay for ghosts)

## Performance & export

- ✅ Lean 2D export (render_3d stripped from 2D games)
- ⬜ Sprite batching audit at 10k+ entities; chunked tilemap render path
- ⬜ Texture atlas packing on export (merge loose sprites into atlases)
- ⬜ Sub-100MB (ideally sub-25MB) 2D game export target

## Editor polish (2D)

- ✅ Rulers, adaptive grid + grid-size input, zoom readout, multi-select +
  box select + group move, cursor coordinates
- ⬜ Undo/redo covering tile painting and 2D transforms (biggest gap)
- ⬜ Tile/entity count + draw call overlay for 2D scenes
- ⬜ Onion-skin ghost of the game camera bounds while playing
- ⬜ 2D project templates: platformer, top-down, shoot-'em-up starter kits
  (each doubles as living documentation of the 2D APIs)

## Suggested next three

1. **2D physics + character controller** — nothing else unlocks more game
   genres; everything above it is decoration until movement feels right.
2. **Autotiling + tile collision authoring** — turns the tile painter from a
   sprite stamper into a level editor.
3. ~~**Named sprite animations + events**~~ ✅ Done — `AnimatedSprite` + the
   Sprite Anim panel; `play_animation` and frame events close the runtime loop.
