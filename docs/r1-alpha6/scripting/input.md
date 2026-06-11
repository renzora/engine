# Input Handling

Read keyboard, mouse, and gamepad input from scripts, and rebind it through the project's action map.

Renzora exposes input to scripts in two layers. The quickest is a set of **read-only context globals** (`input_x`, `mouse_x`, `gamepad_south`, …) that the backend refreshes before every hook. On top of that, the `renzora_input` crate provides a rebindable **action map** (`InputMap` / `ActionState`) that unifies keyboard, mouse, and gamepad behind named actions like `"move"` and `"jump"`, queried with the `input_button_*` / `input_axis_*` functions.

> The context globals are inputs, not state you own. Assigning to `position_x` or `input_x` does nothing — only variables declared in `props()` are read back. To move an entity, call a transform function such as `translate(x, y, z)` (see [Lua](/docs/r1-alpha5/scripting/lua#moving-and-transforming)).

## Lua and Rhai differences

Both backends receive the same read-only input globals each frame, but the query *functions* differ:

| Capability | Lua (`.lua`) | Rhai (`.rhai`) |
|---|---|---|
| Input context globals (`input_x`, `mouse_*`, `gamepad_*`) | Yes | Yes |
| `is_key_pressed` family | `is_key_pressed("KeyW")` | `is_key_pressed(_keys_pressed, "KeyW")` (must pass the key map) |
| Action map (`input_button_*`, `input_axis_1d/2d`) | Yes | **No** |

For movement that should work everywhere, prefer the `input_x` / `input_y` and `gamepad_*` globals — they already combine keyboard and gamepad and are available in both backends. The action-mapped helpers are Lua-only; see [What Rhai can't do](/docs/r1-alpha5/scripting/rhai#what-rhai-cant-do).

## Movement axes

`input_x` and `input_y` are a normalized movement vector built from **WASD and the arrow keys**:

- `input_x`: `-1` for A / Left, `+1` for D / Right.
- `input_y`: `-1` for S / Down, `+1` for W / Up.
- Diagonals are normalized to unit length; no key gives `0`.

```lua
function on_update()
    local speed = 5.0
    -- input_x / input_y are read-only; call translate to actually move.
    translate(input_x * speed * delta, 0, input_y * speed * delta)
end
```

```rhai
fn on_update() {
    let speed = 5.0;
    translate(input_x * speed * delta, 0.0, input_y * speed * delta);
}
```

> These two globals are wired to WASD/arrows directly, independent of the action map. For a stick-aware, rebindable version use the `"move"` action (`input_axis_2d("move")`, Lua only) described below.

## Mouse

All of these are read-only globals, present in both backends.

| Global | Type | Description |
|---|---|---|
| `mouse_x`, `mouse_y` | number | Cursor position in window pixels |
| `mouse_delta_x`, `mouse_delta_y` | number | Cursor movement since last frame (use for camera look) |
| `mouse_scroll` | number | Scroll wheel delta this frame |
| `mouse_left`, `mouse_right`, `mouse_middle` | bool | Button held |
| `mouse_left_just_pressed`, `mouse_right_just_pressed` | bool | Button pressed this frame |

```lua
function on_update()
    -- Mouse look: feed delta into rotation each frame.
    rotate(0, -mouse_delta_x * 0.2, 0)

    if mouse_left_just_pressed then
        print("clicked at " .. mouse_x .. ", " .. mouse_y)
    end
end
```

> There is no `mouse_middle_just_pressed` global, and the scroll value is a single number, not a vector.

## Gamepad

The first connected gamepad is exposed through read-only globals. Sticks and triggers are analog; buttons are booleans.

| Global | Type | Description |
|---|---|---|
| `gamepad_left_x`, `gamepad_left_y` | number | Left stick axes (-1..1) |
| `gamepad_right_x`, `gamepad_right_y` | number | Right stick axes (-1..1) |
| `gamepad_left_trigger`, `gamepad_right_trigger` | number | Analog trigger pressure (0..1) |
| `gamepad_south` / `east` / `west` / `north` | bool | Face buttons (Xbox A/B/X/Y, PlayStation Cross/Circle/Square/Triangle) |
| `gamepad_l1` / `r1` | bool | Bumpers (LeftTrigger / RightTrigger) |
| `gamepad_l2` / `r2` | bool | Triggers as digital buttons (LeftTrigger2 / RightTrigger2) |
| `gamepad_l3` / `r3` | bool | Stick clicks (LeftThumb / RightThumb) |
| `gamepad_select`, `gamepad_start` | bool | Menu buttons |
| `gamepad_dpad_up` / `down` / `left` / `right` | bool | D-pad |

```lua
function on_update()
    -- Left stick movement, right stick look.
    translate(gamepad_left_x * 5 * delta, 0, gamepad_left_y * 5 * delta)
    rotate(0, -gamepad_right_x * 2 * delta, 0)

    if gamepad_south then print("jump") end       -- A / Cross
    if gamepad_right_trigger > 0.5 then print("fire") end
end
```

## Raw keyboard

For keys that aren't part of the movement vector, query them by name. Key names are **Bevy `KeyCode` debug strings** — `"KeyW"`, `"KeyE"`, `"Space"`, `"ShiftLeft"`, `"ArrowUp"`, `"Escape"`, and so on. Letter keys are `"KeyA"`…`"KeyZ"` (not `"A"`), and number-row digits are `"Digit0"`…`"Digit9"`.

```lua
function on_update()
    if is_key_pressed("ShiftLeft") then
        -- sprint while held
    end
    if is_key_just_pressed("Space") then
        apply_impulse(0, 8, 0)   -- jump on the frame Space goes down
    end
    if is_key_just_released("Escape") then
        unlock_cursor()
    end
end
```

| Function (Lua) | Description |
|---|---|
| `is_key_pressed(key)` | True while the key is held |
| `is_key_just_pressed(key)` | True only on the frame the key goes down |
| `is_key_just_released(key)` | True only on the frame the key goes up |

In Rhai the same three functions exist but take the live key table as the first argument, because Rhai has no implicit globals lookup:

```rhai
fn on_update() {
    if is_key_pressed(_keys_pressed, "KeyW") {
        translate(0.0, 0.0, 5.0 * delta);
    }
    if is_key_just_pressed(_keys_just_pressed, "Space") {
        apply_impulse(0.0, 8.0, 0.0);
    }
}
```

> At runtime `is_key_pressed` can match **any** physical key by its Bevy debug name. The action-map bindings below resolve a fixed set of names (letters, digits, `Space`, `Enter`, `Escape`, `Tab`, `Backspace`, `Shift`/`Control`/`Alt` `Left`/`Right`, the four arrows, and `F1`–`F12`).

## The action map

`renzora_input::InputPlugin` (installed as part of the engine foundation) maintains two resources: an `InputMap` of named actions and an `ActionState` recomputed each frame in `PreUpdate`. Scripts read the result by action name, so the same gameplay code works whether the player is on keyboard, mouse, or gamepad — and players can rebind without touching scripts.

These functions are **Lua-only**:

| Function | Returns | Description |
|---|---|---|
| `input_button_pressed(action)` | bool | True while any binding for the action is held |
| `input_button_just_pressed(action)` | bool | True on the frame the action fires |
| `input_button_just_released(action)` | bool | True on the frame the action releases |
| `input_axis_1d(action)` | number | Single axis value (-1..1) |
| `input_axis_2d(action)` | number, number | Two return values — `local x, y = input_axis_2d("move")` |

```lua
function on_update()
    -- Same code on keyboard (WASD) and gamepad (left stick).
    local mx, my = input_axis_2d("move")
    translate(mx * 5 * delta, 0, my * 5 * delta)

    if input_button_just_pressed("jump") then
        apply_impulse(0, 8, 0)
    end
    if input_button_pressed("sprint") then
        -- ...
    end
end
```

### Default actions

A fresh project ships with this action set:

| Action | Kind | Default bindings |
|---|---|---|
| `move` | Axis2D | WASD, arrow keys, left stick |
| `look` | Axis2D | Right stick (mouse look is handled separately via `mouse_delta_*`) |
| `jump` | Button | `Space`, gamepad South |
| `sprint` | Button | `ShiftLeft`, gamepad West |
| `interact` | Button | `KeyE`, gamepad East |
| `primary` | Button | Mouse Left, gamepad RightTrigger2 |
| `secondary` | Button | Mouse Right, gamepad LeftTrigger2 |

### input_map.ron

To customise actions, drop an `input_map.ron` in the project root. On startup `InputPlugin` looks for it in the rpak/VFS first, then on disk in the project directory, and falls back to the built-in defaults if neither is found. The editor's input settings write the same file back.

```ron
(
    actions: [
        (
            name: "move",
            kind: Axis2D,
            bindings: [
                Composite2D(up: "KeyW", down: "KeyS", left: "KeyA", right: "KeyD"),
                Composite2D(up: "ArrowUp", down: "ArrowDown", left: "ArrowLeft", right: "ArrowRight"),
                GamepadAxis("LeftStickX"),
            ],
            dead_zone: 0.15,
        ),
        (
            name: "jump",
            kind: Button,
            bindings: [
                Key("Space"),
                GamepadButton("South"),
            ],
            dead_zone: 0.0,
        ),
    ],
)
```

Each action has a `kind` of `Button`, `Axis1D`, or `Axis2D`, plus a list of bindings and a `dead_zone` for analog inputs. The `InputBinding` variants are:

| Variant | Example | Notes |
|---|---|---|
| `Key(name)` | `Key("Space")` | Bevy `KeyCode` debug string |
| `MouseButton(name)` | `MouseButton("Left")` | `"Left"`, `"Right"`, or `"Middle"` |
| `GamepadButton(name)` | `GamepadButton("South")` | `South`/`East`/`West`/`North`, `LeftTrigger`/`RightTrigger`, `LeftTrigger2`/`RightTrigger2`, `Select`/`Start`, `LeftThumb`/`RightThumb`, `DPadUp`/`Down`/`Left`/`Right` |
| `GamepadAxis(name)` | `GamepadAxis("LeftStickX")` | `LeftStickX`/`Y`, `RightStickX`/`Y`, `LeftZ`/`RightZ` |
| `Composite2D { up, down, left, right }` | `Composite2D(up: "KeyW", ...)` | Four keys combined into a 2D axis |

> The action map is its own subsystem — it does **not** feed the `input_x`/`input_y` globals (those are hard-wired to WASD/arrows). Use `input_axis_2d("move")` if you want movement to respect a rebound `move` action.

## Example: first-person controller (Lua)

```lua
function props()
    return {
        speed      = { value = 5.0, hint = "Walk speed (m/s)" },
        look_speed = { value = 0.2, hint = "Mouse sensitivity" },
        jump_force = { value = 8.0 },
    }
end

function on_ready()
    lock_cursor()
end

function on_update()
    -- Mouse look around Y (yaw).
    rotate(0, -mouse_delta_x * look_speed, 0)

    -- Rebindable movement (keyboard or left stick).
    local mx, my = input_axis_2d("move")
    translate(mx * speed * delta, 0, my * speed * delta)

    -- Sprint doubles speed.
    if input_button_pressed("sprint") then
        translate(mx * speed * delta, 0, my * speed * delta)
    end

    -- Jump only when grounded (subsystem read, mirrored by renzora_physics).
    if input_button_just_pressed("jump") and get("PhysicsReadState.grounded") then
        apply_impulse(0, jump_force, 0)
    end

    if is_key_just_pressed("Escape") then
        unlock_cursor()
    end
end
```

The Rhai equivalent must use the read-only globals (`input_x`/`input_y`, `gamepad_*`) and the map-passing `is_key_pressed(_keys_pressed, "...")` form, since the action-map and grounded-read functions are Lua-only.

## Related

- [Lua](/docs/r1-alpha5/scripting/lua) — the full-surface, native-only backend
- [Rhai](/docs/r1-alpha5/scripting/rhai) — the everywhere-including-web backend (input limitations)
- [Physics](/docs/r1-alpha5/scripting/physics) — forces, impulses, and `PhysicsReadState`
- [Scripting Overview](/docs/r1-alpha5/scripting/overview) — how scripts attach and dispatch by extension
