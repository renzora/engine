//! The Inspector with nothing selected: the scene itself.
//!
//! It used to say "No entity selected" over an otherwise empty panel, the
//! largest area of the editor doing nothing at the moment you are deciding
//! what to work on next. It now shows what belongs to the scene rather than to
//! any one entity you pick: its name and size, the world environment's
//! settings (the sky, fog and sun, which are otherwise only reachable by
//! finding the `world_environment` entity in the Hierarchy), the boot scene
//! and start camera, and a few counts.
//!
//! Built by [`super::rebuild::rebuild_inspector`] like any other view, and
//! rebuilt only when what it shows by name changes. The counts are the
//! exception: they move with every spawn, so instead of feeding the rebuild
//! signature (which would rebuild the page on every spawn anywhere) a small
//! system refreshes their text once a second while the page is up.

use bevy::prelude::*;

use renzora_editor_framework::EditorCommands;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::interaction::{BLOCK, PASS_THROUGH};
use renzora_ember::theme::{rgb, text_muted, text_primary};
use renzora_ember::UiInteraction;

use super::section::{card_bg, info_card};
use super::spec::category_rgb;
use super::{c, empty_label, phosphor_glyph};

/// The scene tile's hue: the Environment page's green, since the page leads
/// with the world.
const SCENE_HUE: (u8, u8, u8) = (110, 231, 160);

/// Which number a [`SceneStat`] text shows.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatKind {
    Entities,
    Meshes,
    Triangles,
    /// The header's subtitle, which carries the entity count too.
    Subtitle { boot: bool },
}

/// A text the once-a-second refresh rewrites (see [`refresh_scene_stats`]).
#[derive(Component)]
pub(crate) struct SceneStat(pub(crate) StatKind);

/// The Game card's "Set as Boot Scene" button.
#[derive(Component)]
pub(crate) struct SetBootSceneBtn;

/// The World card's "Add World Environment" button, when the scene has none.
#[derive(Component)]
pub(crate) struct AddWorldEnvBtn;

/// What the page shows by name, read before the rebuild's `Commands` borrows
/// the world.
pub(crate) struct SceneFacts {
    pub(crate) name: String,
    pub(crate) is_boot: bool,
    /// The boot scene's file name, if the project has one set.
    pub(crate) boot_name: Option<String>,
    /// Whether the active document is a saved scene that is not the boot
    /// scene already, i.e. whether "Set as Boot Scene" has something to do.
    pub(crate) can_set_boot: bool,
    pub(crate) start_camera: Option<String>,
}

fn same_scene(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.replace('\\', "/").trim_start_matches("./").to_lowercase();
    !a.is_empty() && norm(a) == norm(b)
}

fn file_stem(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
        .to_string()
}

/// The active scene document, as `(name, project-relative path if saved)`.
fn active_scene(world: &World) -> Option<(String, Option<String>)> {
    world
        .get_resource::<renzora_ui::DocumentTabState>()
        .and_then(|s| s.tabs.get(s.active_tab))
        .filter(|t| t.kind == renzora_ui::DocTabKind::Scene)
        .map(|t| (t.name.clone(), t.scene_path.clone()))
}

pub(crate) fn scene_facts(world: &World, start_camera: Option<String>) -> SceneFacts {
    let boot_path = world
        .get_resource::<renzora::CurrentProject>()
        .map(|p| p.config.main_scene.clone())
        .filter(|s| !s.is_empty());
    let active = active_scene(world);
    let name = match (&active, &boot_path) {
        (Some((name, _)), _) => name.clone(),
        (None, Some(boot)) => file_stem(boot),
        (None, None) => renzora::lang::t_or("inspector.scene.untitled", "Untitled scene"),
    };
    let active_path = active.as_ref().and_then(|(_, p)| p.clone());
    let is_boot = match (&active_path, &boot_path) {
        (Some(a), Some(b)) => same_scene(a, b),
        _ => false,
    };
    SceneFacts {
        name,
        is_boot,
        boot_name: boot_path.as_deref().map(file_stem),
        can_set_boot: active_path.is_some() && !is_boot,
        start_camera,
    }
}

