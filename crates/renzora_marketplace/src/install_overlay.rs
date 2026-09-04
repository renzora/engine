//! The marketplace "Get / Install" flow: a permissions-style confirmation modal
//! (mirroring `File → Install Plugin…`) that shows what's being installed and,
//! for most assets, lets the user **pick where it lands** via a folder tree of
//! the project's own asset directories. On confirm, the asset downloads on a
//! background thread and extracts/writes into the chosen folder; a result notice
//! reports success.
//!
//! **Plugins are the exception.** A plugin only works from the engine's
//! `plugins/` directory — it is dlopen'd from there at startup — so there is no
//! choice to offer and no tree to show. Their modal states the destination
//! instead, and the finished notice offers the restart that actually loads it.
//!
//! Gating (sign-in / paid ownership) happens at the card before this opens — by
//! the time we get here the asset is known to be installable, either through the
//! authenticated download endpoint or, for free assets, the public preview proxy.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver};
use renzora::RenzoraShellExt;

use crate::auth::marketplace::AssetSummary;
use crate::auth::session::AuthSession;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_display, bind_text};
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    button, folder_new_button, folder_picker, overlay_sized, overlay_val, FolderPick,
};
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

/// What an install worker publishes as it goes, read by the status-bar item.
///
/// Atomics rather than a channel: the status bar samples this every frame and
/// only ever wants the latest value, so there is nothing to queue.
#[derive(Default)]
pub(crate) struct InstallShared {
    /// Bytes downloaded so far. Meaningful only in [`Phase::Downloading`].
    bytes: AtomicU64,
    /// Bytes expected in total, or 0 when unknown — a free asset fetched
    /// through the public proxy never resolves a file list, so its bar sweeps
    /// instead of filling.
    total: AtomicU64,
    /// The current [`Phase`], as its `u8`.
    phase: AtomicU8,
}

/// Where an install has got to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Asking the server for a download URL (an authenticated round trip).
    Resolving = 0,
    Downloading = 1,
    /// Unpacking into the destination folder.
    Writing = 2,
}

impl Phase {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => Phase::Downloading,
            2 => Phase::Writing,
            _ => Phase::Resolving,
        }
    }
}

/// One install running on a background thread.
///
/// A `Vec` of these rather than a single slot, because the confirm overlay
/// closes the instant you press Install: the editor is usable again straight
/// away and there is nothing to stop you starting a second one. A single slot
/// would drop the first install's result on the floor when the second replaced
/// it — the download would finish and simply never be reported.
pub(crate) struct InstallJob {
    name: String,
    category: String,
    rx: Receiver<Result<String, String>>,
    shared: Arc<InstallShared>,
}

/// Installs currently running. Carries each asset's category so a finished
/// **theme** install can refresh the theme picker (via
/// `ThemeManager::scan_themes`) without the user reopening the project — flat
/// themes are otherwise only rescanned on project load.
#[derive(Resource, Default)]
pub(crate) struct InstallJobs(Vec<InstallJob>);

/// The install the open overlay is showing, once Install has been pressed.
///
/// The overlay used to close on confirm and hand the user a toast, which is a
/// poor trade: the thing you were looking at vanishes and the feedback moves to
/// the corner. It now stays put and becomes the progress view, then the result.
#[derive(Resource)]
pub(crate) struct OverlayInstall {
    pub(crate) overlay: Entity,
    pub(crate) name: String,
    pub(crate) shared: Arc<InstallShared>,
    /// Set when the worker finishes; until then the bar is live.
    pub(crate) outcome: Option<Result<String, String>>,
    /// A plugin needs a restart to load, so the result offers one.
    pub(crate) offer_restart: bool,
}

/// The whole determinate bar, hidden while the total is unknown.
#[derive(Component)]
pub(crate) struct InstallBarTrack;
/// The sweeping bar, shown only when there is no total to fill against.
#[derive(Component)]
pub(crate) struct InstallBusyBar;
/// "Close" on the finished-install view.
#[derive(Component)]
pub(crate) struct InstallCloseBtn;

