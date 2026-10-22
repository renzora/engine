//! Finding what an SVG pulls in from outside itself, and where that lives.
//!
//! # Why a pre-scan
//!
//! usvg resolves `<image href="...">` through a synchronous callback, and its
//! default one reads `std::fs`. That cannot work here: on the web there is no
//! file system, in a shipped game the file is inside an `.rpak` archive, and in
//! both cases the only way in is Bevy's *async* asset reader. So the loader reads
//! the XML once up front, collects every external reference, fetches them all
//! through the reader (which also registers each as a hot-reload dependency),
//! and only then hands usvg a callback that answers from that map.
//!
//! # Why paths are sandboxed
//!
//! An SVG is content, often downloaded. `href="../../../../secrets.png"` must
//! not read outside the project, so resolution happens here, lexically, and a
//! path that climbs above the asset root is refused before any read happens.

use std::collections::BTreeSet;
use std::path::{Component, Path};

use resvg::usvg::roxmltree;

/// What the pre-scan found in a document.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scan {
    /// `href` values of `<image>` / `<feImage>` elements that are not `data:`
    /// URIs or same-document `#fragment` references, in document order, deduped.
    pub image_hrefs: Vec<String>,
    /// Every font family named by a `font-family` attribute, a `style`
    /// attribute or a `<style>` sheet, deduped. Generic families included.
    pub font_families: Vec<String>,
    /// Whether the document has any `<text>`. Font probing is skipped without
    /// it, which on the web saves a request per candidate path.
    pub has_text: bool,
}

/// Elements whose `href` names an image file usvg will try to embed.
const IMAGE_ELEMENTS: &[&str] = &["image", "feImage"];

/// CSS generic family keywords. They resolve inside the font database, never
/// to a file, so they are not probed for on disk.
pub const GENERIC_FAMILIES: &[&str] = &[
    "serif",
    "sans-serif",
    "monospace",
    "cursive",
    "fantasy",
    "system-ui",
    "ui-serif",
    "ui-sans-serif",
    "ui-monospace",
    "emoji",
    "math",
];

/// Scan an SVG document's text. Fails only when the XML itself is malformed,
/// which is the same input usvg would reject, so the error is reported early.
pub fn scan(text: &str) -> Result<Scan, roxmltree::Error> {
    // DTDs allowed for the same reason usvg allows them: Illustrator exports
    // declare entities, and refusing those would refuse real files.
    let opts = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    let doc = roxmltree::Document::parse_with_options(text, opts)?;

    let mut hrefs = Vec::new();
    let mut seen_hrefs = BTreeSet::new();
    let mut families = Vec::new();
    let mut seen_families = BTreeSet::new();
    let mut has_text = false;

    for node in doc.descendants().filter(|n| n.is_element()) {
        let tag = node.tag_name().name();
        if tag == "text" {
            has_text = true;
        }
        if IMAGE_ELEMENTS.contains(&tag) {
            // `href` (SVG 2) and `xlink:href` (SVG 1.1) share the local name;
            // roxmltree reports the namespace separately, so both match here.
            if let Some(href) = node
                .attributes()
                .find(|a| a.name() == "href")
                .map(|a| a.value().trim())
            {
                if is_external_href(href) && seen_hrefs.insert(href.to_string()) {
                    hrefs.push(href.to_string());
                }
            }
        }
        for attr in node.attributes() {
            match attr.name() {
                "font-family" => push_families(attr.value(), &mut families, &mut seen_families),
                "style" => {
                    for value in css_font_family_values(attr.value()) {
                        push_families(value, &mut families, &mut seen_families);
                    }
                }
                _ => {}
            }
        }
        if tag == "style" {
            let sheet: String = node.children().filter_map(|c| c.text()).collect();
            for value in css_font_family_values(&sheet) {
                push_families(value, &mut families, &mut seen_families);
            }
        }
    }

    Ok(Scan {
        image_hrefs: hrefs,
        font_families: families,
        has_text,
    })
}

/// True for an `href` naming something outside the document.
fn is_external_href(href: &str) -> bool {
    !href.is_empty()
        && !href.starts_with('#')
        && !href
            .get(..5)
            .is_some_and(|p| p.eq_ignore_ascii_case("data:"))
}

/// The values of every `font-family:` declaration in a CSS fragment.
fn css_font_family_values(css: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = css;
    while let Some(at) = rest.find("font-family") {
        rest = &rest[at + "font-family".len()..];
        let Some(colon) = rest.find(':') else { break };
        // Anything but whitespace between the name and the colon means this was
        // a longer identifier that merely starts with `font-family`.
        if !rest[..colon].trim().is_empty() {
            continue;
        }
        rest = &rest[colon + 1..];
        let end = rest.find([';', '}']).unwrap_or(rest.len());
        out.push(&rest[..end]);
        rest = &rest[end..];
    }
    out
}

