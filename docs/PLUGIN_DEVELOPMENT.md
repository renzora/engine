# Plugin Development Guide

A complete guide to building plugins for the Renzora Engine editor. Covers architecture, the plugin API, UI creation, scene access, event handling, building, testing, and current limitations.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Getting Started](#getting-started)
3. [Plugin Lifecycle](#plugin-lifecycle)
4. [The EditorPlugin Trait](#the-editorplugin-trait)
5. [Plugin Manifest](#plugin-manifest)
6. [Creating UI](#creating-ui)
7. [Panels](#panels)
8. [Menu Items](#menu-items)
9. [Toolbar Items](#toolbar-items)
10. [Status Bar Items](#status-bar-items)
11. [Widget Reference](#widget-reference)
12. [Event System](#event-system)
13. [Scene & Entity Access](#scene--entity-access)
14. [Persistent Settings](#persistent-settings)
15. [Building Plugins](#building-plugins)
16. [Hot Reload](#hot-reload)
17. [Testing](#testing)
18. [Complete Examples](#complete-examples)
19. [API Reference](#api-reference)
20. [Alpha Limitations & Gotchas](#alpha-limitations--gotchas)

---

## Architecture Overview

The plugin system uses a **three-layer architecture** with FFI isolation between plugins and the editor:

```
┌──────────────────────────────────┐
│  Editor (Bevy ECS + egui)        │  Host side - renders UI, manages world
├──────────────────────────────────┤
│  Plugin Host (src/plugin_core/)  │  Manages plugin lifecycle, FFI callbacks
├────────── DLL Boundary ──────────┤
│  Plugin API (editor_plugin_api)  │  Shared crate - types, traits, FFI wrappers
├──────────────────────────────────┤
│  Your Plugin (.dll / .so)        │  Implements EditorPlugin trait
└──────────────────────────────────┘
```

**Key design decisions:**

- Plugins are compiled as **dynamic libraries** (`.dll` on Windows, `.so` on Linux, `.dylib` on macOS)
- No Bevy types or trait objects cross the DLL boundary — only **FFI-safe C-compatible types**
- Plugins communicate with the editor through **function pointer callbacks** (vtable pattern)
- UI content is serialized as **JSON** across the boundary
- All plugin calls are wrapped in `catch_unwind()` — a crashing plugin won't take down the editor
- Plugins allocate and deallocate their own memory (no allocator mismatch)

### File Locations

```
crates/editor_plugin_api/       # Shared API crate (plugins depend on this)
  src/lib.rs                    # declare_plugin! macro
  src/traits.rs                 # EditorPlugin trait
  src/api.rs                    # EditorApi trait
  src/ui.rs                     # Widget enum, UiId, layout types
  src/events.rs                 # EditorEvent, UiEvent, EditorEventType
  src/abi.rs                    # FFI-safe types (EntityId, PluginTransform, etc.)
  src/ffi.rs                    # FFI vtable, HostApi callbacks, FfiEditorApi
  src/prelude.rs                # Convenience re-exports

src/plugin_core/                # Editor-side plugin management
  host.rs                       # PluginHost resource, FFI callback implementations
  api.rs                        # EditorApiImpl (host-side API state)
  mod.rs                        # Bevy plugin, systems

src/ui/panels/plugin_ui.rs      # Renders plugin panels, menus, toolbar, status bar
src/ui_api/renderer.rs          # UiRenderer (Widget → egui translation)
```

---

## Getting Started

### 1. Create a new Rust library project

```bash
cargo init --lib my_plugin
```

### 2. Configure Cargo.toml

```toml
[package]
name = "my_plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]   # Build as dynamic library

[dependencies]
editor_plugin_api = { path = "../engine/crates/editor_plugin_api" }
```

The `cdylib` crate type is essential — it produces a `.dll`/`.so` that the editor can load.

### 3. Write your plugin

```rust
use editor_plugin_api::prelude::*;

pub struct MyPlugin {
    counter: u32,
}

impl EditorPlugin for MyPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest::new("com.myname.my-plugin", "My Plugin", "1.0.0")
    }

    fn on_load(&mut self, api: &mut dyn EditorApi) -> Result<(), PluginError> {
        api.log_info("My plugin loaded!");
        Ok(())
    }

    fn on_unload(&mut self, _api: &mut dyn EditorApi) {}

    fn on_update(&mut self, api: &mut dyn EditorApi, _dt: f32) {
        self.counter += 1;
    }

    fn on_event(&mut self, _api: &mut dyn EditorApi, _event: &EditorEvent) {}
}

// This macro generates the FFI entry point
declare_plugin!(MyPlugin, MyPlugin { counter: 0 });
```

### 4. Build

```bash
cargo build --release
```

This produces `target/release/my_plugin.dll` (or `.so` on Linux).

### 5. Install

Copy the built library to your project's `plugins/` directory, or to the editor's system plugins folder. The editor scans for plugins on startup and when the folder changes.

---

## Plugin Lifecycle

```
UNLOADED
  │  Editor discovers DLL in plugins/ directory
  │  Calls create_plugin() to probe manifest
  │  Checks FFI version and API version
  ▼
LOADING
  │  on_load() called
  │  Register panels, menus, toolbar items
  │  Subscribe to events
  │  Initialize state
  ▼
RUNNING
  │  Every frame:
  │    on_update(api, delta_time) called
  │    Plugin builds UI, polls events, modifies state
  │  On editor events:
  │    on_event(api, event) called
  ▼
UNLOADING
  │  on_unload() called
  │  All registered UI elements auto-removed
  │  Plugin struct dropped
  ▼
UNLOADED
```

### Frame execution order

Each editor frame, plugins are processed in this order:

1. **Sync** — Editor snapshots current Bevy state (selection, transforms, names) into the plugin API
2. **Update** — `on_update()` called for each plugin. Plugins build UI, poll events, queue operations
3. **Apply** — Pending operations (transform changes, spawns, etc.) applied to Bevy world
4. **Render** — Editor renders all UI including plugin panels, menus, toolbar, status bar
5. **Forward** — UI events from rendered widgets are collected and stored for next frame's `poll_ui_events()`
6. **Events** — Editor events (selection change, play mode, etc.) delivered via `on_event()`

---

## The EditorPlugin Trait

Every plugin must implement this trait:

```rust
pub trait EditorPlugin: Send + Sync {
    /// Return plugin metadata (id, name, version, capabilities)
    fn manifest(&self) -> PluginManifest;

    /// Called once when the plugin is loaded. Register UI, subscribe to events.
    fn on_load(&mut self, api: &mut dyn EditorApi) -> Result<(), PluginError>;

    /// Called once when the plugin is unloaded. Clean up resources.
    fn on_unload(&mut self, api: &mut dyn EditorApi);

    /// Called every frame. Build UI, handle events, update state.
    fn on_update(&mut self, api: &mut dyn EditorApi, dt: f32);

    /// Called when subscribed editor events occur.
    fn on_event(&mut self, api: &mut dyn EditorApi, event: &EditorEvent);
}
```

And use the `declare_plugin!` macro to generate the FFI entry point:

```rust
declare_plugin!(MyPlugin, MyPlugin::new());
```

The macro generates:
- A `#[no_mangle] extern "C" fn create_plugin()` entry point
- FFI wrapper functions for each trait method
- Proper memory management (Box allocation/deallocation)

---

## Plugin Manifest

The manifest declares your plugin's identity and capabilities:

```rust
fn manifest(&self) -> PluginManifest {
    PluginManifest::new(
        "com.yourname.plugin-id",   // Unique reverse-domain ID
        "Plugin Display Name",       // Shown in settings
        "1.0.0",                     // Semantic version
    )
    .author("Your Name")
    .description("What this plugin does")
    .capability(PluginCapability::Panel)
    .capability(PluginCapability::MenuItem)
}
```

### Capabilities

Declare what your plugin needs:

| Capability | Allows |
|-----------|--------|
| `Panel` | Register custom panels in the editor |
| `MenuItem` | Add items to editor menus |
| `Toolbar` | Add toolbar buttons |
| `StatusBar` | Add status bar items |
| `EntityAccess` | Read/write entity data |
| `AssetAccess` | Load and list assets |

### Dependencies

Plugins can declare dependencies on other plugins:

```rust
PluginManifest::new(...)
    .dependency(PluginDependency::new("com.other.plugin", ">=1.0.0"))
```

Dependencies are resolved via topological sort — if plugin A depends on B, B loads first. Circular dependencies are detected and rejected.

---

## Creating UI

Plugins create UI using an **abstract widget system**. You build a `Vec<Widget>` and pass it to the editor. The editor translates widgets to egui automatically.

**You never call egui directly.** This keeps the FFI boundary clean and ensures your plugin works with any editor theme.

### Basic pattern

```rust
fn on_update(&mut self, api: &mut dyn EditorApi, dt: f32) {
    // 1. Build UI from current state
    let widgets = vec![
        Widget::heading("My Plugin"),
        Widget::separator(),
        Widget::label(format!("FPS: {:.0}", 1.0 / dt)),
        Widget::button("Reset", UiId::new(1)),
    ];

    // 2. Send to editor for rendering
    api.set_panel_content("my_panel", widgets);

    // 3. Handle interactions from last frame
    for event in api.poll_ui_events() {
        if let UiEvent::ButtonClicked(id) = event {
            if id.0 == 1 {
                self.on_reset();
            }
        }
    }
}
```

UI is rebuilt every frame. This is intentional — it keeps rendering simple and stateless. The editor handles diffing and caching internally.

### Widget identification

Every interactive widget needs a `UiId` so events can be routed back:

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct UiId(pub u64);
```

Use consistent IDs across frames. A common pattern is constants:

```rust
const BTN_APPLY: u64 = 1;
const BTN_CANCEL: u64 = 2;
const SLIDER_SPEED: u64 = 3;
const INPUT_NAME: u64 = 4;
```

---

## Panels

Panels are the primary way plugins add UI to the editor. They appear as tabs alongside built-in panels (Hierarchy, Inspector, Assets).

### Registering a panel

```rust
fn on_load(&mut self, api: &mut dyn EditorApi) -> Result<(), PluginError> {
    let panel = PanelDefinition::new("my_panel", "My Panel")
        .icon(egui_phosphor::regular::GEAR)   // Phosphor icon
        .location(PanelLocation::Right)        // Dock location
        .min_size(250.0, 200.0);               // Minimum dimensions

    api.register_panel(panel);
    Ok(())
}
```

### Panel locations

| Location | Where It Docks |
|----------|---------------|
| `PanelLocation::Left` | Tab alongside Hierarchy panel |
| `PanelLocation::Right` | Tab alongside Inspector panel |
| `PanelLocation::Bottom` | Tab alongside Assets/Console panels |
| `PanelLocation::Floating` | Independent floating window |

### Updating panel content

Call `set_panel_content()` each frame in `on_update()`:

```rust
fn on_update(&mut self, api: &mut dyn EditorApi, _dt: f32) {
    let widgets = vec![
        Widget::heading("Settings"),
        Widget::slider(self.brightness, 0.0, 2.0, UiId::new(1), Some("Brightness".into())),
        Widget::checkbox("Enable bloom", self.bloom, UiId::new(2)),
    ];
    api.set_panel_content("my_panel", widgets);
}
```

---

## Menu Items

Add items to the editor's menu bar:

```rust
fn on_load(&mut self, api: &mut dyn EditorApi) -> Result<(), PluginError> {
    // Simple menu item
    let item = MenuItem::new("My Tool", UiId::new(100))
        .icon("🔧")
        .shortcut("Ctrl+Shift+M");

    api.register_menu_item(MenuLocation::Tools, item);

    // Submenu
    let submenu = MenuItem::new("My Tools", UiId::new(200))
        .submenu(vec![
            MenuItem::new("Option A", UiId::new(201)),
            MenuItem::new("Option B", UiId::new(202)).disabled(),
        ]);

    api.register_menu_item(MenuLocation::Tools, submenu);
    Ok(())
}
```

### Menu locations

| Location | Menu |
|----------|------|
| `MenuLocation::File` | File menu (after built-in items) |
| `MenuLocation::Edit` | Edit menu |
| `MenuLocation::Tools` | Tools menu (primary location for plugins) |
| `MenuLocation::View` | View menu |
| `MenuLocation::Help` | Help menu |
| `MenuLocation::Custom(name)` | Create a new top-level menu |

Menu clicks generate `UiEvent::ButtonClicked` with the item's `UiId`.

---

## Toolbar Items

Add icon buttons to the editor toolbar:

```rust
fn on_load(&mut self, api: &mut dyn EditorApi) -> Result<(), PluginError> {
    api.register_toolbar_item(
        ToolbarItem::new(UiId::new(50), "🔍", "Search entities")
            .group("my_tools")
    );
    Ok(())
}
```

Toolbar clicks generate `UiEvent::ButtonClicked`.

---

## Status Bar Items

Display information in the editor's bottom status bar:

```rust
fn on_update(&mut self, api: &mut dyn EditorApi, dt: f32) {
    let fps = 1.0 / dt.max(0.001);

    api.set_status_item(
        StatusBarItem::new("fps_counter", format!("FPS: {:.0}", fps))
            .icon("📊")
            .tooltip("Current frames per second")
            .align_right()
            .priority(10)
    );
}
```

Status items can be updated every frame. They are removed automatically when the plugin unloads.

| Property | Description |
|----------|-------------|
| `align_right()` | Place on right side of status bar (default is left) |
| `priority(n)` | Higher priority = further from edge |

---

## Widget Reference

### Basic Elements

```rust
Widget::label("Text")                              // Plain text
Widget::Label { text: "Bold".into(), style: TextStyle::Heading1 }  // Styled text
Widget::heading("Section Title")                    // Heading (shorthand for Heading1)
Widget::separator()                                 // Horizontal line
Widget::spacer(16.0)                                // Vertical space
Widget::Empty                                       // Nothing (placeholder)
```

### Buttons

```rust
Widget::button("Click Me", UiId::new(1))           // Standard button
Widget::Button { label: "Save".into(), id: UiId::new(2), enabled: false }  // Disabled
Widget::IconButton {                                 // Icon-only button
    icon: "🗑️".into(),
    tooltip: "Delete".into(),
    id: UiId::new(3),
    enabled: true,
}
```

### Input Widgets

```rust
// Text input (single line)
Widget::text_input("current value", UiId::new(10))

// Multi-line text editor
Widget::TextEdit {
    value: "line 1\nline 2".into(),
    id: UiId::new(11),
    min_lines: 3,
    max_lines: 10,
}

// Checkbox
Widget::checkbox("Enable feature", true, UiId::new(12))

// Float slider
Widget::slider(0.5, 0.0, 1.0, UiId::new(13), Some("Volume".into()))

// Integer slider
Widget::SliderInt { value: 5, min: 0, max: 100, id: UiId::new(14), label: Some("Count".into()) }

// Dropdown
Widget::Dropdown {
    selected: 0,
    options: vec!["Low".into(), "Medium".into(), "High".into()],
    id: UiId::new(15),
}

// Color picker
Widget::ColorPicker { color: [1.0, 0.5, 0.0, 1.0], id: UiId::new(16), alpha: true }
```

### Containers

```rust
// Horizontal row
Widget::Row {
    children: vec![Widget::button("A", UiId::new(1)), Widget::button("B", UiId::new(2))],
    spacing: 8.0,
    align: Align::Center,
}

// Vertical column
Widget::Column {
    children: vec![Widget::label("Line 1"), Widget::label("Line 2")],
    spacing: 4.0,
    align: Align::Start,
}

// Collapsible panel
Widget::Panel {
    title: "Advanced Settings".into(),
    children: vec![/* widgets */],
    collapsible: true,
    default_open: false,
}

// Scroll area
Widget::ScrollArea {
    child: Box::new(Widget::Column { children: long_list, spacing: 4.0, align: Align::Start }),
    max_height: Some(300.0),
    horizontal: false,
}

// Visual group with optional frame
Widget::Group {
    children: vec![/* widgets */],
    frame: true,
}
```

### Complex Widgets

```rust
// Tree node (expandable)
Widget::TreeNode {
    label: "Parent".into(),
    id: UiId::new(20),
    expanded: true,
    leaf: false,
    children: vec![
        Widget::TreeNode { label: "Child A".into(), id: UiId::new(21), expanded: false, leaf: true, children: vec![] },
        Widget::TreeNode { label: "Child B".into(), id: UiId::new(22), expanded: false, leaf: true, children: vec![] },
    ],
}

// Tabs
Widget::Tabs {
    tabs: vec![
        Tab::new("General", vec![Widget::label("General settings here")]),
        Tab::new("Advanced", vec![Widget::label("Advanced settings here")]).closable(),
    ],
    active: 0,
    id: UiId::new(30),
}

// Table
Widget::Table {
    columns: vec![
        TableColumn { header: "Name".into(), width: Size::Fill, sortable: true, resizable: true },
        TableColumn { header: "Value".into(), width: Size::Fixed(80.0), sortable: false, resizable: false },
    ],
    rows: vec![
        TableRow { cells: vec![Widget::label("Speed"), Widget::label("10.0")], id: UiId::new(40) },
        TableRow { cells: vec![Widget::label("Health"), Widget::label("100")], id: UiId::new(41) },
    ],
    id: UiId::new(42),
    striped: true,
}

// Progress bar
Widget::ProgressBar { progress: 0.75, label: Some("75%".into()) }

// Image
Widget::Image { path: "textures/icon.png".into(), size: Some([64.0, 64.0]) }
```

### Text Styles

| Style | Appearance |
|-------|-----------|
| `TextStyle::Body` | Default 14px proportional |
| `TextStyle::Heading1` | Large 24px heading |
| `TextStyle::Heading2` | Medium 20px heading |
| `TextStyle::Heading3` | Small 16px heading |
| `TextStyle::Caption` | Small 12px muted text |
| `TextStyle::Code` | 14px monospace |
| `TextStyle::Label` | 13px form label |

### Layout Types

```rust
pub enum Size {
    Auto,           // Automatic sizing
    Fixed(f32),     // Exact pixels
    Percent(f32),   // Percentage of available space
    Fill,           // Take all remaining space
    FillPortion(u32), // Share of remaining space
}

pub enum Align {
    Start,    // Left/top
    Center,   // Centered
    End,      // Right/bottom
    Stretch,  // Fill available space
}
```

---

## Event System

### UI Events

Generated when users interact with your widgets:

```rust
pub enum UiEvent {
    ButtonClicked(UiId),
    CheckboxToggled { id: UiId, checked: bool },
    SliderChanged { id: UiId, value: f32 },
    SliderIntChanged { id: UiId, value: i32 },
    TextInputChanged { id: UiId, value: String },
    TextInputSubmitted { id: UiId, value: String },    // Enter pressed
    DropdownSelected { id: UiId, index: u32 },
    ColorChanged { id: UiId, color: [f32; 4] },
    TreeNodeToggled { id: UiId, expanded: bool },
    TreeNodeSelected(UiId),
    TabSelected { id: UiId, index: u32 },
    TabClosed { id: UiId, index: u32 },
    TableRowSelected { id: UiId, row: u32 },
    TableSortChanged { id: UiId, column: u32, ascending: bool },
    CustomEvent { type_id: String, data: Vec<u8> },
}
```

Poll them each frame:

```rust
fn on_update(&mut self, api: &mut dyn EditorApi, _dt: f32) {
    for event in api.poll_ui_events() {
        match event {
            UiEvent::ButtonClicked(id) if id.0 == 1 => { /* handle */ }
            UiEvent::SliderChanged { id, value } if id.0 == 2 => {
                self.speed = value;
            }
            UiEvent::TextInputSubmitted { id, value } if id.0 == 3 => {
                self.name = value;
            }
            _ => {}
        }
    }
}
```

### Editor Events

Subscribe to editor state changes:

```rust
fn on_load(&mut self, api: &mut dyn EditorApi) -> Result<(), PluginError> {
    api.subscribe(EditorEventType::EntitySelected);
    api.subscribe(EditorEventType::PlayStarted);
    api.subscribe(EditorEventType::PlayStopped);
    // Or subscribe to everything:
    // api.subscribe(EditorEventType::All);
    Ok(())
}

fn on_event(&mut self, api: &mut dyn EditorApi, event: &EditorEvent) {
    match event {
        EditorEvent::EntitySelected(entity_id) => {
            self.selected = Some(*entity_id);
        }
        EditorEvent::PlayStarted => {
            api.log_info("Play mode started");
        }
        EditorEvent::SceneLoaded { path } => {
            api.log_info(&format!("Scene loaded: {}", path));
        }
        _ => {}
    }
}
```

### Available event types

| Event | When It Fires |
|-------|--------------|
| `EntitySelected(EntityId)` | User selects an entity |
| `EntityDeselected(EntityId)` | User deselects an entity |
| `SceneLoaded { path }` | A scene file is loaded |
| `SceneSaved { path }` | A scene file is saved |
| `PlayStarted` | Play mode entered |
| `PlayStopped` | Play mode exited |
| `ProjectOpened { path }` | A project is opened |
| `ProjectClosed` | Current project closed |
| `UiEvent(UiEvent)` | A UI interaction occurred |
| `CustomEvent { plugin_id, event_type, data }` | Another plugin emitted a custom event |

### Custom events (inter-plugin communication)

Plugins can emit custom events that other plugins receive:

```rust
// Plugin A emits
api.emit_event(CustomEvent {
    event_type: "my_plugin.data_updated".into(),
    data: serde_json::to_vec(&my_data).unwrap(),
});

// Plugin B receives (if subscribed to CustomEvent)
fn on_event(&mut self, _api: &mut dyn EditorApi, event: &EditorEvent) {
    if let EditorEvent::CustomEvent { plugin_id, event_type, data } = event {
        if event_type == "my_plugin.data_updated" {
            // Handle data from other plugin
        }
    }
}
```

---

## Scene & Entity Access

Plugins can read and modify the scene through the `EditorApi`:

### Reading entities

```rust
fn on_update(&mut self, api: &mut dyn EditorApi, _dt: f32) {
    // Get selected entity
    if let Some(entity) = api.get_selected_entity() {
        // Read entity properties
        let name = api.get_entity_name(entity);
        let transform = api.get_transform(entity);
        let visible = api.get_entity_visible(entity);
        let parent = api.get_entity_parent(entity);
        let children = api.get_entity_children(entity);
    }
}
```

### Modifying entities

```rust
// Change selection
api.set_selected_entity(Some(entity_id));

// Rename
api.set_entity_name(entity_id, "New Name".into());

// Move/rotate/scale
api.set_transform(entity_id, &PluginTransform {
    translation: [1.0, 2.0, 3.0],
    rotation: [0.0, 0.0, 0.0, 1.0],  // Quaternion (x, y, z, w)
    scale: [1.0, 1.0, 1.0],
});

// Toggle visibility
api.set_entity_visible(entity_id, false);

// Reparent
api.reparent_entity(entity_id, Some(new_parent_id));
api.reparent_entity(entity_id, None);  // Make root
```

### Creating and destroying entities

```rust
// Spawn a new entity
let new_entity = api.spawn_entity(&EntityDefinition {
    name: "My Entity".into(),
    node_type: "empty".into(),
    transform: PluginTransform::default(),
    parent: None,
});

// Delete an entity
api.despawn_entity(entity_id);
```

### Finding entities

```rust
// Find by name
if let Some(entity) = api.get_entity_by_name("Player") {
    // ...
}
```

### PluginTransform

The FFI-safe transform type:

```rust
#[repr(C)]
pub struct PluginTransform {
    pub translation: [f32; 3],     // Position (x, y, z)
    pub rotation: [f32; 4],        // Quaternion (x, y, z, w)
    pub scale: [f32; 3],           // Scale (x, y, z)
}
```

### EntityId

Opaque handle to an entity:

```rust
#[repr(C)]
pub struct EntityId(pub u64);

impl EntityId {
    pub fn is_valid(&self) -> bool { self.0 != u64::MAX }
}
```

Always check validity before using — entities can be despawned between frames.

### Asset access

```rust
// List assets in a folder
let assets = api.get_asset_list("models/");

// Load an asset
let handle = api.load_asset("textures/icon.png");

// Check loading status
match api.asset_status(handle) {
    AssetStatus::Loading => { /* still loading */ }
    AssetStatus::Loaded => { /* ready to use */ }
    AssetStatus::Failed => { /* load error */ }
    AssetStatus::NotLoaded => { /* not started */ }
}
```

Asset paths are **validated and sandboxed** — you can only access files within the project's assets directory. Absolute paths, `..` traversal, and drive letters are rejected.

### Undo/Redo

```rust
if api.can_undo() {
    api.undo();
}
if api.can_redo() {
    api.redo();
}
```

---

## Persistent Settings

Plugins can save settings that persist across sessions:

```rust
fn on_load(&mut self, api: &mut dyn EditorApi) -> Result<(), PluginError> {
    // Load saved settings
    if let Some(SettingValue::Float(v)) = api.get_setting("brightness") {
        self.brightness = v as f32;
    }
    if let Some(SettingValue::Bool(v)) = api.get_setting("bloom_enabled") {
        self.bloom = v;
    }
    Ok(())
}

fn on_update(&mut self, api: &mut dyn EditorApi, _dt: f32) {
    // Save when values change
    api.set_setting("brightness", SettingValue::Float(self.brightness as f64));
    api.set_setting("bloom_enabled", SettingValue::Bool(self.bloom));
}
```

Settings survive plugin reloads and editor restarts.

---

## Building Plugins

### Build command

```bash
cargo build --release
```

Output:
- Windows: `target/release/my_plugin.dll`
- Linux: `target/release/libmy_plugin.so`
- macOS: `target/release/libmy_plugin.dylib`

### Plugin installation

Copy the built library to one of:
- **Project plugins**: `your_project/plugins/` (project-specific)
- **System plugins**: Editor's built-in plugins directory (global)

The editor scans both locations on startup.

### Dev menu shortcuts

The editor's Dev menu (visible when `dev_mode` is enabled in settings) provides:
- **New Plugin** — Create a plugin from template
- **Open Plugin Source** — Open plugin source directory
- **Build Plugin** (`Ctrl+B`) — Build the current plugin
- **Open Cargo.toml** — Edit plugin dependencies

---

## Hot Reload

The editor watches plugin directories for file changes. When a `.dll`/`.so` is modified:

1. The old plugin instance receives `on_unload()`
2. All registered UI elements are removed
3. The old DLL is released
4. The new DLL is loaded
5. `create_plugin()` is called
6. `on_load()` is called on the new instance

**State is lost on reload** — the old plugin struct is dropped. If you need state to survive hot reload, save it to persistent settings before unload and restore in `on_load()`:

```rust
fn on_unload(&mut self, api: &mut dyn EditorApi) {
    // Save state before reload
    api.set_setting("my_counter", SettingValue::Int(self.counter as i64));
}

fn on_load(&mut self, api: &mut dyn EditorApi) -> Result<(), PluginError> {
    // Restore state after reload
    if let Some(SettingValue::Int(v)) = api.get_setting("my_counter") {
        self.counter = v as u32;
    }
    Ok(())
}
```

---

## Testing

### Unit tests

Test plugin logic without the editor:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest() {
        let plugin = MyPlugin::new();
        let manifest = plugin.manifest();
        assert_eq!(manifest.id, "com.myname.my-plugin");
        assert_eq!(manifest.version, "1.0.0");
    }

    #[test]
    fn test_state_logic() {
        let mut plugin = MyPlugin::new();
        plugin.counter = 5;
        plugin.reset();
        assert_eq!(plugin.counter, 0);
    }
}
```

### Integration testing

Build the plugin, copy it to the editor's plugins directory, and:

1. Launch the editor
2. Open **Settings > Plugins** to verify your plugin is listed and loaded
3. Check the **Console** panel for log messages from your plugin
4. Interact with your plugin's panels, menus, toolbar items
5. Modify the plugin source, rebuild — hot reload should pick up changes

### Debugging

```rust
// Log to the editor's console panel
api.log_info("Debug: variable value");
api.log_warn("Warning: something unexpected");
api.log_error("Error: operation failed");

// Inspect state
api.log_info(&format!("Selected: {:?}", api.get_selected_entity()));
api.log_info(&format!("Transform: {:?}", api.get_transform(entity)));

// Trace UI events
for event in api.poll_ui_events() {
    api.log_info(&format!("UI Event: {:?}", event));
}
```

Log messages appear in the Console panel with a `[Plugin]` prefix.

---

## Complete Examples

### Example 1: FPS Monitor

A simple plugin that shows FPS in the status bar and a diagnostics panel.

```rust
use editor_plugin_api::prelude::*;

pub struct FpsMonitor {
    frame_count: u64,
    fps: f32,
}

impl EditorPlugin for FpsMonitor {
    fn manifest(&self) -> PluginManifest {
        PluginManifest::new("com.example.fps-monitor", "FPS Monitor", "1.0.0")
            .capability(PluginCapability::Panel)
            .capability(PluginCapability::StatusBar)
    }

    fn on_load(&mut self, api: &mut dyn EditorApi) -> Result<(), PluginError> {
        let panel = PanelDefinition::new("fps_panel", "Performance")
            .icon(egui_phosphor::regular::CHART_LINE)
            .location(PanelLocation::Right);
        api.register_panel(panel);
        Ok(())
    }

    fn on_unload(&mut self, _api: &mut dyn EditorApi) {}

    fn on_update(&mut self, api: &mut dyn EditorApi, dt: f32) {
        self.frame_count += 1;
        self.fps = 1.0 / dt.max(0.001);

        // Panel UI
        api.set_panel_content("fps_panel", vec![
            Widget::heading("Performance"),
            Widget::separator(),
            Widget::label(format!("FPS: {:.1}", self.fps)),
            Widget::label(format!("Frame time: {:.2}ms", dt * 1000.0)),
            Widget::label(format!("Total frames: {}", self.frame_count)),
            Widget::separator(),
            Widget::ProgressBar {
                progress: (self.fps / 60.0).min(1.0),
                label: Some(format!("{:.0}/60", self.fps)),
            },
            Widget::separator(),
            Widget::button("Reset counter", UiId::new(1)),
        ]);

        // Status bar
        let color_hint = if self.fps >= 60.0 { "✅" } else if self.fps >= 30.0 { "⚠️" } else { "❌" };
        api.set_status_item(
            StatusBarItem::new("fps", format!("{} {:.0} FPS", color_hint, self.fps))
                .align_right()
                .priority(100)
        );

        // Handle events
        for event in api.poll_ui_events() {
            if let UiEvent::ButtonClicked(id) = event {
                if id.0 == 1 { self.frame_count = 0; }
            }
        }
    }

    fn on_event(&mut self, _api: &mut dyn EditorApi, _event: &EditorEvent) {}
}

declare_plugin!(FpsMonitor, FpsMonitor { frame_count: 0, fps: 0.0 });
```

### Example 2: Entity Inspector Plugin

A plugin that shows detailed info about the selected entity with editing controls.

```rust
use editor_plugin_api::prelude::*;

pub struct EntityInspector {
    selected: Option<EntityId>,
    offset_x: f32,
    offset_y: f32,
    offset_z: f32,
}

impl EditorPlugin for EntityInspector {
    fn manifest(&self) -> PluginManifest {
        PluginManifest::new("com.example.entity-inspector", "Entity Inspector", "1.0.0")
            .capability(PluginCapability::Panel)
            .capability(PluginCapability::EntityAccess)
    }

    fn on_load(&mut self, api: &mut dyn EditorApi) -> Result<(), PluginError> {
        let panel = PanelDefinition::new("entity_inspect", "Entity Details")
            .location(PanelLocation::Right);
        api.register_panel(panel);
        api.subscribe(EditorEventType::EntitySelected);
        api.subscribe(EditorEventType::EntityDeselected);
        Ok(())
    }

    fn on_unload(&mut self, _api: &mut dyn EditorApi) {}

    fn on_update(&mut self, api: &mut dyn EditorApi, _dt: f32) {
        let mut widgets = vec![Widget::heading("Entity Details"), Widget::separator()];

        if let Some(entity) = self.selected {
            if let Some(name) = api.get_entity_name(entity) {
                widgets.push(Widget::label(format!("Name: {}", name)));
            }

            if let Some(transform) = api.get_transform(entity) {
                widgets.push(Widget::separator());
                widgets.push(Widget::Label {
                    text: "Position".into(),
                    style: TextStyle::Heading3,
                });
                widgets.push(Widget::slider(
                    transform.translation[0], -100.0, 100.0,
                    UiId::new(10), Some("X".into()),
                ));
                widgets.push(Widget::slider(
                    transform.translation[1], -100.0, 100.0,
                    UiId::new(11), Some("Y".into()),
                ));
                widgets.push(Widget::slider(
                    transform.translation[2], -100.0, 100.0,
                    UiId::new(12), Some("Z".into()),
                ));

                // Handle slider changes
                for event in api.poll_ui_events() {
                    if let UiEvent::SliderChanged { id, value } = event {
                        let mut t = transform;
                        match id.0 {
                            10 => t.translation[0] = value,
                            11 => t.translation[1] = value,
                            12 => t.translation[2] = value,
                            _ => continue,
                        }
                        api.set_transform(entity, &t);
                    }
                }
            }

            let visible = api.get_entity_visible(entity);
            widgets.push(Widget::separator());
            widgets.push(Widget::checkbox("Visible", visible, UiId::new(20)));

            if let Some(parent) = api.get_entity_parent(entity) {
                if let Some(parent_name) = api.get_entity_name(parent) {
                    widgets.push(Widget::label(format!("Parent: {}", parent_name)));
                }
            }

            let children = api.get_entity_children(entity);
            if !children.is_empty() {
                widgets.push(Widget::label(format!("Children: {}", children.len())));
            }
        } else {
            widgets.push(Widget::label("No entity selected"));
        }

        api.set_panel_content("entity_inspect", widgets);
    }

    fn on_event(&mut self, _api: &mut dyn EditorApi, event: &EditorEvent) {
        match event {
            EditorEvent::EntitySelected(id) => self.selected = Some(*id),
            EditorEvent::EntityDeselected(_) => self.selected = None,
            _ => {}
        }
    }
}

declare_plugin!(EntityInspector, EntityInspector {
    selected: None, offset_x: 0.0, offset_y: 0.0, offset_z: 0.0,
});
```

---

## API Reference

### Logging

| Method | Description |
|--------|-------------|
| `api.log_info(msg)` | Info message to console |
| `api.log_warn(msg)` | Warning message |
| `api.log_error(msg)` | Error message |

### UI Registration (call in `on_load`)

| Method | Description |
|--------|-------------|
| `api.register_panel(PanelDefinition)` | Register a dockable panel |
| `api.register_menu_item(location, MenuItem)` | Add a menu item |
| `api.register_toolbar_item(ToolbarItem)` | Add a toolbar button |
| `api.register_tab(PluginTab)` | Register a tab |

### UI Content (call in `on_update`)

| Method | Description |
|--------|-------------|
| `api.set_panel_content(id, Vec<Widget>)` | Update panel widgets |
| `api.set_tab_content(id, Vec<Widget>)` | Update tab widgets |
| `api.set_status_item(StatusBarItem)` | Update status bar |
| `api.poll_ui_events() -> Vec<UiEvent>` | Get widget interactions |

### Entity Access

| Method | Description |
|--------|-------------|
| `api.get_selected_entity() -> Option<EntityId>` | Current selection |
| `api.set_selected_entity(Option<EntityId>)` | Change selection |
| `api.get_transform(entity) -> Option<PluginTransform>` | Read transform |
| `api.set_transform(entity, &PluginTransform)` | Write transform |
| `api.get_entity_name(entity) -> Option<String>` | Read name |
| `api.set_entity_name(entity, String)` | Write name |
| `api.get_entity_visible(entity) -> bool` | Read visibility |
| `api.set_entity_visible(entity, bool)` | Write visibility |
| `api.get_entity_parent(entity) -> Option<EntityId>` | Get parent |
| `api.get_entity_children(entity) -> Vec<EntityId>` | Get children |
| `api.reparent_entity(entity, Option<EntityId>)` | Change parent |
| `api.spawn_entity(&EntityDefinition) -> EntityId` | Create entity |
| `api.despawn_entity(entity)` | Delete entity |

### Assets

| Method | Description |
|--------|-------------|
| `api.load_asset(path) -> AssetHandle` | Start loading asset |
| `api.asset_status(handle) -> AssetStatus` | Check load status |
| `api.get_asset_list(folder) -> Vec<String>` | List assets in folder |

### Events

| Method | Description |
|--------|-------------|
| `api.subscribe(EditorEventType)` | Subscribe to events |
| `api.emit_event(CustomEvent)` | Send custom event to other plugins |

### Undo/Redo

| Method | Description |
|--------|-------------|
| `api.can_undo() -> bool` | Check if undo available |
| `api.can_redo() -> bool` | Check if redo available |
| `api.undo() -> bool` | Perform undo |
| `api.redo() -> bool` | Perform redo |

### Settings

| Method | Description |
|--------|-------------|
| `api.get_setting(key) -> Option<SettingValue>` | Read setting |
| `api.set_setting(key, SettingValue)` | Write setting |

---

## Alpha Limitations & Gotchas

### Current Limitations

| Limitation | Details |
|-----------|---------|
| **No direct egui access** | Plugins use the abstract Widget system only. You cannot call egui functions directly. |
| **No direct Bevy World access** | All entity/component access goes through the EditorApi. You cannot query arbitrary Bevy components. |
| **No custom components** | Plugins cannot register new component types — only built-in components are accessible. |
| **No custom gizmos** | Plugins cannot draw into the 3D viewport. |
| **No custom shaders** | Plugins cannot register new materials or shader effects. |
| **No async/threading** | Plugin callbacks run on the main thread. Long operations will block the editor. |
| **Limited widget set** | The Widget enum is fixed — you cannot create entirely new widget types (though `Widget::Custom` exists for future extensibility). |
| **State lost on hot reload** | Plugin struct is dropped and recreated. Use persistent settings to survive reloads. |
| **JSON serialization overhead** | Widget content is serialized to JSON for FFI. Very large UIs (1000+ widgets) may have measurable overhead. |
| **Single collider per entity** | Same as the editor — physics limitation, not plugin-specific. |

### Gotchas

**1. Always check EntityId validity**

Entities can be despawned between frames. Never cache an EntityId and assume it's valid later:

```rust
// Bad
let entity = api.get_selected_entity().unwrap(); // might panic next frame

// Good
if let Some(entity) = api.get_selected_entity() {
    if let Some(name) = api.get_entity_name(entity) {
        // Entity still exists
    }
}
```

**2. UiId must be stable across frames**

If you generate random UiIds each frame, event routing will break:

```rust
// Bad - different ID each frame
Widget::button("Click", UiId::new(rand::random()))

// Good - constant ID
const MY_BUTTON: u64 = 1;
Widget::button("Click", UiId::new(MY_BUTTON))
```

**3. Poll events every frame**

UI events queue up. If you skip polling, you'll get stale events:

```rust
fn on_update(&mut self, api: &mut dyn EditorApi, _dt: f32) {
    // Always poll, even if you don't expect events
    for event in api.poll_ui_events() {
        self.handle_event(event);
    }
}
```

**4. set_panel_content replaces, not appends**

Each call replaces the entire panel content. Build your full widget tree each frame:

```rust
// This replaces "Line 1" with "Line 2" — not both
api.set_panel_content("panel", vec![Widget::label("Line 1")]);
api.set_panel_content("panel", vec![Widget::label("Line 2")]); // Only this shows
```

**5. Operations are queued, not immediate**

`set_transform()`, `spawn_entity()`, etc. queue operations that apply after `on_update()` returns. You won't see the result until next frame:

```rust
// This won't work — entity hasn't been created yet
let entity = api.spawn_entity(&def);
let name = api.get_entity_name(entity); // Returns None this frame
```

**6. FFI version must match**

If you update the editor but not your plugin (or vice versa), the FFI version check will reject the plugin. Rebuild plugins after updating the editor.

**7. `cdylib` crate type is required**

Without `crate-type = ["cdylib"]` in Cargo.toml, Rust produces a static library that the editor can't load.

**8. Plugin panics are caught but logged**

If your plugin panics, the editor catches it and logs an error. The plugin may be left in an inconsistent state. Use proper error handling:

```rust
// Bad
let value = some_option.unwrap(); // Panics if None

// Good
let value = match some_option {
    Some(v) => v,
    None => {
        api.log_error("Expected value was missing");
        return;
    }
};
```

**9. Theme colors are automatic**

Don't hardcode colors in your widgets. The editor applies its active theme automatically. Your plugin will look correct in any theme without extra work.

**10. Plugins tab in Settings**

Users can enable/disable individual plugins in **Settings > Plugins**. Handle your plugin being loaded/unloaded gracefully.
