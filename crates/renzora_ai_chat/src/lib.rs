//! AI Chat panel — talk to a local [Ollama] server (or any OpenAI-compatible
//! endpoint) from inside the editor.
//!
//! Adds an "AI Chat" dockable panel: pick a provider and model, type a
//! prompt, and the reply streams in token by token. Requests run on a worker
//! thread so the editor never blocks; deltas arrive through a channel and a
//! per-frame system appends them to the conversation.
//!
//! Providers are presets over three wire protocols:
//!   - **Ollama** (default, local) — `/api/chat` NDJSON streaming, models
//!     from `/api/tags`, no key needed.
//!   - **OpenAI-style SSE** — OpenAI, Grok (xAI), DeepSeek, Gemini (via
//!     Google's `/v1beta/openai` compatibility layer), OpenRouter, and any
//!     custom `/v1` server (LM Studio, llama.cpp). Bearer API key.
//!   - **Anthropic Messages API** — Claude, via `/v1/messages` SSE streaming
//!     (`x-api-key` + `anthropic-version` headers), models from `/v1/models`.
//!
//! Editor-scope distribution plugin: never loads in an exported game, and
//! deleting `librenzora_ai_chat.{dll,so,dylib}` from `plugins/` removes the
//! feature entirely.
//!
//! [Ollama]: https://ollama.com

use std::hash::{Hash, Hasher};
use std::io::BufRead;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;

use bevy::prelude::*;

use renzora::core::RenzoraShellExt;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{
    bind_2way, bind_bg, bind_display, bind_text, keyed_list, KeyedSnapshot,
};
use renzora_ember::settings_sections::RegisterSettingsSection;
use renzora_ember::theme::{accent, rgb, tab_active, text_muted, text_primary};
use renzora_ember::widgets::{
    bind_text_input, button, dropdown, markdown_view, password_input, scroll_view_pinned,
    spinner, text_input, EmberTextInput,
};

/// Wire protocol — how to talk to the server. Several providers share the
/// OpenAI shape, so presets map onto one of these three.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Protocol {
    Ollama,
    OpenAi,
    Anthropic,
}

/// A provider preset: label + protocol + default endpoints. The base URL is
/// user-editable after selection; the paths are fixed per provider.
struct Preset {
    label: &'static str,
    protocol: Protocol,
    base_url: &'static str,
    chat_path: &'static str,
    models_path: &'static str,
    /// Model to preselect when the fetched list contains it.
    preferred_model: Option<&'static str>,
}

/// Grounding rules sent with every request. Small local models in
/// particular will otherwise happily claim to have "reviewed" pages they
/// never saw.
const SYSTEM_PROMPT: &str = "You are the Renzora Engine assistant, embedded in the Renzora \
editor. Renzora is a Bevy-based game engine: gameplay scripts are written in \
Lua or Rhai; editor/game UI is built from HUI HTML templates (bevy_hui); the \
engine is extended with Rust plugins — crates that register a Bevy Plugin via \
renzora::add!(), compiled in statically or shipped as a dylib in the engine's \
plugins/ folder. Help users write scripts, HUI templates and plugins. \
Accuracy rules: you cannot browse the web yourself. '[Engine docs: …]' blocks \
are excerpts from the official Renzora manual — treat them as authoritative \
and prefer them over your own assumptions. '[Fetched page: <url>]' blocks \
are the real content of that page; base any statements about the page \
strictly on them. If a page block says it could not be fetched or looks like \
an empty app shell, say that and do not guess its contents. Never invent \
documentation, APIs, file names or facts; if the provided context doesn't \
cover something, say so plainly. Prefer short, correct answers over long, \
speculative ones. \
Code rules: when writing Lua or Rhai scripts, use ONLY functions, hooks and \
globals that appear in the provided '[Engine docs: …]' blocks. Renzora \
scripts are per-entity: there is NO entity object, no get_component, no \
component classes and no queries — the API is plain global functions (e.g. \
translate, set_position, rotate, set_velocity), read-only per-frame globals \
(delta, position_x, input_x, …) and optional lifecycle hooks you define \
(on_ready, on_update, …). If a function you need is not in the provided \
docs, say it is not documented instead of inventing it.";

const PRESETS: &[Preset] = &[
    Preset {
        label: "Ollama (local)",
        protocol: Protocol::Ollama,
        base_url: "http://localhost:11434",
        chat_path: "/api/chat",
        models_path: "/api/tags",
        preferred_model: None,
    },
    Preset {
        label: "Claude (Anthropic)",
        protocol: Protocol::Anthropic,
        base_url: "https://api.anthropic.com",
        chat_path: "/v1/messages",
        models_path: "/v1/models",
        preferred_model: Some("claude-opus-4-8"),
    },
    Preset {
        label: "OpenAI",
        protocol: Protocol::OpenAi,
        base_url: "https://api.openai.com",
        chat_path: "/v1/chat/completions",
        models_path: "/v1/models",
        preferred_model: None,
    },
    Preset {
        label: "Grok (xAI)",
        protocol: Protocol::OpenAi,
        base_url: "https://api.x.ai",
        chat_path: "/v1/chat/completions",
        models_path: "/v1/models",
        preferred_model: None,
    },
    Preset {
        label: "DeepSeek",
        protocol: Protocol::OpenAi,
        base_url: "https://api.deepseek.com",
        chat_path: "/v1/chat/completions",
        models_path: "/v1/models",
        preferred_model: None,
    },
    Preset {
        label: "Gemini (Google)",
        protocol: Protocol::OpenAi,
        // Google's OpenAI-compatibility layer; the native Gemini API has a
        // different shape, but this endpoint speaks plain OpenAI SSE.
        base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
        chat_path: "/chat/completions",
        models_path: "/models",
        preferred_model: None,
    },
    Preset {
        label: "OpenRouter",
        protocol: Protocol::OpenAi,
        base_url: "https://openrouter.ai/api",
        chat_path: "/v1/chat/completions",
        models_path: "/v1/models",
        preferred_model: None,
    },
    Preset {
        label: "Custom (OpenAI-compatible)",
        protocol: Protocol::OpenAi,
        base_url: "http://localhost:1234",
        chat_path: "/v1/chat/completions",
        models_path: "/v1/models",
        preferred_model: None,
    },
];

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Role {
    User,
    Assistant,
}

#[derive(Clone)]
struct ChatMsg {
    role: Role,
    content: String,
    /// Fetched-page text attached to this (user) message. Sent to the model
    /// with the message but not displayed in the conversation.
    context: Option<String>,
}

enum StreamEvent {
    Delta(String),
    Done,
    Error(String),
    /// Progress note ("Fetching https://…") shown in the status line.
    Status(String),
    /// Fetched-page context to attach to the last user message, so
    /// follow-up turns keep the grounding.
    Context(String),
}

/// The connection settings persisted across editor runs, stored at
/// `~/.config/renzora/ai_chat.json` (APPDATA on Windows). Note the API key
/// is stored in plain text, like most local dev tooling — the file lives in
/// the user's own config directory.
#[derive(Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
struct AiChatConfig {
    preset: usize,
    base_url: String,
    api_key: String,
    docs_path: String,
    model: String,
}

