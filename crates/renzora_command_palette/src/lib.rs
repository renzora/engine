#![allow(dead_code)] // Public surface area kept for upcoming features.

//! Command Palette — fuzzy-searchable modal listing every registered tool
//! and shortcut. Press `Ctrl+P` to open, type to filter, arrow keys to
//! navigate, Enter to execute, Escape to dismiss.
//!
//! This plugin reads its entries from the SDK's `ToolbarRegistry` and
//! `ShortcutRegistry`, so every plugin that registers tools or shortcuts
//! automatically appears here with zero extra wiring.

use std::sync::Arc;

use bevy::prelude::*;
use renzora::core::keybindings::{EditorAction, KeyBindings};
use renzora_editor_framework::{
    AppEditorExt, ShortcutEntry, ShortcutRegistry, SplashState, ToolEntry, ToolbarRegistry,
};

mod native;

// ── State ──────────────────────────────────────────────────────────────────

/// Which corpus the palette searches. `Commands` is the local everything-list
/// (tools, actions, panels, layouts, settings, menu commands); the rest are
/// scoped tabs — `Entities`/`Settings` search locally, the others query
/// renzora.com through [`PaletteRemote`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum PaletteTab {
    #[default]
    Commands,
    Entities,
    Settings,
    Docs,
    Forum,
    Users,
    Feed,
    Courses,
    Marketplace,
}

impl PaletteTab {
    pub const ALL: &'static [PaletteTab] = &[
        PaletteTab::Commands,
        PaletteTab::Entities,
        PaletteTab::Settings,
        PaletteTab::Docs,
        PaletteTab::Forum,
        PaletteTab::Users,
        PaletteTab::Feed,
        PaletteTab::Courses,
        PaletteTab::Marketplace,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            PaletteTab::Commands => "Commands",
            PaletteTab::Entities => "Entities",
            PaletteTab::Settings => "Settings",
            PaletteTab::Docs => "Docs",
            PaletteTab::Forum => "Forum",
            PaletteTab::Users => "Users",
            PaletteTab::Feed => "Feed",
            PaletteTab::Courses => "Courses",
            PaletteTab::Marketplace => "Marketplace",
        }
    }

    /// Tabs whose results come from the renzora.com API.
    pub(crate) fn is_remote(&self) -> bool {
        matches!(
            self,
            PaletteTab::Docs
                | PaletteTab::Forum
                | PaletteTab::Users
                | PaletteTab::Feed
                | PaletteTab::Courses
                | PaletteTab::Marketplace
        )
    }
}

#[derive(Resource, Default)]
pub struct CommandPaletteState {
    pub open: bool,
    pub query: String,
    pub selected: usize,
    /// The active search scope tab.
    pub tab: PaletteTab,
    /// True on the first render after opening — lets us force keyboard focus
    /// on the text input the frame the palette appears.
    pub just_opened: bool,
}

// ── Plugin ─────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct CommandPalettePlugin;

impl Plugin for CommandPalettePlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] CommandPalettePlugin");
        app.init_resource::<CommandPaletteState>()
            .register_shortcut(ShortcutEntry::new(
                "command_palette.toggle",
                "Command Palette",
                "General",
                renzora::core::keybindings::KeyBinding::new(KeyCode::KeyP).ctrl(),
                toggle_palette,
            ))
            .add_systems(
                Update,
                consume_toggle_request.run_if(in_state(SplashState::Editor)),
            );
        // Native (bevy_ui) palette.
        native::register(app);
    }
}

/// Watches for `ToggleCommandPaletteRequested` and toggles the palette.
/// Lets external surfaces (e.g. the title-bar search button) open the
/// palette without depending on this crate.
fn consume_toggle_request(world: &mut World) {
    if world
        .remove_resource::<renzora::core::ToggleCommandPaletteRequested>()
        .is_some()
    {
        toggle_palette(world);
    }
}