/// Hash what the page shows by name into the rebuild signature: the active
/// scene, the boot scene, and which entities hold the world environment and
/// the start camera. The counts are left out on purpose (module doc).
pub(crate) fn hash_scene(world: &World, facts_inputs: (Option<Entity>, Option<&str>), h: &mut impl std::hash::Hasher) {
    use std::hash::Hash;
    if let Some(s) = world.get_resource::<renzora_ui::DocumentTabState>() {
        if let Some(t) = s.tabs.get(s.active_tab) {
            t.id.hash(h);
            t.name.hash(h);
            t.scene_path.hash(h);
        }
    }
    if let Some(p) = world.get_resource::<renzora::CurrentProject>() {
        p.config.main_scene.hash(h);
    }
    facts_inputs.0.map(|e| e.to_bits()).hash(h);
    facts_inputs.1.hash(h);
}

/// The header for the scene: its tile, its name and the counting subtitle.
pub(crate) fn build_scene_header(commands: &mut Commands, fonts: &EmberFonts, facts: &SceneFacts) -> Vec<Entity> {
    let tile = commands
        .spawn((
            Node {
                flex_shrink: 0.0,
                width: Val::Px(44.0),
                height: Val::Px(44.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(rgb(SCENE_HUE).with_alpha(0.16)),
            Name::new("scene-tile"),
        ))
        .id();
    let glyph = phosphor_glyph(commands, fonts, "film-slate", SCENE_HUE, 26.0);
    commands.entity(tile).add_child(glyph);

    let col = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            Name::new("scene-name-column"),
        ))
        .id();
    // 17 lands on the 16px rung, the entity name's size in the same place.
    let title = commands
        .spawn((
            Text::new(facts.name.clone()),
            ui_font(&fonts.ui, 17.0),
            TextColor(rgb(text_primary())),
            Name::new("scene-name"),
        ))
        .id();
    renzora_ember::widgets::ellipsize(commands, title, facts.name.clone(), None);
    let kind = renzora::lang::t_or("inspector.scene.kind", "Scene");
    let sub = commands
        .spawn((
            Text::new(kind.clone()),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(text_muted())),
            SceneStat(StatKind::Subtitle { boot: facts.is_boot }),
            Name::new("scene-subtitle"),
        ))
        .id();
    renzora_ember::widgets::ellipsize(commands, sub, kind, None);
    commands.entity(col).add_children(&[title, sub]);
    vec![tile, col]
}

/// A small muted heading over a group of cards ("World").
pub(crate) fn group_heading(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text.to_uppercase()),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
            Node { margin: UiRect::new(Val::Px(4.0), Val::Px(0.0), Val::Px(4.0), Val::Px(6.0)), ..default() },
        ))
        .id()
}

/// The World card shown in place of the environment's own cards when the
/// scene has no world environment: a line saying so and a button adding one.
pub(crate) fn build_no_world(commands: &mut Commands, fonts: &EmberFonts, theme: Option<&renzora_theme::Theme>) -> Entity {
    let (accent, header_bg) = theme
        .map(|t| category_rgb(t, "environment"))
        .unwrap_or(((130, 210, 160), (40, 54, 46)));
    let (root, body) = info_card(
        commands,
        fonts,
        "globe",
        &renzora::lang::t_or("inspector.scene.world", "World"),
        accent,
        header_bg,
    );
    let note = empty_label(
        commands,
        fonts,
        &renzora::lang::t_or("inspector.scene.no_world", "No world environment in this scene"),
    );
    let btn = action_button(
        commands,
        fonts,
        "plus",
        &renzora::lang::t_or("inspector.scene.add_world", "Add World Environment"),
    );
    commands.entity(btn).insert(AddWorldEnvBtn);
    commands.entity(body).add_children(&[note, btn]);
    root
}

