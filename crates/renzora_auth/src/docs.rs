//! Docs portal + courses API client (all public endpoints).
//!
//! Doc pages are requested with `?format=md` so the editor can render the raw
//! markdown natively with `markdown_view`.
//! Blocking HTTP calls designed to run on background threads.

use serde::Deserialize;

use crate::client::api_base;
#[cfg(not(target_arch = "wasm32"))]
use crate::client::{get_json, urlencoded};

// ── Types ──

#[derive(Debug, Deserialize, Clone)]
pub struct DocVersion {
    pub id: String,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DocVersions {
    #[serde(default)]
    pub default: String,
    #[serde(default)]
    pub versions: Vec<DocVersion>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Sidebar {
    pub version: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub groups: Vec<SidebarGroup>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SidebarGroup {
    pub group: String,
    /// `basic` or `advanced` (empty = basic).
    #[serde(default)]
    pub level: String,
    #[serde(default)]
    pub categories: Vec<SidebarCategory>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SidebarCategory {
    pub category: String,
    #[serde(default)]
    pub pages: Vec<SidebarPage>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SidebarPage {
    pub slug: String,
    pub title: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DocPage {
    pub version: String,
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub category: String,
    /// Raw markdown when fetched with `?format=md`.
    pub content: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DocSearchResult {
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CourseSummary {
    pub id: String,
    pub title: String,
    pub slug: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub cover_image_url: Option<String>,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub difficulty: String,
    #[serde(default)]
    pub price_credits: i64,
    #[serde(default)]
    pub chapter_count: i64,
    #[serde(default)]
    pub enrolled_count: i64,
    #[serde(default)]
    pub rating: f64,
    #[serde(default)]
    pub rating_count: i64,
    #[serde(default)]
    pub creator_name: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CoursesResponse {
    pub courses: Vec<CourseSummary>,
    #[serde(default)]
    pub total: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CourseChapter {
    pub id: String,
    pub title: String,
    pub slug: String,
    #[serde(default)]
    pub sort_order: i64,
    #[serde(default)]
    pub duration_minutes: Option<i64>,
    #[serde(default)]
    pub is_free_preview: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CourseDetail {
    pub id: String,
    pub title: String,
    pub slug: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub cover_image_url: Option<String>,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub difficulty: String,
    #[serde(default)]
    pub price_credits: i64,
    #[serde(default)]
    pub chapters: Vec<CourseChapter>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChapterView {
    #[serde(default)]
    pub title: String,
    /// Chapter body (markdown as authored); empty when locked.
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub video_url: Option<String>,
    #[serde(default)]
    pub locked: bool,
}

// ── API calls ──

/// Available doc versions.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_versions() -> Result<DocVersions, String> {
    get_json(&format!("{}/api/docs/versions", api_base()), None)
}

/// Sidebar tree for a version.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_sidebar(version: &str) -> Result<Sidebar, String> {
    get_json(&format!("{}/api/docs/sidebar/{version}", api_base()), None)
}

/// A doc page as raw markdown.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_page(version: &str, slug: &str) -> Result<DocPage, String> {
    get_json(
        &format!("{}/api/docs/page/{version}/{slug}?format=md", api_base()),
        None,
    )
}

/// Search docs within a version.
#[cfg(not(target_arch = "wasm32"))]
pub fn search(version: &str, query: &str) -> Result<Vec<DocSearchResult>, String> {
    get_json(
        &format!("{}/api/docs/search/{version}?q={}", api_base(), urlencoded(query)),
        None,
    )
}

/// List courses.
#[cfg(not(target_arch = "wasm32"))]
pub fn list_courses(category: Option<&str>, page: u32) -> Result<CoursesResponse, String> {
    let mut url = format!("{}/api/courses?page={page}", api_base());
    if let Some(c) = category {
        url.push_str(&format!("&category={}", urlencoded(c)));
    }
    get_json(&url, None)
}

/// Course detail with chapter list.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_course(slug: &str) -> Result<CourseDetail, String> {
    get_json(&format!("{}/api/courses/{slug}", api_base()), None)
}

/// View a chapter (locked chapters return empty content + `locked: true`).
#[cfg(not(target_arch = "wasm32"))]
pub fn get_chapter(course_slug: &str, chapter_slug: &str) -> Result<ChapterView, String> {
    get_json(
        &format!("{}/api/courses/{course_slug}/chapters/{chapter_slug}/view", api_base()),
        None,
    )
}
