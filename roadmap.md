# Renzora Engine Roadmap

---

## Hierarchy — Selection & Editing
- ✅ Add copy/paste entities to clipboard — [`7f0ccec`](https://github.com/renzora/engine/commit/7f0ccec)
- ✅ Deep-clone all components on duplicate — [`7f0ccec`](https://github.com/renzora/engine/commit/7f0ccec)
- ✅ Add right-click "Add Component" directly from hierarchy — [`6610d1a`](https://github.com/renzora/engine/commit/6610d1a)
- Add batch rename for multiple entities
- Add marquee drag selection in hierarchy
- Add favorites/starred entities

## Hierarchy — Prefabs & Templates
- Add entity templates/prefabs in hierarchy
- Implement prefab/template system for reusable entity compositions

## Inspector — Property Editing
- ✅ Add reset-to-default button per property — [`5d583a3`](https://github.com/renzora/engine/commit/5d583a3)
- ✅ Add copy/paste component values — [`5d583a3`](https://github.com/renzora/engine/commit/5d583a3)
- ✅ Add copy/paste entire components between entities — [`5d583a3`](https://github.com/renzora/engine/commit/5d583a3)
- ✅ Add component drag-reorder — [`5d583a3`](https://github.com/renzora/engine/commit/5d583a3)
- Add batch property editing across multiple entities
- Add property undo/redo history

## Inspector — Views & Filtering
- Add lock inspector to entity (stop following selection)
- Add multi-entity comparison view
- Add component search/filter
- Add component presets/templates

## Asset Browser — Organization
- ✅ Add favorites/starred folders — [`4d96c63`](https://github.com/renzora/engine/commit/4d96c63)
- ✅ Add column sorting in list view — [`4d96c63`](https://github.com/renzora/engine/commit/4d96c63)
- Add recent files panel
- Add file tagging/categories
- Add advanced search (regex, type filter, size filter)

## Asset Browser — File Operations
- ✅ Add file move/cut/copy between folders — [`1cdac2a`](https://github.com/renzora/engine/commit/1cdac2a)
- ✅ Add drag-drop between folders — [`1cdac2a`](https://github.com/renzora/engine/commit/1cdac2a)
- Add batch rename files
- Add folder duplication

## Asset Browser — Preview & Metadata
- ✅ Add asset thumbnail generation system — [`b02b97b`](https://github.com/renzora/engine/commit/b02b97b)
- Add file properties panel
- Add file previewer/quick look panel
- Add split view (multiple folder views)

## Asset Pipeline — Import
- Add asset import settings persistence
- Add reimport-on-source-change for assets
- Implement asset streaming for large worlds

## Viewport — Selection Tools
- Add lasso selection tool
- Add circle/frustum selection

## Viewport — Layout & Preview
- Add split viewport (2-view, 4-view)
- Add camera preset snapshots (save/restore positions)
- Add measurement tools (distance, angle)
- Add material preview ball in viewport

## Viewport — Toolbar Controls
- Add local/global transform toggle in toolbar
- Add animation playback controls in viewport
- ✅ Add post-process preview toggles in toolbar — [`b373115`](https://github.com/renzora/engine/commit/b373115)
- Add depth of field preview toggle

## Menubar — Menus
- ✅ Add File menu (New, Open, Save, Save As, Recent, Exit) — [`ada1ff0`](https://github.com/renzora/engine/commit/ada1ff0)
- ✅ Add Edit menu (Undo, Redo, Cut, Copy, Paste, Select All, Deselect) — [`cc028c8`](https://github.com/renzora/engine/commit/cc028c8)
- ✅ Add Help menu (docs, version, shortcuts reference) — [`ada1ff0`](https://github.com/renzora/engine/commit/ada1ff0)
- ✅ Add command palette / quick search (Ctrl+P) — [`c811e2d`](https://github.com/renzora/engine/commit/c811e2d)
- Add View menu (zoom, fit all, isolation mode, viewport layouts)
- Add Entity menu (Create, Clone, Parent/Unparent)
- Add Tools menu (preferences, keybindings)
- Add Window/Workspace menu (manage panels, save/load layouts)

## Console — Input & Autocomplete
- Add autocomplete for commands
- Add command syntax highlighting
- Add command history search (Ctrl+R)
- Add command suggestions/hints

## Console — Output & Logging
- Add log export to file
- Add log colors per category
- Add regex search support
- ✅ Add REPL execution for scripts in console input bar — [`42e246d`](https://github.com/renzora/engine/commit/42e246d)

## Editor UX — Undo/Redo & Clipboard
- ✅ Add global undo/redo system across all editors — [`63fd98e`](https://github.com/renzora/engine/commit/63fd98e)
- ✅ Wire undo/redo to viewport toolbar and Edit menu — [`cc028c8`](https://github.com/renzora/engine/commit/cc028c8)
- Add global clipboard for entities/components/nodes

## Editor UX — Layouts & Help
- ✅ Add workspace layout save/load — [`6610d1a`](https://github.com/renzora/engine/commit/6610d1a)
- ✅ Add context menu on empty viewport — [`267bfa4`](https://github.com/renzora/engine/commit/267bfa4)
- Add F1 help integration
- Add in-editor help system
- Add macro recording and playback
- Add hot reload status indicator

## Gizmo — Transform Controls
- Add pivot point options (center, origin, bounds, median)
- Add local vs world space toggle
- Add snapping visualization during drag
- Add gizmo size scaling based on distance
- Add relative/absolute positioning number input
- Add gizmo reset/undo mid-interaction
- Add custom component gizmos (user-defined handles)

## Grid — Snapping & Layout
- Add different grid types (isometric, hexagonal)
- Add snap visualization during gizmo drag
- Add snap distance presets/profiles
- Add grid rotation (non-XZ planes)
- Add adaptive grid density (zoom-dependent)
- Add grid plane selection UI
- Add snap angle presets (15, 30, 45, 90 degrees)
- Add vertex/center snap modes
- Add snap to collider/mesh surfaces

## Scene Management — Multi-Scene
- ✅ Add multiple scenes open simultaneously in viewport — [`9bbf37b`](https://github.com/renzora/engine/commit/9bbf37b)
- Add scene merging/composition
- Add additive scene loading at runtime
- Add scene dependencies/references
- Implement scene transition system with loading screens

## Scene Management — Tools
- Add scene thumbnails in asset browser
- Add scene diff/comparison tool
- Add lock/protect scenes from editing
- Add scene-level undo/redo isolation

## Material Editor — Graph Editing
- Add graph undo/redo
- Add copy/paste nodes in graph
- Add node search/filter by name
- Add real-time preview updates while editing (not just on save)

## Material Editor — Advanced
- Add material instances with parameter overrides
- Add material presets/library
- Add material LOD/quality levels
- Add shader variant generation
- Add material domain completeness for Vegetation and Unlit

## Shader Editor — Code Intelligence
- Add inline error highlighting in code
- Add autocomplete for WGSL functions and uniforms
- Add real-time validation as you type
- Add include resolution error marking

## Shader Editor — Project & Preview
- Add multi-file shader project management
- Add shader variant system with conditional compilation
- Add preview for custom bind group materials
- Implement post-process preview in shader editor

## Code Editor — Navigation
- ✅ Add find/replace dialog — [`5d583a3`](https://github.com/renzora/engine/commit/5d583a3)
- Add find all with match highlighting
- Add go to line dialog
- Add go to definition
- Add minimap

## Code Editor — Editing Features
- Add multiple cursor support
- Add column selection
- Add code folding
- Add code snippets
- Add auto-format
- Add comment/uncomment shortcut
- Add bracket matching highlights
- Add split editing (side by side)
- Add file diff view

## Code Editor — Diagnostics
- Add inline error squiggles
- Add autocomplete/intellisense
- Add breakpoint markers in gutter

## Blueprint Editor — Graph UX
- Add comment boxes / annotation nodes
- Add reroute nodes for wire cleanup
- Add node groups / collapsible subgraphs
- Add node search within graph
- Add bookmarks for navigating large graphs
- Add graph validation warnings
- Add implicit type casting nodes

## Blueprint Editor — Extensibility
- Add user-defined custom nodes
- Add reusable subgraph macros/functions
- Add local variables within blueprint
- Add variable scope/lifetime management

## Blueprint Editor — Debugging
- Add breakpoints on nodes
- Add visual execution trace (see which nodes fire)
- Add step-through execution
- Add watch/variable inspection panel

## Blueprint Nodes — Events
- Handle remaining unhandled node types in blueprint compiler
- Wire blueprint delay node completion through timer system
- Implement blueprint on_timer event node
- Implement blueprint on_message event node
- Implement blueprint on_collision event node

## Lifecycle Editor — Nodes
- Add if/else condition branching nodes
- Add custom event response nodes
- Add entity spawn/despawn lifecycle nodes
- Add animation completion wait nodes
- Add input event nodes (key press, mouse, gamepad)
- Add audio control nodes (play, stop, fade)
- Add UI flow nodes (button press, screen transition)
- Add dialogue/cutscene sequencing nodes
- Add save/load game state nodes
- Add persistent variables (save to disk)
- Add struct/complex data pin types
- Add breakpoint/step-through debugging

## Scripting API — Queries & Physics
- Add raycast query API
- Add sphere cast API
- Add overlap test API
- Add shape query API
- Add entity iteration (query all entities)
- Add type-safe component queries

## Scripting API — Async & Communication
- Add coroutines/yield support
- Add async/await patterns
- Add script-to-script message passing
- Add event system integration for scripts
- Add networking API for scripts

## Scripting API — Utilities
- Add file I/O (read/write)
- Add script profiling hooks
- ✅ Verify and complete script hot-reload — [`593825f`](https://github.com/renzora/engine/commit/593825f)

## Animation Editor — Timeline
- Add keyframe click-to-edit in timeline
- Add bezier/spline curve editor
- Add keyframe interpolation modes (linear, bezier, stepped)
- Add animation events/notifies at specific frames

## Animation Editor — Blending & IK
- Add root motion extraction and handling
- Add additive blending UI setup
- Add IK support (inverse kinematics)
- Add animation retargeting between skeletons
- Add foot IK for uneven terrain

## Animation Editor — State Machine
- Build animation state machine visual editor
- Implement blend tree editor
- Implement animation transition/condition editor

## Animation Editor — Quality
- Add animation LOD quality variants
- Add animation compression settings per clip
- Add joint constraint editor

## Particle Editor — Curves & Color
- Add bezier curve editors for properties
- Add animated curves over lifetime
- Add dedicated gradient editor UI
- Add color picker integration in particle nodes

## Particle Editor — Features
- Add sub-emitters (child particles)
- Add trail rendering nodes
- Add GPU particle support
- Add particle collision events
- Add sprite sheet/animation per particle

## Particle Editor — UX
- Add node parameter inline editing
- Add property inspector for selected particle node
- Add undo/redo in particle graph
- Add simulation speed control
- Add particle count display
- Add performance profiling overlay

## Game UI — Text & Typography
- Add rich text (inline formatting, colors, sizes)
- Add multiple font support
- Add line-height/letter-spacing controls
- Add text truncation/ellipsis

## Game UI — Layout System
- Add layout constraints (aspect ratio, min/max size)
- Add responsive breakpoint system
- Add auto-layout (flex-like behavior)
- Add grid layout system
- Add safe area / notch handling for mobile

## Game UI — Interaction & Data
- Add reactive data binding system
- Add two-way data binding
- Add virtual list for large data sets
- Add focus management / tab traversal order
- Add keyboard navigation for UI
- Add scroll snap / pagination
- Add smooth scrolling with momentum

## Game UI — Systems
- Implement game UI keybind rebinding system
- Implement game UI settings row system
- Implement game UI inventory grid drag-drop system

## Terrain — Geometry
- Add terrain holes/cutouts
- Add terrain LOD for distant chunks
- Add multi-terrain support (multiple independent terrains)
- Implement terrain LOD

## Terrain — Painting
- Add slope-based painting (auto-paint by angle)
- Add real-time brush preview before applying
- Add tablet pressure sensitivity for brushes
- Complete terrain GPU splatmap painting pipeline
- Integrate material graph with terrain layers via material_path field

## Terrain — Import & Quality
- Add world machine / advanced heightmap import formats
- Add material LOD variants for terrain layers

## Physics — Backends
- Implement Rapier physics pause/unpause
- Remove Rapier backend if Avian is the chosen backend
- Complete Rapier physics backend
- Add collision layer/group editor UI

## Camera System
- Implement camera shake system
- Implement camera follow targets
- Implement camera blending between cameras
- Implement cinemachine-style camera system with dolly tracks

## Lighting — Editor
- Add point light editor support
- Add spot light editor support
- Add area light editor support
- Add shadow cascade settings
- Add shadow quality presets
- Add light linking/culling per object

## Lighting — Baked & Advanced
- Add light probes (baked)
- Add reflection probes (baked/real-time)
- Add light baking interface
- Add volumetric lighting
- Implement lightmapping / baked global illumination

## Water — Visual Effects
- Add caustics rendering
- Add direction-dependent foam patterns
- Add shore blending / beach detection
- Add animated normal maps
- Add water displacement mapping
- Add subsurface scattering control

## Water — Interaction
- Add underwater post-process integration
- Add refraction quality settings
- Add wake trails behind moving objects
- Add water flow/current vectors

## Post-Processing — Pipeline
- Add effect reordering UI
- Add effect blending/crossfading
- Add transition effects between states
- Add per-camera overrides
- Add effect chains/presets

## Post-Processing — Editor
- Add real-time effect preview window
- Add effect parameter keyframing
- Add undo/redo for effect changes
- Add effect performance warnings

## Rendering — LOD & Culling
- Implement LOD system for meshes
- Implement occlusion culling
- Implement decal system editor integration

## Rendering — Advanced
- Implement cloth simulation editor integration for bevy_silk
- Implement vegetation/foliage scattering system
- Implement destruction/fracture system
- Build unified weather system

## Networking — Script Commands
- Wire NetworkScriptCommand::SendEvent to actually send via Lightyear connection
- Wire NetworkScriptCommand::SpawnRequest to send message to server
- Wire NetworkScriptCommand::Rpc to send as GameEvent message
- Wire lifecycle send_message to Lightyear message API
- Wire lifecycle spawn_entity to send SpawnRequest to server

## Networking — Events & Multiplayer
- Implement lifecycle on_player_joined event handler
- Implement lifecycle on_player_left event handler
- Implement lifecycle on_message event handler
- Implement multiplayer lobby/matchmaking layer

## Navigation — Navmesh
- Add navmesh agent debug display
- Add navmesh editor visualization
- Add navmesh baking UI

## VR/XR
- Implement parabolic arc raycast for VR teleport
- Implement XR teleport parabolic arc raycast
- Add throw velocity on VR grab release (physics impulse)
- Fix VR grab release not applying throw velocity

## Audio
- Build DAW audio arrangement timeline
- Integrate 3D spatial audio with HRTF and distance attenuation

## AI Systems
- Build behavior tree AI system
- Build GOAP AI system

## Gameplay Systems
- Build dialogue system
- Build quest/objective system
- Implement game save/load system with player state
- Implement runtime localization system for games
- Implement runtime input remapping system
- Build runtime developer console for shipped games

## Export Pipeline — Signing & Certificates
- Add custom certificate/keystore code signing
- Add Windows executable signing to CI
- Add iOS provisioning profile & team ID configuration
- Add Android certificate management UI

## Export Pipeline — Build Configuration
- Add splash screen configuration for exported games
- Add version number / build number management
- Add release vs debug build profile selection
- Add per-platform build settings/flags
- Add asset compression profile options
- Add binary obfuscation options

## CI/CD Pipeline
- ✅ Add iOS export target to CI — [`765f0f4`](https://github.com/renzora/engine/commit/765f0f4)
- ✅ Add tvOS export target to CI — [`765f0f4`](https://github.com/renzora/engine/commit/765f0f4)
- Add cargo test step to CI pipeline
- Add nightly/preview builds to CI

## WASM Support
- Add WASM auth support
- Add WASM networking via WebSocket/WebTransport transport
- Add WASM audio backend alternative to Kira

## Plugin Host — Core
- ✅ Implement plugin management UI in Settings panel — [`4ef4b2a`](https://github.com/renzora/engine/commit/4ef4b2a)
- Add hot reload on code change (not just creation/removal)
- Add plugin settings UI
- Add plugin enable/disable per project
- Add plugin configuration persistence

## Plugin Host — Safety & Dependencies
- Add version compatibility enforcement
- Add dependency resolution at runtime
- Add plugin update mechanism
- Add plugin sandbox/permissions
- Add plugin crash isolation

## Hub / Asset Store — Publishing
- Add upload assets to marketplace
- Add create/edit asset listings
- Add asset dependency declaration
- Add license/EULA display

## Hub / Asset Store — Management
- Add version management (installed vs latest)
- Add automatic asset updates
- Add asset versioning/downgrade
- Add dependency resolution & conflict detection
- Add uninstall/cleanup assets
- Add asset bundle management
- Verify and complete asset store backend API

## Debugger / Profiler — Performance
- Add frame timeline with scrubber
- Add detailed GPU profiler (per shader/material timing)
- Add CPU/GPU utilization meters
- Add draw call analyzer
- Add frame timeline profiler
- Add memory usage tracker
- Add CPU/GPU flame graph

## Debugger / Profiler — Inspection
- Add memory leak detection tools
- Add entity inspector with tree drill-down
- Add system ordering graph view
- Add network traffic inspector
- Add asset streaming profiler

## Bevy API Adoption
- Adopt Bevy States trait instead of custom PlayState enum
- Adopt Bevy ComputedStates/SubStates
- Adopt Bevy Required Components
- Adopt Bevy One-Shot Systems where applicable
- Use Bevy Run Conditions more systematically
- Adopt Bevy ECS Observers/Hooks
- Evaluate adopting BSN scene notation
- Leverage Bevy async compute for heavy operations
- Adopt Bevy Asset LoadingState plugin for streaming feedback

## Version Control & Collaboration
- Implement version control integration
- Implement collaborative editing

## Small Bugs / Polish
- Ensure all nav mesh collider types are handled (segment, triangle, polyline, halfspace, custom, voxels currently warn)
- Fix dimmed undo/redo buttons throughout editor (no-op)
- Handle set_menu_item_checked in plugin host

## Documentation
- Generate rustdoc API documentation

## Code Quality — Error Handling
- Audit and replace 715 .unwrap() calls with proper error handling
- Audit and replace 79 .expect() calls with graceful fallbacks
- Remove or implement 53 panic!/todo!/unimplemented! in non-third-party code
- Audit 4 unsafe blocks and add safety documentation

## Code Quality — Architecture
- Tighten module visibility (1285 pub vs 159 pub(crate))
- Add structured logging with enforced log levels
- Add serialization version migration system for scene files
- Split large match statements in blueprint interpreter and material compiler

## Code Quality — Performance
- Add rate limiting/debounce on file watchers and network polls
- Add thread pool / concurrency limits for spawned tasks
- Pre-allocate Vec/HashMap capacity in hot paths
- Reduce excessive .clone() in component data paths

## Code Quality — Runtime Safety
- Add comprehensive entity/resource cleanup verification on play mode exit
- Add scene size bounds check before serialization

## Unit Tests — Core Systems
- Add unit tests for renzora_core (project config, play state, entity tags)
- Add unit tests for renzora_scene (save/load roundtrip, camera serialization)
- Add unit tests for renzora_input (input map loading, action state)
- Add unit tests for renzora_audio (command queue, mixer routing)
- Add unit tests for renzora_keybindings (action binding, modifier handling)
- Add unit tests for renzora_settings (settings persistence, input map)

## Unit Tests — Visual Systems
- Add unit tests for renzora_material (codegen, graph, resolver, node types)
- Add unit tests for renzora_shader (backend registry, shader file parsing, param extraction)
- Add unit tests for renzora_postprocess (effect registration, pipeline ordering)
- Add unit tests for renzora_lighting (azimuth/elevation to direction math)
- Add unit tests for renzora_rt (quality presets, settings)
- Add unit tests for renzora_hanabi (particle data, effect builder)
- Add unit tests for renzora_theme (TOML loading, color serialization)

## Unit Tests — Gameplay Systems
- Add unit tests for renzora_blueprint (interpreter, compiler, graph, node definitions)
- Add unit tests for renzora_scripting (command system, context, Lua backend, Rhai backend)
- Add unit tests for renzora_lifecycle (interpreter, graph loading, variable storage)
- Add unit tests for renzora_animation (state machine transitions, blend trees, tween easing)
- Add unit tests for renzora_physics (Avian backend, character controller, collision shapes)
- Add unit tests for renzora_network (protocol, messages, prediction, client/server setup)
- Add unit tests for renzora_gauges (attribute modifiers, expression evaluation)

## Unit Tests — World Systems
- Add unit tests for renzora_terrain (heightmap, splatmap, brush operations, undo stack)
- Add unit tests for renzora_water (Gerstner wave math, buoyancy calculation)
- Add unit tests for renzora_game_ui (widget spawning, tween system, theme application, canvas scaling)

## Unit Tests — Editor & Tools
- Add unit tests for renzora_ui (dock layout, panel registry, widget rendering)
- Add unit tests for renzora_editor (selection system, inspector registry)
- Add unit tests for renzora_hierarchy (entity tree building, drag-drop reorder)
- Add unit tests for renzora_asset_browser (directory scanning, thumbnail cache)
- Add unit tests for renzora_console (log filtering, command history)
- Add unit tests for renzora_splash (project config persistence)
- Add unit tests for renzora_stinger (state transitions)

## Unit Tests — Pipeline & Packaging
- Add unit tests for renzora_rpak (pack, read, archive roundtrip)
- Add unit tests for renzora_import (OBJ/STL/PLY/FBX conversion, mesh optimization)
- Add unit tests for renzora_export (template management, rpak packing, APK signing)
- Add unit tests for plugin host (plugin loading, ABI validation, dependency graph)
- Add unit tests for plugin API (FFI boundary, event dispatch)

## Integration Tests
- Add integration tests for scene save/load roundtrip (save scene, reload, compare)
- Add integration tests for material graph compile (build graph, compile WGSL, verify output)
- Add integration tests for blueprint interpret (build graph, run interpreter, check commands)
- Add integration tests for script execution (load script, run update, verify side effects)
- Add integration tests for terrain sculpt/paint (apply brush, verify heightmap/splatmap)
- Add integration tests for animation playback (load clip, advance time, check transforms)
- Add integration tests for physics simulation (spawn bodies, step, check positions)
- Add integration tests for asset import pipeline (import OBJ/FBX, verify GLB output)
- Add integration tests for rpak archive (pack project, read back, verify files)
- Add integration tests for export pipeline (export project, verify output structure)
- Add integration tests for play mode enter/exit (enter play, exit, verify cleanup)
- Add integration tests for lifecycle graph (load lifecycle, simulate boot, verify scene load)
- Add integration tests for network protocol (client connect, send message, verify delivery)
- Add integration tests for game UI spawning (spawn canvas with widgets, verify entity hierarchy)

## Test Infrastructure & CI
- Add cargo test step to CI pipeline
- Add test coverage reporting (tarpaulin or llvm-cov)
- Add regression test harness for editor panels (smoke test each panel loads)

## Benchmarks
- Add benchmark suite for material compilation
- Add benchmark suite for blueprint interpretation
- Add benchmark suite for terrain brush operations
- Add benchmark suite for scene serialization/deserialization
- Add benchmark suite for physics step performance
- Add benchmark suite for particle system update
- Add benchmark suite for animation blending
- Add benchmark suite for script execution throughput

## Fuzz & Property Tests
- Add fuzz tests for scene file deserialization
- Add fuzz tests for rpak archive parsing
- Add fuzz tests for material graph loading
- Add fuzz tests for blueprint graph loading
- Add property-based tests for math utilities (vec3, lerp, clamp)
- Add property-based tests for serialization roundtrips

## Snapshot Tests
- Add snapshot tests for WGSL codegen output
- Add snapshot tests for blueprint Lua codegen output