/// The Game card: what the game boots into and which camera it starts on.
pub(crate) fn build_game_card(
    commands: &mut Commands,
    fonts: &EmberFonts,
    facts: &SceneFacts,
    theme: Option<&renzora_theme::Theme>,
) -> Entity {
    let (accent, header_bg) = theme
        .map(|t| category_rgb(t, "camera"))
        .unwrap_or(((150, 200, 255), (36, 48, 63)));
    let (root, body) = info_card(
        commands,
        fonts,
        "game-controller",
        &renzora::lang::t_or("inspector.scene.game", "Game"),
        accent,
        header_bg,
    );
    let none = renzora::lang::t_or("inspector.scene.none", "None");
    let boot = value_text(commands, fonts, facts.boot_name.as_deref().unwrap_or(&none));
    let boot_row = renzora_ember::inspector::inspector_row(
        commands,
        &fonts.ui,
        &renzora::lang::t_or("inspector.scene.boot_scene", "Boot scene"),
        boot,
    );
    let cam = value_text(commands, fonts, facts.start_camera.as_deref().unwrap_or(&none));
    let cam_row = renzora_ember::inspector::inspector_row(
        commands,
        &fonts.ui,
        &renzora::lang::t_or("inspector.scene.start_camera", "Start camera"),
        cam,
    );
    commands.entity(body).add_children(&[boot_row, cam_row]);
    if facts.can_set_boot {
        let btn = action_button(
            commands,
            fonts,
            "star",
            &renzora::lang::t_or("inspector.scene.set_boot", "Set as Boot Scene"),
        );
        commands.entity(btn).insert(SetBootSceneBtn);
        commands.entity(body).add_child(btn);
    }
    root
}

/// The counts: entities, meshes, triangles. Filled in by
/// [`refresh_scene_stats`].
pub(crate) fn build_stats(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            },
            Name::new("scene-stats"),
        ))
        .id();
    let mut tiles = Vec::new();
    for (kind, label) in [
        (StatKind::Entities, renzora::lang::t_or("inspector.scene.entities", "entities")),
        (StatKind::Meshes, renzora::lang::t_or("inspector.scene.meshes", "meshes")),
        (StatKind::Triangles, renzora::lang::t_or("inspector.scene.triangles", "triangles")),
    ] {
        let tile = commands
            .spawn((
                Node {
                    flex_grow: 1.0,
                    flex_basis: Val::Px(0.0),
                    min_width: Val::Px(0.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(card_bg()),
            ))
            .id();
        let value = commands
            .spawn((
                Text::new("-"),
                ui_font(&fonts.ui, 17.0),
                TextColor(rgb(text_primary())),
                SceneStat(kind),
            ))
            .id();
        let name = commands
            .spawn((Text::new(label), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted()))))
            .id();
        commands.entity(tile).add_children(&[value, name]);
        tiles.push(tile);
    }
    commands.entity(row).add_children(&tiles);
    row
}

/// The muted line at the foot of the page.
pub(crate) fn build_hint(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let hint = commands
        .spawn((
            Text::new(renzora::lang::t_or("inspector.scene.hint", "Select an entity to edit it")),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let wrap = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                padding: UiRect::vertical(Val::Px(10.0)),
                ..default()
            },
            PASS_THROUGH,
        ))
        .id();
    commands.entity(wrap).add_child(hint);
    wrap
}

fn value_text(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text.to_string()),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::no_wrap(),
        ))
        .id()
}

/// A full-width ghost button inside a card body.
fn action_button(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str) -> Entity {
    let btn = commands
        .spawn((
            Node {
                height: Val::Px(26.0),
                margin: UiRect::new(Val::Px(4.0), Val::Px(4.0), Val::Px(4.0), Val::Px(2.0)),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(6.0),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.05)),
            UiInteraction::default(),
            BLOCK,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("scene-action"),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_bg(commands, btn, move |w| match w.get::<UiInteraction>(btn) {
        Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => Color::srgba(1.0, 1.0, 1.0, 0.10),
        _ => Color::srgba(1.0, 1.0, 1.0, 0.05),
    });
    let ic = phosphor_glyph(commands, fonts, icon, text_muted(), 13.0);
    let text = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 13.0),
            TextColor(c(text_primary())),
            PASS_THROUGH,
        ))
        .id();
    commands.entity(btn).add_children(&[ic, text]);
    btn
}

/// "1.2M", "34.5K", "812".
fn short_count(n: u64) -> String {
    match n {
        0..=9_999 => n.to_string(),
        10_000..=999_999 => format!("{:.1}K", n as f64 / 1_000.0),
        _ => format!("{:.1}M", n as f64 / 1_000_000.0),
    }
}

