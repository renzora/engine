# Contributing Guide

Renzora is open source and welcomes contributions — this guide covers the workflow, code style, and CI checks your pull request has to pass.

## Code of conduct

Be respectful, constructive, and collaborative. Harassment, trolling, and unconstructive negativity are not tolerated. We're building something together — treat others the way you'd want to be treated.

## Getting started

1. **Fork** the [engine repo](https://github.com/renzora/engine) on GitHub.
2. **Clone** your fork and check out a branch from `main`.
3. **Make your changes**, following the guidelines below.
4. **Run the checks** (`cargo test`, `cargo clippy`, `cargo fmt`) locally.
5. **Push** to your fork and open a **pull request** against `main`.

```bash
git clone https://github.com/YOUR_USERNAME/engine.git
cd engine
git checkout -b fix-spotlight-shadow
# make changes...
cargo fmt
cargo clippy --workspace --no-deps -- -D warnings
cargo test --workspace
git commit -m "Fix spotlight shadow not updating when range changes"
git push origin fix-spotlight-shadow
```

If you're looking for a first contribution, check for issues labeled `good first issue` or `help wanted`.

## Development setup

The full build story — the one-binary / editor-as-removable-cdylib model, the `.cargo/config.toml` aliases, and the Docker cross-compile image — is documented in [Building from Source](/docs/r1-alpha5/setup/building-from-source). The short version:

```bash
cargo renzora        # build the workspace and run the EDITOR
cargo runtime        # run the shipped-game shape (same binary, --no-editor)
cargo server         # run a headless dedicated server (--server)
```

> Use `cargo renzora`, not a bare `cargo run`. The editor is the removable `renzora_editor` cdylib bundle that the binary dlopens from beside itself, so the editor run must be `--workspace` (which `cargo renzora` expands to). There is **no `editor` compile-time feature** — the only build features on the `renzora` binary are `runtime` (default) and `wasm`.

### Toolchain

- Install Rust via [rustup](https://rustup.rs). The version is pinned in **one place**: `docker/Dockerfile` (`FROM rust:1.93.0-bookworm`). There is **no `rust-toolchain.toml`** and the project does **not** require nightly — match the pinned release with `rustup default 1.93.0`.
- **Linux** native builds need a C/C++ toolchain plus the usual Bevy system libraries; the bundled linker config also expects `clang` and `mold` (`.cargo/config.toml` hardcodes `-fuse-ld=/usr/bin/mold` for `x86_64-unknown-linux-gnu`):

  ```bash
  sudo apt install build-essential pkg-config clang mold \
      libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev
  ```

- **Windows** uses `rust-lld` (bundled with rustc — nothing extra to install). MSVC `link.exe` hits the 65535-object limit on `bevy_dylib`, which is why the config switches the linker.

> Heads-up for older docs: there is no `--features solari` raytracing build. `bevy_solari` is not wired in, and the GI tier `LumenQuality::Hwrt` currently renders nothing because wgpu ray tracing is not enabled. Don't add a `solari` feature flag to your build or bug report.

## What to contribute

| Area | How |
|---|---|
| **Bug fixes** | Browse the [issue tracker](https://github.com/renzora/engine/issues). |
| **Documentation** | Edit the markdown under `docs/` in the **website** repo (this site), not the engine repo. |
| **Editor panels** | Register a native bevy_ui panel with the `App` extension APIs `register_shell_panel(id, title, icon, category)` + `register_panel_content(id, scroll, build_fn)`. See [Editor Panels](/docs/r1-alpha5/editor-dev/panels). |
| **Scripting functions** | Add Lua bindings in `renzora_scripting` (or a domain crate's `ScriptExtension`). Rhai is a **subset** — see [Rhai](/docs/r1-alpha5/scripting/rhai) before assuming parity. |
| **Post-process effects** | Annotate a settings struct with `#[renzora_macros::post_process(...)]` and `renzora::add!` the plugin. See [Post-Processing](/docs/r1-alpha5/extending/post-processing). |
| **Plugins** | Self-register with `renzora::add!(MyPlugin)`. See [Building Plugins](/docs/r1-alpha5/extending/plugins). |
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

Tests live in `#[cfg(test)] mod tests` blocks alongside the code. Run the suite the same way CI does:

```bash
cargo test --workspace
cargo test -- scripting::tests        # a specific module
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

What's worth a test: new data structures (serialize/deserialize round-trips), new algorithms (correctness + edge cases), new components (registration and defaults), and networking round-trips (e.g. `crates/renzora_network/tests/host_server.rs` validates host-mode promotion to an in-process `HostClient`).

## Continuous integration

CI runs on every push and pull request to `main` (`.github/workflows/test.yml`). Both jobs run **inside the pinned toolchain image** `ghcr.io/renzora/engine:latest`, so the runner needs nothing installed — `rustc 1.93`, the cross toolchains, and the Linux dev libs are baked into the image.

> CI invokes **`cargo test` and `cargo clippy` directly** inside the image. The `renzora test` / `renzora check` CLI commands wrap the same cargo invocations in the container, so either reproduces CI locally.

The two jobs reproduce locally as:

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
- **Update documentation** (this website's `docs/`) when you change public APIs or add features.
- During review, push additional commits — **don't force-push** mid-review.

### PR checklist

- [ ] `cargo fmt` applied, no unrelated formatting changes
- [ ] `cargo clippy --workspace --no-deps -- -D warnings` is clean
- [ ] `cargo test --workspace` passes
- [ ] New tests added where applicable
- [ ] Branch is up to date with `main`

## Commit messages

Match the existing style:

- **Imperative present tense:** "Add ...", "Fix ...", "Update ...", "Refactor ..."
- **Under ~72 characters**, no trailing period.
- **Say what changed and why.**

```text
Add cylinder collider component with radius and height
Fix spotlight shadow not updating when range changes
Refactor blueprint codegen to support multiple output pins
```

## Reporting issues

Search existing issues first to avoid duplicates. For a bug report, include:

- **Steps to reproduce**, expected vs actual behavior.
- **Environment** — OS, GPU, and `rustc --version`.
- **Run mode** — editor (bundle present), shipped game (`--no-editor`), `--server`, or `--host`. Note that the only build features are `runtime` (default) and `wasm`; there is no `editor` feature to report.
- **Crash logs** — the editor writes `~/.renzora/crashes/last_crash.txt` (plus a native dialog); the shipped game silently appends `crash.log` beside the executable. Attach the relevant one.

## License

The engine is dual-licensed under **MIT OR Apache-2.0** (`LICENSE-MIT` and `LICENSE-APACHE` at the repo root). By contributing, you agree your contributions are licensed under the same terms, without additional conditions.

## What's next?

- [Building from Source](/docs/r1-alpha5/setup/building-from-source) — the full build, aliases, and Docker cross-compile flow
- [Architecture](/docs/r1-alpha5/setup/architecture) — the one-binary, editor-as-removable-cdylib model
- [Building Plugins](/docs/r1-alpha5/extending/plugins) — extend the engine with `renzora::add!`
