//! Tracy profiler bridge — a standalone C-ABI plugin.
//!
//! Streams the engine's diagnostics to a running [Tracy] profiler over Tracy's
//! native protocol: one frame mark per app frame, plus every measurement the
//! host publishes — frame time, FPS, entity count, per-render-pass GPU and CPU
//! times, process CPU and memory — as a named Tracy plot.
//!
//! [Tracy]: https://github.com/wolfpld/tracy
//!
//! ## What this is not
//!
//! **A flame graph.** There are no CPU zones here and there cannot be. Bevy's
//! per-system spans are `#[cfg(feature = "trace")]` inside `bevy_ecs` and its
//! GPU-timestamp zones are `#[cfg(feature = "tracing-tracy")]` inside
//! `bevy_render`: instrumentation that was compiled out does not exist to be
//! switched on, and no plugin loaded at run time can put it back. Tracy's Flame
//! Graph and zone Statistics windows stay empty, and `tracy-csvexport -e`/`-g`
//! return nothing.
//!
//! Everything this streams is a *plot*. That is genuinely enough to find a
//! bottleneck — expand the `render/*/elapsed_gpu` rows and read the maxima — but
//! it will not tell you which system is eating the frame. For that, build the
//! engine with `cargo renzora profile`, which compiles the instrumentation in
//! and moves the plugin ABI as a side effect. Turn this plugin OFF in such a
//! build: Bevy frame-marks there itself, and two marks per frame halves every
//! frame time Tracy reports.
//!
//! ## Why it is a plugin rather than part of the engine
//!
//! It used to be `crates/renzora_tracy`, an rlib linked into the editor bundle,
//! which meant every editor build compiled `tracy-client`'s C sources and
//! carried the client whether or not anyone ever profiled. Here, deleting the
//! `.dll` deletes the feature — no listener socket, no C build in the engine
//! tree, nothing dormant to reason about.
//!
//! The move became possible when the boundary learned to carry measurements
//! (`SystemCall::diagnostics`, ABI MINOR 4.8). Before that a plugin could not
//! read `DiagnosticsStore`, so a bridge had to live inside the host.
//!
//! ## The dormant state, precisely
//!
//! Off, nothing is created: no Tracy client, no socket, no plot names, and the
//! per-frame system returns on its first line. On, the client starts and the
//! bridge feeds it.
//!
//! **Turning it off again stops the feeding but does not shut the client down.**
//! That is a property of `tracy-client`, not a shortcut taken here: there is no
//! `impl Drop for Client` in the crate — shutdown exists only behind its
//! `manual-lifetime` feature, whose start/stop pairing rules are delicate enough
//! that arming them for a profiler nobody has connected to would be the more
//! fragile choice. It costs an idle listener socket and nothing else, because
//! `ondemand` (see `Cargo.toml`) means the client accumulates no trace data
//! until a profiler actually attaches. The `renzora_tracy` this replaces claimed
//! dropping its client resource "tears the connection down and frees the
//! buffers"; it did neither.

use renzora_plugin::diagnostics::Diagnostics;
use renzora_plugin::panel::PanelCommands;
use renzora_plugin::prelude::*;
use std::collections::HashMap;
use std::sync::Mutex;
use tracy_client::{Client, PlotName};

const SETTINGS_ID: &str = "tracy_settings";

/// The master enable toggle.
const ACT_ENABLE: i32 = 1;
/// Per-category toggles. `ACT_CATEGORY + i` is the switch for `CATEGORIES[i]`,
/// so adding a category needs no new constant — and the ids stay stable as long
/// as [`CATEGORIES`] is only appended to, which is what keeps a saved config
/// meaning the same thing after an upgrade.
const ACT_CATEGORY: i32 = 100;