/// Rewrite the page's counts, once a second and whenever the page has just
/// been built, and only while it is up.
///
/// Scene entities are the named ones that are not editor UI (`Node`) and not
/// editor helpers marked hidden from the Hierarchy, which is what the
/// Hierarchy itself lists.
#[allow(clippy::type_complexity)]
pub(crate) fn refresh_scene_stats(
    time: Res<Time>,
    mut last: Local<f32>,
    fresh: Query<(), Added<SceneStat>>,
    mut texts: Query<(&SceneStat, &mut Text)>,
    scene: Query<Option<&Mesh3d>, (With<Name>, Without<Node>, Without<renzora::HideInHierarchy>)>,
    meshes: Res<Assets<Mesh>>,
) {
    if texts.is_empty() {
        return;
    }
    let now = time.elapsed_secs();
    if fresh.is_empty() && now - *last < 1.0 {
        return;
    }
    *last = now;
    let (mut entities, mut mesh_count, mut tris) = (0u64, 0u64, 0u64);
    for mesh in &scene {
        entities += 1;
        if let Some(m) = mesh {
            mesh_count += 1;
            if let Some(mesh) = meshes.get(&m.0) {
                let n = mesh.indices().map(|i| i.len()).unwrap_or_else(|| mesh.count_vertices());
                tris += (n / 3) as u64;
            }
        }
    }
    for (stat, mut text) in &mut texts {
        let want = match stat.0 {
            StatKind::Entities => short_count(entities),
            StatKind::Meshes => short_count(mesh_count),
            StatKind::Triangles => short_count(tris),
            StatKind::Subtitle { boot } => {
                let mut s = format!(
                    "{} \u{00B7} {} {}",
                    renzora::lang::t_or("inspector.scene.kind", "Scene"),
                    short_count(entities),
                    renzora::lang::t_or("inspector.scene.entities", "entities"),
                );
                if boot {
                    s.push_str(&format!(" \u{00B7} {}", renzora::lang::t_or("inspector.scene.boot", "boot scene")));
                }
                s
            }
        };
        if text.0 != want {
            text.0 = want;
        }
    }
}

/// "Set as Boot Scene": point the project's boot scene at the active scene
/// document and save the project file. The same thing the top bar's scene
/// switcher does; the page offers it only for a saved scene that is not
/// already the boot scene.
pub(crate) fn set_boot_scene_click(
    q: Query<&UiInteraction, (With<SetBootSceneBtn>, Changed<UiInteraction>)>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    if !q.iter().any(|i| *i == UiInteraction::Pressed) {
        return;
    }
    cmds.push(|w: &mut World| {
        let Some((_, Some(rel))) = active_scene(w) else {
            return;
        };
        let Some(mut project) = w.get_resource_mut::<renzora::CurrentProject>() else {
            return;
        };
        project.config.main_scene = rel.clone();
        match project.save_config() {
            Ok(()) => renzora::core::console_log::console_success("Scene", format!("Boot scene set to {rel}")),
            Err(e) => renzora::core::console_log::console_error("Scene", format!("Failed to save project config: {e}")),
        }
    });
}

/// "Add World Environment": spawn the World Environment preset, undoably,
/// exactly as Add Entity does.
pub(crate) fn add_world_env_click(
    q: Query<&UiInteraction, (With<AddWorldEnvBtn>, Changed<UiInteraction>)>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    if !q.iter().any(|i| *i == UiInteraction::Pressed) {
        return;
    }
    cmds.push(|w: &mut World| {
        renzora_undo::execute(
            w,
            renzora_undo::UndoContext::Scene,
            Box::new(renzora_undo::SpawnEntityCmd {
                entity: Entity::PLACEHOLDER,
                kind: renzora_undo::SpawnEntityKind::Preset { id: "world_environment".to_string() },
            }),
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_shorten() {
        assert_eq!(short_count(812), "812");
        assert_eq!(short_count(34_500), "34.5K");
        assert_eq!(short_count(1_200_000), "1.2M");
    }

    #[test]
    fn scene_paths_compare_loosely() {
        assert!(same_scene("scenes\\main.bsn", "scenes/main.bsn"));
        assert!(!same_scene("", ""));
        assert!(!same_scene("scenes/a.bsn", "scenes/b.bsn"));
    }
}
