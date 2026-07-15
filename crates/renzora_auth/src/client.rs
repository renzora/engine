//! Shared blocking HTTP helpers for the renzora.com API client modules.
//!
//! All request helpers are native-only and designed to run on background
//! threads, matching the crate's existing `marketplace.rs` conventions.

/// Base URL for the Renzora API. Override with the `RENZORA_API_BASE`
/// environment variable to point the editor at a local/staging server.
pub fn api_base() -> &'static str {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::sync::OnceLock;
        static BASE: OnceLock<String> = OnceLock::new();
        BASE.get_or_init(|| {
            std::env::var("RENZORA_API_BASE")
                .ok()
                .map(|s| s.trim_end_matches('/').to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "https://renzora.com".to_string())
        })
    }
    #[cfg(target_arch = "wasm32")]
    {
        "https://renzora.com"
    }
}

/// Simple percent-encoding for query parameters.
pub(crate) fn urlencoded(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}

/// Shared agent: HTTP error statuses come back as responses (not opaque
/// `ureq` errors) so we can surface the server's actual {"error": ...} message.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn agent() -> &'static ureq::Agent {
    use std::sync::OnceLock;
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .http_status_as_error(false)
                .build(),
        )
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn read_json<T: serde::de::DeserializeOwned>(response: ureq::http::Response<ureq::Body>) -> Result<T, String> {
    let status = response.status();
    let body = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Failed to read response: {e}"))?;
    if !status.is_success() {
        // The API answers errors as {"error": "message"} — show the message.
        let msg = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(|e| e.to_string()))
            .unwrap_or_else(|| format!("HTTP {}", status.as_u16()));
        return Err(msg);
    }
    serde_json::from_str(&body).map_err(|e| format!("Failed to parse response: {e}"))
}

/// GET a JSON endpoint, optionally authenticated.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn get_json<T: serde::de::DeserializeOwned>(
    url: &str,
    token: Option<&str>,
) -> Result<T, String> {
    let mut req = agent().get(url);
    if let Some(t) = token {
        req = req.header("Authorization", &format!("Bearer {t}"));
    }
    let response = req.call().map_err(|e| format!("Request failed: {e}"))?;
    read_json(response)
}

/// POST a JSON body, optionally authenticated.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn post_json<T: serde::de::DeserializeOwned>(
    url: &str,
    body: &impl serde::Serialize,
    token: Option<&str>,
) -> Result<T, String> {
    let json = serde_json::to_string(body).map_err(|e| e.to_string())?;
    let mut req = agent().post(url).header("Content-Type", "application/json");
    if let Some(t) = token {
        req = req.header("Authorization", &format!("Bearer {t}"));
    }
    let response = req
        .send(json.as_bytes())
        .map_err(|e| format!("Request failed: {e}"))?;
    read_json(response)
}

/// PUT a JSON body, optionally authenticated.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn put_json<T: serde::de::DeserializeOwned>(
    url: &str,
    body: &impl serde::Serialize,
    token: Option<&str>,
) -> Result<T, String> {
    let json = serde_json::to_string(body).map_err(|e| e.to_string())?;
    let mut req = agent().put(url).header("Content-Type", "application/json");
    if let Some(t) = token {
        req = req.header("Authorization", &format!("Bearer {t}"));
    }
    let response = req
        .send(json.as_bytes())
        .map_err(|e| format!("Request failed: {e}"))?;
    read_json(response)
}

/// DELETE an endpoint, optionally authenticated.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn delete_json<T: serde::de::DeserializeOwned>(
    url: &str,
    token: Option<&str>,
) -> Result<T, String> {
    let mut req = agent().delete(url);
    if let Some(t) = token {
        req = req.header("Authorization", &format!("Bearer {t}"));
    }
    let response = req.call().map_err(|e| format!("Request failed: {e}"))?;
    read_json(response)
}