fn toggle_palette(world: &mut World) {
    let mut state = world.resource_mut::<CommandPaletteState>();
    state.open = !state.open;
    if state.open {
        state.query.clear();
        state.selected = 0;
        state.tab = PaletteTab::Commands;
        state.just_opened = true;
    }
}

// ── Items ──────────────────────────────────────────────────────────────────

/// One row in the palette. Handlers are cloned `Arc`s so we can invoke them
/// after the UI frees its borrows.
#[derive(Clone)]
struct PaletteItem {
    kind: &'static str,
    label: String,
    /// Optional right-aligned secondary text (e.g. current keybinding).
    detail: Option<String>,
    handler: Arc<dyn Fn(&mut World) + Send + Sync>,
}

fn collect_items(
    toolbar: &ToolbarRegistry,
    shortcuts: &ShortcutRegistry,
    keybindings: &KeyBindings,
    world: &World,
) -> Vec<PaletteItem> {
    let mut out: Vec<PaletteItem> = Vec::new();

    // Tools — skip ones whose `visible` predicate is currently false so the
    // palette reflects context (e.g. "Paint Foliage" only when a terrain is
    // selected, "Join Selected" only when ≥2 meshes are selected).
    for entry in toolbar.entries() {
        if !(entry.visible)(world) {
            continue;
        }
        out.push(tool_item(entry));
    }

    // Plugin shortcuts — always visible, include the current keybinding.
    for entry in shortcuts.entries() {
        let binding = keybindings
            .get_plugin(entry.id)
            .map(|b| b.display())
            .unwrap_or_else(|| "Unbound".to_string());
        let handler = entry.handler.clone();
        out.push(PaletteItem {
            kind: "Action",
            label: entry.display_name.to_string(),
            detail: Some(binding),
            handler: Arc::new(move |w| (handler)(w)),
        });
    }

    // Built-in editor actions — every entry in the EditorAction enum.
    // Invoke by dispatching through KeyBindings so the existing consumer
    // systems (viewport, gizmo, scene, camera) fire exactly as they would
    // for a real key press. Skip hold-based camera movement actions —
    // they're not sensible as one-shot palette commands.
    for action in EditorAction::all() {
        if is_hold_action(action) {
            continue;
        }
        let binding = keybindings
            .get(action)
            .map(|b| b.display())
            .unwrap_or_else(|| "Unbound".to_string());
        out.push(PaletteItem {
            kind: action.category(),
            label: action.display_name().to_string(),
            detail: Some(binding),
            handler: Arc::new(move |w: &mut World| {
                if let Some(kb) = w.get_resource::<KeyBindings>() {
                    kb.dispatch(action);
                }
            }),
        });
    }

    // Layouts — every visible workspace layout becomes a "Switch to X" entry.
    if let Some(manager) = world.get_resource::<renzora_editor_framework::LayoutManager>() {
        let layouts: Vec<(usize, String)> = manager
            .visible_layouts()
            .map(|(i, l)| (i, l.name.clone()))
            .collect();
        for (idx, name) in layouts {
            let label = format!("Switch to {}", name);
            out.push(PaletteItem {
                kind: "Layout",
                label,
                detail: None,
                handler: Arc::new(move |w: &mut World| {
                    w.resource_scope::<renzora_editor_framework::LayoutManager, _>(|w, mut mgr| {
                        if let Some(mut docking) =
                            w.get_resource_mut::<renzora_editor_framework::DockingState>()
                        {
                            mgr.switch(idx, &mut docking);
                        }
                    });
                }),
            });
        }
    }

    // Panels — every registered panel can be opened via "Open <Panel>".
    // Focuses the panel if already in the dock; otherwise adds it to the
    // first leaf in traversal order.
    if let Some(registry) = world.get_resource::<renzora_editor_framework::PanelRegistry>() {
        let panels: Vec<(String, String)> = registry
            .iter()
            .map(|p| (p.id().to_string(), p.title().to_string()))
            .collect();
        for (id, title) in panels {
            let label = format!("Open {}", title);
            out.push(PaletteItem {
                kind: "Panel",
                label,
                detail: None,
                handler: Arc::new(move |w: &mut World| {
                    if let Some(mut docking) = w.get_resource_mut::<renzora_editor_framework::DockingState>()
                    {
                        docking.tree.focus_or_add_panel(&id);
                    }
                }),
            });
        }
    }

    // Settings tabs — open the settings overlay on a specific tab.
    use renzora_editor_framework::SettingsTab;
    let settings_tabs: &[(SettingsTab, &str)] = &[
        (SettingsTab::Project, "Project"),
        (SettingsTab::Interface, "Interface"),
        (SettingsTab::Editor, "Editor"),
        (SettingsTab::Viewport, "Viewport"),
        (SettingsTab::Scripting, "Scripting"),
        (SettingsTab::Assets, "Assets"),
        (SettingsTab::Input, "Input"),
        (SettingsTab::Shortcuts, "Shortcuts"),
        (SettingsTab::Theme, "Theme"),
        (SettingsTab::Plugins, "Plugins"),
    ];
    for (tab, name) in settings_tabs {
        let tab = *tab;
        let label = format!("Settings: {}", name);
        out.push(PaletteItem {
            kind: "Settings",
            label,
            detail: None,
            handler: Arc::new(move |w: &mut World| {
                if let Some(mut s) = w.get_resource_mut::<renzora_editor_framework::EditorSettings>() {
                    s.show_settings = true;
                    s.settings_tab = tab;
                }
            }),
        });
    }

    // File-menu commands — mirror the title bar's File menu so users can
    // dispatch them from the palette without picking from the menu.
    // (New Project / Open Project route through editor-private file dialogs
    //  and aren't exposed as marker resources, so we omit them here.)
    let menu_items: &[(&str, fn(&mut World))] = &[
        ("File: New Scene", |w| {
            w.insert_resource(renzora::core::NewSceneRequested);
        }),
        ("File: Open Scene...", |w| {
            w.insert_resource(renzora::core::OpenSceneRequested);
        }),
        ("File: Save", |w| {
            w.insert_resource(renzora::core::SaveSceneRequested);
        }),
        ("File: Save As...", |w| {
            w.insert_resource(renzora::core::SaveAsSceneRequested);
        }),
        ("File: Export Project...", |w| {
            w.insert_resource(renzora::core::ExportRequested);
        }),
        ("File: Import...", |w| {
            w.insert_resource(renzora::core::ImportRequested);
        }),
        ("Help: Getting Started Tutorial", |w| {
            w.insert_resource(renzora::core::TutorialRequested);
        }),
    ];
    for (label, handler) in menu_items {
        let h = *handler;
        out.push(PaletteItem {
            kind: "Menu",
            label: label.to_string(),
            detail: None,
            handler: Arc::new(move |w: &mut World| h(w)),
        });
    }

    // Docs — open external documentation URLs in the user's browser.
    let docs: &[(&str, &str)] = &[
        ("Documentation: Home", "https://renzora.com/docs"),
        (
            "Documentation: YouTube Channel",
            "https://youtube.com/@renzoragame",
        ),
        ("Documentation: Discord", "https://discord.gg/9UHUGUyDJv"),
        ("Documentation: GitHub", "https://github.com/renzora/engine"),
    ];
    for (label, url) in docs {
        let url = url.to_string();
        out.push(PaletteItem {
            kind: "Docs",
            label: label.to_string(),
            detail: Some("Opens browser".to_string()),
            handler: Arc::new(move |_w: &mut World| {
                open_url(&url);
            }),
        });
    }

    out
}

