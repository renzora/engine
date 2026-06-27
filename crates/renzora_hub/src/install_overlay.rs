//! The marketplace "Get / Install" flow: a permissions-style confirmation modal
//! (mirroring `File → Install Plugin…`) that shows what's being installed and
//! lets the user **pick where it lands** via a folder tree of the project's own
//! asset directories. On confirm, the asset downloads on a background thread and
//! extracts/writes into the chosen folder; a result notice reports success.
//!
//! Gating (sign-in / paid ownership) happens at the card before this opens — by
//! the time we get here the asset is known to be installable, either through the
//! authenticated download endpoint or, for free assets, the public preview proxy.

use std::path::{Path, PathBuf};

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver};

use renzora_auth::marketplace::AssetSummary;
use renzora_auth::session::AuthSession;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::bind_bg;
use renzora_ember::theme::*;
use renzora_ember::widgets::{button, overlay_sized, scroll_view};

use crate::install;

/// The asset awaiting install confirmation plus the folder the user has picked.
/// Lives only while the confirm overlay is up; dismissing the overlay
/// (Escape / backdrop / X) leaves it inert until the next "Get" replaces it.
#[derive(Resource)]
pub(crate) struct PendingInstall {
    asset: AssetSummary,
    overlay: Entity,
    dest: PathBuf,
    /// Cloned signed-in session (if any) so the download thread can authenticate.
    session: Option<AuthSession>,
}

/// In-flight install result, polled to raise the completion notice.
#[derive(Resource)]
pub(crate) struct InstallResult(Receiver<Result<String, String>>);

#[derive(Component)]
pub(crate) struct InstallConfirmBtn;
#[derive(Component)]
pub(crate) struct InstallDismissBtn(Entity);
#[derive(Component)]
pub(crate) struct FolderRow(PathBuf);

pub(crate) fn register(app: &mut App) {
    app.add_systems(
        Update,
        (install_buttons, folder_click, poll_install_result),
    );
}

/// Open the confirm overlay for `asset`. Exclusive-world entry (queued from the
/// card's click system) so it can read `CurrentProject` / `AuthSession` and spawn
/// the folder tree in one shot.
pub(crate) fn open(world: &mut World, asset: AssetSummary) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    let Some(project) = world.get_resource::<renzora::core::CurrentProject>() else {
        return;
    };
    let root = project.path.clone();
    let session = world
        .get_resource::<AuthSession>()
        .filter(|s| s.is_signed_in())
        .map(clone_session);

    // Default destination = the category's conventional subfolder. Create it up
    // front so it shows in the tree even on a fresh project.
    let default_dest = root.join(install::install_dir_for_category(&asset.category));
    let _ = std::fs::create_dir_all(&default_dest);
    let folders = scan_dirs(&root);

    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, world);

    let (overlay, content) = overlay_sized(&mut commands, &fonts, "Install Asset", 560.0, 460.0, true);

    let price = if asset.price_credits == 0 {
        "Free".to_string()
    } else {
        format!("{} credits", asset.price_credits)
    };
    let mut kids = vec![
        info_row(&mut commands, &fonts, "Asset", &asset.name),
        info_row(&mut commands, &fonts, "Category", &asset.category),
        info_row(&mut commands, &fonts, "Creator", &asset.creator_name),
        info_row(&mut commands, &fonts, "Price", &price),
        section_label(&mut commands, &fonts, "Install into"),
    ];

    // Folder picker: the project's own directory structure. The bordered box
    // flex-grows to fill the overlay so the buttons stay pinned to the bottom
    // (no dead space), and the rows scroll inside it.
    let tree = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            ..default()
        })
        .id();
    let mut rows = Vec::new();
    for (path, depth, name) in &folders {
        rows.push(folder_row(&mut commands, &fonts, path.clone(), *depth, name));
    }
    commands.entity(tree).add_children(&rows);
    let tree_scroll = scroll_view(&mut commands, tree);
    let tree_box = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: Val::Px(60.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                padding: UiRect::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id();
    commands.entity(tree_box).add_child(tree_scroll);
    kids.push(tree_box);

    kids.push(paragraph(
        &mut commands,
        &fonts,
        "Renzora downloads this asset and writes its files into the folder you \
         pick above. Only install assets from sources you trust.",
        rgb(text_muted()),
    ));

    let buttons = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::FlexEnd,
            column_gap: Val::Px(8.0),
            margin: UiRect::top(Val::Px(8.0)),
            ..default()
        })
        .id();
    let cancel = button(&mut commands, &fonts.ui, "Cancel");
    commands.entity(cancel).insert(InstallDismissBtn(overlay));
    let install_btn = button(&mut commands, &fonts.ui, "Download & Install");
    commands.entity(install_btn).insert(InstallConfirmBtn);
    commands.entity(buttons).add_children(&[cancel, install_btn]);
    kids.push(buttons);

    // Pad the content so it isn't flush against the overlay edge.
    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            row_gap: Val::Px(6.0),
            padding: UiRect::all(Val::Px(14.0)),
            ..default()
        })
        .id();
    commands.entity(body).add_children(&kids);
    commands.entity(content).add_child(body);

    queue.apply(world);
    world.insert_resource(PendingInstall { asset, overlay, dest: default_dest, session });
}

