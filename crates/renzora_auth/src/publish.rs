//! Publish (upload) API client for the Renzora marketplace and game store.
//!
//! Mirrors the website's `/marketplace/upload` wizard (`crates/web/src/pages/
//! upload.rs`): the metadata is a single JSON text field posted as multipart
//! alongside the main `file` and an optional `thumbnail`, then screenshots /
//! video / audio previews are attached one-per-request to the item's `/media`
//! endpoint after the upload returns the new item's id + slug.
//!
//! Two content types share this module, exactly as the web wizard's step 1 does:
//! **assets** (`/api/marketplace/*`) and **games** (`/api/games/*`). All calls
//! are blocking and meant to run on a worker thread (the crate convention).

use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
use crate::client::{api_base, post_json, post_multipart_form, require_token, FilePart};
use crate::session::AuthSession;

/// Which store an upload targets. The web wizard picks this in step 1; it selects
/// the categories endpoint, the upload endpoint, and the `/media` field names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContentType {
    Asset,
    Game,
}

impl ContentType {
    /// Human label used in the review step and success toast.
    pub fn label(self) -> &'static str {
        match self {
            ContentType::Asset => "Marketplace Asset",
            ContentType::Game => "Game",
        }
    }
}

/// The upload metadata JSON, serialized into the multipart `metadata` field.
///
/// Field-for-field the object the web wizard's `handleSubmit` builds: the asset
/// path adds `tags` / `download_filename` / optional `credit_*`; the game path
/// sends only the common five. `skip_serializing_if` keeps the game payload lean
/// (no `tags: null`) and omits credit fields unless a creator was named.
#[derive(Serialize, Default, Clone)]
pub struct PublishMeta {
    pub name: String,
    pub description: String,
    pub category: String,
    pub price_credits: i64,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credit_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credit_url: Option<String>,
}

/// The slice of an upload response we act on: the new item's id (to attach media
/// to) and slug (to link to). The server returns the full `AssetDetail`; serde
/// ignores the rest.
#[derive(Deserialize, Clone, Debug, Default)]
pub struct UploadedItem {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub slug: String,
}

/// One tag suggestion from the autocomplete endpoint. Only the name is used; the
/// endpoint also returns `id`/`slug`/`approved` which serde drops.
#[derive(Deserialize, Clone, Debug)]
pub struct TagOption {
    pub name: String,
}

// ── Categories ────────────────────────────────────────────────────────────────

/// List game-store categories. Same shape as `marketplace::list_categories`
/// (`{id,name,slug,description,icon}`), just a different endpoint — the web
/// wizard swaps the URL on the content type in step 2.
#[cfg(not(target_arch = "wasm32"))]
pub fn list_game_categories() -> Result<Vec<crate::marketplace::Category>, String> {
    crate::client::get_json(&format!("{}/api/games/categories", api_base()), None)
}

// ── Tags ──────────────────────────────────────────────────────────────────────

/// Autocomplete tags by prefix. Public endpoint. `q` empty returns the approved
/// set; the wizard only calls it with a non-empty query.
#[cfg(not(target_arch = "wasm32"))]
pub fn search_tags(query: &str) -> Result<Vec<TagOption>, String> {
    let url = format!("{}/api/marketplace/tags?q={}", api_base(), urlencoded(query));
    crate::client::get_json(&url, None)
}

/// Submit a brand-new tag for review, returning it so the wizard can add the
/// pill immediately (it's usable pending approval, mirroring the website).
#[cfg(not(target_arch = "wasm32"))]
pub fn submit_tag(session: &AuthSession, name: &str) -> Result<TagOption, String> {
    let token = require_token(session)?;
    post_json(
        &format!("{}/api/marketplace/tags/submit", api_base()),
        &serde_json::json!({ "name": name }),
        Some(token),
    )
}

// ── Upload (main item) ────────────────────────────────────────────────────────

