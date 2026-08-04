# Next session — plan and state

Branch: `plugin-abi`. Seven commits:

- `e304191f` feat(build): statically link Bevy; split the editor into its own binary
- `7a802449` refactor(scripting): remove the Rhai backend
- `3ebcea18` feat(script): the language-backend boundary, behind a `script` feature
- `d72555fb` feat(script): register a language backend through the ABI (MINOR 3)
- `af7315d4` refactor(script): share one command enum; make extensions declarative
- `d2400344` feat(script): drive a language plugin through the ScriptBackend trait
- `ab87f550` feat(lua): move the interpreter into plugins/lua

**Nothing here has been run.** Everything is verified to compile, with
`cargo tree` confirming the structural properties. The engine has not been
launched since Bevy went static.

---

## 0. Launch it — do this before anything else

```
cargo renzora dist
```

Then run `dist/windows-x64/renzora-editor.exe`.

This is the highest-value action available and it gates most of what follows.
The migration converted 13 distribution plugins from `dlopen` to static
`inventory` registration. That change **cannot fail at compile time** — a
plugin that no longer registers just silently does nothing.

What to check:

1. Editor boots, panels render, a scene loads.
2. Boot log plugin count vs. a pre-migration build (`git stash` + build `67bb7ce5`
   if you want a clean baseline).
3. The 13 specifically: vignette, forward decal, lumen GI, solari, cloth,
   ragdoll, light2d, text3d, pool water, procedural tree, gaussian splatting,
   tracy toggle, AI chat panel.
4. `plugins/` still loads — 62 C-ABI plugins, unaffected by any of this.
5. `renzora.exe` (the runtime) launches a project as a game.

If plugin counts match, the risky part of the migration is confirmed and
steps 1–3 below become mechanical.

---

## 1. `inventory` → explicit feature-gated plugin lists

**Why:** `inventory` existed so `dlopen`'d plugins could self-register. Nothing
is dlopen'd any more — the ex-cdylibs are ordinary Bevy plugins linked into the
binary. An explicit list is simpler, idiomatic Bevy, and deletes a pile of
dead-strip scaffolding.

**Scale:** 160 `add!` sites across 95 crates → two explicit lists.
`priority` is used in only 3 real places (`-100`, `-50`, `0`), so ordering is
nearly flat.

**Deletes with it:**

- `renzora::add!`, `for_each_static_plugin`, `StaticPlugin`, `PluginScope`,
  the `inventory` dependency
- **both keepalive `build.rs` files** (`renzora_runtime`, `renzora_editor`) —
  they generate `pub use renzora_foo;` lines purely so linked crates' ctors run
- **`crates/renzora_static_plugins`** entirely — that aggregator exists *only*
  to force inventory ctors into a lean export
- `src/main.rs`'s `extern crate renzora_static_plugins` keepalive

Four separate dead-strip workarounds collapse into "call the function".

**Fan-out sites to replace:**

- `crates/renzora_runtime/src/lib.rs:924` — Runtime scope
- `crates/renzora_editor/src/lib.rs:72` — Editor scope (inside `install()`)

**Approach:** automatable. Parse `add!(Type, Scope[, priority = N])` from all
160 sites, generate the two lists with `#[cfg(feature = "…")]` gates matching
each optional dep, strip the `add!` lines, delete the macro.

**Hazard:** with `inventory`, adding a crate registers it automatically. With
explicit lists, **forgetting one is a silent feature loss** — no compile error.
This is exactly why step 0 matters: without a known-good plugin count to diff
against, you cannot tell whether a missing plugin came from this step or from
the earlier cdylib conversion.

---

## 2. Lua -> C-ABI plugin - **DONE**

Landed as five commits on `plugin-abi`. **Still unlaunched** - everything below
is verified to compile and unit-test, nothing has been run.

The shape that shipped: the scripting *system* stays statically linked (hooks,
the 115-command vocabulary, the context, the apply queue) and the *interpreter*
moved to `plugins/lua`. `backend.rs` is the contract a scripting plugin
implements, so a Wren or Python backend needs no engine change.

- `crates/renzora_plugin/src/script/` - the boundary. Hand-rolled codec (the
  guest side must stay dependency-free), compiled into both sides so there is
  one encoder and nothing to drift. 40 unit tests, including a round trip of
  every one of the 115 command variants.
