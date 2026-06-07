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

#[derive(Resource, Default)]
pub struct CommandPaletteState {
    pub open: bool,
    pub query: String,
    pub selected: usize,
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

renzora::add!(CommandPalettePlugin, Editor);
