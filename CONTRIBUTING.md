# Contributing to Renzora Engine

Thanks for your interest in contributing to Renzora! This guide covers everything you need to get started.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [AI-Assisted Contributions](#ai-assisted-contributions)
- [Reporting Issues](#reporting-issues)
- [Pull Requests](#pull-requests)
- [Development Setup](#development-setup)
- [Code Style](#code-style)
- [Testing](#testing)
- [Documentation](#documentation)
- [Commit Messages](#commit-messages)
- [License](#license)

## Code of Conduct

Be respectful, constructive, and collaborative. We're building something together — treat others the way you'd want to be treated. Harassment, trolling, and unconstructive negativity will not be tolerated.

## Getting Started

1. **Fork** the repository on GitHub
2. **Clone** your fork locally
3. **Create a branch** from `main` for your work
4. **Make your changes**, following the guidelines below
5. **Test** your changes
6. **Push** to your fork and submit a **pull request**

If you're looking for a good first contribution, check for issues labeled `good first issue` or `help wanted`.

## AI-Assisted Contributions

**Renzora is AI-friendly.** Use an assistant to write code, tests, or docs — we
review AI-assisted PRs on exactly the same terms as everything else. What we
insist on is that you own what you submit:

1. **Audit every line before you submit it.** You must be able to explain each
   change in review. "The model wrote it" is not an answer to a review question.
2. **Validate it by running it** — build it, launch it, exercise what you
   changed. A patch that has only ever been read is not validated.
3. **Write tests** that would fail without your change. Bug fixes get a
   regression test.
4. **Disclose the model and version** with an `Assisted-by:` commit trailer:

   ```text
   feat(physics): cylinder collider with radius and height

   Assisted-by: Claude Opus 5 (claude-opus-5)
   ```

   Format is `Assisted-by: <name> <version>`, with the exact model identifier in
   parentheses when the tool exposes one. One line per model. It's
   `Assisted-by:`, not `Co-authored-by:` — **the human who opens the PR is the
   sole author and is fully responsible for the contribution.**
5. **Never open a PR you haven't read in full.** Piping model output straight
   into a branch is the one thing this policy forbids outright, and bulk
   AI-generated PRs will be closed.

The complete policy — what "audited" means in practice, the repo-specific traps
AI-assisted patches hit most often, licensing, and AI in issues and reviews — is
at [docs/r1-alpha7/contributing/ai-policy.md](docs/r1-alpha7/contributing/ai-policy.md)
(published at <https://renzora.com/docs/r1-alpha7/contributing/ai-policy>).

## Reporting Issues

Before opening an issue, search existing issues to avoid duplicates. When filing a new issue, use the appropriate template:

### Bug Reports

Include:
- **Steps to reproduce** — Minimal, concrete steps to trigger the bug
- **Expected behavior** — What you expected to happen
- **Actual behavior** — What actually happened
- **Environment** — OS, GPU, engine version, and `rustc --version`
- **Run mode** — editor (`renzora-editor`), shipped game (`renzora`), or the runtime launched with `--server` (headless), `--host` (listen server), or `--vr`
- **Logs / screenshots** — Console output, error messages, or screenshots if applicable

### Crashes

The editor writes a crash report to `~/.renzora/crashes/last_crash.txt` (and shows a native dialog); a shipped game silently appends to a `crash.log` beside the executable. Attach whichever applies.

### Feature Requests

Include:
- **Problem statement** — What are you trying to do that isn't possible or is difficult?
- **Proposed solution** — How you'd like it to work
- **Alternatives considered** — Other approaches you thought about
- **Context** — Which part of the engine this affects (editor, runtime, scripting, etc.)

## Pull Requests

### Before You Start

- **Open an issue first** for non-trivial changes. This lets us discuss the approach before you invest time writing code.
- **Small PRs are preferred.** A focused PR that does one thing well is easier to review than a large PR that touches many systems.
- **Check [`docs/roadmap.md`](docs/roadmap.md)** and the issue tracker to see what areas need work.

### PR Guidelines

1. **Branch from `main`** — Name your branch descriptively (e.g., `fix-spotlight-shadow`, `add-cylinder-collider`)
2. **One concern per PR** — Don't mix bug fixes with new features or refactors
3. **Write tests** for new functionality when the module has existing test coverage
4. **Update the docs** under `docs/r1-alpha7/` if you change public APIs or add features — see [Documentation](#documentation)
5. **Don't break existing tests** — run the suite before submitting (see [Testing](#testing))
6. **Keep changes minimal** — Don't refactor unrelated code, add unnecessary comments, or reformat files you didn't change

### PR Checklist

- [ ] `cargo clippy --profile dist` is clean (warnings are denied in CI)
- [ ] Tests pass for the crates you touched
- [ ] New tests added for new functionality (where applicable)
- [ ] Docs updated under `docs/r1-alpha7/` (if you changed behavior or APIs)
- [ ] No unrelated formatting changes
- [ ] Branch is up to date with `main`
- [ ] AI-assisted work is audited and disclosed with an `Assisted-by:` trailer

### Review Process

- A maintainer will review your PR and may request changes
- Address review feedback by pushing additional commits (don't force-push during review)
- Once approved, a maintainer will merge your PR

## Development Setup

**You do not need Docker to develop on Renzora.** `cargo renzora` builds the workspace natively, stages a complete engine into `dist/<platform>/`, and launches it. Docker's job here is *cross-compilation* — producing export templates for platforms you don't own — plus reproducing CI exactly.

### Prerequisites

- **Rust**, installed via [rustup](https://rustup.rs/). `rust-toolchain.toml` pins the version (currently **1.95.0**) and rustup selects it automatically; nightly is not required.
- **Git**
- Your platform's usual native build dependencies — a C/C++ toolchain, and on Linux the graphics/audio dev headers:

  ```bash
  sudo apt install pkg-config libx11-dev libxcursor-dev libxrandr-dev libxi-dev \
    libwayland-dev libxkbcommon-dev libasound2-dev libudev-dev
  ```

- **Docker** — optional. Needed only for cross-compiling export templates (`renzora build <platform>`) and for running `renzora check` / `renzora test`, which reproduce CI inside the pinned image. The `renzora` CLI that drives it is published separately: `cargo install renzora`.

### Building and Running

```bash
git clone https://github.com/renzora/engine.git
cd engine
cargo renzora                 # build, stage dist/<platform>/, and launch the editor
cargo renzora dist            # build and stage without launching
cargo renzora plugin <name>   # rebuild one standalone plugin and stage it (hot reload)
cargo renzora sync            # regenerate the plugin wiring from the renzora::add! declarations
```

The first build is slow; every one after that is incremental. Building the editor always builds the runtime too — the runtime binary doubles as the dedicated server (`--server`) and the listen server (`--host`).

### Always pass `--profile dist`

**Every cargo command in this repo takes `--profile dist`, or goes through `cargo renzora`.**

```bash
cargo check  --profile dist [-p <crate>]    # the fast gate while editing
cargo clippy --profile dist [-p <crate>]    # reproduces the CI lint job
cargo test   --profile dist -p <crate>      # links and runs natively
```

A bare `cargo build`/`check`/`clippy`/`test` defaults to the `dev` profile and creates a *second* full set of artefacts under `target/debug/`. This workspace is far too large for two of them to coexist — ours once reached 314 GB and filled the disk. A full disk doesn't fail cleanly: rustc writes truncated `.rmeta`/`.rlib` files and the next crate to read them fails with errors that look like source bugs in code nobody touched. **A compile error in a crate you didn't touch, that disappears when you run again, is a disk-space error.** If `target/debug/` already exists, delete it.

## Code Style

### Formatting

Use default `rustfmt`. Run `cargo fmt` before committing, and don't hand-format in ways that conflict with it.

### Naming

- **Types:** `PascalCase` — `BlueprintGraph`, `ComponentRegistry`, `SelectionState`
- **Functions / variables:** `snake_case` — `spawn_entity`, `handle_input`
- **Constants:** `SCREAMING_SNAKE_CASE`
- **Modules:** `snake_case`, matching the file name
- **Crates:** `renzora_<name>`

### General Conventions

- Follow existing patterns in the module you're modifying
- Use Bevy's ECS idioms — systems, components, resources, events
- **Comment the *why*, not the *what*.** This codebase's hallmark is `//!` module and `///` item docs that explain why the code is shaped the way it is, what edge case it handles, and what previously went wrong. Match that density and voice; don't add narration that restates the code.
- Keep functions focused — if a function is doing too much, split it
- Avoid `unwrap()` in production code paths; use proper error handling or `expect()` with a message

### Module Organization

Modules typically follow this structure:

```rust
//! Module-level documentation explaining purpose.

mod submodule;
pub use submodule::*;

use bevy::prelude::*;

// Types, then systems, then helpers
```

## Testing

### Running Tests

Per-crate tests link and run natively, which is an order of magnitude faster than a container round-trip and the best way to iterate:

```bash
cargo test --profile dist -p renzora_physics
cargo test --profile dist -p renzora_ember parse_templates
```

To reproduce CI exactly — same rustc, same libs, same exclusions — run it in the container via the CLI:

```bash
renzora test                          # the workspace suite
renzora test --package renzora_net    # a single crate
renzora test host_client_promoted     # a single test by substring
```

> **`cargo test --workspace` does not pass natively**, and that's expected. It builds *example* targets, and two vendored XR crates have examples that never got a Bevy 0.19 rename. CI doesn't hit this because it excludes those crates. Test per-crate, or use `renzora test`.

CI runs `cargo test` and `cargo clippy -D warnings` inside the shared base image, excluding the vendored Bevy-ecosystem crates (`bevy_gauge`, `bevy_hanabi`, `bevy_mod_outline`, `bevy_silk`, `vleue_navigator`, `bevy_mod_openxr`, `bevy_mod_xr`, `bevy_xr_utils`) — they're third-party code copied into the tree, and running their suites just re-tests upstream. New first-party crates are covered automatically by `--workspace`.

### Writing Tests

- Place unit tests in a `#[cfg(test)] mod tests` block within the source file
- Cross-crate tests go in `crates/<crate>/tests/*.rs` (see `renzora_ember`, `renzora_inspector`, `renzora_net`, `renzora_physics`, `renzora_plugin` for real examples)
- Build a `MinimalPlugins` app and call `app.update()` to test systems headlessly — nothing in the suite needs a GPU or a window
- Focus on logic, serialization round-trips, and edge cases

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_serialization_roundtrips() {
        let original = create_test_node();
        let serialized = ron::to_string(&original).unwrap();
        let deserialized: Node = ron::from_str(&serialized).unwrap();
        assert_eq!(original, deserialized);
    }
}
```

### What to Test

- New data structures: serialization/deserialization round-trips
- New algorithms: correctness with edge cases
- New components: registration and default values
- Blueprint nodes: input/output types and code generation

## Documentation

Docs live in **this** repo under `docs/`, and pushing them to `main` auto-publishes to <https://renzora.com/docs> — you never copy anything by hand.

- **Edit `docs/r1-alpha7/` only.** It's the current development version. `docs/r1-alpha6/` and older are released and frozen; don't mirror changes into them.
- Add a new page to `docs/r1-alpha7/_sidebar.json` or it won't appear in the navigation.
- **A feature without its docs update is unfinished.** If you ship a new scripting function, inspector field, plugin capability, or editor panel, update the matching page in the same PR.

## Commit Messages

This repo uses [Conventional Commits](https://www.conventionalcommits.org/):

- **`type(scope): subject`** — types are `feat`, `fix`, `docs`, `refactor`, `chore`, `ci`, `security`; the scope is optional
- **Imperative mood**, under ~72 characters, **no trailing period**
- **Say what changed and why**

Good examples (real commits from this repo):

```text
feat(scripting): camera field of view
fix(import): harden the folder-import walk and unify the queue path
refactor(audio): delete kira; renzora_audio becomes the API and nothing else
docs(r1-alpha7): audio is a plugin, not a library the engine links
```

Bad examples:

```text
fixed stuff
WIP
update
Changes to the rendering system to improve the way that shadows are calculated for spot lights
```

## License

Renzora is dual-licensed under **MIT OR Apache-2.0** ([`LICENSE-MIT`](LICENSE-MIT) and [`LICENSE-APACHE`](LICENSE-APACHE)). By contributing, you agree that your contributions are licensed under the same terms, without any additional terms or conditions.
