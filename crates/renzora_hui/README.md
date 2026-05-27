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

## Editor canvas (feature `editor`)

When the `editor` feature is on, every node a template builds is tagged as a
`renzora_game_ui` widget, so the existing UI canvas preview shows the template's
entities and lets you select and drag them. Dragging a node that has a markup
`id` records its position into a `HuiLayoutOverrides` component on the spawning
`HtmlNode` entity; the override is saved with the scene and re-applied after each
hot-reload, so layout tweaks survive template edits. The `.html` file is never
rewritten — markup stays the source of structure.
