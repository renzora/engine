# Engine User Guide

A comprehensive guide to using the engine editor, covering all major features from viewport navigation to terrain sculpting.

---

## Table of Contents

1. [Getting Started](#getting-started)
2. [Editor Layout](#editor-layout)
3. [Viewport & Camera Controls](#viewport--camera-controls)
4. [Selection](#selection)
5. [Transform Gizmos](#transform-gizmos)
6. [Modal Transforms (Blender-style)](#modal-transforms-blender-style)
7. [Creating Objects](#creating-objects)
8. [Components](#components)
9. [Inspector Panel](#inspector-panel)
10. [Terrain Editing](#terrain-editing)
11. [Scene Management](#scene-management)
12. [Play Mode](#play-mode)
13. [Scripting](#scripting)
14. [Materials & Shaders](#materials--shaders)
15. [Lighting](#lighting)
16. [Physics](#physics)
17. [Post-Processing Effects](#post-processing-effects)
18. [Asset Browser](#asset-browser)
19. [Debug & Profiling Panels](#debug--profiling-panels)
20. [Keyboard Shortcuts Reference](#keyboard-shortcuts-reference)
21. [Tips & Tricks](#tips--tricks)

---

## Getting Started

The engine is a Bevy-based 3D/2D game editor with an integrated runtime. It follows conventions familiar to users of Blender and Unreal Engine, with Blender-style modal transforms (G/R/S) and Unreal-style camera fly controls.

---

## Editor Layout

The editor window is split into several panels:

| Panel | Location | Purpose |
|-------|----------|---------|
| **Title Bar / Menu Bar** | Top | File, Edit menus, play controls, window title |
| **Toolbar** | Below title bar | Tool selection, gizmo modes, snap toggles |
| **Hierarchy** | Left | Entity tree with parent-child relationships |
| **Viewport** | Center | 3D/2D scene rendering and interaction |
| **Inspector** | Right | Component properties for selected entity |
| **Bottom Panel** | Bottom | Tabs for Assets, Console, Animation, and more |

The bottom panel can be minimized with `Ctrl+Backtick` and resized by dragging its top edge.

### Viewport Modes

The viewport automatically switches between 3D and 2D modes based on the selected entity:
- **3D Mode**: For meshes, cameras, lights, physics bodies, terrain, particles
- **2D Mode**: For sprites, Camera2D, UI components (panels, labels, buttons, images)

---

## Viewport & Camera Controls

The camera uses an **orbit model** with a focus point. All movement is relative to this focus.

### Mouse Controls

| Input | Action |
|-------|--------|
| **Right Mouse + Drag** | Look around (fly mode) |
| **Right Mouse + WASD** | Fly through scene |
| **Alt + Left Mouse Drag** | Orbit around focus point |
| **Middle Mouse Drag** | Orbit around focus point |
| **Left Mouse Drag** | Forward/backward dolly + horizontal yaw |
| **Scroll Wheel** | Zoom in/out |

### Fly Mode (Hold Right Mouse Button)

| Key | Action |
|-----|--------|
| `W` | Move forward |
| `S` | Move backward |
| `A` | Strafe left |
| `D` | Strafe right |
| `Q` | Move down |
| `E` | Move up |
| `Shift` (hold) | Double movement speed |
| `Ctrl` (hold) | Quarter movement speed (slow mode) |

Movement speed scales with distance from the focus point when "distance relative speed" is enabled in settings.

### View Angles (Numpad)

| Key | View |
|-----|------|
| `Numpad 1` | Front |
| `Ctrl + Numpad 1` | Back |
| `Numpad 3` | Right |
| `Ctrl + Numpad 3` | Left |
| `Numpad 7` | Top |
| `Ctrl + Numpad 7` | Bottom |
| `Numpad 5` | Toggle Perspective / Orthographic |

### Focus

Press `F` to focus the camera on the currently selected entity (sets orbit distance to 5 units).

### Camera Settings

Accessible via `Ctrl + ,` (Settings):
- `move_speed`: WASD speed (default 10.0)
- `look_sensitivity`: Mouse look speed (default 0.3)
- `orbit_sensitivity`: Orbit speed (default 0.5)
- `pan_sensitivity`: Pan speed (default 1.0)
- `zoom_sensitivity`: Scroll zoom speed (default 1.0)
- `invert_y`: Invert vertical mouse axis
- `distance_relative_speed`: Scale movement by distance from focus
- `left_click_pan`: Enable/disable left-click drag

---

## Selection

### Basic Selection

| Input | Action |
|-------|--------|
| **Left Click** on entity | Select single entity |
| **Ctrl + Click** | Toggle entity in/out of selection |
| **Shift + Click** | Add to selection (or range select in hierarchy) |
| **Click empty space** | Deselect all |
| `Escape` | Deselect all |

### Box Selection

In **Select** tool mode (`Q`), click and drag on empty space to box-select multiple entities. Entities whose screen-space positions fall within the box are selected.

### Selection Feedback

- **Primary selection**: Orange outline (width 3.0)
- **Multi-selection**: Lighter orange outline (0.8 alpha)
- **Terrain**: Yellow border instead of outline
- **Cameras**: Custom camera gizmo (no outline)
- **Particles**: Wireframe diamond shape

### Locked Entities

Entities can be locked via the hierarchy panel (lock icon). Locked entities cannot be selected or interacted with in the viewport.

---

## Transform Gizmos

### Tool Modes

| Key | Tool | Description |
|-----|------|-------------|
| `Q` | Select | No gizmo, selection only |
| `W` | Translate | Move arrows (axes) and planes |
| `E` | Rotate | Rotation circles per axis |
| `R` | Scale | Scale handles per axis |

### Gizmo Interaction

**Translate Mode:**
- Drag a colored **arrow** to move along that axis (red=X, green=Y, blue=Z)
- Drag a colored **plane square** to move on that plane (XY, XZ, YZ)
- Drag the **center cube** to move freely on the camera plane

**Rotate Mode:**
- Drag a colored **circle** to rotate around that axis

**Scale Mode:**
- Drag axis **endpoints** to scale along that axis
- Drag center for uniform scale

### Gizmo Colors

| Axis | Color |
|------|-------|
| X | Red |
| Y | Green |
| Z | Blue |
| Hovered/Active | Yellow highlight |

### Snap Settings

| Feature | Activation | Default |
|---------|-----------|---------|
| Grid snap (translate) | Hold `Ctrl` while dragging | 1.0 unit |
| Rotation snap | Hold `Ctrl` while rotating | 15 degrees |
| Scale snap | Hold `Ctrl` while scaling | 0.25 |
| Object snap | Automatic (if enabled) | 0.5 unit radius |
| Floor snap | Automatic (if enabled) | Y = 0.0 |

Snap settings are configurable in the toolbar or settings panel.

---

## Modal Transforms (Blender-style)

The engine supports Blender's G/R/S modal transform workflow for precise, keyboard-driven manipulation.

### Activation

| Key | Mode |
|-----|------|
| `G` | **Grab** (translate) |
| `R` | **Rotate** |
| `S` | **Scale** |

Once activated, move the mouse to apply the transform in real-time. The cursor wraps at viewport edges for infinite mouse movement.

### Axis Constraints

While a modal transform is active:

| Key | Constraint |
|-----|-----------|
| `X` | Lock to X axis |
| `Y` | Lock to Y axis |
| `Z` | Lock to Z axis |
| `Shift + X` | Lock to YZ plane (everything except X) |
| `Shift + Y` | Lock to XZ plane (everything except Y) |
| `Shift + Z` | Lock to XY plane (everything except Z) |

Press the same axis key again to remove the constraint.

### Numeric Input

While in a modal transform, type numbers for precise values:

| Key | Action |
|-----|--------|
| `0-9` | Enter digits |
| `.` | Decimal point |
| `-` | Toggle negative |
| `Backspace` | Delete last character |

Example workflow: Press `G`, then `Z`, then type `2.5`, then `Enter` to move the entity exactly 2.5 units along the Z axis.

### Confirm / Cancel

| Input | Action |
|-------|--------|
| `Enter` or `Left Click` | Confirm transform |
| `Escape` or `Right Click` | Cancel (revert to original) |

---

## Creating Objects

### From the Create Menu

Use the menu bar or right-click in the hierarchy to create entities from presets:

**3D Objects:**
Cube, Sphere, Cylinder, Plane, Cone, Torus, Capsule, Wedge, Stairs, Arch, Half Cylinder, Quarter Pipe, Corner, Prism, Pyramid, Pipe, Ring, Wall, Ramp, Hemisphere, Curved Wall, Doorway, Window Wall, L-Shape, T-Shape, Cross, Funnel, Gutter, Spiral Stairs, Pillar

**Lights:** Point Light, Directional Light, Spot Light

**Cameras:** Camera 3D, Camera 2D, Camera Rig

**Physics:** Rigid Body (with collider)

**2D:** Sprite

**UI:** Panel, Label, Button, Image

**Environment:** World Environment, Skybox

### Duplicating

| Shortcut | Action |
|----------|--------|
| `Ctrl + D` | Duplicate selected entity |
| `Alt + D` | Duplicate and immediately enter grab mode |

Duplicates are offset by (0.5, 0, 0.5) units for visibility and receive "(Copy)" name suffixes.

### Deleting

Press `Delete` to delete the selected entity and all its children.

---

## Components

### Adding Components

1. Select an entity in the hierarchy or viewport
2. In the Inspector panel, click **"Add Component"**
3. Browse categories or use the search bar to filter
4. Click a component to add it

Components that conflict with already-present components are grayed out. Components with unmet requirements cannot be added until dependencies are present.

### Built-in Component Categories

| Category | Components |
|----------|-----------|
| **Rendering** | MeshRenderer, Sprite2D, MeshletMesh, Material |
| **Lighting** | PointLight, DirectionalLight, SpotLight, AmbientLight, SunDisc |
| **Camera** | Camera3D, Camera2D, CameraRig |
| **Physics** | RigidBody, CollisionShape (Box, Sphere, Capsule, Cylinder, Mesh) |
| **Gameplay** | Health, NavigationAgent |
| **Scripting** | Script |
| **Audio** | AudioListener |
| **UI** | UIPanel, UILabel, UIButton, UIImage |
| **Effects** | HanabiEffect (GPU particles) |
| **Environment** | WorldEnvironment, Skybox, Clouds, Terrain |
| **Post-Processing** | Fog, Bloom, DepthOfField, MotionBlur, Tonemapping, AmbientOcclusion, AntiAliasing, Reflections, Vignette, FilmGrain, GodRays, and 15+ more |

### Removing Components

Right-click a component header in the inspector, or use the remove button, to remove it from the entity.

### Component Conflicts & Requirements

Some components are mutually exclusive (e.g., Camera3D and Camera2D). The registry enforces these rules:
- **Conflicts**: Cannot have both components on the same entity
- **Requirements**: Must add dependency components first

---

## Inspector Panel

The Inspector panel shows all components on the selected entity and provides editing widgets.

### Property Types

| Type | Widget |
|------|--------|
| Float / Int | Drag slider |
| Bool | Checkbox |
| String | Text input |
| Vec2 / Vec3 | Per-component drag sliders (X, Y, Z) |
| Color | Color picker with RGBA channels |
| Enum | Dropdown selector |
| Entity reference | Entity picker |
| File path | Browse button + drop zone |

### Entity Properties

At the top of the inspector:
- **Name**: Editable text field
- **Tag**: Comma-separated tags for grouping and script lookup
- **Visible**: Toggle entity visibility
- **Locked**: Toggle entity interaction lock

---

## Terrain Editing

### Creating Terrain

1. Create a terrain entity from the Environment presets or add the `Terrain` component
2. Configure terrain grid: chunks_x, chunks_z (default 4x4), chunk_size (64 units), resolution (65 vertices per edge)
3. Set height range: min_height (-10) to max_height (100)

### Terrain Tool

Press `T` (or select Terrain Sculpt from the toolbar) to activate the terrain brush.

### Brush Types (17 total)

| Brush | Description |
|-------|-------------|
| **Raise** | Raise terrain height |
| **Lower** | Lower terrain height |
| **Sculpt** | Raise by default, lower with Shift held |
| **Smooth** | Blend heights with neighbors |
| **Flatten** | Flatten to a target height |
| **Set Height** | Set exact height value |
| **Erase** | Reset to default height |
| **Ramp** | Create smooth ramp between two points |
| **Erosion** | Simulate erosion effects |
| **Hydro** | Hydraulic erosion simulation |
| **Noise** | Apply procedural noise |
| **Retop** | Retopologize terrain mesh |

### Brush Settings

| Setting | Description | Range |
|---------|-------------|-------|
| **Radius** | Brush size in world units | 1.0 - 200.0 |
| **Strength** | Effect intensity | 0.0 - 1.0 |
| **Target Height** | For flatten/set height | 0.0 - 1.0 (normalized) |
| **Falloff** | Edge softness | 0.0 - 1.0 |

### Brush Shapes

- **Circle** (default)
- **Square**
- **Diamond**

### Falloff Types

- **Smooth** (cosine, default)
- **Linear**
- **Spherical**
- **Tip** (cubic, sharp center)
- **Flat** (no falloff)

### Brush Size Control

Scroll the **mouse wheel** while the terrain tool is active to resize the brush:
- Scroll up: Increase radius (x1.1)
- Scroll down: Decrease radius (x0.9)

### Sculpting Workflow

1. Press `T` to enter terrain sculpt mode
2. Select brush type from the terrain panel
3. Adjust radius, strength, and falloff
4. Left-click and drag on terrain to sculpt
5. Use scroll wheel to resize brush on the fly
6. Press `Q`/`W`/`E`/`R` to return to a standard tool mode

---

## Scene Management

### File Operations

| Shortcut | Action |
|----------|--------|
| `Ctrl + N` | New scene |
| `Ctrl + O` | Open scene |
| `Ctrl + S` | Save scene |
| `Ctrl + Shift + S` | Save scene as... |

### Scene Format

Scenes are saved as `.ron` (Rusty Object Notation) files. These contain:
- All entity components (serialized via Bevy Reflect)
- Entity hierarchy relationships
- Editor metadata (camera state, expanded hierarchy nodes)

### Scene Tabs

The editor supports multiple open scenes as tabs:
- Each tab has independent camera state
- Switching tabs hides/shows the relevant entities
- Closing a tab despawns its entities
- Modified tabs show an unsaved indicator

### Editor Metadata

Saved alongside the scene (stripped on export):
- Camera focus point, distance, yaw, pitch
- Which hierarchy nodes are expanded (by entity name)

---

## Play Mode

### Controls

| Shortcut | Action |
|----------|--------|
| `F5` | Play (fullscreen game view) |
| `Shift + F5` | Run scripts only (keep editor view) |
| `Escape` | Stop play mode |

### What Happens in Play Mode

1. The engine finds the default game camera (marked `is_default_camera = true`)
2. The editor camera is disabled, game camera is activated
3. Physics components are spawned for all physics/collision entities
4. Scripts begin executing (`on_ready()` then `on_update()` each frame)
5. The editor UI is hidden (fullscreen play) or retained (scripts-only)

### Scripts-Only Mode

`Shift + F5` runs scripts and physics without switching cameras. This is useful for testing game logic while keeping the editor view and controls.

### Stopping

Press `Escape` to stop:
- Game camera is removed
- Editor camera is restored
- Runtime physics components are despawned
- Scripts stop executing
- Scene state is restored to pre-play

---

## Scripting

### Language

The engine uses **Rhai**, a lightweight embedded scripting language with Rust-like syntax.

### Script Files

- Extension: `.rhai`
- Location: `project/scripts/` folder
- Can also use `.blueprint` files (compiled to Rhai)

### Attaching Scripts

1. Select an entity
2. Add the **Script** component
3. Click **Browse** or drag-and-drop a `.rhai` file onto the script slot
4. Multiple scripts can be attached to a single entity

### Script Lifecycle

```rhai
// Called once when the entity initializes
fn on_ready() {
    print("Hello from script!");
}

// Called every frame
fn on_update() {
    let speed = get_prop("speed");
    let pos = get_position();
    set_position(pos.x + speed * delta_time(), pos.y, pos.z);
}

// Declare editable properties (shown in inspector)
fn props() {
    return #{
        speed: #{ value: 5.0 },
        jump_height: #{ value: 2.0 },
        enable_debug: #{ value: true, hint: "Show debug visuals" }
    };
}
```

### Available Script API

**Transform:**
- `set_position(x, y, z)` / `get_position()` - Entity position
- `set_rotation(x, y, z)` / `get_rotation()` - Euler rotation
- `set_scale(x, y, z)` / `get_scale()` - Entity scale

**Math:**
- `vec2(x, y)`, `vec3(x, y, z)`, `color(r, g, b, a)` - Constructors
- `lerp(a, b, t)`, `clamp(v, min, max)` - Interpolation
- `abs()`, `min()`, `max()`, `sin()`, `cos()`, `sqrt()`, `pow()`
- `floor()`, `ceil()`, `round()`
- `random()`, `random_range(min, max)`
- `length(v)`, `normalize(v)`, `distance(a, b)` - Vector math

**Input:**
- `is_key_pressed(key)` - Key held down
- `is_key_just_pressed(key)` - Key pressed this frame

**Physics:**
- `add_force(x, y, z)` - Apply continuous force
- `apply_impulse(x, y, z)` - Instant velocity change
- `set_velocity(x, y, z)` - Set velocity directly
- `raycast(origin, direction, distance)` - Physics raycast

**Audio:**
- `play_sound(path)` - Play audio clip
- `play_sound_3d(path, position)` - Spatial audio

**Entity Queries:**
- `find_entity(name)` - Find entity by name
- `find_entities_with_tag(tag)` - Find by tag
- `get(entity, property)` / `set(entity, property, value)` - Component access

**Timers:**
- `start_timer(name, seconds)` - Start a named timer
- `is_timer_finished(name)` - Check if timer completed

**Health:**
- `take_damage(entity, amount)` - Deal damage
- `heal(entity, amount)` - Restore health

**Debug Drawing:**
- `debug_draw_line(start, end, color, duration)`
- `debug_draw_sphere(position, radius, color, duration)`
- `debug_log(message)`

### Script Variables

Variables declared in `props()` appear in the inspector and can be edited at design time. Supported types:
- Float, Int, Bool, String
- Vec2, Vec3
- Color (Vec4 RGBA)

---

## Materials & Shaders

### Blueprint Materials

The engine uses a node-graph based material system:
- Materials are authored as **blueprints** with visual node connections
- Each blueprint compiles to a **WGSL shader** at runtime
- Up to 4 textures per material
- PBR (Physically Based Rendering) support
- Hot-reloading when source files change

### Material Properties

Standard PBR properties:
- Base color / albedo
- Roughness
- Metallic
- Normal mapping
- Emissive

### Applying Materials

1. Select a mesh entity
2. In the MeshRenderer component, assign a material
3. Materials can reference `.material_bp` blueprint files

---

## Lighting

### Light Types

**Point Light:**
- Omni-directional, falls off with distance
- Properties: color, intensity (lumens), range, radius, shadows

**Directional Light:**
- Parallel rays (sun-like)
- Properties: color, illuminance (lux), shadows
- Use `SunData` component for angle-based positioning (azimuth + elevation)

**Spot Light:**
- Cone-shaped light with inner/outer angle falloff
- Properties: color, intensity, range, inner_angle, outer_angle, shadows

**Ambient Light:**
- Global fill lighting

### View Toggles

| Shortcut | Toggle |
|----------|--------|
| `Shift + Z` | Unlit mode (disable all lighting) |

---

## Physics

Physics uses the **Avian3D** engine (when the `physics` feature is enabled).

### Body Types

- **Dynamic**: Affected by gravity and forces
- **Static**: Immovable collider
- **Kinematic**: Script-controlled movement

### Collider Shapes

Box, Sphere, Capsule, Cylinder, Mesh (from entity's mesh)

### Physics in Play Mode

Physics components are spawned when entering play mode and despawned when stopping. Physics simulation pauses when play mode is paused.

---

## Post-Processing Effects

Add post-processing components to any entity (typically the World Environment):

| Effect | Description |
|--------|-------------|
| Fog | Distance/volumetric fog |
| Bloom | Light glow bleeding |
| Depth of Field | Focus blur |
| Motion Blur | Movement trails |
| Tonemapping | Color grading / exposure |
| Ambient Occlusion | SSAO shadows in crevices |
| Anti-Aliasing | FXAA, SMAA, TAA, or CAS |
| Reflections | Screen-space reflections |
| Vignette | Edge darkening |
| Film Grain | Noise overlay |
| God Rays | Volumetric light shafts |
| Gaussian Blur | Full-screen blur |
| Pixelation | Retro pixel effect |
| CRT | CRT monitor simulation |
| And more... | 15+ additional effects |

---

## Asset Browser

Located in the bottom panel under the **Assets** tab.

### Features

- **View modes**: Tile grid (64-128px thumbnails) and list view
- **Navigation**: Folder tree, breadcrumb path, back/forward
- **Search**: Filter assets by name
- **Preview**: Thumbnails for models, textures, materials, and shaders
- **Drag & Drop**: Drag assets onto entities or into the viewport

### Supported Asset Types

| Type | Extensions |
|------|-----------|
| 3D Models | .gltf, .glb, .obj, .fbx |
| Textures | .png, .jpg, .bmp, .tga, .hdr, .exr |
| Audio | .wav, .ogg, .mp3, .flac |
| Shaders | .wgsl, .glsl |
| Scenes | .ron |
| Scripts | .rhai, .blueprint |

### Import

Via **File > Import**:
- **Import Assets**: All supported types
- **3D Model**: With options for mesh handling, collider import, axis conversion
- **Image**: Texture import
- **Audio**: Sound file import

---

## Debug & Profiling Panels

### Performance Panel

- **FPS**: Current, Average, Min, Max, 1% Low (color-coded: green 60+, yellow 30-60, red <30)
- **Frame Time**: Milliseconds with 16.67ms target line
- **Entity Count**: Live count with history graph
- **System Info**: CPU/Memory usage

### Render Statistics

- GPU timing with target overlay
- Draw calls, triangle/vertex counts
- Shader invocations (vertex, fragment, compute)
- Per-render-pass timing breakdown

### Culling Debug

- Distance-based culling toggle (max range 10-2000m)
- Fade zone configuration
- Live stats: visible, frustum-culled, distance-culled, fading
- Distance distribution histogram

### Physics Debug

- Body counts by type (dynamic, static, kinematic)
- Collider distribution
- Physics step timing
- Collision pair counts
- Debug visualization toggles

### Additional Debug Tools

- **Render Pipeline**: Interactive node graph of render passes with GPU timing
- **Node Explorer**: Raw entity component inspection
- **State Recorder**: State snapshots for debugging
- **Stress Test**: Performance testing utilities

---

## Keyboard Shortcuts Reference

### File

| Shortcut | Action |
|----------|--------|
| `Ctrl + N` | New Scene |
| `Ctrl + O` | Open Scene |
| `Ctrl + S` | Save Scene |
| `Ctrl + Shift + S` | Save Scene As |
| `Ctrl + ,` | Open Settings |

### Edit

| Shortcut | Action |
|----------|--------|
| `Ctrl + Z` | Undo |
| `Ctrl + Y` | Redo |
| `Ctrl + D` | Duplicate |
| `Alt + D` | Duplicate + Grab |
| `Delete` | Delete selected |
| `Escape` | Deselect all |

### Tools

| Shortcut | Action |
|----------|--------|
| `Q` | Select tool |
| `W` | Translate gizmo |
| `E` | Rotate gizmo |
| `R` | Scale gizmo |

### Modal Transforms

| Shortcut | Action |
|----------|--------|
| `G` | Grab (move) mode |
| `R` | Rotate mode |
| `S` | Scale mode |
| `X` / `Y` / `Z` | Constrain to axis |
| `Shift + X/Y/Z` | Constrain to plane |
| `0-9`, `.`, `-` | Numeric input |
| `Backspace` | Delete last digit |
| `Enter` / Left Click | Confirm |
| `Escape` / Right Click | Cancel |

### Camera

| Shortcut | Action |
|----------|--------|
| `F` | Focus on selected entity |
| `Numpad 1/3/7` | Front / Right / Top view |
| `Ctrl + Numpad 1/3/7` | Back / Left / Bottom view |
| `Numpad 5` | Toggle Perspective / Orthographic |

### View

| Shortcut | Action |
|----------|--------|
| `Z` | Toggle wireframe |
| `Shift + Z` | Toggle unlit mode |
| `H` | Toggle grid |
| `Ctrl + Backtick` | Toggle bottom panel |

### Play

| Shortcut | Action |
|----------|--------|
| `F5` | Play / Stop (fullscreen) |
| `Shift + F5` | Run scripts only |

### All shortcuts are customizable via **Settings > Shortcuts** (`Ctrl + ,`).

---

## Tips & Tricks

### General Workflow

- **Keyboard shortcuts are context-aware**: They won't fire when you're typing in a text field.
- **Undo everything**: Almost all operations (move, rotate, scale, rename, reparent, add/remove components, duplicate, delete) are undoable with `Ctrl+Z`. The history holds up to 100 commands.
- **Rapid transforms merge**: Consecutive drag transforms within 0.5 seconds merge into a single undo step, so dragging an object smoothly won't flood your undo history.

### Selection

- **Ctrl+Click** in the viewport to toggle entities in and out of multi-selection.
- **Shift+Click** in the hierarchy panel to range-select between the anchor and target entity.
- Use the **lock icon** in the hierarchy to prevent accidental selection of background/environment entities.

### Transform Tricks

- **Alt+D** is the fastest way to duplicate and reposition: it duplicates the entity and immediately enters grab mode.
- **Modal transforms with numeric input** are extremely precise: `G Z 1.5 Enter` moves an entity exactly 1.5 units on Z.
- **Shift+Axis** excludes an axis: `G Shift+Z` moves freely on XY but locks Z in place.
- **Grid snapping** (hold `Ctrl` while dragging) defaults to 1-unit intervals. Adjust in snap settings for finer/coarser grids.

### Camera

- **F key** is your best friend: quickly frame the selected entity to focus your view.
- **Numpad views** are great for precise alignment. Use `Numpad 1` for front, `Numpad 7` for top-down.
- **Scroll wheel zoom** speed is relative to distance, so it stays useful at all scales.
- **Hold Shift** during WASD fly mode for 2x speed when traversing large scenes.

### Terrain

- **Scroll wheel resizes the brush** while the terrain tool is active. No need to go back to the panel.
- Use **Smooth brush** after raising/lowering to blend harsh edges.
- **Flatten brush** captures the height at the click point, so click on the height you want, then paint.
- The **Sculpt brush** combines raise and lower: normal click raises, hold Shift to lower.

### Scene Management

- **Multiple scene tabs** let you work on several levels at once. Each tab preserves its own camera state.
- **Scene instances** let you reference external `.ron` files. Changes to the source scene propagate to all instances.
- **Scripts-only mode** (`Shift+F5`) is great for testing game logic without losing your editor viewport.

### Scripting

- Use `props()` to expose variables in the inspector. This lets designers tweak values without editing code.
- Multiple scripts per entity are supported. Each has independent `on_ready`/`on_update` lifecycle.
- Script errors show inline in the inspector. Check there first if something isn't working.
- `find_entity("name")` and `find_entities_with_tag("tag")` are the primary ways to reference other entities from scripts.

### Performance

- Use the **Culling Debug** panel to see how many entities are being rendered. Enable distance culling to hide far-off objects.
- The **Performance panel** shows 1% low FPS, which is more telling than average FPS for identifying hitches.
- **Wireframe mode** (`Z`) helps visualize mesh density and identify overly-detailed geometry.
- Use **MeshletMesh** components for large, detailed meshes that benefit from GPU-driven rendering.

### Asset Workflow

- **Drag and drop** assets from the browser onto entities to assign them (models, textures, scripts).
- The asset browser generates **thumbnails** for 3D models automatically.
- Use **File > Import > 3D Model** for fine control over axis conversion and mesh handling.

### Keybinding Customization

- Open **Settings > Shortcuts** (`Ctrl + ,`) to rebind any key.
- Click "Press key..." next to any action and press your desired key combination.
- Supports Ctrl, Shift, Alt modifiers in any combination.
- "Reset All to Defaults" at the bottom to restore factory bindings.
