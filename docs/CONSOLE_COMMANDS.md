# Console Commands

The console panel includes a built-in command system for common editor operations. Commands are prefixed with `/` and executed directly — no Rhai syntax needed. Any input without a `/` prefix is evaluated as a Rhai expression as before.

## Quick Start

Open the Console panel and type:

```
/help            — list all commands
/wireframe       — toggle wireframe
/set camera.speed 20  — change a setting
/fps             — show current FPS
```

## Commands

| Command | Description |
|---------|-------------|
| `/clear` | Clear console output |
| `/help [command]` | List all commands, or show detailed help for a specific command |
| `/set <path> <value>` | Set a setting value (bool, float, int, string) |
| `/get <path>` | Query the current value of a setting |
| `/toggle <path>` | Toggle a boolean setting |
| `/list` | List all available setting paths |
| `/wireframe` | Toggle wireframe mode |
| `/grid` | Toggle grid visibility |
| `/shadows` | Toggle shadows |
| `/lighting` | Toggle lighting |
| `/snap <translate\|rotate\|scale> [value]` | Toggle snap or set snap value |
| `/fps` | Show current FPS and frame time |
| `/play` | Enter play mode |
| `/stop` | Stop play mode |
| `/settings` | Open/close settings panel |
| `/dev` | Toggle developer mode |

## Setting Paths

Use these dotted paths with `/set`, `/get`, `/toggle`, and `/list`.

### Grid & Viewport

| Path | Type | Description |
|------|------|-------------|
| `grid` | bool | Show grid |
| `subgrid` | bool | Show subgrid |
| `axis_gizmo` | bool | Show axis gizmo |
| `grid.size` | float | Grid size |
| `grid.divisions` | uint | Grid divisions |

### Rendering

| Path | Type | Description |
|------|------|-------------|
| `render.textures` | bool | Show textures |
| `render.wireframe` | bool | Wireframe overlay |
| `render.lighting` | bool | Enable lighting |
| `render.shadows` | bool | Enable shadows |

### Selection & Collision

| Path | Type | Description |
|------|------|-------------|
| `selection.highlight` | `outline` / `gizmo` | Selection highlight mode |
| `selection.on_top` | bool | Selection boundary rendered on top |
| `collision.gizmos` | `selected` / `always` | Collision gizmo visibility |

### General

| Path | Type | Description |
|------|------|-------------|
| `dev_mode` | bool | Developer mode |
| `font_size` | float | UI font size |

### Camera

| Path | Type | Description |
|------|------|-------------|
| `camera.speed` | float | Camera move speed |
| `camera.look_sensitivity` | float | Look/rotation sensitivity |
| `camera.orbit_sensitivity` | float | Orbit sensitivity |
| `camera.pan_sensitivity` | float | Pan sensitivity |
| `camera.zoom_sensitivity` | float | Zoom (scroll wheel) sensitivity |
| `camera.invert_y` | bool | Invert Y axis |
| `camera.left_click_pan` | bool | Left-click drag camera pan |

### Scripts

| Path | Type | Description |
|------|------|-------------|
| `scripts.rerun_on_ready` | bool | Rerun `on_ready` when scripts are reloaded |
| `scripts.game_camera` | bool | Use game camera in play mode |
| `scripts.hide_cursor` | bool | Hide cursor in play mode |

### Snap

| Path | Type | Description |
|------|------|-------------|
| `snap.translate` | bool | Position snap enabled |
| `snap.translate.value` | float | Position snap increment (units) |
| `snap.rotate` | bool | Rotation snap enabled |
| `snap.rotate.value` | float | Rotation snap increment (degrees) |
| `snap.scale` | bool | Scale snap enabled |
| `snap.scale.value` | float | Scale snap increment |
| `snap.object` | bool | Snap to nearby objects |
| `snap.floor` | bool | Snap to floor |

## Examples

### Toggling settings quickly

```
/wireframe           — toggle wireframe on/off
/toggle grid         — toggle grid on/off
/toggle camera.invert_y
```

### Querying and changing values

```
/get camera.speed         → camera.speed = 10
/set camera.speed 25      → camera.speed = 25
/set grid.divisions 20    → grid.divisions = 20
```

### Snap configuration

```
/snap translate           — toggle position snap on/off
/snap translate 0.5       — enable position snap with 0.5 unit increment
/snap rotate 45           — enable rotation snap at 45° increments
```

### Play mode

```
/play                — enter play mode
/stop                — stop play mode
```

### Combining with Rhai

The console still supports Rhai expressions for anything not prefixed with `/`:

```
2 + 2                → 4
spawn("Cube")        → spawns a cube entity
```
