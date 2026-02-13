# Contributing to Renzora Engine

Thanks for your interest in contributing to Renzora! This guide covers everything you need to get started.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Reporting Issues](#reporting-issues)
- [Pull Requests](#pull-requests)
- [Development Setup](#development-setup)
- [Code Style](#code-style)
- [Testing](#testing)
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

## Reporting Issues

Before opening an issue, search existing issues to avoid duplicates. When filing a new issue, use the appropriate template:

### Bug Reports

Include:
- **Steps to reproduce** — Minimal, concrete steps to trigger the bug
- **Expected behavior** — What you expected to happen
- **Actual behavior** — What actually happened
- **Environment** — OS, GPU, Rust toolchain version (`rustc --version`)
- **Feature flags** — Which Cargo features you built with (e.g., `editor`, `solari`)
- **Logs / screenshots** — Console output, error messages, or screenshots if applicable

### Feature Requests

Include:
- **Problem statement** — What are you trying to do that isn't possible or is difficult?
- **Proposed solution** — How you'd like it to work
- **Alternatives considered** — Other approaches you thought about
- **Context** — Which part of the engine this affects (editor, runtime, scripting, etc.)

### Crashes

If the engine crashes, check the `crash.log` file generated in the working directory. Include its contents in your report.

## Pull Requests

### Before You Start

- **Open an issue first** for non-trivial changes. This lets us discuss the approach before you invest time writing code.
- **Small PRs are preferred.** A focused PR that does one thing well is easier to review than a large PR that touches many systems.
- **Check the feature parity table** in the README to see what areas need work.

### PR Guidelines

1. **Branch from `main`** — Name your branch descriptively (e.g., `fix-spotlight-shadow`, `add-cylinder-collider`)
2. **One concern per PR** — Don't mix bug fixes with new features or refactors
3. **Write tests** for new functionality when the module has existing test coverage
4. **Update documentation** if you change public APIs or add new features
5. **Don't break existing tests** — Run `cargo test` before submitting
6. **Keep changes minimal** — Don't refactor unrelated code, add unnecessary comments, or reformat files you didn't change

### PR Checklist

- [ ] Code compiles without warnings (`cargo build`)
- [ ] All existing tests pass (`cargo test`)
- [ ] New tests added for new functionality (where applicable)
- [ ] No unrelated formatting changes
- [ ] Branch is up to date with `main`

### Review Process

- A maintainer will review your PR and may request changes
- Address review feedback by pushing additional commits (don't force-push during review)
- Once approved, a maintainer will merge your PR

## Development Setup

### Prerequisites

- **Rust (nightly)** — Install from [rustup.rs](https://rustup.rs/). The project's `rust-toolchain.toml` will select the correct toolchain automatically.
- **Windows 10/11, Linux, or macOS**
- **Linux only:** `sudo apt install libwayland-dev`

### Building and Running

```bash
cargo run                      # Run the editor (default features)
cargo run --features solari    # With raytracing (requires Vulkan SDK + DLSS SDK)
cargo test                     # Run the full test suite
```

### Faster Linking (Recommended)

**Windows:**
```bash
rustup component add llvm-tools-preview
```

**Linux:**
```bash
sudo apt install lld clang
```

See the [README](README.md) for detailed setup instructions including optional Solari/raytracing prerequisites.

## Code Style

### Formatting

Use default `rustfmt` settings. Run `cargo fmt` before committing. Don't manually format code in ways that conflict with `rustfmt`.

### Naming

- **Types:** `PascalCase` — `BlueprintGraph`, `ComponentRegistry`, `SelectionState`
- **Functions / variables:** `snake_case` — `spawn_entity`, `handle_input`
- **Constants:** `SCREAMING_SNAKE_CASE`
- **Modules:** `snake_case`, matching the file name

### General Conventions

- Follow existing patterns in the module you're modifying
- Use Bevy's ECS idioms — systems, components, resources, events
- Prefer `///` doc comments on public items
- Use `//!` module-level doc comments to explain a module's purpose
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

```bash
cargo test                                    # Full suite
cargo test -- blueprint::graph_tests          # Specific module
cargo test -- scripting::tests                # Scripting tests
cargo test -- component_system::tests         # Component registry tests
```

### Writing Tests

- Place tests in a `#[cfg(test)] mod tests` block within the source file
- Test module names should match the module being tested
- Focus on testing logic, serialization round-trips, and edge cases
- Tests that require a full Bevy `App` or `World` should set up minimal state

Example:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_serialization_roundtrip() {
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

## Commit Messages

Follow the existing commit style:

- **Imperative present tense:** "Add ...", "Fix ...", "Update ...", "Refactor ..."
- **Concise but descriptive:** Aim for under 72 characters
- **No trailing period**
- **Focus on what changed and why**

Good examples:
```
Add cylinder collider component with radius and height
Fix spotlight shadow not updating when range changes
Refactor blueprint codegen to support multiple output pins
Update Avian3D to 0.5 for collision layer fixes
```

Bad examples:
```
fixed stuff
WIP
update
Changes to the rendering system to improve the way that shadows are calculated for spot lights
```

## License

By contributing to Renzora, you agree that your contributions will be licensed under the [Apache License 2.0](LICENSE.md). Any contribution intentionally submitted for inclusion in the project is submitted under the same license, without any additional terms or conditions.
