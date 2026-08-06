//! The conversation, and the markup it renders to.
//!
//! Everything lives in one `Mutex` static because a plugin system must be
//! zero-sized — it is rebuilt from nothing on every call, so it can capture
//! nothing. That is the standard shape for plugin state, not a workaround.

use std::sync::Mutex;

/// Who said a thing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    /// Errors and notices. Rendered differently and never sent to the model.
    System,
}

impl Role {
    fn label(self) -> &'static str {
        match self {
            Role::User => "You",
            Role::Assistant => "AI",
            Role::System => "--",
        }
    }

    /// Ollama's role names. `System` never reaches this — it is filtered out of
    /// the request — but a total match is better than an `unreachable!` in a
    /// plugin, where a panic aborts rather than unwinding.
    fn wire(self) -> &'static str {
        match self {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
        }
    }
}

pub struct State {
    pub messages: Vec<(Role, String)>,
    /// The prompt box's live contents, mirrored here on every keystroke by the
    /// input's own panel action — the plugin cannot read a widget on demand, so
    /// it caches what it was last told.
    pub draft: String,
    /// Set while a reply is streaming. The Send button becomes Stop.
    pub streaming: bool,
    /// Partial NDJSON left over from the previous chunk. Chunks are
    /// transport-sized, so a line can and does span two of them.
    pub carry: String,
    /// Index into [`PRESETS`]. Persisted, and what decides the protocol.
    pub preset: usize,
    /// The server root, user-editable after picking a preset. The chat path
    /// comes from the preset and is not editable, matching the in-tree crate.
    pub base_url: String,
    pub model: String,
    pub docs_folder: Option<String>,
    /// Bearer token for a hosted provider. Empty for Ollama, which wants none.
    pub api_key: String,
    pub status: String,
    /// Set by anything that changes what should be on screen; cleared by the
    /// redraw. Without it the plugin would rebuild and re-send the whole markup
    /// every frame — the host would compare and discard it, but the string
    /// building is not free.
    pub dirty: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            draft: String::new(),
            streaming: false,
            carry: String::new(),
            // Ollama's default, which needs no key and no account. A hosted
            // provider is the same shape — one URL and one bearer token — but
            // asking for a key before the panel does anything at all is a poor
            // first run.
            preset: 0,
            base_url: PRESETS[0].base_url.to_string(),
            model: "llama3.2".to_string(),
            docs_folder: None,
            api_key: String::new(),
            status: "Ready".to_string(),
            dirty: true,
        }
    }
}

/// Which wire protocol a preset speaks.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// NDJSON, one object per line, no auth.
    Ollama,
    /// SSE, `x-api-key` + a pinned `anthropic-version`.
    Anthropic,
}

/// A provider: label, protocol, and the endpoint it defaults to.
///
/// Mirrors `crates/renzora_ai_chat`'s `PRESETS` — same labels, same base URLs,
/// same chat paths — so the shared config file means the same thing to both.
/// The base URL stays user-editable after selection; the path is fixed per
/// provider.
pub struct Preset {
    pub label: &'static str,
    pub protocol: Protocol,
    pub base_url: &'static str,
    pub chat_path: &'static str,
}

/// Deliberately shorter than the in-tree list. Every provider there beyond these
/// two speaks the OpenAI shape, which is a third `request_body` and a third
/// stream parser — offering a preset this plugin cannot actually talk to would
/// be worse than not listing it.
pub static PRESETS: &[Preset] = &[
    Preset {
        label: "Ollama (local)",
        protocol: Protocol::Ollama,
        base_url: "http://localhost:11434",
        chat_path: "/api/chat",
    },
    Preset {
        label: "Claude (Anthropic)",
        protocol: Protocol::Anthropic,
        base_url: "https://api.anthropic.com",
        chat_path: "/v1/messages",
    },
];

pub static STATE: Mutex<Option<State>> = Mutex::new(None);

/// Where the connection settings live, matching `crates/renzora_ai_chat` byte
/// for byte so the two read each other's file.
///
/// A plugin can do this itself because it keeps `std` — there is no config
/// service in the ABI, and for one file there does not need to be. What it
/// cannot do is *ask* where the engine keeps its config, so the convention is
/// duplicated here; a `renzora.config` domain over the reply channel is the
/// right fix if a second plugin ever needs it.
fn config_path() -> Option<std::path::PathBuf> {
    let base = if cfg!(windows) {
        std::path::PathBuf::from(std::env::var_os("APPDATA")?)
    } else {
        std::path::PathBuf::from(std::env::var_os("HOME")?).join(".config")
    };
    Some(base.join("renzora").join("ai_chat.json"))
}