#[derive(Component)]
pub(crate) struct InstallConfirmBtn;
#[derive(Component)]
pub(crate) struct InstallDismissBtn(Entity);
/// "Restart Editor" on the finished-install notice.
#[derive(Component)]
pub(crate) struct InstallRestartBtn;

pub(crate) fn register(app: &mut App) {
    app.init_resource::<InstallJobs>();
    app.add_systems(
        Update,
        (
            install_buttons,
            poll_install_result,
            restart_button,
            close_button,
            drive_install_bar,
            forget_closed_overlay,
        ),
    );
    // A background install is invisible without this: the confirm overlay
    // closes on Install and the next thing that happens is a modal, minutes
    // later. The status bar is where a job that outlives its dialog belongs.
    app.register_shell_status_item(renzora::ShellStatusItem {
        id: "marketplace.install",
        align: renzora::ShellStatusAlign::Left,
        order: -50,
        render: install_status_segments,
    });
}

/// One status-bar segment per running install.
fn install_status_segments(world: &World) -> Vec<renzora::ShellStatusSegment> {
    let Some(jobs) = world.get_resource::<InstallJobs>() else {
        return Vec::new();
    };
    jobs.0
        .iter()
        .map(|job| {
            let phase = Phase::from_u8(job.shared.phase.load(Ordering::Relaxed));
            let text = match phase {
                Phase::Resolving => format!("Installing {}…", job.name),
                Phase::Downloading => format!(
                    "Installing {} — {}",
                    job.name,
                    human_bytes(job.shared.bytes.load(Ordering::Relaxed))
                ),
                Phase::Writing => format!("Installing {} — writing files", job.name),
            };
            renzora::ShellStatusSegment::new("download-simple", text, accent_rgb())
                .bar(renzora::ShellStatusBar::Busy)
        })
        .collect()
}

/// Bytes as a short human string. One decimal above a megabyte and none below:
/// this sits in an 11px status bar, and "4.2 MB" changing to "4.3 MB" is legible
/// movement where "4,394,124 bytes" is noise.
fn human_bytes(n: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    if n >= MB {
        format!("{:.1} MB", n as f64 / MB as f64)
    } else if n >= KB {
        format!("{} KB", n / KB)
    } else {
        format!("{n} B")
    }
}

/// The install action's green — the same one the website's asset page uses for
/// its Download button.
const GREEN: (u8, u8, u8) = (22, 163, 74);

