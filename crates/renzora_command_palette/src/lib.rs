#![allow(deprecated)] // egui API rename pending; will migrate at next bevy_egui bump.
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
use bevy_egui::egui::{self, Align, Color32, Layout, RichText};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass};
use renzora::core::keybindings::{EditorAction, KeyBindings};
use renzora_editor::{
    AppEditorExt, EditorCommands, ShortcutEntry, ShortcutRegistry, SplashState, ToolEntry,
    ToolbarRegistry,
};
use renzora_theme::ThemeManager;

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
                EguiPrimaryContextPass,
                render_palette.run_if(in_state(SplashState::Editor)),
            )
            .add_systems(
                Update,
                consume_toggle_request.run_if(in_state(SplashState::Editor)),
            );
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
/// after `egui::Ui` frees its borrows.
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
    if let Some(manager) = world.get_resource::<renzora_editor::LayoutManager>() {
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
                    w.resource_scope::<renzora_editor::LayoutManager, _>(|w, mut mgr| {
                        if let Some(mut docking) =
                            w.get_resource_mut::<renzora_editor::DockingState>()
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
    if let Some(registry) = world.get_resource::<renzora_editor::PanelRegistry>() {
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
                    if let Some(mut docking) = w.get_resource_mut::<renzora_editor::DockingState>()
                    {
                        docking.tree.focus_or_add_panel(&id);
                    }
                }),
            });
        }
    }

    // Settings tabs — open the settings overlay on a specific tab.
    use renzora_editor::SettingsTab;
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
                if let Some(mut s) = w.get_resource_mut::<renzora_editor::EditorSettings>() {
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

// ── Render ─────────────────────────────────────────────────────────────────

fn render_palette(world: &mut World) {
    // Early exit if closed — avoid egui work.
    if !world.resource::<CommandPaletteState>().open {
        return;
    }

    // Snapshot everything we need before taking the egui context.
    let items = {
        let toolbar = world.resource::<ToolbarRegistry>().clone();
        let shortcuts = world.resource::<ShortcutRegistry>().clone();
        let keybindings = world.resource::<KeyBindings>().clone();
        let all = collect_items(&toolbar, &shortcuts, &keybindings, world);
        let query = world.resource::<CommandPaletteState>().query.clone();
        filter_items(all, &query)
    };

    // Clamp selection inside the filtered list.
    {
        let mut state = world.resource_mut::<CommandPaletteState>();
        if state.selected >= items.len().max(1) {
            state.selected = items.len().saturating_sub(1);
        }
    }

    let theme_snapshot = world
        .get_resource::<ThemeManager>()
        .map(|m| m.active_theme.clone());

    // Pull the egui context via SystemState since we're in an exclusive system.
    let ctx = {
        let mut state: bevy::ecs::system::SystemState<EguiContexts> =
            bevy::ecs::system::SystemState::new(world);
        let mut ctxs = state.get_mut(world);
        let Ok(ctx) = ctxs.ctx_mut() else { return };
        ctx.clone()
    };

    // Keyboard input captured before the modal renders so we can act on
    // Enter/Escape/Arrow without interfering with the text edit.
    let (enter_pressed, escape_pressed, up_pressed, down_pressed) = ctx.input(|i| {
        (
            i.key_pressed(egui::Key::Enter),
            i.key_pressed(egui::Key::Escape),
            i.key_pressed(egui::Key::ArrowUp),
            i.key_pressed(egui::Key::ArrowDown),
        )
    });

    // Snapshot the current selection + query, then re-enter the state to mutate.
    let (mut query, mut selected, just_opened) = {
        let s = world.resource::<CommandPaletteState>();
        (s.query.clone(), s.selected, s.just_opened)
    };

    if escape_pressed {
        let mut state = world.resource_mut::<CommandPaletteState>();
        state.open = false;
        state.just_opened = false;
        return;
    }

    if up_pressed && !items.is_empty() {
        selected = if selected == 0 {
            items.len() - 1
        } else {
            selected - 1
        };
    }
    if down_pressed && !items.is_empty() {
        selected = (selected + 1) % items.len();
    }

    // Render the modal.
    let screen = ctx.input(|i| i.screen_rect());
    let panel_w = 560.0_f32.min(screen.width() - 40.0);
    let panel_pos = egui::Pos2::new((screen.width() - panel_w) * 0.5, screen.height() * 0.22);

    let (bg, border, row_hover, text_primary, text_muted, accent) = if let Some(t) = &theme_snapshot
    {
        (
            t.surfaces.panel.to_color32(),
            t.widgets.border.to_color32(),
            t.widgets.hovered_bg.to_color32(),
            t.text.primary.to_color32(),
            t.text.muted.to_color32(),
            t.semantic.accent.to_color32(),
        )
    } else {
        (
            Color32::from_rgb(30, 30, 32),
            Color32::from_rgb(80, 80, 88),
            Color32::from_rgb(55, 55, 65),
            Color32::WHITE,
            Color32::from_gray(160),
            Color32::from_rgb(90, 170, 255),
        )
    };

    // Backdrop (full-screen dim) — captures clicks outside the panel to close.
    let mut close_requested = false;
    egui::Area::new(egui::Id::new("command_palette_backdrop"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::Pos2::ZERO)
        .show(&ctx, |ui| {
            let resp = ui.allocate_rect(screen, egui::Sense::click());
            ui.painter()
                .rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 120));
            if resp.clicked() {
                close_requested = true;
            }
        });

    let mut run_selected = false;

    egui::Area::new(egui::Id::new("command_palette"))
        .order(egui::Order::Tooltip)
        .fixed_pos(panel_pos)
        .show(&ctx, |ui| {
            let frame = egui::Frame::new()
                .fill(bg)
                .stroke(egui::Stroke::new(1.0, border))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::same(8));

            frame.show(ui, |ui| {
                ui.set_width(panel_w);

                // Text input
                let edit = egui::TextEdit::singleline(&mut query)
                    .hint_text("Search tools and actions…")
                    .font(egui::TextStyle::Heading)
                    .text_color(text_primary)
                    .desired_width(panel_w - 16.0);
                let edit_resp = ui.add(edit);
                if just_opened {
                    edit_resp.request_focus();
                }
                if !edit_resp.has_focus() && !just_opened {
                    edit_resp.request_focus();
                }
                ui.add_space(4.0);

                // Results
                egui::ScrollArea::vertical()
                    .max_height(360.0)
                    .show(ui, |ui| {
                        if items.is_empty() {
                            ui.add_space(20.0);
                            ui.vertical_centered(|ui| {
                                ui.label(RichText::new("No matches").color(text_muted).size(13.0));
                            });
                            return;
                        }

                        for (i, item) in items.iter().enumerate() {
                            let is_sel = i == selected;
                            let row_rect = ui.allocate_response(
                                egui::Vec2::new(panel_w - 16.0, 24.0),
                                egui::Sense::click(),
                            );

                            if is_sel || row_rect.hovered() {
                                ui.painter().rect_filled(
                                    row_rect.rect,
                                    egui::CornerRadius::same(4),
                                    row_hover,
                                );
                            }

                            let row_inner = row_rect.rect.shrink2(egui::Vec2::new(6.0, 2.0));
                            ui.scope_builder(
                                egui::UiBuilder::new()
                                    .max_rect(row_inner)
                                    .layout(Layout::left_to_right(Align::Center)),
                                |ui| {
                                    ui.label(
                                        RichText::new(item.kind)
                                            .color(accent)
                                            .size(10.0)
                                            .monospace(),
                                    );
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new(&item.label).color(text_primary).size(12.0),
                                    );
                                    if let Some(detail) = &item.detail {
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                ui.label(
                                                    RichText::new(detail)
                                                        .color(text_muted)
                                                        .monospace()
                                                        .size(11.0),
                                                );
                                            },
                                        );
                                    }
                                },
                            );

                            if row_rect.clicked() {
                                selected = i;
                                run_selected = true;
                            }
                        }
                    });
            });
        });

    // Commit query / selection back to state.
    {
        let mut state = world.resource_mut::<CommandPaletteState>();
        state.query = query;
        state.selected = selected;
        state.just_opened = false;
    }

    if close_requested {
        world.resource_mut::<CommandPaletteState>().open = false;
        return;
    }

    if (enter_pressed || run_selected) && !items.is_empty() {
        let handler = items[selected].handler.clone();
        // Close before running so the handler can open another modal without
        // leaving the palette visible behind it.
        world.resource_mut::<CommandPaletteState>().open = false;

        // Defer execution through EditorCommands if possible so actions that
        // also use the deferred queue (e.g. tool activations) run in the
        // expected order. Fall back to direct invocation if the resource
        // isn't initialised.
        if world.get_resource::<EditorCommands>().is_some() {
            world
                .resource::<EditorCommands>()
                .push(move |w: &mut World| (handler)(w));
        } else {
            (handler)(world);
        }
    }
}

renzora::add!(CommandPalettePlugin, Editor);