/// Extract the bearer token from a session, or fail like existing callers do.
pub(crate) fn require_token(session: &crate::session::AuthSession) -> Result<&str, String> {
    session.access_token.as_deref().ok_or_else(|| "Not signed in".to_string())
}


/// POST one file as multipart/form-data (field name + filename + bytes).
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn post_multipart<T: serde::de::DeserializeOwned>(
    url: &str,
    field: &str,
    filename: &str,
    content_type: &str,
    bytes: &[u8],
    token: &str,
) -> Result<T, String> {
    multipart("POST", url, field, filename, content_type, bytes, token)
}

/// PUT one file as multipart/form-data (avatar/banner uploads use PUT).
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn put_multipart<T: serde::de::DeserializeOwned>(
    url: &str,
    field: &str,
    filename: &str,
    content_type: &str,
    bytes: &[u8],
    token: &str,
) -> Result<T, String> {
    multipart("PUT", url, field, filename, content_type, bytes, token)
}

/// Shared single-file multipart body builder for POST/PUT.
#[cfg(not(target_arch = "wasm32"))]
fn multipart<T: serde::de::DeserializeOwned>(
    method: &str,
    url: &str,
    field: &str,
    filename: &str,
    content_type: &str,
    bytes: &[u8],
    token: &str,
) -> Result<T, String> {
    let boundary = format!("----renzora{:x}", std::process::id() as u64 ^ bytes.len() as u64 ^ 0x5eed);
    let mut body = Vec::with_capacity(bytes.len() + 512);
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"{field}\"; filename=\"{filename}\"\r\n").as_bytes(),
    );
    body.extend_from_slice(format!("Content-Type: {content_type}\r\n\r\n").as_bytes());
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let ct = format!("multipart/form-data; boundary={boundary}");
    let auth = format!("Bearer {token}");
    let req = match method {
        "PUT" => agent().put(url),
        _ => agent().post(url),
    };
    let response = req
        .header("Authorization", &auth)
        .header("Content-Type", &ct)
        .send(&body[..])
        .map_err(|e| format!("Upload failed: {e}"))?;
    read_json(response)
}

/// One file part of a multi-part form: the field name the server reads it under,
/// the original filename, its MIME type, and the raw bytes.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct FilePart<'a> {
    pub field: &'a str,
    pub filename: &'a str,
    pub content_type: &'a str,
    pub bytes: &'a [u8],
}

/// POST a `multipart/form-data` body mixing plain text fields with any number of
/// file parts. The marketplace/game upload endpoints need this: a `metadata`
/// JSON text field alongside the `file` (and optional `thumbnail`) binaries,
/// which the single-file [`post_multipart`] can't express.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn post_multipart_form<T: serde::de::DeserializeOwned>(
    url: &str,
    text_fields: &[(&str, &str)],
    files: &[FilePart],
    token: &str,
) -> Result<T, String> {
    // A boundary that can't appear in the payload: seed off the process id and
    // the total byte length so concurrent uploads don't collide.
    let total: usize = files.iter().map(|f| f.bytes.len()).sum();
    let boundary = format!("----renzora{:x}", std::process::id() as u64 ^ total as u64 ^ 0xf0_5eed);
    let mut body = Vec::with_capacity(total + 1024);

    for (name, value) in text_fields {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
        );
        body.extend_from_slice(value.as_bytes());
        body.extend_from_slice(b"\r\n");
    }
    for f in files {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\n",
                f.field, f.filename
            )
            .as_bytes(),
        );
        body.extend_from_slice(format!("Content-Type: {}\r\n\r\n", f.content_type).as_bytes());
        body.extend_from_slice(f.bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    let ct = format!("multipart/form-data; boundary={boundary}");
    let response = agent()
        .post(url)
        .header("Authorization", &format!("Bearer {token}"))
        .header("Content-Type", &ct)
        .send(&body[..])
        .map_err(|e| format!("Upload failed: {e}"))?;
    read_json(response)
}