fn accent_rgb() -> [u8; 3] {
    let (r, g, b) = accent();
    [r, g, b]
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
    // Only for the picker's benefit — a plugin never lands in the project, so
    // don't create a `plugins/` folder there that nothing will ever use.
    if !install::is_plugin_category(&asset.category) {
        let _ = std::fs::create_dir_all(&default_dest);
    }

    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, world);

    // A plugin has no folder tree, and 460px of dialog around four rows of text
    // is mostly empty space. Only the tree needs a fixed height — it is the one
    // thing here that scrolls, and it has to be given room to.
    let is_plugin = install::is_plugin_category(&asset.category);
    let height = if is_plugin { Val::Auto } else { Val::Px(460.0) };
    let (overlay, content) =
        overlay_val(&mut commands, &fonts, "Install Asset", Val::Px(560.0), height, true);

    // Above the item overlay, which is what opened this. `overlay_sized` hands
    // out `GlobalZIndex(8000)` — fine for a modal raised from a panel, but the
    // item overlay deliberately sits at 9600 to clear the docked panels, so the
    // default put the install dialog 1600 BEHIND the thing that launched it: the
    // backdrop dimmed, and the card itself was hidden. Between the item overlay
    // and the lightbox at 9900, which stays topmost.
    commands.entity(overlay).insert(GlobalZIndex(9700));

    let price = if asset.price_credits == 0 {
        "Free".to_string()
    } else {
        format!("{} credits", asset.price_credits)
    };
    // Header: the artwork on the left, the details beside it. Stacked, the four
    // label/value rows read as a form to fill in; next to the thing they
    // describe they read as a confirmation of what is about to be installed,
    // which is what this overlay is for.
    let header = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FlexStart,
            column_gap: Val::Px(12.0),
            ..default()
        })
        .id();
    let thumb = install_thumb(&mut commands, &fonts, &asset);
    let details = commands
        .spawn(Node {
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    let rows = [
        info_row(&mut commands, &fonts, "Asset", &asset.name),
        info_row(&mut commands, &fonts, "Category", &asset.category),
        info_row(&mut commands, &fonts, "Creator", &asset.creator_name),
        info_row(&mut commands, &fonts, "Price", &price),
    ];
    commands.entity(details).add_children(&rows);
    commands.entity(header).add_children(&[thumb, details]);

    // A plugin has exactly one valid home, so it gets a statement rather than a
    // picker; everything else gets the tree.
    let mut kids = vec![header];
    let mut picker = Entity::PLACEHOLDER;
    if is_plugin {
        // Already installed? Say so, and say what pressing the button will do —
        // reinstalling the same asset replaces it in place rather than adding a
        // second copy.
        let existing = crate::installed::find_by_asset(&asset.id);
        match &existing {
            Some(p) if p.version == asset.version => {
                kids.push(info_row(&mut commands, &fonts, "Installed", &format!("v{} as '{}'", p.version, p.dir_name)));
                kids.push(paragraph(
                    &mut commands,
                    &fonts,
                    "This plugin is already installed at this version. Installing \
                     again replaces it and rebuilds it on the next start.",
                    rgb(text_muted()),
                ));
            }
            Some(p) => {
                kids.push(info_row(&mut commands, &fonts, "Installed", &format!("v{} as '{}'", p.version, p.dir_name)));
                kids.push(paragraph(
                    &mut commands,
                    &fonts,
                    &format!(
                        "Updating to v{}. It replaces the copy you have and is rebuilt on the next start.",
                        asset.version
                    ),
                    rgb(text_muted()),
                ));
            }
            None => {
                kids.push(info_row(&mut commands, &fonts, "Installs into", "plugins/"));
                kids.push(paragraph(
                    &mut commands,
                    &fonts,
                    "Plugins load from the engine's plugins folder at startup, so this \
                     one installs there. Plugins are native code with full editor \
                     privileges — only install ones from sources you trust.",
                    rgb(text_muted()),
                ));
            }
        }
    } else {
        kids.push(section_label(&mut commands, &fonts, "Install into"));
        // Destination: the project's own directory structure, via ember's shared
        // picker (the same widget the hierarchy's Create-asset overlay uses). It
        // flex-grows to fill the overlay so the buttons stay pinned to the bottom.
        picker = folder_picker(&mut commands, &fonts, &root, &default_dest, 1);
        kids.push(picker);
        kids.push(paragraph(
            &mut commands,
            &fonts,
            "Renzora downloads this asset and writes its files into the folder you \
             pick above. Only install assets from sources you trust.",
            rgb(text_muted()),
        ));
    }

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
    // Green, and named for what it does: a plugin is installed, everything else
    // is downloaded into your project.
    // The button says what will happen, which for an installed plugin is not
    // "Install".
    let label = if !is_plugin {
        "Download"
    } else if crate::installed::find_by_asset(&asset.id).is_some_and(|p| p.version != asset.version) {
        "Update"
    } else if crate::installed::find_by_asset(&asset.id).is_some() {
        "Reinstall"
    } else {
        "Install"
    };
    let install_btn = crate::util::pill_button(&mut commands, &fonts, label, GREEN, (255, 255, 255));
    commands.entity(install_btn).insert(InstallConfirmBtn);
    if is_plugin {
        commands.entity(buttons).add_children(&[cancel, install_btn]);
    } else {
        // New Folder rides in the button row rather than under the tree — one row
        // of controls, not two. It floats at the row's left edge (absolute, out
        // of flow), so the Cancel/Install pair lays out untouched.
        let new_folder = folder_new_button(&mut commands, &fonts, picker);
        commands.entity(buttons).add_children(&[new_folder, cancel, install_btn]);
    }
    kids.push(buttons);

    // Everything above is the confirm step; wrap it so pressing Install can
    // swap it out for the progress view rather than closing the overlay.
    let confirm_step = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() })
        .id();
    commands.entity(confirm_step).add_children(&kids);
    bind_display(&mut commands, confirm_step, |w| {
        w.get_resource::<OverlayInstall>().is_none()
    });

    let progress_step = build_progress_step(&mut commands, &fonts);
    bind_display(&mut commands, progress_step, |w| {
        w.get_resource::<OverlayInstall>().is_some()
    });
    let kids = vec![confirm_step, progress_step];

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
    // A previous install's state would otherwise decide which face this new
    // overlay opens on.
    world.remove_resource::<OverlayInstall>();
    world.insert_resource(PendingInstall { asset, overlay, default_dest, session });
}

