//! The splash dashboard's **Templates** page: browse starter templates and
//! create a project from one, before any project is open.
//!
//! # A starter template is a project
//!
//! Not a description of one, and not a library entry to instantiate later: the
//! download *is* the finished project. So this page has no install step and no
//! local cache. You pick a template, you pick a folder, and what lands there is
//! a project — openable, in your recents, indistinguishable from one you made
//! yourself. That is also why the folder is chosen *after* the template: the
//! template is the interesting decision, and a modal file dialog is a bad place
//! to still be making it.
//!
//! # Why it is a page and not a dropdown on Projects
//!
//! Because the options are downloaded, the choice is a browse rather than a
//! menu: it needs thumbnails, descriptions and a search box, none of which fit
//! in a file dialog. **New Project** stays on the Projects page next to the
//! button that comes here, so the fast path is still one click and never waits
//! on the network.
//!
//! # Why this lives in this crate rather than in `renzora_splash`
//!
//! The catalogue client and the session are here, and `renzora_splash` is a
//! dependency of the *runtime* — a splash that reached for them would put the
//! marketplace's whole dependency tree in the shipped game binary. So the page
//! registers from this side, through the registry `renzora_splash` exposes for
//! exactly this. `splash_plugins` is here for the same reason and this page
//! mirrors its shape deliberately.

use bevy::prelude::*;
use bevy::ui::widget::NodeImageMode;
use bevy::ui::FocusPolicy;
use crossbeam_channel::{unbounded, Receiver};

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_display, bind_text, bind_with, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::widgets::{scroll_view, text_input, EmberTextInput};
use renzora_splash::{register_splash_section, SplashSection, TEMPLATES_SECTION_ID};

use crate::auth::marketplace::{AssetSummary, MarketplaceListResponse};
use crate::auth::session::AuthSession;
use crate::thumbs::HubThumbs;
use crate::util::{hash64, session_clone, signed_in};

/// The category slug the marketplace files starter templates under.
const TEMPLATE_CATEGORY: &str = "starters";

fn card_bg() -> Color {
    Color::srgba(16.0 / 255.0, 18.0 / 255.0, 28.0 / 255.0, 0.86)
}
fn border_soft() -> Color {
    Color::srgba(1.0, 1.0, 1.0, 0.07)
}
fn muted() -> Color {
    Color::srgb(0.60, 0.63, 0.72)
}

#[derive(Resource, Default)]
struct TemplateStore {
    search: String,
    assets: Vec<AssetSummary>,
    loading: bool,
    error: Option<String>,
    initialized: bool,
    /// Set by the search field; consumed by [`refetch`].
    dirty: bool,
    rx: Option<Receiver<Result<MarketplaceListResponse, String>>>,
    /// Ids with a download in flight, so a second click on the same card is a
    /// no-op rather than a second copy landing in the same folder.
    creating: Vec<String>,
    jobs: Vec<Job>,
    /// The most recent attempt's outcome, shown as a banner under the toolbar.
    /// Success does not linger: it is replaced by the editor opening.
    notice: Option<Result<String, String>>,
}

struct Job {
    asset_id: String,
    rx: Receiver<Result<CreatedProject, String>>,
}

/// A project this page just wrote to disk, on its way to being opened.
struct CreatedProject {
    path: std::path::PathBuf,
    name: String,
}

// ── Markers ──────────────────────────────────────────────────────────────────

#[derive(Component)]
struct TemplateSearch;
#[derive(Component)]
struct TemplateUseBtn(AssetSummary);
#[derive(Component)]
struct TemplateRefreshBtn;

// ── Registration ─────────────────────────────────────────────────────────────

pub(crate) fn register(app: &mut App) {
    app.init_resource::<TemplateStore>();
    register_splash_section(
        app,
        // Ordered just after Projects: it is the other half of "start something",
        // and a page you reach from a button on Projects should be next to it.
        SplashSection::new(TEMPLATES_SECTION_ID, "blueprint", "Templates", 5, build),
    );
    app.add_systems(
        Update,
        (
            init_fetch,
            poll_list,
            refetch,
            search_sync,
            request_thumbs,
            use_click,
            refresh_click,
            poll_jobs,
        )
            .run_if(in_state(renzora::SplashState::Splash)),
    );
}