/// A group of diagnostics the user can turn off as a unit.
///
/// Grouped rather than listed one-plot-at-a-time because the render paths are
/// per-pass and open-ended — a scene with more passes has more of them, and a
/// list of forty checkboxes that changes as you load a level is not a settings
/// panel. The categories below are stable regardless of what the scene does.
struct Category {
    /// Stable key written to the config. **Never rename one**: an old config
    /// naming a key this build does not know falls back to that category's
    /// default, silently turning a plot the user had off back on.
    key: &'static str,
    label: &'static str,
    /// Whether a path belongs to this category. Order matters — [`categorise`]
    /// takes the first match, so the specific render predicates must precede
    /// any general one.
    matches: fn(&str) -> bool,
    default: bool,
}

/// The category table. Append only; see [`Category::key`].
///
/// Everything defaults ON except shader invocation counts, which are the one
/// group that is usually noise: they are raw counters in the millions, so Tracy
/// autoscales them and they sit on the plot list crowding out the millisecond
/// timings that a frame-budget question actually turns on.
const CATEGORIES: &[Category] = &[
    Category {
        key: "frame",
        label: "Frame (fps, frame time)",
        matches: |p| matches!(p, "fps" | "frame_time" | "frame_count"),
        default: true,
    },
    Category {
        key: "entities",
        label: "Entity count",
        matches: |p| p == "entity_count",
        default: true,
    },
    Category {
        key: "system",
        label: "CPU & memory",
        matches: |p| p.starts_with("system/") || p.starts_with("process/"),
        default: true,
    },
    Category {
        key: "render_gpu",
        label: "Render passes — GPU time",
        matches: |p| p.starts_with("render/") && p.ends_with("/elapsed_gpu"),
        default: true,
    },
    Category {
        key: "render_cpu",
        label: "Render passes — CPU time",
        matches: |p| p.starts_with("render/") && p.ends_with("/elapsed_cpu"),
        default: true,
    },
    // The GPU pipeline-statistics family. `_primitives_out` is in here and not
    // in `other` because it is the same kind of number as the invocation
    // counters beside it — a real capture carries `clipper_invocations` and
    // `clipper_primitives_out` for every pass, and a filter that took one and
    // left the other would still leave the plot list crowded while claiming to
    // have cleared it. Matched by suffix rather than listed, since the set is
    // per-pass and grows with the scene.
    Category {
        key: "invocations",
        label: "Shader & pipeline counters",
        matches: |p| p.ends_with("_invocations") || p.ends_with("_primitives_out"),
        default: false,
    },
    // The catch-all, and the reason it exists: the host's diagnostic set is open
    // — any engine crate or other plugin may register its own path, and several
    // do. Without this they would match no category and a filter built from the
    // list above would silently drop them, which is indistinguishable from the
    // engine having stopped measuring. Anything unrecognised is shown.
    Category {
        key: "other",
        label: "Other diagnostics",
        matches: |_| true,
        default: true,
    },
];

/// Which category a path falls into. Always succeeds — the last entry matches
/// everything.
fn categorise(path: &str) -> usize {
    CATEGORIES
        .iter()
        .position(|c| (c.matches)(path))
        .unwrap_or(CATEGORIES.len() - 1)
}

/// Everything the bridge owns, in one lock.
///
/// A `static` rather than a plugin resource because the settings action handler
/// runs on the *editor's* UI systems, not in a plugin system, and so has no
/// `SystemCall` to reach a resource through. One `Mutex` is the shape `ai_chat`
/// uses for the same reason.
static STATE: Mutex<Option<State>> = Mutex::new(None);

struct State {
    /// The user's opt-in, mirrored to disk on every change.
    enabled: bool,
    /// Per-category enables, parallel to [`CATEGORIES`].
    categories: Vec<bool>,
    /// Held only while streaming. Starting it is what opens Tracy's listener.
    client: Option<Client>,
    /// Which category each path fell into, so the string matching in
    /// [`categorise`] runs once per distinct path rather than once per path per
    /// frame. At a few dozen paths either would be fine; this is a plot bridge
    /// whose own cost should not show up in what it is plotting.
    category_of: HashMap<String, usize>,
    /// Tracy's `PlotName` requires `'static` storage but diagnostic paths are
    /// dynamic strings, so each distinct path is leaked once and cached. The set
    /// is small (a few dozen) and stops growing after the first frames, so the
    /// leak is bounded — this is `new_leak`'s intended use, not an oversight.
    ///
    /// **Populated only when a path is actually plotted**, which is why it is
    /// separate from `category_of` rather than a second field in it. A category
    /// the user never turns on then leaks nothing at all — and leaking eagerly
    /// bought nothing anyway, since re-enabling a category sees every one of its
    /// paths again on the very next frame.
    plots: HashMap<String, PlotName>,
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let state = guard.get_or_insert_with(|| {
        let cfg = load_config();
        State {
            enabled: cfg.0,
            categories: cfg.1,
            client: None,
            category_of: HashMap::new(),
            plots: HashMap::new(),
        }
    });
    f(state)
}

