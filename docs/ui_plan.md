# Renzora UI Plan

Status of the markup-driven UI system — now living in `renzora_ember` — and the road to **Cinder**, the first UI-layer particle system in the Bevy ecosystem.

> **Status as of 2026-06.** Shipped: the markup runtime (ember's own entity-tree
> loader on the `bevy_hui` parser), the scripting bridge, vector widgets on WGSL
> `UiMaterial`s, editor integration, and inspector → `.html` write-back. On the
> roadmap (not yet shipped): closing the write-back hot-reload round-trip, a
> small animated-shader UI library, and **Cinder**, the UI particle system, which
> is unstarted future work.

**Legend:** ✅ shipped · 🔜 planned · ❓ open decision · 🧪 needs in-editor verification

> ⚠️ **Naming note.** The old `renzora_hui` crate no longer exists. It was merged
> into `renzora_ember` in three stages and **deleted**. The planned rename to
> `renzora_markup` never happened — everything folded into `renzora_ember`
> instead. Wherever you see `renzora_hui` / `HuiPlugin` in older docs, read
> `renzora_ember::markup` / `MarkupPlugin`. (The still-vendored parser fork at
> `crates/bevy_hui` is a different thing and is unaffected.)

---

## 1. Vision

Author game UI as **hot-reloadable markup** (à la Unity's UI Toolkit), drive it
from **scripts**, position it in the **editor** like any other object, and make
it **immersive** with shaders and particles — a combination no engine cleanly
offers. Unity has markup (UXML/USS); Godot has visual node UI; neither blends
"author in markup" with "drag the result on a canvas," and none put particles in
the UI layer. Renzora does all of it.

The engine is **Bevy 0.18**, and the entire UI stack is **`bevy_ui`-native** —
`egui`/`bevy_egui` and `vello`/`bevy_vello` have both been fully removed.

---

## 2. Crate & plugin map ✅

The whole UI layer is two crates plus a vendored parser:

| Crate | Role | Key plugins / types |
|---|---|---|
| `renzora_ember` | The unified UI framework: markup runtime, ~80 native widgets, docking, theming, reactive bindings. Used by editor and games alike. | `EmberPlugin`, `markup::MarkupPlugin` |
| `renzora_ember/editor` (package `renzora_ember_editor`) | Editor-only markup integration: entity preset, hierarchy icons, template-path inspector, bevy_ui style-component inspectors with write-back. | `HuiEditorBundlePlugin`, `HuiEditorPlugin`, `HuiInspectorPlugin` |
| `renzora_game_ui` | `bevy_ui` widgets, the game-UI canvas, the SDF shape widgets, and the shared data types markup roots use. | `UiCanvas`, `UiWidget`, `HtmlTemplatePath`, `UiTheme`, `shapes::*` |
| `crates/bevy_hui` (vendored fork) | **Parser only** — `.html` → typed AST + the `.html` asset loader. None of its runtime is used. | `LoaderPlugin`, `HtmlTemplate` |

Two ember plugins matter:

- **`renzora_ember::markup::MarkupPlugin`** (formerly `HuiPlugin`) — the markup
  runtime. It self-registers via `renzora::add!(MarkupPlugin)` at **Runtime**
  scope, so it runs in the editor viewport **and** in exported games.
- **`renzora_ember::EmberPlugin`** — the general widget/theme/dock framework. It
  bundles `style::ThemePlugin` + `dock::DockPlugin` + `widgets::WidgetsPlugin` +
  `reactive::ReactivePlugin`.

The editor half registers via `renzora::add!(HuiEditorBundlePlugin, Editor)` and
is linked only by the editor bundle.

> The editor subcrate's **package** was renamed `renzora_hui_editor` →
> `renzora_ember_editor`, but its **plugin type names** are still
> `HuiEditorPlugin` / `HuiInspectorPlugin` / `HuiEditorBundlePlugin`. That is
> intentional; don't "fix" them in docs.

**Dependency rule that shapes everything:** `renzora_ember` depends on
`renzora_game_ui` (directly — there is no longer a `renzora_hui` link in
between), so `renzora_game_ui` can **not** depend back on ember. The shared data
types markup roots need — `HtmlTemplatePath`, `UiCanvas`, `UiWidget` — therefore
live in `renzora_game_ui`; `renzora_ember::markup` owns only the loader/runtime
behavior.

---

## 3. Architecture at a glance

```text
assets/ui/*.html
      │ author
      ▼
bevy_hui LoaderPlugin ── parse ──▶ HtmlTemplate (asset, typed AST)
      │
      │ HtmlTemplatePath inserted on an entity (UiCanvas, viewport drop, hui_spawn)
      ▼
renzora_ember::markup::loader ── walk AST ──▶ one real bevy_ui entity per node
      │                                        (Node / Text / TextFont / TextColor /
      │                                         BackgroundColor / BorderColor, …)
      ▼
  events (on_press=…) ─▶ bevy_hui OnUiPress family ─▶ renzora::ScriptUiInbox
                                                       ─▶ Lua on_ui(name, args, entity)
  vector="…"          ─▶ ember WGSL UiMaterial (ArcMaterial / ChartMaterial / WaveMaterial)
```

The defining change from the original plan: **markup is an entity tree, not an
opaque builder.** Every `<node>`/`<text>`/`<image>`/`<button>` becomes a real
`bevy_ui` entity with standard components attached directly. There is **no
`HtmlNode`, no `HtmlStyle`, no per-frame style re-assertion, and no scope-root
wrapper** — the components hold the truth and `bevy_ui` lays out and renders them
like any other UI.

---

## 4. Markup runtime — `bevy_hui` parser + ember loader ✅

`MarkupPlugin` adds **only** `bevy_hui::prelude::LoaderPlugin`, which registers
`HtmlTemplate` as an asset and the `.html` `AssetLoader`. None of bevy_hui's own
runtime — `BuildPlugin`, `TransitionPlugin`, `CompilePlugin`,
`HtmlAutoLoadPlugin`, `FunctionBindings`, `HtmlComponents` — is registered.
Everything downstream of the AST is ember's `loader.rs`.

### Authoring format

UI is authored as hot-reloadable **`.html`** files under `assets/ui/`.

- `{single-brace}` = **build-time** property substitution (resolved once, from
  `<property>` defaults and `template="..."` overrides).
- `{{ double-brace }}` = **per-frame reactive binding**, re-resolved every frame
  by reflection against live ECS components.

```html
<!-- assets/ui/hud.html -->
<template>
    <property name="accent">#39C5FF</property>
    <node position="absolute" left="24px" top="24px" width="280px"
          flex_direction="column" row_gap="8px" padding="12px"
          background="#11151Cdd" border_radius="10px">
        <text font_size="14" font_color="{accent}">HP {{ Health.Health.current }}</text>
        <node height="10px" background="#222" border_radius="5px">
            <node fill="Health.Health.current" fill_min="0" fill_max="100"
                  height="100%" background="{accent}" border_radius="5px" />
        </node>
        <button on_press="open_menu" padding="8px 14px" background="#1d2530"
                hover:background="#26303d">Menu</button>
    </node>
</template>
```

### Element set

The parser exposes seven built-in `NodeType`s; everything else is `Custom(name)`.

| Tag | Meaning |
|---|---|
| `<node>` | Generic flex/grid box. |
| `<text>` | Text content (`{prop}` and `{{ binding }}` allowed). |
| `<image>` | `src=` image; always gets a `MarkupImage` marker for the editor slot. |
| `<button>` | A `<node>` that also carries `bevy_ui::Button` for `Interaction`. |
| `<slot/>` | Re-parents the caller's children into a component template (React/Vue-style). |
| `<template>` | The file's root wrapper. |
| `<property>` | Declares a build-time `{prop}` default. |

The loader additionally recognizes four custom tags:

| Tag | Attributes | Behavior |
|---|---|---|
| `<input>` | `bind`, `placeholder`, `password` | Focusable text field (`TextInput` + `Button`). |
| `<icon>` | `name`, `size`/`font_size`, `font_color` | Phosphor glyph rendered in the icon font. |
| `<for tag="...">` | `tag` | Repeats its children once per entity carrying that `EntityTag`. |
| `<node template="path.html">` | unknown attrs become `{prop}` overrides | Expands another template onto this entity (component composition). |

> ⚠️ **The file-stem custom-tag registry is gone.** A bare `<custom_tag>` with no
> `template=` no longer resolves to `assets/ui/components/custom_tag.html`. It now
> emits a warning (`<custom_tag> is not a built-in element — use <node
> template="path/to/custom_tag.html"> instead`) and renders nothing. Components
> must be referenced by **explicit path** via `<node template="...">`.

```html
<!-- panel.html — a reusable component with a slot -->
<template>
    <property name="title">Panel</property>
    <node padding="16px" background="#11151C" border_radius="10px">
        <text>{title}</text>
        <slot/>
    </node>
</template>

<!-- using it: explicit path, not <panel ...> -->
<node template="ui/panel.html" title="Vitals">
    <node template="ui/stat_bar.html" label="HP" fill="72%"/>
    <node template="ui/stat_bar.html" label="MP" fill="40%"/>
</node>
```

### Layout attributes

Statically-applied styles map straight onto `bevy_ui::Node` and its color
slots: `display`, `position`, `left`/`right`/`top`/`bottom`,
`width`/`height`, `min_width`/`max_width`/`min_height`/`max_height`,
`aspect_ratio`, `margin`, `padding`, `border`/`border_color`/`border_radius`,
`background`, `flex_direction`/`flex_wrap`/`flex_grow`/`flex_shrink`/`flex_basis`,
`row_gap`/`column_gap`,
`align_items`/`justify_items`/`align_self`/`justify_self`/`align_content`/`justify_content`,
the `grid_*` family, `font_size`, and `font_color`. `hover:` / `pressed:`
background/border overrides plus `delay`/`duration` become an `Interactive`
transition.

> `{{ }}` runtime bindings work **only** in text content and in `show=`, plus the
> vector `value=`/`data=`/`readout=` attributes. Ordinary style attributes are
> computed **once** at build time — there is no `{{ }}` in arbitrary style
> attributes.

### Bindings & control flow

| Form | Resolves to |
|---|---|
| `{{ Component.field }}` | a field on the host entity's component (walks up `ChildOf`). |
| `{{ Entity.Component.field }}` | a field on the entity with that `Name`. |
| `{{ _scriptVar }}` | a script variable read back from the host's `ScriptComponent`. |
| `{{ Name }}` | the host entity's `Name`. |
| `show="{{ cond }}"` | conditional `Display::None`; supports `and`/`or`/`not`, `< > <= >= == !=`, parentheses, quoted strings. |

### Interaction kernel

The markup widget **kernel** (`markup/widgets.rs`) is a small set of
attribute-driven behaviors. Targets are **plain reflection paths**, not `{{ }}`.
Writes route through `renzora_scripting`'s tested paths — component fields go to
`ScriptReflectionQueue`, script vars are written on the entity's
`ScriptComponent`.

| Attribute | Component | Effect |
|---|---|---|
| `toggle="Path.bool"` | `Toggle` | Click flips a bound boolean (checkbox/switch). |
| `drag_value="Path.num" drag_min drag_max` | `DragValue` | Drag horizontally to set a number (slider/scrollbar). |
| `fill="Path.num" fill_min fill_max` | `ValueFill` | Node width tracks the value's fraction of the range (progress/bar fill). |
| `toggles="name"` | `Disclose` | Click shows/hides the entity with that `Name` (dropdown/accordion/modal/tabs). |

Plus drag-and-drop and decoration attributes the loader stamps directly:
`draggable`, `drag_item`, `dropzone`/`drop_tag`/`on_drop`, `cursor=`,
`gradient=` and `shadow=` (native `bevy_ui` decoration), and the special
`name="cursor_follow"` for cursor tracking.

### Events → `on_ui` ✅

`on_press` / `on_enter` / `on_exit` / `on_spawn` / `on_change` are parsed into
bevy_hui's **`OnUiPress` / `OnUiEnter` / `OnUiExit` / `OnUiSpawn` / `OnUiChange`**
components. `interactions.rs` watches `Changed<Interaction>` on those entities and
pushes a `renzora::UiCallback` into `renzora::ScriptUiInbox`; `renzora_scripting`
drains it each frame and calls every script's `on_ui` hook.

```html
<button on_press="start_game">Play</button>
```

> There is **no `MarkupOnPress` component**, and no `MarkupId` / `MarkupClass` /
> `class=` handling. `id=`/`name=` simply set the entity `Name` (an `id` is shown
> as `#id` in the hierarchy). The callback's third argument is the firing node's
> `Entity::to_bits()` as a **u64 integer**, not an entity handle.

---

## 5. Vector widgets — WGSL `UiMaterial`s ✅

`vector="..."` widgets used to be drawn with an external vector-graphics crate
(`vello`). **`vello` was dropped.** They now render with ember's own WGSL
`UiMaterial`s as ordinary `bevy_ui` `MaterialNode`s, with `bevy_text` children for
labels and readouts — no `Camera2d`, no `VelloView`, no `UiVelloScene`, no
`RenderLayers` plumbing.

| `vector=` value (aliases) | Material / shader | Notes |
|---|---|---|
| `arc` (`gauge`, `ring`) | `ArcMaterial` (`gauge.wgsl`) | Stroked track + value fill; optional centred `readout`. |
| `bars` (`bar`) | plain `bevy_ui` rects | One rectangle per datum. |
| `line` (`chart`) | `ChartMaterial` (`chart.wgsl`) | Cartesian series (≤32 samples). |
| `wave` (`waveform`) | `WaveMaterial` (`waveform.wgsl`) | Cartesian series (≤32 samples). |
| `speedometer` (`dial`) | `ArcMaterial` + `bevy_text` | Composite: arc + numeric labels + needle + centre readout. |

```html
<node vector="speedometer" value="{{ Vehicle.speed }}" min="0" max="240"
      color="#39C5FF" unit="km/h" readout="{{ Vehicle.speed }}"
      width="180px" height="180px" />
```

Common attributes: `value` (literal or `{{ path }}`), `data` (comma string,
literal or `{{ path }}`, for `bars`/`line`/`wave`), `min`/`max`, `color`,
`track`, `fill`, `thickness`, `count`, `start` (deg), `sweep` (deg), `inset`
(px), `len`, `size`, `readout`, `unit`, `readsize`.

> The standalone `ticks` / `labels` / `needle` primitives no longer exist — they
> are baked into the `speedometer` composite. The `renzora_gauges` crate was
> removed entirely; gauge drawing is now ember's `arc`/`gauge` widget
> (`ArcMaterial`).

`renzora_game_ui::shapes` also still ships its own SDF `UiMaterial` shape widgets
(`circle`, `arc`, `radial_progress`, `wedge`, `polygon`, `triangle`,
`rectangle`, `line`) — proof that shader-driven UI is a working pattern, not new
tech.

> The gauge/chart/waveform material plugins are `is_plugin_added`-guarded because
> both `WidgetsPlugin` and `markup::vector::plugin` register them.

---

## 6. Scripting bridge ✅

Markup events broadcast to **every script's `on_ui`** hook, the same way
`on_rpc` broadcasts network RPCs:

```lua
-- attached to any entity in the scene
function on_ui(name, args, entity)
    if name == "start_game" then
        action("hui_spawn", { template = "ui/hud.html" })
    elseif name == "open_menu" then
        action("hui_show", { name = "main_menu_root" })
    end
end
```

> `on_ui` is **Lua-only**. The Rhai backend supports only `props`, `on_ready`, and
> `on_update`; it never receives `on_ui` (nor `on_rpc`/`on_http`/`on_player_*`).
> Use `.lua` for any script that reacts to UI events.

Scripts also spawn and toggle markup through the `action()` escape hatch
(handled by `markup/lua_bridge.rs`). The handler verbs are still named `hui_*`:

| Action | Effect |
|---|---|
| `action("hui_spawn", { template = "ui/x.html" })` | Spawns a `UiCanvas` root carrying `HtmlTemplatePath`; the loader builds the tree under it. |
| `action("hui_despawn", { template = "ui/x.html" })` / `{ name = "..." }` | Despawns matching template roots, or the named entity. |
| `action("hui_hide", { name = "..." })` / `action("hui_show", { name = "..." })` | Toggles `Visibility` on the named entity. |

---

## 7. Editor integration & write-back ✅ 🧪

All editor-only, in the `renzora_ember_editor` subcrate (the
`HuiEditorBundlePlugin`, linked by the editor bundle). It compiles and registers;
the *visual* behavior still wants in-editor verification.

- **Create:** "+ Add Entity → UI" presets, asset-panel "Create → HTML Template",
  and an inspector **Template** field that is a `.html` asset slot.
- **Place:** drag a `.html` from the asset panel onto the viewport or onto the
  game-UI canvas; both insert `HtmlTemplatePath` on a host entity and let the
  loader build the tree.
- **Select:** the loader stamps `renzora_game_ui::UiWidget` on **every** built
  node, so the canvas hit-tests down to the deepest visible markup element —
  clicking a `<text>` inside a `<panel>` selects the text.
- **Write-back:** each built node carries a `MarkupSource { template_handle,
  node_path }` (`markup/provenance.rs`). Inspector edits (and drag/resize of
  `left/top/width/height`) patch the `.html` file via span-tracked surgical
  string edits (`markup/writeback.rs::write_attr_to_markup`), preserving comments
  and formatting.

> ⚠️ **The round-trip is not closed yet.** Write-back patches the file, but
> hot-reload-on-`Modified` respawn of the rendered tree is **not implemented** —
> editing the `.html` on disk does not yet automatically rebuild the live nodes.
> Closing this loop is the next editor task.
>
> Note also that `renzora_game_ui_editor`'s native `ui_canvas` panel is WIP and
> **not yet registered**; the legacy canvas panel is still the active one.

---

## 8. Theming ✅

Two theme layers, both data-driven and repaint-on-change:

| Layer | Type | Source | Applies to |
|---|---|---|---|
| Editor / ember widgets | `renzora_ember::style::Theme` (per-`Role` `StyleToken`s, built on `theme::Palette`) | project `themes/*.toml` | `Styled` components via `apply_theme` |
| Game UI | `renzora_game_ui::UiTheme` (semantic tokens) | project config | `UiThemed`-marked entities |

The standalone `renzora_theme` (`ThemeColor`, `ThemeManager`, TOML loader) and
`renzora_theme_status` crates still exist; their egui pieces were removed in the
egui → bevy_ui migration.

> Because the markup loader spawns plain `bevy_ui` entities, themed colors and
> per-node markup colors coexist — markup sets explicit colors, `apply_theme`
> repaints `Styled` widgets.

---

## 9. Cinder — UI particle system 🔜 (flagship, unstarted)

> **Status: unstarted future work.** No `cinder` module exists yet. The intent
> (recorded in `renzora_ember`'s crate docs and `Cargo.toml`) is to migrate it in
> **as part of `renzora_ember`** — i.e. `renzora_ember::cinder`, not a separate
> `renzora_cinder` crate. This section is a design sketch, not shipped code.

**The gap:** every Bevy particle crate (`bevy_hanabi`, `bevy_enoki`,
`bevy_particle_systems`) renders in world/camera space. **None** composite with
the `bevy_ui` layer. Cinder would be the first UI-space particle system for
Bevy — sparks off a health bar, embers behind a menu, a burst on level-up,
correctly layered with UI and shipping in exports.

### Architecture: pooled CPU particles **as UI nodes**

Fits the entity-tree model exactly. An **emitter** is a UI node; each
**particle** is a small child UI node (colored quad → later `UiMaterial`-shaded
or sprite) advanced each frame and recycled. Chosen over render-to-texture /
overlay-camera because it:

- **composes natively** — real UI nodes layer with other UI, respect the canvas,
  scale with `UiScale`, and ship in exports with zero pipeline work;
- is **right-sized** — UI FX want tens–hundreds of particles, not GPU millions;
- is **verifiable without a GPU** — the sim is plain ECS logic;
- **upgrades cleanly** — swap quads for `UiMaterial`/sprites or a GPU-instanced
  fast path later without changing the authoring API.

### Sketch

- `CinderEmitter { rate, burst, shape, lifetime, speed, spread, gravity,
  max_particles, looping }` + over-life curves (`color`/`size`/`opacity`/
  `rotation`/`velocity_damping`).
- `CinderParticle { age, lifetime, velocity, seed }` (pooled; hidden when dead).
- **Markup:** `<emitter rate="20" lifetime="0.8s" gravity="0 400" color="#FF9...">`
  via the `<node template>` / custom-attribute path.
- **Script:** `burst("HealthBar", 30)` / `emit_on("X")` through the `action()`
  bridge.

---

## 10. Roadmap (suggested order)

1. **Close the write-back round-trip** (§7) — hot-reload the rendered tree on
   `.html` `Modified`, so editor edits and disk edits both rebuild live.
2. **One animated shader UI material** — a glowing/pulsing health bar fed `time`
   + `value` uniforms, exposed as a `vector=`-style widget. Visible "immersive UI"
   on the existing `UiMaterial` plumbing.
3. **Cinder vertical slice** (§9) — crate-internal `cinder` module + emitter +
   particle + sim + a demo. De-risk the novel piece early.
4. Iterate Cinder (curves/shapes/pooling → shaded particles) and grow the
   shader-effect library.

---

## 11. Verification & constraints

- Agent work is compile-checked (`cargo check`) and gated by CI
  (`cargo test --workspace`, which runs the markup parse tests).
- Editor/visual behavior is verified by running the editor; new visual features
  ship as **compile-clean vertical slices** the user runs and tunes from
  screenshots, rather than large unverified drops.

---

## 12. Naming / lore

`bevy_hanabi` = fireworks · `bevy_enoki` = mushroom · `bevy_hui` = markup …
**`cinder`** = the glowing bits that fly off fire. Markup builds the bar; cinder
throws sparks off it — a one-word identity for "the first UI particle system for
Bevy." It will live inside `renzora_ember`, alongside the markup runtime and the
widget library, so the whole UI story ships from one crate.