impl AiChatConfig {
    fn from_chat(c: &AiChat) -> Self {
        Self {
            preset: c.preset,
            base_url: c.base_url.clone(),
            api_key: c.api_key.clone(),
            docs_path: c.docs_path.clone(),
            model: c.model.clone(),
        }
    }

    fn apply(self, c: &mut AiChat) {
        c.preset = self.preset.min(PRESETS.len() - 1);
        if !self.base_url.is_empty() {
            c.base_url = self.base_url;
        }
        c.api_key = self.api_key;
        if !self.docs_path.is_empty() {
            c.docs_path = self.docs_path;
        }
        c.model = self.model;
    }
}

fn config_path() -> Option<std::path::PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(std::path::PathBuf::from)?
    } else {
        std::path::PathBuf::from(std::env::var_os("HOME")?).join(".config")
    };
    Some(base.join("renzora").join("ai_chat.json"))
}

fn load_config() -> Option<AiChatConfig> {
    serde_json::from_str(&std::fs::read_to_string(config_path()?).ok()?).ok()
}

fn save_config(cfg: &AiChatConfig) {
    let Some(path) = config_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(cfg) {
        let _ = std::fs::write(path, json);
    }
}

/// Debounced save of the connection settings whenever they change. The
/// resource also changes every frame while a reply streams, so the dirty
/// flag + value comparison keep writes to actual config edits.
fn persist_settings(
    chat: Res<AiChat>,
    time: Res<Time>,
    mut pending: Local<bool>,
    mut last_saved: Local<Option<AiChatConfig>>,
    mut last_save_at: Local<f64>,
) {
    if chat.is_changed() {
        *pending = true;
    }
    let now = time.elapsed_secs_f64();
    if !*pending || now - *last_save_at < 1.0 {
        return;
    }
    let cfg = AiChatConfig::from_chat(&chat);
    if last_saved.as_ref() != Some(&cfg) {
        save_config(&cfg);
        *last_saved = Some(cfg);
    }
    *pending = false;
    *last_save_at = now;
}

/// Conversation + connection + in-flight request state. Receivers are
/// wrapped in `Mutex` only because `Resource` needs `Sync`; each is read
/// from one system.
#[derive(Resource)]
struct AiChat {
    /// Index into [`PRESETS`].
    preset: usize,
    base_url: String,
    api_key: String,
    /// Local folder of markdown docs used for retrieval grounding.
    docs_path: String,
    model: String,
    models: Vec<String>,
    messages: Vec<ChatMsg>,
    stream: Option<Mutex<Receiver<StreamEvent>>>,
    models_rx: Option<Mutex<Receiver<Result<Vec<String>, String>>>>,
    status: String,
}

impl Default for AiChat {
    fn default() -> Self {
        Self {
            preset: 0,
            base_url: PRESETS[0].base_url.to_string(),
            api_key: String::new(),
            docs_path: if std::path::Path::new("docs").is_dir() {
                "docs".to_string()
            } else {
                String::new()
            },
            model: String::new(),
            models: Vec::new(),
            messages: Vec::new(),
            stream: None,
            models_rx: None,
            status: "Connecting…".to_string(),
        }
    }
}

#[derive(Component)]
struct SendBtn;
#[derive(Component)]
struct ClearBtn;
#[derive(Component)]
struct RefreshBtn;
#[derive(Component)]
struct PromptInput;
/// The animated dots inside the thinking bubble.
#[derive(Component)]
struct ThinkingDots;
/// "Choose…" button next to the docs-folder setting.
#[derive(Component)]
struct DocsBrowseBtn;

#[derive(Default)]
pub struct AiChatPlugin;

impl Plugin for AiChatPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] AiChatPlugin (AI Chat panel)");
        let mut chat = AiChat::default();
        if let Some(cfg) = load_config() {
            cfg.apply(&mut chat);
        }
        app.insert_resource(chat);
        app.register_shell_panel("ai_chat", "AI Chat", "robot", "AI");
        app.register_panel_content("ai_chat", false, build_panel);
        // Connection config lives in Settings → Plugins, not the chat panel.
        app.register_settings_section("ai_chat", "AI Chat", "robot", build_settings);
        app.add_systems(
            Update,
            (
                refresh_click,
                docs_browse_click,
                send_prompt,
                drain_models,
                drain_stream,
                animate_thinking,
                persist_settings,
            )
                .run_if(in_state(renzora::SplashState::Editor)),
        );
    }
}

// ── Panel UI ─────────────────────────────────────────────────────────────────

fn build_panel(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(8.0)),
            row_gap: Val::Px(6.0),
            min_height: Val::Px(0.0),
            ..default()
        })
        .id();

    // Status line — shows "Thinking…" while a reply streams.
    let status = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted()))))
        .id();
    bind_text(commands, status, |w| {
        w.get_resource::<AiChat>()
            .map(|c| c.status.clone())
            .unwrap_or_default()
    });

    // Model row: dropdown (rebuilt when the model list changes) + actions.
    // Provider/URL/key live in Settings → Plugins → AI Chat.
    let model_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .id();
    let model_slot = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            ..default()
        })
        .id();
    keyed_list(commands, model_slot, model_dropdown_snapshot);
    let refresh = button(commands, &fonts.ui, "Refresh");
    commands.entity(refresh).insert(RefreshBtn);
    let clear = button(commands, &fonts.ui, "Clear");
    commands.entity(clear).insert(ClearBtn);
    commands
        .entity(model_row)
        .add_children(&[model_slot, refresh, clear]);

    // Prompt row.
    let prompt_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            align_items: AlignItems::Center,
            width: Val::Percent(100.0),
            ..default()
        })
        .id();
    let prompt = text_input(commands, &fonts.ui, "Type a message...", "");
    commands.entity(prompt).insert(PromptInput);
    grow(commands, prompt, 1.0);
    // Send ⇄ Stop toggle: while a reply streams, the button turns red,
    // shows a spinner, and a click cancels generation.
    let send = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(tab_active())),
            Interaction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            SendBtn,
            Name::new("send-stop"),
        ))
        .id();
    fn streaming(w: &World) -> bool {
        w.get_resource::<AiChat>().is_some_and(|c| c.stream.is_some())
    }
    bind_bg(commands, send, |w| {
        if streaming(w) {
            rgb((170, 70, 70))
        } else {
            rgb(tab_active())
        }
    });
    let spin = spinner(commands);
    bind_display(commands, spin, streaming);
    let send_label = commands
        .spawn((Text::new("Send"), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, send_label, |w| {
        if streaming(w) { "Stop".to_string() } else { "Send".to_string() }
    });
    commands.entity(send).add_children(&[spin, send_label]);
    commands.entity(prompt_row).add_children(&[prompt, send]);

    // Conversation — keyed by message index, rebuilt when content changes
    // (which, while streaming, is every frame for the last row). Lives in a
    // bottom-pinned scroll view: it follows new content while the user is at
    // the bottom, releases when they scroll up, and re-follows at the bottom.
    let messages = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            width: Val::Percent(100.0),
            ..default()
        })
        .id();
    keyed_list(commands, messages, message_snapshot);
    // The thinking bubble sits after the list rows: the keyed list inserts
    // its rows at index 0, so this extra child stays last.
    let thinking = thinking_bubble(commands, fonts);
    commands.entity(messages).add_child(thinking);
    let messages_scroll = scroll_view_pinned(commands, messages);

    let hint = commands
        .spawn((
            Text::new(
                "Provider, server URL and API key live in Settings → Plugins \
                 → AI Chat (Ollama local, or Claude / OpenAI / Grok / DeepSeek \
                 / Gemini / OpenRouter with an API key). Paste a URL in your \
                 message and the page is fetched and given to the model as \
                 real context. Prompts and fetched pages go only to the \
                 server configured there.",
            ),
            ui_font(&fonts.ui, 9.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_display(commands, hint, |w| {
        w.get_resource::<AiChat>()
            .is_some_and(|c| c.messages.is_empty())
    });

    commands
        .entity(root)
        .add_children(&[status, model_row, hint, messages_scroll, prompt_row]);

    // Initial model list fetch once the panel exists.
    commands.queue(start_models_fetch);
    root
}

/// "AI is typing" bubble — robot icon + animated dots, visible while a reply
/// streams. The dots are driven by [`animate_thinking`].
fn thinking_bubble(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            border_radius: BorderRadius::all(Val::Px(8.0)),
            align_self: AlignSelf::FlexStart,
            ..default()
        })
        .id();
    commands
        .entity(row)
        .insert(BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.05)));
    bind_display(commands, row, |w| {
        w.get_resource::<AiChat>().is_some_and(|c| c.stream.is_some())
    });
    let icon = icon_text(commands, &fonts.phosphor, "robot", (120, 210, 120), 12.0);
    let dots = commands
        .spawn((
            Text::new("·"),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
            ThinkingDots,
        ))
        .id();
    commands.entity(row).add_children(&[icon, dots]);
    row
}