// ── The bridge ───────────────────────────────────────────────────────────────

/// Push this frame's measurements as plots, then close the frame on Tracy's
/// timeline.
///
/// Runs in `Last` so the diagnostics it reads are this frame's finished numbers
/// rather than a mix of this frame's and the previous one's, and so the frame
/// mark lands after everything that contributed to the frame.
fn pump(diags: Diagnostics) {
    with(|state| {
        if !state.enabled {
            return;
        }
        // Started here rather than at init so the toggle can turn profiling ON
        // without a restart. `Client::start()` is idempotent — it returns a
        // handle to the running client if there is one — so calling it on the
        // first enabled frame costs a check thereafter.
        let client = state.client.get_or_insert_with(Client::start);

        for d in diags.iter() {
            // NaN is the normal state for a diagnostic that has registered but
            // not yet been sampled. Tracy will happily accept it and then draw a
            // plot with a hole in it, which reads as "the engine stopped
            // measuring" rather than "this had not started yet".
            if !d.is_valid() {
                continue;
            }
            let category = match state.category_of.get(&d.path) {
                Some(c) => *c,
                None => {
                    let c = categorise(&d.path);
                    state.category_of.insert(d.path.clone(), c);
                    c
                }
            };
            // Checked BEFORE the plot name is created. Emitting a value is what
            // makes a row appear in Tracy, and Tracy's protocol has no message
            // that removes one — so a category left off never puts a row on the
            // timeline in the first place, which is the only point at which this
            // decision can still be made.
            if !state.categories.get(category).copied().unwrap_or(true) {
                continue;
            }
            let name = match state.plots.get(&d.path) {
                Some(name) => *name,
                None => {
                    let name = PlotName::new_leak(d.path.clone());
                    state.plots.insert(d.path.clone(), name);
                    name
                }
            };
            // The smoothed value: raw frame time is noisy enough that a plot of
            // it is unreadable, and Tracy does its own aggregation on top.
            client.plot(name, d.smoothed);
        }

        client.frame_mark();
    });
}

// ── Settings ─────────────────────────────────────────────────────────────────

fn settings_markup(enabled: bool, categories: &[bool]) -> String {
    let mut m = String::from(
        "Node { flex_direction: Column, row_gap: Px(8.0), width: Percent(100.0) }\nChildren [\n",
    );
    m.push_str("    Text(\"Enable Tracy\"),\n");
    m.push_str(&format!(
        "    ( EmberToggle {{ on: {enabled} }} PanelActionId {{ action: {ACT_ENABLE} }} ),\n"
    ));
    m.push_str(
        "    Text(\"Streams to a running Tracy server on 127.0.0.1:8086. Takes effect \
         immediately. Plots only \\u{2014} there is no flame graph without a profiling \
         build.\"),\n",
    );

    // The category switches stay rendered while Tracy is off rather than being
    // hidden behind it: choosing what to capture before starting a capture is
    // the normal order, and a settings panel that empties itself when you turn
    // the feature off is a worse way to say "these do nothing right now".
    m.push_str("    Text(\"Plots\"),\n");
    // Says what turning one off does, because the answer is not the obvious one
    // and the difference is visible on screen. Tracy's protocol has no message
    // that removes a plot — the complete set is PlotData{Int,Float,Double},
    // PlotConfig and PlotName — so a row already on the timeline stays there
    // showing a frozen line. Reconnecting drops it, because the on-demand client
    // discards plot data while nothing is attached and replays only GPU
    // contexts, lock names and thread names on connect.
    m.push_str(
        "    Text(\"Turning one off stops it immediately. Rows already on Tracy's timeline \
         stay until you reconnect \\u{2014} its protocol has no way to remove a plot. \
         Reconnect, or set these before connecting, for a clean capture.\"),\n",
    );
    for (i, cat) in CATEGORIES.iter().enumerate() {
        let on = categories.get(i).copied().unwrap_or(cat.default);
        let label = cat.label;
        let action = ACT_CATEGORY + i as i32;
        m.push_str(&format!("    Text(\"{label}\"),\n"));
        m.push_str(&format!(
            "    ( EmberToggle {{ on: {on} }} PanelActionId {{ action: {action} }} ),\n"
        ));
    }

    m.push_str("]\n");
    m
}