- `sys::Interface` gained `add_script_backend` - MINOR 2 -> 3. The only domain
  that needed a table entry, because it is the only one where the host calls
  *into* the plugin.
- `crates/renzora_scripting/src/plugin_backend.rs` - the engine side.
- `plugins/lua/` - the interpreter, tracked as a rename so history follows it.

**The `ScriptExtension` "blocker" was half dead code.** All five implementations
were the same four lines (pack args, fire an action), and `populate_context` /
`setup_lua_context` / `ExtensionData` were empty stubs nobody read. The trait is
now declarative, so **mlua is gone from `renzora_physics`, `_animation`,
`_navmesh`, `_ragdoll` and `_lang`**, and a second language inherits every
binding for free.

**Decisions that were open, now settled:**

- **Hooks are op codes, not one function pointer each.** Adding a tenth hook is
  therefore *not* an ABI change - a plugin that does not know an op returns
  `UnknownOp` and the host treats it as an undefined hook. The doc's worry about
  freezing the set at eight does not apply.
- **Two backends can coexist**, routed by file extension, which `ScriptEngine`
  already did. Two claiming the *same* extension is refused - first wins - since
  otherwise the choice would depend on directory iteration order.
- **The host keeps file I/O.** A plugin gets `source` + `version` and never
  opens a path, or rpak-backed exports would break. Blueprint compilation also
  stays host-side: `renzora_blueprint` links Bevy.

**Per-frame cost:** the context splits frame-global from per-entity, encoded and
decoded once per frame via `frame_seq`. Note this bounds what the *boundary*
adds; `execution.rs` still clones the named-entity map and the key/action tables
per scripted entity, which predates all of this. Removing those is now possible
and is the obvious follow-up.

**Fixed in passing:** `LuaBackend::evict_entity` existed and nothing ever called
it, so the VM map grew for the life of the process. `ScriptBackend::evict` plus a
`RemovedComponents` system makes it reachable.

**What to check when you launch:**

1. `cargo renzora dist` builds `plugins/lua` automatically (xtask iterates
   `plugins/*`). Confirm `lua.dll` lands in `dist/windows-x64/plugins/`.
2. Boot log: `[scripting] backend `Lua` handles .lua, .blueprint, .bp` then
   `[scripting] adopting `Lua``. Absent = the plugin did not load.
3. A script that moves something, reads `get(...)`, calls `apply_force`, and
   declares a prop - those exercise commands, host reads, declared bindings and
   the inspector round trip respectively.
4. Hot reload: edit a running script, confirm the VM rebuilds.
5. `docker/build-all.sh` does **not** build standalone plugins at all - see §3.
   Only the xtask path stages them.

## 3. Finish the migration's loose ends

- **`docker/build-all.sh`'s `copy_shared_libs`** (lines ~133–203) still stages
  `bevy_dylib` + `renzora` + the editor bundle + Rust `std`. Needs the same
  treatment `xtask::stage()` already got: copy two executables, drop the
  shared-lib pass. Also **delete its runtime lane (~291–307)** — dead code, no
  `run_lane` invokes it, and its two-target-dir design is what this replaces.
- **`wrap_linux_appimage` / `wrap_macos_app`** move the file literally named
  `renzora`. Fine for the game; the editor needs its own wrapper or ships flat.
- **Stale comments** describing the dlopen architecture: `.cargo/config.toml`
  (the `prefer-dynamic` block and the `crt-static` note), most
  `crates/renzora_*/Cargo.toml` headers, `renzora_editor/src/lib.rs`'s whole
  "Step A/Step B preconditions" section. Prose only, no build impact.
- **`crt-static` is now viable.** It was disabled because it "changes crate
  disambiguators, breaking TypeId across dylib boundaries". There are no dylib
  boundaries left, so that reason is void — enabling it drops the
  `VCRUNTIME140.dll` dependency. Land it separately with a plugin-allocated
  string round-trip test, since C-ABI plugins would then be on a different CRT
  heap.

---

## 4. Docs

CLAUDE.md **§3 is obsolete wholesale** — there is no shared `bevy_dylib`, no
`plugin_bevy_hash`, no ABI to match. §2's "cargo test does NOT link natively on
Windows" is also too broad: `cargo test -p renzora --lib` links fine and ran 9
tests. It is the full-workspace suite that fails.

