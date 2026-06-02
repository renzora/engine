//! Bevy-native ECS Stats panel — entity count + history graph and three
//! collapsible lists (archetypes, component types, resources). The lists are
//! reactive `keyed_list`s; the collapse state lives in [`EcsExpanded`] (a
//! separate resource so `update_ecs_stats` can't clobber it).

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_ember::font::{icon_glyph, icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_text, keyed_list, KeyedSnapshot};
use renzora_ember::theme::{rgb, TEXT_MUTED, TEXT_PRIMARY};
use renzora_ember::widgets::{line_chart_live, ChartStyle};
use renzora_editor::SplashState;

use crate::state::EcsStatsState;

use super::{big_stat, root, section};

const FAINT_BG: (u8, u8, u8) = (30, 30, 36);
const SECONDARY: (u8, u8, u8) = (170, 170, 180);

/// Which collapsible section a header toggles.
#[derive(Component, Clone, Copy, PartialEq)]
enum Section {
    Archetypes,
    Components,
    Resources,
}

/// Per-section collapse state (independent of the egui panel's `show_*` flags,
/// which live in its private RwLock copy).
#[derive(Resource, Default)]
struct EcsExpanded {
    archetypes: bool,
    components: bool,
    resources: bool,
}

impl Section {
    fn get(self, e: &EcsExpanded) -> bool {
        match self {
            Section::Archetypes => e.archetypes,
            Section::Components => e.components,
            Section::Resources => e.resources,
        }
    }
    fn toggle(self, e: &mut EcsExpanded) {
        match self {
            Section::Archetypes => e.archetypes = !e.archetypes,
            Section::Components => e.components = !e.components,
            Section::Resources => e.resources = !e.resources,
        }
    }
}

pub(super) fn register_ecs_stats(app: &mut App) {
    app.init_resource::<EcsExpanded>();
    app.register_panel_content("ecs_stats", true, build_ecs_stats);
    app.add_systems(Update, toggle_click.run_if(in_state(SplashState::Editor)));
}

fn ecs<R: Default>(w: &World, f: impl FnOnce(&EcsStatsState) -> R) -> R {
    w.get_resource::<EcsStatsState>().map(f).unwrap_or_default()
}

fn expanded(w: &World, s: Section) -> bool {
    w.get_resource::<EcsExpanded>().map(|e| s.get(e)).unwrap_or(false)
}

fn short_name(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or(name)
}

fn hash_str(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

// ── Build ────────────────────────────────────────────────────────────────────

fn build_ecs_stats(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = root(commands);

    let ent_label = section(commands, fonts, "Entities");
    let ent_big = big_stat(
        commands,
        fonts,
        "entities",
        |w| format!("{}", ecs(w, |s| s.entity_count)),
        |_| rgb(TEXT_PRIMARY),
    );

    // "Archetypes: N    Component Types: M"
    let sub = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let arch_k = small(commands, fonts, "Archetypes:", SECONDARY);
    let arch_v = bound(commands, fonts, TEXT_PRIMARY, |w| format!("{}", ecs(w, |s| s.archetype_count)));
    let comp_k = small(commands, fonts, "Component Types:", SECONDARY);
    let comp_v = bound(commands, fonts, TEXT_PRIMARY, |w| {
        format!("{}", ecs(w, |s| s.component_stats.len()))
    });
    commands.entity(sub).add_children(&[arch_k, arch_v, comp_k, comp_v]);

    let chart = line_chart_live(
        commands,
        ChartStyle {
            color: rgb((100, 180, 220)),
            min: Some(0.0),
            max: None,
            target: None,
            height: 50.0,
        },
        |w| ecs(w, |s| s.entity_count_history.iter().copied().collect()),
    );

    let arch = collapsible(
        commands,
        fonts,
        Section::Archetypes,
        |w| format!("Archetypes ({})", ecs(w, |s| s.archetype_count)),
        archetypes_snapshot,
    );
    let comp = collapsible(
        commands,
        fonts,
        Section::Components,
        |w| format!("Component Types ({})", ecs(w, |s| s.component_stats.len())),
        components_snapshot,
    );
    let res = collapsible(
        commands,
        fonts,
        Section::Resources,
        |w| format!("Resources ({})", ecs(w, |s| s.resources.len())),
        resources_snapshot,
    );

    commands
        .entity(root)
        .add_children(&[ent_label, ent_big, sub, chart, arch, comp, res]);
    root
}

fn small(commands: &mut Commands, fonts: &EmberFonts, text: &str, color: (u8, u8, u8)) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(color)),
        ))
        .id()
}

fn bound<T>(commands: &mut Commands, fonts: &EmberFonts, color: (u8, u8, u8), value: T) -> Entity
where
    T: Fn(&World) -> String + Send + Sync + 'static,
{
    let e = small(commands, fonts, "", color);
    bind_text(commands, e, value);
    e
}

