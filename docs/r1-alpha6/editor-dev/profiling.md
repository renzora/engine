# Profiling with Tracy

Renzora ships a **Tracy profiler bridge** (`renzora_tracy`) — a standalone
distribution plugin that streams live engine telemetry to a running
[Tracy](https://github.com/wolfpld/tracy) profiler over its native protocol:

- a **frame mark** per app frame, and
- every Bevy diagnostic as a named Tracy plot — frame time, FPS, entity count,
  per-render-pass GPU/CPU span times, and system CPU/memory where the platform
  supports it.

> **Per-system CPU zones** (the detailed timeline of which ECS system ran when)
> come from Bevy's `trace_tracy` feature, which is **not** in the normal build —
> it has no runtime off-switch and would arm Tracy at every launch. The bridge
> above gives frame marks + plots with no such cost. If you need the full CPU
> zone timeline, make a dedicated profiling build that re-adds `trace_tracy`
> (this moves the ABI hash, so build all distribution plugins against it too).

## Enabling it

Tracy is **gated behind two switches**, because activating it both connects the
Tracy client (a network listener + capture ring buffers) and turns on Bevy's
per-frame system-stat sampling — all of which cost real RAM/CPU. It stays
completely dormant unless **both** are on:

1. **Dev Mode** — Settings → Editor → Developer → *Dev Mode*.
2. **Tracy Profiler** — Settings → Plugins → *Tracy Profiler* → *Enable Tracy*.

The gate is **read once at startup**, so changing either switch takes effect the
next time you launch the editor. Both persist across runs (Dev Mode in
`~/.renzora/editor.toml`; the Tracy opt-in in `~/.config/renzora/tracy.json`,
or `%APPDATA%\renzora\tracy.json` on Windows).

> **Leave Tracy off when you're not profiling.** When dormant the plugin adds
> *nothing* — no client, no diagnostic sampling, no per-frame work, so it has a
> zero memory footprint. Only when both switches are on (and after a restart)
> does it stand up the client and the system-stat diagnostics that consume RAM.

## Capturing

Enable the two switches above and restart the editor, then start a Tracy server
(the desktop `Tracy.exe` profiler, or the headless `tracy-capture` CLI). The
editor connects and the timeline fills with frame marks and plots. Because the
bridge is Editor-scoped, it profiles the editor — including gameplay running in
the viewport's play mode.

## How it's wired (for plugin authors)

`renzora_tracy` is a self-contained distribution plugin: it depends only on
`bevy`, the `renzora` contract, `renzora_ember` (its settings toggle), and
`renzora_ui` (the "applies on restart" toast). It

- reads the host's dev-mode flag via `renzora::load_dev_mode()` — a persisted
  accessor on the shared contract, so the plugin needn't link the editor's
  `EditorSettings` type,
- registers its own *Tracy Profiler* category with `register_settings_section`,
- persists its opt-in to the user's config dir itself, and
- gates the bridge at startup on `dev_mode && opt-in`, adding *nothing* (not even
  the diagnostic sources) when off.

Nothing about Tracy is hardcoded into the editor or the contract.