/// Cycle the thinking-bubble dots (`·` → `··` → `···`).
fn animate_thinking(time: Res<Time>, mut dots: Query<&mut Text, With<ThinkingDots>>) {
    let phase = (time.elapsed_secs() * 2.5) as usize % 3;
    let s = ["·", "··", "···"][phase];
    for mut t in &mut dots {
        if t.0 != s {
            t.0 = s.to_string();
        }
    }
}

/// Settings → Plugins → AI Chat: provider, server URL, API key. Inputs are
/// two-way-bound to [`AiChat`], so the panel always reads current values.
fn build_settings(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();

    let provider_row = settings_row(commands, fonts, "Provider");
    let labels: Vec<&str> = PRESETS.iter().map(|p| p.label).collect();
    let provider = dropdown(commands, fonts, &labels, 0);
    // Selecting a provider resets the URL to its default and refreshes the
    // model list (the API key is kept — switching back and forth shouldn't
    // wipe it; each provider needs its own key pasted in anyway).
    bind_2way(
        commands,
        provider,
        |w| w.get_resource::<AiChat>().map(|c| c.preset).unwrap_or(0),
        |w, idx| {
            if let Some(mut c) = w.get_resource_mut::<AiChat>() {
                let idx = (*idx).min(PRESETS.len() - 1);
                c.preset = idx;
                c.base_url = PRESETS[idx].base_url.to_string();
                c.models = Vec::new();
                c.model = String::new();
            }
            start_models_fetch(w);
        },
    );
    commands.entity(provider_row).add_child(provider);

    let url_row = settings_row(commands, fonts, "Server URL");
    let url = text_input(commands, &fonts.ui, "server URL", PRESETS[0].base_url);
    grow(commands, url, 1.0);
    bind_text_input(
        commands,
        url,
        |w| {
            w.get_resource::<AiChat>()
                .map(|c| c.base_url.clone())
                .unwrap_or_default()
        },
        |w, v| {
            if let Some(mut c) = w.get_resource_mut::<AiChat>() {
                let v = v.trim().trim_end_matches('/');
                c.base_url = if v.is_empty() {
                    PRESETS[c.preset].base_url.to_string()
                } else {
                    v.to_string()
                };
            }
        },
    );
    commands.entity(url_row).add_child(url);

    let key_row = settings_row(commands, fonts, "API key");
    let key = password_input(commands, &fonts.ui, "optional", "");
    grow(commands, key, 1.0);
    bind_text_input(
        commands,
        key,
        |w| {
            w.get_resource::<AiChat>()
                .map(|c| c.api_key.clone())
                .unwrap_or_default()
        },
        |w, v| {
            if let Some(mut c) = w.get_resource_mut::<AiChat>() {
                c.api_key = v.trim().to_string();
            }
        },
    );
    commands.entity(key_row).add_child(key);

    let docs_row = settings_row(commands, fonts, "Docs folder");
    let docs = text_input(commands, &fonts.ui, "local folder of markdown docs (optional)", "");
    grow(commands, docs, 1.0);
    bind_text_input(
        commands,
        docs,
        |w| {
            w.get_resource::<AiChat>()
                .map(|c| c.docs_path.clone())
                .unwrap_or_default()
        },
        |w, v| {
            if let Some(mut c) = w.get_resource_mut::<AiChat>() {
                c.docs_path = v.trim().to_string();
            }
        },
    );
    commands.entity(docs_row).add_child(docs);
    let browse = button(commands, &fonts.ui, "Choose…");
    commands.entity(browse).insert(DocsBrowseBtn);
    commands.entity(docs_row).add_child(browse);

    let note = commands
        .spawn((
            Text::new(
                "Used by the AI Chat panel. Ollama runs locally and needs no \
                 key; cloud providers (Claude, OpenAI, Grok, DeepSeek, Gemini, \
                 OpenRouter) need that provider's API key. Selecting a \
                 provider resets the URL to its default. The key is kept in \
                 memory only and is sent solely to the configured server. \
                 Point 'Docs folder' at a local copy of the engine manual \
                 (markdown): each prompt then includes the most relevant \
                 excerpts, which beats fetching a script-rendered docs site. \
                 Refresh the model list from the panel after changing these.",
            ),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
            Node {
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            },
        ))
        .id();

    commands
        .entity(col)
        .add_children(&[provider_row, url_row, key_row, docs_row, note]);
    col
}

fn settings_row(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            width: Val::Percent(100.0),
            ..default()
        })
        .id();
    let l = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node {
                width: Val::Px(90.0),
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .id();
    commands.entity(row).add_child(l);
    row
}

/// Adjust a widget's `Node` in place — replacing it would wipe the widget's
/// own padding/border styling.
fn grow(commands: &mut Commands, entity: Entity, factor: f32) {
    commands.entity(entity).queue(move |mut e: EntityWorldMut| {
        if let Some(mut n) = e.get_mut::<Node>() {
            n.flex_grow = factor;
        }
    });
}