fn open_url(url: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn();
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(url).spawn();
        #[cfg(all(unix, not(target_os = "macos")))]
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

/// Actions that fire continuously while a key is held (camera WASD) aren't
/// useful in a palette — they'd fire once per invocation and feel broken.
fn is_hold_action(action: EditorAction) -> bool {
    matches!(
        action,
        EditorAction::CameraMoveForward
            | EditorAction::CameraMoveBackward
            | EditorAction::CameraMoveLeft
            | EditorAction::CameraMoveRight
            | EditorAction::CameraMoveUp
            | EditorAction::CameraMoveDown
            | EditorAction::CameraMoveFaster
    )
}

fn tool_item(entry: &ToolEntry) -> PaletteItem {
    let activate = entry.activate.clone();
    PaletteItem {
        kind: "Tool",
        label: entry.tooltip.to_string(),
        detail: None,
        handler: Arc::new(move |w| (activate)(w)),
    }
}

fn filter_items(items: Vec<PaletteItem>, query: &str) -> Vec<PaletteItem> {
    if query.is_empty() {
        return items;
    }
    let q = query.to_lowercase();
    items
        .into_iter()
        .filter(|i| i.label.to_lowercase().contains(&q) || i.kind.to_lowercase().contains(&q))
        .collect()
}

// ── Entities tab (local) ─────────────────────────────────────────────────────

/// Named scene entities matching `query`; picking one selects it. UI nodes and
/// cameras are excluded — this searches the *scene*, not the editor chrome.
fn collect_entity_items(world: &mut World, query: &str) -> Vec<PaletteItem> {
    const MAX: usize = 120;
    type SceneEntityFilter = (With<Transform>, Without<Node>, Without<Camera>);
    let q = query.to_lowercase();
    let mut qy = world.query_filtered::<(Entity, &Name), SceneEntityFilter>();
    let mut out = Vec::new();
    for (e, name) in qy.iter(world) {
        if !q.is_empty() && !name.as_str().to_lowercase().contains(&q) {
            continue;
        }
        out.push(PaletteItem {
            kind: "Entity",
            label: name.as_str().to_string(),
            detail: None,
            handler: Arc::new(move |w: &mut World| {
                if let Some(sel) = w.get_resource::<renzora_editor_framework::EditorSelection>() {
                    sel.set(Some(e));
                }
            }),
        });
        if out.len() >= MAX {
            break;
        }
    }
    out.sort_by_key(|i| i.label.to_lowercase());
    out
}

// ── Remote tabs (renzora.com) ────────────────────────────────────────────────

/// What activating a remote search result does.
#[derive(Clone)]
enum RemoteAction {
    /// Open a URL in the system browser (docs pages).
    Url(String),
    /// Deep-link into a social panel (forum thread, profile, feed post…).
    Social(renzora::core::SocialPanelRequest),
    /// Focus/open a dock panel by id (marketplace → the store).
    Panel(&'static str),
}

/// One remote search result, carried from the worker thread to the UI.
struct RemoteHit {
    kind: &'static str,
    label: String,
    detail: Option<String>,
    action: RemoteAction,
}

impl RemoteHit {
    /// A palette row for this hit (cheap: clones the strings + action).
    fn item(&self) -> PaletteItem {
        let action = self.action.clone();
        PaletteItem {
            kind: self.kind,
            label: self.label.clone(),
            detail: self.detail.clone(),
            handler: Arc::new(move |w: &mut World| match &action {
                RemoteAction::Url(url) => open_url(url),
                RemoteAction::Social(req) => {
                    if let Some(mut bridge) = w.get_resource_mut::<renzora::core::SocialBridge>() {
                        bridge.open_panel_request = Some(req.clone());
                    }
                }
                RemoteAction::Panel(id) => {
                    if let Some(mut docking) =
                        w.get_resource_mut::<renzora_editor_framework::DockingState>()
                    {
                        docking.tree.focus_or_add_panel(id);
                    }
                }
            }),
        }
    }
}

/// Async state for the remote search tabs: a debounced dispatch plus a results
/// cache keyed by (tab, query), so switching back to a tab is instant and a
/// slow response for a stale query is dropped.
#[derive(Resource)]
struct PaletteRemote {
    /// The (tab, query) the cached `results` answer.
    have: Option<(PaletteTab, String)>,
    /// The most recent dispatch in flight.
    sent: Option<(PaletteTab, String)>,
    /// Debounced dispatch target: fire when `Time::elapsed` passes the f64.
    due: Option<(PaletteTab, String, f64)>,
    loading: bool,
    results: Vec<RemoteHit>,
    /// Bumped when results land so the UI re-signatures and rebuilds.
    generation: u64,
    tx: crossbeam_channel::Sender<(PaletteTab, String, Vec<RemoteHit>)>,
    rx: crossbeam_channel::Receiver<(PaletteTab, String, Vec<RemoteHit>)>,
}

impl Default for PaletteRemote {
    fn default() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            have: None,
            sent: None,
            due: None,
            loading: false,
            results: Vec::new(),
            generation: 0,
            tx,
            rx,
        }
    }
}

