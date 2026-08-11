//! UI Layout panel — where bevy_ui's per-frame cost actually goes.
//!
//! This exists because of a concrete misdiagnosis. Opening the inspector's
//! component sections cost ~3.3 ms/frame (60 fps → 50 fps), and the obvious
//! suspect was reactivity — "the inspector must be recomputing every frame".
//! It was not: the whole reactive layer measured 0.23 ms, and only four
//! bindings produced a new value that frame. Something else was spending 3.3 ms,
//! and no panel could say what.
//!
//! Opening a section does not add binding work. It adds *visible nodes*, and
//! bevy_ui charges for those whether or not anything about them changed:
//! `ui_layout_system` walks the tree unconditionally, and text measurement runs
//! for every visible label before it. So the number worth watching is not "how
//! many bindings ran" but "how many nodes are on screen and what does laying
//! them out cost".
//!
//! ## How the timings are taken
//!
//! Bevy exposes no per-system timing without a Tracy build, so this brackets
//! the UI pipeline with three timestamps around the public system sets:
//!
//! ```text
//!   A ── UiSystems::Prepare ‥ Propagate ‥ Content ── B ── Layout ── C
//!        └──────── content (text measurement) ──────┘   └─ taffy ─┘
//! ```
//!
//! `content` is where glyph shaping and intrinsic sizing happen — the half that
//! collapsing a section actually skips. `layout` is taffy solving the tree.
//! Splitting them matters: they have completely different fixes. Text-bound
//! means fewer/cheaper labels; taffy-bound means fewer nodes.
//!
//! The spans cover everything scheduled in those sets, not solely bevy's own
//! systems — which is the honest thing to report, since a plugin's layout-time
//! system costs the frame just as much.
//!
//! ## Counting is gated
//!
//! Walking every `Node` to report totals would itself cost frame time in a
//! panel about frame time. The counts refresh only while this panel is the
//! active tab, and then only every 30 frames.

use std::time::Instant;

use bevy::diagnostic::{Diagnostic, DiagnosticPath, Diagnostics, RegisterDiagnostic};
use bevy::prelude::*;
use bevy::ui::UiSystems;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::Rx;
use renzora_ember::theme::{rgb, text_muted, text_primary};
use renzora_ember::widgets::{line_chart_live, ChartStyle};

use super::camera::faint_box;
use super::{big_stat, label_row, section};

pub(super) const PANEL: &str = "ui_layout";

/// Live UI-pipeline cost. Read by this panel; available to any system.
#[derive(Resource, Default)]
pub struct UiLayoutStats {
    /// `Prepare` → `Content`: propagation and text measurement, ms.
    pub content_ms: f32,
    /// `Content` → `Layout`: taffy solving the tree, ms.
    pub layout_ms: f32,
    /// Nodes in the world, refreshed while this panel is open.
    pub nodes_total: usize,
    /// Nodes whose own `display` is `None`. They still cost a tree walk, but
    /// not text measurement — which is why collapsing a section helps.
    pub nodes_hidden: usize,
    /// Nodes carrying `Text`. Measurement cost scales with these, not with
    /// the raw node count — a panel of labels is dearer than a panel of boxes.
    pub text_nodes: usize,
    /// Of those, the ones not hidden — the population text measurement
    /// actually runs over, and the number that drops when a section collapses.
    pub text_nodes_visible: usize,
    /// Recent `content + layout` per frame, oldest → newest.
    pub history_ms: Vec<f32>,
    /// Internal: start of the current frame's UI pipeline.
    start: Option<Instant>,
    /// Internal: the `Content`/`Layout` boundary.
    mid: Option<Instant>,
    frame: u64,
}

impl UiLayoutStats {
    pub const HISTORY_LEN: usize = 240;

    pub fn total_ms(&self) -> f32 {
        self.content_ms + self.layout_ms
    }
}