/// Read one `"key": "value"` string out of the config.
///
/// The same not-a-JSON-parser reasoning as `extract_content`: four flat string
/// fields do not justify the crate's only dependency. Anything it cannot read is
/// left at its default, so a hand-edited or newer file degrades rather than
/// wiping the settings.
fn read_field(src: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let at = src.find(&needle)? + needle.len();
    let rest = &src[at..];
    let open = rest.find('"')?;
    let mut out = String::new();
    let mut chars = rest[open + 1..].chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(other) => out.push(other),
                None => break,
            },
            c => out.push(c),
        }
    }
    None
}

impl State {
    /// Load what was saved last time, if anything.
    pub fn load(&mut self) {
        let Some(text) = config_path().and_then(|p| std::fs::read_to_string(p).ok()) else {
            return;
        };
        if let Some(v) = read_field(&text, "base_url").filter(|v| !v.is_empty()) {
            self.base_url = v;
        }
        // `preset` is a number, so it needs its own reader rather than
        // `read_field`, which only understands quoted strings.
        if let Some(at) = text.find("\"preset\"") {
            let digits: String = text[at + 8..]
                .chars()
                .skip_while(|c| !c.is_ascii_digit())
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if let Ok(n) = digits.parse::<usize>() {
                self.preset = n.min(PRESETS.len() - 1);
            }
        }
        if let Some(v) = read_field(&text, "model").filter(|v| !v.is_empty()) {
            self.model = v;
        }
        if let Some(v) = read_field(&text, "api_key").filter(|v| !v.is_empty()) {
            self.api_key = v;
        }
        if let Some(v) = read_field(&text, "docs_path").filter(|v| !v.is_empty()) {
            self.docs_folder = Some(v);
        }
    }

    /// Write the connection settings back.
    ///
    /// Called when a setting changes, not every frame — the transcript changes
    /// constantly while a reply streams and none of it belongs in the config.
    pub fn save(&self) {
        let Some(path) = config_path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // Written to match `crates/renzora_ai_chat`'s `AiChatConfig` field for
        // field, including `preset`, which this plugin does not use but must not
        // drop — the in-tree version reads the same file and would lose it.
        let body = format!(
            concat!(
                "{{\n",
                "  \"preset\": {},\n",
                "  \"base_url\": \"{}\",\n",
                "  \"api_key\": \"{}\",\n",
                "  \"docs_path\": \"{}\",\n",
                "  \"model\": \"{}\"\n",
                "}}\n",
            ),
            self.preset,
            json_escape(&self.base_url),
            json_escape(&self.api_key),
            json_escape(self.docs_folder.as_deref().unwrap_or("")),
            json_escape(&self.model),
        );
        let _ = std::fs::write(path, body);
    }
}

/// Run `f` against the state, creating it on first use.
///
/// Poison-tolerant: a panic while holding this lock has already been reported by
/// the host, and refusing to run afterwards would leave a dead panel with no way
/// back. The worst case is a half-updated conversation, not unsound memory.
pub fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(State::default))
}

impl State {
    /// Append to the last message if it is the assistant's, else start one.
    /// Streaming deltas arrive token by token and must not each become a row.
    pub fn push_delta(&mut self, delta: &str) {
        match self.messages.last_mut() {
            Some((Role::Assistant, text)) if self.streaming => text.push_str(delta),
            _ => self.messages.push((Role::Assistant, delta.to_string())),
        }
        self.dirty = true;
    }

    pub fn say(&mut self, role: Role, text: impl Into<String>) {
        self.messages.push((role, text.into()));
        self.dirty = true;
    }

