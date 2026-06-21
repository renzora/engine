//! Bevy-native (ember) port of the egui `HubLibraryPanel` ("My Library"):
//! purchased assets fetched in the background, filtered, with a per-row
//! thumbnail and Install button. State lives in `HubLibraryData`; background
//! results arrive over a crossbeam channel polled each frame.

use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver};

use renzora_auth::marketplace::AssetSummary;
use renzora_auth::session::AuthSession;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_text, bind_with, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{text_input, EmberTextInput};
use renzora::SplashState;

use crate::install;
use crate::thumbs::HubThumbs;

const GREEN: (u8, u8, u8) = (52, 180, 96);
const RED: (u8, u8, u8) = (224, 80, 80);
/// Library assets shown per page.
const LIB_PER_PAGE: usize = 8;

enum LibraryResult {
    Assets(Result<Vec<AssetSummary>, String>),
    Install(Result<String, String>),
}

#[derive(Resource)]
struct HubLibraryData {
    assets: Vec<AssetSummary>,
    filter: String,
    loading: bool,
    error: Option<String>,
    status: Option<String>,
    installing_id: Option<String>,
    rx: Option<Receiver<LibraryResult>>,
    needs_refresh: bool,
    /// Current page index (0-based) into the filtered list.
    page: usize,
}

impl Default for HubLibraryData {
    fn default() -> Self {
        Self {
            assets: Vec::new(),
            filter: String::new(),
            loading: false,
            error: None,
            status: None,
            installing_id: None,
            rx: None,
            needs_refresh: true,
            page: 0,
        }
    }
}

impl HubLibraryData {
    fn filtered(&self) -> Vec<AssetSummary> {
        let f = self.filter.to_lowercase();
        self.assets
            .iter()
            .filter(|a| {
                f.is_empty() || a.name.to_lowercase().contains(&f) || a.category.to_lowercase().contains(&f)
            })
            .cloned()
            .collect()
    }

    fn total_pages(&self) -> usize {
        self.filtered().len().div_ceil(LIB_PER_PAGE).max(1)
    }

    /// `page` clamped into the valid range for the current filter.
    fn current_page(&self) -> usize {
        self.page.min(self.total_pages().saturating_sub(1))
    }
}

pub struct NativeHubLibrary;

impl Plugin for NativeHubLibrary {
    fn build(&self, app: &mut App) {
        app.init_resource::<HubLibraryData>();
        app.register_panel_content("hub_library", true, build);
        app.add_systems(
            Update,
            (
                poll_library,
                library_refresh,
                library_refresh_click,
                library_install_click,
                library_filter_sync,
                library_prev_click,
                library_next_click,
                request_lib_thumbs,
            )
                .run_if(in_state(SplashState::Editor)),
        );
    }
}

#[derive(Component)]
struct LibFilter;
#[derive(Component)]
struct LibRefreshBtn;
#[derive(Component)]
struct LibInstallBtn(String);
#[derive(Component)]
struct LibPrevBtn;
#[derive(Component)]
struct LibNextBtn;

// ── Build ────────────────────────────────────────────────────────────────────