/// Confirm / cancel the install.
fn install_buttons(
    confirm: Query<&Interaction, (With<InstallConfirmBtn>, Changed<Interaction>)>,
    dismiss: Query<(&Interaction, &InstallDismissBtn), Changed<Interaction>>,
    pending: Option<Res<PendingInstall>>,
    pick: Res<FolderPick>,
    mut jobs: ResMut<InstallJobs>,
    mut commands: Commands,
) {
    for (interaction, btn) in &dismiss {
        if *interaction == Interaction::Pressed {
            commands.entity(btn.0).despawn();
            commands.remove_resource::<PendingInstall>();
            commands.remove_resource::<OverlayInstall>();
        }
    }

    if !confirm.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Some(pending) = pending else { return };
    let asset = pending.asset.clone();
    // A plugin shows no picker, so `FolderPick` still holds whatever the last
    // non-plugin install chose — take the category's own directory instead of
    // that stale path.
    let dest = if install::install_dir_for_category(&pending.asset.category) == "plugins" {
        pending.default_dest.clone()
    } else {
        pick.path().map(Path::to_path_buf).unwrap_or_else(|| pending.default_dest.clone())
    };
    let session = pending.session.as_ref().map(clone_session);
    let overlay = pending.overlay;
    commands.remove_resource::<PendingInstall>();

    let (tx, rx) = unbounded();
    let shared = Arc::new(InstallShared::default());
    jobs.0.push(InstallJob {
        name: asset.name.clone(),
        category: asset.category.clone(),
        rx,
        shared: shared.clone(),
    });

    // The overlay stays and turns into the progress view. No toast: the press
    // has a visible consequence right where the user is looking.
    commands.insert_resource(OverlayInstall {
        overlay,
        name: asset.name.clone(),
        shared: shared.clone(),
        outcome: None,
        offer_restart: install::install_dir_for_category(&asset.category) == "plugins",
    });

    spawn_install(session, asset, dest, tx, shared);
}

