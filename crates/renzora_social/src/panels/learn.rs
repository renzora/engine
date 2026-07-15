//! Docs panel — the renzora.com docs portal, right where you build: version
//! dropdown, searchable sidebar tree, and markdown pages. All public API, so
//! it works signed out.

use std::collections::HashMap;

use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver, Sender};
use renzora::core::{RenzoraShellExt, SocialPanelRequest};
use renzora::SplashState;
use renzora_auth::docs::{DocPage, DocSearchResult, DocVersions, Sidebar};
use renzora_ember::dock::panel_active;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{keyed_list_tokened, Bound, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{dropdown, markdown_view, text_input, EmberTextInput, HoverTint};

use crate::toasts::{ToastQueue, Tone};
use crate::util::{self, hash64, HUE_LEARN};
use crate::PendingSocialRequest;

pub(crate) const PANEL_ID: &str = "social_learn";

const SEARCH_DEBOUNCE_SECS: f64 = 0.4;

pub(crate) enum LearnResult {
    Versions(Result<DocVersions, String>),
    Sidebar(Result<Sidebar, String>),
    Page(Result<DocPage, String>),
    Search(Result<Vec<DocSearchResult>, String>),
}

#[derive(Resource)]
pub(crate) struct LearnPanel {
    pub versions: DocVersions,
    pub doc_version: Option<String>,
    pub sidebar: Option<Sidebar>,
    pub page: Option<DocPage>,
    /// Slug of the page the sidebar should highlight as current. Set the instant
    /// a page is clicked (before its content loads) so selection feels immediate.
    pub selected: Option<String>,
    pub page_cache: HashMap<(String, String), DocPage>,
    pub results: Vec<DocSearchResult>,
    pub last_query: String,
    pub pending_query: Option<(String, f64)>,
    pub loading: bool,
    pub error: Option<String>,
    pub version: u64,
    pub loaded_once: bool,
    pub tx: Sender<LearnResult>,
    rx: Receiver<LearnResult>,
}

impl Default for LearnPanel {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Self {
            versions: DocVersions { default: String::new(), versions: Vec::new() },
            doc_version: None,
            sidebar: None,
            page: None,
            selected: None,
            page_cache: HashMap::new(),
            results: Vec::new(),
            last_query: String::new(),
            pending_query: None,
            loading: false,
            error: None,
            version: 0,
            loaded_once: false,
            tx,
            rx,
        }
    }
}

impl LearnPanel {
    pub fn bump(&mut self) {
        self.version = self.version.wrapping_add(1);
    }

