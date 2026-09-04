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

/// What an install worker publishes as it goes, read by the status-bar item.
///
/// Atomics rather than a channel: the status bar samples this every frame and
/// only ever wants the latest value, so there is nothing to queue.
#[derive(Default)]
pub(crate) struct InstallShared {
    /// Bytes downloaded so far. Meaningful only in [`Phase::Downloading`].
    bytes: AtomicU64,
    /// The current [`Phase`], as its `u8`.
    phase: AtomicU8,
}

/// Where an install has got to. There is no percentage anywhere in this: no
/// part of the transport or the catalogue reports a file's size, so the byte
/// count is the only true number and the bar stays indeterminate.
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
        (install_buttons, poll_install_result, restart_button),
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

    let (overlay, content) = overlay_sized(&mut commands, &fonts, "Install Asset", 560.0, 460.0, true);

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
    let is_plugin = install::install_dir_for_category(&asset.category) == "plugins";

    let mut kids = vec![header];
    let mut picker = Entity::PLACEHOLDER;
    if is_plugin {
        kids.push(info_row(&mut commands, &fonts, "Installs into", "plugins/"));
        kids.push(paragraph(
            &mut commands,
            &fonts,
            "Plugins load from the engine's plugins folder at startup, so this \
             one installs there. Plugins are native code with full editor \
             privileges — only install ones from sources you trust.",
            rgb(text_muted()),
        ));
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
    let install_btn = crate::util::pill_button(
        &mut commands,
        &fonts,
        if is_plugin { "Install" } else { "Download" },
        GREEN,
        (255, 255, 255),
    );
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
    mut jobs: ResMut<InstallJobs>,
    mut toasts: ResMut<crate::toasts::ToastQueue>,
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
    // A plugin shows no picker, so `FolderPick` still holds whatever the last
    // non-plugin install chose — take the category's own directory instead of
    // that stale path.
    let dest = if install::install_dir_for_category(&pending.asset.category) == "plugins" {
        pending.default_dest.clone()
    } else {
        pick.path().map(Path::to_path_buf).unwrap_or_else(|| pending.default_dest.clone())
    };
    let session = pending.session.as_ref().map(clone_session);
    commands.remove_resource::<PendingInstall>();

    // Say where it went. The overlay is gone by the time the download starts, so
    // without this the press has no visible consequence at all until the modal
    // arrives — which for a large asset is a long way off.
    toasts.push(
        crate::toasts::Tone::Info,
        format!("Installing {} in the background", asset.name),
        None,
    );

    let (tx, rx) = unbounded();
    let shared = Arc::new(InstallShared::default());
    jobs.0.push(InstallJob {
        name: asset.name.clone(),
        category: asset.category.clone(),
        rx,
        shared: shared.clone(),
    });
    spawn_install(session, asset, dest, tx, shared);
}

/// Raise the completion notice when a background install finishes.
fn poll_install_result(
    jobs: Option<ResMut<InstallJobs>>,
    fonts: Option<Res<EmberFonts>>,
    mut theme_manager: Option<ResMut<ThemeManager>>,
    mut commands: Commands,
) {
    let (Some(mut jobs), Some(fonts)) = (jobs, fonts) else { return };
    // Drain every job that has an answer, keep the rest. `retain` rather than an
    // index scan because a finished job is removed while its neighbours keep
    // running.
    let mut finished: Vec<(String, Result<String, String>)> = Vec::new();
    jobs.0.retain(|job| match job.rx.try_recv() {
        Ok(outcome) => {
            finished.push((job.category.clone(), outcome));
            false
        }
        Err(_) => true,
    });
    if finished.is_empty() {
        return;
    }

    for (category, outcome) in finished {
        let dir = install::install_dir_for_category(&category);
        let (title, body) = match outcome {
            Ok(msg) => {
                renzora::core::console_log::console_info("Marketplace", msg.clone());
                // A freshly installed flat theme is only picked up by the picker
                // on a rescan; do it now so it appears without reopening the
                // project.
                if dir == "themes" {
                    if let Some(manager) = theme_manager.as_mut() {
                        manager.scan_themes();
                    }
                }
                ("Asset Installed".to_string(), msg)
            }
            Err(e) => ("Install Failed".to_string(), e),
        };
        // A plugin is opened once, during `App` assembly, so a new one on disk
        // is not a new one in the process — the notice offers the restart rather
        // than leaving the user to work out that the thing they just installed
        // is not there.
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
        let path = install::install_plugin_source(&bytes)?;
        let crate_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
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
            crate_name: crate_name.clone(),
            ..Default::default()
        };
        if let Err(e) = install::write_plugin_sidecar(&path, &meta) {
            let _ = std::fs::remove_dir_all(&path);
            return Err(format!("Could not finish installing '{crate_name}': {e}"));
        }
        return Ok(format!(
            "Installed \"{}\" as plugin '{crate_name}'. It is built on the next start.",
            asset.name
        ));
    }

    let path = install::install_asset_into(dest, &asset.category, &asset.name, &url, &filename, &bytes)?;
    Ok(format!("Installed \"{}\" into {}", asset.name, path.display()))
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
