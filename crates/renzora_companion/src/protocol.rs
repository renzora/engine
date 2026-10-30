//! What the editor and its companion say to each other.
//!
//! One JSON object per line over a loopback TCP socket, the same framing the
//! live link uses with Play's game. Control traffic is small and rare, so a
//! readable format costs nothing and makes a stuck job easy to diagnose. The
//! profiler's frame stream is a different, binary channel; it does not go
//! through here.

use serde::{Deserialize, Serialize};

/// Bumped whenever a message changes shape.
///
/// The editor and the companion are the same executable, so in practice they
/// always agree. The number is for the case where they do not: a companion
/// left running by an editor that crashed mid-rebuild, met by the next one. It
/// is refused rather than half-understood.
pub const PROTOCOL_VERSION: u32 = 1;

/// Identifies one job for its whole life, chosen by the editor.
pub type JobId = u64;

/// One line on the wire.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    /// Companion to editor, always the first line. The token is the one the
    /// editor passed on the companion's command line, which is what proves the
    /// connection comes from the process the editor started and not from
    /// anything else on the machine that found the port.
    Hello { token: String, version: u32 },
    /// Editor to companion: run a job.
    Submit { id: JobId, job: Job },
    /// Companion to editor: how a job is getting on.
    Progress {
        id: JobId,
        /// 0..=1, or `None` while the total is unknown.
        fraction: Option<f32>,
        note: String,
    },
    /// Companion to editor: a job finished, one way or the other.
    Done { id: JobId, result: Result<String, String> },
    /// Editor to companion: bring its window to the front.
    Show,
    /// Editor to companion: stop a job if it is still queued or running. Its
    /// `Done` still arrives, as `Err("cancelled")`.
    Cancel { id: JobId },
    /// Editor to companion: what the editor is doing that work should keep out
    /// of the way of. GPU jobs wait while `playing`.
    EditorState { playing: bool },
}

/// Work the companion can do.
///
/// One variant per kind of job. Phase 0 has only [`Job::Echo`]; real work
/// (thumbnails, uploads, compiles) moves here from the editor one kind at a
/// time.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "job", rename_all = "snake_case")]
pub enum Job {
    /// Wait `millis`, then answer with `text`. The editor sends one as soon as
    /// the companion connects, which proves the whole round trip (submit, run
    /// on a worker, report back) and measures it.
    Echo { text: String, millis: u64 },
    /// Work an editor crate queued through `renzora::CompanionJobs`: a handler
    /// `kind` and its JSON arguments. One variant for every kind, so a crate
    /// adding a job does not change this protocol.
    Run { kind: String, payload: String, target: String },
}

impl Job {
    /// A short name for the Console and the companion's job list.
    pub fn label(&self) -> &'static str {
        match self {
            Job::Echo { .. } => "Echo",
            Job::Run { kind, .. } => match kind.as_str() {
                "texture_thumbnail" => "Texture thumbnail",
                "model_thumbnail" => "Model thumbnail",
                "rust_script" => "Rust script",
                "rvg_bake" => "Virtual geometry",
                "import" | "model_convert" => "Import",
                "model_build" => "Model file",
                "export" => "Export",
                "upload" => "Upload",
                "plugin_build" => "Plugin build",
                _ => "Job",
            },
        }
    }

    /// What the job works on, for lists: a file, usually.
    pub fn target(&self) -> String {
        match self {
            Job::Echo { text, .. } => text.clone(),
            Job::Run { target, .. } => target.clone(),
        }
    }

    /// Higher runs first. Work someone is waiting to see beats background
    /// filling-in.
    pub fn priority(&self) -> u8 {
        match self {
            Job::Echo { .. } => 100,
            Job::Run { kind, .. } => match kind.as_str() {
                // Someone just saved and is waiting to run it.
                "rust_script" => 90,
                "export" | "upload" | "import" | "model_convert" | "model_build" => 70,
                "plugin_build" => 60,
                "texture_thumbnail" | "model_thumbnail" => 40,
                "rvg_bake" => 30,
                _ => 50,
            },
        }
    }

    /// Whether it renders, and so waits while a game is playing as well as
    /// while a profile records.
    pub fn uses_gpu(&self) -> bool {
        matches!(self, Job::Run { kind, .. } if kind == "model_thumbnail" || kind == "material_thumbnail")
    }

    /// Whether it runs on the companion's main thread rather than a worker:
    /// a job that renders needs the `World` and the render app, which live
    /// there. See `app::render`.
    pub fn runs_on_main(&self) -> bool {
        self.uses_gpu()
    }
}

/// `message` as one line, newline included.
pub fn encode(message: &Message) -> String {
    // Serializing an enum of strings and numbers cannot fail.
    let mut line = serde_json::to_string(message).unwrap_or_default();
    line.push('\n');
    line
}

/// One received line, or `None` if it is not a message this version knows.
pub fn decode(line: &str) -> Option<Message> {
    serde_json::from_str(line.trim_end()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_survives_the_wire() {
        let messages = [
            Message::Hello {
                token: "abc".into(),
                version: PROTOCOL_VERSION,
            },
            Message::Submit {
                id: 7,
                job: Job::Echo {
                    text: "ping".into(),
                    millis: 0,
                },
            },
            Message::Progress {
                id: 7,
                fraction: Some(0.5),
                note: "halfway".into(),
            },
            Message::Progress {
                id: 7,
                fraction: None,
                note: String::new(),
            },
            Message::Done {
                id: 7,
                result: Ok("ping".into()),
            },
            Message::Done {
                id: 8,
                result: Err("no disk".into()),
            },
            Message::Show,
            Message::Cancel { id: 3 },
            Message::EditorState { playing: true },
        ];
        for message in messages {
            let line = encode(&message);
            assert!(line.ends_with('\n') && !line[..line.len() - 1].contains('\n'));
            assert_eq!(decode(&line), Some(message));
        }
    }

    /// A line from a different version, or plain garbage, is dropped rather than
    /// misread.
    #[test]
    fn an_unknown_line_is_ignored() {
        assert_eq!(decode("{\"type\":\"teleport\"}"), None);
        assert_eq!(decode("not json"), None);
    }
}
