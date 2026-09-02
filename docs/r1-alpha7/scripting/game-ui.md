# Game UI

Game UI is everything your players see on screen: health bars, score counters, menus, buttons, and pop-ups. In Renzora you build it with simple `.html` markup files that update live, then bring it to life with a little Lua.

You don't need to be a programmer to start. If you've ever written a web page, this will feel familiar — and if you haven't, the examples below are short enough to copy and tweak.

## Two ways to build UI

Renzora gives you two friendly ways to make game UI, and you can mix them freely:

- **Markup** — write small `.html` files (the `renzora_ember` system). Best for HUDs and menus you want to lay out by hand. Save the file and the UI updates instantly while the game is running.
- **Canvas widgets** — drag ready-made widgets (buttons, sliders, bars) right into your scene in the editor. Best if you'd rather click than type.

Most makers start with markup, so we'll cover that first.

The screenshot below shows the kinds of building blocks you get out of the box — headings, charts, sliders, color pickers, gauges, timelines, and more.

![The renzora_ember UI gallery showing built-in components: typography, charts, sliders, color pickers, forms, gauges, and timelines](/assets/previews/renzora_ember.png)

## Your first UI file

A UI file is just an `.html` document with one `<template>` at the top. Each tag becomes one box on screen. Here's a tiny HUD with a title and a red health bar:

```html
<!-- ui/hud.html -->
<template>
    <node
        position="absolute" top="24px" left="24px"
        flex_direction="column" row_gap="8px"
        padding="16px" width="280px"
        background="#11151C" border_radius="12px"
    >
        <text font_size="18" font_color="#FFFFFF">Vitals</text>
        <node width="100%" height="14px" background="#1B2233" border_radius="999px">
            <node name="health_fill" width="72%" height="100%" background="#E74C3C" border_radius="999px" />
        </node>
    </node>
</template>
```

Save it, and the panel appears. Change a color or a width, save again, and it updates without restarting. That fast loop is the whole point of markup.

Selecting a UI entity opens its `.html` in the built-in code editor, and pressing **Ctrl+S** there hot-reloads every canvas using that template right away — no need to re-run the game. (Editing the same attributes through the inspector updates the entity live without a rebuild, so your selection stays put.)

### The tags you'll use

A handful of tags cover almost everything:

| Tag | What it's for |
|---|---|
| `<node>` | A box — your main layout building block |
| `<text>` | Words on screen |
| `<image>` | A picture (`src="..."`) |
| `<button>` | Something the player can click |
| `<input>` | A text field the player types into |
| `<icon name="...">` | A small icon, e.g. `<icon name="check" />` |

There are a few more tags for repeating lists and reusing components. See the [Scripting API](/docs/r1-alpha7/api/scripting) for the full list.

### Draw order

Tags paint in the order you write them, like HTML: a later sibling draws on top
of an earlier one. So the usual way to put content on a panel is to write the
background first and the content after it:

```html
<node width="395px" height="57px">
    <image src="assets/ui/bar_bg.png" position="absolute" top="0" left="0"
           width="395px" height="57px" />
    <text>100 / 100</text>   <!-- drawn on top of the background -->
</node>
```

Widgets you *drag into a scene* follow the opposite convention — the canvas
editor lists them like layers, where the top row is front-most — but that
applies only to those entities, never to markup.

### Sizes and spacing

Lengths are written like CSS: `12px`, `50%`, `auto`, or the viewport units
`vw` / `vh` / `vmin` / `vmax`. A bare `0` is fine and means `0px`.

`padding`, `margin`, `border` and `border_radius` take one, two or four values:

| Form | Meaning |
|---|---|
| `padding="10px"` | all four sides |
| `padding="10px 20px"` | **horizontal, then vertical** |
| `padding="5px 10px 5px 10px"` | top, right, bottom, left |

Note the two-value form: it is horizontal first, which is the opposite way round
from CSS. The four-value form follows CSS exactly.

## Making the UI show live values

The best part: your UI can show numbers that change as the game runs. Wrap a value in **double braces** and it re-reads every frame:

```html
<text font_size="14" font_color="#FFFFFF">Score: {{ Player.score }}</text>
<text font_size="12" font_color="#8A93A2">Lives: {{ Player.lives }}</text>
```

What goes inside the braces? A few common forms:

- `{{ score }}` — a variable from the script on the same entity as this UI.
- `{{ Player.score }}` — a variable on the entity you named `Player`.
- `{{ Name }}` — the entity's name.

So when your Lua script changes `score`, the text on screen changes too. You don't have to do anything else.

### Show or hide things

Use `show=` with a condition to flash a warning or reveal a menu only when it matters:

