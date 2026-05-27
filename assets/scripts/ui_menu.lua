-- UI Menu — demonstrates authoring UI in markup (bevy_hui) and handling its
-- callbacks from Lua. Attach to ONE entity.
--
-- Pairs with assets/ui/example_menu.html, whose <menu_button>s carry
-- `action="start_game"` etc. Because those names have no Rust binding, renzora_hui
-- forwards them here as on_ui(name, args, entity).

-- Spawn the markup menu once when this entity comes alive.
function on_ready()
    action("hui_spawn", { template = "ui/example_menu.html" })
end

-- Called for every markup callback with no Rust binding (broadcast to all
-- scripts, like on_rpc).
--   name   the markup callback name (value of on_press / on_change / on_spawn)
--   args   table of the node's `tag:`-prefixed attributes (all strings)
--   entity raw entity bits of the firing node
function on_ui(name, args, entity)
    if name == "start_game" then
        -- `tag:scene="level_01"` on the button arrives as args.scene
        print("Start game -> scene " .. tostring(args.scene))
    elseif name == "open_settings" then
        print("Open settings")
    elseif name == "open_credits" then
        print("Open credits")
    elseif name == "quit_game" then
        print("Quit requested")
    end
end