/// A collapsible section: a clickable caret+title header over a faint-backed
/// body whose rows are a reactive `keyed_list` (empty + hidden when collapsed).
fn collapsible<F, S>(commands: &mut Commands, fonts: &EmberFonts, sec: Section, title: F, snapshot: S) -> Entity
where
    F: Fn(&World) -> String + Send + Sync + 'static,
    S: Fn(&World) -> KeyedSnapshot + Send + Sync + 'static,
{
    let col = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            margin: UiRect::top(Val::Px(8.0)),
            ..default()
        })
        .id();

    let header = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            },
            Interaction::default(),
            EcsToggle(sec),
            Name::new("ecs-section-header"),
        ))
        .id();
    let caret = icon_text(commands, &fonts.phosphor, "caret-right", TEXT_MUTED, 12.0);
    bind_text(commands, caret, move |w| {
        let name = if expanded(w, sec) { "caret-down" } else { "caret-right" };
        icon_glyph(name).unwrap_or(' ').to_string()
    });
    let title_e = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();
    bind_text(commands, title_e, title);
    commands.entity(header).add_children(&[caret, title_e]);

    let body = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(8.0)),
                row_gap: Val::Px(2.0),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(FAINT_BG)),
            Name::new("ecs-section-body"),
        ))
        .id();
    bind_display(commands, body, move |w| expanded(w, sec));
    keyed_list(commands, body, snapshot);

    commands.entity(col).add_children(&[header, body]);
    col
}

#[derive(Component)]
struct EcsToggle(Section);

fn toggle_click(
    q: Query<(&Interaction, &EcsToggle), Changed<Interaction>>,
    mut expanded: ResMut<EcsExpanded>,
) {
    for (interaction, toggle) in &q {
        if *interaction == Interaction::Pressed {
            toggle.0.toggle(&mut expanded);
        }
    }
}

// ── List snapshots ───────────────────────────────────────────────────────────

/// A simple "muted text" placeholder row (empty/“none” states).
fn note_row(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(TEXT_MUTED)),
        ))
        .id()
}

/// A `count   detail` row (mono count + secondary detail).
fn count_row(commands: &mut Commands, fonts: &EmberFonts, count: &str, detail: &str) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let c = commands
        .spawn((
            Text::new(count),
            ui_font(&fonts.mono, 10.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();
    let d = commands
        .spawn((
            Text::new(detail),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(SECONDARY)),
        ))
        .id();
    commands.entity(row).add_children(&[c, d]);
    row
}

fn archetypes_snapshot(world: &World) -> KeyedSnapshot {
    if !expanded(world, Section::Archetypes) {
        return empty();
    }
    let archs = ecs(world, |s| s.top_archetypes.clone());
    if archs.is_empty() {
        return note_snapshot("No archetypes");
    }
    let rows: Vec<(String, String)> = archs
        .iter()
        .take(20)
        .map(|a| {
            let count = format!("{:>5}", a.entity_count);
            let detail = if a.components.len() > 3 {
                format!(
                    "{}, ... +{}",
                    a.components.iter().take(3).map(|s| short_name(s)).collect::<Vec<_>>().join(", "),
                    a.components.len() - 3
                )
            } else {
                a.components.iter().map(|s| short_name(s)).collect::<Vec<_>>().join(", ")
            };
            (count, detail)
        })
        .collect();
    // Key by archetype identity (component set); hash by entity count.
    let items: Vec<(u64, u64)> = archs
        .iter()
        .take(20)
        .map(|a| (hash_str(&a.components.join("|")), a.entity_count as u64))
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| count_row(c, f, &rows[i].0, &rows[i].1)),
    }
}

fn components_snapshot(world: &World) -> KeyedSnapshot {
    if !expanded(world, Section::Components) {
        return empty();
    }
    let stats = ecs(world, |s| s.component_stats.clone());
    if stats.is_empty() {
        return note_snapshot("No components");
    }
    let total = stats.len();
    let shown = total.min(30);
    let rows: Vec<(String, String)> = stats
        .iter()
        .take(30)
        .map(|c| (format!("{:>6}", c.instance_count), short_name(&c.name).to_string()))
        .collect();
    let more = total.saturating_sub(30);

    let mut items: Vec<(u64, u64)> = Vec::with_capacity(shown + 1);
    for c in stats.iter().take(30) {
        items.push((hash_str(&c.name), c.instance_count as u64));
    }
    if more > 0 {
        items.push((u64::MAX - 1, more as u64));
    }
    KeyedSnapshot {
        items,
        build: Box::new(move |cmds, f, i| {
            if i < rows.len() {
                count_row(cmds, f, &rows[i].0, &rows[i].1)
            } else {
                note_row(cmds, f, &format!("... and {} more", more))
            }
        }),
    }
}

fn resources_snapshot(world: &World) -> KeyedSnapshot {
    if !expanded(world, Section::Resources) {
        return empty();
    }
    let res = ecs(world, |s| s.resources.clone());
    if res.is_empty() {
        return note_snapshot("Resource tracking not enabled");
    }
    let names: Vec<String> = res.iter().take(50).map(|r| short_name(r).to_string()).collect();
    let items: Vec<(u64, u64)> = res.iter().take(50).map(|r| (hash_str(r), 0)).collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            c.spawn((
                Text::new(names[i].clone()),
                ui_font(&f.ui, 10.0),
                TextColor(rgb(SECONDARY)),
            ))
            .id()
        }),
    }
}

fn empty() -> KeyedSnapshot {
    KeyedSnapshot {
        items: Vec::new(),
        build: Box::new(|_, _, _| Entity::PLACEHOLDER),
    }
}

fn note_snapshot(text: &'static str) -> KeyedSnapshot {
    KeyedSnapshot {
        items: vec![(u64::MAX, 0)],
        build: Box::new(move |c, f, _| note_row(c, f, text)),
    }
}