fn signed_in(w: &World) -> bool {
    w.get_resource::<AuthSession>().map(|s| s.is_signed_in()).unwrap_or(false)
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            padding: UiRect::all(Val::Px(6.0)),
            ..default()
        })
        .id();

    // Signed-out empty state.
    let signed_out = centered(commands, fonts, "user", "Sign in to view your library", Some("Purchased assets will appear here"));
    bind_display(commands, signed_out, |w| !signed_in(w));

    // Signed-in column.
    let body = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() })
        .id();
    bind_display(commands, body, signed_in);

    // Toolbar: filter + refresh.
    let toolbar = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() })
        .id();
    let filter = text_input(commands, &fonts.ui, "Filter assets...", "");
    commands.entity(filter).insert((
        LibFilter,
        Node {
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
    ));
    let refresh = commands
        .spawn((
            Node { width: Val::Px(26.0), height: Val::Px(24.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(rgb(hover_bg())),
            Interaction::default(),
            LibRefreshBtn,
            Name::new("lib-refresh"),
        ))
        .id();
    let r_icon = icon_text(commands, &fonts.phosphor, "arrow-clockwise", text_primary(), 14.0);
    commands.entity(refresh).add_child(r_icon);
    commands.entity(toolbar).add_children(&[filter, refresh]);

    // Status / error line.
    let status = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_muted())))).id();
    bind_text(commands, status, |w| {
        let d = w.resource::<HubLibraryData>();
        if let Some(e) = &d.error { format!("\u{26a0} {e}") } else { d.status.clone().unwrap_or_default() }
    });
    renzora_ember::reactive::bind_text_color(commands, status, |w| {
        let d = w.resource::<HubLibraryData>();
        if d.error.is_some() { rgb(RED) } else { rgb(GREEN) }
    });
    bind_display(commands, status, |w| {
        let d = w.resource::<HubLibraryData>();
        d.error.is_some() || d.status.is_some()
    });

    // Asset list.
    let list = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() })
        .id();
    keyed_list(commands, list, library_snapshot);

    // Pager — Prev / "Page X / Y" / Next; hidden when there's a single page.
    let pager = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::Center, column_gap: Val::Px(10.0), margin: UiRect::top(Val::Px(4.0)), ..default() })
        .id();
    let prev = pager_btn(commands, fonts, "caret-left", LibPrevBtn);
    let indicator = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id();
    bind_text(commands, indicator, |w| {
        let d = w.resource::<HubLibraryData>();
        format!("Page {} / {}", d.current_page() + 1, d.total_pages())
    });
    let next = pager_btn(commands, fonts, "caret-right", LibNextBtn);
    commands.entity(pager).add_children(&[prev, indicator, next]);
    bind_display(commands, pager, |w| w.resource::<HubLibraryData>().total_pages() > 1);

    commands.entity(body).add_children(&[toolbar, status, list, pager]);
    commands.entity(root).add_children(&[signed_out, body]);
    root
}

