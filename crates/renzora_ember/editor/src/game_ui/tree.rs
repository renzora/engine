//! The **UI Hierarchy** panel: the canvases in the scene, and the node tree of
//! the template each one mounts.
//!
//! # Why this is not the scene hierarchy with a filter
//!
//! Because it is not showing the same thing. The scene hierarchy lists entities
//! you can transform, parent and save; a UI is a `.html` document whose nodes
//! are rebuilt from the file every time it loads, and which the scene
//! deliberately does not serialise (see the `HideInHierarchy` note in
//! `markup/loader.rs`). Those two trees answer different questions and only one
//! of them is useful while you are laying out a menu — a mesh in the list is
//! something you cannot do anything UI-shaped to.
//!
//! Filtering the scene panel by workspace was the alternative, and it is worse:
//! the same panel would show different contents depending on where you were
//! standing, which is state the user cannot see. Two panels, two jobs.
//!
//! # Why a cache resource
//!
//! The rows come from a `Resource` a normal system rebuilds, not from queries
//! inside the reactive snapshot. That is the pattern the scene hierarchy uses
//! (`HierarchyTreeCache`) and it exists because a snapshot runs inside the
//! reactive layer, where a full archetype walk per frame is exactly what the
//! layer is trying to avoid.

use bevy::prelude::*;

use renzora::core::RenzoraShellExt;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::tracked::{bind_bg, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;

use renzora_ember::game_ui::{UiCanvas, UiWidget};
use renzora_ember::markup::HtmlTemplatePath;

const PANEL_ID: &str = "ui_hierarchy";
/// Indent per level. Smaller than the scene tree's — a template nests deeper
/// than a scene does, and a `<node>` six levels in still has to fit the column.
const INDENT: f32 = 12.0;

/// One row of the panel.
#[derive(Clone)]
pub(crate) struct UiTreeRow {
    pub entity: Entity,
    pub depth: usize,
    pub name: String,
    pub icon: &'static str,
    /// Canvases are the roots and read as headings; markup nodes are content.
    pub is_canvas: bool,
    /// A canvas with no template yet — drawn muted, with a hint instead of a
    /// subtree, because it is the one state that needs an instruction.
    pub empty_canvas: bool,
}

#[derive(Resource, Default)]
pub(crate) struct UiTreeCache {
    pub rows: Vec<UiTreeRow>,
    /// Bumped on every rebuild that changed something, so the keyed list only
    /// re-runs its snapshot when the tree actually moved.
    pub version: u64,
}

#[derive(Component)]
struct UiTreeRowBtn(Entity);

pub(crate) fn register(app: &mut App) {
    app.init_resource::<UiTreeCache>();
    app.register_shell_panel("ui_hierarchy", "UI Hierarchy", "tree-structure", "Scene");
    app.register_panel_content(PANEL_ID, true, build);
    app.add_systems(
        Update,
        (rebuild_ui_tree, ui_tree_click).run_if(in_state(renzora::SplashState::Editor)),
    );
}

fn build(commands: &mut Commands, _fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::vertical(Val::Px(4.0)),
                ..default()
            },
            Name::new("ui-hierarchy-root"),
        ))
        .id();
    keyed_list(commands, root, tree_snapshot);
    root
}

fn tree_snapshot(world: &Rx) -> KeyedSnapshot {
    let Some(cache) = world.get_resource::<UiTreeCache>() else {
        return KeyedSnapshot {
            items: Vec::new(),
            build: Box::new(|c, _, _| c.spawn(Node::default()).id()),
        };
    };
    if cache.rows.is_empty() {
        return KeyedSnapshot {
            items: vec![(u64::MAX, 0)],
            build: Box::new(|c, f, _| {
                c.spawn((
                    Text::new("No UI canvases in this scene.\nAdd Entity \u{2192} UI \u{2192} UI Canvas."),
                    ui_font(&f.ui, 11.0),
                    TextColor(rgb(text_muted())),
                    Node { margin: UiRect::all(Val::Px(10.0)), ..default() },
                ))
                .id()
            }),
        };
    }
    let rows = cache.rows.clone();
    use std::hash::{Hash, Hasher};
    let items: Vec<(u64, u64)> = rows
        .iter()
        .map(|r| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            r.entity.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (&r.name, r.depth, r.icon, r.is_canvas, r.empty_canvas).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| row(c, f, &rows[i])),
    }
}

fn row(commands: &mut Commands, fonts: &EmberFonts, r: &UiTreeRow) -> Entity {
    let entity = r.entity;
    let node = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                min_height: Val::Px(22.0),
                padding: UiRect {
                    left: Val::Px(8.0 + r.depth as f32 * INDENT),
                    right: Val::Px(8.0),
                    top: Val::Px(2.0),
                    bottom: Val::Px(2.0),
                },
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            UiTreeRowBtn(entity),
            Name::new("ui-tree-row"),
        ))
        .id();
    bind_bg(commands, node, move |w| {
        let selected = w
            .get_resource::<renzora::EditorSelection>()
            .is_some_and(|s| s.get_all().contains(&entity));
        if selected {
            rgb(accent()).with_alpha(0.22)
        } else if matches!(
            w.get::<Interaction>(node),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        ) {
            rgb(hover_bg())
        } else {
            Color::NONE
        }
    });

    let colour = if r.is_canvas { accent() } else { text_muted() };
    let ic = icon_text(commands, &fonts.phosphor, r.icon, colour, 13.0);
    commands.entity(ic).insert(bevy::ui::FocusPolicy::Pass);
    let label = commands
        .spawn((
            Text::new(r.name.clone()),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(if r.is_canvas { text_primary() } else { value_text() })),
            bevy::text::TextLayout::no_wrap(),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    let mut kids = vec![ic, label];
    if r.empty_canvas {
        kids.push(
            commands
                .spawn((
                    Text::new("no template"),
                    ui_font(&fonts.ui, 10.0),
                    TextColor(rgb(placeholder())),
                    bevy::text::TextLayout::no_wrap(),
                    bevy::ui::FocusPolicy::Pass,
                    Node { margin: UiRect::left(Val::Px(4.0)), ..default() },
                ))
                .id(),
        );
    }
    commands.entity(node).add_children(&kids);
    node
}

