# bevy_procedural_tree (vendored + forked)

Procedurally generated 3D trees for Bevy. **Vendored** into the renzora workspace
from the upstream crate by **Affinator**:

- Upstream: <https://github.com/Affinator/bevy_procedural_tree>
- Itself a Rust port of **ez-tree** by dgreenheck: <https://github.com/dgreenheck/ez-tree>
- License: MIT OR Apache-2.0 (see `LICENSE_MIT` / `LICENSE_APACHE`)

It is consumed by the `renzora_procedural_tree` distribution plugin (runtime) and
`renzora_procedural_tree_editor` (Add-Entity preset + inspector).

## Fork changes vs upstream 0.3.0

- **Dropped the `inspector` feature** and its `bevy-inspector-egui` dependency —
  renzora has no egui. `TreeMeshSettings` collapses to a single (non-egui)
  definition.
- **Added `serde` derives** on `Tree`, `TreeMeshSettings` and the nested param
  types, plus `#[reflect(Component, Serialize, Deserialize, Default)]` on `Tree`.
  renzora serialises scenes through the reflect `ReflectSerialize` path, so this
  is what lets a `Tree` round-trip through save/load. The `Handle`-bearing
  material-override fields are `#[serde(skip)] #[reflect(ignore)]` (handles don't
  remap across loads — the material falls back to the default on reload).
- **`Leaves` is now `pub`** (so the wrapper can find the generated leaf child to
  hide it) and is **no longer registered for reflection** (it stores an `Entity`
  link that must never be persisted; the child is regenerated from `Tree` on
  load).
- The `on_add` hook **no longer overwrites the parent entity's `Name`** — the
  parent keeps the name the user/preset gave it.
- **Default leaf material now uses the alpha-cutout leaf texture** (ambientCG
  LeafSet005, CC0 — see `textures/SOURCE.txt`) with `AlphaMode::Mask(0.5)` +
  `double_sided`/`cull_mode: None`, and the default bark is tinted brown.
  Upstream's *default* (no override) was flat green / white, which renders each
  leaf billboard as a solid rectangle (the perpendicular "Double" pair reads as
  an "X"). The textures are embedded via `include_bytes!` so the look works out
  of the box for a dlopen plugin with no loose asset files. Requires Bevy's
  `png` feature (enabled in this workspace).
- `edition` set to 2021 to match the workspace.

To re-sync with upstream, re-apply these diffs against a fresh checkout.