    fn open_page(&mut self, slug: String) {
        let Some(version) = self.doc_version.clone() else { return };
        // Remember the target immediately so the sidebar highlights the clicked
        // page this frame — even on a cache miss, before the fetch returns.
        self.selected = Some(slug.clone());
        if let Some(page) = self.page_cache.get(&(version.clone(), slug.clone())) {
            self.page = Some(page.clone());
            self.bump();
            return;
        }
        // Cache miss: the page arrives async. Bump NOW so the click repaints
        // (loading state + the newly-selected sidebar row) instead of the view
        // sitting stale until `poll_results` lands the page — that stall was the
        // "clicking doesn't switch until I change theme" bug: the keyed lists are
        // tokened on `version`, so a click that changes state without bumping is
        // invisible until the next unrelated bump (or a theme rebuild).
        self.loading = true;
        self.bump();
        let tx = self.tx.clone();
        spawn_thread(move || {
            let _ = tx.send(LearnResult::Page(renzora_auth::docs::get_page(&version, &slug)));
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_thread(f: impl FnOnce() + Send + 'static) {
    std::thread::spawn(f);
}
#[cfg(target_arch = "wasm32")]
fn spawn_thread(_f: impl FnOnce() + Send + 'static) {}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<LearnPanel>();
    app.register_shell_panel(PANEL_ID, "Docs", "book-open", "Community");
    app.register_panel_content(PANEL_ID, false, build);
    app.add_systems(
        Update,
        (
            poll_results,
            auto_load.run_if(panel_active(PANEL_ID)),
            search_debounce.run_if(panel_active(PANEL_ID)),
            clicks,
            consume_request,
        )
            .run_if(in_state(SplashState::Editor)),
    );
}

// ── Systems ──────────────────────────────────────────────────────────────────

fn poll_results(mut panel: ResMut<LearnPanel>, mut toasts: ResMut<ToastQueue>) {
    let mut got = Vec::new();
    while let Ok(r) = panel.rx.try_recv() {
        got.push(r);
    }
    for r in got {
        match r {
            LearnResult::Versions(Ok(v)) => {
                let default = if v.default.is_empty() {
                    v.versions.first().map(|x| x.id.clone()).unwrap_or_default()
                } else {
                    v.default.clone()
                };
                panel.versions = v;
                if panel.doc_version.is_none() && !default.is_empty() {
                    panel.doc_version = Some(default.clone());
                    let tx = panel.tx.clone();
                    spawn_thread(move || {
                        let _ = tx.send(LearnResult::Sidebar(renzora_auth::docs::get_sidebar(&default)));
                    });
                }
                panel.bump();
            }
            LearnResult::Sidebar(Ok(sb)) => {
                // Open the first page by default.
                let first = sb
                    .groups
                    .first()
                    .and_then(|g| g.categories.first())
                    .and_then(|c| c.pages.first())
                    .map(|p| p.slug.clone());
                panel.sidebar = Some(sb);
                panel.loading = false;
                if panel.page.is_none() {
                    if let Some(slug) = first {
                        panel.open_page(slug);
                    }
                }
                panel.bump();
            }
            LearnResult::Page(Ok(page)) => {
                panel.page_cache.insert((page.version.clone(), page.slug.clone()), page.clone());
                panel.selected = Some(page.slug.clone());
                panel.page = Some(page);
                panel.loading = false;
                panel.bump();
            }
            LearnResult::Search(Ok(results)) => {
                panel.results = results;
                panel.bump();
            }
            LearnResult::Versions(Err(e)) | LearnResult::Sidebar(Err(e)) | LearnResult::Page(Err(e)) => {
                panel.loading = false;
                panel.error = Some(e.clone());
                toasts.push(Tone::Error, e, None);
                panel.bump();
            }
            LearnResult::Search(Err(_)) => {}
        }
    }
}

/// One-shot load of the version list (sidebar/page fetches chain from the
/// result). `loaded_once` is set at SPAWN time so this can never loop.
fn auto_load(mut panel: ResMut<LearnPanel>) {
    if !panel.loaded_once {
        panel.loaded_once = true;
        panel.loading = true;
        let tx = panel.tx.clone();
        spawn_thread(move || {
            let _ = tx.send(LearnResult::Versions(renzora_auth::docs::get_versions()));
        });
    }
}

fn search_debounce(
    mut panel: ResMut<LearnPanel>,
    time: Res<Time>,
    inputs: Query<&EmberTextInput, With<DocSearchInput>>,
) {
    let Ok(input) = inputs.single() else { return };
    let query = input.value.trim().to_string();
    let now = time.elapsed_secs_f64();

    if query != panel.last_query {
        panel.last_query = query.clone();
        if query.len() >= 2 {
            panel.pending_query = Some((query, now + SEARCH_DEBOUNCE_SECS));
        } else {
            panel.pending_query = None;
            if !panel.results.is_empty() {
                panel.results.clear();
                panel.bump();
            }
        }
        return;
    }
    if let Some((q, deadline)) = panel.pending_query.clone() {
        if now >= deadline {
            panel.pending_query = None;
            let Some(version) = panel.doc_version.clone() else { return };
            let tx = panel.tx.clone();
            spawn_thread(move || {
                let _ = tx.send(LearnResult::Search(renzora_auth::docs::search(&version, &q)));
            });
        }
    }
}

fn consume_request(mut pending: ResMut<PendingSocialRequest>) {
    if matches!(pending.0, Some(SocialPanelRequest::Learn)) {
        pending.0 = None;
    }
}

// ── Clicks ───────────────────────────────────────────────────────────────────

#[derive(Component)]
struct DocPageBtn(String);
#[derive(Component)]
struct DocVersionDropdown(Vec<String>);
#[derive(Component)]
struct DocSearchInput;

fn clicks(
    mut panel: ResMut<LearnPanel>,
    pages: Query<(&Interaction, &DocPageBtn), Changed<Interaction>>,
    version_dropdowns: Query<(&Bound<usize>, &DocVersionDropdown), Changed<Bound<usize>>>,
) {
    for (i, b) in &pages {
        if *i == Interaction::Pressed {
            panel.results.clear();
            panel.open_page(b.0.clone());
        }
    }
    for (b, dd) in &version_dropdowns {
        let Some(id) = dd.0.get(b.0) else { continue };
        if panel.doc_version.as_deref() != Some(id.as_str()) {
            panel.doc_version = Some(id.clone());
            panel.sidebar = None;
            panel.page = None;
            panel.selected = None;
            panel.results.clear();
            panel.bump();
            let version = id.clone();
            let tx = panel.tx.clone();
            spawn_thread(move || {
                let _ = tx.send(LearnResult::Sidebar(renzora_auth::docs::get_sidebar(&version)));
            });
        }
    }
}

// ── Build ────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // Two columns straight away — no header chrome, the docs ARE the panel.
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(8.0),
            padding: UiRect::all(Val::Px(8.0)),
            min_height: Val::Px(0.0),
            ..default()
        })
        .id();

    // Sidebar column.
    let side_col = commands
        .spawn(Node {
            width: Val::Px(230.0),
            height: Val::Percent(100.0),
            min_height: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            flex_shrink: 0.0,
            ..default()
        })
        .id();
    let search = text_input(commands, &fonts.ui, "Search docs...", "");
    commands.entity(search).insert(DocSearchInput);
    let side_list = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(1.0), ..default() })
        .id();
    keyed_list_tokened(
        commands,
        side_list,
        |w| w.get_resource::<LearnPanel>().map(|p| p.version).unwrap_or(0),
        sidebar_snapshot,
    );
    let side_scroll = renzora_ember::widgets::scroll_view(commands, side_list);
    commands.entity(side_scroll).insert(Node {
        width: Val::Percent(100.0),
        flex_grow: 1.0,
        min_height: Val::Px(0.0),
        overflow: Overflow::scroll_y(),
        ..default()
    });
    commands.entity(side_col).add_children(&[search, side_scroll]);

    // Page column.
    let page_col = commands
        .spawn(Node {
            flex_grow: 1.0,
            height: Val::Percent(100.0),
            min_height: Val::Px(0.0),
            min_width: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .id();
    let page_inner = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), padding: UiRect::all(Val::Px(4.0)), ..default() })
        .id();
    keyed_list_tokened(
        commands,
        page_inner,
        |w| w.get_resource::<LearnPanel>().map(|p| p.version).unwrap_or(0),
        page_snapshot,
    );
    let page_scroll = renzora_ember::widgets::scroll_view(commands, page_inner);
    commands.entity(page_scroll).insert(Node {
        width: Val::Percent(100.0),
        flex_grow: 1.0,
        min_height: Val::Px(0.0),
        overflow: Overflow::scroll_y(),
        ..default()
    });
    commands.entity(page_col).add_child(page_scroll);

    commands.entity(root).add_children(&[side_col, page_col]);
    root
}