/// Raise the completion notice when a background install finishes.
fn poll_install_result(
    jobs: Option<ResMut<InstallJobs>>,
    fonts: Option<Res<EmberFonts>>,
    mut active: Option<ResMut<OverlayInstall>>,
    mut theme_manager: Option<ResMut<ThemeManager>>,
    mut commands: Commands,
) {
    let (Some(mut jobs), Some(fonts)) = (jobs, fonts) else { return };
    // Drain every job that has an answer, keep the rest. `retain` rather than an
    // index scan because a finished job is removed while its neighbours keep
    // running.
    let mut finished: Vec<(String, String, Result<String, String>)> = Vec::new();
    jobs.0.retain(|job| match job.rx.try_recv() {
        Ok(outcome) => {
            finished.push((job.name.clone(), job.category.clone(), outcome));
            false
        }
        Err(_) => true,
    });
    if finished.is_empty() {
        return;
    }

    for (name, category, outcome) in finished {
        let dir = install::install_dir_for_category(&category);
        // Side effects first, and once, however the result is then reported.
        if let Ok(msg) = &outcome {
            renzora::core::console_log::console_info("Marketplace", msg.clone());
            // A freshly installed flat theme is only picked up by the picker on
            // a rescan; do it now so it appears without reopening the project.
            if dir == "themes" {
                if let Some(manager) = theme_manager.as_mut() {
                    manager.scan_themes();
                }
            }
        }

        // If this is the install the overlay is showing, it reports the result
        // itself — a modal on top of the overlay that is already saying the same
        // thing would be two notices for one event.
        if let Some(a) = active.as_deref_mut() {
            if a.name == name && a.outcome.is_none() {
                a.outcome = Some(outcome);
                continue;
            }
        }

        let (title, body) = match outcome {
            Ok(msg) => ("Asset Installed".to_string(), msg),
            Err(e) => ("Install Failed".to_string(), e),
        };

        // Otherwise it outlived its overlay (closed, or a second install), so it
        // falls back to the notice. A plugin is opened once during `App`
        // assembly, so a new one on disk is not a new one in the process — the
        // notice offers the restart rather than leaving the user to work out
        // that the thing they just installed is not there.
        let offer_restart = dir == "plugins" && title == "Asset Installed";
        let f = fonts.clone();
        commands.queue(move |world: &mut World| {
            let mut queue = CommandQueue::default();
            {
                let mut commands = Commands::new(&mut queue, world);
                spawn_notice(&mut commands, &f, &title, &body, offer_restart);
            }
            queue.apply(world);
        });
    }
}

/// "Restart Editor" on the install notice.
fn restart_button(q: Query<&Interaction, (With<InstallRestartBtn>, Changed<Interaction>)>) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        renzora::restart_process();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_install(
    session: Option<AuthSession>,
    asset: AssetSummary,
    dest: PathBuf,
    tx: crossbeam_channel::Sender<Result<String, String>>,
    shared: Arc<InstallShared>,
) {
    std::thread::spawn(move || {
        let _ = tx.send(run_install(session.as_ref(), &asset, &dest, &shared));
    });
}

#[cfg(target_arch = "wasm32")]
fn spawn_install(
    _session: Option<AuthSession>,
    _asset: AssetSummary,
    _dest: PathBuf,
    tx: crossbeam_channel::Sender<Result<String, String>>,
    _shared: Arc<InstallShared>,
) {
    let _ = tx.send(Err("Downloads aren't supported in the browser yet".into()));
}