/// Minimum query length for the tabs that hit a server-side search endpoint.
/// The catalog tabs (feed/courses/marketplace) list their first page unfiltered.
fn min_query_len(tab: PaletteTab) -> usize {
    match tab {
        PaletteTab::Docs | PaletteTab::Forum | PaletteTab::Users => 2,
        _ => 0,
    }
}

/// Debounce + dispatch remote searches, and drain finished ones.
fn remote_search(
    time: Res<Time>,
    state: Res<CommandPaletteState>,
    session: Option<Res<renzora_auth::AuthSession>>,
    mut remote: ResMut<PaletteRemote>,
) {
    // Land finished searches (only the one we're still waiting for).
    let mut landed = Vec::new();
    while let Ok(r) = remote.rx.try_recv() {
        landed.push(r);
    }
    for (tab, q, hits) in landed {
        if remote.sent.as_ref().is_some_and(|(t, sq)| *t == tab && *sq == q) {
            remote.results = hits;
            remote.have = Some((tab, q));
            remote.sent = None;
            remote.loading = false;
            remote.generation = remote.generation.wrapping_add(1);
        }
    }

    if !state.open || !state.tab.is_remote() {
        remote.due = None;
        return;
    }
    let want = (state.tab, state.query.trim().to_string());
    if want.1.chars().count() < min_query_len(want.0) {
        // Below the threshold: show the "type to search" hint, fetch nothing.
        if remote.have.as_ref() != Some(&want) {
            remote.results.clear();
            remote.have = Some(want);
            remote.due = None;
            remote.generation = remote.generation.wrapping_add(1);
        }
        return;
    }
    if remote.have.as_ref() == Some(&want) || remote.sent.as_ref() == Some(&want) {
        return;
    }
    let now = time.elapsed_secs_f64();
    match &remote.due {
        Some((t, q, at)) if *t == want.0 && *q == want.1 => {
            if now >= *at {
                remote.due = None;
                remote.sent = Some(want.clone());
                remote.loading = true;
                let session = session.map(|s| renzora_auth::AuthSession {
                    user: s.user.clone(),
                    access_token: s.access_token.clone(),
                    refresh_token: None,
                });
                dispatch_remote(want.0, want.1, session, remote.tx.clone());
            }
        }
        _ => remote.due = Some((want.0, want.1.clone(), now + 0.3)),
    }
}

