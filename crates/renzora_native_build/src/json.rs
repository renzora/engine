//! A minimal JSON reader, for cargo's machine output and the SDK manifest.
//!
//! Hand-rolled because this crate carries no dependencies on purpose (see its
//! `Cargo.toml`), and because the shapes it has to read are narrow: one-line
//! `compiler-artifact` messages from `cargo --message-format=json`, and the
//! small manifest `cargo renzora sdk` writes.
//!
//! Moved here from `xtask`, which had the only copy. It is now shared with
//! `renzora_plugin_build`, so the editor and a source checkout read cargo's
//! output through the same code rather than two hand-rolled parsers that agreed
//! by coincidence.

/// Pull a `"field": "value"` string out of JSON text.
///
/// Whitespace after the colon is optional because both shapes occur here:
/// cargo's `--message-format=json` is dense, while the SDK manifest is
/// pretty-printed for anyone reading it in a release.
pub fn string(text: &str, field: &str) -> Option<String> {
    let rest = after_key(text, field)?;
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// The text just past `"field":` and any following whitespace.
fn after_key<'a>(text: &'a str, field: &str) -> Option<&'a str> {
    let key = format!("\"{field}\":");
    let start = text.find(&key)? + key.len();
    Some(text[start..].trim_start())
}

/// Pull a `"field":["a","b"]` array of strings out of one JSON line.
///
/// It scans quotes and escapes rather than splitting on commas: these values are
/// absolute paths, Windows ones arrive with every separator escaped as `\\`, and
/// a naive split would also break on any path containing a comma.
pub fn string_array(text: &str, field: &str) -> Vec<String> {
    let Some(rest) = after_key(text, field).and_then(|r| r.strip_prefix('[')) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_str = false;
    let mut escaped = false;
    for c in rest.chars() {
        if !in_str {
            match c {
                '"' => in_str = true,
                ']' => break,
                _ => {}
            }
        } else if escaped {
            cur.push(c);
            escaped = false;
        } else {
            match c {
                '\\' => escaped = true,
                '"' => {
                    in_str = false;
                    out.push(std::mem::take(&mut cur));
                }
                _ => cur.push(c),
            }
        }
    }
    out
}

/// Every value of a repeated `"field":"value"` key across a whole document.
///
/// `string` finds the first and stops, which is right for a one-line message and
/// wrong for `cargo metadata`, where the question is "does the name `bevy_ecs`
/// appear ANYWHERE in the resolved graph". Scanning for every occurrence is what
/// makes that answerable without a real JSON tree.
pub fn all_strings(text: &str, field: &str) -> Vec<String> {
    let key = format!("\"{field}\":\"");
    let mut out = Vec::new();
    for part in text.split(&key).skip(1) {
        if let Some(end) = part.find('"') {
            out.push(part[..end].to_string());
        }
    }
    out
}