pub(super) fn register_ui_layout(app: &mut App) {
    app.init_resource::<UiLayoutStats>();
    // panel-systems-ungated: these bracket Bevy's own UI sets to time them. The
    // measurement has to span the layout run itself, which happens whether or
    // not this panel is open — and the numbers feed the diagnostic store below,
    // whose whole point is being recorded while nobody is looking.
    app.add_systems(
        PostUpdate,
        (
            mark_start.before(UiSystems::Prepare),
            // `Content` is the last set before layout, so "after Content" is
            // the boundary between measurement and solving.
            mark_mid.after(UiSystems::Content).before(UiSystems::Layout),
            mark_end.after(UiSystems::Layout),
        ),
    );
    // Counting walks every node, so it runs last and only when observed —
    // `count_nodes` checks that for itself rather than being gated here.
    // panel-systems-ungated: self-gating, and it feeds the diagnostics below.
    app.add_systems(Last, count_nodes);
    app.register_panel_content(PANEL, true, build);

    // Publish into Bevy's diagnostic store as well as the panel, so the split
    // can be watched over time instead of only while staring at it — the Tracy
    // bridge (`plugins/tracy`) plots every diagnostic it finds, and a spike that
    // lasts one frame is exactly the kind this panel's live readout misses.
    for path in [
        &CONTENT_MS,
        &LAYOUT_MS,
        &NODES_TOTAL,
        &NODES_HIDDEN,
        &TEXT_NODES,
        &TEXT_NODES_VISIBLE,
    ] {
        app.register_diagnostic(Diagnostic::new(path.clone()));
    }
    // panel-systems-ungated: publishing into the diagnostic store is exactly
    // the part that must NOT stop when the panel is hidden — a spike that lasts
    // one frame is what this exists to catch, and the Tracy bridge plots it
    // whether or not anyone has the panel open.
    app.add_systems(Last, publish_ui_layout_diagnostics.after(count_nodes));
}

/// `Prepare` → `Content`: propagation and text measurement, ms.
pub const CONTENT_MS: DiagnosticPath = DiagnosticPath::const_new("ui/content_ms");
/// `Content` → `Layout`: taffy solving the tree, ms.
pub const LAYOUT_MS: DiagnosticPath = DiagnosticPath::const_new("ui/layout_ms");
/// Live `Node` count.
pub const NODES_TOTAL: DiagnosticPath = DiagnosticPath::const_new("ui/nodes_total");
/// Nodes at `Display::None`. Against `nodes_total` this is the share of the UI
/// tree that exists but is never drawn — mostly widget chrome (carets, selection
/// highlights, slider rails, popovers) built eagerly and hidden for its whole
/// life. It is the ceiling on what lazy chrome could remove.
pub const NODES_HIDDEN: DiagnosticPath = DiagnosticPath::const_new("ui/nodes_hidden");
/// Live `Node` + `Text` count — what text measurement scales with.
pub const TEXT_NODES: DiagnosticPath = DiagnosticPath::const_new("ui/text_nodes");
/// Of those, the ones actually visible — the population text measurement runs
/// over.
pub const TEXT_NODES_VISIBLE: DiagnosticPath =
    DiagnosticPath::const_new("ui/text_nodes_visible");

/// Copy the pipeline split and the node counts into the diagnostic store.
///
/// The two timings are free: `mark_start`/`mark_mid`/`mark_end` already run
/// every frame whether or not the panel is open, so this only forwards numbers
/// that exist.
///
/// The two totals come straight from the queries because `iter().len()` is
/// answered from archetype metadata without touching an entity — free every
/// frame. The two filtered counts come from [`UiLayoutStats`], refreshed by
/// [`count_nodes`] every 30th frame, because those are O(entities) and not worth
/// paying for at full rate.
fn publish_ui_layout_diagnostics(
    mut diags: Diagnostics,
    stats: Res<UiLayoutStats>,
    nodes: Query<&Node>,
    texts: Query<&Node, With<Text>>,
) {
    diags.add_measurement(&CONTENT_MS, || stats.content_ms as f64);
    diags.add_measurement(&LAYOUT_MS, || stats.layout_ms as f64);
    diags.add_measurement(&NODES_TOTAL, || nodes.iter().len() as f64);
    diags.add_measurement(&TEXT_NODES, || texts.iter().len() as f64);
    diags.add_measurement(&NODES_HIDDEN, || stats.nodes_hidden as f64);
    diags.add_measurement(&TEXT_NODES_VISIBLE, || stats.text_nodes_visible as f64);
}

fn mark_start(mut stats: ResMut<UiLayoutStats>) {
    // Deliberately NOT `bypass_change_detection`.
    //
    // The instinct is to bypass: these fields are written every frame and
    // bumping the tick dirties the bindings that read them. But those bindings
    // are dependency-tracked now, and a dep that never goes dirty is a binding
    // that never runs — bypassing here froze this panel on its initial zeroes
    // while the numbers behind it updated fine.
    //
    // A live profiler is meant to refresh every frame, so the dirty is the
    // correct outcome, and it costs the handful of bindings this panel owns.
    // The general rule the gate introduces: `bypass_change_detection` on a
    // resource any binding reads is now a staleness bug, not an optimisation.
    let s = &mut *stats;
    s.frame = s.frame.wrapping_add(1);
    s.start = Some(Instant::now());
    s.mid = None;
}

fn mark_mid(mut stats: ResMut<UiLayoutStats>) {
    stats.mid = Some(Instant::now());
}