// ── Snapshots ────────────────────────────────────────────────────────────────

fn sidebar_snapshot(w: &World) -> KeyedSnapshot {
    let Some(panel) = w.get_resource::<LearnPanel>() else {
        return util::empty_snapshot();
    };

    // Search results replace the tree while a query is live.
    if !panel.results.is_empty() {
        let results = panel.results.clone();
        let keys = results.iter().map(|r| (hash64(&r.slug), hash64(&r.title))).collect();
        return KeyedSnapshot {
            items: keys,
            build: Box::new(move |commands, fonts, i| {
                let r = &results[i];
                let row = commands
                    .spawn((
                        Node { width: Val::Percent(100.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)), flex_direction: FlexDirection::Column, ..default() },
                        Interaction::default(),
                        DocPageBtn(r.slug.clone()),
                    ))
                    .id();
                let t = commands
                    .spawn((Text::new(r.title.clone()), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_primary()))))
                    .id();
                let c = commands
                    .spawn((Text::new(format!("{} › {}", r.group, r.category)), ui_font(&fonts.ui, 8.5), TextColor(rgb(placeholder()))))
                    .id();
                commands.entity(row).add_children(&[t, c]);
                row
            }),
        };
    }

    let Some(sidebar) = panel.sidebar.clone() else {
        return note("Loading docs...");
    };
    // Highlight the clicked page immediately: `selected` is set on click, ahead
    // of `page` (which only updates once the fetch lands).
    let current = panel.selected.clone().or_else(|| panel.page.as_ref().map(|p| p.slug.clone()));
    let versions = panel.versions.versions.clone();
    let active_version = panel.doc_version.clone().unwrap_or_default();

    // Flatten: version switcher + group/category headers + page rows.
    enum Row {
        VersionDropdown(Vec<(String, String)>, usize),
        Group(String),
        Category(String),
        Page(String, String, bool, usize),
    }
    let mut rows = Vec::new();
    if versions.len() > 1 {
        let opts: Vec<(String, String)> = versions
            .iter()
            .map(|v| (v.id.clone(), if v.label.is_empty() { v.id.clone() } else { v.label.clone() }))
            .collect();
        let selected = opts.iter().position(|(id, _)| *id == active_version).unwrap_or(0);
        rows.push(Row::VersionDropdown(opts, selected));
    }
    for g in &sidebar.groups {
        rows.push(Row::Group(g.group.clone()));
        for c in &g.categories {
            rows.push(Row::Category(c.category.clone()));
            for (pi, p) in c.pages.iter().enumerate() {
                rows.push(Row::Page(p.slug.clone(), p.title.clone(), current.as_deref() == Some(p.slug.as_str()), pi));
            }
        }
    }

    let keys = rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let h = match r {
                Row::VersionDropdown(opts, sel) => hash64(&("v", opts.len(), *sel)),
                Row::Group(g) => hash64(&("g", g)),
                Row::Category(c) => hash64(&("c", c)),
                Row::Page(slug, _, active, pi) => hash64(&("p", slug, *active, *pi)),
            };
            (i as u64 + 10, h)
        })
        .collect();
    KeyedSnapshot {
        items: keys,
        build: Box::new(move |commands, fonts, i| match &rows[i] {
            Row::VersionDropdown(opts, selected) => {
                let labels: Vec<&str> = opts.iter().map(|(_, l)| l.as_str()).collect();
                let dd = dropdown(commands, fonts, &labels, *selected);
                commands
                    .entity(dd)
                    .insert(DocVersionDropdown(opts.iter().map(|(id, _)| id.clone()).collect()));
                dd
            }
            Row::Group(g) => {
                // Group header: accent bar + label, section-style.
                let row = commands
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(6.0),
                            padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                            margin: UiRect::top(Val::Px(8.0)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(renzora_ember::widgets::tint(HUE_LEARN, 26)),
                    ))
                    .id();
                let bar = commands
                    .spawn((
                        Node { width: Val::Px(3.0), height: Val::Px(12.0), border_radius: BorderRadius::all(Val::Px(2.0)), ..default() },
                        BackgroundColor(renzora_ember::widgets::tint(HUE_LEARN, 255)),
                    ))
                    .id();
                let t = commands
                    .spawn((Text::new(g.to_uppercase()), ui_font(&fonts.ui, 9.0), TextColor(rgb(text_primary()))))
                    .id();
                commands.entity(row).add_children(&[bar, t]);
                row
            }
            Row::Category(c) => commands
                .spawn((Text::new(c.clone()), ui_font(&fonts.ui, 9.5), TextColor(rgb(text_muted())), Node { margin: UiRect::new(Val::Px(6.0), Val::Px(0.0), Val::Px(5.0), Val::Px(1.0)), ..default() }))
                .id(),
            Row::Page(slug, title, active, pi) => {
                let stripe = if pi % 2 == 0 { row_even() } else { row_odd() };
                let row = commands
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::axes(Val::Px(8.0), Val::Px(3.5)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(if *active {
                            renzora_ember::widgets::tint(HUE_LEARN, 50)
                        } else {
                            rgb(stripe)
                        }),
                        Interaction::default(),
                        HoverTint::solid(
                            if *active { renzora_ember::widgets::tint(HUE_LEARN, 50) } else { rgb(stripe) },
                            rgb(hover_bg()),
                            renzora_ember::widgets::tint(HUE_LEARN, 60),
                        ),
                        DocPageBtn(slug.clone()),
                    ))
                    .id();
                let t = commands
                    .spawn((
                        Text::new(title.clone()),
                        ui_font(&fonts.ui, 10.5),
                        TextColor(if *active {
                            renzora_ember::widgets::tint(HUE_LEARN, 255)
                        } else {
                            rgb(text_primary())
                        }),
                    ))
                    .id();
                commands.entity(row).add_child(t);
                row
            }
        }),
    }
}

