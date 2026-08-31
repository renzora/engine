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

// `urlencoded` lived here for the search endpoints the deleted panels used
// (forum, users, feed). Every remaining caller builds its query through the
// marketplace list endpoint, which takes its parameters pre-encoded.

/// Send a request and read the reply as JSON.
///
/// The whole body of every helper below, because they differ only in verb and
/// payload. `Response::json` is what surfaces the API's own `{"error": "…"}`
/// on a non-2xx — the alternative is reporting "HTTP 400" and discarding the
/// reason, which is what this code did before it moved onto `renzora_net`.
#[cfg(not(target_arch = "wasm32"))]
fn send_json<T: serde::de::DeserializeOwned>(
    request: renzora::net::Request,
) -> Result<T, String> {
    request
        .send()
        .map_err(|e| format!("Request failed: {e}"))?
        .json()
        .map_err(|e| e.to_string())
}

/// GET, returning the raw response for a caller that decodes it itself.
///
/// `marketplace.rs` wants this because several of its endpoints are read
/// best-effort — an unrated asset answering 404 must yield an empty rating, not
/// a toast — and that decision belongs at the call site rather than here.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn get_json_raw(
    url: &str,
    token: Option<&str>,
) -> Result<renzora::net::Response, String> {
    renzora::net::Request::get(url)
        .maybe_bearer(token)
        .send()
        .map_err(|e| format!("Request failed: {e}"))
}

/// POST a JSON body, returning the raw response. See [`get_json_raw`].
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn post_json_raw(
    url: &str,
    body: &impl serde::Serialize,
    token: Option<&str>,
) -> Result<renzora::net::Response, String> {
    renzora::net::Request::post(url)
        .json(body)
        .maybe_bearer(token)
        .send()
        .map_err(|e| format!("Request failed: {e}"))
}

/// GET a JSON endpoint, optionally authenticated.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn get_json<T: serde::de::DeserializeOwned>(
    url: &str,
    token: Option<&str>,
) -> Result<T, String> {
    send_json(renzora::net::Request::get(url).maybe_bearer(token))
}

/// POST a JSON body, optionally authenticated.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn post_json<T: serde::de::DeserializeOwned>(
    url: &str,
    body: &impl serde::Serialize,
    token: Option<&str>,
) -> Result<T, String> {
    send_json(renzora::net::Request::post(url).json(body).maybe_bearer(token))
}

/// PUT a JSON body, optionally authenticated.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn put_json<T: serde::de::DeserializeOwned>(
    url: &str,
    body: &impl serde::Serialize,
    token: Option<&str>,
) -> Result<T, String> {
    send_json(renzora::net::Request::put(url).json(body).maybe_bearer(token))
}

/// DELETE an endpoint, optionally authenticated.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn delete_json<T: serde::de::DeserializeOwned>(
    url: &str,
    token: Option<&str>,
) -> Result<T, String> {
    send_json(renzora::net::Request::delete(url).maybe_bearer(token))
}

/// Extract the bearer token from a session, or fail like existing callers do.
pub(crate) fn require_token(session: &super::session::AuthSession) -> Result<&str, String> {
    session.access_token.as_deref().ok_or_else(|| "Not signed in".to_string())
}


// `post_multipart` went with the feed's image uploads. The remaining single-file
// uploads (avatar, banner) are all PUT, and the uploader's multi-field form uses
// `post_multipart_form` below; `multipart` still backs both.

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

    let request = match method {
        "PUT" => renzora::net::Request::put(url),
        _ => renzora::net::Request::post(url),
    };
    request
        .bearer(token)
        .body(&format!("multipart/form-data; boundary={boundary}"), body)
        .send()
        .map_err(|e| format!("Upload failed: {e}"))?
        .json()
        .map_err(|e| e.to_string())
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

    renzora::net::Request::post(url)
        .bearer(token)
        .body(&format!("multipart/form-data; boundary={boundary}"), body)
        .send()
        .map_err(|e| format!("Upload failed: {e}"))?
        .json()
        .map_err(|e| e.to_string())
}