fn mark_end(mut stats: ResMut<UiLayoutStats>) {
    let s = &mut *stats;
    let now = Instant::now();
    let (Some(start), Some(mid)) = (s.start, s.mid) else {
        return;
    };
    s.content_ms = mid.duration_since(start).as_secs_f32() * 1e3;
    s.layout_ms = now.duration_since(mid).as_secs_f32() * 1e3;

    let total = s.content_ms + s.layout_ms;
    if s.history_ms.len() >= UiLayoutStats::HISTORY_LEN {
        s.history_ms.remove(0);
    }
    s.history_ms.push(total);
}

/// Refresh the node counts.
///
/// Two of the four are `filter().count()` and so O(entities), which is why this
/// is throttled to every 30th frame — over a few thousand nodes that amortises
/// to well under a hundred visits a frame.
///
/// It used to *also* require the panel to be open. That gate is gone because the
/// counts are now published as diagnostics, and the hidden-node fraction is one
/// of the more useful numbers the editor produces: it is the size of the prize
/// for spawning widget chrome lazily, and you cannot see it in a capture if it
/// only exists while a panel is being looked at.
fn count_nodes(
    mut stats: ResMut<UiLayoutStats>,
    nodes: Query<&Node>,
    texts: Query<&Node, With<Text>>,
) {
    if !stats.frame.is_multiple_of(30) {
        return;
    }
    let s = &mut *stats;
    s.nodes_total = nodes.iter().len();
    s.nodes_hidden = nodes.iter().filter(|n| n.display == Display::None).count();
    s.text_nodes = texts.iter().len();
    s.text_nodes_visible = texts.iter().filter(|n| n.display != Display::None).count();
}

fn ul<R: Default>(w: &Rx, f: impl FnOnce(&UiLayoutStats) -> R) -> R {
    w.get_resource::<UiLayoutStats>().map(f).unwrap_or_default()
}

/// Green under a millisecond, amber to four, red past that — the same scale the
/// reactivity panel uses, so the two are directly comparable at a glance. That
/// comparison is the point: it is what tells you which of the two is worth
/// optimising.
fn cost_color(ms: f32) -> Color {
    if ms > 4.0 {
        rgb((230, 110, 110))
    } else if ms > 1.0 {
        rgb((230, 200, 110))
    } else {
        rgb((120, 210, 120))
    }
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = super::root(commands);

    let total = big_stat(
        commands,
        fonts,
        "ms/frame UI layout",
        |w| ul(w, |s| format!("{:.2}", s.total_ms())),
        |w| cost_color(ul(w, |s| s.total_ms())),
    );
    let nodes = big_stat(
        commands,
        fonts,
        "ui nodes",
        |w| ul(w, |s| s.nodes_total).to_string(),
        |_| rgb(text_primary()),
    );
    let chart = line_chart_live(
        commands,
        ChartStyle {
            color: rgb((230, 170, 110)),
            min: Some(0.0),
            max: None,
            target: None,
            height: 40.0,
        },
        |w| ul(w, |s| s.history_ms.clone()),
    );

    let split_label = section(commands, fonts, "Where The Time Goes");
    let split = faint_box(commands);
    let split_rows = [
        // Text measurement vs taffy: the two halves have different fixes, so
        // the split is the actionable part of this panel.
        label_row(commands, fonts, "Content (text measure)", |w| {
            ul(w, |s| format!("{:.2} ms", s.content_ms))
        }),
        label_row(commands, fonts, "Layout (taffy)", |w| {
            ul(w, |s| format!("{:.2} ms", s.layout_ms))
        }),
    ];
    commands.entity(split).add_children(&split_rows);

    let counts_label = section(commands, fonts, "Node Census");
    let counts = faint_box(commands);
    let count_rows = [
        label_row(commands, fonts, "Nodes total", |w| {
            ul(w, |s| s.nodes_total).to_string()
        }),
        label_row(commands, fonts, "Hidden (display: none)", |w| {
            ul(w, |s| s.nodes_hidden).to_string()
        }),
        label_row(commands, fonts, "Text nodes", |w| {
            ul(w, |s| s.text_nodes).to_string()
        }),
        // The ratio that predicts the cost: measurement scales with visible
        // text, not with the raw node count.
        label_row(commands, fonts, "Text nodes visible", |w| {
            ul(w, |s| s.text_nodes_visible).to_string()
        }),
    ];
    commands.entity(counts).add_children(&count_rows);

    let note = commands
        .spawn((
            Text::new(
                "Counts refresh every 30 frames while this tab is open.\n\
                 Compare `ms/frame UI layout` against UI Reactivity's \
                 `ms/frame recompute` — whichever is larger is the one worth \
                 optimising. Opening a panel adds nodes, not bindings.",
            ),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
            Node {
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            },
        ))
        .id();

    commands.entity(root).add_children(&[
        total,
        nodes,
        chart,
        split_label,
        split,
        counts_label,
        counts,
        note,
    ]);
    root
}
