# Editor / Runtime / Plugin Architecture — Plan

Status: **prerequisite DONE — now executing ("Operation Merge").** The egui → bevy_ui
migration landed (2026-06; `bevy_egui` removed from every workspace crate, bevy_ui/ember
is the sole editor UI, the F10/`EditorUiBackend` dual-backend switch is deleted). The
heavy/special thing the editor carried (egui + its bevy-feature pull) is gone, so the
"one build + lean game" corner of the trilemma (§4) is now reachable. Sections 5/6/9 below
are kept for history but their egui-specific premises are **resolved** — see §10 for the
current, concrete plan.

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
renzora::add!(MyPlugin);                              // Runtime (default)
renzora::add!(MyPlugin, Editor);                      // editor only
renzora::add!(MyPlugin, Runtime);                     // explicit (same as default)
renzora::add!(MyFoundation, Runtime, priority = -100);
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

---

## 10. Operation Merge — current state (post-egui, completed architecture)

**End state the user wanted (now shipped):** one game binary; the editor ships as a
*removable* dylib (`renzora_editor.dll` — the editor bundle cdylib). With the dll present
beside the exe it's the editor; delete it and the same binary is the exported game. A
community plugin built once loads in both.

### 10.1 Old build (historical — what we changed away from)
The two halves *used to be* the **same `renzora` binary** (`renzora_app`), differing only by
a feature on the binary:
- editor  = `--bin renzora --no-default-features --features editor` → turned on
  `renzora_runtime/editor`, which **statically linked ~50 editor crates**
  (`editor_reexports.rs` generated in `renzora_runtime/build.rs`) + called
  `add_editor_plugins()` (`for_each_static_plugin(Editor)`).
- runtime = `--features runtime` → no editor crates compiled in.

That **editor feature on the binary is gone.** The `renzora` binary is now **always
runtime-shaped** (no editor crates statically linked); editor functionality arrives only via
the loadable bundle. The aliases reflect this: `build-editor` and `build-all` are both
`build --profile dist --workspace` (binary + editor bundle + distribution plugins in one
invocation, one shared `bevy_dylib`), while `build-runtime` is the lean `build --profile
dist --bin renzora` (no bundle, no editor crates).

`bevy = { features = ["dynamic_linking"] }` + `prefer-dynamic` (`.cargo/config.toml`) ⇒
`bevy_dylib` + `renzora.dll` ship as shared libs (the editor framework rlib
`renzora_editor_framework` is baked in; the only types crossing the binary↔bundle boundary
live in `renzora.dll`). Plugin scope (`add!(_, Editor|Runtime)`) is a **runtime** filter
(`for_each_static_plugin`), NOT a compile strip — "editor vs runtime" is now *whether the
bundle dll is present beside the exe*, not a binary feature.

### 10.2 Shipped architecture
1. **One `cargo build --workspace`** builds the `renzora` binary AND the **editor bundle
   cdylib** together. Cargo unifies features across the workspace, so there is exactly **one
   `bevy_dylib`** (its feature set = the union editor∪runtime; with egui gone the editor's
   extra pull is small). The game ships that same `bevy_dylib` → identical hash → plugins
   built once load in both. *(This is the ABI fix; §8 step 2.)*
2. **Editor = a loadable bundle**, not a statically-linked feature. The cdylib
   **`renzora_editor`** (`crates/renzora_editor`) statically links the ~50 editor-only crates
   as rlibs and exports one `plugin_install_scope(&mut App, host_scope)` (the
   `export_plugin_bundle!` door, §7). At startup the binary dlopens it from `<exe_dir>` if
   present and installs its `Editor`-scope plugins. Absent ⇒ game. *(§8 step 3.)*
3. **Dual-mode crates stay in the binary** (`renzora_physics`, `renzora_lighting`, … — they
   run in every game). Their now-`bevy_ui` inspectors register into the editor registries
   whose **contract types live in `renzora.dll`** (the shared SDK dll, so TypeIds match
   across the bundle boundary — §5). With no editor bundle, those registrations are harmless
   (nothing reads them); with the bundle, the editor panels draw them. The editor framework
   itself (`renzora_editor_framework`) is now an **rlib**, baked into both the binary and the
   bundle — no separate editor dll is needed because the only types crossing the boundary
   live in `renzora.dll`.

### 10.3 Sequenced steps (all completed)
- **Step 0 — MEASURE the gap (done first; only a full build showed it).** Built both halves
  and compared the dylib the host links: `cargo build-editor` then `cargo build-runtime`,
  then looked at `target/dist/` (or `deps/`) for `bevy_dylib-<hash>.{dll,so,dylib}`. With the
  build now unified under `--workspace` (binary + bundle from one invocation), there is one
  `bevy_dylib` and the hash matches, so plugins built once load in both.
