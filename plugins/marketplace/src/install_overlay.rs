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
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::theme::*;
use renzora_ember::widgets::{button, folder_new_button, folder_picker, overlay_sized, FolderPick};
use renzora_theme::ThemeManager;

use crate::install;

/// The asset awaiting install confirmation. Lives only while the confirm overlay
/// is up; dismissing the overlay (Escape / backdrop / X) leaves it inert until
/// the next "Get" replaces it. The chosen destination isn't here — it lives in
/// ember's [`FolderPick`], owned by the shared picker widget.
#[derive(Resource)]
pub(crate) struct PendingInstall {
    asset: AssetSummary,
    overlay: Entity,
    /// Where the picker was seeded, and the fallback if it somehow has no pick.
    default_dest: PathBuf,
    /// Cloned signed-in session (if any) so the download thread can authenticate.
    session: Option<AuthSession>,
}

/// In-flight install result, polled to raise the completion notice. Carries the
/// installed asset's category so a finished **theme** install can refresh the
/// theme picker (via `ThemeManager::scan_themes`) without the user reopening the
/// project — flat themes are otherwise only rescanned on project load.
#[derive(Resource)]
pub(crate) struct InstallResult {
    rx: Receiver<Result<String, String>>,
    category: String,
}

#[derive(Component)]
pub(crate) struct InstallConfirmBtn;
#[derive(Component)]
pub(crate) struct InstallDismissBtn(Entity);

pub(crate) fn register(app: &mut App) {
    app.add_systems(Update, (install_buttons, poll_install_result));
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

    // Destination: the project's own directory structure, via ember's shared
    // picker (the same widget the hierarchy's Create-asset overlay uses). It
    // flex-grows to fill the overlay so the buttons stay pinned to the bottom.
    let picker = folder_picker(&mut commands, &fonts, &root, &default_dest, 1);
    kids.push(picker);

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
    // New Folder rides in the button row rather than under the tree — one row of
    // controls, not two. It floats at the row's left edge (absolute, out of
    // flow), so the Cancel/Install pair lays out untouched.
    let new_folder = folder_new_button(&mut commands, &fonts, picker);
    let cancel = button(&mut commands, &fonts.ui, "Cancel");
    commands.entity(cancel).insert(InstallDismissBtn(overlay));
    let install_btn = button(&mut commands, &fonts.ui, "Download & Install");
    commands.entity(install_btn).insert(InstallConfirmBtn);
    commands.entity(buttons).add_children(&[new_folder, cancel, install_btn]);
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
    world.insert_resource(PendingInstall { asset, overlay, default_dest, session });
}

/// Confirm / cancel the install.
fn install_buttons(
    confirm: Query<&Interaction, (With<InstallConfirmBtn>, Changed<Interaction>)>,
    dismiss: Query<(&Interaction, &InstallDismissBtn), Changed<Interaction>>,
    pending: Option<Res<PendingInstall>>,
    pick: Res<FolderPick>,
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
    let dest = pick.path().map(Path::to_path_buf).unwrap_or_else(|| pending.default_dest.clone());
    let session = pending.session.as_ref().map(clone_session);
    commands.remove_resource::<PendingInstall>();

    let (tx, rx) = unbounded();
    let category = asset.category.clone();
    commands.insert_resource(InstallResult { rx, category });
    spawn_install(session, asset, dest, tx);
}

/// Raise the completion notice when the background install finishes.
fn poll_install_result(
    result: Option<Res<InstallResult>>,
    fonts: Option<Res<EmberFonts>>,
    mut theme_manager: Option<ResMut<ThemeManager>>,
    mut commands: Commands,
) {
    let (Some(result), Some(fonts)) = (result, fonts) else { return };
    let Ok(outcome) = result.rx.try_recv() else { return };
    let installed_theme = install::install_dir_for_category(&result.category) == "themes";
    commands.remove_resource::<InstallResult>();
    let (title, body) = match outcome {
        Ok(msg) => {
            renzora::core::console_log::console_info("Marketplace", msg.clone());
            // A freshly installed flat theme is only picked up by the picker on a
            // rescan; do it now so it appears without reopening the project.
            if installed_theme {
                if let Some(manager) = theme_manager.as_mut() {
                    manager.scan_themes();
                }
            }
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
    let path = install::install_asset_into(dest, &asset.category, &asset.name, &url, &filename, &bytes)?;
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
