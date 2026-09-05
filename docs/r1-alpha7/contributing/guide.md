# Contributing Guide

Renzora is open source and welcomes contributions — this guide covers the workflow, code style, and CI checks your pull request has to pass.

## Code of conduct

Be respectful, constructive, and collaborative. Harassment, trolling, and unconstructive negativity are not tolerated. We're building something together — treat others the way you'd want to be treated.

## Getting started

1. **Fork** the [engine repo](https://github.com/renzora/engine) on GitHub.
2. **Clone** your fork and check out a branch from `main`.
3. **Make your changes**, following the guidelines below.
4. **Run the checks** locally — `cargo clippy --profile dist` and the tests for the crates you touched.
5. **Push** to your fork and open a **pull request** against `main`.

```bash
git clone https://github.com/YOUR_USERNAME/engine.git
cd engine
git checkout -b fix-spotlight-shadow
cargo renzora                        # build, stage dist/, and launch the editor
# make changes...
cargo fmt
cargo clippy --profile dist          # the CI lint gate, natively
cargo test --profile dist -p renzora_physics
git commit -m "fix(lighting): update spotlight shadow when range changes"
git push origin fix-spotlight-shadow
```

If you're looking for a first contribution, check for issues labeled `good first issue` or `help wanted`.

## Using AI

You're welcome to use an AI assistant — we don't review AI-assisted PRs any
differently. In exchange, you audit every line you submit, you prove it works
with tests, and you name the model and version in an `Assisted-by:` commit
trailer. The full terms are on the [AI Policy](ai-policy.md) page; read it before
opening your first AI-assisted PR.

## Development setup

The full build story — the one-binary / editor-as-removable-cdylib model and the cross-compile images — is documented in [Building from a Checkout](/docs/r1-alpha7/setup/building-from-source). The short version:

```bash
cargo renzora                    # build the workspace and run the EDITOR
cargo renzora dist               # build and stage without launching
cargo check  --profile dist      # fast gate while editing
cargo clippy --profile dist      # reproduces the CI lint job
cargo test   --profile dist -p <crate>   # per-crate tests, natively
renzora test                     # the full suite, exactly as CI runs it (container)
```

> **Always pass `--profile dist`.** A bare cargo command defaults to the `dev` profile and creates a *second* full set of artefacts under `target/debug/`; this workspace is far too large for two of them, and a full disk surfaces as bogus compile errors in crates you never touched rather than as a disk error.

> You do not need Docker to develop on Renzora. Docker is a **cross-compiler** — it builds export templates for platforms you don't own — and a way to reproduce CI exactly. It is not the install path. Nothing is `dlopen`'d against Bevy any more: in-workspace plugins are statically linked `rlib`s wired in by a build-time generator, and [standalone plugins](/docs/r1-alpha7/extending/standalone-plugins) are C-ABI cdylibs that link no Bevy at all, so neither needs a canonical build environment. The editor is the removable `renzora_editor` cdylib bundle that the binary dlopens from beside itself; there is **no `editor` compile-time feature** — the only build features on the `renzora` binary are `runtime` (default) and `wasm`.

### Toolchain

- You need **Git** and **rustup**. `rust-toolchain.toml` pins the Rust version and rustup selects it automatically; the project does **not** require nightly. You will also need your platform's usual native build dependencies — a C/C++ toolchain, and on Linux the X11/Wayland/ALSA/udev dev headers (the list mirrors `docker/base/Dockerfile`).
- The Rust version is pinned in two lockstep files: `rust-toolchain.toml` (native) and `docker/base/Dockerfile` (`FROM rust:1.95.0-bookworm`, container). A bump must edit both.
- **Docker** is needed for two things only: cross-compiling export templates (`renzora build <platform>`) and reproducing CI exactly (`renzora check` / `renzora test`).
- Linux uses `mold` and Windows uses `rust-lld` (MSVC `link.exe` hits the 65535-object limit). `.cargo/config.toml` sets that up for native builds as well as the container, so a native link succeeds.
- **`cargo test --profile dist -p <crate>` links and runs natively**, including on Windows, and it is the fastest way to iterate. `cargo test --workspace` is the one that doesn't: it builds *example* targets, and two vendored XR crates have examples that never got a Bevy 0.19 rename. CI never hits this because it excludes those crates — test per-crate, or use `renzora test`.

> Heads-up: hardware ray-traced GI ships via the optional **`renzora_solari`** plugin (Bevy Solari), enabled by the `bevy_solari` Bevy feature in the workspace `Cargo.toml` and activated at runtime only on RT-capable GPUs — see [Solari ray-traced GI](../rendering/solari.md). There is still no `--features solari` *build* flag; Solari is a drop-in plugin, not a build variant. Lumen's separate `LumenQuality::Hwrt` tier remains an unimplemented placeholder and renders nothing.

## What to contribute

| Area | How |
|---|---|
| **Bug fixes** | Browse the [issue tracker](https://github.com/renzora/engine/issues). |
| **Documentation** | Edit the markdown under `docs/r1-alpha7/` in the **engine** repo; pushing to `main` auto-publishes it to this site. Older `docs/r1-alpha*` directories are frozen releases — leave them alone. |
| **Editor panels** | Register a native bevy_ui panel with the `App` extension APIs `register_shell_panel(id, title, icon, category)` + `register_panel_content(id, scroll, build_fn)`. See [Editor Panels](/docs/r1-alpha7/editor-dev/panels). |
| **Scripting functions** | Declare them from the owning domain crate via the `ScriptExtension` trait, so every language backend builds them. Engine-wide primitives live in the language plugin's `register_api()` (`plugins/lua`). |
| **Post-process effects** | Annotate a settings struct with `#[renzora_macros::post_process(...)]` and `renzora::add!` the plugin. See [Post-Processing](/docs/r1-alpha7/extending/post-processing). |
| **Plugins** | Declare with `renzora::add!(MyPlugin)` — a build-time generator reads that line *as text* and writes the committed static plugin lists, so keep it on one line at the top level. See [Building Plugins](/docs/r1-alpha7/extending/plugins). |
| **Export targets** | Improve a platform lane in `docker/build-all.sh`. |

> The editor has no `EditorPanel` trait you "implement and register" — panels are plain bevy_ui content functions registered through the two `App` extension methods above. Anything claiming an egui `EditorPanel` trait is stale (egui was fully removed).

## Code style

### Formatting

Use default `rustfmt`. Run `cargo fmt` before committing, and don't hand-format in ways that conflict with it.

### Naming

- **Types:** `PascalCase` — `BlueprintGraph`, `ScriptComponent`, `LumenLighting`, `DockTree`.
- **Functions / variables:** `snake_case` — `spawn_entity`, `handle_input`.
- **Constants:** `SCREAMING_SNAKE_CASE`.
- **Modules:** `snake_case`, matching the file name.

### General conventions

- Follow existing patterns in the module you're touching.
- Use Bevy's ECS idioms — systems, components, resources, events.
- Prefer `///` doc comments on public items and `//!` at the top of a module.
- Avoid `unwrap()` in production code paths; use proper error handling or `expect()` with a message.
- Keep changes minimal — don't refactor unrelated code or reformat files you didn't change.

## Testing

Tests live in `#[cfg(test)] mod tests` blocks alongside the code. Iterate per-crate natively, then reproduce CI in the container before you submit:

```bash
cargo test --profile dist -p renzora_physics   # natively, fast
renzora test                                   # the full suite, exactly as CI runs it
renzora test --package renzora_net             # one crate, in the container
```

Focus on logic, serialization round-trips, and edge cases:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blueprint_graph_roundtrips() {
        let original = sample_graph();
        let serialized = ron::to_string(&original).unwrap();
        let restored: BlueprintGraph = ron::from_str(&serialized).unwrap();
        assert_eq!(original, restored);
    }
}
```

What's worth a test: new data structures (serialize/deserialize round-trips), new algorithms (correctness + edge cases), and new components (registration and defaults). Cross-crate tests go in `crates/<crate>/tests/*.rs` — `renzora_plugin/tests/abi_order.rs` pins the C-ABI interface layout, `renzora_bsn/tests/raw_roundtrip.rs` round-trips the scene format, `renzora_ember/tests/parse_templates.rs` proves every shipped UI template parses without a GPU, and `renzora_net/tests/round_trip.rs` covers the wire codec.

## Continuous integration

CI runs on every push and pull request to `main` (`.github/workflows/test.yml`). Both jobs run **inside the shared base image** `ghcr.io/renzora/base:latest`, so the runner needs nothing installed — `rustc 1.95` and the Linux dev libs are baked into the base (the per-platform cross toolchains aren't needed to test first-party crates).

> CI invokes **`cargo test` and `cargo clippy`** inside the image. The `renzora test` / `renzora check` CLI commands wrap those same cargo invocations in the container, so they reproduce CI locally — run those, not a native `cargo`.

Each job runs this inside the image (reproduce with `renzora test` / `renzora check`):

```bash
# Test job — first-party crates only; the vendored Bevy-ecosystem crates are excluded
cargo test --workspace \
  --exclude bevy_gauge --exclude bevy_hanabi --exclude bevy_mod_outline \
  --exclude bevy_silk --exclude vleue_navigator \
  --exclude bevy_mod_openxr --exclude bevy_mod_xr --exclude bevy_xr_utils

# Clippy job — warnings are denied
cargo clippy --workspace --no-deps \
  --exclude bevy_gauge --exclude bevy_hanabi --exclude bevy_mod_outline \
  --exclude bevy_silk --exclude vleue_navigator \
  --exclude bevy_mod_openxr --exclude bevy_mod_xr --exclude bevy_xr_utils \
  -- -D warnings \
  -A clippy::too_many_arguments \
  -A clippy::type_complexity
```

The vendored crates (`bevy_*`, `vleue_navigator`) are third-party code copied into the tree — they still build as dependencies, but their own test suites are skipped to avoid re-testing upstream. `too_many_arguments` and `type_complexity` are allowed because they're inherent to Bevy systems and queries. New first-party crates are covered automatically via `--workspace`.

## Pull requests

- **Open an issue first** for non-trivial changes so the approach can be discussed.
- **One concern per PR** — don't mix a bug fix with a feature or a refactor.
- **Branch from `main`** with a descriptive name (`fix-spotlight-shadow`, `add-cylinder-collider`).
- **Write tests** for new functionality when the module already has coverage.
- **Update documentation** — the markdown under `docs/r1-alpha7/` in the engine repo — when you change public APIs or add features. New pages also need an entry in `docs/r1-alpha7/_sidebar.json`.
- During review, push additional commits — **don't force-push** mid-review.

### PR checklist

- [ ] `cargo fmt` applied, no unrelated formatting changes
- [ ] `cargo clippy --profile dist` (or `renzora check`) is clean — warnings are denied in CI
- [ ] Tests pass for the crates you touched (`cargo test --profile dist -p <crate>`, or `renzora test`)
- [ ] Docs updated under `docs/r1-alpha7/` if behavior or APIs changed
- [ ] New tests added where applicable
- [ ] Branch is up to date with `main`
- [ ] AI-assisted work is audited and disclosed with an `Assisted-by:` trailer ([AI Policy](ai-policy.md))

## Commit messages

This repo uses [Conventional Commits](https://www.conventionalcommits.org/):

- **`type(scope): subject`** — types are `feat`, `fix`, `docs`, `refactor`, `chore`, `ci`, `security`; the scope is optional.
- **Imperative mood**, under ~72 characters, no trailing period.
- **Say what changed and why.**

```text
feat(scripting): camera field of view
fix(import): harden the folder-import walk and unify the queue path
refactor(audio): delete kira; renzora_audio becomes the API and nothing else
docs(r1-alpha7): audio is a plugin, not a library the engine links
```

## Reporting issues

Search existing issues first to avoid duplicates. For a bug report, include:

- **Steps to reproduce**, expected vs actual behavior.
- **Environment** — OS, GPU, and `rustc --version`.
- **Run mode** — editor, shipped game, or the runtime launched with `--server` (headless), `--host` (listen server), or `--vr`. Both modes are the same `renzora` binary: it is the editor when the editor image (`renzora_editor.dll` / `.so` / `.dylib`) is beside it and the game when it isn't, so name the mode rather than an executable. There is no `--no-editor` flag; removing that one file is how you get a game. Note also that the only build features are `runtime` (default) and `wasm`; there is no `editor` feature to report.
- **Crash logs** — the editor writes `~/.renzora/crashes/last_crash.txt` (plus a native dialog); the shipped game silently appends `crash.log` beside the executable. Attach the relevant one.

## License

The engine is dual-licensed under **MIT OR Apache-2.0** (`LICENSE-MIT` and `LICENSE-APACHE` at the repo root). By contributing, you agree your contributions are licensed under the same terms, without additional conditions.

## What's next?

- [AI Policy](ai-policy.md) — using AI assistants, auditing, testing, and disclosure
- [Building from Source](/docs/r1-alpha7/setup/building-from-source) — the full build, aliases, and Docker cross-compile flow
- [Architecture](/docs/r1-alpha7/setup/architecture) — the one-binary, editor-as-removable-cdylib model
- [Building Plugins](/docs/r1-alpha7/extending/plugins) — extend the engine with `renzora::add!`
