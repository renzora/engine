//! Bevy-native Physics Debug panel — status, rigid-body counts (with a stacked
//! proportion bar), colliders by type, step-time graph, a collapsible collision-
//! pairs list, and the debug-visualization toggles. The toggles are two-way bound
//! straight to `PhysicsDebugState` (no DebugBridge).

use std::cmp::Reverse;
use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_2way, bind_display, bind_text, bind_text_color, bind_with, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{checkbox, collapsible, line_chart_live, ChartStyle};

use crate::state::{ColliderShapeType, PhysicsDebugState};

const SECONDARY: (u8, u8, u8) = (170, 170, 180);
const FAINT_BG: (u8, u8, u8) = (30, 30, 36);
const TRACK_BG: (u8, u8, u8) = (18, 18, 24);
const DYNAMIC: (u8, u8, u8) = (100, 180, 220);
const KINEMATIC: (u8, u8, u8) = (200, 180, 100);
const STATIC: (u8, u8, u8) = (150, 150, 160);

pub fn register_native_physics_debug(app: &mut App) {
    app.register_panel_content("physics_debug", true, build);
}

fn phys<R: Default>(w: &World, f: impl FnOnce(&PhysicsDebugState) -> R) -> R {
    w.get_resource::<PhysicsDebugState>().map(f).unwrap_or_default()
}
fn set_phys(w: &mut World, f: impl FnOnce(&mut PhysicsDebugState)) {
    if let Some(mut s) = w.get_resource_mut::<PhysicsDebugState>() {
        f(&mut s);
    }
}

fn shape_color(shape: ColliderShapeType) -> (u8, u8, u8) {
    match shape {
        ColliderShapeType::Sphere => (100, 180, 220),
        ColliderShapeType::Box => (180, 140, 200),
        ColliderShapeType::Capsule => (140, 200, 140),
        ColliderShapeType::Cylinder => (200, 160, 100),
        ColliderShapeType::Cone => (200, 120, 120),
        ColliderShapeType::ConvexHull => (120, 180, 180),
        ColliderShapeType::TriMesh => (180, 180, 120),
        ColliderShapeType::HeightField => (120, 140, 180),
        ColliderShapeType::Compound => (180, 140, 160),
        ColliderShapeType::Unknown => (120, 120, 120),
    }
}
fn step_color(ms: f32) -> Color {
    if ms <= 2.0 {
        rgb((100, 200, 100))
    } else if ms <= 5.0 {
        rgb((200, 200, 100))
    } else if ms <= 10.0 {
        rgb((200, 150, 80))
    } else {
        rgb((200, 100, 100))
    }
}

// ── Small builders ───────────────────────────────────────────────────────────

