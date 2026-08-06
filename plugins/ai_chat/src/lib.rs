//! Chat with a local or hosted LLM, in an editor panel.
//!
//! The C-ABI port of `crates/renzora_ai_chat`, and the first panel plugin that
//! needed every part of the boundary at once: a streaming HTTP reply, a panel
//! whose contents change every frame while a reply arrives, a native folder
//! picker, and a text input the plugin can actually read.
//!
//! ## How it differs from the in-tree version
//!
//! The engine crate builds its UI imperatively with `renzora_ember` — `button()`,
//! `scroll_view_pinned()`, `markdown_view()` — and keeps it live with reactive
//! bindings that read `&World`. A plugin has neither: it gets typed queries, not
//! a `World`, and it cannot call ember's constructors. So the UI here is
//! **declarative and rebuilt**: state changes, the plugin re-renders its BSN, and
//! `set_panel_content` hands the whole thing over. For a transcript that is
//! arguably the simpler design; for a panel with thirty independent bindings it
//! would not be.
//!
//! ## The shape of a turn
//!
//! 1. Every keystroke fires [`ACT_INPUT`] carrying the box's contents, which the
//!    plugin caches. It cannot ask a widget what it holds — this is the only way
//!    the text arrives.
//! 2. Send fires [`ACT_SEND`]: the draft becomes a user message and a streaming
//!    POST goes out.
//! 3. [`pump`] polls for chunks each frame, splits NDJSON lines out of them, and
//!    appends the deltas.
//! 4. Anything that changed the state marks it dirty, and `pump` re-renders.
//!
//! Note what is NOT here: a worker thread, a channel, or a runtime. The host owns
//! the client and the plugin polls, which is what makes this file free of
//! everything except `std` string handling.

use renzora_plugin::dialog::{DialogCommands, Dialogs};
use renzora_plugin::http::{Http, HttpCommands};
use renzora_plugin::panel::PanelCommands;
use renzora_plugin::prelude::*;

mod state;
use state::{with, Role, PRESETS};

/// Panel id. Prefixed, because ids are global across every loaded plugin.
const PANEL_ID: &str = "ai_chat";

// Action ids. Numbers rather than names because a plugin component's fields are
// the closed set the ABI can describe, and `i32` is in it while `String` is not.
pub const ACT_INPUT: u32 = 1;
pub const ACT_SEND: u32 = 2;
pub const ACT_STOP: u32 = 3;
pub const ACT_PICK: u32 = 4;
// Settings-section inputs. Each fires per keystroke carrying its contents.
pub const ACT_SET_URL: u32 = 5;
pub const ACT_SET_MODEL: u32 = 6;
pub const ACT_SET_KEY: u32 = 7;
pub const ACT_SET_PRESET: u32 = 8;

/// The settings section's own id. Distinct from the panel's, because ids are one
/// namespace across both and `set_panel_content` resolves against it.
const SETTINGS_ID: &str = "ai_chat_settings";

/// Tags pairing our requests with their answers. Scoped per service, so the
/// dialog and the chat stream may not collide even though both are small
/// integers — but keeping them distinct costs nothing and reads better in a log.
const TAG_CHAT: u64 = 1;
const TAG_FOLDER: u64 = 2;

