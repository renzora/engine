# Core Concepts

Renzora builds a whole game out of three ideas: entities, components and scenes. Once those click, the rest of the editor follows.

## Entities

An entity is a thing in your game. A character, a light, a camera, a tree, a sound. Everything you place in a scene is an entity.

The **Hierarchy** panel lists every entity in the scene you have open.

![The Hierarchy panel listing entities such as Terrain, World Environment, Camera and an imported model expanded to show its child pieces.](/assets/previews/hierarchy.png)

Click **+ Add Entity** to make one, click a row to select it, and drag one row onto another to nest it. A nested entity follows its parent when the parent moves, which is how an imported model keeps its pieces together.

On its own an entity is empty. Components are what make it something.

## Components

A component is one piece attached to an entity. Each adds one capability, and stacking a few builds something real.

| Component | Gives the entity |
|---|---|
| Transform | A position, rotation and scale |
| Mesh and Material | A shape and how its surface looks |
| Light | The ability to light the scene |
| Rigid Body and Collider | Physics |
| Scripts | Behaviour of your own |

Select an entity and its components fill the **Inspector**, each in its own collapsible section.

![The Inspector for a selected entity showing its Name, Transform, Visibility and Directional Light components.](/assets/previews/inspector.png)

Edit values directly by typing, dragging or picking a colour. **Add** attaches a new component. The trash icon on a section removes one.

## Scenes

A scene is a saved collection of entities and their components. Think of it as one level, room or screen. Saving writes a `.bsn` file that records everything in the Hierarchy.

Your project picks which scene loads first, under **Settings > Project**. You can split a game across many scenes and load them as the player moves between areas. The editor reopens whatever scene you last had open, while an exported game always starts from the boot scene.

## Behaviour

To make something happen you attach a script to an entity. Renzora compiles Rust scripts, and language backends from the [Marketplace](/docs/r1-alpha8/marketplace/browsing) add others.

See [Scripting Overview](/docs/r1-alpha8/scripting/overview).

## How a frame works

You never drive the game loop yourself. Every frame the engine reads input, runs your scripts, steps physics, updates animation and transforms, then draws.

You describe entities, components and scripts. The engine runs them in the right order, on every platform you export to.

## What's next

- [Your First Project](/docs/r1-alpha8/getting-started/first-project)
- [Scenes and Hierarchy](/docs/r1-alpha8/editor/scenes)
- [Scripting Overview](/docs/r1-alpha8/scripting/overview)
