# Renzora UI Plan

Status of the markup-driven UI system and the road to **Cinder** — the first
UI-layer particle system in the Bevy ecosystem.

**Legend:** ✅ shipped · 🔜 planned · ❓ open decision · 🧪 needs live-editor verification

---

## 1. Vision

Author game UI as **hot-reloadable markup** (à la Unity's UI Toolkit), drive it
from **Lua**, position it in the **editor** like any other object, and make it
**immersive** with shaders and particles — a combination no engine cleanly
offers. Unity has markup (UXML/USS); Godot has visual node UI; neither blends
"author in markup" with "drag the result on a canvas," and none put particles in
the UI layer. Renzora does all of it.

Building blocks:
- **`bevy_hui` 0.6** (matches workspace Bevy 0.18) — pseudo-HTML/XML templates.
- **`renzora_hui`** — engine wrapper: Lua bridge, editor integration, template binding.
- **`renzora_game_ui`** — bevy_ui widgets, the canvas editor, `UiMaterial` SDF shapes.
- **`renzora_cinder`** 🔜 — UI-space particle system (new, flagship).

---

## 2. Architecture at a glance

```
assets/ui/*.html  ──author──▶  bevy_hui templates
        │                          │ build
        ▼                          ▼
HtmlTemplatePath (renzora_game_ui)  HtmlNode (child) ──▶ bevy_ui node tree
        │ observer (renzora_hui)                         (flex/grid layout)
        ▼
   markup callbacks (on_press=…) ──▶ Lua on_ui(name,args,entity)   [renzora_hui → renzora core → renzora_scripting]

Editor:  + Add Entity / drag-drop / canvas drop  ──▶ spawn_html_template_at  ──▶ draggable instance
Render:  bevy_ui pass  +  UiMaterial shaders (shapes today, effects 🔜)  +  Cinder UI particles 🔜
```

**Dependency rule that shapes everything:** `renzora_hui` depends on
`renzora_game_ui`, so `game_ui` (and `renzora_viewport`) can **not** depend on
`renzora_hui`. Shared data types (e.g. `HtmlTemplatePath`) therefore live in
`game_ui`; `renzora_hui` owns only the bevy_hui-specific behavior.

---

## 3. Markup UI — `bevy_hui` integration ✅

- `HuiPlugin` registers `bevy_hui::HuiPlugin` + `HuiAutoLoadPlugin(["ui/components"])`.
  Templates live in `assets/ui/*.html`; files under `assets/ui/components/`
  auto-register as custom tags by file stem (`menu_button.html` → `<menu_button>`).
- Self-registers via `renzora::add!(HuiPlugin)` and is linked into `renzora_runtime`.
- Demo templates: `health_bar`, `speedometer`, `scoreboard`, `inventory`, `hud`
  + reusable `stat_bar`, `item_slot` components.
- A CI parse test (`crates/renzora_hui/tests/parse_templates.rs`) parses every
  `assets/ui/**/*.html` so markup errors fail CI (runs on Linux; can't link
  locally on Windows due to the `renzora` dylib symbol cap).

### bevy_hui capability reference (it's a CSS *subset*, not an engine)
- **Has:** flexbox + CSS grid, box model (size/min/max, margin, padding, border,
  `border_radius`, outline), colors w/ alpha, box-shadow + text-shadow,
  9-slice/tiled images, `hover:`/`pressed:`/`active:` states, eased transitions
  (`delay` + `ease`) = CSS `transition`, sprite-atlas flipbook animation,
  `<property>` + `{var}` substitution.
- **Lacks:** `transform`/rotate/scale, `@keyframes`, `::before`/`::after`,
  gradients, filters/blur, `calc()`/media queries, an `opacity` property.
- **Implication:** layout/structure/hover come from markup; motion, gauges,
  rotation, gradients, and FX come from renzora (`UiMaterial` shaders, shape
  widgets, Cinder, or script-driven uniforms).

---

## 4. Scripting bridge ✅

- Markup callbacks (`on_press`/`on_change`/`on_spawn`) with no Rust binding fall
  through to every script's `on_ui(name, args, entity)` hook — broadcast, like
  `on_rpc`. `tag:`-prefixed attributes arrive as `args`.
- Implementation mirrors the RPC path: `renzora::ScriptUiInbox` + `UiCallback`
  (core) → `ScriptBackend::call_on_ui` drained in `execution.rs` → Lua `on_ui`.
- Fallback forwarders are registered into bevy_hui's `FunctionBindings` so Rust
  bindings keep precedence and there are no "function not bound" warnings.
- `action("hui_spawn", { template = "ui/x.html" })` spawns a template from script.

---

## 5. Editor integration ✅ 🧪

All editor-only, behind `renzora_hui`'s `editor` feature (wired into the runtime
editor cascade). Everything compiles; the *visual* behavior needs in-editor checks.