fn page_snapshot(w: &World) -> KeyedSnapshot {
    let Some(panel) = w.get_resource::<LearnPanel>() else {
        return util::empty_snapshot();
    };
    let Some(page) = panel.page.clone() else {
        return note("Select a page");
    };
    KeyedSnapshot {
        items: vec![(hash64(&(&page.version, &page.slug)), hash64(&page.content))],
        build: Box::new(move |commands, fonts, _| {
            let wrap = commands
                .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() })
                .id();
            let crumb = commands
                .spawn((
                    Text::new(format!("{} › {}", page.group, page.category)),
                    ui_font(&fonts.ui, 9.0),
                    TextColor(rgb(placeholder())),
                ))
                .id();
            let md = markdown_view(commands, fonts, &page.content);
            commands.entity(wrap).add_children(&[crumb, md]);
            wrap
        }),
    }
}

fn note(msg: &'static str) -> KeyedSnapshot {
    KeyedSnapshot {
        items: vec![(u64::MAX, hash64(msg))],
        build: Box::new(move |commands, fonts, _| {
            commands
                .spawn((Text::new(msg), ui_font(&fonts.ui, 11.0), TextColor(rgb(placeholder())), Node { margin: UiRect::top(Val::Px(8.0)), ..default() }))
                .id()
        }),
    }
}
