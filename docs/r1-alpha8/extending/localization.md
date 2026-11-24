# Localization

Renzora ships a single, process-wide translation table that every crate, the
editor bundle, and every dlopen'd plugin read through one function:

```rust
renzora::lang::t("menu.file") // → "File", "Fichier", "ファイル", …
```

The table lives in the `renzora` contract dylib, so there is exactly one copy
across the plugin boundary. The `renzora_lang` runtime plugin fills it
from three sources and keeps the active language in sync.

## How strings resolve

`t(key)` resolves in this order and never fails:

1. The **active language**, if it has the key.
2. **English** (`en`), the guaranteed-complete fallback.
3. The **key itself** (e.g. `"menu.file"`).

Because of the fallback chain, converting a hardcoded literal to `t("…")` is
always safe: an untranslated key shows readable English, and a key with no
entry at all shows the key. The UI is never blank.

For runtime values, interpolate `{name}` placeholders:

```rust
// "Saved {file}" → "Saved player.scene"
renzora::lang::t_args("status.saved", &[("file", &name)]);
```

## Where languages come from

1. **Built-in packs** are compiled into the binary from the repo-root
   `languages/` directory, so an exported game is fully localized with no extra
   files.
2. **External packs** — any `languages/*.toml` next to the executable or in the
   working directory — are loaded on top and *override* a built-in of the same
   `code`, key for key. The folder is re-scanned every couple of seconds, so
   editing a pack updates the running editor live. This is the install path a
   marketplace language pack will use.
3. **Project packs**: a project's own `languages/*.toml` hold the game's
   strings, in the same format. They merge over the built-ins (a project's
   `fr.toml` only needs the game's keys), and a project may add a language the
   engine does not ship. The editor loads them when the project opens and
   re-reads one when it is saved; an export packs every one of them into the
   game, which loads them before it picks the language it starts in.
4. **Plugin contributions**: see below.

## Pack format

A language pack is a TOML file with a `[meta]` header and a flat `[strings]`
table of `key = "value"` pairs:

```toml
[meta]
name = "Français"      # native name shown in the picker
code = "fr"            # BCP-47-ish code: "fr", "pt-BR", "zh", "zh-TW"
author = "Renzora Team"
version = "1.0"

[strings]
"common.ok" = "OK"
"menu.file" = "Fichier"
"menu.edit" = "Édition"
```

Keys are dotted, grouped by area (`common.*`, `menu.*`, `inspector.*`, one
section per panel). English (`languages/en.toml`) is the source of truth for
which keys exist.

## Adding translations from a plugin

Any `renzora_*` plugin can localize its own UI from its `build()`. The
registration API is on the contract crate, so a plugin needs **no** dependency
on `renzora_lang`:

```rust
impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        // From an embedded pack (same [meta]/[strings] format):
        let _ = renzora::lang::register_pack_str(
            include_str!("../languages/de.toml"),
        );

        // …or inline, for a handful of keys (no TOML file needed):
        renzora::lang::register_translations("en", [
            ("myplugin.title", "My Plugin"),
            ("myplugin.run",   "Run"),
        ]);
    }
}
```

Registration order doesn't matter — the table accumulates every contribution and
`t()` resolves against the active language at call time. Later writes to the
same `(code, key)` win, so a user pack can override your plugin's strings too.

## Switching language

```rust
renzora::lang::set_active("ja");        // change active language
renzora::lang::active_code();           // "ja"
renzora::lang::available();             // Vec<LocaleMeta> for a picker
```

Setting an as-yet-unknown code is allowed: install the pack later and it lights
up without another call.

### The language a session starts in

The editor and a game choose differently, because the editor's language is the
person editing's preference and a game's is the game's:

- **The editor** starts in your interface language, **Settings > Appearance >
  Language**, saved in `~/.renzora/settings.toml` under `[app] language`.
- **A game** starts in the project's `default_language` (a language code in
  `project.toml`, such as `"de"` or `"pt-BR"`). Without one it follows the
  player's system language when a pack for it is available (`de-DE` finds
  `de`, `zh-Hant-TW` finds `zh-TW`), and otherwise starts in English. It never
  reads the editor's `settings.toml`, so a game exported from a German editor
  does not start in German on a player's machine unless the project says so.

```toml
# project.toml, a top-level key (above the first [section])
default_language = "fr"
```

The `RENZORA_LANG` environment variable overrides both, which is handy for
testing a translation or taking screenshots. Either way the language can still
be changed while the game runs with `set_active`. Game UI markup written with
`{{ t("key") }}` follows the change on the next frame, with nothing to rebuild:

```html
<text font_size="18">{{ t("menu.play") }}</text>
```

### Reacting to changes

When the active language changes or packs (re)load, the plugin fires a
`LanguageChanged` message and bumps a global revision counter:

```rust
// Event-driven rebuild:
fn relocalize(mut events: MessageReader<renzora::lang::LanguageChanged>) {
    for _ in events.read() { /* rebuild cached text */ }
}

// …or gate a reactive panel on the counter:
renzora::lang::revision(); // monotonic; compare to a stored value
```

## Scripting

Scripts get a `tr(key)` function:

```lua
set_text(label, tr("hud.score"))
```

