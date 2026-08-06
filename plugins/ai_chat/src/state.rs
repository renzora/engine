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
    pub endpoint: String,
    pub model: String,
    pub docs_folder: Option<String>,
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
            endpoint: "http://localhost:11434/api/chat".to_string(),
            model: "llama3.2".to_string(),
            docs_folder: None,
            status: "Ready".to_string(),
            dirty: true,
        }
    }
}

pub static STATE: Mutex<Option<State>> = Mutex::new(None);

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

        m.push_str(&format!("    Text(\"{}\"),\n", bsn_escape(&self.status)));

        // Model + docs row.
        m.push_str("    ( Node { flex_direction: Row, column_gap: Px(6.0) }\n      Children [\n");
        m.push_str(&format!(
            "        Text(\"{}\"),\n",
            bsn_escape(&format!("model: {}", self.model))
        ));
        m.push_str(&format!(
            "        ( Button PanelActionId {{ action: {} }} Children [ Text(\"Docs folder\") ] ),\n",
            crate::ACT_PICK
        ));
        m.push_str("      ] ),\n");

        if let Some(folder) = &self.docs_folder {
            m.push_str(&format!("    Text(\"{}\"),\n", bsn_escape(folder)));
        }

        // Transcript.
        for (role, text) in &self.messages {
            m.push_str(&format!(
                "    Text(\"{}\"),\n",
                bsn_escape(&format!("{}: {}", role.label(), wrap(text, 88)))
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
        m.push_str(&format!(
            "    ( Button PanelActionId {{ action: {action} }} Children [ Text(\"{label}\") ] ),\n"
        ));

        m.push_str("]\n");
        m
    }
}