/// Handle a toggle — the master switch or one of the category switches.
///
/// Runs on the editor's UI systems, where a panic would abort the process — the
/// host's thunk carries the guard, which is why this can be ordinary code.
fn on_action(mut action: Action) {
    let id: i32 = action.name().parse().unwrap_or(0);
    // A toggle crosses as 0.0 or 1.0 in `value`; there is no separate boolean
    // channel in the action payload.
    let on = action.value > 0.5;

    let markup = with(|state| {
        if id == ACT_ENABLE {
            state.enabled = on;
        } else if let Some(i) = (id - ACT_CATEGORY)
            .try_into()
            .ok()
            .filter(|i: &usize| *i < CATEGORIES.len())
        {
            // Grown rather than indexed blindly: a config written before a
            // category was appended leaves the vector short, and this handler is
            // reachable from a switch the markup has already drawn for it.
            //
            // Filled with each category's OWN default, not with `true`. Blanket
            // `true` was wrong in the one direction that matters: it would switch
            // on the counter group, whose whole reason for defaulting off is that
            // it buries the millisecond timings — and it would do so silently,
            // as a side effect of the user touching some unrelated switch.
            while state.categories.len() < CATEGORIES.len() {
                state.categories.push(CATEGORIES[state.categories.len()].default);
            }
            state.categories[i] = on;
        } else {
            // An id from markup this build did not write. Redrawing on it would
            // fight whatever did.
            return None;
        }
        save_config(state.enabled, &state.categories);
        Some(settings_markup(state.enabled, &state.categories))
    });

    // Redraw so the switches reflect what was persisted rather than only what
    // the widget animated to — they diverge if the write failed.
    if let Some(markup) = markup {
        action.commands.set_panel_content(SETTINGS_ID, &markup);
    }
}

// ── Persistence ──────────────────────────────────────────────────────────────

/// `%APPDATA%/renzora/tracy.json` on Windows, `~/.config/renzora/tracy.json`
/// elsewhere — the same path and format `renzora_tracy` used, so an existing
/// opt-in survives the move to a plugin.
fn config_path() -> Option<std::path::PathBuf> {
    let base = if cfg!(windows) {
        std::path::PathBuf::from(std::env::var_os("APPDATA")?)
    } else {
        std::path::PathBuf::from(std::env::var_os("HOME")?).join(".config")
    };
    Some(base.join("renzora").join("tracy.json"))
}

/// Read one `"key": true|false` out of the flat config.
///
/// Hand-parsed rather than with serde, because a plugin having zero
/// dependencies beyond `renzora_plugin` is the design and a handful of booleans
/// is not worth breaking it for. The file this writes is flat, one key per line,
/// so finding the key and reading the next word is the whole grammar.
fn read_flag(text: &str, key: &str, default: bool) -> bool {
    let needle = format!("\"{key}\"");
    match text
        .split_once(&needle)
        .and_then(|(_, rest)| rest.split_once(':'))
    {
        Some((_, v)) => v.trim_start().starts_with("true"),
        // Absent, not false. A config written before a category existed must
        // give that category its default rather than silently off — which for
        // the render timings would mean a profiler that came back from an
        // upgrade plotting nothing, with a settings panel insisting it was on.
        None => default,
    }
}