fn root(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            padding: UiRect::all(Val::Px(8.0)),
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id()
}
fn section(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
            Node { margin: UiRect::top(Val::Px(8.0)), ..default() },
        ))
        .id()
}
fn faint_box(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(8.0)),
                row_gap: Val::Px(3.0),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(FAINT_BG)),
        ))
        .id()
}
fn text(commands: &mut Commands, fonts: &EmberFonts, s: &str, size: f32, color: (u8, u8, u8)) -> Entity {
    commands
        .spawn((Text::new(s), ui_font(&fonts.ui, size), TextColor(rgb(color))))
        .id()
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = root(commands);

    // Unavailable placeholder.
    let none = commands
        .spawn((
            Text::new("No physics bodies detected"),
            ui_font(&fonts.ui, 14.0),
            TextColor(rgb(text_muted())),
            Node { margin: UiRect::all(Val::Px(40.0)), ..default() },
        ))
        .id();
    bind_display(commands, none, |w| !phys(w, |s| s.physics_available));

    let content = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0),
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    bind_display(commands, content, |w| phys(w, |s| s.physics_available));

    let mut kids: Vec<Entity> = Vec::new();

    // Status.
    let status = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() })
        .id();
    let st_label = text(commands, fonts, "Physics", 12.0, text_muted());
    let dot = text(commands, fonts, "\u{25cf}", 11.0, (150, 200, 120));
    bind_text_color(commands, dot, |w| if phys(w, |s| s.simulation_running) { rgb((120, 210, 120)) } else { rgb((220, 180, 80)) });
    let st_text = text(commands, fonts, "", 11.0, SECONDARY);
    bind_text(commands, st_text, |w| if phys(w, |s| s.simulation_running) { "Running".into() } else { "Paused".into() });
    bind_text_color(commands, st_text, |w| if phys(w, |s| s.simulation_running) { rgb((120, 210, 120)) } else { rgb((220, 180, 80)) });
    commands.entity(status).add_children(&[st_label, dot, st_text]);
    kids.push(status);

    // Rigid bodies.
    kids.push(section(commands, fonts, "Rigid Bodies"));
    kids.push(big(commands, fonts, "total", |w| phys(w, |s| s.total_body_count()).to_string()));
    let legend = commands
        .spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(12.0), ..default() })
        .id();
    let ld = dot_legend(commands, fonts, DYNAMIC, "Dynamic", |w| phys(w, |s| s.dynamic_body_count));
    let lk = dot_legend(commands, fonts, KINEMATIC, "Kinematic", |w| phys(w, |s| s.kinematic_body_count));
    let ls = dot_legend(commands, fonts, STATIC, "Static", |w| phys(w, |s| s.static_body_count));
    commands.entity(legend).add_children(&[ld, lk, ls]);
    kids.push(legend);
    kids.push(stacked_bar(commands));

    // Colliders.
    kids.push(section(commands, fonts, "Colliders"));
    kids.push(big(commands, fonts, "total", |w| phys(w, |s| s.collider_count).to_string()));
    let by_type = commands
        .spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() })
        .id();
    keyed_list(commands, by_type, colliders_snapshot);
    kids.push(by_type);

    // Step time.
    kids.push(section(commands, fonts, "Step Time"));
    let st_row = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::FlexEnd, column_gap: Val::Px(6.0), ..default() })
        .id();
    let st_big = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 20.0), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, st_big, |w| format!("{:.2}", phys(w, |s| s.step_time_ms)));
    bind_text_color(commands, st_big, |w| step_color(phys(w, |s| s.step_time_ms)));
    let st_ms = text(commands, fonts, "ms", 11.0, text_muted());
    let st_avg = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 10.0), TextColor(rgb(SECONDARY)), Node { margin: UiRect::bottom(Val::Px(2.0)), ..default() }))
        .id();
    bind_text(commands, st_avg, |w| format!("avg: {:.2}ms", phys(w, |s| s.avg_step_time_ms)));
    commands.entity(st_row).add_children(&[st_big, st_ms, st_avg]);
    kids.push(st_row);
    kids.push(line_chart_live(
        commands,
        ChartStyle { color: rgb((100, 200, 100)), min: Some(0.0), max: None, target: None, height: 40.0 },
        |w| phys(w, |s| s.step_time_history.iter().copied().collect()),
    ));

    // Collision pairs (collapsible).
    let (cp, cp_body) = collapsible(commands, fonts, None, "Collision Pairs", false);
    keyed_list(commands, cp_body, collision_snapshot);
    kids.push(cp);

    // Debug visualization toggles.
    kids.push(section(commands, fonts, "Debug Visualization"));
    let toggles = faint_box(commands);
    let t = [
        toggle_row(commands, fonts, "Show Colliders", |w| phys(w, |s| s.debug_toggles.show_colliders), |w, v| set_phys(w, move |s| s.debug_toggles.show_colliders = v)),
        toggle_row(commands, fonts, "Show Contacts", |w| phys(w, |s| s.debug_toggles.show_contacts), |w, v| set_phys(w, move |s| s.debug_toggles.show_contacts = v)),
        toggle_row(commands, fonts, "Show AABBs", |w| phys(w, |s| s.debug_toggles.show_aabbs), |w, v| set_phys(w, move |s| s.debug_toggles.show_aabbs = v)),
        toggle_row(commands, fonts, "Show Velocities", |w| phys(w, |s| s.debug_toggles.show_velocities), |w, v| set_phys(w, move |s| s.debug_toggles.show_velocities = v)),
        toggle_row(commands, fonts, "Show Center of Mass", |w| phys(w, |s| s.debug_toggles.show_center_of_mass), |w, v| set_phys(w, move |s| s.debug_toggles.show_center_of_mass = v)),
        toggle_row(commands, fonts, "Show Joints", |w| phys(w, |s| s.debug_toggles.show_joints), |w, v| set_phys(w, move |s| s.debug_toggles.show_joints = v)),
    ];
    commands.entity(toggles).add_children(&t);
    kids.push(toggles);

    commands.entity(content).add_children(&kids);
    commands.entity(root).add_children(&[none, content]);
    root
}

fn big<V>(commands: &mut Commands, fonts: &EmberFonts, unit: &str, value: V) -> Entity
where
    V: Fn(&World) -> String + Send + Sync + 'static,
{
    let row = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::FlexEnd, column_gap: Val::Px(6.0), ..default() })
        .id();
    let num = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 24.0), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, num, value);
    let u = commands
        .spawn((Text::new(unit), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), Node { margin: UiRect::bottom(Val::Px(3.0)), ..default() }))
        .id();
    commands.entity(row).add_children(&[num, u]);
    row
}

fn dot_legend<C>(commands: &mut Commands, fonts: &EmberFonts, color: (u8, u8, u8), label: &str, count: C) -> Entity
where
    C: Fn(&World) -> usize + Send + Sync + 'static,
{
    let row = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), ..default() })
        .id();
    let dot = text(commands, fonts, "\u{25cf}", 10.0, color);
    let label = label.to_string();
    let t = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 10.0), TextColor(rgb(SECONDARY))))
        .id();
    bind_text(commands, t, move |w| format!("{}: {}", label, count(w)));
    commands.entity(row).add_children(&[dot, t]);
    row
}