/// Clicking a row selects that entity — the same `EditorSelection` the canvas
/// overlay and the inspector read, so the three stay in step without knowing
/// about each other.
fn ui_tree_click(
    q: Query<(&Interaction, &UiTreeRowBtn), Changed<Interaction>>,
    selection: Option<Res<renzora::EditorSelection>>,
) {
    let Some(selection) = selection else { return };
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed {
            selection.set(Some(btn.0));
        }
    }
}

/// Rebuild the row list from the world.
///
/// Runs only while the panel is on screen, and writes only when the result
/// differs — the tree is stable for long stretches and a `version` bump is what
/// makes the reactive list re-run.
fn rebuild_ui_tree(
    mut cache: ResMut<UiTreeCache>,
    canvases: Query<(Entity, Option<&Name>, Option<&HtmlTemplatePath>), With<UiCanvas>>,
    children_q: Query<&Children>,
    named: Query<(Option<&Name>, Option<&UiWidget>)>,
    kinds: Query<(Option<&Text>, Option<&ImageNode>, Option<&Button>)>,
    dock: Option<Res<renzora_ember::dock::Dock>>,
    fixed: Option<Res<renzora_ember::dock::FixedDock>>,
    wins: Option<Res<renzora_ember::dock::DockWindows>>,
) {
    if !renzora_ember::dock::panel_visible_anywhere(
        PANEL_ID,
        dock.as_deref(),
        fixed.as_deref(),
        wins.as_deref(),
    ) {
        return;
    }

    let mut rows: Vec<UiTreeRow> = Vec::new();
    // Sorted by entity so the list is stable frame to frame; canvases have no
    // authored order of their own here (`sort_order` is z, not list position).
    let mut all: Vec<(Entity, Option<&Name>, Option<&HtmlTemplatePath>)> =
        canvases.iter().collect();
    all.sort_by_key(|(e, ..)| *e);
    for (entity, name, template) in all {
        let has_template = template.is_some_and(|t| !t.0.trim().is_empty());
        rows.push(UiTreeRow {
            entity,
            depth: 0,
            name: name.map(|n| n.as_str().to_string()).unwrap_or_else(|| "UI Canvas".into()),
            icon: "frame-corners",
            is_canvas: true,
            empty_canvas: !has_template,
        });
        if has_template {
            push_subtree(entity, 1, &children_q, &named, &kinds, &mut rows);
        }
    }

    let changed = rows.len() != cache.rows.len()
        || rows.iter().zip(cache.rows.iter()).any(|(a, b)| {
            a.entity != b.entity
                || a.depth != b.depth
                || a.name != b.name
                || a.icon != b.icon
                || a.empty_canvas != b.empty_canvas
        });
    if changed {
        cache.rows = rows;
        cache.version = cache.version.wrapping_add(1);
    }
}

/// Walk the markup tree under `parent`, emitting a row per node.
///
/// Only entities carrying `UiWidget` are emitted — that is what the loader
/// stamps on every markup node, so it is exactly "things the template made" and
/// excludes any plumbing that happens to be parented in.
fn push_subtree(
    parent: Entity,
    depth: usize,
    children_q: &Query<&Children>,
    named: &Query<(Option<&Name>, Option<&UiWidget>)>,
    kinds: &Query<(Option<&Text>, Option<&ImageNode>, Option<&Button>)>,
    rows: &mut Vec<UiTreeRow>,
) {
    // A template can nest arbitrarily and a malformed one could, in principle,
    // cycle through a bad parent link. Depth-cap rather than trust it.
    if depth > 32 {
        return;
    }
    let Ok(children) = children_q.get(parent) else {
        return;
    };
    for child in children.iter() {
        let Ok((name, widget)) = named.get(child) else {
            continue;
        };
        if widget.is_none() {
            continue;
        }
        rows.push(UiTreeRow {
            entity: child,
            depth,
            name: name.map(|n| n.as_str().to_string()).unwrap_or_else(|| "node".into()),
            icon: node_icon(child, kinds),
            is_canvas: false,
            empty_canvas: false,
        });
        push_subtree(child, depth + 1, children_q, named, kinds, rows);
    }
}

/// A glyph for what the node *is*, from the components the loader gave it —
/// there is no node-type component to read, and re-deriving it from the markup
/// would mean holding the template asset here just to draw an icon.
fn node_icon(
    entity: Entity,
    kinds: &Query<(Option<&Text>, Option<&ImageNode>, Option<&Button>)>,
) -> &'static str {
    match kinds.get(entity) {
        Ok((_, _, Some(_))) => "cursor-click",
        Ok((Some(_), _, _)) => "text-t",
        Ok((_, Some(_), _)) => "image",
        _ => "square",
    }
}