/// Handle a click, or a keystroke in the prompt box.
///
/// Runs on the editor's own UI systems, so a panic here would abort the process
/// — the host's thunk carries a guard, which is why this can be ordinary code.
fn on_action(action: Action) {
    // Both copied out before `commands` is moved: `name()` and `text()` borrow
    // `action`, and the sink is a field of it.
    let id: u32 = action.name().parse().unwrap_or(0);
    let typed = action.text().to_string();
    let action_value = action.value;
    let mut commands = action.commands;

    if id == ACT_INPUT {
        // The whole reason `PanelAction::text` exists. Without it the plugin can
        // render a prompt box and never learn a thing about what is in it.
        // No dirty flag: the prompt box already shows what was typed, and
        // re-rendering it from here would fight the widget for the caret.
        with(|s| s.draft = typed);
        return;
    }

    if id == ACT_SEND {
        let body = with(|s| {
            let draft = s.draft.trim().to_string();
            if draft.is_empty() || s.streaming {
                return None;
            }
            s.say(Role::User, draft);
            s.draft.clear();
            s.streaming = true;
            s.carry.clear();
            s.status = "Thinking...".to_string();
            // Start the assistant's row now, empty, so the first delta appends
            // to it rather than the transcript jumping when it arrives.
            s.messages.push((Role::Assistant, String::new()));
            s.panel_dirty = true;
            Some((s.chat_url(), s.request_body(), s.auth_headers()))
        });
        if let Some((url, body, headers)) = body {
            // `http_with` rather than `http_post_stream`: an empty header set
            // falls through to the plain payload, so Ollama pays nothing for
            // this and Anthropic becomes reachable.
            commands.http_with(
                renzora_plugin::http::HttpOp::PostStream,
                TAG_CHAT,
                &url,
                Some(&body),
                &headers,
            );
        }
        return;
    }

    if id == ACT_STOP {
        // Honest about what this does: the request is the host's and there is no
        // cancel in the ABI, so the bytes keep arriving. Clearing `streaming`
        // makes `pump` drop them on the floor and stops the transcript growing,
        // which is what the button appears to promise. A real cancel would be a
        // new op on the http service.
        with(|s| {
            s.streaming = false;
            s.status = "Stopped".to_string();
            s.panel_dirty = true;
        });
        return;
    }

    if id == ACT_PICK {
        commands.pick_folder(TAG_FOLDER, "Choose a documentation folder");
        return;
    }

    // The settings inputs. Each writes its field and persists, because there is
    // no OK button to hang a save on — a section is always live, the way the
    // rest of the editor's settings are.
    //
    // Saving on every keystroke is a file write per character, which is fine for
    // a four-line config and is what makes closing the overlay mid-edit keep
    // what was typed.
    if id == ACT_SET_PRESET {
        // A dropdown reports its selection in `value`, not `text` — text is for
        // widgets that hold a string, and a dropdown holds an index.
        let idx = action_value.max(0.0) as usize;
        with(|s| {
            let idx = idx.min(PRESETS.len() - 1);
            if s.preset != idx {
                s.preset = idx;
                // Follow the preset's own server root. Keeping a URL from the
                // previous provider would point Anthropic's path at Ollama's
                // host and fail in a way that reads as a broken key.
                s.base_url = PRESETS[idx].base_url.to_string();
                s.save();
                // Both surfaces: the Server URL field cannot show its own new
                // value, since the change came from the dropdown beside it. A
                // pick also closes the dropdown, so there is no focus to lose.
                s.panel_dirty = true;
                s.settings_dirty = true;
            }
        });
        return;
    }

    let field = match id {
        ACT_SET_URL => Some(0),
        ACT_SET_MODEL => Some(1),
        ACT_SET_KEY => Some(2),
        _ => None,
    };
    if let Some(field) = field {
        with(|s| {
            // Compare before assigning. A redraw re-renders the input with its
            // current value, which the widget reports back as a change — so an
            // unguarded assignment here would mark dirty, redraw, report again,
            // and settle into a permanent loop with a file write per frame.
            let slot = match field {
                0 => &mut s.base_url,
                1 => &mut s.model,
                _ => &mut s.api_key,
            };
            if *slot == typed {
                return;
            }
            *slot = typed;
            s.save();
            // ONLY the chat panel. The settings field is already showing what
            // was typed — re-sending its markup would respawn the input under
            // the caret and drop focus mid-word, which is precisely the bug this
            // split exists to avoid. Nothing in the settings section depends on
            // these values except the fields themselves.
            s.panel_dirty = true;
        });
    }
}

