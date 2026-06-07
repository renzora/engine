//! Bevy-native (ember) Scripts panel — lists the selected entity's attached
//! scripts; click to open, eye to toggle, trash to detach, "New Script" to
//! create+attach. Reads `EditorSelection` + `ScriptComponent`; writes go through
//! `EditorCommands` (the same path the egui panel used; drained in bevy_ui mode).

use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use bevy::prelude::*;

use renzora_editor_framework::{EditorCommands, EditorSelection, SplashState};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_bg, bind_display, keyed_list, KeyedSnapshot};
use renzora_ember::theme::{accent, close_red, placeholder, rgb, section_bg, tab_hover, text_muted, text_primary};
use renzora_scripting::ScriptComponent;

use crate::scripts_on_entity::create_and_attach_new_script;
use crate::state::CodeEditorState;


#[derive(Component)]
struct NewScriptBtn;
#[derive(Component)]
struct ScriptOpen(Option<PathBuf>);
#[derive(Component)]
struct ScriptToggle(Entity, u32);
#[derive(Component)]
struct ScriptDetach(Entity, u32);

pub fn register_native_scripts_on_entity(app: &mut App) {
    app.register_panel_content("scripts_on_entity", true, build);
    app.add_systems(
        Update,
        (new_script_click, script_open_click, script_toggle_click, script_detach_click)
            .run_if(in_state(SplashState::Editor)),
    );
}

fn selected(w: &World) -> Option<Entity> {
    w.get_resource::<EditorSelection>().and_then(|s| s.get())
}
fn project_root(w: &World) -> Option<PathBuf> {
    w.get_resource::<renzora::core::CurrentProject>().map(|p| p.path.clone())
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            ..default()
        })
        .id();

    // Toolbar: New Script (only with an entity selected).
    let toolbar = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            padding: UiRect::axes(Val::Px(4.0), Val::Px(4.0)),
            ..default()
        })
        .id();
    let new_btn = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(tab_hover())),
            Interaction::default(),
            NewScriptBtn,
            Name::new("new-script"),
        ))
        .id();
    let np_icon = icon_text(commands, &fonts.phosphor, "file-plus", text_muted(), 12.0);
    let np_text = commands
        .spawn((Text::new("New Script"), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary()))))
        .id();
    commands.entity(new_btn).add_children(&[np_icon, np_text]);
    commands.entity(toolbar).add_child(new_btn);
    bind_display(commands, toolbar, |w| selected(w).is_some());

    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    keyed_list(commands, list, scripts_snapshot);

    commands.entity(root).add_children(&[toolbar, list]);
    root
}

// ── List ─────────────────────────────────────────────────────────────────────

struct RowData {
    id: u32,
    entity: Entity,
    display: String,
    enabled: bool,
    exists: bool,
    resolved: Option<PathBuf>,
}

enum Item {
    Note(String),
    Row(RowData),
}

fn scripts_snapshot(world: &World) -> KeyedSnapshot {
    let items: Vec<Item> = build_items(world);
    let keyed: Vec<(u64, u64)> = items
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            match it {
                Item::Note(s) => (0u8, s).hash(&mut h),
                Item::Row(r) => (1u8, r.entity.to_bits(), r.id, &r.display, r.enabled, r.exists).hash(&mut h),
            }
            (i as u64, h.finish())
        })
        .collect();
    KeyedSnapshot {
        items: keyed,
        build: Box::new(move |c, f, i| match &items[i] {
            Item::Note(s) => note(c, f, s),
            Item::Row(r) => script_row(c, f, r),
        }),
    }
}

fn build_items(world: &World) -> Vec<Item> {
    let Some(entity) = selected(world) else {
        return vec![Item::Note("Select an entity to see its scripts".into())];
    };
    let Some(sc) = world.get::<ScriptComponent>(entity) else {
        return vec![Item::Note("This entity has no scripts.".into())];
    };
    if sc.scripts.is_empty() {
        return vec![Item::Note("No scripts attached".into())];
    }
    let root = project_root(world);
    sc.scripts
        .iter()
        .map(|e| {
            let display = e
                .script_path
                .as_ref()
                .and_then(|p| p.file_name().and_then(|n| n.to_str()).map(|s| s.to_string()))
                .unwrap_or_else(|| {
                    if e.script_id.is_empty() {
                        format!("(script {})", e.id)
                    } else {
                        e.script_id.clone()
                    }
                });
            let resolved = e.script_path.as_ref().map(|p| {
                if p.is_absolute() {
                    p.clone()
                } else if let Some(root) = &root {
                    root.join(p)
                } else {
                    p.clone()
                }
            });
            let exists = match &resolved {
                Some(p) => p.exists(),
                None => true,
            };
            Item::Row(RowData {
                id: e.id,
                entity,
                display,
                enabled: e.enabled,
                exists,
                resolved,
            })
        })
        .collect()
}

