# Your First Project

Make something move on screen. You will create a project, drop an object into the scene, give it a script, and press Play.

## Create the project

Open the editor. You land on the [dashboard](/docs/r1-alpha8/getting-started/dashboard), on its Projects page.

Click **New Project** and choose a folder. The folder's name becomes the project's name. The editor creates the project and opens its starting scene.

<!-- screenshot: dashboard_new_project.png - the New Project flow, folder picker -->

A fresh project is two things on disk:

```text
my-game/
├── project.toml
└── scenes/
    └── main.bsn
```

`project.toml` is your game's settings, managed by **Settings > Project**. `main.bsn` is the scene that loads first, empty to start with.

You will add more folders as you go. The editor makes them when it needs them.

### The walkthrough

The first time you open the editor a small card appears in the corner and walks you through the basics: moving the camera, selecting, using the gizmo, then panels, Settings and themes.

Each step asks you to actually do the thing, and the editor notices when you have. Steps that point at a button glow the target and float an arrow at it. Drag the card by its header if it is in the way.

<!-- screenshot: tutorial_card.png - the tutorial card with a glowing target and green arrow pointing at a button -->

There is more than one chapter. Finish the first and **Help > Getting Started Tutorial** reopens at the chapter list. Chapters unlock in order.

**Skip** moves past one step. The **✕** closes the tutorial and counts as seen, so it will not open itself again.

## Add an object

In the **Hierarchy** panel, click **+ Add Entity**. A search overlay opens with everything you can put in a scene: shapes, lights, cameras and more, sorted into categories.

![The Add Entity overlay: search or browse categories such as Lighting and Camera to drop shapes, lights and cameras into your scene.](/assets/previews/add_entity.png)

Pick **Cube**. It appears in the middle of the scene, already selected. Click **+ Add Entity** again and add a **Directional Light** so the cube is not sitting in the dark.

## Move it around

The big window in the middle is the viewport. Your cube is at the centre, with a coloured gizmo on it.

![An object selected in the viewport with the coloured gizmo you drag to move it.](/assets/previews/viewport.png)

Drag the arrows to move it. Use the toolbar along the viewport's top edge to switch between Move, Rotate and Scale. `Ctrl+Z` undoes.

To look around: right-drag to look, `W` `A` `S` `D` to fly while holding right mouse, scroll to zoom, and `F` to focus whatever is selected.

## Change its properties

With the cube selected, look at the **Inspector**. At the top is the entity header: its icon, its ID, its label colour and an eye to hide it. Below that is **Transform** with Position, Rotation and Scale, plus one section per component.

![The Inspector showing a selected object's name, transform and component settings.](/assets/previews/inspector.png)

Type new numbers into the Transform fields and watch the cube move.

## Make it spin

Scripts give an entity behaviour. Renzora compiles Rust scripts on save.

In the **Assets** panel, click **Add** and pick **Rust Script**. You get `new_script.rs`, and the file opens in the code editor with a working starter in it:

```rust
use bevy::prelude::*;
use renzora::ScriptCtx;

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if let Some(mut transform) = ctx.get_mut::<Transform>() {
        transform.rotate_y(dt);
    }
}

renzora::script!(update);
```

`update` runs once per frame, for each entity the script is attached to. `ctx` is that entity plus the whole world. This one spins whatever it is on around its Y axis.

Press `Ctrl+S` to save. The script compiles in the background, which takes about a second the first time. Watch the **Console** for the result.

### Attach it

Drag `new_script.rs` from the Assets panel onto your cube in the Hierarchy. Or select the cube, find the **Scripts** component in the Inspector, and click **Add Script**.

<!-- screenshot: script_component.png - the Scripts component in the Inspector with a .rs script attached -->

## Press Play

Hit **Play** in the top bar, or `F5`.

The editor swaps to your game camera, hides its own gizmos and grid, and runs. Your cube spins. **Stop** puts everything back exactly as it was, so nothing you do while playing changes your scene.

Try **Simulate** instead. That runs the script while leaving the editor live, so you can keep your camera, select the cube mid-spin, and watch its rotation change in the Inspector.

## Save

`Ctrl+S` saves the scene.

One thing to know: only named entities are saved. Everything you add from **+ Add Entity** gets a name automatically, so this rarely comes up, but if something vanishes after a reload, check it has a name.

## What's next

- [Scenes and Hierarchy](/docs/r1-alpha8/editor/scenes) for building a real level
- [Rust Scripts](/docs/r1-alpha8/scripting/rust-scripts) for what a script can do
- [Materials](/docs/r1-alpha8/editor/materials) for making surfaces look right
- [Export Overview](/docs/r1-alpha8/exporting/overview) for shipping it