fn push_families(list: &str, out: &mut Vec<String>, seen: &mut BTreeSet<String>) {
    for name in list.split(',') {
        let name = name
            .trim()
            .trim_end_matches("!important")
            .trim()
            .trim_matches(|c| c == '\'' || c == '"')
            .trim();
        if !name.is_empty() && seen.insert(name.to_ascii_lowercase()) {
            out.push(name.to_string());
        }
    }
}

/// Why an `href` was not followed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HrefError {
    /// `../` climbed above the asset root (or above the SVG's own folder, for
    /// an SVG loaded by absolute path, where no project root is known).
    EscapesRoot,
    /// A URL scheme (`http:`, `file:`, ...). There is no network or raw file
    /// access by design: everything goes through the asset reader.
    UnsupportedScheme(String),
    /// A drive-letter or UNC path, which names a file outside any project.
    Absolute,
    /// Nothing left after stripping (`href=""`, `href="./"`).
    Empty,
}

impl std::fmt::Display for HrefError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HrefError::EscapesRoot => f.write_str("it points outside the project"),
            HrefError::UnsupportedScheme(s) => {
                write!(f, "`{s}:` references are not supported (only project files)")
            }
            HrefError::Absolute => f.write_str("absolute file paths are not allowed"),
            HrefError::Empty => f.write_str("the reference is empty"),
        }
    }
}

/// Resolve `href`, found inside the SVG at asset path `svg_path`, to the asset
/// path of the file it names.
///
/// - Relative references resolve against the SVG's own folder.
/// - A leading `/` means the asset root (the project), like a site root.
/// - `%20`-style escapes are decoded; `?query` and `#fragment` are dropped.
/// - Anything that would leave the root is [`HrefError::EscapesRoot`].
///
/// When `svg_path` is itself absolute (the editor can load files outside the
/// project), there is no project root to stay under, so the SVG's own folder
/// becomes the sandbox and the result is an absolute path inside it.
pub fn resolve_href(svg_path: &Path, href: &str) -> Result<String, HrefError> {
    let decoded = percent_decode(href.trim());
    let href = decoded
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .replace('\\', "/");

    if let Some(colon) = href.find(':') {
        let scheme = &href[..colon];
        let is_scheme = !scheme.is_empty()
            && !scheme.contains('/')
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
        if is_scheme && scheme.len() == 1 {
            // `C:/...`: a Windows drive, not a scheme.
            return Err(HrefError::Absolute);
        }
        if is_scheme {
            return Err(HrefError::UnsupportedScheme(scheme.to_ascii_lowercase()));
        }
    }
    if href.starts_with("//") {
        return Err(HrefError::Absolute);
    }

    let parent = svg_path.parent().unwrap_or(Path::new(""));
    let sandboxed_to_folder = parent.has_root()
        || parent
            .components()
            .any(|c| matches!(c, Component::Prefix(_)));

    let mut segments: Vec<String> = Vec::new();
    if !sandboxed_to_folder && !href.starts_with('/') {
        for c in parent.components() {
            match c {
                Component::Normal(s) => segments.push(s.to_string_lossy().into_owned()),
                // The SVG's own path already climbs (an odd asset path); keep
                // the climb only while there is something to pop.
                Component::ParentDir if segments.pop().is_none() => {
                    return Err(HrefError::EscapesRoot);
                }
                _ => {}
            }
        }
    }
    for seg in href.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if segments.pop().is_none() {
                    return Err(HrefError::EscapesRoot);
                }
            }
            s => segments.push(s.to_string()),
        }
    }
    if segments.is_empty() {
        return Err(HrefError::Empty);
    }
    let relative = segments.join("/");
    if sandboxed_to_folder {
        let base = parent.to_string_lossy().replace('\\', "/");
        let base = base.trim_end_matches('/');
        Ok(format!("{base}/{relative}"))
    } else {
        Ok(relative)
    }
}

