# Engine Docs

Plans, roadmaps, and migration notes for the Renzora engine repo — not user documentation.

This folder holds **internal design docs only**: forward-looking plans, phased roadmaps, and upgrade notes that live alongside the code. They describe intent and direction, so parts may be **aspirational, in-progress, or abandoned** — always check the doc against the current source before relying on it.

> Full **user and developer documentation** (scripting, markup/UI templates, plugin development, the widget catalog) lives at **<https://renzora.com/docs>**. The API-reference pages that used to sit here have moved there.

## Plans & Roadmaps

| Doc | What it covers |
|---|---|
| [roadmap.md](roadmap.md) | Overall feature roadmap (linked from the splash screen). |
| [renzora_network_plan.md](renzora_network_plan.md) | Phased plan for full Lightyear 0.26 multiplayer coverage. Only Phase 0, the RPC core, host mode, and basic `Transform` replication/interpolation are shipped today; the rest is aspirational or stub-only. |
| [networking-test-plan.md](networking-test-plan.md) | Hands-on in-engine checklist and HUD-driven test scripts for the multiplayer that exists now. |
| [renzora_lumen_plan.md](renzora_lumen_plan.md) | Original design for the `renzora_lumen` GI plugin. Note: the SDF architecture was abandoned in favour of CPU geometry voxelization, and the `Hwrt` tier is not yet wired. |
| [ui_plan.md](ui_plan.md) | Historical plan for the game-facing markup UI system. Predates the `renzora_hui` → `renzora_ember` merge; the live runtime is now `renzora_ember::markup` (`MarkupPlugin`). |

## Architecture & Migration

| Doc | What it covers |
|---|---|
| [editor-runtime-plugin-architecture.md](editor-runtime-plugin-architecture.md) | The "one binary, editor-as-removable-cdylib" model (Operation Merge) and the plugin ABI. The merge is now fully shipped; the document's older sections are kept for history. |
| [BEVY_0.19_MIGRATION.md](BEVY_0.19_MIGRATION.md) | Upgrade notes for the planned Bevy 0.19 bump. The engine currently runs on **Bevy 0.18**. |