- **Step A — re-add the bundle door (additive, safe). ✅ DONE.** `export_plugin_bundle!` in
  `crates/renzora/src/plugin_meta.rs` (emits a single `plugin_install_scope(*mut App, u8) -> u32`
  + `plugin_bevy_hash`, per-plugin `catch_unwind` so one bad plugin can't abort the rest and no
  panic crosses `extern "C"`, returns a failure count; optional `foundation = [...]` ordered
  prefix). `dynamic_plugin_loader` prefers `plugin_install_scope`, falls back to `plugin_create`,
  same `plugin_bevy_hash` gate; `scan_plugins` now skips bundles. The `crates/renzora_editor`
  cdylib instantiates the macro. Pure addition; the working build is untouched.

  **Load-bearing findings from the Step-A adversarial review (preconditions for B–E):**
  1. **The inventory is GLOBAL.** `inventory::collect!(StaticPlugin)` lives in the *shared*
     `renzora` dylib (one registry across the boundary). So `plugin_install_scope` replays
     **every matching-scope plugin in that global registry**, not "the bundle's own". It works
     only because scopes partition (editor host installs `Editor`-only; runtime host installs
     `Runtime`-only — the two scopes are exclusive).
  2. **Deployment contract (hard).** A build either statically links + installs editor plugins
     (`add_editor_plugins`) **OR** ships them as the bundle — *never both*, or they double-add and
     Bevy panics. ⇒ **Step C must stop the host statically registering editor-scope plugins**, and
     the bundle is **not runtime-testable until then** (dropping it into a current *editor* build's
     `plugins/` would double-register).
  3. **Foundation isn't in the inventory.** `AssetRegistryPlugin`/`RenzoraEditorPlugin`/
     `KeybindingsPlugin` (editor) are added explicitly + ordered, not via `add!`. Step B must pass
     them through `export_plugin_bundle!(foundation = [...])` so they install first, in order.
  4. **Build via `--workspace` only.** `dynamic_linking` reaches the bundle's `bevy` solely via
     resolver-2 feature unification with `renzora_app`. Built in isolation → separate static bevy →
     `plugin_bevy_hash` mismatch → silent reject. **The user must verify the workspace-built bundle's
     hash == the host's** (ties into Step 0).
  5. **Step B keepalive.** Adding the ~44 editor crates as plain deps isn't enough — replicate
     `renzora_runtime`'s `pub use renzora_<crate>;` so the linker keeps each rlib's
     `inventory::submit!` ctor; otherwise an empty bundle loads and installs nothing.
- **Step B — editor bundle crate. ✅ DONE.** `crates/renzora_editor` (cdylib) depends on
  every editor-only crate (the former `renzora_runtime/editor` set) + the SDK foundation, and
  `export_plugin_bundle!`s them. Built by `--workspace`.
- **Step C — one build. ✅ DONE.** `renzora` builds runtime-shaped (editor crates no longer
  statically linked into it); editor functionality comes only via the bundle. The aliases are
  collapsed: `build-editor`/`build-all` = `build --profile dist --workspace` (binary + bundle
  + one `bevy_dylib`), `build-runtime` = `build --profile dist --bin renzora`. The editor
  framework is the `renzora_editor_framework` rlib; contract types stay shared in `renzora.dll`.
- **Step D — startup load. ✅ DONE.** Binary dlopens the `renzora_editor` bundle from
  `<exe_dir>` when present (editor), skips when absent (game). Wired into `src/main.rs`'s
  `load_global_plugins` (it also looks beside the exe, not only `plugins/`).
- **Step E — packaging. ✅ DONE.** Editor dist = binary + `bevy_dylib` + `renzora.dll` +
  `renzora_editor.dll` (the bundle). Game export = same minus the bundle dll. `renzora_export`
  templates + `docker/build-all.sh` updated.

### 10.4 Constraints / risks (as resolved)
- I (assistant) **cannot full-build/link locally** (Windows dylib 64k-symbol cap; `cargo
  check` only) — the **user ran the real builds + tests**. Step 0's measurement and every
  build-shape change were validated by the user.
- The one-plugin-per-cdylib FFI limit (§5) is why the bundle door (Step A) was mandatory.
- The working editor build stayed intact throughout: Steps A/B were additive; only Step C
  changed the binary's link shape — staged behind the bundle being proven first.
