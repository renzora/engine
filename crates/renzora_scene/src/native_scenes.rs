//! Bevy-native (ember) port of the egui `ScenesPanel`: a "New Scene" button over
//! a list of `<project>/scenes/*.ron`, with the active scene highlighted, the
//! boot scene starred, double-click to open, and a right-click context menu
//! (Open / Set as Boot / Delete). Reuses `panel.rs`'s scene helpers.

use std::path::PathBuf;

use bevy::prelude::*;

use renzora::core::CurrentProject;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_bg, bind_display, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{icon_label_button, menu_item, menu_item_styled, menu_sep, screen_menu};
use renzora_editor_framework::{EditorCommands, SplashState};

use crate::panel::{list_scenes, open_scene, paths_equal, unique_scene_path, EMPTY_SCENE_RON};

pub struct NativeScenesPanel;

impl Plugin for NativeScenesPanel {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScenesState>();
        app.register_panel_content("scenes", true, build);
        app.add_systems(
            Update,
            (new_scene_click, scenes_track_hover, scenes_click, scenes_context_menu)
                .run_if(in_state(SplashState::Editor)),
        );
    }
}

#[derive(Resource, Default)]
struct ScenesState {
    hovered: Option<PathBuf>,
    last_click: Option<(PathBuf, f64)>,
}

#[derive(Component)]
struct ScenesRoot;
#[derive(Component)]
struct NewSceneBtn;
#[derive(Component)]
struct SceneRow {
    path: PathBuf,
}

// ── Scene path helpers (project-relative resolution) ─────────────────────────

fn scenes_dir(w: &World) -> Option<PathBuf> {
    w.get_resource::<CurrentProject>().map(|p| p.resolve_path("scenes"))
}

/// Abs path of the scene the active document tab points at.
fn current_scene_abs(w: &World) -> Option<PathBuf> {
    let project = w.get_resource::<CurrentProject>()?;
    let tabs = w.get_resource::<renzora_ui::DocumentTabState>()?;
    tabs.tabs
        .get(tabs.active_tab)
        .and_then(|t| t.scene_path.as_ref())
        .map(|p| project.resolve_path(p))
}

fn boot_scene_abs(w: &World) -> Option<PathBuf> {
    let project = w.get_resource::<CurrentProject>()?;
    if project.config.main_scene.is_empty() {
        None
    } else {
        Some(project.resolve_path(&project.config.main_scene))
    }
}

fn is_current(w: &World, path: &std::path::Path) -> bool {
    current_scene_abs(w).map(|c| paths_equal(&c, path)).unwrap_or(false)
}

fn is_boot(w: &World, path: &std::path::Path) -> bool {
    boot_scene_abs(w).map(|c| paths_equal(&c, path)).unwrap_or(false)
}

// ── Panel ────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(6.0)),
                ..default()
            },
            bevy::ui::RelativeCursorPosition::default(),
            ScenesRoot,
            Name::new("scenes-root"),
        ))
        .id();

    let new_btn = icon_label_button(commands, fonts, "plus", "New Scene");
    commands.entity(new_btn).insert((
        NewSceneBtn,
        Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(5.0),
            ..default()
        },
    ));

    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    keyed_list(commands, list, scenes_snapshot);

    commands.entity(root).add_children(&[new_btn, list]);
    root
}

fn scenes_snapshot(world: &World) -> KeyedSnapshot {
    let Some(dir) = scenes_dir(world) else {
        return note_snapshot("No project open.");
    };
    let entries = list_scenes(&dir);
    if entries.is_empty() {
        return note_snapshot("No scenes yet. Click \"New Scene\".");
    }
    let items: Vec<(u64, u64)> = entries
        .iter()
        .map(|p| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            use std::hash::{Hash, Hasher};
            p.hash(&mut h);
            (h.finish(), 0)
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| scene_row(c, f, entries[i].clone())),
    }
}

fn scene_row(commands: &mut Commands, fonts: &EmberFonts, path: PathBuf) -> Entity {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("?")
        .to_string();
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(24.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            SceneRow { path: path.clone() },
            Name::new(format!("scene:{name}")),
        ))
        .id();
    // Current → accent tint, hover → hover surface, else faint.
    {
        let p = path.clone();
        bind_bg(commands, row, move |w| {
            if is_current(w, &p) {
                rgb(accent()).with_alpha(0.25)
            } else if matches!(
                w.get::<Interaction>(row),
                Some(Interaction::Hovered) | Some(Interaction::Pressed)
            ) {
                rgb(hover_bg())
            } else {
                rgb(section_bg())
            }
        });
    }
    let icon = icon_text(commands, &fonts.phosphor, "film-slate", text_muted(), 13.0);
    let label = commands
        .spawn((Text::new(name), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())))).id();
    let gap = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    // Boot star (shown only for the boot scene).
    let star = icon_text(commands, &fonts.phosphor, "star", c_accent(), 12.0);
    {
        let p = path.clone();
        bind_display(commands, star, move |w| is_boot(w, &p));
    }
    commands.entity(row).add_children(&[icon, label, gap, star]);
    row
}