fn pager_btn<M: Component>(commands: &mut Commands, fonts: &EmberFonts, icon: &str, marker: M) -> Entity {
    let btn = commands
        .spawn((
            Node { width: Val::Px(26.0), height: Val::Px(24.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(rgb(hover_bg())),
            Interaction::default(),
            marker,
            Name::new("lib-pager-btn"),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_primary(), 13.0);
    commands.entity(btn).add_child(ic);
    btn
}

fn library_snapshot(world: &World) -> KeyedSnapshot {
    let d = world.resource::<HubLibraryData>();
    if d.loading {
        return note_snapshot("Loading library...");
    }
    if d.assets.is_empty() {
        return note_snapshot("No purchased assets yet. Browse the Store to find assets.");
    }
    let all = d.filtered();
    let installing = d.installing_id.clone();
    if all.is_empty() {
        return note_snapshot("No matching assets.");
    }
    // Slice to the current page.
    let page = d.current_page();
    let start = page * LIB_PER_PAGE;
    let end = (start + LIB_PER_PAGE).min(all.len());
    let assets: Vec<AssetSummary> = all[start..end].to_vec();
    use std::hash::{Hash, Hasher};
    let items: Vec<(u64, u64)> = assets
        .iter()
        .map(|a| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            a.id.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (&a.name, &a.category, installing.as_deref() == Some(&a.id)).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| asset_row(c, f, &assets[i], installing.as_deref() == Some(&assets[i].id))),
    }
}

fn asset_row(commands: &mut Commands, fonts: &EmberFonts, a: &AssetSummary, installing: bool) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(58.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            BorderColor::all(rgb(border())),
            Name::new("lib-asset"),
        ))
        .id();

    // Thumbnail (44px) — package placeholder under an ImageNode that fills once
    // the thumbnail downloads.
    let thumb = commands
        .spawn((
            Node { width: Val::Px(44.0), height: Val::Px(44.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(3.0)), overflow: Overflow::clip(), flex_shrink: 0.0, ..default() },
            BackgroundColor(rgb(hover_bg())),
        ))
        .id();
    let ph = icon_text(commands, &fonts.phosphor, "package", text_muted(), 16.0);
    commands.entity(thumb).add_child(ph);
    if let Some(url) = a.thumbnail_url.clone() {
        let img = commands
            .spawn((
                ImageNode::default(),
                Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), display: Display::None, ..default() },
            ))
            .id();
        bind_with(
            commands,
            img,
            move |w| w.get_resource::<HubThumbs>().and_then(|t| t.get(&url)),
            |w, e, handle: &Option<Handle<Image>>| {
                if let Some(h) = handle {
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
        commands.entity(thumb).add_child(img);
    }

    // Name + category pill + install path.
    let info = commands
        .spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() })
        .id();
    let name = commands
        .spawn((Text::new(a.name.clone()), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())), bevy::text::TextLayout::no_wrap(), Node { overflow: Overflow::clip(), ..default() }))
        .id();
    let meta = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() })
        .id();
    let pill = commands
        .spawn((Node { padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(2.0)), ..default() }, BackgroundColor(rgb(card_bg())))).id();
    let pill_t = commands.spawn((Text::new(a.category.clone()), ui_font(&fonts.ui, 9.0), TextColor(rgb(value_text())))).id();
    commands.entity(pill).add_child(pill_t);
    let dir = commands.spawn((Text::new(format!("{}/", install::install_dir_for_category(&a.category))), ui_font(&fonts.ui, 9.0), TextColor(rgb(placeholder())))).id();
    commands.entity(meta).add_children(&[pill, dir]);
    commands.entity(info).add_children(&[name, meta]);

    // Install button.
    let install_btn = commands
        .spawn((
            Node { width: Val::Px(72.0), height: Val::Px(24.0), flex_shrink: 0.0, align_items: AlignItems::Center, justify_content: JustifyContent::Center, column_gap: Val::Px(4.0), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() },
            BackgroundColor(if installing { rgb(text_muted()) } else { rgb(GREEN) }),
            Interaction::default(),
            LibInstallBtn(a.id.clone()),
            Name::new("lib-install"),
        ))
        .id();
    let bi = icon_text(commands, &fonts.phosphor, if installing { "spinner" } else { "download-simple" }, (255, 255, 255), 11.0);
    let bt = commands.spawn((Text::new(if installing { "..." } else { "Install" }), ui_font(&fonts.ui, 10.0), TextColor(rgb((255, 255, 255))))).id();
    commands.entity(install_btn).add_children(&[bi, bt]);

    commands.entity(row).add_children(&[thumb, info, install_btn]);
    row
}

fn centered(commands: &mut Commands, fonts: &EmberFonts, icon: &str, title: &str, sub: Option<&str>) -> Entity {
    let col = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, padding: UiRect::top(Val::Px(50.0)), row_gap: Val::Px(8.0), ..default() })
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 34.0);
    let t = commands.spawn((Text::new(title.to_string()), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_muted())))).id();
    let mut kids = vec![ic, t];
    if let Some(s) = sub {
        kids.push(commands.spawn((Text::new(s.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(placeholder())))).id());
    }
    commands.entity(col).add_children(&kids);
    col
}

fn note_snapshot(text: &'static str) -> KeyedSnapshot {
    KeyedSnapshot {
        items: vec![(u64::MAX, 0)],
        build: Box::new(move |c, f, _| {
            c.spawn((
                Text::new(text),
                ui_font(&f.ui, 11.0),
                TextColor(rgb(text_muted())),
                Node { margin: UiRect::all(Val::Px(12.0)), ..default() },
            ))
            .id()
        }),
    }
}

// ── Systems ──────────────────────────────────────────────────────────────────

fn library_filter_sync(input: Query<&EmberTextInput, With<LibFilter>>, mut data: ResMut<HubLibraryData>) {
    for inp in &input {
        if data.filter != inp.value {
            data.filter = inp.value.clone();
            data.page = 0; // a new filter resets to the first page
        }
    }
}

fn library_prev_click(q: Query<&Interaction, (With<LibPrevBtn>, Changed<Interaction>)>, mut data: ResMut<HubLibraryData>) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        data.page = data.current_page().saturating_sub(1);
    }
}