/// Confirm / cancel the install.
fn install_buttons(
    confirm: Query<&Interaction, (With<InstallConfirmBtn>, Changed<Interaction>)>,
    dismiss: Query<(&Interaction, &InstallDismissBtn), Changed<Interaction>>,
    pending: Option<Res<PendingInstall>>,
    mut commands: Commands,
) {
    for (interaction, btn) in &dismiss {
        if *interaction == Interaction::Pressed {
            commands.entity(btn.0).despawn();
            commands.remove_resource::<PendingInstall>();
        }
    }

    if !confirm.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Some(pending) = pending else { return };
    commands.entity(pending.overlay).despawn();

    let asset = pending.asset.clone();
    let dest = pending.dest.clone();
    let session = pending.session.as_ref().map(clone_session);
    commands.remove_resource::<PendingInstall>();

    let (tx, rx) = unbounded();
    commands.insert_resource(InstallResult(rx));
    spawn_install(session, asset, dest, tx);
}

/// Click a folder row → it becomes the install destination.
fn folder_click(
    q: Query<(&Interaction, &FolderRow), Changed<Interaction>>,
    pending: Option<ResMut<PendingInstall>>,
) {
    let Some(mut pending) = pending else { return };
    for (interaction, row) in &q {
        if *interaction == Interaction::Pressed && pending.dest != row.0 {
            pending.dest = row.0.clone();
        }
    }
}