/// Fetch the asset bytes (authenticated download when signed in, otherwise the
/// public preview proxy for free assets) and install into `dest`.
#[cfg(not(target_arch = "wasm32"))]
fn run_install(
    session: Option<&AuthSession>,
    asset: &AssetSummary,
    dest: &Path,
    shared: &InstallShared,
) -> Result<String, String> {
    use crate::auth::marketplace as mk;
    let mut on_bytes = |n: u64| shared.bytes.store(n, Ordering::Relaxed);
    let (bytes, filename, url) = if let Some(s) = session.filter(|s| s.is_signed_in()) {
        // Resolving is a round trip of its own, and on a slow link it is a
        // second or two of a status bar that would otherwise claim to be
        // downloading nothing.
        shared.phase.store(Phase::Resolving as u8, Ordering::Relaxed);
        let dl = mk::download_asset(s, &asset.id)?;
        // The catalogue knows the size even though the transport does not, so
        // the bar can fill rather than sweep.
        shared.total.store(dl.total_bytes().unwrap_or(0), Ordering::Relaxed);
        shared.phase.store(Phase::Downloading as u8, Ordering::Relaxed);
        let bytes = mk::download_file_progress(&dl.download_url, &mut on_bytes)?;
        (bytes, dl.download_filename, dl.download_url)
    } else if asset.price_credits == 0 {
        let url = mk::preview_file_url(&asset.id);
        shared.phase.store(Phase::Downloading as u8, Ordering::Relaxed);
        let bytes = mk::download_file_progress(&url, &mut on_bytes)?;
        (bytes, String::new(), url)
    } else {
        return Err("Sign in to download this asset".into());
    };
    shared.phase.store(Phase::Writing as u8, Ordering::Relaxed);

    // A plugin is a source tree, not an asset file. It goes to the engine's own
    // `plugins/` directory under its crate name, where `prebuild` compiles it on
    // the next launch — `dest` (a project folder) is not somewhere anything
    // would ever look for it.
    if install::is_plugin_category(&asset.category) {
        let done = install::install_plugin_source(&asset.id, &bytes)?;
        // The sidecar ties the installed source back to its listing, and it is
        // also what marks this directory as marketplace-owned: `xtask`'s
        // `prune_orphans` deletes any staged plugin directory without one, since
        // that is how it recognises a leftover copy of a repo plugin. Failing
        // the install is better than leaving a plugin the next `cargo renzora`
        // would silently delete.
        let meta = install::PluginSidecar {
            asset_id: asset.id.clone(),
            name: asset.name.clone(),
            slug: asset.slug.clone(),
            version: asset.version.clone(),
            category: asset.category.clone(),
            crate_name: done.dir_name.clone(),
            ..Default::default()
        };
        if let Err(e) = install::write_plugin_sidecar(&done.path, &meta) {
            let _ = std::fs::remove_dir_all(&done.path);
            return Err(format!("Could not finish installing '{}': {e}", done.dir_name));
        }
        let verb = if done.updated { "Updated" } else { "Installed" };
        // A rename is not a footnote: the plugin builds and loads under the new
        // name, so anything the user does with it later uses that name.
        let renamed = match &done.renamed_from {
            Some(wanted) => format!(
                " Another plugin already uses '{wanted}', so this one installed as '{}'.",
                done.dir_name
            ),
            None => String::new(),
        };
        return Ok(format!(
            "{verb} \"{}\" as plugin '{}'.{renamed} It is built on the next start.",
            asset.name, done.dir_name
        ));
    }

    let path = install::install_asset_into(dest, &asset.category, &asset.name, &url, &filename, &bytes)?;
    Ok(format!("Installed \"{}\" into {}", asset.name, path.display()))
}

/// The overlay's second face: what Install turns it into.
///
/// Both bars are built up front and swapped by `bind_display` — which one is
/// right is not known until the server answers with a file size, and rebuilding
/// the subtree at that moment would restart the sweep animation.
fn build_progress_step(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.0),
            padding: UiRect::vertical(Val::Px(6.0)),
            ..default()
        })
        .id();

    let title = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, title, |w| match w.get_resource::<OverlayInstall>() {
        Some(s) => match &s.outcome {
            None => format!("Installing {}", s.name),
            Some(Ok(_)) => format!("{} installed", s.name),
            Some(Err(_)) => format!("{} failed to install", s.name),
        },
        None => String::new(),
    });

    // Determinate: the catalogue gave a size.
    let track = renzora_ember::widgets::progress_sized(commands, 0.0, 420.0, 8.0);
    commands.entity(track).insert(InstallBarTrack);
    bind_display(commands, track, |w| {
        w.get_resource::<OverlayInstall>()
            .is_some_and(|s| s.outcome.is_none() && s.shared.total.load(Ordering::Relaxed) > 0)
    });

    // Indeterminate: it did not, so the bar sweeps instead of lying.
    let busy = renzora_ember::widgets::progress_indeterminate(commands, 420.0, 8.0);
    commands.entity(busy).insert(InstallBusyBar);
    bind_display(commands, busy, |w| {
        w.get_resource::<OverlayInstall>()
            .is_some_and(|s| s.outcome.is_none() && s.shared.total.load(Ordering::Relaxed) == 0)
    });

    let caption = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_muted()))))
        .id();
    bind_text(commands, caption, |w| {
        let Some(s) = w.get_resource::<OverlayInstall>() else {
            return String::new();
        };
        if let Some(outcome) = &s.outcome {
            return match outcome {
                Ok(msg) => msg.clone(),
                Err(e) => e.clone(),
            };
        }
        let done = s.shared.bytes.load(Ordering::Relaxed);
        let total = s.shared.total.load(Ordering::Relaxed);
        match Phase::from_u8(s.shared.phase.load(Ordering::Relaxed)) {
            Phase::Resolving => "Preparing download…".to_string(),
            Phase::Downloading if total > 0 => format!(
                "{} of {} — {}%",
                human_bytes(done),
                human_bytes(total),
                (done as f64 / total as f64 * 100.0).min(100.0).round() as u32
            ),
            Phase::Downloading => format!("{} downloaded", human_bytes(done)),
            Phase::Writing => "Writing files…".to_string(),
        }
    });

    // Buttons appear only once there is something to decide.
    let buttons = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::FlexEnd,
            column_gap: Val::Px(8.0),
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        })
        .id();
    bind_display(commands, buttons, |w| {
        w.get_resource::<OverlayInstall>().is_some_and(|s| s.outcome.is_some())
    });
    let restart = crate::util::pill_button(commands, fonts, "Restart Editor", GREEN, (255, 255, 255));
    commands.entity(restart).insert(InstallRestartBtn);
    bind_display(commands, restart, |w| {
        w.get_resource::<OverlayInstall>()
            .is_some_and(|s| s.offer_restart && matches!(s.outcome, Some(Ok(_))))
    });
    let close = button(commands, &fonts.ui, "Close");
    commands.entity(close).insert(InstallCloseBtn);
    commands.entity(buttons).add_children(&[restart, close]);

    commands.entity(col).add_children(&[title, track, busy, caption, buttons]);
    col
}