/// One-item keyed list whose hash is the model list: when the available
/// models change, the dropdown is rebuilt with fresh options and a fresh
/// two-way binding (index ↔ `AiChat::model`).
fn model_dropdown_snapshot(world: &World) -> KeyedSnapshot {
    let (models, current) = world
        .get_resource::<AiChat>()
        .map(|c| (c.models.clone(), c.model.clone()))
        .unwrap_or_default();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    models.hash(&mut h);
    let items = vec![(0u64, h.finish())];
    KeyedSnapshot {
        items,
        build: Box::new(move |commands, fonts, _| {
            if models.is_empty() {
                return commands
                    .spawn((
                        Text::new("no models — check URL, then Refresh"),
                        ui_font(&fonts.ui, 11.0),
                        TextColor(rgb(text_muted())),
                    ))
                    .id();
            }
            let opts: Vec<&str> = models.iter().map(String::as_str).collect();
            let sel = models.iter().position(|m| *m == current).unwrap_or(0);
            let dd = dropdown(commands, fonts, &opts, sel);
            let read_models = models.clone();
            let write_models = models.clone();
            bind_2way(
                commands,
                dd,
                move |w| {
                    w.get_resource::<AiChat>()
                        .and_then(|c| read_models.iter().position(|m| *m == c.model))
                        .unwrap_or(0)
                },
                move |w, idx: &usize| {
                    if let Some(mut c) = w.get_resource_mut::<AiChat>() {
                        if let Some(m) = write_models.get(*idx) {
                            c.model = m.clone();
                        }
                    }
                },
            );
            dd
        }),
    }
}

fn message_snapshot(world: &World) -> KeyedSnapshot {
    let messages: Vec<ChatMsg> = world
        .get_resource::<AiChat>()
        .map(|c| c.messages.clone())
        .unwrap_or_default();
    let items = messages
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            m.role.hash(&mut h);
            m.content.hash(&mut h);
            (i as u64, h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |commands, fonts, i| message_row(commands, fonts, &messages[i])),
    }
}

fn message_row(commands: &mut Commands, fonts: &EmberFonts, msg: &ChatMsg) -> Entity {
    let (icon, label, color) = match msg.role {
        Role::User => ("user-circle", "You", accent()),
        Role::Assistant => ("robot", "AI", (120, 210, 120)),
    };
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0),
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    let who = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    let ico = icon_text(commands, &fonts.phosphor, icon, color, 12.0);
    let name = commands
        .spawn((Text::new(label), ui_font(&fonts.ui, 10.0), TextColor(rgb(color))))
        .id();
    commands.entity(who).add_children(&[ico, name]);

    // The assistant speaks markdown (headings, lists, code blocks); user
    // prompts render verbatim.
    let body = match msg.role {
        Role::Assistant if !msg.content.is_empty() => {
            markdown_view(commands, fonts, &msg.content)
        }
        _ => commands
            .spawn((
                Text::new(if msg.content.is_empty() { "…" } else { &msg.content }),
                ui_font(&fonts.ui, 11.5),
                TextColor(rgb(text_primary())),
            ))
            .id(),
    };
    commands.entity(row).add_children(&[who, body]);
    row
}

// ── Model list fetching ──────────────────────────────────────────────────────

/// "Choose…" next to the docs-folder input → native folder picker. The
/// input reflects the new value through its binding.
fn docs_browse_click(
    buttons: Query<&Interaction, (With<DocsBrowseBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if buttons.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|world: &mut World| {
            let Some(folder) = rfd::FileDialog::new()
                .set_title("Select the engine docs folder")
                .pick_folder()
            else {
                return;
            };
            if let Some(mut chat) = world.get_resource_mut::<AiChat>() {
                chat.docs_path = folder.display().to_string();
            }
        });
    }
}

fn refresh_click(
    buttons: Query<&Interaction, (With<RefreshBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if buttons.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(start_models_fetch);
    }
}

/// Fetch the provider's model list on a worker thread (connection settings
/// are kept current by the Settings-section bindings).
fn start_models_fetch(world: &mut World) {
    let Some(mut chat) = world.get_resource_mut::<AiChat>() else {
        return;
    };
    let (preset, url, key) = (chat.preset, chat.base_url.clone(), chat.api_key.clone());
    let (tx, rx) = channel();
    chat.models_rx = Some(Mutex::new(rx));
    chat.status = format!("Loading models from {url}…");
    std::thread::Builder::new()
        .name("ai-chat-models".into())
        .spawn(move || {
            let _ = tx.send(fetch_models(&PRESETS[preset], &url, &key));
        })
        .ok();
}

fn drain_models(mut chat: ResMut<AiChat>) {
    let chat = &mut *chat;
    let Some(rx) = &chat.models_rx else { return };
    let result = match rx.lock().ok().and_then(|rx| rx.try_recv().ok()) {
        Some(r) => r,
        None => return,
    };
    chat.models_rx = None;
    match result {
        Ok(models) => {
            chat.status = format!(
                "{} model{} at {}",
                models.len(),
                if models.len() == 1 { "" } else { "s" },
                chat.base_url
            );
            if !models.contains(&chat.model) {
                let preferred = PRESETS[chat.preset]
                    .preferred_model
                    .map(str::to_string)
                    .filter(|m| models.contains(m));
                chat.model = preferred.or_else(|| models.first().cloned()).unwrap_or_default();
            }
            chat.models = models;
        }
        Err(e) => {
            chat.models = Vec::new();
            chat.model = String::new();
            chat.status = e;
        }
    }
}

