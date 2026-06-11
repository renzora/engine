# Scripting Overview

Scripting is how you give things in your game *behavior* — make a door open, a coin spin, an enemy chase the player. The good news: it's completely optional, and you can mix and match the approach that feels best to you.

Renzora gives you three ways to add logic, and they all work together:

- **Blueprints** — a no-code, drag-and-connect visual system. Great if you'd rather not write code.
- **Lua** — a friendly, popular scripting language. The most fully-featured option.
- **Rhai** — a lighter scripting language, handy when you export your game to the **web**.

You can use one, or several at once, even on the same object. Start wherever you're comfortable.

## Prefer no code? Use Blueprints

If writing code isn't your thing, you don't have to. **Blueprints** let you build behavior by dropping nodes onto a canvas and wiring them together — "when this happens, do that."

It's a full system on its own, so it has its own guide. See **[Blueprints](./blueprints)** to get started with the visual editor.

The rest of this page is a gentle look at the text-script side (Lua and Rhai).

## The code editor

Renzora has a built-in code editor, so you never have to leave the engine to write a script. Open the **Code** tab and you'll get a tidy editor with tabs for each open file, syntax highlighting, and the file path along the bottom.

![The built-in Code editor showing a Lua car-physics script, with tabs for several open .lua files and the file path along the bottom.](/assets/previews/code_editor.png)

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

The same idea written in Rhai looks almost identical:

```rhai
// bob.rhai
fn on_update() {
    let bob = sin(elapsed * 2.0) * 0.1;
    set_position(position_x, position_y + bob, position_z);
}
```

## Attaching a script to an object

In the editor:

1. Select the object you want to bring to life.
2. In its properties, add a **script entry**.
3. Point that entry at a file in your project's `scripts/` folder.

That's it — press play and the script runs. Edit and save the file and it **hot-reloads** automatically, so you can tweak numbers and see the change without restarting.

> Tip: an object becomes scriptable as soon as it has a name, so most of the time the script slot is already waiting for you.

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

## Lua or Rhai?

Both run inside the same engine, and you pick simply by the file extension — `.lua` files run as Lua, `.rhai` files run as Rhai. A quick rule of thumb:

- **Lua** — your default. It has the largest set of built-in functions (input, physics, audio, animation, networking, and more).
- **Rhai** — reach for this when you're exporting to the **web**, where Lua isn't available. It supports a smaller set of features.

## Where to go next

This page is just the warm-up. When you're ready for the full toolbox:

- **[Lua reference](./lua)** — every lifecycle hook and built-in function, with examples.
- **[Rhai reference](./rhai)** — the supported subset for native and web.
- **[Scripting API](/docs/r1-alpha5/api/scripting)** — the complete function catalog.
- **[Blueprints](./blueprints)** — the no-code visual option.