```html
<node show="{{ Player.Health.current < 25 }}" background="#E74C3C" />
<text show='{{ Player.team == "red" }}'>RED TEAM</text>
```

Conditions understand `and`, `or`, `not`, comparisons (`< > <= >= == !=`), and parentheses.

## Buttons that do something

To make a button run code, give it an `on_press` name:

```html
<button name="btn_play"
        padding="14px" background="#1B1F27" border_radius="8px"
        on_press="press_play" on_enter="hover_play">
    <text font_color="#FFFFFF">Play</text>
</button>
```

Then catch that name in your Lua script's `on_ui` function:

```lua
function on_ui(name, args, entity)
    if name == "press_play" then
        start_game()
    elseif name == "hover_play" then
        play_sound("audio/menu_button.mp3", 0.5)
    end
end
```

`on_press` runs on click; `on_enter` and `on_exit` run when the mouse moves over or away. UI event handling like this is **Lua-only**.

## Gauges, bars, and charts

Need a circular gauge, a bar chart, or a speedometer? Add `vector="..."` to a node and Renzora draws it for you:

| `vector=` | Draws |
|---|---|
| `gauge` (or `arc`, `ring`) | A circular gauge |
| `bars` | A bar chart |
| `line` (or `chart`) | A line chart |
| `wave` | A waveform |
| `speedometer` (or `dial`) | A full dial with ticks, labels, and a needle |

```html
<node vector="gauge" width="160px" height="160px"
      value="{{ Player.fuel }}" min="0" max="100"
      color="#4C8BF5" readout="{{ Player.fuel }}" />
```

The `value`, `data`, and `readout` fields accept live `{{ }}` bindings, so a fuel gauge or speedometer tracks your game in real time. For the full list of widget options, see the [Scripting API](/docs/r1-alpha7/api/scripting).

## Images

`<image src="...">` draws a picture. Three attributes control *how* it is drawn,
and for hand-drawn or pixel-art UI you will usually want all three.

### `pixelated` — keep pixel art sharp

By default an image is sampled smoothly, which is right for a photo and wrong
for pixel art: a 40px icon drawn at 44px comes out blurry. Add
`pixelated="true"` and each texel stays a hard square at any size.

```html
<image src="assets/ui/icon_hp.png" pixelated="true" width="44px" height="44px" />
```

Every attribute needs a value — the markup grammar is strictly `key="value"`, so
a bare `pixelated` is a parse error on the whole tag, not an ignored attribute.

It is per-image on purpose. The editor's own interface is built from this same
markup, so there is no project-wide "this game is pixel art" switch that
wouldn't also resample the editor's icons.

### `image_mode` — nine-slicing and tiling

A panel, frame or slot drawn from a small texture has a decorated border.
Stretching it to a useful size smears that border along with the middle.
Nine-slicing holds the corners at their authored size and stretches only the
middle:

```html
<!-- 93x76 source with a ~20px ornate border, drawn at 640x440 -->
<image src="assets/ui/panel.png" pixelated
       image_mode="sliced(20)" width="640px" height="440px" />
```

| `image_mode=` | Draws |
|---|---|
| `auto` | At the texture's own size (the default) |
| `stretch` | Stretched to the node's box |
| `sliced(8)` | Nine-slice, 8px border on all four sides |
| `sliced(8, 12)` | 8px left/right, 12px top/bottom |
| `sliced(l, r, t, b)` | Each side given separately |
| `tiled(1.0)` | Repeat rather than stretch |

The border is measured in **source texture pixels**, so it doesn't change when
the node is resized — that is the whole point. Corners never scale, which is
what keeps a nine-sliced pixel-art frame from growing fat corners.

A value that doesn't parse logs a warning and falls back to `auto`, rather than
being ignored in silence.

### Bars drawn from images

A bar drawn as a flat colour is filled by making it narrower. A bar drawn from a
texture cannot be: shrinking an image squashes the whole picture into less space
instead of showing less of it, so a health bar with bevelled ends and segment
ticks compresses those details as it drains.

So an image fill **crops** instead. Put the fill image over the background image
and give it `image_fill`:

```html
<node width="553px" height="80px">
    <image src="assets/ui/bar_bg.png" pixelated
           position="absolute" top="0" left="0" width="553px" height="80px" />
    <image name="hp_fill" src="assets/ui/bar_fill_hp.png" pixelated
           position="absolute" top="13px" left="22px" height="55px"
           image_fill="1.0" fill_dir="left_to_right" fill_extent="447" />
</node>
```

| Attribute | Meaning |
|---|---|
| `image_fill="0.75"` | Starting fraction, 0 to 1 |
| `fill_dir=` | `left_to_right` (default), `right_to_left`, `bottom_to_top`, `top_to_bottom` |
| `fill_extent="447"` | On-screen width at full, in px. Omit to draw the source at 1:1 |