fn c_accent() -> (u8, u8, u8) {
    accent()
}

fn note_snapshot(text: &'static str) -> KeyedSnapshot {
    KeyedSnapshot {
        items: vec![(u64::MAX, 0)],
        build: Box::new(move |c, f, _| {
            c.spawn((
                Text::new(text),
                ui_font(&f.ui, 11.0),
                TextColor(rgb(text_muted())),
                Node { margin: UiRect::all(Val::Px(8.0)), ..default() },
            ))
            .id()
        }),
    }
}

// ── Systems ──────────────────────────────────────────────────────────────────

fn new_scene_click(
    q: Query<&Interaction, (With<NewSceneBtn>, Changed<Interaction>)>,
    cmds: Option<Res<EditorCommands>>,
    project: Option<Res<CurrentProject>>,
) {
    let (Some(cmds), Some(project)) = (cmds, project) else { return };
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let dir = project.resolve_path("scenes");
    cmds.push(move |_w: &mut World| {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            renzora::core::console_log::console_error("Scene", format!("Failed to create scenes/: {e}"));
            return;
        }
        let new_path = unique_scene_path(&dir, "untitled");
        if let Err(e) = std::fs::write(&new_path, EMPTY_SCENE_RON) {
            renzora::core::console_log::console_error("Scene", format!("Failed to create scene: {e}"));
            return;
        }
        renzora::core::console_log::console_success("Scene", format!("Created {}", new_path.display()));
    });
}

fn scenes_track_hover(rows: Query<(&Interaction, &SceneRow)>, mut state: ResMut<ScenesState>) {
    for (interaction, row) in &rows {
        if matches!(interaction, Interaction::Hovered | Interaction::Pressed)
            && state.hovered.as_deref() != Some(row.path.as_path())
        {
            state.hovered = Some(row.path.clone());
        }
    }
}

fn scenes_click(
    q: Query<(&Interaction, &SceneRow), Changed<Interaction>>,
    mut state: ResMut<ScenesState>,
    time: Res<Time>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    let now = time.elapsed_secs_f64();
    for (interaction, row) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let double = state
            .last_click
            .as_ref()
            .is_some_and(|(p, t)| p == &row.path && now - t < 0.4);
        if double {
            state.last_click = None;
            let target = row.path.clone();
            cmds.push(move |w: &mut World| open_scene(w, &target));
        } else {
            state.last_click = Some((row.path.clone(), now));
        }
    }
}

fn scenes_context_menu(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    state: Res<ScenesState>,
    roots: Query<&bevy::ui::RelativeCursorPosition, With<ScenesRoot>>,
    mut commands: Commands,
) {
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    let Some(fonts) = fonts else { return };
    if !roots.iter().any(|rcp| rcp.cursor_over) {
        return;
    }
    let Some(path) = state.hovered.clone() else { return };
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else { return };
    let menu = screen_menu(&mut commands, cursor.x, cursor.y);
    let kids = vec![
        menu_item(&mut commands, &fonts, "arrow-square-out", "Open", {
            let path = path.clone();
            move |w| open_scene(w, &path)
        }),
        menu_item(&mut commands, &fonts, "star", "Set as Boot Scene", {
            let path = path.clone();
            move |w| set_boot_scene(w, &path)
        }),
        menu_sep(&mut commands),
        menu_item_styled(&mut commands, &fonts, "trash", "Delete", (224, 96, 88), (224, 96, 88), {
            let path = path.clone();
            move |_| {
                if let Err(e) = std::fs::remove_file(&path) {
                    renzora::core::console_log::console_error("Scene", format!("Delete failed: {e}"));
                }
            }
        }),
    ];
    commands.entity(menu).add_children(&kids);
}

fn set_boot_scene(world: &mut World, target: &std::path::Path) {
    if let Some(mut project) = world.get_resource_mut::<CurrentProject>() {
        if let Some(rel) = project.make_relative(target) {
            project.config.main_scene = rel;
            if let Err(e) = project.save_config() {
                renzora::core::console_log::console_error("Scene", format!("Failed to save project config: {e}"));
            }
        }
    }
}
