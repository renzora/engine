# Script Backends — adding a language

The engine ships a scripting **system** and no interpreter. Hooks, the command
vocabulary, the context, the queue that applies commands to the world — all of
that is statically linked and language-agnostic. Which language you can actually
write scripts in is decided by which plugin is installed.

**Rust is the primary scripting language** and does not go through this page at
all: a `.rs` script is compiled to a native plugin and called with `&mut World`
(see [Rust Scripts](../scripting/rust-scripts.md)). This page is about adding an
*interpreted* language beside it.

## What you are building

An ordinary [native plugin](native-plugins.md) that registers a `ScriptBackend`
instead of (or as well as) systems and components.

```toml
[package]
name = "wren"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["dylib"]

[dependencies]
bevy = "0.19"
renzora = "0.1"
wren-sys = "..."   # whatever your interpreter needs
```

## The shape of a call

```text
  engine                                 backend
  ------                                 -------
  walk the scripted entities
  build a ScriptContext        ------->
  resolve + read the source    ------->  compile / reuse VM
                                         run the hook
                               <-------  Vec<ScriptCommand>
  apply the commands to the World
```

The engine owns everything with a Bevy type in it: walking the scripted entities,
building the context, resolving and reading the script file, applying whatever
comes back. Your plugin owns exactly one thing — turning source text plus a
context into a list of `ScriptCommand`s.

That indirection is why a backend needs no `&mut World`. It describes what it
wants and the engine decides when and how, which is what lets an interpreter run
without an exclusive system.

## The trait

```rust
use bevy::prelude::*;
use renzora_scripting::backend::{AppScriptBackendExt, ScriptBackend};
use renzora_scripting::{ScriptCommand, ScriptContext, ScriptVariables};
use std::path::{Path, PathBuf};

#[derive(Default)]
struct WrenBackend { /* your VMs */ }

impl ScriptBackend for WrenBackend {
    fn name(&self) -> &str { "Wren" }
    fn extensions(&self) -> &[&str] { &["wren"] }

    fn set_scripts_folder(&mut self, path: PathBuf) { /* … */ }
    fn set_file_reader(&mut self, reader: FileReader) { /* see below */ }

    fn get_available_scripts(&self) -> Vec<(String, PathBuf)> { /* … */ }
    fn get_script_props(&self, path: &Path) -> Vec<ScriptVariableDefinition> {
        // Parse whatever your language's prop syntax is, for the inspector.
        Vec::new()
    }

    fn call_on_ready(
        &self,
        path: &Path,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<Vec<ScriptCommand>, String> { /* … */ }

    fn call_on_update(
        &self,
        path: &Path,
        ctx: &mut ScriptContext,
        vars: &mut ScriptVariables,
    ) -> Result<Vec<ScriptCommand>, String> { /* … */ }

    fn needs_reload(&self, path: &Path) -> bool { /* … */ }
    fn reload(&self, path: &Path) -> Result<(), String> { /* … */ }
    fn eval_expression(&self, expr: &str) -> Result<String, String> { /* console REPL */ }

    // Every other hook has a no-op default — implement the ones your language
    // supports and the rest simply never fire.
}

pub struct WrenPlugin;

impl Plugin for WrenPlugin {
    fn build(&self, app: &mut App) {
        app.add_script_backend(WrenBackend::default());
    }
}

renzora::plugin!(WrenPlugin, Runtime);
```

`add_script_backend` uses `get_resource_or_insert_with`, so your plugin has no
ordering relationship with `ScriptingPlugin` — registration works whichever
happened to be added first.

## Rules that are not optional

**Read scripts through the file reader, not `std::fs`.** `set_file_reader` hands
you a closure the engine owns. Exported and Android builds read scripts out of an
rpak archive through it, so a backend doing its own `std::fs` would work
perfectly in the editor and fail in every shipped game — the worst possible place
for that difference to appear.

**Drop cached state in `evict`.** A backend that keeps a VM per `(entity,
script)` — which is every backend that wants a script's globals to be per-entity
state — otherwise grows that map forever as entities churn. An empty `path` means
every script on that entity; a zero `entity` means every entity running that
script.

**Do not panic.** A panic in a hook takes the frame with it. Return
`Err(String)`; the engine logs it against the script and carries on.

## Hooks

Every hook has a conventional name, so a script ported between languages does not
need renaming:

`on_ready`, `on_update`, `on_rpc`, `on_ui`, `on_draw`, `on_animation_event`,
`on_http`, `on_player_joined`, `on_player_left`, `on_scene_loaded`,
`on_scene_load_failed`, `on_event`.

A script that does not define a hook is the common case, not an error — most
define two. Return `Ok` with an empty `Vec`.

Every hook but `on_ready` and `on_update` has a default implementation that does
nothing, so adding a thirteenth later breaks no existing backend.

## Declared bindings

Domain crates *declare* script functions rather than writing them — `apply_force`,
`nav_set_destination`, `tr` and so on (see
[Script API Bindings](script-bindings.md)). Read them from the
`ScriptExtensions` resource and build a function for each:

- `BindingKind::Action` — pack the parameters and push a `ScriptCommand::Action`.
  A `ParamKind::Vec3` consumes **three** script arguments and produces one.
- `BindingKind::Read` — read the named reflected field, substituting the call's
  arguments into the path with `renzora_scripting::extension::substitute` so
  every language resolves `clip_lengths.{0}` identically.
- `BindingKind::Translate` — look the argument up in the localization table.

Honouring these is what makes a new language useful immediately rather than
after every domain crate has been taught about it.

## Two languages at once

Backends are routed by file extension, so a project can have `.rs` and `.wren`
entities side by side. Several backends may be registered; two claiming the
*same* extension is refused, because otherwise which interpreter ran a script
would depend on plugin load order, and one project would behave differently on
two machines.

## Reference

| Thing | Where |
|---|---|
| The trait | `crates/renzora_scripting/src/backend.rs` |
| The command vocabulary | `crates/renzora_scripting/src/command.rs` |
| Declared bindings | `crates/renzora_scripting/src/extension.rs` |
