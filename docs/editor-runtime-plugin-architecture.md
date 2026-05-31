# Editor / Runtime / Plugin Architecture — Plan

Status: **planned, not started.** Prerequisite (egui → bevy_ui migration) is being done
first; this document is the plan to execute *after* that lands.

---

## 1. The goal (the actual problem)

A **community plugin, built once, must load in BOTH the shipped editor and the shipped
runtime.** That is only possible if the editor and the runtime share an **identical
`bevy_dylib`** — same compiled Bevy, same SVH hash (the `bevy_dylib-<hash>.dll` suffix).
`bevy/dynamic_linking` + `prefer-dynamic` (`.cargo/config.toml`) is opted in for exactly
this. If a plugin links one `bevy_dylib` and the host ships another, the plugin won't
load.

Everything below exists to serve that one requirement.

---

## 2. Current shipped files (what each is)

| File | What it is | Needed by |
|---|---|---|
| `bevy_dylib` | The Bevy engine (render, ECS, math). | editor + runtime |
| `renzora.dll` | Core Renzora SDK: `add!`, plugin scopes, shared types, the post-process framework. | editor + runtime |
| `renzora_editor.dll` | **Editor SDK**: `EditorPanel`, the inspector registry, `egui` re-exports. | editor (+ any plugin with an inspector) |
| `plugins/*.dll` | `dlopen`'d distribution plugins. Some post-process effects already ship this way. | whoever installs them |

A **plugin** = one feature (a bloom effect, the physics system, an inspector panel).
Its **scope** decides when it loads:

```rust
renzora::add!(MyPlugin);                              // editor + games (default)
renzora::add!(MyPlugin, Editor);                      // editor only
renzora::add!(MyPlugin, Runtime);                     // games only
renzora::add!(MyPlugin, EditorAndRuntime, priority = -100);
```

---

## 3. Root cause of the ABI break

`docker/build-all.sh` builds the two halves **in isolation and with different scope**:

- editor  = `cargo build --workspace`
- runtime = `cargo build --bin renzora --no-default-features --features runtime`

Cargo unifies features across whatever is in the build. The `--workspace` editor build
pulls features from *every* crate (egui, asset tooling, XR, vendored forks…); the lean
`--bin` runtime build pulls a subset. `bevy_dylib`'s hash = a function of its own +
its internal deps' features, so **any feature delta shifts the hash**. → two different
`bevy_dylib`s → plugins can't be built once.

