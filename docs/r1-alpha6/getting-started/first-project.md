# Your First Project

Let's make something move on screen. In this guide you'll create a project, drop an object into your scene, give it a quick script, and press Play — all inside the editor, no setup gymnastics required.

## Open the editor

You can get Renzora two ways: grab a prebuilt build from [renzora.com/download](/download), or install the command-line tool with `cargo install renzora`. Either way, the next step is the same — open the editor. (The [Installation guide](/docs/r1-alpha5/getting-started/installation) has the exact commands for your platform.)

When the editor starts you'll land on a project picker. Click **New Project**, type a name, and choose a folder on your computer. That's it — the editor builds the project and opens its starting scene, ready to go.

## What's inside a new project

A fresh project is just a few files on disk:

```text
my-game/
├── project.toml      # your game's settings
├── scenes/
│   └── main.ron      # the scene that loads first (empty to start)
└── plugins/          # optional drop-in plugins
```

You'll add more folders as you grow — `assets/` for models, textures, and sounds, and `scripts/` for your `.lua` and `.rhai` files. Your starting scene, `scenes/main.ron`, is valid but empty:

```ron
(
  resources: {},
  entities: {},
)
```

You usually never touch `project.toml` by hand — the editor's Settings panel manages it for you. If you do peek inside, just three keys really matter:

```toml
name = "My Game"
version = "0.1.0"
main_scene = "scenes/main.ron"   # the scene that loads at startup
```

There's a `[window]` section for size and an optional autoload list and more. You can leave all of that alone for now and let the editor's Settings panel manage it.

## Add your first object

Your new scene is empty, so let's put something in it.

In the **Hierarchy** panel, click **+ Add Entity** at the top. A search overlay pops up with everything you can drop into a scene — basic shapes like Cube and Sphere, lights, cameras, and more — sorted into categories down the left.

![The Add Entity overlay: search or browse categories like Lighting and Camera to drop shapes, lights, and cameras into your scene.](/assets/previews/add_entity.png)

Pick **Cube**. It appears in the middle of your scene and is selected automatically. Then click **+ Add Entity** again and add a **Directional Light** so your cube isn't sitting in the dark.

## See it in the viewport

The big window in the center is the **viewport** — your live 3D view of the scene. Your new cube is sitting at the center.

With the cube selected, a colored handle (a "gizmo") appears on it. Drag the arrows to move it, and use the toolbar to switch between the Move, Rotate, and Scale tools. Made a mistake? `Ctrl+Z` undoes it.

![An object selected in the viewport, with the colored gizmo you drag to move it around the scene.](/assets/previews/viewport.png)

## Find it in the Hierarchy

Every object you add shows up in the **Hierarchy** panel as a list. This is your scene's table of contents — click any entry to select that object, and objects can be nested inside others to keep things tidy.

![The Hierarchy panel lists everything in the scene, with the + Add Entity button at the top.](/assets/previews/hierarchy.png)

## Tweak it in the Inspector

Select your cube and look at the **Inspector** panel. This is where you change an object's properties.

You'll see its **Name**, a **Transform** section with Position, Rotation, and Scale, a **Visibility** toggle, and a section for each component the object has. Type new numbers into the Transform fields and watch the cube update in the viewport instantly.

![The Inspector showing a selected object's properties: name, transform (position, rotation, scale), and component settings.](/assets/previews/inspector.png)

## Make it move

A little script will make the cube spin. Scripts are plain text files, and Renzora picks the language by the file extension — `.lua` runs Lua, `.rhai` runs Rhai. (Rhai also works in web exports, where Lua doesn't.)

Create a file at `scripts/spin.lua`:

```lua
-- Spins the entity continuously.
function props()
    return {
        speed = { value = 45.0, hint = "degrees per second" },
    }
end

function on_update()
    rotate(0, speed * delta, 0)   -- spin around the Y axis
end
```

Two friendly things to know:

- Anything you return from `props()` shows up in the Inspector, so you can tweak it without editing code. Here, `speed` becomes a slider-friendly value you can change live.
- `on_update()` runs every frame. `delta` is the time since the last frame, which keeps the spin smooth at any frame rate.

To attach the script: select the cube, find the **Scripts** section in the Inspector, and point it at `scripts/spin.lua`. The full list of functions you can call (moving, input, audio, and more) lives in the [Scripting API](/docs/r1-alpha5/api/scripting).

## Press Play

Hit **`F5`** to play. The viewport switches to your game and the cube starts spinning. Press `F5` again to stop and go back to editing.

While it's running, change `speed` in the Inspector — the spinning cube picks up the new value right away.

## Save your work

Press **`Ctrl+S`** to save. Your scene is written to a `.ron` file in the `scenes/` folder. Renzora only saves what you actually authored (not temporary runtime data), so scene files stay small and easy to read.

## What's next?

- [Editor Overview](/docs/r1-alpha5/getting-started/editor-overview) — a tour of every panel.
- [Core Concepts](/docs/r1-alpha5/getting-started/concepts) — how scenes, entities, and scripts fit together.
- [Scripting Overview](/docs/r1-alpha5/scripting/overview) — Lua, Rhai, and visual Blueprints.
