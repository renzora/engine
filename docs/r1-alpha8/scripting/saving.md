# Saving the Game

A save keeps the values your game chooses (the level reached, coins, which doors are open, the player's settings) in a **slot** on the player's device, so the next session can pick up where this one stopped.

Saves hold named values, not a copy of the scene. You decide what matters, and a save made before you edited a level still loads after you edit it.

## Values first, then a slot

Saving has two steps:

1. **Set values** as the game goes on. They are kept in memory, shared by every script.
2. **Write** them to a slot when the player reaches a checkpoint or picks Save. Only then do they reach the disk.

Loading is the reverse: it replaces the values in memory with the slot's.

```lua
-- checkpoint.lua: save when the player walks through
function on_ready()
    if save_exists("autosave") then
        save_load("autosave")
    end
    coins = save_get("coins", 0)
end

function on_update()
    if is_key_just_pressed("E") then
        coins = coins + 1
        save_set("coins", coins)
    end
end

function on_event(name, args)
    if name == "checkpoint_reached" then
        save_set("level", args.level)
        local ok, err = save_write("autosave")
        if not ok then
            print("could not save: " .. err)
        end
    end
end
```

A value set but never written is gone when the game quits. That is what lets a player who dies go back to their last save.

## Values

| Function | Returns | Description |
|---|---|---|
| `save_set(key, value)` | | Store a value. A number, boolean, string, or a `{x=, y=}` / `{x=, y=, z=}` table |
| `save_get(key, default)` | value | The stored value, or `default` if there is none |
| `save_has(key)` | bool | True if the key has a value |
| `save_remove(key)` | bool | Forget one value; true if it was there |
| `save_clear()` | | Forget every value, for a new game. Slots on disk are not touched |
| `save_keys()` | list | Every key with a value |

Whole numbers come back as whole numbers: `save_set("coins", 42)` reads back `42`, not `42.0`.

## Slots

| Function | Returns | Description |
|---|---|---|
| `save_write(slot)` | ok, message | Write every value to `slot`, replacing what it held |
| `save_load(slot)` | ok, message | Replace every value with what `slot` holds |
| `save_exists(slot)` | bool | True if `slot` has a save |
| `save_delete(slot)` | ok, message | Remove `slot`. Removing one that is not there is not an error |
| `save_slots()` | list | Every slot as `{ name = "slot1", saved_at = 1730000000 }`, newest first |

A slot name is 1 to 64 letters, digits, `-` or `_`: `"autosave"`, `"slot_2"`, `"quick-save"`.

Writing and loading return two values, `ok` and `message`. On failure `ok` is `false` and `message` says why, ready to show the player.

- A failed **load** leaves the values as they were, so a damaged save does not also wipe the game in progress.
- A **write** goes to a temporary file first and replaces the old save only once it is complete, so a crash or a full disk mid-save leaves the previous save intact.

`saved_at` is seconds since 1970, for showing "saved 2 hours ago" in a load menu.

```lua
-- load_menu.lua: list the saves, newest first
for _, slot in ipairs(save_slots()) do
    print(slot.name .. " saved at " .. slot.saved_at)
end
```

## Where saves are kept

Each slot is one `<slot>.json` file in a `saves` folder named for your game (the `name` in `project.toml`):

| Platform | Folder |
|---|---|
| Windows | `%APPDATA%\<game>\saves` |
| macOS, iOS | `~/Library/Application Support/<game>/saves` |
| Linux | `~/.local/share/<game>/saves` (or `$XDG_DATA_HOME/<game>/saves`) |
| Android | the app's private data folder, `/saves` |
| Web | kept in memory until the tab closes |

Pressing **Play** in the editor runs your game the same way a player's copy does, so a playtest writes to this same folder. Delete the folder to start fresh.

The files are plain JSON and the player can read and edit them. Keep anything a player should not be able to change on a server, not in a save.

Renaming the game in `project.toml` changes the folder name, so saves made under the old name stop showing up.

## Rust scripts

The same store is `renzora::save`:

```rust
use renzora::component::ScriptValue;

renzora::save::set("coins", ScriptValue::Int(42));
if let Err(e) = renzora::save::write("autosave") {
    warn!("could not save: {e}");
}
```

Lua and Rust scripts share one set of values, so either can write what the other reads.

## Related

- [Scripting Overview](./overview)
- [Rust Scripts](./rust-scripts)