fn fetch_models(preset: &Preset, url: &str, key: &str) -> Result<Vec<String>, String> {
    let endpoint = format!("{url}{}", preset.models_path);
    let mut req = agent().get(&endpoint);
    req = authorize(req, preset.protocol, key);
    let mut res = req
        .call()
        .map_err(|e| format!("Can't reach {endpoint}: {e}"))?;
    let body = res
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("Bad response from {endpoint}: {e}"))?;
    if res.status().as_u16() >= 400 {
        return Err(format!(
            "HTTP {} from {endpoint}: {}",
            res.status(),
            api_error_message(&body)
        ));
    }
    let value: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("Bad JSON from {endpoint}: {e}"))?;
    let list = match preset.protocol {
        Protocol::Ollama => value["models"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|m| m["name"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        // OpenAI-style and Anthropic both return `{"data": [{"id": ...}]}`.
        Protocol::OpenAi | Protocol::Anthropic => value["data"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|m| m["id"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
    };
    Ok(list)
}

// ── Send / receive ───────────────────────────────────────────────────────────

/// Starts a request on Send-click or when the prompt ends with a newline
/// (the input widget inserts `\n` on Enter — used here as "submit").
fn send_prompt(
    send: Query<&Interaction, (With<SendBtn>, Changed<Interaction>)>,
    clear: Query<&Interaction, (With<ClearBtn>, Changed<Interaction>)>,
    mut inputs: Query<(&mut EmberTextInput, Has<PromptInput>)>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
    mut chat: ResMut<AiChat>,
) {
    if clear.iter().any(|i| *i == Interaction::Pressed) && chat.stream.is_none() {
        chat.messages.clear();
    }

    let pressed = send.iter().any(|i| *i == Interaction::Pressed);
    if chat.stream.is_some() {
        if pressed {
            // Stop: dropping the receiver makes the worker thread abort on
            // its next send. Keep whatever already streamed in.
            chat.stream = None;
            if chat.messages.last().is_some_and(|m| m.content.is_empty()) {
                chat.messages.pop();
            }
            chat.status = "Stopped.".to_string();
        }
        // Don't queue submissions mid-stream; just strip Enter's newline so
        // it can't auto-send once the stream ends.
        for (mut input, is_prompt) in &mut inputs {
            if is_prompt && input.value.ends_with('\n') {
                input.value.pop();
            }
        }
        return;
    }

    let mut submit = pressed;
    let mut prompt = None;
    for (mut input, is_prompt) in &mut inputs {
        if !is_prompt {
            continue;
        }
        if input.value.ends_with('\n') {
            submit = true;
            input.value.pop();
        }
        if submit {
            let text = input.value.trim().to_string();
            if !text.is_empty() {
                prompt = Some(text);
                input.value.clear();
                // The widget only redraws its display text on keystrokes /
                // external-binding changes — reset it to the placeholder
                // ourselves so the sent prompt doesn't linger in the box.
                if let Ok((mut t, mut c)) = texts.get_mut(input.text_entity) {
                    t.0 = input.placeholder.clone();
                    c.0 = rgb(text_muted());
                }
            }
        }
    }

    let Some(prompt) = prompt else { return };
    if chat.model.is_empty() {
        chat.status = "Pick a model first (Refresh to load the list).".to_string();
        chat.messages.push(ChatMsg {
            role: Role::User,
            content: prompt,
            context: None,
        });
        return;
    }

    // URLs in the prompt get fetched on the worker thread and injected as
    // grounding context before the request is sent.
    let urls = extract_urls(&prompt);
    let query = prompt.clone();
    chat.messages.push(ChatMsg {
        role: Role::User,
        content: prompt,
        context: None,
    });
    // History snapshot for the request — fetched-page context rides along
    // with its message; the empty Assistant message added after is the
    // streaming target, not part of the payload.
    let history: Vec<(&'static str, String)> = chat
        .messages
        .iter()
        .map(|m| {
            let content = match &m.context {
                Some(ctx) => format!("{}\n\n{ctx}", m.content),
                None => m.content.clone(),
            };
            (
                match m.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                },
                content,
            )
        })
        .collect();
    chat.messages.push(ChatMsg {
        role: Role::Assistant,
        content: String::new(),
        context: None,
    });

    let (tx, rx) = channel();
    chat.stream = Some(Mutex::new(rx));
    let job = RequestJob {
        preset: chat.preset,
        base_url: chat.base_url.clone(),
        api_key: chat.api_key.clone(),
        model: chat.model.clone(),
        query,
        history,
        urls,
        docs_path: chat.docs_path.clone(),
    };
    std::thread::Builder::new()
        .name("ai-chat".into())
        .spawn(move || run_request(job, tx))
        .ok();
}

/// Everything the worker thread needs for one chat request.
struct RequestJob {
    preset: usize,
    base_url: String,
    api_key: String,
    model: String,
    /// The user's raw prompt — used to score docs chunks and crawled links.
    query: String,
    history: Vec<(&'static str, String)>,
    urls: Vec<String>,
    docs_path: String,
}

/// Applies streamed deltas to the in-progress assistant message.
fn drain_stream(mut chat: ResMut<AiChat>) {
    let chat = &mut *chat;
    let Some(stream) = &chat.stream else { return };

    let mut finished: Option<Option<String>> = None; // Some(err?) when done
    let mut deltas = String::new();
    let mut status = None;
    let mut context = None;
    {
        let Ok(rx) = stream.lock() else { return };
        while let Ok(event) = rx.try_recv() {
            match event {
                StreamEvent::Delta(d) => deltas.push_str(&d),
                StreamEvent::Done => finished = Some(None),
                StreamEvent::Error(e) => finished = Some(Some(e)),
                StreamEvent::Status(st) => status = Some(st),
                StreamEvent::Context(ctx) => context = Some(ctx),
            }
        }
    }
    if let Some(st) = status {
        chat.status = st;
    }
    if let Some(ctx) = context {
        // Attach to the message being answered so follow-up turns keep the
        // grounding (the last user message — the streaming target sits after).
        if let Some(m) = chat.messages.iter_mut().rev().find(|m| m.role == Role::User) {
            m.context = Some(ctx);
        }
    }

    if !deltas.is_empty() {
        if let Some(last) = chat.messages.last_mut() {
            last.content.push_str(&deltas);
        }
    }
    if let Some(error) = finished {
        chat.stream = None;
        match error {
            None => {
                let docs_note = if chat.docs_path.is_empty() {
                    " · ungrounded — set a docs folder in Settings → Plugins → AI Chat"
                } else {
                    ""
                };
                chat.status = format!(
                    "{} · {}{docs_note}",
                    chat.model,
                    chat.base_url.trim_start_matches("http://")
                );
            }
            Some(e) => {
                chat.status = e;
                // Drop the empty streaming target so the failed exchange
                // doesn't leave a dangling "…" bubble.
                if chat.messages.last().is_some_and(|m| m.content.is_empty()) {
                    chat.messages.pop();
                }
            }
        }
    }
}

/// Worker thread: streaming chat request, provider-specific wire format.
fn run_request(mut job: RequestJob, tx: Sender<StreamEvent>) {
    let preset = &PRESETS[job.preset];
    // Ground the request: local docs excerpts first (authoritative), then any
    // URLs from the prompt (plus a shallow crawl of related same-site links).
    // Failures are reported INTO the context so the model knows the page was
    // not seen (instead of inventing it).
    let mut context = String::new();

    if !job.docs_path.is_empty() {
        let _ = tx.send(StreamEvent::Status("Searching engine docs…".to_string()));
        if resolve_docs_root(&job.docs_path).is_none() {
            let _ = tx.send(StreamEvent::Status(format!(
                "Docs folder '{}' not found — check Settings → Plugins → AI Chat",
                job.docs_path
            )));
        } else {
            let excerpts = retrieve_docs(&job.docs_path, &job.query);
            let _ = tx.send(StreamEvent::Status(format!(
                "Grounding with {} docs excerpt{}…",
                excerpts.len(),
                if excerpts.len() == 1 { "" } else { "s" }
            )));
            for (label, text) in excerpts {
                context.push_str(&format!("[Engine docs: {label}]\n{text}\n\n"));
            }
        }
    }

    for page_url in &job.urls {
        let _ = tx.send(StreamEvent::Status(format!("Fetching {page_url}…")));
        match fetch_raw(page_url) {
            Ok(html) => {
                let mut text = clamp_text(html_to_text(&html), PAGE_CHAR_BUDGET);
                if text.len() < 200 {
                    // App-shell detection: tell the model the truth about
                    // script-rendered pages instead of letting it guess.
                    text.push_str(
                        "\n[This page appears to be script-rendered (an empty app \
shell); its real content is not visible to a plain fetch.]",
                    );
                }
                context.push_str(&format!("[Fetched page: {page_url}]\n{text}\n\n"));

                // Shallow crawl: follow up to 3 same-site links whose label
                // or URL relates to the prompt.
                for link in related_links(&html, page_url, &job.query, 3) {
                    let _ = tx.send(StreamEvent::Status(format!("Fetching {link}…")));
                    if let Ok(linked) = fetch_raw(&link) {
                        let text = clamp_text(html_to_text(&linked), CRAWL_CHAR_BUDGET);
                        if text.len() >= 200 {
                            context.push_str(&format!("[Fetched page: {link}]\n{text}\n\n"));
                        }
                    }
                }
            }
            Err(e) => {
                context.push_str(&format!(
                    "[Could not fetch {page_url}: {e}. You have NOT seen this page.]\n\n"
                ));
            }
        }
    }

    if !context.is_empty() {
        let context = context.trim_end().to_string();
        if let Some(last) = job.history.iter_mut().rev().find(|(role, _)| *role == "user") {
            last.1 = format!("{}\n\n{context}", last.1);
        }
        // Persist on the stored message so follow-up turns keep the grounding.
        let _ = tx.send(StreamEvent::Context(context));
        let _ = tx.send(StreamEvent::Status("Thinking…".to_string()));
    }

    let (model, key, history) = (job.model, job.api_key, job.history);
    let url = job.base_url;

    let messages: Vec<serde_json::Value> = history
        .iter()
        .map(|(role, content)| serde_json::json!({ "role": role, "content": content }))
        .collect();
    let endpoint = format!("{url}{}", preset.chat_path);
    let payload = match preset.protocol {
        Protocol::Ollama => serde_json::json!({
            "model": model,
            "stream": true,
            // System prompt rides as the first message; raise Ollama's
            // default context window (often only ~4K tokens — silent
            // truncation of long chats is a major hallucination source).
            "messages": with_system(messages),
            "options": { "num_ctx": 16384 },
        }),
        Protocol::OpenAi => serde_json::json!({
            "model": model,
            "stream": true,
            "messages": with_system(messages),
        }),
        // The Messages API requires max_tokens and takes the system prompt
        // as a top-level field, not a message role.
        Protocol::Anthropic => serde_json::json!({
            "model": model,
            "max_tokens": 8192,
            "stream": true,
            "system": SYSTEM_PROMPT,
            "messages": messages,
        }),
    };

    let req = authorize(
        agent().post(&endpoint).header("Content-Type", "application/json"),
        preset.protocol,
        &key,
    );
    let mut response = match req.send(payload.to_string().as_bytes()) {
        Ok(r) => r,
        Err(e) => {
            let _ = tx.send(StreamEvent::Error(format!(
                "Can't reach {endpoint} ({e}). Is the server running?"
            )));
            return;
        }
    };

    // Surface the server's own error message (e.g. Ollama's
    // "model not found") instead of a bare status code.
    if response.status().as_u16() >= 400 {
        let status = response.status();
        let body = response
            .body_mut()
            .read_to_string()
            .unwrap_or_default();
        let _ = tx.send(StreamEvent::Error(format!(
            "HTTP {status}: {}",
            api_error_message(&body)
        )));
        return;
    }

    let reader = std::io::BufReader::new(response.into_body().into_reader());
    for line in reader.lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // OpenAI-style SSE frames are `data: {json}` / `data: [DONE]`;
        // Ollama is bare NDJSON. Normalize to the JSON part.
        let json_part = line.strip_prefix("data:").map(str::trim).unwrap_or(line);
        if json_part == "[DONE]" {
            break;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(json_part) else {
            continue;
        };
        if let Some(err) = value["error"].as_str() {
            let _ = tx.send(StreamEvent::Error(format!("Server: {err}")));
            return;
        }
        if let Some(err) = value["error"]["message"].as_str() {
            let _ = tx.send(StreamEvent::Error(format!("Server: {err}")));
            return;
        }
        let delta = match preset.protocol {
            Protocol::Ollama => value["message"]["content"].as_str(),
            Protocol::OpenAi => value["choices"][0]["delta"]["content"].as_str(),
            // Messages API: `content_block_delta` frames carry `text_delta`s.
            Protocol::Anthropic => value["delta"]["text"].as_str(),
        };
        if let Some(delta) = delta {
            if !delta.is_empty() && tx.send(StreamEvent::Delta(delta.to_string())).is_err() {
                return; // panel gone; stop streaming
            }
        }
        // End-of-turn: Ollama sets `done`; Anthropic emits `message_stop`
        // (OpenAI-style streams end with `data: [DONE]`, handled above).
        if value["done"].as_bool() == Some(true)
            || value["type"].as_str() == Some("message_stop")
        {
            break;
        }
    }
    let _ = tx.send(StreamEvent::Done);
}

/// Prepend the system prompt as the first chat message (Ollama / OpenAI
/// style; Anthropic takes it as a top-level field instead).
fn with_system(mut messages: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    messages.insert(
        0,
        serde_json::json!({ "role": "system", "content": SYSTEM_PROMPT }),
    );
    messages
}

/// Pull plausible http(s) URLs out of a prompt (max 3, surrounding
/// punctuation trimmed).
fn extract_urls(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter_map(|token| {
            let start = token.find("http://").or_else(|| token.find("https://"))?;
            let url = token[start..].trim_end_matches(['.', ',', ';', ':', ')', ']', '>', '"', '\''])
                ;
            (url.len() > 10).then(|| url.to_string())
        })
        .take(3)
        .collect()
}

/// Per-page character budget for fetched context. Small local models often
/// run with limited context windows; this keeps one page from eating it all.
const PAGE_CHAR_BUDGET: usize = 9000;

/// Per crawled-link character budget (smaller than the primary page's).
const CRAWL_CHAR_BUDGET: usize = 5000;
/// Total character budget for local-docs excerpts.
const DOCS_CHAR_BUDGET: usize = 9000;

/// Fetch a URL's raw body (HTML).
fn fetch_raw(url: &str) -> Result<String, String> {
    let mut res = agent()
        .get(url)
        .header("User-Agent", "renzora-ai-chat/0.1")
        .call()
        .map_err(|e| format!("{e}"))?;
    if res.status().as_u16() >= 400 {
        return Err(format!("HTTP {}", res.status()));
    }
    res.body_mut()
        .read_to_string()
        .map_err(|e| format!("unreadable body: {e}"))
}

/// Truncate on a char boundary and say so — a silently cut page is another
/// way to make the model fill gaps with guesses.
fn clamp_text(mut text: String, budget: usize) -> String {
    if text.len() > budget {
        let mut cut = budget;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
        text.push_str("\n[…page truncated…]");
    }
    text
}

/// Same-site links from `html` ranked by how much their URL relates to the
/// prompt; top `max`, query-relevant only.
fn related_links(html: &str, base: &str, query: &str, max: usize) -> Vec<String> {
    let origin = {
        let Some(scheme_end) = base.find("://") else { return Vec::new() };
        match base[scheme_end + 3..].find('/') {
            Some(host_end) => &base[..scheme_end + 3 + host_end],
            None => base,
        }
    };
    let terms = query_terms(query);
    if terms.is_empty() {
        return Vec::new();
    }
    let mut seen = std::collections::HashSet::new();
    let mut scored: Vec<(usize, String)> = Vec::new();
    let mut rest = html;
    while let Some(idx) = rest.find("href=") {
        rest = &rest[idx + 5..];
        let Some(quote) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            continue;
        };
        let Some(end) = rest[1..].find(quote) else { continue };
        let href = &rest[1..1 + end];
        rest = &rest[1 + end..];

        let url = if href.starts_with(origin) {
            href.to_string()
        } else if href.starts_with('/') && !href.starts_with("//") {
            format!("{origin}{href}")
        } else {
            continue; // off-site, fragment, mailto, relative — skip
        };
        let url = url.split('#').next().unwrap_or(&url).to_string();
        if url == base || !seen.insert(url.clone()) {
            continue;
        }
        let lc = url.to_lowercase();
        let score = terms.iter().filter(|t| lc.contains(*t)).count();
        if score > 0 {
            scored.push((score, url));
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().take(max).map(|(_, u)| u).collect()
}

// ── Local docs retrieval ─────────────────────────────────────────────────────

/// Pull the most relevant manual excerpts for `query` from the docs folder.
/// Reads the tree fresh per request — the manual is a couple of MB, and
/// statelessness beats cache invalidation here.
fn retrieve_docs(path: &str, query: &str) -> Vec<(String, String)> {
    let Some(root) = resolve_docs_root(path) else {
        return Vec::new();
    };
    let terms = query_terms(query);
    if terms.is_empty() {
        return Vec::new();
    }

    // Collect heading-bounded chunks from every markdown file.
    struct Chunk {
        label: String,
        text: String,
        score: f32,
    }
    let mut chunks: Vec<Chunk> = Vec::new();
    let mut dirs = vec![root.clone()];
    let mut files_seen = 0usize;
    while let Some(dir) = dirs.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let p = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if p.is_dir() {
                if !name.starts_with('.') && name != "node_modules" {
                    dirs.push(p);
                }
                continue;
            }
            if files_seen > 600 {
                break;
            }
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !matches!(ext, "md" | "mdx" | "markdown" | "txt") {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&p) else { continue };
            files_seen += 1;
            let rel = p
                .strip_prefix(&root)
                .unwrap_or(&p)
                .display()
                .to_string();

            let mut heading = String::new();
            let mut cur = String::new();
            let flush = |heading: &str, cur: &mut String, chunks: &mut Vec<Chunk>| {
                let text = cur.trim();
                if text.len() > 40 {
                    let text_lc = text.to_lowercase();
                    let heading_lc = heading.to_lowercase();
                    let path_lc = rel.to_lowercase();
                    let mut hits = 0usize;
                    let mut matched_terms = 0usize;
                    for t in &terms {
                        let mut term_hits = 0usize;
                        for v in term_variants(t) {
                            term_hits = term_hits.max(text_lc.matches(v.as_str()).count().min(4));
                            if heading_lc.contains(v.as_str()) {
                                term_hits += 4;
                            }
                            // Reference files are named after their topic
                            // (api/scripting.md, scripting/lua.md) — a path
                            // match is a strong relevance signal.
                            if path_lc.contains(v.as_str()) {
                                term_hits += 3;
                            }
                        }
                        if term_hits > 0 {
                            matched_terms += 1;
                        }
                        hits += term_hits;
                    }
                    // Distinct-term coverage dominates raw repetition.
                    let hits = hits * matched_terms * matched_terms;
                    if hits > 0 {
                        chunks.push(Chunk {
                            label: if heading.is_empty() {
                                rel.clone()
                            } else {
                                format!("{rel} § {heading}")
                            },
                            score: hits as f32 / (text.len() as f32).sqrt(),
                            text: text.to_string(),
                        });
                    }
                }
                cur.clear();
            };
            let mut in_fence = false;
            for line in content.lines() {
                if line.trim_start().starts_with("```") {
                    in_fence = !in_fence;
                }
                if !in_fence {
                    if let Some(h) = line.strip_prefix('#') {
                        flush(&heading, &mut cur, &mut chunks);
                        heading = h.trim_start_matches('#').trim().to_string();
                    }
                }
                cur.push_str(line);
                cur.push('\n');
                if cur.len() > 2200 && !in_fence {
                    flush(&heading, &mut cur, &mut chunks);
                }
            }
            flush(&heading, &mut cur, &mut chunks);
        }
    }

    chunks.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut out = Vec::new();
    let mut budget = DOCS_CHAR_BUDGET;
    for chunk in chunks.into_iter().take(5) {
        if chunk.text.len() > budget {
            break;
        }
        budget -= chunk.text.len();
        out.push((chunk.label, chunk.text));
    }
    out
}

/// Resolve the docs root, descending into the right version folder: honors
/// `_versions.json` (`status: "current"`, else `default`), otherwise picks
/// the newest version-looking subfolder (natural sort, so alpha10 > alpha9),
/// otherwise uses the folder as-is. Relative paths resolve against the
/// editor's working directory.
fn resolve_docs_root(path: &str) -> Option<std::path::PathBuf> {
    let p = std::path::PathBuf::from(path);
    let p = if p.is_absolute() {
        p
    } else {
        std::env::current_dir().ok()?.join(p)
    };
    if !p.is_dir() {
        return None;
    }
    if let Ok(manifest) = std::fs::read_to_string(p.join("_versions.json")) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&manifest) {
            let current = v["versions"]
                .as_array()
                .and_then(|a| {
                    a.iter()
                        .find(|e| e["status"].as_str() == Some("current"))
                        .and_then(|e| e["id"].as_str())
                })
                .or_else(|| v["default"].as_str());
            if let Some(id) = current {
                let vp = p.join(id);
                if vp.is_dir() {
                    return Some(vp);
                }
            }
        }
    }
    let mut versions: Vec<String> = std::fs::read_dir(&p)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| {
            !n.starts_with('.')
                && n.chars().any(|c| c.is_ascii_digit())
                && (n.starts_with('r') || n.starts_with('v') || n.contains("alpha") || n.contains("beta"))
        })
        .collect();
    if versions.is_empty() {
        return Some(p);
    }
    versions.sort_by(|a, b| natural_cmp(a, b));
    versions.last().map(|v| p.join(v))
}