fn library_next_click(q: Query<&Interaction, (With<LibNextBtn>, Changed<Interaction>)>, mut data: ResMut<HubLibraryData>) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        let max = data.total_pages().saturating_sub(1);
        data.page = (data.current_page() + 1).min(max);
    }
}

fn poll_library(mut data: ResMut<HubLibraryData>) {
    let Some(rx) = data.rx.as_ref() else { return };
    let mut got = Vec::new();
    while let Ok(res) = rx.try_recv() {
        got.push(res);
    }
    for res in got {
        match res {
            LibraryResult::Assets(Ok(assets)) => {
                data.assets = assets;
                data.loading = false;
            }
            LibraryResult::Assets(Err(e)) => {
                data.error = Some(e);
                data.loading = false;
            }
            LibraryResult::Install(Ok(msg)) => {
                data.status = Some(msg);
                data.installing_id = None;
            }
            LibraryResult::Install(Err(e)) => {
                data.error = Some(e);
                data.installing_id = None;
            }
        }
    }
}

fn library_refresh(mut data: ResMut<HubLibraryData>, session: Option<Res<AuthSession>>) {
    if !data.needs_refresh {
        return;
    }
    let Some(session) = session else { return };
    if !session.is_signed_in() {
        return;
    }
    data.needs_refresh = false;
    fetch_my_assets(&mut data, &session);
}

fn library_refresh_click(q: Query<&Interaction, (With<LibRefreshBtn>, Changed<Interaction>)>, mut data: ResMut<HubLibraryData>) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        data.needs_refresh = true;
    }
}

fn request_lib_thumbs(data: Res<HubLibraryData>, mut thumbs: ResMut<HubThumbs>) {
    for a in &data.assets {
        if let Some(url) = &a.thumbnail_url {
            thumbs.request(url);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn fetch_my_assets(data: &mut HubLibraryData, session: &AuthSession) {
    let session = clone_session(session);
    let (tx, rx) = unbounded();
    data.rx = Some(rx);
    data.loading = true;
    std::thread::spawn(move || {
        let result = renzora_auth::marketplace::get_my_assets(&session).map(|r| r.assets);
        let _ = tx.send(LibraryResult::Assets(result));
    });
}

#[cfg(target_arch = "wasm32")]
fn fetch_my_assets(_data: &mut HubLibraryData, _session: &AuthSession) {}

fn library_install_click(
    q: Query<(&Interaction, &LibInstallBtn), Changed<Interaction>>,
    mut data: ResMut<HubLibraryData>,
    session: Option<Res<AuthSession>>,
    project: Option<Res<renzora::core::CurrentProject>>,
) {
    let (Some(session), Some(project)) = (session, project) else { return };
    if data.installing_id.is_some() {
        return;
    }
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(asset) = data.assets.iter().find(|a| a.id == btn.0).cloned() else { continue };
        install_asset(&mut data, &session, &asset, project.path.clone());
        break;
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn install_asset(data: &mut HubLibraryData, session: &AuthSession, asset: &AssetSummary, project_path: std::path::PathBuf) {
    let session = clone_session(session);
    let asset_id = asset.id.clone();
    let asset_name = asset.name.clone();
    let category = asset.category.clone();
    data.installing_id = Some(asset_id.clone());
    let (tx, rx) = unbounded();
    data.rx = Some(rx);
    std::thread::spawn(move || {
        let result = (|| {
            let dl = renzora_auth::marketplace::download_asset(&session, &asset_id)?;
            let bytes = renzora_auth::marketplace::download_file(&dl.download_url)?;
            install::install_asset_with_filename(&project_path, &category, &asset_name, &dl.download_url, &dl.download_filename, &bytes)?;
            Ok(format!("Installed \"{asset_name}\""))
        })();
        let _ = tx.send(LibraryResult::Install(result));
    });
}

#[cfg(target_arch = "wasm32")]
fn install_asset(_d: &mut HubLibraryData, _s: &AuthSession, _a: &AssetSummary, _p: std::path::PathBuf) {}

fn clone_session(s: &AuthSession) -> AuthSession {
    AuthSession {
        user: s.user.clone(),
        access_token: s.access_token.clone(),
        refresh_token: s.refresh_token.clone(),
    }
}