`docs/r1-alpha7/` only (alpha5/alpha6 are frozen). 41 pages reference
scripting; the Rhai-specific ones are now wrong.

Record the PE measurements so the export-cap myth dies: `bevy_dylib` exported
44,148 names, `renzora.dll` 1,458, **`renzora.exe` 0**. Executables export
nothing — the 65,535 cap was a property of `crate-type = ["dylib"]`, not of
program size.

---

## 5. Smaller items, verified but unlanded

- **`renzora_bluenoise`** — orphan. Nothing in the workspace depends on it; it
  is swept in by the `crates/renzora_*` member glob. Wire it or delete it.
- **`renzora_rmip`'s `bake` feature leaks into every shipped game.**
  `renzora_import` (editor-only) enables `bake = ["dep:image", "dep:intel_tex_2"]`,
  and `renzora_engine` (runtime) deps `renzora_rmip` plain — so feature
  unification puts a full image codec stack plus the Intel texture compressor
  in every export. Split into `renzora_rmip_bake`.
- **`renzora_network` / `renzora_shader` declare `editor = []`** with zero
  `cfg(feature = "editor")` sites. Dead features; delete them and the
  `"renzora_network/editor"` line in `renzora_editor/Cargo.toml`.
- **Lean export ships zero C-ABI plugins.** `renzora_export/src/overlay.rs:1005`
  sets `is_lean` and skips both the shared-lib copy *and* the `plugins/` copy
  block. A static host dlopens C-ABI plugins fine. Relax one conditional; 62
  plugins restored.
- **Exported VR games ship without `openxr_loader.dll`** — the export
  allowlist silently drops it although xtask stages it.
- **`renzora_forward_decal` and `renzora_pool_water`** carry a non-optional
  `renzora_editor_framework` dep while registering at Runtime scope, so ~4k
  lines of editor framework land in every shipped game. Split per CLAUDE.md §5
  into `renzora_<name>` + `renzora_<name>/editor/`.
- **`reflect_source.rs`** (committed, inert). Reflection-driven inspector rows,
  `RENZORA_REFLECT_INSPECTOR=gaps|all` to try. Blocked on a feature-level
  declaration — see the note at the end of this file.

---

## Binary sizes, for reference

Measured on `dist` profile (`opt-level = 2`, `strip = "symbols"`, no LTO):

| | |
|---|---|
| `renzora.exe` | 170 MB |
| `renzora-editor.exe` | 238 MB |
| `renzora.pdb` / `renzora_editor.pdb` | 91 / 122 MB (separate, never staged) |

Old dynamic footprint for comparison: shipped game ~218 MB
(85 exe + 121 `bevy_dylib` + 11 `renzora.dll`), editor install ~349 MB.

**Do not enable LTO on `dist`.** Measured: `lto = "thin"` made both binaries
*larger* (170→174, 238→243) and the build slower. At `opt-level = 2` thin LTO's
dominant effect is cross-crate inlining, which is not size-constrained, and the
duplicated inlined bodies outweigh what dead-stripping removes. LTO only pays
for itself on size when paired with `opt-level = "s"` — which is what
`[profile.dist-lean]` already does. This is recorded in the profile comment.

`.text` is 117 MB of the 170 MB file — **31% of the binary is not code**.
Prime suspects for that 52 MB, unmeasured: Bevy features that embed binary
blobs (`tonemapping_luts`, `smaa_luts`, `bluenoise_texture`, `dfg_lut`,
`area_light_luts`, `default_font`) plus 186 `.wgsl` files. Worth measuring
before optimising.

Top `.text` contributors: `bevy_ecs` 5.9, `renzora_ember` 5.8, `bevy_reflect`
5.1, **`avian3d` 4.5 + `avian2d` 4.2**, `bevy_pbr` 4.4, `bevy_hanabi` 3.2 +
`renzora_hanabi` 2.0, `naga` 3.1. The real lever is not compiler flags — it is
`capabilities.rs` not compiling what a given game does not use. No game needs
both physics engines.

---

## Which static plugins can become C-ABI plugins

Assessed after converting all 13 from `cdylib` to `rlib`. **The deciding factor
is not size — it is whether the plugin's actual work lives in a Bevy-linking
dependency**, because a C-ABI plugin links no Bevy at all.

### Cannot be C-ABI — the plugin *is* a Bevy library

