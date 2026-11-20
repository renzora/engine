# Debug Probes

A **probe** is a debug readout your plugin offers: frame times, a list of what is loaded, a warning when something is wrong, a switch that turns a feature off. It shows on the [companion's](../editor/companion) **Debug** screen, beside the engine's own probes (Performance, Memory, ECS Stats, Camera Debug, Scene Diagnostics and the rest).

A probe has no UI code. You write a function that builds a report from the `World`; the companion draws it. That is also why probes are cheap: a probe is built only while the companion is showing it, a few times a second, and the editor's window never lays out anything for it.

## A probe

```rust
use bevy::prelude::*;
use renzora::probes::{ProbeDef, ProbeItem, ProbeLevel, ProbeReport, RegisterProbe};

fn build(world: &mut World) -> ProbeReport {
    let enemies = world.query::<&Enemy>().iter(world).count();
    let mut report = ProbeReport::default();
    report
        .section("Enemies")
        .item(ProbeItem::figure("Alive", enemies.to_string(), "enemies", ProbeLevel::Normal));
    if enemies > 500 {
        report.item(ProbeItem::note("Over the 500 the AI budget assumes.", ProbeLevel::Warn));
    }
    report
}

app.register_probe(ProbeDef {
    id: "my_game.enemies",
    title: "Enemies",
    icon: "skull",
    category: "Gameplay",
    build,
    act: None,
});
```

The companion lists probes by `category`, then `title`; `icon` is a Phosphor icon name.

## What a report can hold

A report is a list of sections, each a title and a list of items:

| Item | Shows |
|---|---|
| `ProbeItem::figure(label, value, unit, level)` | A headline number, drawn large |
| `ProbeItem::stat(label, value)` / `stat_at(label, value, level)` | A labelled value on one line |
| `ProbeItem::series(label, values, unit)` | A history as a sparkline, oldest first |
| `ProbeItem::bar(label, fraction, value, level)` | A filled bar from 0 to 1, with its value in words |
| `ProbeItem::table(&[columns], rows)` | Rows under column headings; the card shows the first 40 |
| `ProbeItem::note(text, level)` | A sentence: a warning, an explanation, an empty state |
| `ProbeItem::toggle(id, label, on)` | An on/off control |
| `ProbeItem::button(id, label)` | A push button |

`ProbeLevel` colours a value: `Good` green, `Warn` amber, `Bad` red, `Muted` grey, `Normal` plain.

## Controls

A toggle or button calls your probe's `act` with the item's `id`, and for a toggle its new value:

```rust
fn act(world: &mut World, item: &str, value: Option<bool>) {
    if item == "spawning" {
        world.resource_mut::<SpawnSettings>().enabled = value.unwrap_or(true);
    }
}
```

The next report shows the change. Encode what the control acts on in its id, as `"select:42"`, and parse it in `act`.

## Gathering only while watched

When a probe needs data a system keeps current every frame (a rolling history, a walk over every mesh), gate that system so it runs only while the probe is shown:

```rust
use renzora::probes::probe_watched;

app.add_systems(Update, track_enemy_history.run_if(probe_watched("my_game.enemies")));
```

A probe nobody is watching then costs nothing at all.

## Where probes run

Probes run in the process the companion is connected to: the editor, or a game started with **Play** or exported with **Allow profiling**. A probe registered by an editor-only plugin exists only in the editor; register it from a runtime plugin to see it in a game as well.

## Related

- [The Companion](../editor/companion)
- [Editor Panels from a Plugin](./panels)