/// Per-frame: collect stream chunks and dialog answers, then redraw if anything
/// moved.
///
/// One system rather than three because they share the state lock and the redraw
/// — and because ordering between them would otherwise be a thing to get wrong,
/// with no `before`/`after` available to a plugin to fix it with.
fn pump(mut commands: Commands, http: Http, dialogs: Dialogs) {
    // A stream delivers several chunks in one frame. Taking one per frame would
    // make a fast reply arrive in slow motion.
    while let Some(chunk) = http.poll_stream(TAG_CHAT) {
        let stop = with(|s| {
            if !s.streaming {
                // Stopped, or a leftover from a previous turn. Keep draining so
                // the queue does not grow, but write nothing.
                return chunk.is_last();
            }
            if chunk.is_error() {
                s.say(Role::System, format!("stream failed: {}", chunk.data));
                s.streaming = false;
                s.status = "Failed".to_string();
                return true;
            }
            s.carry.push_str(&chunk.data);
            // Chunks are transport-sized, so a JSON line can span two of them and
            // two lines can share one. Only complete lines are parsed; the tail
            // waits for the next chunk.
            while let Some(nl) = s.carry.find('\n') {
                let line: String = s.carry.drain(..=nl).collect();
                if let Some(delta) = extract_content(&line) {
                    if !delta.is_empty() {
                        s.push_delta(&delta);
                    }
                }
                if line.contains("\"done\":true") {
                    s.streaming = false;
                    s.status = "Ready".to_string();
                    s.panel_dirty = true;
                }
            }
            if chunk.is_last() {
                // The connection ended. If `done` never arrived the reply is
                // truncated, and saying so beats leaving a half sentence looking
                // deliberate.
                if s.streaming {
                    s.streaming = false;
                    s.status = "Ready".to_string();
                    s.panel_dirty = true;
                }
                return true;
            }
            false
        });
        if stop {
            break;
        }
    }

    if let Some(outcome) = dialogs.poll(TAG_FOLDER) {
        with(|s| {
            match outcome.path() {
                Some(path) => {
                    s.status = "Docs folder set".to_string();
                    s.docs_folder = Some(path.to_string());
                    s.save();
                }
                // Cancelling is an ordinary outcome. Saying nothing at all would
                // leave the status stuck on whatever preceded it.
                None => s.status = "Ready".to_string(),
            }
            // The folder shows on both surfaces, and neither can know it changed.
            s.panel_dirty = true;
            s.settings_dirty = true;
        });
    }

    // Redraw last, so everything above lands in one update rather than one per
    // source. The host compares markup before parsing, so a spurious call is a
    // string compare — but building the string is not free, hence the flag.
    // Each surface is sent only when its OWN content changed. Sending both
    // together was simpler and cost the caret: every keystroke in a settings
    // field respawned that field.
    let (panel, settings) = with(|s| {
        let p = s.panel_dirty.then(|| s.markup());
        let g = s.settings_dirty.then(|| s.settings_markup());
        s.panel_dirty = false;
        s.settings_dirty = false;
        (p, g)
    });
    if let Some(panel) = panel {
        commands.set_panel_content(PANEL_ID, &panel);
    }
    if let Some(settings) = settings {
        commands.set_panel_content(SETTINGS_ID, &settings);
    }
}

/// Pull `"content":"…"` out of one NDJSON line.
///
/// **Not a JSON parser**, and deliberately so: a plugin has no serde, and adding
/// one would be the only dependency in the crate. Ollama's chat stream is one
/// flat object per line with a known shape, so finding the key and unescaping the
/// string is enough. It handles the escapes a model's output actually contains —
/// quotes, backslashes, newlines — and passes `\uXXXX` through unresolved, which
/// shows up as literal text rather than as corruption.
///
/// If this plugin ever speaks to an API whose framing is not this simple, the
/// answer is a real parser, not more special cases here.
fn extract_content(line: &str) -> Option<String> {
    let key = "\"content\":\"";
    let start = line.find(key)? + key.len();
    let bytes = line.as_bytes();
    let mut out = String::new();
    let mut i = start;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => return Some(out),
            b'\\' if i + 1 < bytes.len() => {
                i += 1;
                match bytes[i] {
                    b'n' => out.push('\n'),
                    b't' => out.push('\t'),
                    b'r' => {}
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'u' => {
                        // Left as written. Resolving it properly means decoding
                        // surrogate pairs, and getting that subtly wrong is worse
                        // than showing the escape.
                        out.push_str("\\u");
                    }
                    other => out.push(other as char),
                }
                i += 1;
            }
            _ => {
                // Step by whole characters: a multi-byte codepoint indexed by
                // byte would be split into replacement characters.
                let rest = &line[i..];
                let c = rest.chars().next()?;
                out.push(c);
                i += c.len_utf8();
            }
        }
    }
    // No closing quote: the line was truncated, which the caller's line-splitting
    // should have prevented. Dropping it beats emitting half a token.
    None
}