/// Raise the completion notice when the background install finishes.
fn poll_install_result(
    result: Option<Res<InstallResult>>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let (Some(result), Some(fonts)) = (result, fonts) else { return };
    let Ok(outcome) = result.0.try_recv() else { return };
    commands.remove_resource::<InstallResult>();
    let (title, body) = match outcome {
        Ok(msg) => {
            renzora::core::console_log::console_info("Marketplace", msg.clone());
            ("Asset Installed".to_string(), msg)
        }
        Err(e) => ("Install Failed".to_string(), e),
    };
    let f = fonts.clone();
    commands.queue(move |world: &mut World| {
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            spawn_notice(&mut commands, &f, &title, &body);
        }
        queue.apply(world);
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_install(
    session: Option<AuthSession>,
    asset: AssetSummary,
    dest: PathBuf,
    tx: crossbeam_channel::Sender<Result<String, String>>,
) {
    std::thread::spawn(move || {
        let _ = tx.send(run_install(session.as_ref(), &asset, &dest));
    });
}

#[cfg(target_arch = "wasm32")]
fn spawn_install(
    _session: Option<AuthSession>,
    _asset: AssetSummary,
    _dest: PathBuf,
    tx: crossbeam_channel::Sender<Result<String, String>>,
) {
    let _ = tx.send(Err("Downloads aren't supported in the browser yet".into()));
}

/// Fetch the asset bytes (authenticated download when signed in, otherwise the
/// public preview proxy for free assets) and install into `dest`.
#[cfg(not(target_arch = "wasm32"))]
fn run_install(session: Option<&AuthSession>, asset: &AssetSummary, dest: &Path) -> Result<String, String> {
    use renzora_auth::marketplace as mk;
    let (bytes, filename, url) = if let Some(s) = session.filter(|s| s.is_signed_in()) {
        let dl = mk::download_asset(s, &asset.id)?;
        let bytes = mk::download_file(&dl.download_url)?;
        (bytes, dl.download_filename, dl.download_url)
    } else if asset.price_credits == 0 {
        let url = mk::preview_file_url(&asset.id);
        let bytes = mk::download_file(&url)?;
        (bytes, String::new(), url)
    } else {
        return Err("Sign in to download this asset".into());
    };
    let path = install::install_asset_into(dest, &asset.name, &url, &filename, &bytes)?;
    // Plugins get a metadata sidecar next to the dll so a lean export can trace
    // it back to source and the official editor can fetch the right per-release
    // dll. Non-fatal: a missing sidecar doesn't fail the install.
    if install::install_dir_for_category(&asset.category) == "plugins" {
        let crate_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.strip_prefix("lib").unwrap_or(s).to_string())
            .unwrap_or_default();
        let meta = install::PluginSidecar {
            asset_id: asset.id.clone(),
            name: asset.name.clone(),
            slug: asset.slug.clone(),
            version: asset.version.clone(),
            category: asset.category.clone(),
            crate_name,
            ..Default::default()
        };
        if let Err(e) = install::write_plugin_sidecar(&path, &meta) {
            bevy::log::warn!("[hub] plugin sidecar not written: {e}");
        }
    }
    Ok(format!("Installed \"{}\" into {}", asset.name, path.display()))
}

// ── Folder tree ───────────────────────────────────────────────────────────────

/// Recursively list the project's directories (two levels deep), skipping
/// hidden / build / dependency folders, so the user can target any existing
/// asset folder. Capped to keep the list bounded on huge projects.
fn scan_dirs(root: &Path) -> Vec<(PathBuf, usize, String)> {
    fn rec(dir: &Path, depth: usize, max: usize, out: &mut Vec<(PathBuf, usize, String)>) {
        if depth > max || out.len() > 300 {
            return;
        }
        let Ok(read) = std::fs::read_dir(dir) else { return };
        let mut entries: Vec<PathBuf> = read
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        entries.sort();
        for path in entries {
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            out.push((path.clone(), depth, name));
            rec(&path, depth + 1, max, out);
        }
    }
    let mut out = Vec::new();
    rec(root, 0, 1, &mut out);
    out
}

fn folder_row(commands: &mut Commands, fonts: &EmberFonts, path: PathBuf, depth: usize, name: &str) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(22.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::left(Val::Px(8.0 + depth as f32 * 14.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            FolderRow(path.clone()),
            Name::new("install-folder"),
        ))
        .id();
    let p = path.clone();
    bind_bg(commands, row, move |w| {
        let selected = w.get_resource::<PendingInstall>().map(|pi| pi.dest == p).unwrap_or(false);
        if selected {
            rgb(accent()).with_alpha(0.20)
        } else if matches!(w.get::<Interaction>(row), Some(Interaction::Hovered) | Some(Interaction::Pressed)) {
            rgb(hover_bg())
        } else {
            Color::NONE
        }
    });
    let icon = icon_text(commands, &fonts.phosphor, "folder", text_muted(), 12.0);
    let lbl = commands
        .spawn((Text::new(name.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())))).id();
    commands.entity(row).add_children(&[icon, lbl]);
    row
}

// ── Small UI helpers (mirror `plugin_install`) ────────────────────────────────

fn info_row(commands: &mut Commands, fonts: &EmberFonts, label: &str, value: &str) -> Entity {
    let row = commands
        .spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(10.0), ..default() })
        .id();
    let l = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node { width: Val::Px(70.0), flex_shrink: 0.0, ..default() },
        ))
        .id();
    let v = commands
        .spawn((Text::new(value), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())))).id();
    commands.entity(row).add_children(&[l, v]);
    row
}

fn section_label(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node { margin: UiRect::top(Val::Px(4.0)), ..default() },
        ))
        .id()
}

fn paragraph(commands: &mut Commands, fonts: &EmberFonts, text: &str, color: Color) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 10.5),
            TextColor(color),
            Node { margin: UiRect::top(Val::Px(4.0)), ..default() },
        ))
        .id()
}

fn spawn_notice(commands: &mut Commands, fonts: &EmberFonts, title: &str, body: &str) {
    let (root, content) = overlay_sized(commands, fonts, title, 460.0, 170.0, true);
    let body_node = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, padding: UiRect::all(Val::Px(14.0)), row_gap: Val::Px(8.0), ..default() })
        .id();
    let text = paragraph(commands, fonts, body, rgb(text_primary()));
    let buttons = commands
        .spawn(Node { flex_direction: FlexDirection::Row, justify_content: JustifyContent::FlexEnd, ..default() })
        .id();
    let ok = button(commands, &fonts.ui, "OK");
    commands.entity(ok).insert(InstallDismissBtn(root));
    commands.entity(buttons).add_child(ok);
    commands.entity(body_node).add_children(&[text, buttons]);
    commands.entity(content).add_child(body_node);
}

fn clone_session(s: &AuthSession) -> AuthSession {
    AuthSession {
        user: s.user.clone(),
        access_token: s.access_token.clone(),
        refresh_token: s.refresh_token.clone(),
    }
}