/// Run one remote search on a worker thread and send the hits back.
#[cfg(not(target_arch = "wasm32"))]
fn dispatch_remote(
    tab: PaletteTab,
    query: String,
    session: Option<renzora_auth::AuthSession>,
    tx: crossbeam_channel::Sender<(PaletteTab, String, Vec<RemoteHit>)>,
) {
    std::thread::spawn(move || {
        let q = query.to_lowercase();
        let hits: Vec<RemoteHit> = match tab {
            PaletteTab::Docs => {
                // Resolve the site's default docs version once per process.
                static VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();
                let version = VERSION.get_or_init(|| {
                    renzora_auth::docs::get_versions()
                        .ok()
                        .filter(|v| !v.default.is_empty())
                        .map(|v| v.default)
                        .unwrap_or_else(|| "r1-alpha6".to_string())
                });
                renzora_auth::docs::search(version, &query)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|d| RemoteHit {
                        kind: "Doc",
                        label: d.title,
                        detail: Some(format!("{} · {}", d.group, d.category)),
                        action: RemoteAction::Url(format!(
                            "https://renzora.com/docs/{}/{}",
                            d.version, d.slug
                        )),
                    })
                    .collect()
            }
            PaletteTab::Forum => renzora_auth::forum::search_forum(&query)
                .unwrap_or_default()
                .into_iter()
                .map(|h| RemoteHit {
                    kind: "Thread",
                    label: h.title,
                    detail: Some(format!("by {} · {} posts", h.author_name, h.post_count)),
                    action: RemoteAction::Social(renzora::core::SocialPanelRequest::Forum {
                        thread_slug: Some(h.slug),
                    }),
                })
                .collect(),
            PaletteTab::Users => renzora_auth::social::search_users(&query)
                .unwrap_or_default()
                .into_iter()
                .map(|u| RemoteHit {
                    kind: "User",
                    detail: (u.role != "user" && !u.role.is_empty()).then(|| u.role.clone()),
                    action: RemoteAction::Social(renzora::core::SocialPanelRequest::Profile {
                        username: Some(u.username.clone()),
                    }),
                    label: u.username,
                })
                .collect(),
            PaletteTab::Feed => {
                // No feed-search endpoint: pull the latest page and filter here.
                let posts = session
                    .as_ref()
                    .map(|s| renzora_auth::feed::get_feed(s, None, 30, None).unwrap_or_default())
                    .unwrap_or_default();
                posts
                    .into_iter()
                    .filter(|p| {
                        q.is_empty()
                            || p.body.to_lowercase().contains(&q)
                            || p.username.to_lowercase().contains(&q)
                    })
                    .take(30)
                    .map(|p| {
                        let mut body: String = p.body.chars().take(64).collect();
                        if body.len() < p.body.len() {
                            body.push('…');
                        }
                        RemoteHit {
                            kind: "Post",
                            label: format!("{}: {}", p.username, body.replace('\n', " ")),
                            detail: Some(format!("{} comments", p.comment_count)),
                            action: RemoteAction::Social(renzora::core::SocialPanelRequest::Feed {
                                post_id: Some(p.id),
                            }),
                        }
                    })
                    .collect()
            }
            PaletteTab::Courses => renzora_auth::docs::list_courses(None, 1)
                .map(|r| r.courses)
                .unwrap_or_default()
                .into_iter()
                .filter(|c| {
                    q.is_empty()
                        || c.title.to_lowercase().contains(&q)
                        || c.category.to_lowercase().contains(&q)
                })
                .map(|c| RemoteHit {
                    kind: "Course",
                    label: c.title,
                    detail: Some(if c.category.is_empty() {
                        format!("{} chapters", c.chapter_count)
                    } else {
                        format!("{} · {} chapters", c.category, c.chapter_count)
                    }),
                    action: RemoteAction::Social(renzora::core::SocialPanelRequest::Learn),
                })
                .collect(),
            PaletteTab::Marketplace => {
                let query_opt = (!query.trim().is_empty()).then_some(query.trim());
                renzora_auth::marketplace::list_assets(query_opt, None, None, 1, None, None)
                    .map(|r| r.assets)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|a| RemoteHit {
                        kind: "Asset",
                        label: a.name,
                        detail: Some(if a.price_credits == 0 {
                            format!("{} · free", a.category)
                        } else {
                            format!("{} · {} credits", a.category, a.price_credits)
                        }),
                        action: RemoteAction::Panel("hub_store"),
                    })
                    .collect()
            }
            _ => Vec::new(),
        };
        let _ = tx.send((tab, query, hits));
    });
}

#[cfg(target_arch = "wasm32")]
fn dispatch_remote(
    tab: PaletteTab,
    query: String,
    _session: Option<renzora_auth::AuthSession>,
    tx: crossbeam_channel::Sender<(PaletteTab, String, Vec<RemoteHit>)>,
) {
    let _ = tx.send((tab, query, Vec::new()));
}

renzora::add!(CommandPalettePlugin, Editor);
