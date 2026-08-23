# Scripting Overview

Scripting is how you give things in your game *behavior* — make a door open, a coin spin, an enemy chase the player. The good news: it's completely optional, and you can mix and match the approach that feels best to you.

Renzora gives you two ways to add logic, and they work together:

- **Blueprints** — a no-code, drag-and-connect visual system. Great if you'd rather not write code.
- **Lua** — a friendly, popular scripting language, with the full set of built-in functions.

You can use either, or both at once, even on the same object. Start wherever
you're comfortable.

> **Scripting languages are plugins.** Lua arrives as `plugins/lua`, which the
> editor ships with. That is why a language can be added without touching the
> engine — see
> [Script Backends](/docs/r1-alpha7/extending/script-backends) if you want to
> bring your own.

## Prefer no code? Use Blueprints

If writing code isn't your thing, you don't have to. **Blueprints** let you build behavior by dropping nodes onto a canvas and wiring them together — "when this happens, do that."

It's a full system on its own, so it has its own guide. See **[Blueprints](./blueprints)** to get started with the visual editor.

The rest of this page is a gentle look at the text-script side.

## The code editor

Renzora has a built-in code editor, so you never have to leave the engine to write a script. Open the **Code** tab and you'll get a tidy editor with tabs for each open file, syntax highlighting, and the file path along the bottom.

![The built-in Code editor showing a Lua car-physics script, with tabs for several open .lua files and the file path along the bottom.](/assets/previews/code_editor.png)

You can open scripts a few ways: double-click a `.lua` file in the asset
browser — which opens it as a [document tab](/docs/r1-alpha7/editor/scenes#working-with-document-tabs)
and takes you to the Scripting workspace, exactly as double-clicking a material
opens one and takes you to Materials — drop one onto the editor, or **select an
entity** — the code editor
follows your selection and shows that entity's editable sources: every script
attached to it, one tab per script, with the first focused. UI works the same
way — select a template and its `.html` opens; select a **UI Canvas** and every
template under it opens as tabs. Switching to another entity *replaces* the tabs
with the new entity's sources (it isn't additive); any tab with unsaved changes
is kept so you never lose edits. Selecting an entity with no editable source
leaves the editor as it was.

Scripts live in your project's `scripts/` folder. Each script is just a text file with a few functions in it that the engine calls for you at the right moments.

## A tiny example

Here's about as small as a Lua script gets — it gently bobs an object up and down forever:

```lua
-- bob.lua
function on_update()
    local bob = math.sin(elapsed * 2.0) * 0.1
    set_position(position_x, position_y + bob, position_z)
end
```

A few things to notice:

- `on_update()` is a **lifecycle hook** — a function the engine runs automatically every frame. There are a couple of others, like `on_ready()` (runs once at the start).
- `elapsed`, `position_x`, and friends are **context values** the engine fills in for you each frame, so you can read where the object is and how much time has passed.
- `set_position(...)` is one of many built-in functions for acting on the world.

## Attaching a script to an object

In the editor:

1. Select the object you want to bring to life.
2. In its properties, add a **script entry**.
3. Point that entry at a file in your project's `scripts/` folder.

That's it — press play and the script runs. Edit and save the file and it **hot-reloads** automatically, so you can tweak numbers and see the change without restarting.

> Tip: an object becomes scriptable as soon as it has a name, so most of the time the script slot is already waiting for you.

## Previewing one script without play mode

Sometimes you just want to see *this one* script run — a UI canvas animation, a spinning coin — without entering full play mode and running everything else. Each script entry has a **play button** in its header (next to the enable toggle). Press it and that single script starts running live in edit mode; the icon turns into a green **pause** while it's active. Press it again to stop.

It's the fastest way to iterate on a `on_draw` HUD or a small animated behavior: leave preview on, edit the file, and hot-reload shows your change immediately. Preview only ever runs the scripts you've explicitly toggled — the rest of the scene stays still — and it never touches your saved scene (the preview flag isn't serialized). Entering real play mode ignores it and runs everything as usual.

## Exposing settings in the editor

You'll often want a knob you can tweak in the editor without touching code — a speed, a color, a damage number. Add a `props()` function and those values show up as editable fields next to your object:

```lua
function props()
    return {
        speed  = { value = 10.0, hint = "Movement speed" },
        damage = { value = 25,   hint = "Hit damage" },
    }
end
```

Whatever value you give is also the **type** (a decimal becomes a number, `true`/`false` becomes a checkbox, and so on), and the `hint` text shows up as a helpful tooltip.

## Which file extensions run?

A script runs if a language plugin has claimed its extension. With the shipped
`plugins/lua`, that means `.lua` — plus `.blueprint`/`.bp`, which the engine
compiles to Lua for it.

Remove that plugin and `.lua` files simply stop running; add another language
plugin and its extension starts working alongside Lua, on the same project.

## Where to go next

This page is just the warm-up. When you're ready for the full toolbox:

- **[Lua reference](./lua)** — every lifecycle hook and built-in function, with examples.
- **[Scripting API](/docs/r1-alpha7/api/scripting)** — the complete function catalog.
- **[Blueprints](./blueprints)** — the no-code visual option.
