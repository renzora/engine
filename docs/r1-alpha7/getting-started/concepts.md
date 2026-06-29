# Core Concepts

Renzora builds your whole game out of a few simple ideas. Once you know what an *entity*, a *component*, and a *scene* are, the rest of the editor clicks into place — no coding required to follow along.

Under the hood Renzora runs on the **Bevy 0.19** engine, but you don't need to know Bevy to make a game. You'll mostly work visually, and these three words cover almost everything you see on screen.

## Entities: the "things" in your world

An **entity** is just a *thing* in your game — a character, a light, a camera, a tree, a sound. Every object you place in a scene is an entity.

The **Hierarchy** panel lists every entity in the scene you have open.

![The Hierarchy panel listing entities such as Terrain, World Environment, Camera, and an imported Bistro_Godot.glb model expanded to show its child pieces.](/assets/previews/hierarchy.png)

In the editor you can:

- Click **+ Add Entity** to create a new thing.
- Click any row to select it, then edit it in the Inspector (next section).
- Click the **eye** icon to hide or show an entity.
- Click the **lock** icon so you can't move it by accident.
- Drag one row onto another to **nest** it — child entities follow their parent when it moves. In the screenshot the imported `Bistro_Godot.glb` model holds many child pieces.
- Type in the **Search** box to find an entity by name fast.

On its own an entity is empty. It's the *components* you attach that give it a shape, a position, and behaviour.

## Components: the parts that make an entity what it is

A **component** is a single piece you attach to an entity. Each one adds one capability, and stacking a few together builds something real:

- A **Transform** gives it a position, rotation, and scale.
- A **Mesh** + **Material** give it a 3D model and how it looks.
- A **Light** makes it shine.
- A **rigid body / collider** lets the physics engine push it around.
- A **script** gives it custom behaviour.

Select an entity in the Hierarchy and its components show up in the **Inspector** panel, each in its own collapsible section.

![The Inspector panel for the selected "World Environment" entity, showing its Name, Transform, Visibility, Directional Light, Volumetric Light, and TAA components.](/assets/previews/inspector.png)

In the Inspector you can:

- Edit values directly — type a number, drag it, flip a switch, or pick a color.
- Click **Add** to attach a new component.
- Use **Filter components...** to jump to one quickly.
- Click the trash icon on a section to remove that component.

In the shot above, the selected "World Environment" entity carries a Transform (its rotation), a Directional Light (the sun — here an Illuminance of 40000 with shadows on), and a few rendering options. They're all just components layered onto one entity.

> **For programmers:** components are plain Rust data structs, and *systems* are functions that read and change them each frame. You can write your own and make their fields editable right here in the Inspector. See [Creating Components](/docs/r1-alpha5/engine-core/components) for the full guide.

## Scenes: a saved world

A **scene** is a saved collection of entities and their components — think of it as one level, room, or screen of your game. Saving a scene writes a `.ron` file that records everything you built in the Hierarchy.

You choose which scene loads first in your project's `project.toml`:

```toml
name = "MyProject"
version = "1.0.0"
main_scene = "scenes/main.ron"
```

You can split a big game across many scenes and load them as the player moves between areas. The editor remembers the last scene you had open, while the exported game always starts from `main_scene`.

## Giving entities behaviour with scripts

To make things *do* something — move, take damage, open a door — you attach a **script** to an entity. Renzora gives you three ways, and you can mix them in one project:

- **Blueprints** (`.blueprint`) — visual node graphs you wire together. Great if you'd rather not write code.
- **Lua** (`.lua`) — a friendly text scripting language with the full Renzora API (native desktop and mobile).
- **Rhai** (`.rhai`) — a lighter scripting language that also runs on the **Web** export.

Just give an entity a **Name** in the editor and it's ready to hold scripts. See the [Scripting Overview](/docs/r1-alpha5/scripting/overview) to get started.

## How a frame works

You never have to drive the game loop yourself. Every frame, the engine does roughly this for you:

1. Read input (keyboard, mouse, gamepad).
2. Run your scripts and game logic.
3. Step the physics (Renzora uses the **Avian** physics engine).
4. Update positions and animations.
5. Draw the frame.

You just describe your entities, components, and scripts — the engine runs everything in the right order, on every platform you export to (Windows, Linux, macOS, Android, iOS, and Web).

## What's next?

- [Your First Project](/docs/r1-alpha5/getting-started/first-project) — create a project and open the editor
- [Scenes & Hierarchy](/docs/r1-alpha5/editor/scenes) — build a world in the editor
- [Scripting Overview](/docs/r1-alpha5/scripting/overview) — add gameplay logic
- [Creating Components](/docs/r1-alpha5/engine-core/components) — write your own components in Rust
- [Building Plugins](/docs/r1-alpha5/extending/plugins) — extend the engine itself