/// Compare strings with digit runs compared numerically (`alpha10 > alpha9`).
fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let (ab, bb) = (a.as_bytes(), b.as_bytes());
    let (mut i, mut j) = (0, 0);
    while i < ab.len() && j < bb.len() {
        if ab[i].is_ascii_digit() && bb[j].is_ascii_digit() {
            let (mut x, mut y) = (0u64, 0u64);
            while i < ab.len() && ab[i].is_ascii_digit() {
                x = x * 10 + (ab[i] - b'0') as u64;
                i += 1;
            }
            while j < bb.len() && bb[j].is_ascii_digit() {
                y = y * 10 + (bb[j] - b'0') as u64;
                j += 1;
            }
            match x.cmp(&y) {
                std::cmp::Ordering::Equal => {}
                ord => return ord,
            }
        } else {
            match ab[i].cmp(&bb[j]) {
                std::cmp::Ordering::Equal => {
                    i += 1;
                    j += 1;
                }
                ord => return ord,
            }
        }
    }
    a.len().cmp(&b.len())
}

/// Stemmed alternatives for a term, so "rotates"/"rotating" match "rotate".
fn term_variants(term: &str) -> Vec<String> {
    let mut out = vec![term.to_string()];
    for suffix in ["ing", "es", "ed", "s"] {
        if let Some(stem) = term.strip_suffix(suffix) {
            if stem.len() >= 3 {
                out.push(stem.to_string());
                break;
            }
        }
    }
    out
}