**The durable fix is structural: one build, one feature set, for both.** Feature-by-feature
alignment (forcing the runtime to match the editor's feature requests) was tried and is
**fragile whack-a-mole** — every new workspace crate that pulls a bevy feature re-diverges
it. Abandoned in favour of "one build."

---

## 4. The trilemma (the core tension)

You can pick **any two**:

| Approach | One build (ABI fixed) | Lean game (no egui) | No big refactor |
|---|:--:|:--:|:--:|
| Editor code compiled into one binary | ✅ | ❌ ships egui | ✅ |
| Separate editor/runtime builds (today) | ❌ hash diverges | ✅ | ✅ |
| Split dual-mode crates / drop egui | ✅ | ✅ | ❌ |

The only way to get **one build + lean game** is to remove the thing that makes the editor
"heavy and special" — **egui** — from anything a game links. That is what the chosen
direction (Section 6) does.

---

## 5. The wall: dual-mode crates

"The editor" is **two** kinds of crate:

1. **~50 editor-only crates** (`renzora_inspector`, `renzora_hierarchy`, `renzora_viewport`,
   `renzora_gizmo`, `renzora_console`, …). Optional (`dep:`) — these **can** move into a
   loadable editor DLL cleanly.
2. **~30 dual-mode crates** (`renzora_physics`, `renzora_lighting`, `renzora_ssr`,
   `renzora_engine`, …). **Non-optional runtime deps** — in every game. Their editor UI is
   compiled **into the same crate** by a feature flag:

   ```toml
   # crates/renzora_physics/Cargo.toml
   editor = ["dep:renzora_editor", "dep:bevy_egui", "dep:egui-phosphor", "dep:renzora_theme"]
   renzora_physics = { path = "../renzora_physics" }   # non-optional; ships in games
   ```

   The inspector (`crates/renzora_physics/src/inspector.rs`) uses `bevy_egui::egui`
   **directly** (custom `egui::DragValue`, `egui::ComboBox`, …). So `renzora_physics` is one
   crate holding *both* simulation and an egui inspector. You cannot relocate "the editor
   half" into a DLL because it is not a separate compilation unit.

### Key facts learned (don't re-discover these)

- **Scope is a runtime switch, not a compile-time strip.** `add!(_, Editor)` only decides
  whether a plugin *wakes up* at startup (`for_each_static_plugin` checks scope). It does
  **not** remove the code from the file. Two scoped plugins in one crate → the whole crate
  (egui and all) is still compiled → egui still ships. To *physically* remove editor code
  from a game, it must be a **separate crate** that the game's link graph excludes.
- **The editor SDK registry must be shared (same `TypeId`)** between a dual-mode inspector
  (which registers "how to edit a RigidBody") and the inspector *panel* (which reads the
  registry to draw it). If those live in different DLLs, the crate defining the registry
  (`renzora_editor`) **must be a shared DLL** — an rlib baked into both gives two separate
  registries and nothing shows up. (This is why `renzora_editor` is a `dylib` today.)
- **One-plugin-per-cdylib FFI limit.** `add!`'s `plugin_create`/`plugin_scope` exports are
  unmangled, so a dlopen cdylib may contain exactly one. Crates exporting multiple plugins
  need the bundle door (Section 7).

---

## 6. Chosen direction: migrate egui → bevy_ui FIRST

`bevy_ui` is already inside `bevy_dylib` and already used by the runtime. If the editor
draws with `bevy_ui`/HUI instead of egui:

- **Nothing to strip.** Editor/inspector code becomes the same UI system the game already
  ships — no 21 MB egui dependency riding along. A dual-mode crate's inspector can stay
  put and cost ~nothing in a game.
- **The egui-driven `bevy_dylib` divergence disappears** (a big chunk of the hash delta was
  egui pulling `bevy_winit/default` etc.).
- **The ~30-crate split becomes unnecessary** — there's no egui left to isolate.
- Aligns with the in-progress HUI / markup migration (it's finishing a path already taken).

### Honest caveats

- **Largest effort of all options** — every panel/inspector/widget rewritten on
  `bevy_ui`/HUI. Months, incremental.
- **The code editor is the hard case** — syntax-highlighted text editing is where the prior
  egui→bevy_ui attempt was reverted. That panel may stay egui (or a custom widget) longest.
- **bevy_ui alone does NOT fix the hash.** The root is *two builds* (Section 3); egui is one
  contributor. You still need **one build**. bevy_ui just makes that one build produce a
  *lean* game for free instead of one carrying egui.

---

## 7. The plugin-DLL mechanism (prototyped, reverted — re-add when needed)

To ship many editor-only plugins as **one** loadable DLL (working around the
one-plugin-per-cdylib limit), a `export_plugin_bundle!` macro was prototyped:

- A cdylib that statically links N `add!` plugins (as rlibs) exports a single
  `plugin_install_scope(&mut App, host_scope)` that replays every registered plugin of the
  matching scope into the `App`.
- The `dynamic_plugin_loader` prefers `plugin_install_scope` when present (bundle), and
  falls back to `plugin_create` for single-plugin community cdylibs. Both check
  `plugin_bevy_hash` before installing.

This was reverted along with the rest of the refactor (see Section 9). Re-introduce it if/
when the editor panels become a loadable bundle. It is **purely additive** — the existing
single-plugin community path is untouched.

---

## 8. Sequencing (the actual roadmap)

1. **NOW — egui → bevy_ui/HUI migration** (in progress, done first). Panel by panel; the
   game gets leaner on its own as each lands. Code editor last / special-cased.
2. **THEN — one build for both.** Build editor and runtime from **one `--workspace`
   invocation / one feature set** → identical `bevy_dylib` → community plugins build once
   and load in both. This is the ABI fix.
3. **THEN (optional) — leaner games via loadable editor.** Move editor-only panels into a
   loadable bundle (`renzora_editor.dll` via `export_plugin_bundle!`, or `plugins/` with
   `Editor` scope) so a shipped game simply omits the file. With egui gone, whether a
   dual-mode crate's (now-`bevy_ui`) inspector rides along or is split is a cheap, local
   decision rather than a forced 30-crate refactor.

Each step is independent and shippable; don't block one on the next.

---

## 9. Dead ends (tried and reverted — do not repeat)

- **Rename `renzora_editor` → `renzora_editor_api` + a thin `renzora_editor` facade DLL**
  (to collapse the editor into "one DLL"). Reverted. Reasons: (a) "one editor DLL" is
  impossible while dual-mode inspectors live in the binary and must share the SDK registry
  — that forces the SDK to stay a shared DLL, so it's **two DLLs minimum** regardless; and
  (b) the rename touched **131 crates / 466 usages / 226 files**. The bevy_ui migration
  removes the motivation entirely.
- **Per-feature alignment** in root `Cargo.toml` (`bevy_winit`, `ahash`, `gltf`,
  `bevy_state`, image formats, `webgpu`) to force the runtime build to match the editor's
  feature requests. Fragile — re-diverges on every new workspace crate. Replaced by the
  structural "one build" fix.