- **Create:** "+ Add Entity → UI → HTML Template" (`EntityPreset`); right-click
  asset panel → "Create → HTML Template" (starter file); inspector "Add Component".
- **Edit:** double-click a `.html` → opens in the code editor; inspector
  **Template** field is a `.html` asset slot (drag-drop or pick); changing it
  rebuilds the markup live.
- **Place:** drag a `.html` from the asset panel onto the viewport (3D/2D →
  `renzora_viewport::html_drop`) or onto the canvas in UI mode
  (`renzora_game_ui::canvas`). Both route through `spawn_html_template_at`.
- **Export:** `.html` added to the rpak reference scanner, and `ui/**/*.html`
  force-included (component templates are referenced by tag, not path, and the
  archive reader's `read_directory` serves bevy_hui's folder autoload in exports).

---

## 6. Entity model & the flex-vs-drag question ❓

### Current model ✅
`spawn_html_template_at` creates **one instance entity** with `HtmlTemplatePath`
+ `UiWidget` + an **absolute** `Node`, parented under a `UiCanvas`. The binding
observer puts `HtmlNode` on a **child**, so:
- bevy_hui owns/rebuilds only the child subtree (hot-reload safe);
- the instance entity is a stable, draggable, **scene-saved** unit whose position
  is just its `Node` (no overlay/UiId machinery needed);
- inner markup nodes are untagged → you drag the template as a whole.

### The open conflict ❓
Forcing the instance **absolute** means dropped templates are content-sized at a
fixed point — but flex sizing (e.g. a stretched/anchored layout) is often what
you *want*, and is the thing bevy_hui is best at. Dragging then fights the
layout. Desired: **respect flex by default, allow drag as an opt-in override.**

**Options:**
- **(A) Flex default + per-instance scene override (recommended).** Elements
  follow markup/flex layout. Dragging *opts an element out of flow* → absolute
  position stored as scene data on the instance (keyed per markup `id`), with a
  **"Reset to layout"** to snap back. The `.html` is never rewritten, so reused
  components/instances stay independent. This is the "prefab + instance override"
  model. Requires re-introducing per-node identity for elements you want to
  override individually (markup `id`).
- **(B) Dragging rewrites the `.html`.** True WYSIWYG, single source of truth.
  Fine for single-use templates, but moving a *reused* component/instance moves
  every use of it, and it contends with hot-reload + comment/format preservation.

**Recommendation:** (A). Revert the always-absolute default so flex is honored,
add the scene-stored override + reset affordance. (B) optionally later as an
explicit "bake to markup" action for one-off templates.
**Decision: pending sign-off.**

---

## 7. Custom-tag bridge — renzora widgets in markup 🔜

