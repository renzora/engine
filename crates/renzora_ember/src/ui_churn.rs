//! A probe for UI work nobody asked for: once a second, which panel's nodes
//! changed in a way that makes Bevy lay the UI out again.
//!
//! Bevy's layout is incremental. It re-lays out a node only when one of a
//! handful of inputs changed (its `Node`, its measured text, its scroll
//! position, its transform or its children), and every such change dirties the
//! whole path up to the window root. A system that writes one of those every
//! frame, even with the same value through `DerefMut`, therefore costs a layout
//! pass every frame while the screen looks perfectly still. The profiler shows
//! the cost (`ui_layout_system`) but not who caused it; this names the panel
//! and the kind of change, which is the part you need to find the code.
//!
//! Off unless `RENZORA_UI_CHURN` is set when the app starts, and then it only
//! reads change ticks, so it never adds a change of its own. It reports through
//! the Console under the `UI` category.

use std::collections::HashMap;
use std::time::Duration;

use bevy::prelude::*;
use bevy::ui::{ContentSize, ScrollPosition, UiSystems, UiTransform};

use crate::dock::TabPane;

pub(crate) fn plugin(app: &mut App) {
    if std::env::var_os("RENZORA_UI_CHURN").is_none() {
        return;
    }
    app.init_resource::<Tally>()
        // After text is measured (`Content`, which writes `ContentSize`) and
        // before layout reads the changes, so each frame's causes are counted
        // in the frame they cost.
        .add_systems(PostUpdate, probe.after(UiSystems::Content).before(UiSystems::Layout));
}

/// Changes seen since the last report, keyed by (panel, kind of change).
#[derive(Resource, Default)]
struct Tally {
    frames: u32,
    since: Duration,
    counts: HashMap<(String, &'static str), Count>,
}

#[derive(Default)]
struct Count {
    changes: u32,
    /// One changed node, described, so the report points somewhere specific.
    example: Option<String>,
}

#[allow(clippy::too_many_arguments)]
fn probe(
    time: Res<Time<Real>>,
    mut tally: ResMut<Tally>,
    nodes: Query<(Entity, Ref<Node>)>,
    text_sizes: Query<Entity, (Changed<ContentSize>, With<Node>)>,
    scrolls: Query<Entity, (Changed<ScrollPosition>, With<Node>)>,
    transforms: Query<Entity, (Changed<UiTransform>, With<Node>)>,
    children: Query<Entity, (Changed<Children>, With<Node>)>,
    reparented: Query<Entity, (Changed<ChildOf>, With<Node>)>,
    fonts: Query<Entity, (Changed<TextFont>, With<Node>)>,
    mut removed: RemovedComponents<Node>,
    parents: Query<&ChildOf>,
    panes: Query<&TabPane>,
    names: Query<&Name>,
    texts: Query<&Text>,
) {
    let mut seen: Vec<(Entity, &'static str)> = Vec::new();
    for (entity, node) in &nodes {
        if node.is_added() {
            seen.push((entity, "spawned"));
        } else if node.is_changed() {
            seen.push((entity, "Node written"));
        }
    }
    seen.extend(text_sizes.iter().map(|e| (e, "text re-measured")));
    seen.extend(scrolls.iter().map(|e| (e, "scroll position")));
    seen.extend(transforms.iter().map(|e| (e, "UiTransform")));
    seen.extend(children.iter().map(|e| (e, "children changed")));
    seen.extend(reparented.iter().map(|e| (e, "reparented")));
    seen.extend(fonts.iter().map(|e| (e, "font changed")));

    for (entity, kind) in seen {
        let panel = panel_of(entity, &parents, &panes);
        let count = tally.counts.entry((panel, kind)).or_default();
        count.changes += 1;
        if count.example.is_none() {
            count.example = Some(describe(entity, &names, &texts));
        }
    }
    let despawned = removed.read().count() as u32;
    if despawned > 0 {
        tally.counts.entry(("(despawned)".into(), "Node removed")).or_default().changes += despawned;
    }

    tally.frames += 1;
    tally.since += time.delta();
    if tally.since < Duration::from_secs(1) {
        return;
    }
    report(&mut tally);
}

/// The id of the dock panel `entity` sits in, or `chrome` for everything
/// outside one (menus, the status bar, toolbars, floating UI).
fn panel_of(entity: Entity, parents: &Query<&ChildOf>, panes: &Query<&TabPane>) -> String {
    let mut at = entity;
    loop {
        if let Ok(pane) = panes.get(at) {
            return pane.id.clone();
        }
        match parents.get(at) {
            Ok(child_of) => at = child_of.parent(),
            Err(_) => return "chrome".into(),
        }
    }
}

/// What a person would recognise the node by: its text if it has some, else
/// its name, else just its id.
fn describe(entity: Entity, names: &Query<&Name>, texts: &Query<&Text>) -> String {
    if let Ok(text) = texts.get(entity) {
        let mut shown: String = text.0.chars().take(32).collect();
        if text.0.chars().count() > 32 {
            shown.push('…');
        }
        return format!("{entity} \"{shown}\"");
    }
    match names.get(entity) {
        Ok(name) => format!("{entity} {name}"),
        Err(_) => format!("{entity}"),
    }
}

fn report(tally: &mut Tally) {
    let frames = tally.frames.max(1);
    let mut rows: Vec<_> = tally.counts.drain().collect();
    rows.sort_by_key(|row| std::cmp::Reverse(row.1.changes));
    if rows.is_empty() {
        renzora::console_log::console_info("UI", format!("[ui-churn] nothing changed over {frames} frames"));
    } else {
        renzora::console_log::console_info(
            "UI",
            format!("[ui-churn] {} kinds of change over {frames} frames, busiest first:", rows.len()),
        );
        for ((panel, kind), count) in rows.into_iter().take(12) {
            let per_frame = count.changes as f32 / frames as f32;
            renzora::console_log::console_info(
                "UI",
                format!(
                    "[ui-churn]   {panel}: {kind}, {per_frame:.1}/frame, e.g. {}",
                    count.example.unwrap_or_default()
                ),
            );
        }
    }
    tally.frames = 0;
    tally.since = Duration::ZERO;
}