// ── Page ─────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let page = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(14.0),
                padding: UiRect::all(Val::Px(22.0)),
                ..default()
            },
            FocusPolicy::Pass,
            Name::new("splash-page-templates"),
        ))
        .id();

    let title = commands
        .spawn((
            Text::new("Templates".to_string()),
            ui_font(&fonts.ui, 21.0),
            TextColor(Color::srgb(0.90, 0.92, 0.96)),
            FocusPolicy::Pass,
        ))
        .id();
    let sub = commands
        .spawn((
            Text::new(
                "Start a project from a finished one. Pick a template, then choose a folder \
                 to put it in."
                    .to_string(),
            ),
            ui_font(&fonts.ui, 12.0),
            TextColor(muted()),
            FocusPolicy::Pass,
        ))
        .id();

    let toolbar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let search = build_search(commands, fonts);
    let refresh = small_button(commands, fonts, "arrows-clockwise", "Refresh");
    commands.entity(refresh).insert(TemplateRefreshBtn);
    commands.entity(toolbar).add_children(&[search, refresh]);

    let notice = notice_banner(commands, fonts);

    let list = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                padding: UiRect::right(Val::Px(6.0)),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    keyed_list(commands, list, listings_snapshot);
    let scroll = scroll_view(commands, list);

    commands
        .entity(page)
        .add_children(&[title, sub, toolbar, notice, scroll]);
    page
}

fn build_search(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                max_width: Val::Px(340.0),
                height: Val::Px(34.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::horizontal(Val::Px(11.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(10.0 / 255.0, 12.0 / 255.0, 20.0 / 255.0, 0.88)),
            BorderColor::all(border_soft()),
            FocusPolicy::Block,
        ))
        .id();
    let mag = icon_text(
        commands,
        &fonts.phosphor,
        "magnifying-glass",
        (150, 158, 178),
        14.0,
    );
    commands.entity(mag).insert(FocusPolicy::Pass);
    let field = text_input(commands, &fonts.ui, "Search templates…", "");
    commands.entity(field).insert((
        TemplateSearch,
        Node {
            flex_grow: 1.0,
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::NONE),
        BorderColor::all(Color::NONE),
    ));
    commands.entity(row).add_children(&[mag, field]);
    row
}

fn small_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                height: Val::Px(34.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(7.0),
                padding: UiRect::horizontal(Val::Px(13.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.06)),
            Interaction::default(),
            FocusPolicy::Block,
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, (224, 228, 240), 13.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let t = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 12.0),
            TextColor(Color::srgb(0.87, 0.89, 0.94)),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(btn).add_children(&[ic, t]);
    btn
}

/// The outcome of the last attempt. Only failures dwell here in practice: a
/// success is immediately followed by the editor opening over the top.
fn notice_banner(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(9.0),
                padding: UiRect::all(Val::Px(11.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.55, 0.20, 0.20, 0.20)),
            BorderColor::all(Color::srgba(1.0, 0.45, 0.45, 0.30)),
            FocusPolicy::Pass,
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, "warning", (255, 170, 170), 14.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let text = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.ui, 11.5),
            TextColor(Color::srgb(1.0, 0.82, 0.82)),
            FocusPolicy::Pass,
        ))
        .id();
    bind_text(commands, text, |w: &Rx| {
        w.get_resource::<TemplateStore>()
            .and_then(|s| s.notice.as_ref())
            .map(|n| match n {
                Ok(m) | Err(m) => m.clone(),
            })
            .unwrap_or_default()
    });
    commands.entity(row).add_children(&[ic, text]);
    bind_display(commands, row, |w: &Rx| {
        w.get_resource::<TemplateStore>()
            .is_some_and(|s| s.notice.is_some())
    });
    row
}