| crate | loc | why |
|---|---|---|
| `renzora_cloth` | 25 | the sim is `bevy_silk` |
| `renzora_procedural_tree` | 66 | mesh gen is `bevy_procedural_tree` |
| `renzora_solari` | 468 | wraps Bevy's own `SolariPlugins` |
| `renzora_gaussian_splatting` | 665 | the pipeline is `bevy_gaussian_splatting` |
| `renzora_light2d` | 949 | the renderer is `bevy_firefly` |
| `renzora_lumen` | 3,390 | own render graph, compute passes, bind groups |
| `renzora_rt` | 447 | own render pipeline |
| `renzora_preview` | 1,097 | render-to-texture |
| `renzora_tracy` | 231 | `tracing-subscriber` integration |

The thin ones are thin *because* the weight is in the dependency.
`renzora_cloth` at 25 lines looks trivially portable and is the opposite —
there would be nothing left to port, and `bevy_silk` cannot cross the boundary.

**These stay as rlibs.** Not a defeat: they are deep engine rendering features
that arguably belong in the engine, and `capabilities.rs` already strips them
per game.

### Plausibly C-ABI

| crate | loc | needs |
|---|---|---|
| `renzora_text3d` | 560 | **`plugins/text3d` already exists** — check whether it supersedes this and delete |
| `renzora_forward_decal` | 159 | insert Bevy's `ForwardDecal`; only dep is `renzora_editor_framework`, which the `/editor` split removes anyway |
| `renzora_vignette` | 166 | insert Bevy's built-in `Vignette`; **no non-Bevy deps at all** |
| `renzora_ragdoll` | 538 | Avian bodies per bone — the ABI has `physics.rs` |
| `renzora_pool_water` | 925 | custom material — `plugin_material` exists |
| `renzora_ai_chat` | 1,962 | panel + HTTP — the ABI has both |

**Unconfirmed capability** that gates vignette and forward_decal: whether a
C-ABI plugin can insert a **host-defined** component (Bevy's `Vignette`) rather
than only its own registered ones. `ecs.rs` has `register_component`,
`component_id_of` and an `unresolved_component` path that hints at referring to
components the plugin does not own — read it before promising these work.

**Suggested order:** `renzora_text3d` (possibly free), then `renzora_vignette`
(smallest real port, proves the host-component-insert case), then the rest.

None of these are on the critical path. They are the better long-term shape,
not a blocker.

---

## Note on the reflection inspector (`reflect_source.rs`)

Committed but `Off` by default, for a reason worth remembering.

The field-generation layer works — ranges via `#[reflect(@0.0f32..=5.0f32)]`,
enum dropdowns, nested structs, remove, enable. It was proven end-to-end on
`renzora_vignette`: deleting its 44-line `InspectorEntry` dropped the crate's
`renzora` dep to **zero features** (−53/+16 lines). That change was **reverted**
because the default had to go `Off`, which would have left vignette with no
inspector at all.

Three things reflection cannot supply, all discovered by running it:

1. **Which components are user-facing.** `FillGaps` mode looked safe and was
   not: "has no hand-written entry" correlates strongly with "is not authored
   state". `renzora_lumen` re-`try_insert`s `RtLighting` from its routing system
   every time settings change, so deleting the component or flipping its toggle
   silently reverted within a frame.
2. **Add Component curation.** Inferring addability from `#[reflect(Default)]`
   was tried and reverted — the menu filled with `AngularDamping`,
   `CenterOfMass`, `ColliderConstructorHierarchy`, twice each, because avian2d
   and avian3d both register those names. Not a filtering problem: **reflection
   enumerates components, the menu offers features.**
3. **Presentation** — icon, category, grouping.

**The unblocking idea** (not implemented): declare it at *plugin* level in
`renzora::add!`, which every plugin already calls and which is not the editor
contract:

```rust
renzora::add!(VignettePlugin, component = VignetteSettings,
              name = "Vignette", icon = "aperture");
```

One line replaces the 44-line entry except the field data. It is feature-level
(so the section is named for the plugin, not the component), it declares which
component is *authored* (so derived ones like `RtLighting` are never shown,
killing both bugs by construction), and it needs no `AppEditorExt` or
`InspectorEntry` import.

**Caveat:** step 1 of this plan deletes `add!`. If both land, the declaration
belongs wherever the explicit plugin list ends up instead. Sequence them
together or the design changes under you.
