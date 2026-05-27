# renzora_hui

Author `bevy_ui` as hot-reloadable, HTML/XML-style markup instead of Rust spawn
code — wrapping [`bevy_hui`](https://crates.io/crates/bevy_hui) and bridging it
into Renzora's Lua scripting and editor canvas.

## Authoring UI

Templates live under `assets/ui/`. Reusable component templates under
`assets/ui/components/` are auto-registered by file stem (so a file
`menu_button.html` becomes the `<menu_button>` tag).

```html
<!-- assets/ui/example_menu.html -->
<template>
    <node id="menu_root" flex_direction="column" align_items="center">
        <menu_button text="Start Game" action="start_game" tag:scene="level_01" />
        <menu_button text="Quit"       action="quit_game" />
    </node>
</template>
```

Spawn it from Rust:

```rust
cmd.spawn(bevy_hui::prelude::HtmlNode(server.load("ui/example_menu.html")));
```

…or from a Lua script:

```lua
action("hui_spawn", { template = "ui/example_menu.html" })
```

## Driving game logic from markup

A markup callback (`on_press`, `on_change`, `on_spawn`) resolves through
bevy_hui's function bindings. If a name is bound to a Rust one-shot system it
runs that. **Otherwise it falls through to every script's `on_ui` hook** —
broadcast, just like `on_rpc`:

```lua
-- the value of on_press / action="" arrives as `name`;
-- `tag:`-prefixed attributes arrive as the `args` table;
-- `entity` is the firing node's raw entity bits.
function on_ui(name, args, entity)
    if name == "start_game" then
        load_scene(args.scene)   -- tag:scene="level_01"
    elseif name == "quit_game" then
        quit()
    end
end
```

Rust bindings take precedence; Lua is the fallback. See
`assets/scripts/ui_menu.lua` for a complete example.

## Editor (feature `editor`)

**Create one** from the hierarchy's **+ Add Entity → UI → "HTML Template"**. That
spawns a full-screen UI Canvas with a template child (pointed at
`ui/example_menu.html` by default) so it renders in the canvas preview. Select
the child and use the inspector's **Template** field to pick any `.html`.

Every node a template builds is tagged as a `renzora_game_ui` widget, so the
canvas preview shows the template's entities and lets you select and drag them.
Dragging a node that has a markup `id` records its position into a
`HuiLayoutOverrides` component on the spawning entity; the override is saved with
the scene and re-applied after each hot-reload, so layout tweaks survive template
edits. The `.html` file is never rewritten — markup stays the source of structure.

### `HtmlTemplatePath`

Which template an entity displays is stored as a serializable
`HtmlTemplatePath(String)` component (since `HtmlNode`'s `Handle` doesn't
round-trip through scenes). A runtime observer turns the path into a loaded
`HtmlNode`, so scene-authored templates load in **exported games**, not just the
editor. Set the path from Rust or let the inspector manage it.

> Known limitation: changing the path on an *already-built* entity reloads the
> handle but bevy_hui won't rebuild in place (it marks built trees `FullyBuild`).
> Re-create the entity, or hot-reload the file, to see a different template.
