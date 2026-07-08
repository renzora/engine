# Sprite Animation

Animate a 2D character from sprite sheets. This is **not** a separate animation
system — it rides the engine's existing pieces: a **`SpriteSheet`** picks a cell,
the **property timeline** keyframes which cell (`SpriteSheet.frame`), and each
movement is an ordinary **`AnimatorComponent`** clip. The **Sprite Anim** panel
(2D panel group, "film-strip" icon) is just a *fast frame picker* that writes
those clips for you.

## The pieces

- **`SpriteImagePath` / `SpriteSheet`** — a dropped sheet is a `Sprite` with a
  `SpriteImagePath`; add a `SpriteSheet` (columns × rows + `frame`) to slice it.
- **`SpriteImages`** — for characters whose actions live in **separate** sheets
  (`Idle.png`, `Run.png`, …). It holds a *list* of image paths plus an `active`
  **index**; a system binds `Sprite.image` to `images[active]`. Because `active`
  is a plain number, the timeline can **keyframe which sheet** (a string path
  couldn't be keyframed — only numbers can).
- **`AnimatorComponent` + `.anim` clips** — the movements (`idle_n`, `run_e`).
  Each is a property clip with a `SpriteSheet.frame` track and, on a multi-sheet
  character, a `SpriteImages.active` track. These are regular timeline clips.

## The workflow

**1. Make the sprite.** Drag a sheet from the asset browser **into the 2D
viewport** — it becomes a `Sprite` at that spot. Select it.

**2. Set the grid.** In the inspector's **Sprite Image** component, set **H
Frames** / **V Frames** (e.g. `8 × 6`) — the grid lives here (Godot-style), no
separate Sprite Sheet component. The Sprite Anim panel reads it and shows the
overlay on the sheet.

**3. Pick the frames.** In the palette (wheel zooms, right-drag pans, `− / +`
buttons zoom):
- **left-drag** a rectangle, or **click** one cell, or **Ctrl+click** cells one
  at a time to build an exact sequence.

Selected cells are **numbered in playback order**. A single column reads
top→bottom, a single row left→right — so for an 8-directional sheet (8 columns =
8 directions, 6 rows = 6 frames) you just drag each column.

**4. Create the clip.** Set the **FPS**, type a **name** (`idle_s`), and click
**Create Clip**. This writes `animations/idle_s.anim` — a `SpriteSheet.frame`
track through your cells (plus a `SpriteImages.index` track on multi-sheet
sprites) — onto the entity's `AnimatorComponent` and **opens it in the Timeline
panel**. The name field clears and re-focuses, so you can immediately name and
create the next clip. Repeat for each direction/action.

## Multiple sheets

Multiple sheets live on the **Sprite Image** inspector component:

- **Image** is a **dropdown** of the sheet names (never a drop target) — it's how
  you pick the active sheet.
- **Add Sheet** is the drop area: drop a sheet here and it's appended to the list,
  populating the Image dropdown. The first drop turns a single-image sprite into
  a multi-sheet one.

The **Image** dropdown has a **keyframe button**, so each clip pins its sheet (a
`SpriteImages.index` track, alongside `frame`). Play an `idle_*` clip → `Idle.png`;
play `run_*` → `Run.png`.

The **Sprite Anim panel** also has a **Sheet** dropdown (same list) — switch it to
the sheet you're animating, and the panel's palette shows that sheet; the clips
you then create pin to it. (Both dropdowns drive the same `SpriteImages.index`,
so they stay in sync.)

The **Sprite Anim panel does not switch sheets** — it just picks cells from
whichever sheet is currently indexed. So: in the inspector, set **Index** to the
sheet you're animating, then in the panel pick that sheet's cells and Create the
clips; change **Index**, repeat. The panel reads the entity's **Sprite Sheet**
component for the grid (no grid inputs of its own).

> If clips you made earlier don't switch sheets, they predate this and have no
> `active` track — recreate them (with the right image tab selected first).

## Editing and playing

The panel only *creates* clips — everything else is the **Timeline** panel and
scripting, because these are ordinary animator clips:

- **Edit / retime** in the timeline: the created clip opens there with its
  `frame` keyframes; move them, change durations, add `flip_x` or `active`
  tracks, add frame-event markers.
- **Preview** by scrubbing or the timeline's play button.
- **Play in game** from a script — `play_animation("idle_s")` drives it exactly
  like a skeletal clip (an entity has one or the other):

```lua
-- 8-directional character.
local dirs = { "e", "ne", "n", "nw", "w", "sw", "s", "se" } -- CCW from +x
local facing, current = "s", ""

function on_ready() play_animation("idle_s") end

function on_update(dt)
    if input_x ~= 0 or input_y ~= 0 then
        local a = math.atan(input_y, input_x)
        facing = dirs[(math.floor((a / (2 * math.pi) * 8) + 0.5) % 8) + 1]
    end
    local clip = "idle_" .. facing        -- "run_" .. facing while moving
    if clip ~= current then current = clip; play_animation(clip) end
end
```

Or set `AnimatorComponent.default_clip` (the panel sets it to your first clip) so
one plays on load.