/// Decode `%XX` escapes. Malformed escapes are kept literally, which is what a
/// browser does, so a file really named `100%.png` still resolves.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = |b: u8| (b as char).to_digit(16);
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// File names a font family might be saved under, most likely first:
/// `Open Sans` -> `Open Sans`, `OpenSans`, `Open-Sans`, each bare and with
/// `-Regular`, each `.ttf` then `.otf`.
///
/// This is a convenience, not the contract: the family a font *declares* is
/// what usvg matches, and the `fonts` list in the sidecar names files exactly.
/// The guesses cover how font files are conventionally named so that dropping
/// `fonts/Open Sans.ttf` or `fonts/OpenSans-Regular.ttf` into a project just
/// works.
pub fn font_file_candidates(family: &str) -> Vec<String> {
    let mut stems: Vec<String> = Vec::new();
    for stem in [
        family.to_string(),
        family.replace(' ', ""),
        family.replace(' ', "-"),
    ] {
        if !stems.contains(&stem) {
            stems.push(stem);
        }
    }
    let mut out = Vec::new();
    for stem in &stems {
        for suffix in ["", "-Regular"] {
            for ext in ["ttf", "otf"] {
                out.push(format!("{stem}{suffix}.{ext}"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> &Path {
        Path::new(s)
    }

    #[test]
    fn relative_hrefs_resolve_against_the_svg_folder() {
        assert_eq!(resolve_href(p("ui/icons/a.svg"), "b.png").unwrap(), "ui/icons/b.png");
        assert_eq!(resolve_href(p("ui/icons/a.svg"), "./img/b.png").unwrap(), "ui/icons/img/b.png");
        assert_eq!(resolve_href(p("ui/icons/a.svg"), "../b.png").unwrap(), "ui/b.png");
        assert_eq!(resolve_href(p("a.svg"), "b.png").unwrap(), "b.png");
    }

    #[test]
    fn rooted_hrefs_start_at_the_project() {
        assert_eq!(resolve_href(p("ui/icons/a.svg"), "/textures/b.png").unwrap(), "textures/b.png");
    }

    #[test]
    fn escaping_the_root_is_refused() {
        assert_eq!(resolve_href(p("ui/a.svg"), "../../b.png"), Err(HrefError::EscapesRoot));
        assert_eq!(resolve_href(p("a.svg"), "../b.png"), Err(HrefError::EscapesRoot));
        assert_eq!(resolve_href(p("ui/a.svg"), "/../b.png"), Err(HrefError::EscapesRoot));
        assert_eq!(
            resolve_href(p("ui/a.svg"), "img/../../../etc/passwd"),
            Err(HrefError::EscapesRoot)
        );
        // Percent-encoding must not smuggle a `..` past the check.
        assert_eq!(resolve_href(p("ui/a.svg"), "%2e%2e/%2e%2e/b.png"), Err(HrefError::EscapesRoot));
    }

    #[test]
    fn schemes_and_drives_are_refused() {
        assert_eq!(
            resolve_href(p("a.svg"), "https://example.com/b.png"),
            Err(HrefError::UnsupportedScheme("https".into()))
        );
        assert_eq!(
            resolve_href(p("a.svg"), "file:///etc/passwd"),
            Err(HrefError::UnsupportedScheme("file".into()))
        );
        assert_eq!(resolve_href(p("a.svg"), "C:/Windows/b.png"), Err(HrefError::Absolute));
        assert_eq!(resolve_href(p("a.svg"), "\\\\server\\share\\b.png"), Err(HrefError::Absolute));
    }

    #[test]
    fn an_svg_outside_the_project_is_sandboxed_to_its_folder() {
        let svg = if cfg!(windows) { "C:/art/logo.svg" } else { "/art/logo.svg" };
        let base = if cfg!(windows) { "C:/art" } else { "/art" };
        assert_eq!(resolve_href(p(svg), "img/b.png").unwrap(), format!("{base}/img/b.png"));
        assert_eq!(resolve_href(p(svg), "../b.png"), Err(HrefError::EscapesRoot));
    }

    #[test]
    fn escapes_queries_and_fragments() {
        assert_eq!(resolve_href(p("a.svg"), "my%20pic.png?v=2#x").unwrap(), "my pic.png");
        assert_eq!(resolve_href(p("a.svg"), "100%.png").unwrap(), "100%.png");
        assert_eq!(resolve_href(p("a.svg"), "./"), Err(HrefError::Empty));
    }

    #[test]
    fn scan_finds_external_images_but_not_data_uris() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
            <image href="a.png"/>
            <image xlink:href="sub/b.jpg"/>
            <image href="data:image/png;base64,AAAA"/>
            <image href="#local"/>
            <image href="a.png"/>
            <filter><feImage href="c.webp"/></filter>
        </svg>"##;
        let s = scan(svg).unwrap();
        assert_eq!(s.image_hrefs, vec!["a.png", "sub/b.jpg", "c.webp"]);
        assert!(!s.has_text);
    }

    #[test]
    fn scan_collects_font_families_from_every_place_css_can_put_them() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
            <style>.t { font-family: 'Brand Sans', serif; }</style>
            <text font-family="Open Sans, sans-serif">a</text>
            <text style="fill:red; font-family: &quot;Mono Thing&quot;">b</text>
        </svg>"#;
        let s = scan(svg).unwrap();
        assert!(s.has_text);
        assert_eq!(
            s.font_families,
            vec!["Brand Sans", "serif", "Open Sans", "sans-serif", "Mono Thing"]
        );
    }

    #[test]
    fn malformed_xml_fails_the_scan() {
        assert!(scan("<svg><g></svg>").is_err());
    }

    #[test]
    fn font_candidates_cover_common_file_names() {
        let c = font_file_candidates("Open Sans");
        assert_eq!(c[0], "Open Sans.ttf");
        assert!(c.contains(&"OpenSans-Regular.ttf".to_string()));
        assert!(c.contains(&"Open-Sans.otf".to_string()));
        // A one-word family produces no duplicate stems.
        assert_eq!(font_file_candidates("Inter").len(), 4);
    }
}
