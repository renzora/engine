# Docs

Guides, plans, and migration notes for the engine.

## Architecture

- [editor-runtime-plugin-architecture.md](editor-runtime-plugin-architecture.md) — editor/runtime/plugin ABI and the "one build, editor-as-removable-bundle" plan.
- [renzora_markup.md](renzora_markup.md) — markup as a serialization format for a `bevy_ui` entity tree (round-tripping loader/runtime layer).

## API Reference

- [scripting_api.md](scripting_api.md) — Lua scripting API: lifecycle hooks, context globals, and world-acting functions.
- [template_api.md](template_api.md) — HUI `.html` markup template elements, attributes, bindings, and control flow.
- [hui_components.md](hui_components.md) — UI component catalog and roadmap (markup-composed widgets over engine behaviors).

## Roadmaps & Plans

- [roadmap.md](roadmap.md) — feature roadmap (linked from the splash screen).
- [ui_plan.md](ui_plan.md) — game-facing markup UI system and the road to the Cinder UI particle system.
- [renzora_network_plan.md](renzora_network_plan.md) — phased plan for full Lightyear networking coverage.
- [networking-test-plan.md](networking-test-plan.md) — hands-on in-engine checklist + scripts to test multiplayer (HUD-driven).
- [renzora_lumen_plan.md](renzora_lumen_plan.md) — Lumen-inspired GI plugin plan.
- [BEVY_0.19_MIGRATION.md](BEVY_0.19_MIGRATION.md) — Bevy 0.19 upgrade notes.

## Guides

- [plugin-development.md](plugin-development.md) — building plugins, components, and scripting against the engine's API.