/// "Close" on the finished view — the same teardown as Cancel, from the other
/// face of the overlay.
fn close_button(
    close: Query<&Interaction, (With<InstallCloseBtn>, Changed<Interaction>)>,
    active: Option<Res<OverlayInstall>>,
    mut commands: Commands,
) {
    if !close.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    if let Some(active) = &active {
        commands.entity(active.overlay).despawn();
    }
    commands.remove_resource::<OverlayInstall>();
}

/// Drop the active-install state once its overlay is gone.
///
/// The X, Escape and a backdrop click all despawn the overlay through ember's
/// own dismissal, which knows nothing about this resource. Left behind, it would
/// make the *next* install overlay open on the progress face of an install that
/// already finished. The install itself is unaffected — it keeps running and
/// falls back to the notice, which is what that fallback is for.
fn forget_closed_overlay(
    active: Option<Res<OverlayInstall>>,
    alive: Query<()>,
    mut commands: Commands,
) {
    let Some(active) = active else { return };
    if alive.get(active.overlay).is_err() {
        commands.remove_resource::<OverlayInstall>();
    }
}

/// Resize the determinate bar's fill from the live byte count.
///
/// A system rather than a binding because `bind_*` drives text and visibility,
/// and this is a `Node` width.
fn drive_install_bar(
    active: Option<Res<OverlayInstall>>,
    tracks: Query<&Children, With<InstallBarTrack>>,
    mut fills: Query<&mut Node>,
) {
    let Some(active) = active else { return };
    let total = active.shared.total.load(Ordering::Relaxed);
    if total == 0 {
        return;
    }
    let done = active.shared.bytes.load(Ordering::Relaxed);
    // Writing has no byte count of its own; leave the bar full rather than
    // letting it drop back to zero at the last moment.
    let frac = if active.outcome.is_some()
        || Phase::from_u8(active.shared.phase.load(Ordering::Relaxed)) == Phase::Writing
    {
        1.0
    } else {
        (done as f64 / total as f64).clamp(0.0, 1.0)
    };
    for children in &tracks {
        for child in children.iter() {
            if let Ok(mut node) = fills.get_mut(child) {
                node.width = Val::Percent(frac as f32 * 100.0);
            }
        }
    }
}

// ── Small UI helpers (mirror `plugin_install`) ────────────────────────────────