Then drive it from a script — Lua by reflection, Rust by writing the component:

```lua
set_on("hp_fill", "UiImageFill.value", current_hp / max_hp)
```

```rust
// In a Rust script, with the entity from MarkupNameIndex:
if let Some(mut fill) = world.get_mut::<UiImageFill>(bar) {
    fill.value = current_hp / max_hp;
}
```

A `right_to_left` or `bottom_to_top` fill shrinks toward its layout origin, so
anchor it to the far edge (`position_type: absolute` with `right: 0` or
`bottom: 0`) or it will drain from the wrong end. Left- and top-origin fills,
the common case, need nothing.

The same applies to the binding form: `fill="Player.Health.current"` on a node
carrying an image crops it, and on a plain coloured node resizes it, because
squashing a bar texture is never what you want. `fill_dir` and `fill_extent`
work on both.

## Gamepad navigation

Menus are navigable with a controller out of the box. The d-pad or left stick
moves a focus between `<button>` elements, and **South** (A / Cross) activates
the focused one.

There is nothing to turn on and nothing to annotate. Focus is published by
writing `Interaction::Hovered` on the focused button and activation by writing
`Interaction::Pressed` for one frame — the same component a mouse writes — so
`on_press` handlers, hover styling and transitions all respond exactly as they
do to a click, and a button that works with a mouse works on a pad.

Movement picks the nearest button in the direction pressed, weighting sideways
distance so pressing "down" stays inside the column you are looking at. Holding
a direction repeats after a short delay. Pushing past the end of a list keeps
the current selection rather than dropping focus.

Moving the mouse releases gamepad focus, so a player who switches input mid-menu
doesn't fight a stuck highlight; pressing a direction takes it back.

To read the focus or turn the whole thing off for a screen that reads the pad
itself — a gameplay HUD, where the same button press would otherwise do two
things — use the `UiGamepadNav` resource:

```rust
world.resource_mut::<UiGamepadNav>().enabled = false;
```

## Showing and hiding UI from a script

Your script spawns and hides UI with `action(...)`. The common verbs:

| Verb | What it does |
|---|---|
| `hui_spawn` | Show a UI file, e.g. `{ template = "ui/hud.html" }` |
| `hui_despawn` | Remove a UI file |
| `hui_hide` / `hui_show` | Hide or show a named piece of UI |
| `quit` | Close the game |

```lua
function on_ready()
    -- Show the HUD as soon as this entity wakes up
    action("hui_spawn", { template = "ui/hud.html" })
end

function open_pause_menu()
    action("hui_hide",  { name = "hud_root" })
    action("hui_spawn", { template = "ui/pause_menu.html" })
end
```

These `action()` verbs are **Lua-only**.

## Building UI by dragging in the editor

Prefer clicking to typing? The second path lets you drop ready-made widgets straight into your scene and arrange them in the viewport. Below, a match screen is being assembled from widget cards on the left, with their colors and values edited on the right — and a big "START MATCH" button laid out in the center.

![The Renzora editor building a game-UI screen from drag-in widgets, with a START MATCH button and widget settings panels](/assets/previews/ui.png)

Each widget is an entity that lives in your scene and saves with it, so it's there next time you open the project. There are widgets for the usual things — buttons, sliders, checkboxes, dropdowns, text inputs, progress and health bars, tooltips, modal pop-ups, and basic shapes.

When you add a canvas yourself — **Add Entity → UI Canvas**, or the **New UI** scene starter — a blank `ui/<name>.html` is created alongside it and linked as its template, so selecting the canvas opens that file in the code editor. A canvas that appears *on your behalf*, to host something you dropped into an empty scene, doesn't get one: it keeps the template or widget you dropped, rather than adding a second, empty template file to your project.

**Widgets always live under a UI Canvas.** The canvas is what scopes its widgets to the game view; a widget outside one has nowhere to render. So the editor keeps that relationship intact for you: if you drag a widget out to the scene root (or under a non-UI entity), it's automatically re-homed under a fresh **UI Canvas** rather than escaping into the editor's own interface — you'll simply see a new canvas appear in the hierarchy holding it. Having more than one canvas is fine (a HUD and a pause menu, say). The reverse is also enforced: a canvas can't become a child of a widget — drop one there and it pops back to the top level.

## Canvas scaling

You design against a fixed size — **Ref Width** and **Ref Height** on the canvas, `1280 × 720` by default. Players don't have that window. **Scale Mode** decides what happens in between:

