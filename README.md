# Renzora Engine

A game engine built on [Bevy](https://bevyengine.org/) with a visual editor for creating 3D games.

## Prerequisites

1. **Install Rust** from [rustup.rs](https://rustup.rs/) (this gives you `rustup`, `cargo`, and `rustc`)
2. Windows 10/11, Linux, or macOS

### Optional (Faster Linking)

For faster compile times, add the LLD linker:

**Windows:**
```bash
rustup component add llvm-tools-preview
```

**Linux:**
```bash
sudo apt install lld clang  # Ubuntu/Debian
```

Then add `.cargo/config.toml` to the project (see Configuration section).

## Building

### Editor (Development)

The editor uses dynamic linking for fast iteration:

```bash
cargo run --features editor
```

### Runtime (Release)

The runtime must be statically linked for distribution. Use a separate target directory to avoid cargo reusing the editor's dynamic-linked Bevy:

**Windows (PowerShell):**
```powershell
$env:CARGO_TARGET_DIR="target-runtime"; cargo build --release --features runtime --bin renzora_runtime
cp target-runtime/release/renzora_runtime.exe runtimes/windows/
```

**Windows (Command Prompt):**
```cmd
set CARGO_TARGET_DIR=target-runtime && cargo build --release --features runtime --bin renzora_runtime
copy target-runtime\release\renzora_runtime.exe runtimes\windows\
```

**Linux/macOS:**
```bash
CARGO_TARGET_DIR=target-runtime cargo build --release --features runtime --bin renzora_runtime
cp target-runtime/release/renzora_runtime runtimes/linux/  # or runtimes/macos/
```

**Why a separate target directory?** The editor uses `bevy/dynamic_linking` for fast builds. Cargo caches this and would reuse it for the runtime, resulting in a tiny (~1.5MB) binary that crashes. The separate directory forces a clean static build (~50MB).

## Project Structure

```
renzora/
├── src/
│   ├── main.rs              # Editor entry point
│   ├── runtime/
│   │   └── main.rs          # Runtime entry point
│   ├── export/              # Game export/packaging
│   ├── core/                # Core editor resources
│   ├── ui/                  # Editor UI panels
│   └── shared/              # Shared code (editor + runtime)
├── runtimes/
│   └── windows/
│       └── renzora_runtime.exe  # Pre-built runtime for exports
├── assets/                  # Editor assets
└── Cargo.toml
```

## Features

| Feature | Description |
|---------|-------------|
| `editor` | Full editor with UI, asset browser, scene editing (default) |
| `runtime` | Minimal runtime for exported games |

## Scripting API

Renzora supports scripting via **Rhai** (a lightweight embedded scripting language) and **Blueprints** (visual node-based scripting).

### Rhai Scripts

Scripts are `.rhai` files stored in your project's `scripts/` folder. Attach them to entities via the Script component in the inspector.

#### Lifecycle Functions

```rhai
fn on_ready() {
    // Called once when entity spawns
    print_log("Hello from script!")
}

fn on_update() {
    // Called every frame
    translate(input_x * 5.0 * delta, 0.0, -input_y * 5.0 * delta)
}
```

#### Script Properties

Expose configurable variables in the inspector:

```rhai
fn props() {
    #{
        speed: { default: 5.0, hint: "Movement speed" },
        jump_height: { default: 2.0, hint: "Jump force" }
    }
}

fn on_update() {
    let spd = vars["speed"];
    translate(input_x * spd * delta, 0.0, 0.0)
}
```

#### Available Variables

| Variable | Type | Description |
|----------|------|-------------|
| `delta` | Float | Frame delta time in seconds |
| `elapsed` | Float | Total elapsed time since start |
| `position_x/y/z` | Float | Current position |
| `rotation_x/y/z` | Float | Current rotation (degrees) |
| `scale_x/y/z` | Float | Current scale |
| `input_x/y` | Float | WASD/arrow key input (-1 to 1) |
| `mouse_x/y` | Float | Mouse position |
| `mouse_delta_x/y` | Float | Mouse movement delta |
| `vars` | Map | Script properties from inspector |
| `children` | Array | Child entity names |
| `has_parent` | Bool | Whether entity has a parent |

**Gamepad Variables:**
| Variable | Description |
|----------|-------------|
| `gamepad_left_x/y` | Left stick axes (-1 to 1) |
| `gamepad_right_x/y` | Right stick axes (-1 to 1) |
| `gamepad_left_trigger` | Left trigger (0 to 1) |
| `gamepad_right_trigger` | Right trigger (0 to 1) |
| `gamepad_a/b/x/y` | Face buttons (bool) |
| `gamepad_lb/rb` | Shoulder buttons (bool) |
| `gamepad_dpad_up/down/left/right` | D-pad (bool) |

#### Transform Functions

```rhai
// Position
set_position(x, y, z)           // Set absolute position
translate(x, y, z)              // Move by offset

// Rotation (degrees)
set_rotation(pitch, yaw, roll)  // Set absolute rotation

// Child access (Godot-style $ChildName syntax also works)
set_child_position("ChildName", x, y, z)
set_child_rotation("ChildName", pitch, yaw, roll)
child_translate("ChildName", x, y, z)

// Parent access
parent_set_position(x, y, z)
parent_set_rotation(pitch, yaw, roll)
parent_translate(x, y, z)
```

#### Math Functions

```rhai
lerp(a, b, t)                   // Linear interpolation
clamp(value, min, max)          // Clamp value to range
sin(radians), cos(radians)      // Trigonometric functions
sqrt(x), abs(x), pow(base, exp) // Math utilities
floor(x), ceil(x), round(x)     // Rounding
min(a, b), max(a, b)            // Min/max
deg_to_rad(degrees)             // Convert degrees to radians
rad_to_deg(radians)             // Convert radians to degrees
```

#### Environment Functions

```rhai
// Lighting
set_sun_angles(azimuth, elevation)  // Sun position in degrees
set_ambient_brightness(value)       // Ambient light intensity
set_ambient_color(r, g, b)          // Ambient color (0-1)
set_exposure(value)                 // Camera exposure

// Sky
set_sky_top_color(r, g, b)          // Procedural sky top
set_sky_horizon_color(r, g, b)      // Procedural sky horizon

// Fog
set_fog(enabled, start, end)        // Enable/configure fog
set_fog_color(r, g, b)              // Fog color
```

#### Example: FPS Controller

```rhai
fn props() {
    #{
        speed: { default: 5.0, hint: "Move speed" },
        sensitivity: { default: 0.2, hint: "Mouse sensitivity" }
    }
}

fn on_update() {
    let spd = vars["speed"];
    let sens = vars["sensitivity"];

    // WASD movement
    translate(input_x * spd * delta, 0.0, -input_y * spd * delta)

    // Mouse look
    set_rotation(
        clamp(rotation_x - mouse_delta_y * sens, -89.0, 89.0),
        rotation_y - mouse_delta_x * sens,
        0.0
    )
}
```

---

## Blueprint Visual Scripting

Blueprints are node-based visual scripts that compile to Rhai code. They provide a beginner-friendly way to create game logic without writing code.

### Getting Started

1. Select an entity in the Hierarchy
2. Open the Blueprint panel
3. Right-click to add nodes
4. Connect nodes by dragging from output pins to input pins
5. Start with an **Event** node (On Ready or On Update) as your entry point

### Pin Types

Pins are the connection points on nodes. Different colors indicate different data types:

| Color | Type | Description |
|-------|------|-------------|
| White | **Flow** | Execution flow - determines order of operations |
| Green | **Float** | Decimal number (e.g., 1.0, -3.5, 0.001) |
| Red | **Bool** | True or false |
| Pink | **String** | Text (e.g., "Hello World") |
| Yellow | **Vec2** | 2D vector (X, Y) |
| Orange | **Vec3** | 3D vector (X, Y, Z) |
| Cyan | **Any** | Accepts any type |

### Node Categories

---

### Events (Red)

Entry points for blueprint execution. Every blueprint needs at least one event node.

#### On Ready
Called once when the entity spawns or scene loads.

| Pin | Direction | Type | Description |
|-----|-----------|------|-------------|
| exec | Output | Flow | Execution continues here |

**Use for:** Initialization, setting starting values, one-time setup.

#### On Update
Called every frame during gameplay.

| Pin | Direction | Type | Description |
|-----|-----------|------|-------------|
| exec | Output | Flow | Execution continues here |
| delta | Output | Float | Time since last frame (seconds) |

**Use for:** Movement, input handling, continuous updates.

---

### Math (Green)

Mathematical operations for calculations.

#### Add
Adds two values together (A + B).

| Pin | Direction | Type | Default |
|-----|-----------|------|---------|
| A | Input | Float | 0.0 |
| B | Input | Float | 0.0 |
| Result | Output | Float | — |

#### Subtract
Subtracts B from A (A - B).

| Pin | Direction | Type | Default |
|-----|-----------|------|---------|
| A | Input | Float | 0.0 |
| B | Input | Float | 0.0 |
| Result | Output | Float | — |

#### Multiply
Multiplies two values (A * B).

| Pin | Direction | Type | Default |
|-----|-----------|------|---------|
| A | Input | Float | 1.0 |
| B | Input | Float | 1.0 |
| Result | Output | Float | — |

#### Divide
Divides A by B (A / B).

| Pin | Direction | Type | Default |
|-----|-----------|------|---------|
| A | Input | Float | 1.0 |
| B | Input | Float | 1.0 |
| Result | Output | Float | — |

#### Lerp
Linear interpolation between A and B by T.
- T=0 returns A
- T=1 returns B
- T=0.5 returns midpoint

| Pin | Direction | Type | Default |
|-----|-----------|------|---------|
| A | Input | Float | 0.0 |
| B | Input | Float | 1.0 |
| T | Input | Float | 0.5 |
| Result | Output | Float | — |

#### Clamp
Restricts a value to stay within min/max bounds.

| Pin | Direction | Type | Default |
|-----|-----------|------|---------|
| Value | Input | Float | 0.0 |
| Min | Input | Float | 0.0 |
| Max | Input | Float | 1.0 |
| Result | Output | Float | — |

#### Abs
Returns the absolute (positive) value.

| Pin | Direction | Type | Default |
|-----|-----------|------|---------|
| Value | Input | Float | 0.0 |
| Result | Output | Float | — |

#### Min
Returns the smaller of two values.

| Pin | Direction | Type | Default |
|-----|-----------|------|---------|
| A | Input | Float | 0.0 |
| B | Input | Float | 0.0 |
| Result | Output | Float | — |

#### Max
Returns the larger of two values.

| Pin | Direction | Type | Default |
|-----|-----------|------|---------|
| A | Input | Float | 0.0 |
| B | Input | Float | 0.0 |
| Result | Output | Float | — |

#### Sin
Sine of the input angle (in radians).

| Pin | Direction | Type | Default |
|-----|-----------|------|---------|
| Radians | Input | Float | 0.0 |
| Result | Output | Float | — |

#### Cos
Cosine of the input angle (in radians).

| Pin | Direction | Type | Default |
|-----|-----------|------|---------|
| Radians | Input | Float | 0.0 |
| Result | Output | Float | — |

---

### Logic (Blue)

Control flow and boolean operations.

#### Branch
Executes different paths based on a condition (if/else).

| Pin | Direction | Type | Default |
|-----|-----------|------|---------|
| exec | Input | Flow | — |
| Condition | Input | Bool | false |
| True | Output | Flow | — |
| False | Output | Flow | — |

**Example:** Check if player health > 0, then continue game, else show game over.

#### Compare
Compares two values using a comparison operator.

| Pin | Direction | Type | Default |
|-----|-----------|------|---------|
| A | Input | Float | 0.0 |
| B | Input | Float | 0.0 |
| Result | Output | Bool | — |

**Operators:** `==` (equal), `!=` (not equal), `>` (greater), `<` (less), `>=` (greater or equal), `<=` (less or equal)

Set the operator in the node's properties panel.

#### And
Logical AND - true only if both inputs are true.

| Pin | Direction | Type | Default |
|-----|-----------|------|---------|
| A | Input | Bool | false |
| B | Input | Bool | false |
| Result | Output | Bool | — |

#### Or
Logical OR - true if either input is true.

| Pin | Direction | Type | Default |
|-----|-----------|------|---------|
| A | Input | Bool | false |
| B | Input | Bool | false |
| Result | Output | Bool | — |

#### Not
Logical NOT - inverts the boolean value.

| Pin | Direction | Type | Default |
|-----|-----------|------|---------|
| Value | Input | Bool | false |
| Result | Output | Bool | — |

---

### Transform (Orange)

Manipulate entity position, rotation, and scale.

#### Get Position
Gets the entity's current world position.

| Pin | Direction | Type |
|-----|-----------|------|
| Position | Output | Vec3 |
| X | Output | Float |
| Y | Output | Float |
| Z | Output | Float |

#### Set Position
Sets the entity's world position to exact coordinates.

| Pin | Direction | Type | Default |
|-----|-----------|------|---------|
| exec | Input | Flow | — |
| X | Input | Float | 0.0 |
| Y | Input | Float | 0.0 |
| Z | Input | Float | 0.0 |
| exec | Output | Flow | — |

#### Translate
Moves the entity by an offset (relative movement).

| Pin | Direction | Type | Default |
|-----|-----------|------|---------|
| exec | Input | Flow | — |
| X | Input | Float | 0.0 |
| Y | Input | Float | 0.0 |
| Z | Input | Float | 0.0 |
| exec | Output | Flow | — |

**Tip:** Multiply by delta time for frame-rate independent movement.

#### Get Rotation
Gets the entity's rotation as euler angles in degrees.

| Pin | Direction | Type |
|-----|-----------|------|
| Rotation | Output | Vec3 |
| Pitch | Output | Float |
| Yaw | Output | Float |
| Roll | Output | Float |

#### Set Rotation
Sets the entity's rotation in degrees (euler angles).

| Pin | Direction | Type | Default |
|-----|-----------|------|---------|
| exec | Input | Flow | — |
| Pitch | Input | Float | 0.0 |
| Yaw | Input | Float | 0.0 |
| Roll | Input | Float | 0.0 |
| exec | Output | Flow | — |

#### Rotate
Rotates the entity by an offset in degrees.

| Pin | Direction | Type | Default |
|-----|-----------|------|---------|
| exec | Input | Flow | — |
| Pitch | Input | Float | 0.0 |
| Yaw | Input | Float | 0.0 |
| Roll | Input | Float | 0.0 |
| exec | Output | Flow | — |

---

### Input (Yellow)

Read player input from keyboard, mouse, and gamepad.

#### Get Input Axis
Gets WASD/Arrow key input as -1 to 1 values.

| Pin | Direction | Type | Description |
|-----|-----------|------|-------------|
| X | Output | Float | A/D or Left/Right (-1 to 1) |
| Y | Output | Float | W/S or Up/Down (-1 to 1) |

**Usage:** Connect to Translate node for basic movement.

#### Is Key Pressed
Checks if a specific key is currently held down.

| Pin | Direction | Type |
|-----|-----------|------|
| Pressed | Output | Bool |

**Properties:** Set the key name in the node properties (e.g., "Space", "Shift", "E").

**Key Names:** Space, Shift, Control, Alt, Enter, Escape, Tab, A-Z, 0-9, F1-F12, Up, Down, Left, Right

#### Get Mouse Position
Gets the mouse cursor position in screen coordinates.

| Pin | Direction | Type |
|-----|-----------|------|
| Position | Output | Vec2 |
| X | Output | Float |
| Y | Output | Float |

#### Get Mouse Delta
Gets how much the mouse moved since last frame.

| Pin | Direction | Type |
|-----|-----------|------|
| Delta | Output | Vec2 |
| X | Output | Float |
| Y | Output | Float |

**Usage:** Connect to rotation for mouse-look camera control.

---

### Time (Light Blue)

Time-related values for animation and timing.

#### Get Delta Time
Gets the time since last frame in seconds.

| Pin | Direction | Type |
|-----|-----------|------|
| Delta | Output | Float |

**Usage:** Multiply movement/rotation by delta to make it frame-rate independent.

#### Get Elapsed Time
Gets the total time since the game started.

| Pin | Direction | Type |
|-----|-----------|------|
| Elapsed | Output | Float |

**Usage:** Use with Sin/Cos for oscillating animations, or for timed events.

---

### Variables (Cyan)

Read and write graph variables for storing state.

#### Get Variable
Reads the value of a named variable.

| Pin | Direction | Type |
|-----|-----------|------|
| Value | Output | Any |

**Properties:** Set the variable name in node properties.

#### Set Variable
Writes a value to a named variable.

| Pin | Direction | Type |
|-----|-----------|------|
| exec | Input | Flow |
| Value | Input | Any |
| exec | Output | Flow |

**Properties:** Set the variable name in node properties.

---

### Utility (Gray)

General purpose nodes for debugging and flow control.

#### Print
Outputs a message to the console for debugging.

| Pin | Direction | Type | Default |
|-----|-----------|------|---------|
| exec | Input | Flow | — |
| Message | Input | String | "Hello" |
| exec | Output | Flow | — |

#### Sequence
Executes multiple flow paths in order (0, 1, 2, 3).

| Pin | Direction | Type |
|-----|-----------|------|
| exec | Input | Flow |
| Then 0 | Output | Flow |
| Then 1 | Output | Flow |
| Then 2 | Output | Flow |
| Then 3 | Output | Flow |

**Usage:** When you need to do multiple things from one event.

#### Comment
A non-functional note for documenting your blueprint. Does not execute any code.

---

### Blueprint Examples

#### Example 1: Basic Movement
Move entity with WASD keys:

```
[On Update] ──exec──> [Translate]
     │                    ↑
     └──delta────────────(multiply with input)

[Get Input Axis] ──X──> [Multiply] ──> Translate.X
                        ↑
[Get Delta Time] ──────(5.0 speed)

[Get Input Axis] ──Y──> [Multiply] ──> Translate.Z (negated)
```

#### Example 2: Rotating Object
Continuously rotate an object:

```
[On Update] ──exec──> [Rotate]
     │                  ↑
     └──delta──> [Multiply] ──> Rotate.Yaw
                    ↑
                  (45.0 degrees per second)
```

#### Example 3: Conditional Logic
Only move when Space is held:

```
[On Update] ──exec──> [Branch] ──True──> [Translate]
                         ↑
              [Is Key Pressed: Space]
```

#### Example 4: Oscillating Motion (Bob)
Make an object bob up and down:

```
[On Update] ──exec──> [Set Position]
                          ↑ Y
[Get Elapsed Time] ──> [Multiply] ──> [Sin] ──> [Multiply]
                          ↑                         ↑
                        (2.0 speed)            (0.5 amplitude)
```

---

### Built-in Scripts

Ready-to-use scripts available in the Add Script menu:

| Script | Description | Variables |
|--------|-------------|-----------|
| **Rotate** | Continuous rotation | speed (45.0), axis (0,1,0) |
| **Simple Movement** | WASD movement | speed (5.0), use_local_space (false) |
| **Mouse Look** | FPS camera control | sensitivity (0.2), invert_y (false) |
| **Follow Target** | Smooth follow | target (vec3), smoothing (5.0), offset (vec3) |
| **Bob** | Oscillating motion | amplitude (0.5), speed (2.0), axis (0,1,0) |
| **Debug Info** | Print entity state | print_position (true), interval (1.0) |

## Export System

Renzora packages games into a single executable:

- **Pack Format (RPCK v2):** Custom binary format with file table and zstd compression
- **Asset Discovery:** Automatically finds all assets referenced by your scenes
- **Compression:** zstd level 3, skips already-compressed formats (PNG, JPG, MP3, GLB)
- **Single File:** Runtime + assets appended into one executable

### Pack Format Structure

```
HEADER (28 bytes):
  Magic: "RPCK" (4 bytes)
  Version: u32 (4 bytes)
  Header Size: u32 (4 bytes)
  Flags: u32 (4 bytes)
  File Count: u32 (4 bytes)
  Data Offset: u64 (8 bytes)

FILE TABLE (per file):
  Path Length: u32
  Path: UTF-8 string
  Offset: u64
  Size: u64 (original)
  Compressed Size: u64
  Flags: u32 (bit 0 = compressed)

DATA SECTION:
  Compressed/raw file contents

FOOTER (12 bytes):
  Pack Start Offset: u64
  Magic: "RPCK" (4 bytes)
```

## Configuration

### Cargo.toml Profiles

The project uses Bevy's recommended optimization settings:

```toml
[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 3

[profile.release]
lto = "thin"
```

### Faster Linking (Optional)

Create `.cargo/config.toml`:

```toml
# Windows
[target.x86_64-pc-windows-msvc]
linker = "rust-lld.exe"

# Linux
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=lld"]
```

## Troubleshooting

### Runtime crashes immediately

Run from terminal to see error messages:
```bash
cd export_folder
./YourGame.exe
```

The runtime will show crash details and wait for Enter before closing.

### Small runtime binary (~1.5MB)

This means Bevy was compiled with dynamic linking. Use a separate target directory:
```powershell
$env:CARGO_TARGET_DIR="target-runtime"; cargo build --release --features runtime --bin renzora_runtime
```

The correct size should be ~50MB (statically linked Bevy).

### Export shows "Runtime not found"

Ensure the runtime binary exists at:
```
runtimes/windows/renzora_runtime.exe
```

Build it with the commands in the "Runtime (Release)" section above.

## License

Apache License (see LICENSE.md)
Version 2.0, January 2004
http://www.apache.org/licenses/