/// Meaningful query words for scoring docs chunks and crawl links.
fn query_terms(query: &str) -> Vec<String> {
    const STOP: &[&str] = &[
        "the", "and", "for", "with", "what", "how", "this", "that", "you", "your", "can",
        "are", "does", "about", "from", "into", "look", "like", "need", "want", "please",
        "tell", "scan", "website", "page", "http", "https", "com", "docs", "renzora",
    ];
    let mut out: Vec<String> = Vec::new();
    for word in query
        .to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
    {
        if word.len() >= 3 && !STOP.contains(&word) && !out.iter().any(|w| w == word) {
            out.push(word.to_string());
        }
    }
    out
}

/// Crude HTML → text: drops tags plus `<script>`/`<style>` contents, turns
/// block-element boundaries into newlines, decodes common entities, and
/// collapses blank runs. Not a real parser — good enough for docs pages.
fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 4);
    let mut rest = html;
    let mut skip_until: Option<&str> = None;
    while let Some(open) = rest.find('<') {
        if skip_until.is_none() {
            out.push_str(&rest[..open]);
        }
        rest = &rest[open..];
        let Some(close) = rest.find('>') else { break };
        let tag = rest[1..close].trim_start_matches('/').to_ascii_lowercase();
        let tag_name: String = tag.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
        if let Some(until) = skip_until {
            if rest.starts_with("</") && tag_name == until {
                skip_until = None;
            }
        } else if tag_name == "script" || tag_name == "style" {
            skip_until = Some(if tag_name == "script" { "script" } else { "style" });
        } else if matches!(
            tag_name.as_str(),
            "p" | "br" | "div" | "li" | "tr" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "section" | "article" | "pre"
        ) {
            out.push('\n');
        }
        rest = &rest[close + 1..];
    }
    if skip_until.is_none() {
        out.push_str(rest);
    }
    let decoded = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
    // Collapse whitespace: trim lines, drop runs of blank lines.
    let mut text = String::with_capacity(decoded.len());
    let mut blank_run = 0;
    for line in decoded.lines() {
        let line = line.trim();
        if line.is_empty() {
            blank_run += 1;
            if blank_run > 1 {
                continue;
            }
        } else {
            blank_run = 0;
        }
        text.push_str(line);
        text.push('\n');
    }
    text.trim().to_string()
}