fn note(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                padding: UiRect::top(Val::Px(12.0)),
                ..default()
            },
        ))
        .id()
}

fn script_row(commands: &mut Commands, fonts: &EmberFonts, r: &RowData) -> Entity {
    let bar_color = if r.enabled { accent() } else { placeholder() };
    let name_color = if !r.exists {
        close_red()
    } else if r.enabled {
        text_primary()
    } else {
        text_muted()
    };
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            width: Val::Percent(100.0),
            height: Val::Px(26.0),
            ..default()
        })
        .id();
    let bar = commands
        .spawn((
            Node { width: Val::Px(2.0), height: Val::Px(18.0), margin: UiRect::right(Val::Px(8.0)), ..default() },
            BackgroundColor(rgb(bar_color)),
        ))
        .id();

    // Open zone (name + missing badge) — the click-to-open target.
    let open = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                flex_grow: 1.0,
                height: Val::Percent(100.0),
                column_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            ScriptOpen(r.resolved.clone()),
            Name::new("script-open"),
        ))
        .id();
    bind_bg(commands, open, move |w| match w.get::<Interaction>(open) {
        Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(section_bg()),
        _ => Color::NONE,
    });
    let name = commands
        .spawn((Text::new(r.display.clone()), ui_font(&fonts.ui, 11.5), TextColor(rgb(name_color))))
        .id();
    let mut open_kids = vec![name];
    if !r.exists {
        open_kids.push(
            commands
                .spawn((Text::new("missing"), ui_font(&fonts.ui, 9.5), TextColor(rgb(close_red()))))
                .id(),
        );
    }
    commands.entity(open).add_children(&open_kids);

    // Toggle (eye / eye-slash).
    let toggle = commands
        .spawn((
            Node { width: Val::Px(20.0), height: Val::Percent(100.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() },
            Interaction::default(),
            ScriptToggle(r.entity, r.id),
            Name::new("script-toggle"),
        ))
        .id();
    let (eye, eye_color) = if r.enabled { ("eye", accent()) } else { ("eye-slash", text_muted()) };
    let eye_icon = icon_text(commands, &fonts.phosphor, eye, eye_color, 13.0);
    commands.entity(toggle).add_child(eye_icon);

    // Detach (trash).
    let detach = commands
        .spawn((
            Node { width: Val::Px(20.0), height: Val::Percent(100.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() },
            Interaction::default(),
            ScriptDetach(r.entity, r.id),
            Name::new("script-detach"),
        ))
        .id();
    let trash = icon_text(commands, &fonts.phosphor, "trash", close_red(), 13.0);
    commands.entity(detach).add_child(trash);

    commands.entity(row).add_children(&[bar, open, toggle, detach]);
    row
}

// ── Click systems ────────────────────────────────────────────────────────────

fn new_script_click(
    q: Query<&Interaction, (With<NewScriptBtn>, Changed<Interaction>)>,
    selection: Option<Res<EditorSelection>>,
    project: Option<Res<renzora::core::CurrentProject>>,
    cmds: Option<Res<EditorCommands>>,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let (Some(e), Some(root), Some(c)) = (
        selection.and_then(|s| s.get()),
        project.map(|p| p.path.clone()),
        cmds,
    ) else {
        return;
    };
    c.push(move |world: &mut World| create_and_attach_new_script(world, e, root));
}

fn script_open_click(
    q: Query<(&Interaction, &ScriptOpen), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(c) = cmds else { return };
    for (interaction, open) in &q {
        if *interaction == Interaction::Pressed {
            if let Some(path) = &open.0 {
                if path.exists() {
                    let path = path.clone();
                    c.push(move |world: &mut World| {
                        if let Some(mut s) = world.get_resource_mut::<CodeEditorState>() {
                            s.open_file(path);
                        }
                    });
                }
            }
        }
    }
}

fn script_toggle_click(
    q: Query<(&Interaction, &ScriptToggle), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(c) = cmds else { return };
    for (interaction, t) in &q {
        if *interaction == Interaction::Pressed {
            let (entity, id) = (t.0, t.1);
            c.push(move |world: &mut World| {
                if let Some(mut sc) = world.get_mut::<ScriptComponent>(entity) {
                    if let Some(entry) = sc.scripts.iter_mut().find(|e| e.id == id) {
                        entry.enabled = !entry.enabled;
                    }
                }
            });
        }
    }
}

fn script_detach_click(
    q: Query<(&Interaction, &ScriptDetach), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(c) = cmds else { return };
    for (interaction, d) in &q {
        if *interaction == Interaction::Pressed {
            let (entity, id) = (d.0, d.1);
            c.push(move |world: &mut World| {
                if let Some(mut sc) = world.get_mut::<ScriptComponent>(entity) {
                    sc.scripts.retain(|e| e.id != id);
                }
            });
        }
    }
}