`bevy_hui`'s `HtmlComponents::register` maps a custom tag → an arbitrary spawn
function. Use it to expose renzora's own widgets in markup:
- `<radial_gauge value="0.58">`, `<arc>`, `<wedge>` → spawn the existing
  `renzora_game_ui` `UiMaterial` SDF shape widgets (real dials/arcs that flat CSS
  can't draw).
- Later: `<emitter>` → a **Cinder** emitter (see §9).

This is the highest-leverage way to get "rich, intricate" UI into markup without
waiting on bevy_hui to grow CSS features. Small effort; big expressive payoff.

---

## 8. Shader UI effects 🔜

Renzora **already** renders custom WGSL in the UI pass — `renzora_game_ui::shapes`
implements `UiMaterial` (Circle/Arc/RadialProgress/Wedge/…) via `UiMaterialPlugin`.
So shader-driven UI is a *working pattern*, not new tech.

Planned: a small library of animated `UiMaterial`s — glow, pulse, gradient fill,
dissolve, scanline/holographic panels — fed a `time` uniform (animates) and a
`value` uniform (reacts to data, e.g. health). Exposed as markup tags via §7 and
drivable from Lua. A glowing/pulsing shader health bar likely delivers most of
the "immersive" feel before particles even enter the picture.

---

## 9. `renzora_cinder` — UI particle system 🔜 (flagship)

**The gap:** every Bevy particle crate (`bevy_hanabi`, `bevy_enoki`,
`bevy_particle_systems`, Sprinkles) renders in world/camera space. **None**
composite with the `bevy_ui` layer. Cinder is the first UI-space particle system
for Bevy — sparks off a health bar, embers behind a menu, a burst on level-up,
correctly layered with UI and shipping in exports.

### Architecture: pooled CPU particles **as UI nodes**
An **emitter** is a UI node; each **particle** is a small child UI node (colored
quad → later `UiMaterial`-shaded/sprite) advanced each frame by a system, then
recycled. Chosen over render-to-texture / overlay-camera because it:
- **composes natively** — real UI nodes layer with other UI, respect the canvas,
  scale with `UiScale`, and ship in exports with zero pipeline work;
- is **right-sized** — UI FX want tens–hundreds of particles, not GPU millions;
- is **verifiable without a GPU** — the sim is plain ECS logic;
- **upgrades cleanly** — swap quads for `UiMaterial`/sprites or a GPU-instanced
  fast path later without changing the authoring API.

### Components (sketch)
- `CinderEmitter { rate, burst, shape (point/line/rect/circle), lifetime,
  speed/velocity, spread, gravity, max_particles, looping, world/local }`
- Over-life curves: `color_over_life`, `size_over_life`, `rotation_over_life`,
  `opacity_over_life`, `velocity_damping`.
- `CinderParticle { age, lifetime, velocity, seed }` (pooled; hidden when dead).
- Optional `CinderMaterial` (UiMaterial) for additive glow / textured sparks.

### Authoring & control
- **Markup:** `<emitter rate="20" lifetime="0.8s" gravity="0 400" color="#FF9..">`
  via the §7 bridge.
- **Editor:** an emitter widget in the game_ui palette/canvas; RON-configurable,
  hot-reloadable (mirrors `bevy_enoki`'s config approach — notably by Lommix, the
  `bevy_hui` author, so the ecosystem story is coherent).
- **Lua:** `burst("HealthBar", 30)` / `emit_on("X")` via the scripting bridge —
  e.g. spray cinders when health drops.

### Phasing
1. Crate scaffold + `CinderEmitter`/`CinderParticle` + sim system + a demo
   (continuous + burst). Compile-clean **vertical slice** to verify visually.
2. Over-life curves + emitter shapes + pooling/perf pass.
3. `<emitter>` markup tag + game_ui widget + Lua `burst`/`emit` API.
4. `UiMaterial`-shaded particles (additive glow, sprites); optional GPU-instanced
   fast path.

---

## 10. Roadmap (suggested order)

1. **Custom-tag bridge** (§7) — quick win; unlocks gauges/shapes in markup.
2. **One animated shader UI material** (§8) — a glowing/pulsing health bar,
   end-to-end, exposed as a tag. Visible "immersive UI."
3. **Cinder vertical slice** (§9 phase 1) — de-risk the novel piece early.
4. **Flex-vs-drag override** (§6) — orthogonal; settle and implement once signed off.
5. Iterate Cinder (phases 2–4) + grow the shader-effect library.

---

## 11. Verification & constraints

- **No GPU editor on the Windows dev box** → editor/visual behavior is verified by
  the user; agent work is compile-checked (`cargo check`) + CI.
- **Local linking cap:** the `renzora` dylib exceeds Windows' 64k export limit, so
  full builds/tests don't link locally; use `cargo check`. CI (ubuntu,
  `cargo test --workspace`) runs the parse test and others.
- New visual features ship as **compile-clean vertical slices** the user runs and
  tunes from screenshots, rather than large unverified drops.

---

## 12. Naming / lore

- `bevy_hanabi` = fireworks · `bevy_enoki` = mushroom · `bevy_hui` = markup …
  **`cinder`** = the glowing bits that fly off fire. `hui` builds the bar;
  `cinder` throws sparks off it. One-word identity for "the first UI particle
  system for Bevy."