| Mode | What it does |
|---|---|
| **Fit** (default) | Lays the canvas out at exactly the reference size and scales that whole box to fit the window, centred, with bars on the leftover axis. What you composed is what ships. |
| **Expand** | Scales text and padding, but lets the canvas fill the window so the layout re-flows to the real aspect ratio. |
| **Constant** | No scaling. One authored pixel is one screen pixel. |

The difference only shows up once a canvas holds more than one thing. A single centred panel looks the same in every mode; a centred panel *and* a bottom-left dialogue box do not — under **Expand** each is resolved against the live window, so widening it walks them apart, while under **Fit** the whole design box moves as one piece.

So: **Fit** for menus, dialogue, anything you laid out by eye. **Expand** for a HUD whose corners are meant to hug the screen edges however wide it gets. **Constant** for pixel-art UI that must not resample.

Fit is what the UI editor shows you, which is why it's the default — the editor renders at the reference size, so the panel and the game agree. World UI Panels ignore the setting entirely: their surface *is* the reference resolution, so there's nothing to reconcile.

## UI in the 3D world

A canvas draws its UI flat across the screen. A **World UI Panel** draws the same kind of `.html` template onto a flat surface *inside* the scene — a monitor on a wall, a control terminal you walk up to, a floating menu in VR. It's a real 3D object: it has a position and rotation, it's lit by the scene, and things can pass in front of it.

Add one with **Add Entity → UI → World UI Panel**. It comes with a starter template so you can see it straight away; the panel carries an **HTML Template** field just like any other UI entity, so point it at any of your `.html` files (in the inspector, or by editing the file it created) and that template is what appears on the surface. Everything else about authoring is identical — the same tags, the same live `{{ Component.field }}` values, the same hot-reload on **Ctrl+S**.

Two settings are specific to the panel, both in the inspector:

- **Size** — how big the surface is in the world, in meters (width × height).
- **Resolution** — how many pixels the template is drawn at before it's mapped onto the surface. Higher is crisper but costs more; the default (1280×720) suits a wall-sized panel.

Panels are **interactive**, not just decorative. Point at one and click — with the mouse in the editor viewport, or with a controller in VR — and the buttons, sliders and hover states respond exactly as they do on a normal canvas. Aiming anywhere on the surface maps to the matching spot on the template, so a button in the top-right of the file is a button in the top-right of the panel.

Unlike a canvas, a panel doesn't hold widget entities — its content is entirely the template it points at, so you build it by editing the `.html`, not by dragging widgets onto it.

### Controlling those widgets from a script

Widgets respond to a set of `ui_*` verbs. You target a widget by the `name` you gave it in the editor:

```lua
action("ui_set_text",   { name = "score_label", text = "Score: " .. score })
action("ui_set_slider", { name = "volume", value = 0.5 })
action("ui_set_theme",  { theme = "light" })
```

For anything without a dedicated verb (like driving a health bar's fill), use `set_on`:

```lua
-- Fill a bar named "health_fill" from 0.0 to 1.0
set_on("health_fill", "UiBarFill.value", current_hp / max_hp)
```

The built-in themes are `dark` (default), `light`, and `high_contrast`. Color values in `ui_set_color` are floats from `0.0` to `1.0`, not `0`–`255`. The full verb list lives in the [Lua reference](./lua).

## Putting it together: a scripted HUD

Here's a small, complete script that spawns a HUD, keeps the health bar filled, and switches to a game-over screen when health hits zero:

```lua
-- hud.lua — attach this to one entity in the scene
function props()
    return {
        max_health = { value = 100, hint = "Player max HP" },
        _hp        = { value = 100, hint = "Current HP" },
        _score     = { value = 0,   hint = "Score" },
    }
end

function on_ready()
    action("hui_spawn", { template = "ui/hud.html" })
    _hp = max_health
    _score = 0
end

function on_update()
    -- ui/hud.html shows {{ _hp }} and {{ _score }} directly,
    -- and we size the health bar from the current fraction.
    set_on("health_fill", "UiBarFill.value", _hp / max_health)

    if _hp <= 0 then
        action("hui_despawn", { template = "ui/hud.html" })
        action("hui_spawn",   { template = "ui/game_over.html" })
    end
end

-- Buttons in the markup route here by their on_press="..." name
function on_ui(name, args, entity)
    if name == "restart" then
        _hp = max_health
        _score = 0
    end
end
```

> UI scripting (`action()`, `set_on`/`get_on`, and `on_ui`) runs through the script VM — write your UI logic in `.lua`.

## See also

- [Scripting Overview](./overview) — backends, hooks, and the `action()` escape hatch.
- [Lua reference](./lua) — the full function catalog, including `set_on`/`get_on`.
- [Scripting API](/docs/r1-alpha7/api/scripting) — the complete UI tag, binding, and widget reference.