/// The opt-in and the per-category enables. A missing or unreadable file reads
/// as "off, with default categories" — defaulting a profiler to ON because its
/// config could not be parsed is the wrong direction to fail in.
fn load_config() -> (bool, Vec<bool>) {
    let defaults = || CATEGORIES.iter().map(|c| c.default).collect::<Vec<_>>();
    let Some(path) = config_path() else {
        return (false, defaults());
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return (false, defaults());
    };
    (
        read_flag(&text, "enabled", false),
        CATEGORIES
            .iter()
            .map(|c| read_flag(&text, c.key, c.default))
            .collect(),
    )
}

fn save_config(enabled: bool, categories: &[bool]) {
    let Some(path) = config_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut json = format!("{{\n  \"enabled\": {enabled}");
    for (i, cat) in CATEGORIES.iter().enumerate() {
        let on = categories.get(i).copied().unwrap_or(cat.default);
        json.push_str(&format!(",\n  \"{}\": {on}", cat.key));
    }
    json.push_str("\n}\n");
    let _ = std::fs::write(path, json);
}

// ── Plugin ───────────────────────────────────────────────────────────────────

pub struct TracyPlugin;

impl Plugin for TracyPlugin {
    fn build(&self, app: &mut App) {
        let markup = with(|s| settings_markup(s.enabled, &s.categories));
        app.add_settings_section(
            Panel::new(
                SETTINGS_ID,
                "Tracy Profiler",
                Scene(Box::leak(markup.into_boxed_str())),
            )
            .icon("pulse")
            .on_action(on_action),
        )
        .add_systems(Last, pump);
    }
}

renzora_plugin::add!(TracyPlugin, Editor);

#[cfg(test)]
mod tests {
    use super::*;

    /// Rebuild the config text and read every flag back. The round trip is the
    /// only thing keeping the hand-rolled parser honest against the writer.
    fn round_trip(enabled: bool, cats: &[bool]) -> (bool, Vec<bool>) {
        let mut json = format!("{{\n  \"enabled\": {enabled}");
        for (i, cat) in CATEGORIES.iter().enumerate() {
            let on = cats.get(i).copied().unwrap_or(cat.default);
            json.push_str(&format!(",\n  \"{}\": {on}", cat.key));
        }
        json.push_str("\n}\n");
        (
            read_flag(&json, "enabled", false),
            CATEGORIES
                .iter()
                .map(|c| read_flag(&json, c.key, c.default))
                .collect(),
        )
    }

    #[test]
    fn round_trips_every_flag() {
        let all_on = vec![true; CATEGORIES.len()];
        let all_off = vec![false; CATEGORIES.len()];
        assert_eq!(round_trip(true, &all_on), (true, all_on.clone()));
        assert_eq!(round_trip(false, &all_off), (false, all_off));
        // Mixed, so a writer that emitted a constant would fail here.
        let mixed: Vec<bool> = (0..CATEGORIES.len()).map(|i| i % 2 == 0).collect();
        assert_eq!(round_trip(true, &mixed), (true, mixed));
    }

    /// Anything unparseable is off. A profiler that armed itself because a
    /// config file was truncated would be a genuinely unpleasant surprise.
    #[test]
    fn garbage_reads_as_off() {
        for text in ["", "{}", "not json", "{\"enabled\":", "{\"enabled\": maybe}"] {
            assert!(!read_flag(text, "enabled", false), "{text:?} should be off");
        }
    }

    /// A key the file does not mention takes its DEFAULT, not `false`. This is
    /// the upgrade path: a config written before a category existed must not
    /// silently turn that category off, or a profiler that came back from an
    /// upgrade would plot nothing while its panel insisted everything was on.
    #[test]
    fn a_missing_category_takes_its_default() {
        let old = "{\n  \"enabled\": true\n}\n";
        for cat in CATEGORIES {
            assert_eq!(
                read_flag(old, cat.key, cat.default),
                cat.default,
                "{} should fall back to its default",
                cat.key
            );
        }
    }