    /// The request body. Hand-built rather than serialised, because a plugin has
    /// no serde and pulling one in for four fields would be the only dependency
    /// in the crate.
    ///
    /// `System` rows are the plugin's own notices — errors, "cancelled" — and
    /// are filtered out rather than sent, since the model did not say them and
    /// should not be told it did.
    pub fn preset(&self) -> &'static Preset {
        &PRESETS[self.preset.min(PRESETS.len() - 1)]
    }

    /// The full chat URL: the user's server root plus the preset's fixed path.
    ///
    /// Trailing slashes are trimmed so `https://host/` and `https://host` both
    /// work — a pasted URL has one about half the time.
    pub fn chat_url(&self) -> String {
        format!(
            "{}{}",
            self.base_url.trim_end_matches('/'),
            self.preset().chat_path
        )
    }

    /// The auth headers this provider wants. Empty for Ollama, which wants none.
    ///
    /// Anthropic is the reason header support exists: it takes `x-api-key` plus
    /// a pinned `anthropic-version`, not a bearer token, so "just send
    /// Authorization" would not have reached it.
    pub fn auth_headers(&self) -> renzora_plugin::http::HttpHeaders {
        use renzora_plugin::http::HttpHeaders;
        let h = HttpHeaders::new();
        if self.api_key.is_empty() {
            return h;
        }
        match self.preset().protocol {
            Protocol::Ollama => h,
            Protocol::Anthropic => h
                .add("x-api-key", &self.api_key)
                .add("anthropic-version", "2023-06-01"),
        }
    }

    pub fn request_body(&self) -> String {
        let mut b = String::from("{\"model\":\"");
        b.push_str(&json_escape(&self.model));
        b.push_str("\",\"stream\":true,\"messages\":[");
        let mut first = true;
        if let Some(folder) = &self.docs_folder {
            b.push_str("{\"role\":\"system\",\"content\":\"");
            b.push_str(&json_escape(&format!(
                "Reference documentation lives at {folder}."
            )));
            b.push_str("\"}");
            first = false;
        }
        for (role, text) in self.messages.iter().filter(|(r, _)| *r != Role::System) {
            if !first {
                b.push(',');
            }
            first = false;
            b.push_str("{\"role\":\"");
            b.push_str(role.wire());
            b.push_str("\",\"content\":\"");
            b.push_str(&json_escape(text));
            b.push_str("\"}");
        }
        b.push_str("]}");
        b
    }
}

/// Escape a string for a JSON double-quoted value.
///
/// Control characters below 0x20 have to go as `\u00XX` or the document is
/// invalid — a literal newline inside a JSON string is not legal, and a pasted
/// snippet is full of them.
pub fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Escape a string to sit inside a BSN `Text("…")` literal.
///
/// Separate from [`json_escape`] on purpose. They look alike and are not the
/// same grammar, and a single escaper shared between them would be a bug waiting
/// for the first message containing a brace: BSN newlines must not be literal,
/// because the parser would see the string end mid-line and the whole panel
/// would fail to parse — which shows up as a panel that silently stops updating.
pub fn bsn_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            '\t' => out.push_str("    "),
            c => out.push(c),
        }
    }
    out
}

/// A `Text` node at a sane size.
///
/// BSN's bare `Text("x")` inherits the base UI font size, which is a heading in
/// a dense panel — the first render came out roughly twice the size of every
/// other panel in the editor. `TextFont` has to be named explicitly because a
/// plugin cannot reach the editor's own text styles.
pub fn text(body: &str, size: f32) -> String {
    format!(
        "( Text(\"{}\") TextFont {{ font_size: {size} }} )",
        bsn_escape(body)
    )
}

/// A clickable button carrying a panel action.
///
/// `Node` and `Interaction` are spelled out rather than left to `Button`'s
/// `#[require(..)]`, because a reflected spawn does not apply required
/// components — a bare `Button` yields an entity with no `Node` (hence the
/// B0004 hierarchy warnings) and no `Interaction`, so it renders as loose text
/// and never dispatches.
pub fn button(label: &str, action: u32) -> String {
    format!(
        "( Node {{ padding: {{ left: Px(8.0), right: Px(8.0), top: Px(3.0), bottom: Px(3.0) }} }}          Button Interaction PanelActionId {{ action: {action} }}          Children [ {} ] )",
        text(label, 12.0)
    )
}

/// One transcript row, wrapped so long replies do not run off the panel.
///
/// Wrapping by hand because the row is a `Text` node built from a string, and
/// the plugin cannot reach into the layout to ask how wide the panel is. 88
/// columns is a guess that reads acceptably at the default panel width; it
/// breaks on whitespace so a URL stays clickable-looking rather than sliced.
fn wrap(text: &str, width: usize) -> String {
    let mut out = String::new();
    for (i, para) in text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let mut col = 0;
        for word in para.split(' ') {
            if col > 0 && col + word.chars().count() + 1 > width {
                out.push('\n');
                col = 0;
            } else if col > 0 {
                out.push(' ');
                col += 1;
            }
            out.push_str(word);
            col += word.chars().count();
        }
    }
    out
}

