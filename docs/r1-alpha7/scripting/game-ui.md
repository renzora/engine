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

There are a few more tags for repeating lists and reusing components. See the [Scripting API](/docs/r1-alpha5/api/scripting) for the full list.

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

The `value`, `data`, and `readout` fields accept live `{{ }}` bindings, so a fuel gauge or speedometer tracks your game in real time. For the full list of widget options, see the [Scripting API](/docs/r1-alpha5/api/scripting).

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

> UI scripting (`action()`, `set_on`/`get_on`, and `on_ui`) is **Lua-only** — write your UI logic in `.lua`. Rhai scripts can still feed values into `{{ }}` bindings, but they can't call the UI verbs.

## See also

- [Scripting Overview](./overview) — backends, hooks, and the `action()` escape hatch.
- [Lua reference](./lua) — the full function catalog, including `set_on`/`get_on`.
- [Scripting API](/docs/r1-alpha5/api/scripting) — the complete UI tag, binding, and widget reference.