// ── Listings ─────────────────────────────────────────────────────────────────

fn listings_snapshot(world: &Rx) -> KeyedSnapshot {
    let Some(store) = world.get_resource::<TemplateStore>() else {
        return note_snapshot("");
    };
    if store.loading && store.assets.is_empty() {
        return note_snapshot("Loading templates…");
    }
    if let Some(err) = &store.error {
        return note_snapshot(&format!("Couldn't reach the marketplace: {err}"));
    }
    if store.assets.is_empty() {
        return note_snapshot(
            "No templates published yet. When there are, they'll show up here.",
        );
    }

    let signed = signed_in(world);
    let assets = store.assets.clone();
    let busy: Vec<String> = store.creating.clone();
    let items: Vec<(u64, u64)> = assets
        .iter()
        .map(|a| {
            let key = hash64(&("t", &a.id));
            let content = hash64(&(
                &a.name,
                &a.description,
                a.price_credits,
                busy.contains(&a.id),
                signed,
            ));
            (key, content)
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |commands, fonts, i| {
            let a = &assets[i];
            listing_card(commands, fonts, a, busy.contains(&a.id), signed)
        }),
    }
}

fn note_snapshot(message: &str) -> KeyedSnapshot {
    let message = message.to_string();
    KeyedSnapshot {
        items: if message.is_empty() {
            Vec::new()
        } else {
            vec![(hash64(&("note", &message)), 0)]
        },
        build: Box::new(move |commands, fonts, _| {
            commands
                .spawn((
                    Text::new(message.clone()),
                    ui_font(&fonts.ui, 12.0),
                    TextColor(muted()),
                    FocusPolicy::Pass,
                ))
                .id()
        }),
    }
}

fn listing_card(
    commands: &mut Commands,
    fonts: &EmberFonts,
    asset: &AssetSummary,
    busy: bool,
    signed: bool,
) -> Entity {
    let card = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(14.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(card_bg()),
            BorderColor::all(border_soft()),
            FocusPolicy::Block,
        ))
        .id();

    let thumb = build_thumb(commands, fonts, asset);

    let info = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let name = commands
        .spawn((
            Text::new(asset.name.clone()),
            ui_font(&fonts.ui, 13.5),
            TextColor(Color::srgb(0.90, 0.92, 0.96)),
            FocusPolicy::Pass,
        ))
        .id();
    let desc = commands
        .spawn((
            Text::new(elide(first_line(&asset.description), 220)),
            ui_font(&fonts.ui, 11.0),
            TextColor(muted()),
            FocusPolicy::Pass,
        ))
        .id();
    let by = commands
        .spawn((
            Text::new(format!(
                "{}  ·  {}",
                asset.creator_name,
                if asset.price_credits == 0 {
                    "Free".to_string()
                } else {
                    format!("{} credits", asset.price_credits)
                }
            )),
            ui_font(&fonts.ui, 10.0),
            TextColor(Color::srgb(0.45, 0.48, 0.56)),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(info).add_children(&[name, desc, by]);

    // One action, and it says what actually happens: you are not installing an
    // asset, you are getting a project.
    let (label, enabled) = if busy {
        ("Creating…", false)
    } else if asset.price_credits > 0 && !signed {
        ("Sign in", false)
    } else {
        ("Use this template", true)
    };
    let action = small_button(commands, fonts, "folder-plus", label);
    if enabled {
        commands
            .entity(action)
            .insert(TemplateUseBtn(asset.clone()));
    }

    commands.entity(card).add_children(&[thumb, info, action]);
    card
}

fn build_thumb(commands: &mut Commands, fonts: &EmberFonts, asset: &AssetSummary) -> Entity {
    let frame = commands
        .spawn((
            Node {
                width: Val::Px(96.0),
                height: Val::Px(64.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(7.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.04)),
            FocusPolicy::Pass,
        ))
        .id();
    match &asset.thumbnail_url {
        Some(url) => {
            let url = url.clone();
            let img = commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    ImageNode::default().with_mode(NodeImageMode::Stretch),
                    FocusPolicy::Pass,
                ))
                .id();
            bind_with(
                commands,
                img,
                move |w| w.get_resource::<HubThumbs>().and_then(|t| t.get(&url)),
                |w, e, handle: &Option<Handle<Image>>| {
                    let Some(h) = handle.clone() else { return };
                    if let Some(mut node) = w.get_mut::<ImageNode>(e) {
                        node.image = h;
                    }
                },
            );
            commands.entity(frame).add_child(img);
        }
        None => {
            let ic = icon_text(commands, &fonts.phosphor, "blueprint", (110, 150, 255), 24.0);
            commands.entity(ic).insert(FocusPolicy::Pass);
            commands.entity(frame).add_child(ic);
        }
    }
    frame
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("").trim()
}

