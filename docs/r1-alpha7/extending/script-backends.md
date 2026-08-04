# Script Backends — adding a language

The engine ships a scripting **system** and no interpreter. Hooks, the command
vocabulary, the context, the queue that applies commands to the world — all of
that is statically linked and language-agnostic. Which language you can actually
write scripts in is decided by which plugin is present in `plugins/`.

`plugins/lua` supplies Lua. This page is about supplying something else.

## What you are building

An ordinary [standalone plugin](/docs/r1-alpha7/extending/standalone-plugins) —
a `cdylib` that links no Bevy and needs no engine checkout — which registers a
`Backend` instead of (or as well as) systems and components.

```toml
[workspace]

[package]
name = "wren"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
renzora_plugin = { path = "../../crates/renzora_plugin", features = ["script"] }
wren-sys = "..."   # whatever your interpreter needs
```

The `script` feature adds the command vocabulary, the contexts, the codec, and
the one `Interface` entry a backend registers through.

## The shape of a call

```text
  host                                   plugin
  ----                                   ------
  encode FrameContext  -- once/frame -->  (cached by frame_seq)
  encode EntityContext -- per entity -->
  read + hand over source ------------>   compile / reuse VM
                                          run the hook
                       <-- ScriptReply -  commands, vars, draws
  apply commands to the World
```

The host owns everything with a Bevy type in it: walking the scripted entities,
building the context, resolving and reading the script file, applying whatever
comes back. Your plugin owns exactly one thing — turning source text plus a
context into a list of `ScriptCommand`s.

## The trait

```rust
use renzora_plugin::script::*;

#[derive(Default)]
struct WrenBackend { /* your VMs */ }

impl Backend for WrenBackend {
    const NAME: &'static str = "Wren";
    const EXTENSIONS: &'static [&'static str] = &["wren"];

    fn set_bindings(&mut self, bindings: &[Binding]) {
        // Functions domain crates declared. Build them into every VM you make.
    }

    fn props(&mut self, script: &ScriptRef) -> Vec<VarDef> {
        // Parse whatever your language's prop syntax is, for the inspector.
        Vec::new()
    }

    fn hook(
        &mut self,
        script: &ScriptRef,
        hook: Hook,
        ctx: &Ctx,
        reply: &mut ScriptReply,
    ) -> Result<(), String> {
        // Run hook.fn_name() if the script defines it; push commands into
        // `reply.commands`.
        Ok(())
    }

    fn eval(&mut self, expr: &str) -> Result<String, String> { /* console REPL */ }

    fn evict(&mut self, path: &str, entity: u64) { /* drop cached VMs */ }
}

renzora_plugin::script_backend!(WrenBackend);

pub struct WrenPlugin;
impl Plugin for WrenPlugin {
    fn build(&self, app: &mut App) {
        app.add_script_backend(script_backend::desc());
    }
}
renzora_plugin::add!(WrenPlugin);
```

The `script_backend!` macro emits the `extern "C"` entry point and the state it
needs. It is a macro rather than a generic because the entry point must be a bare
function pointer with nowhere to carry state, so it needs a `static` — and a
`static` cannot be generic over your backend type.

## Rules that are not optional

**Never open a script file.** You are handed `script.source` and
`script.version`; rebuild your VM when the version changes and that is the whole
of hot-reload support. Exported and Android builds read scripts out of an rpak
archive through a closure the engine owns, so a plugin doing its own `std::fs`
would work in the editor and fail in every shipped game.

**Cache the frame context by `frame_seq`.** The context arrives in two halves.
The frame half — time, input, pressed keys, gamepads, the named-entity lookup —
is identical for every scripted entity and the host encodes it once. If you
decode it per entity you have given that saving straight back. The ergonomic
layer does this for you if you use `Backend`; only raw `dispatch` users need to
think about it.

**Host reads are valid only during the call.** `ctx.host` is backed by a `&World`
the engine drops when your hook returns. If your interpreter registers its
functions once at VM creation — which it should — you will need to stash the
table somewhere those closures can reach and clear it when the call ends. See
`plugins/lua/src/host.rs` for the thread-local-plus-guard pattern.

**Do not panic across the boundary.** The dispatcher catches panics for you and
reports `ScriptStatus::Panicked`, but an abort from an `extern "C"` frame would
take the editor with it, so do not defeat the guard.

## Hooks

`Hook::fn_name()` gives the conventional name for each, so every language agrees
and a script ported between them does not need renaming:

`on_ready`, `on_update`, `on_rpc`, `on_ui`, `on_draw`, `on_animation_event`,
`on_http`, `on_player_joined`, `on_player_left`.

A script that does not define a hook is the common case, not an error — most
define two of the nine. Return `Ok` with an empty reply.

Hooks are selected by an op code rather than one function pointer each, so a
tenth hook added later is **not** an ABI break: a plugin that does not know an op
returns `UnknownOp` and the host treats it exactly like an undefined hook.

## Declared bindings

`set_bindings` hands you what domain crates declared — `apply_force`,
`nav_set_destination`, `tr` and so on (see
[Script API Bindings](/docs/r1-alpha7/extending/script-bindings)). Build a
function for each:

- `BindingKind::Action` — pack the parameters and push a `ScriptCommand::Action`.
  A `ParamKind::Vec3` consumes **three** script arguments and produces one.
- `BindingKind::Read` — call `ctx.host.get(...)`, substituting the call's
  arguments into the path with `renzora_plugin::script::substitute` so every
  language resolves `clip_lengths.{0}` identically.
- `BindingKind::Translate` — call `ctx.host.translate(key)`.

Honouring these is what makes a new language useful immediately rather than
after every domain crate has been taught about it.

## Two languages at once

Backends are routed by file extension, so a project can have `.lua` and `.wren`
entities side by side. Two backends claiming the *same* extension is refused —
the first registration wins and the second is logged — because otherwise which
interpreter ran a script would depend on plugin load order, which is directory
iteration order, and one project would behave differently on two machines.

## Blueprints

`.blueprint`/`.bp` graphs are compiled to **Lua** by the host before the source
reaches any backend, because `renzora_blueprint` links Bevy and cannot cross the
boundary. Claim those extensions only if your language can execute Lua.

## Reference

| Thing | Where |
|---|---|
| The boundary | `crates/renzora_plugin/src/script/` |
| The engine side | `crates/renzora_scripting/src/plugin_backend.rs` |
| A working backend | `plugins/lua/` |