/// The asset's artwork, square, for the left of the header.
///
/// Fixed pixel size rather than an aspect ratio: this sits beside four rows of
/// text whose height does not change, so the artwork should not either — and a
/// square that grew with the overlay would push the detail rows into a narrow
/// column at the widths where the name is longest.
///
/// Falls back to the category glyph when the asset has no thumbnail, so the
/// header keeps its shape rather than collapsing to text.
fn install_thumb(commands: &mut Commands, fonts: &EmberFonts, a: &AssetSummary) -> Entity {
    const SIDE: f32 = 96.0;
    let frame = commands
        .spawn((
            Node {
                width: Val::Px(SIDE),
                height: Val::Px(SIDE),
                flex_shrink: 0.0,
                position_type: PositionType::Relative,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(rgb(hover_bg())),
        ))
        .id();

    let ph = renzora_ember::font::icon_text(commands, &fonts.phosphor, "package", text_muted(), 30.0);
    commands.entity(frame).add_child(ph);

    if let Some(url) = a.thumbnail_url.clone() {
        let img = commands
            .spawn((
                ImageNode::default(),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    display: Display::None,
                    ..default()
                },
            ))
            .id();
        renzora_ember::reactive::tracked::bind_with(
            commands,
            img,
            move |w| w.get_resource::<crate::thumbs::HubThumbs>().and_then(|t| t.get(&url)),
            |w, e, h: &Option<Handle<Image>>| {
                if let Some(h) = h {
                    if let Some(mut n) = w.get_mut::<ImageNode>(e) {
                        if n.image != *h {
                            n.image = h.clone();
                        }
                    }
                    if let Some(mut node) = w.get_mut::<Node>(e) {
                        node.display = Display::Flex;
                    }
                }
            },
        );
        commands.entity(frame).add_child(img);
    }
    frame
}

fn info_row(commands: &mut Commands, fonts: &EmberFonts, label: &str, value: &str) -> Entity {
    let row = commands
        .spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(10.0), ..default() })
        .id();
    let l = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            // Wide enough for "Installs into", the longest label here — at 70 it
            // wrapped onto two lines and pushed its value out of alignment.
            Node { width: Val::Px(88.0), flex_shrink: 0.0, ..default() },
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

/// The finished-install modal. `offer_restart` adds a **Restart Editor** button
/// beside OK, for the one category where finishing isn't enough.
fn spawn_notice(
    commands: &mut Commands,
    fonts: &EmberFonts,
    title: &str,
    body: &str,
    offer_restart: bool,
) {
    let height = if offer_restart { 200.0 } else { 170.0 };
    let (root, content) = overlay_sized(commands, fonts, title, 460.0, height, true);
    let body_node = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, padding: UiRect::all(Val::Px(14.0)), row_gap: Val::Px(8.0), ..default() })
        .id();
    let text = paragraph(commands, fonts, body, rgb(text_primary()));
    let mut kids = vec![text];
    if offer_restart {
        kids.push(paragraph(
            commands,
            fonts,
            "Plugins load when the editor starts, so this one won't appear until \
             you restart. Unsaved work is not saved for you.",
            rgb(text_muted()),
        ));
    }
    let buttons = commands
        .spawn(Node { flex_direction: FlexDirection::Row, justify_content: JustifyContent::FlexEnd, column_gap: Val::Px(8.0), ..default() })
        .id();
    let ok = button(commands, &fonts.ui, "OK");
    commands.entity(ok).insert(InstallDismissBtn(root));
    let mut btns = vec![ok];
    if offer_restart {
        let restart = button(commands, &fonts.ui, "Restart Editor");
        commands.entity(restart).insert(InstallRestartBtn);
        btns.push(restart);
    }
    commands.entity(buttons).add_children(&btns);
    kids.push(buttons);
    commands.entity(body_node).add_children(&kids);
    commands.entity(content).add_child(body_node);
}

fn clone_session(s: &AuthSession) -> AuthSession {
    AuthSession {
        user: s.user.clone(),
        access_token: s.access_token.clone(),
        refresh_token: s.refresh_token.clone(),
    }
}