fn elide(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", cut.trim_end())
}

// ── Fetching ─────────────────────────────────────────────────────────────────

fn init_fetch(mut store: ResMut<TemplateStore>) {
    if store.initialized {
        return;
    }
    store.initialized = true;
    fetch(&mut store);
}

fn refetch(mut store: ResMut<TemplateStore>) {
    if store.dirty {
        store.dirty = false;
        fetch(&mut store);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn fetch(store: &mut TemplateStore) {
    let query = (!store.search.trim().is_empty()).then(|| store.search.trim().to_string());
    let (tx, rx) = unbounded();
    store.rx = Some(rx);
    store.loading = true;
    store.error = None;
    std::thread::spawn(move || {
        let result = crate::auth::marketplace::list_assets(
            query.as_deref(),
            Some(TEMPLATE_CATEGORY),
            Some("popular"),
            1,
            None,
            None,
        );
        let _ = tx.send(result);
    });
}

#[cfg(target_arch = "wasm32")]
fn fetch(_store: &mut TemplateStore) {}

fn poll_list(mut store: ResMut<TemplateStore>) {
    let mut got = Vec::new();
    if let Some(rx) = store.rx.as_ref() {
        while let Ok(r) = rx.try_recv() {
            got.push(r);
        }
    }
    for r in got {
        store.loading = false;
        match r {
            // Deliberately unfiltered, unlike the Plugins page. That page shows
            // only first-party listings unprompted because a plugin is code
            // compiled into the editor process; a template is data you look at
            // before you commit to it, and the risk is not comparable.
            Ok(resp) => {
                store.assets = resp.assets;
                store.error = None;
            }
            Err(e) => store.error = Some(e),
        }
    }
}

fn request_thumbs(store: Res<TemplateStore>, mut thumbs: ResMut<HubThumbs>) {
    for a in &store.assets {
        if let Some(url) = &a.thumbnail_url {
            thumbs.request(url);
        }
    }
}

fn search_sync(
    inputs: Query<&EmberTextInput, With<TemplateSearch>>,
    mut store: ResMut<TemplateStore>,
) {
    for input in &inputs {
        if input.value != store.search {
            store.search = input.value.clone();
            store.dirty = true;
        }
    }
}

fn refresh_click(
    q: Query<&Interaction, (With<TemplateRefreshBtn>, Changed<Interaction>)>,
    mut store: ResMut<TemplateStore>,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        store.dirty = true;
    }
}

// ── Creating ─────────────────────────────────────────────────────────────────

fn use_click(
    q: Query<(&Interaction, &TemplateUseBtn), Changed<Interaction>>,
    session: Res<AuthSession>,
    mut store: ResMut<TemplateStore>,
) {
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let asset = btn.0.clone();
        if store.creating.contains(&asset.id) {
            continue;
        }

        // The folder dialog runs here, on the main thread, before anything is
        // downloaded: asking where to put it only after the bytes arrive would
        // mean a cancel had already cost the download.
        let Some(folder) = pick_folder(&asset.name) else {
            continue;
        };

        let session = session.is_signed_in().then(|| session_clone(&session));
        store.creating.push(asset.id.clone());
        store.notice = None;
        let (tx, rx) = unbounded();
        store.jobs.push(Job { asset_id: asset.id.clone(), rx });
        spawn_create(session, asset, folder, tx);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn pick_folder(template_name: &str) -> Option<std::path::PathBuf> {
    rfd::FileDialog::new()
        .set_title(format!("New project from \"{template_name}\" — choose a folder"))
        .pick_folder()
}

#[cfg(target_arch = "wasm32")]
fn pick_folder(_template_name: &str) -> Option<std::path::PathBuf> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_create(
    session: Option<AuthSession>,
    asset: AssetSummary,
    folder: std::path::PathBuf,
    tx: crossbeam_channel::Sender<Result<CreatedProject, String>>,
) {
    std::thread::Builder::new()
        .name("renzora-splash-template".to_string())
        .spawn(move || {
            let _ = tx.send(run_create(session.as_ref(), &asset, &folder));
        })
        .ok();
}

#[cfg(target_arch = "wasm32")]
fn spawn_create(
    _session: Option<AuthSession>,
    _asset: AssetSummary,
    _folder: std::path::PathBuf,
    tx: crossbeam_channel::Sender<Result<CreatedProject, String>>,
) {
    let _ = tx.send(Err("Creating from a template isn't supported in the browser yet".into()));
}

/// Download the template and write it into `folder` as a project.
///
/// The same two-door download the store's installer uses: the authenticated
/// endpoint when there is a session (which is also what enforces ownership), and
/// the public preview proxy for a free listing when there is not.
#[cfg(not(target_arch = "wasm32"))]
fn run_create(
    session: Option<&AuthSession>,
    asset: &AssetSummary,
    folder: &std::path::Path,
) -> Result<CreatedProject, String> {
    use crate::auth::marketplace as mk;

    let mut ignore = |_: u64| {};
    let bytes = if let Some(s) = session.filter(|s| s.is_signed_in()) {
        let dl = mk::download_asset(s, &asset.id)?;
        mk::download_file_progress(&dl.download_url, &mut ignore)?
    } else if asset.price_credits == 0 {
        mk::download_file_progress(&mk::preview_file_url(&asset.id), &mut ignore)?
    } else {
        return Err("Sign in to use this template".into());
    };

    let path = crate::install::install_starter_project(folder, &bytes)?;
    Ok(CreatedProject { path, name: asset.name.clone() })
}

/// Open the project as soon as one finishes writing.
///
/// Straight into it rather than back to the Projects list: the user asked for a
/// project and now has one, and a success notice they have to act on again is a
/// step that exists only because the code was easier to write that way.
fn poll_jobs(world: &mut World) {
    let mut done: Vec<(String, Result<CreatedProject, String>)> = Vec::new();
    {
        let Some(store) = world.get_resource::<TemplateStore>() else {
            return;
        };
        for job in &store.jobs {
            if let Ok(result) = job.rx.try_recv() {
                done.push((job.asset_id.clone(), result));
            }
        }
    }
    if done.is_empty() {
        return;
    }

    let mut opened = None;
    if let Some(mut store) = world.get_resource_mut::<TemplateStore>() {
        for (id, result) in done {
            store.jobs.retain(|j| j.asset_id != id);
            store.creating.retain(|c| c != &id);
            match result {
                Ok(created) => {
                    store.notice = None;
                    opened = Some(created);
                }
                Err(e) => store.notice = Some(Err(e)),
            }
        }
    }

    let Some(created) = opened else { return };
    match renzora_splash::open_project(&created.path.join("project.toml")) {
        Ok(project) => renzora_splash::enter_created_project(world, project),
        Err(e) => {
            if let Some(mut store) = world.get_resource_mut::<TemplateStore>() {
                store.notice = Some(Err(format!(
                    "Created '{}' but couldn't open it: {e}",
                    created.name
                )));
            }
        }
    }
}