/// The main file part of an upload: the bytes plus the name/MIME the server
/// stores. Read off disk on the worker thread just before the request.
#[cfg(not(target_arch = "wasm32"))]
pub struct UploadFile {
    pub filename: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

/// Publish an asset: `POST /api/marketplace/upload` with the `metadata` JSON, the
/// main `file`, and an optional cover `thumbnail`. Returns the new asset's
/// id + slug. Auto-publishes server-side (there is no draft state in the wizard).
#[cfg(not(target_arch = "wasm32"))]
pub fn upload_asset(
    session: &AuthSession,
    meta: &PublishMeta,
    file: &UploadFile,
    thumbnail: Option<&UploadFile>,
) -> Result<UploadedItem, String> {
    upload_item(session, "/api/marketplace/upload", meta, file, thumbnail)
}

/// Publish a game: `POST /api/games/upload`. Same multipart shape as an asset.
#[cfg(not(target_arch = "wasm32"))]
pub fn upload_game(
    session: &AuthSession,
    meta: &PublishMeta,
    file: &UploadFile,
    thumbnail: Option<&UploadFile>,
) -> Result<UploadedItem, String> {
    upload_item(session, "/api/games/upload", meta, file, thumbnail)
}

#[cfg(not(target_arch = "wasm32"))]
fn upload_item(
    session: &AuthSession,
    path: &str,
    meta: &PublishMeta,
    file: &UploadFile,
    thumbnail: Option<&UploadFile>,
) -> Result<UploadedItem, String> {
    let token = require_token(session)?;
    let json = serde_json::to_string(meta).map_err(|e| e.to_string())?;

    let mut files = vec![FilePart {
        field: "file",
        filename: &file.filename,
        content_type: &file.content_type,
        bytes: &file.bytes,
    }];
    if let Some(t) = thumbnail {
        files.push(FilePart {
            field: "thumbnail",
            filename: &t.filename,
            content_type: &t.content_type,
            bytes: &t.bytes,
        });
    }
    post_multipart_form(
        &format!("{}{}", api_base(), path),
        &[("metadata", json.as_str())],
        &files,
        token,
    )
}

// ── Media (screenshots / video / audio) ───────────────────────────────────────

/// A preview-media attachment posted after the main upload. Matches the web
/// wizard's per-item `/media` calls.
#[cfg(not(target_arch = "wasm32"))]
pub enum MediaUpload {
    /// A screenshot / gallery image (`media_type=image` + `file`).
    Image(UploadFile),
    /// An audio preview (`media_type=audio` + `file`).
    Audio(UploadFile),
    /// A video preview by URL (`video_url=<url>`, no file).
    Video(String),
}

/// Attach one preview-media item to a published **asset**:
/// `POST /api/marketplace/{id}/media`. Best-effort — the wizard ignores media
/// failures so a flaky screenshot never fails an otherwise-successful publish.
#[cfg(not(target_arch = "wasm32"))]
pub fn add_asset_media(
    session: &AuthSession,
    asset_id: &str,
    media: &MediaUpload,
) -> Result<(), String> {
    let token = require_token(session)?;
    let url = format!("{}/api/marketplace/{}/media", api_base(), asset_id);
    let _: serde_json::Value = match media {
        MediaUpload::Image(f) => post_multipart_form(
            &url,
            &[("media_type", "image")],
            &[FilePart { field: "file", filename: &f.filename, content_type: &f.content_type, bytes: &f.bytes }],
            token,
        )?,
        MediaUpload::Audio(f) => post_multipart_form(
            &url,
            &[("media_type", "audio")],
            &[FilePart { field: "file", filename: &f.filename, content_type: &f.content_type, bytes: &f.bytes }],
            token,
        )?,
        MediaUpload::Video(link) => post_multipart_form(
            &url,
            &[("video_url", link.as_str())],
            &[],
            token,
        )?,
    };
    Ok(())
}

/// Attach a screenshot to a published **game**: `POST /api/games/{id}/media`.
/// Games take `type=image` + a `sort_order` (assets don't); only images are
/// supported here, matching the web wizard.
#[cfg(not(target_arch = "wasm32"))]
pub fn add_game_media(
    session: &AuthSession,
    game_id: &str,
    sort_order: usize,
    file: &UploadFile,
) -> Result<(), String> {
    let token = require_token(session)?;
    let url = format!("{}/api/games/{}/media", api_base(), game_id);
    let sort = sort_order.to_string();
    let _: serde_json::Value = post_multipart_form(
        &url,
        &[("type", "image"), ("sort_order", sort.as_str())],
        &[FilePart { field: "file", filename: &file.filename, content_type: &file.content_type, bytes: &file.bytes }],
        token,
    )?;
    Ok(())
}

/// Simple percent-encoding for query parameters (mirrors `marketplace`/`client`).
fn urlencoded(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}