/// Attach the protocol's auth headers. Anthropic uses `x-api-key` + a
/// pinned `anthropic-version`; everything else is a Bearer token.
fn authorize<B>(
    req: ureq::RequestBuilder<B>,
    protocol: Protocol,
    key: &str,
) -> ureq::RequestBuilder<B> {
    match protocol {
        Protocol::Anthropic => req
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01"),
        _ if !key.is_empty() => req.header("Authorization", &format!("Bearer {key}")),
        _ => req,
    }
}

/// An agent that returns 4xx/5xx responses instead of erroring, so error
/// bodies (which carry the useful message) stay readable.
fn agent() -> ureq::Agent {
    ureq::config::Config::builder()
        .http_status_as_error(false)
        .build()
        .new_agent()
}

/// Extract the human-readable message from an API error body:
/// Ollama uses `{"error": "..."}`; OpenAI-compatible servers use
/// `{"error": {"message": "..."}}`. Falls back to the (truncated) raw body.
fn api_error_message(body: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(msg) = v["error"].as_str() {
            return msg.to_string();
        }
        if let Some(msg) = v["error"]["message"].as_str() {
            return msg.to_string();
        }
    }
    let trimmed = body.trim();
    if trimmed.is_empty() {
        "(no error body)".to_string()
    } else {
        trimmed.chars().take(300).collect()
    }
}

renzora::add!(AiChatPlugin, Editor);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_extracted_and_trimmed() {
        let urls = extract_urls(
            "look at https://renzora.com/docs/r1-alpha5, then (http://localhost:8080/x) please",
        );
        assert_eq!(
            urls,
            vec![
                "https://renzora.com/docs/r1-alpha5".to_string(),
                "http://localhost:8080/x".to_string(),
            ]
        );
        assert!(extract_urls("no links here").is_empty());
    }

    #[test]
    fn natural_sort_orders_versions() {
        let mut v = vec!["r1-alpha10", "r1-alpha5", "r1-alpha9"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, vec!["r1-alpha5", "r1-alpha9", "r1-alpha10"]);
    }

    #[test]
    fn related_links_same_site_and_scored() {
        let html = r#"<a href="/docs/r1/scripting/lua">Lua</a>
            <a href="https://renzora.com/docs/r1/scripting/rhai">Rhai</a>
            <a href="https://other.site/scripting">x</a>
            <a href="/pricing">pricing</a>"#;
        let links = related_links(html, "https://renzora.com/docs/r1", "lua scripting", 3);
        assert!(links.contains(&"https://renzora.com/docs/r1/scripting/lua".to_string()));
        assert!(links.contains(&"https://renzora.com/docs/r1/scripting/rhai".to_string()));
        assert!(!links.iter().any(|l| l.contains("other.site") || l.contains("pricing")));
    }

    #[test]
    fn docs_retrieval_finds_manual_sections() {
        // The engine repo ships the manual under docs/<version>/ with a
        // _versions.json marking the current version.
        let docs = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs");
        let root = resolve_docs_root(docs).expect("docs root resolves");
        assert!(root.ends_with("r1-alpha5"), "honors _versions.json current: {root:?}");
        let hits = retrieve_docs(docs, "how do lua lifecycle hooks work in scripts");
        assert!(!hits.is_empty(), "lua docs should match");
        assert!(hits.iter().any(|(label, _)| label.contains("scripting")), "{hits:?}");
    }

    #[test]
    fn code_gen_query_retrieves_api_reference() {
        // The exact failure mode seen in the wild: "rotates" must stem to
        // match the api reference's `rotate(x, y, z)` table.
        let docs = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs");
        let hits = retrieve_docs(docs, "write me a lua script that rotates a cube");
        assert!(
            hits.iter().any(|(label, text)| label.contains("api/scripting")
                || text.contains("rotate(")),
            "API reference should be retrieved: {:?}",
            hits.iter().map(|(l, _)| l).collect::<Vec<_>>()
        );
    }

    #[test]
    fn html_reduces_to_text() {
        let html = r#"<html><head><style>.x{color:red}</style>
            <script>var a = "<p>not text</p>";</script></head>
            <body><h1>Title</h1><p>Hello &amp; welcome.</p>
            <ul><li>one</li><li>two</li></ul></body></html>"#;
        let text = html_to_text(html);
        assert!(text.contains("Title"));
        assert!(text.contains("Hello & welcome."));
        assert!(text.contains("one"));
        assert!(!text.contains("color:red"));
        assert!(!text.contains("not text"));
    }
}