    /// Every toggle must carry an action id, or the section renders switches
    /// that animate and report nothing.
    #[test]
    fn settings_markup_binds_every_toggle() {
        let cats = vec![true; CATEGORIES.len()];
        let m = settings_markup(true, &cats);
        assert!(m.contains(&format!("PanelActionId {{ action: {ACT_ENABLE} }}")));
        for (i, cat) in CATEGORIES.iter().enumerate() {
            let action = ACT_CATEGORY + i as i32;
            assert!(
                m.contains(&format!("PanelActionId {{ action: {action} }}")),
                "{} has no action id",
                cat.key
            );
            assert!(m.contains(cat.label), "{} has no label", cat.key);
        }
        // The master id must not collide with a category id, or one switch
        // would drive two settings.
        assert!(ACT_CATEGORY > ACT_ENABLE);
    }

    #[test]
    fn markup_reflects_the_toggle_states() {
        let cats = vec![false; CATEGORIES.len()];
        let m = settings_markup(false, &cats);
        assert!(!m.contains("on: true"), "everything was off");
        assert!(settings_markup(true, &vec![true; CATEGORIES.len()]).contains("on: true"));
    }

    /// Categorisation is first-match, so the specific render predicates must win
    /// over the catch-all — and each path must land where the label claims.
    #[test]
    fn paths_land_in_the_right_category() {
        let key = |p: &str| CATEGORIES[categorise(p)].key;
        assert_eq!(key("fps"), "frame");
        assert_eq!(key("frame_time"), "frame");
        assert_eq!(key("entity_count"), "entities");
        assert_eq!(key("system/cpu_usage"), "system");
        assert_eq!(key("process/mem_usage"), "system");
        assert_eq!(key("render/main_opaque_pass_3d/elapsed_gpu"), "render_gpu");
        assert_eq!(key("render/main_opaque_pass_3d/elapsed_cpu"), "render_cpu");
        // Every pipeline-statistics suffix a real capture actually carries,
        // taken from one: the first version of this matched only
        // `_invocations` and let five `_primitives_out` rows through into
        // `other`, where they were shown despite the group being off.
        for p in [
            "render/main_opaque_pass_3d/vertex_shader_invocations",
            "render/ui/fragment_shader_invocations",
            "render/taa/clipper_invocations",
            "render/bin_unpacking/compute_shader_invocations",
            "render/ui/clipper_primitives_out",
            "render/early prepass/clipper_primitives_out",
        ] {
            assert_eq!(key(p), "invocations", "{p} escaped the counter group");
        }
    }

    /// An unknown path is SHOWN, not dropped. The host's diagnostic set is open
    /// — any crate may register a path — and a filter that silently swallowed
    /// them would be indistinguishable from the engine having stopped measuring.
    #[test]
    fn an_unknown_path_falls_through_to_other() {
        assert_eq!(CATEGORIES[categorise("my_plugin/widgets_drawn")].key, "other");
        assert!(CATEGORIES.last().unwrap().default, "`other` must default on");
    }

    /// Growing a short config must use each category's own default. Filling with
    /// a blanket `true` would switch the counter group on behind the user's back,
    /// as a side effect of touching some unrelated switch.
    #[test]
    fn growing_a_short_config_uses_defaults() {
        let mut cats: Vec<bool> = Vec::new();
        while cats.len() < CATEGORIES.len() {
            cats.push(CATEGORIES[cats.len()].default);
        }
        let expected: Vec<bool> = CATEGORIES.iter().map(|c| c.default).collect();
        assert_eq!(cats, expected);
        assert!(!cats[CATEGORIES.iter().position(|c| c.key == "invocations").unwrap()]);
    }

    /// Category keys are the config's field names, so a duplicate would make one
    /// silently shadow the other on load.
    #[test]
    fn category_keys_are_unique() {
        for (i, a) in CATEGORIES.iter().enumerate() {
            for b in &CATEGORIES[i + 1..] {
                assert_ne!(a.key, b.key, "duplicate category key");
            }
        }
    }
}