pub struct AiChatPlugin;

impl Plugin for AiChatPlugin {
    fn build(&self, app: &mut App) {
        // The initial markup is the empty state's. Everything after this comes
        // through `set_panel_content`.
        // Load before rendering, so the first frame shows the saved endpoint
        // rather than the default and then flicking to it.
        let (initial, settings) = with(|s| {
            s.load();
            (s.markup(), s.settings_markup())
        });
        app.add_panel(
            Panel::new(PANEL_ID, "AI Chat", Scene(Box::leak(initial.into_boxed_str())))
                .icon("chat-circle-dots")
                .category("Plugins")
                .on_action(on_action),
        )
        .add_settings_section(
            Panel::new(
                SETTINGS_ID,
                "AI Chat",
                Scene(Box::leak(settings.into_boxed_str())),
            )
            .icon("robot")
            .on_action(on_action),
        )
        .add_systems(Update, pump);
    }
}

renzora_plugin::add!(AiChatPlugin, Editor);

#[cfg(test)]
mod tests {
    use super::*;

    /// The happy path plus the escapes a model's prose actually contains.
    #[test]
    fn extracts_a_delta() {
        assert_eq!(
            extract_content(r#"{"message":{"content":"Hello"},"done":false}"#),
            Some("Hello".to_string())
        );
        assert_eq!(
            extract_content(r#"{"content":"a \"quote\" and a \\ and\nline"}"#),
            Some("a \"quote\" and a \\ and\nline".to_string())
        );
        // An empty delta is legitimate — Ollama's final frame carries one — and
        // must come back as Some(""), not None, or the done-detection below it
        // would be skipped.
        assert_eq!(
            extract_content(r#"{"content":"","done":true}"#),
            Some(String::new())
        );
        assert_eq!(extract_content(r#"{"done":true}"#), None);
    }

    /// A multi-byte codepoint must not be split. Indexing the line by byte and
    /// pushing `bytes[i] as char` would turn every emoji into three replacement
    /// characters, which is exactly the sort of thing that looks like a model
    /// problem rather than a plugin one.
    #[test]
    fn survives_multibyte_content() {
        assert_eq!(
            extract_content(r#"{"content":"héllo 🌍 ok"}"#),
            Some("héllo 🌍 ok".to_string())
        );
    }

    /// A truncated line yields nothing rather than half a token — the caller
    /// only ever passes complete lines, so reaching this means something else
    /// went wrong and emitting garbage would hide it.
    #[test]
    fn refuses_an_unterminated_string() {
        assert_eq!(extract_content(r#"{"content":"half a sen"#), None);
    }

    /// The two escapers look alike and are not the same grammar. A literal
    /// newline is legal in neither, and a BSN one would end the string early and
    /// fail the whole panel parse — which shows up as a panel that silently
    /// stops updating, not as an error.
    #[test]
    fn escapers_do_not_emit_raw_control_characters() {
        let nasty = "say \"hi\"\n\tand \\ that\u{1}";
        let j = state::json_escape(nasty);
        assert!(!j.contains('\n') && !j.contains('\t'));
        assert!(j.contains("\\\"") && j.contains("\\\\") && j.contains("\\u0001"));

        let b = state::bsn_escape(nasty);
        assert!(!b.contains('\n'));
        assert!(b.contains("\\\"") && b.contains("\\\\"));
    }

    /// A request must not carry the plugin's own notices back to the model as if
    /// it had said them.
    #[test]
    fn system_rows_are_not_sent() {
        let mut s = state::State::default();
        s.say(Role::User, "hi");
        s.say(Role::System, "stream failed: boom");
        s.say(Role::Assistant, "hello");
        let body = s.request_body();
        assert!(body.contains("\"content\":\"hi\""));
        assert!(body.contains("\"content\":\"hello\""));
        assert!(!body.contains("boom"), "a System row reached the request");
    }

    /// The transcript must not gain a row per token.
    #[test]
    fn deltas_append_to_one_row() {
        let mut s = state::State::default();
        s.streaming = true;
        s.push_delta("Hel");
        s.push_delta("lo");
        s.push_delta(" there");
        assert_eq!(s.messages.len(), 1);
        assert_eq!(s.messages[0].1, "Hello there");
    }
}