impl State {
    /// Render the whole panel.
    ///
    /// Rebuilt in full every time rather than patched, because
    /// `set_panel_content` replaces the markup wholesale — and because the host
    /// compares before it parses, so an unchanged panel costs a string compare.
    pub fn markup(&self) -> String {
        let mut m = String::from(
            "Node { flex_direction: Column, row_gap: Px(6.0), width: Percent(100.0) }\nChildren [\n",
        );

        m.push_str(&format!("    {},\n", text(&self.status, 12.0)));

        // Model + docs row.
        m.push_str("    ( Node { flex_direction: Row, column_gap: Px(6.0) }\n      Children [\n");
        m.push_str(&format!(
            "        {},\n",
            text(&format!("model: {}", self.model), 12.0)
        ));
        m.push_str(&format!(
            "        {},\n",
            button("Docs folder", crate::ACT_PICK)
        ));
        m.push_str("      ] ),\n");

        if let Some(folder) = &self.docs_folder {
            m.push_str(&format!("    {},\n", text(folder, 11.0)));
        }

        // Transcript.
        for (role, body) in &self.messages {
            m.push_str(&format!(
                "    {},\n",
                text(&format!("{}: {}", role.label(), wrap(body, 88)), 12.0)
            ));
        }

        // Prompt. The action id goes on `EmberInput`, whose `EmberTextInput`
        // child reports every keystroke back through it.
        m.push_str(&format!(
            "    ( EmberInput {{ placeholder: \"Type a message...\", value: \"{}\" }} \
             PanelActionId {{ action: {} }} ),\n",
            bsn_escape(&self.draft),
            crate::ACT_INPUT
        ));

        // Send / Stop. A `Button` rather than `EmberButtonWidget`: dispatch wants
        // the action id and the `Interaction` on ONE entity, and the ember widget
        // builds its clickable box as a child — so the id would sit on an entity
        // that never registers a press. This is the trap the API status page
        // calls out, and it costs a panel that looks right and does nothing.
        let (label, action) = if self.streaming {
            ("Stop", crate::ACT_STOP)
        } else {
            ("Send", crate::ACT_SEND)
        };
        m.push_str(&format!("    {},\n", button(label, action)));

        m.push_str("]\n");
        m
    }

    /// The Settings → Plugins section: everything that persists.
    ///
    /// Separate markup from [`markup`](Self::markup) but the same mechanism —
    /// a settings section is a panel that renders in the Settings overlay, so
    /// `set_panel_content` updates it under its own id exactly the same way.
    ///
    /// The connection settings live here rather than in the chat panel, matching
    /// where `crates/renzora_ai_chat` puts them: they are configured once and
    /// then never touched, and a panel you talk to every day should not carry
    /// three text boxes you set in the first minute.
    pub fn settings_markup(&self) -> String {
        let mut m = String::from(
            "Node { flex_direction: Column, row_gap: Px(8.0), width: Percent(100.0) }\nChildren [\n",
        );

        // Row order matches `crates/renzora_ai_chat`'s section exactly —
        // Provider, Server URL, API key, Docs folder — so the two are the same
        // panel to look at even though one is built imperatively and this one is
        // markup. Model sits under Provider here because this plugin cannot
        // fetch the model list (that needs a second request shape per protocol),
        // so it is typed rather than chosen.
        m.push_str(&format!("    {},\n", text("Provider", 12.0)));
        let labels: Vec<String> = PRESETS
            .iter()
            .map(|p| format!("\"{}\"", bsn_escape(p.label)))
            .collect();
        m.push_str(&format!(
            "    ( EmberDropdown {{ options: [{}], selected: {} }} \
             PanelActionId {{ action: {} }} ),\n",
            labels.join(", "),
            self.preset.min(PRESETS.len() - 1),
            crate::ACT_SET_PRESET
        ));

        m.push_str(&format!("    {},\n", text("Model", 12.0)));
        m.push_str(&format!(
            "    ( EmberInput {{ placeholder: \"llama3.2\", value: \"{}\" }} \
             PanelActionId {{ action: {} }} ),\n",
            bsn_escape(&self.model),
            crate::ACT_SET_MODEL
        ));

        m.push_str(&format!("    {},\n", text("Server URL", 12.0)));
        m.push_str(&format!(
            "    ( EmberInput {{ placeholder: \"{}\", value: \"{}\" }} \
             PanelActionId {{ action: {} }} ),\n",
            bsn_escape(PRESETS[self.preset.min(PRESETS.len() - 1)].base_url),
            bsn_escape(&self.base_url),
            crate::ACT_SET_URL
        ));

        m.push_str(&format!("    {},\n", text("API key", 12.0)));
        m.push_str(&format!(
            "    ( EmberInput {{ placeholder: \"optional\", value: \"{}\" }} \
             PanelActionId {{ action: {} }} ),\n",
            bsn_escape(&self.api_key),
            crate::ACT_SET_KEY
        ));

        m.push_str(&format!("    {},\n", text("Docs folder", 12.0)));
        m.push_str(&format!(
            "    ( Node {{ flex_direction: Row, column_gap: Px(6.0), align_items: Center }}\n      \
             Children [\n        {},\n        {},\n      ] ),\n",
            text(self.docs_folder.as_deref().unwrap_or("(none)"), 11.0),
            button("Browse...", crate::ACT_PICK)
        ));

        m.push_str("]\n");
        m
    }
}