/// Stacked dynamic/kinematic/static proportion bar (each segment width is a
/// live binding).
fn stacked_bar(commands: &mut Commands) -> Entity {
    let track = commands
        .spawn((
            Node {
                width: Val::Px(250.0),
                height: Val::Px(16.0),
                flex_direction: FlexDirection::Row,
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                margin: UiRect::top(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(TRACK_BG)),
        ))
        .id();
    let seg = |commands: &mut Commands, color: (u8, u8, u8), pick: fn(&PhysicsDebugState) -> usize| -> Entity {
        let s = commands
            .spawn((Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(rgb(color))))
            .id();
        bind_with(commands, s, move |w| {
            phys(w, |st| {
                let total = st.total_body_count().max(1) as f32;
                pick(st) as f32 / total * 100.0
            })
        }, |w, e, pct: &f32| {
            if let Some(mut n) = w.get_mut::<Node>(e) {
                n.width = Val::Percent(*pct);
            }
        });
        s
    };
    let d = seg(commands, DYNAMIC, |s| s.dynamic_body_count);
    let k = seg(commands, KINEMATIC, |s| s.kinematic_body_count);
    let st = seg(commands, STATIC, |s| s.static_body_count);
    commands.entity(track).add_children(&[d, k, st]);
    track
}

fn toggle_row<G, S>(commands: &mut Commands, fonts: &EmberFonts, label: &str, get: G, set_fn: S) -> Entity
where
    G: Fn(&World) -> bool + Send + Sync + 'static,
    S: Fn(&mut World, bool) + Send + Sync + 'static,
{
    let row = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() })
        .id();
    let cb = checkbox(commands, false);
    bind_2way(commands, cb, get, move |w, v| set_fn(w, *v));
    let l = text(commands, fonts, label, 11.0, text_primary());
    commands.entity(row).add_children(&[cb, l]);
    row
}

// ── Lists ────────────────────────────────────────────────────────────────────

fn colliders_snapshot(world: &World) -> KeyedSnapshot {
    let mut v: Vec<(ColliderShapeType, usize)> =
        phys(world, |s| s.colliders_by_type.iter().map(|(k, c)| (*k, *c)).collect());
    v.sort_by_key(|(_, c)| Reverse(*c));
    v.truncate(6);
    let items: Vec<(u64, u64)> = v
        .iter()
        .map(|(shape, count)| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            format!("{}", shape).hash(&mut h);
            (h.finish(), *count as u64)
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (shape, count) = v[i];
            let row = c
                .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), ..default() })
                .id();
            let sq = c
                .spawn((Text::new("\u{25a0}"), ui_font(&f.ui, 10.0), TextColor(rgb(shape_color(shape)))))
                .id();
            let t = c
                .spawn((Text::new(format!("{}: {}", shape, count)), ui_font(&f.ui, 10.0), TextColor(rgb(SECONDARY))))
                .id();
            c.entity(row).add_children(&[sq, t]);
            row
        }),
    }
}

fn collision_snapshot(world: &World) -> KeyedSnapshot {
    let pairs = phys(world, |s| s.collision_pairs.iter().take(10).map(|p| (p.entity_a, p.entity_b, p.contact_count)).collect::<Vec<_>>());
    let total = phys(world, |s| s.collision_pair_count);
    if pairs.is_empty() {
        return KeyedSnapshot {
            items: vec![(u64::MAX, 0)],
            build: Box::new(|c, f, _| {
                c.spawn((Text::new("No active collisions"), ui_font(&f.ui, 10.0), TextColor(rgb(text_muted())))).id()
            }),
        };
    }
    let mut items: Vec<(u64, u64)> = pairs
        .iter()
        .enumerate()
        .map(|(i, (a, b, n))| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (a.to_bits(), b.to_bits(), *n).hash(&mut h);
            (i as u64, h.finish())
        })
        .collect();
    if total > 10 {
        items.push((u64::MAX - 1, total as u64));
    }
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            if i < pairs.len() {
                let (a, b, n) = pairs[i];
                let row = c
                    .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), ..default() })
                    .id();
                let ea = c.spawn((Text::new(format!("{:?}", a)), ui_font(&f.mono, 9.0), TextColor(rgb(SECONDARY)))).id();
                let arrow = c.spawn((Text::new("\u{2194}"), ui_font(&f.ui, 9.0), TextColor(rgb(text_muted())))).id();
                let eb = c.spawn((Text::new(format!("{:?}", b)), ui_font(&f.mono, 9.0), TextColor(rgb(SECONDARY)))).id();
                let nc = c.spawn((Text::new(format!("({} contacts)", n)), ui_font(&f.ui, 9.0), TextColor(rgb(text_muted())))).id();
                c.entity(row).add_children(&[ea, arrow, eb, nc]);
                row
            } else {
                c.spawn((Text::new(format!("... and {} more", total.saturating_sub(10))), ui_font(&f.ui, 9.0), TextColor(rgb(text_muted())))).id()
            }
        }),
    }
}
