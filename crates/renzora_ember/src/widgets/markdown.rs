//! Markdown view — renders a markdown subset into ember UI nodes.
//!
//! Built for chat/AI/docs output: fenced code blocks (highlighted via the code
//! editor's tokenizer), `#`–`####` headings, bulleted and numbered lists with
//! indent levels, blockquotes, GitHub-style pipe tables, horizontal rules,
//! images (`![alt](url)`, downloaded on a background thread), and inline
//! `` `code` `` / `**bold**` / `*italic*` / `[link](url)` spans. Anything else
//! renders as a wrapping paragraph — malformed markdown falls back to plain
//! text, it never panics. This is a one-shot builder — to update, rebuild
//! (e.g. from a keyed list row whose hash is the content).
//!
//! Links open in the system browser ([`markdown_link_click`]); images download
//! via the [`MarkdownImages`] cache ([`markdown_images_sync`]). Both systems
//! are registered by `WidgetsPlugin`. Relative `/paths` resolve against the
//! [`MarkdownBaseUrl`] resource (default `https://renzora.com`).

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use crossbeam_channel::{unbounded, Receiver, Sender};

use crate::font::{ui_font, EmberFonts};
use crate::theme::*;

use super::code_editor::highlight::tokenize;

const BODY_SIZE: f32 = 12.0;
const TABLE_SIZE: f32 = 11.5;
const HEADING_SIZES: [f32; 4] = [16.0, 14.5, 13.5, 12.5];
/// Inline/code-span tint — warm amber, readable on dark panel backgrounds.
const C_INLINE_CODE: (u8, u8, u8) = (235, 200, 140);
/// Cap on a rendered image's display height (px); width caps at 100%.
const IMAGE_MAX_H: f32 = 360.0;

/// Render `source` (markdown subset) into a column of nodes.
pub fn markdown_view(commands: &mut Commands, fonts: &EmberFonts, source: &str) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            },
            Name::new("markdown"),
        ))
        .id();

    let mut kids: Vec<Entity> = Vec::new();
    let mut lines = source.lines().peekable();
    while let Some(line) = lines.next() {
        // Fenced code block: ``` [lang] … ```
        if line.trim_start().starts_with("```") {
            let mut code = String::new();
            for code_line in lines.by_ref() {
                if code_line.trim_start().starts_with("```") {
                    break;
                }
                code.push_str(code_line);
                code.push('\n');
            }
            kids.push(code_block(commands, fonts, code.trim_end_matches('\n')));
            continue;
        }

        let trimmed = line.trim_start();
        let indent = (line.len() - trimmed.len()) / 2;

        if trimmed.is_empty() {
            kids.push(spacer(commands, 3.0));
            continue;
        }

        // Horizontal rule: --- / *** / ___ (checked before bullets so `* * *`
        // isn't mistaken for a list item).
        if is_hr(trimmed) {
            kids.push(rule(commands));
            continue;
        }

        // Heading: # … #### (block-level only).
        if let Some(rest) = heading(trimmed) {
            let (level, text) = rest;
            kids.push(inline_line(
                commands,
                fonts,
                text,
                HEADING_SIZES[level.min(3)],
                0,
                None,
                text_primary(),
            ));
            continue;
        }

        // Image on its own line: ![alt](url) — renders the actual image.
        if trimmed.starts_with("![") {
            if let Some((alt, url)) = parse_image_line(trimmed) {
                kids.push(image_block(commands, fonts, &alt, &url));
                continue;
            }
        }

        // Blockquote: consecutive `>` lines gather into one accent-striped block.
        if trimmed.starts_with('>') {
            let mut quote: Vec<String> = vec![strip_quote(trimmed).to_string()];
            while let Some(next) = lines.peek() {
                let t = next.trim_start();
                if t.starts_with('>') {
                    quote.push(strip_quote(t).to_string());
                    lines.next();
                } else {
                    break;
                }
            }
            kids.push(blockquote(commands, fonts, &quote));
            continue;
        }

        // Pipe table: `| a | b |` header + `|---|---|` separator + body rows.
        if trimmed.starts_with('|') {
            let is_table = lines
                .peek()
                .is_some_and(|next| is_table_separator(next.trim()));
            if is_table {
                let header = split_cells(trimmed);
                lines.next(); // the separator row
                let mut rows: Vec<Vec<String>> = Vec::new();
                while let Some(next) = lines.peek() {
                    let t = next.trim();
                    if t.starts_with('|') && !is_table_separator(t) {
                        rows.push(split_cells(t));
                        lines.next();
                    } else {
                        break;
                    }
                }
                kids.push(table_block(commands, fonts, &header, &rows));
                continue;
            }
        }

        // Bullet / numbered list item.
        if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("• "))
        {
            kids.push(inline_line(commands, fonts, rest, BODY_SIZE, indent, Some("•"), value_text()));
            continue;
        }
        if let Some((marker, rest)) = numbered(trimmed) {
            kids.push(inline_line(commands, fonts, rest, BODY_SIZE, indent, Some(marker), value_text()));
            continue;
        }

        // Plain paragraph (indent preserved).
        kids.push(inline_line(commands, fonts, trimmed, BODY_SIZE, indent, None, value_text()));
    }

    commands.entity(root).add_children(&kids);
    root
}

/// A read-only, syntax-highlighted code block (Rust-like keyword palette —
/// reuses the code editor's single-pass tokenizer).
pub fn code_block(commands: &mut Commands, fonts: &EmberFonts, code: &str) -> Entity {
    let block = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                margin: UiRect::vertical(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                row_gap: Val::Px(1.0),
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            BorderColor::all(rgb(border())),
            Name::new("code-block"),
        ))
        .id();
    let lines: Vec<Entity> = code
        .lines()
        .map(|line| {
            let root = commands
                .spawn((
                    Text::new(""),
                    ui_font(&fonts.mono, 11.0),
                    TextColor(rgb(text_primary())),
                ))
                .id();
            let spans: Vec<Entity> = tokenize(line)
                .into_iter()
                .map(|(text, color)| {
                    commands
                        .spawn((TextSpan::new(text), ui_font(&fonts.mono, 11.0), TextColor(rgb(color))))
                        .id()
                })
                .collect();
            commands.entity(root).add_children(&spans);
            root
        })
        .collect();
    commands.entity(block).add_children(&lines);
    block
}

// ── Block rendering ──────────────────────────────────────────────────────────

/// `> quoted` lines — a left accent strip + muted, inline-parsed lines.
fn blockquote(commands: &mut Commands, fonts: &EmberFonts, quote_lines: &[String]) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                margin: UiRect::vertical(Val::Px(2.0)),
                ..default()
            },
            Name::new("md-quote"),
        ))
        .id();
    let a = accent();
    let strip = commands
        .spawn((
            Node {
                width: Val::Px(3.0),
                align_self: AlignSelf::Stretch,
                flex_shrink: 0.0,
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(rgba([a.0, a.1, a.2, 153])), // ~60% alpha
        ))
        .id();
    let col = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            flex_grow: 1.0,
            flex_shrink: 1.0,
            ..default()
        })
        .id();
    let inner: Vec<Entity> = quote_lines
        .iter()
        .map(|l| {
            if l.trim().is_empty() {
                spacer(commands, 3.0)
            } else {
                inline_line(commands, fonts, l, BODY_SIZE, 0, None, text_muted())
            }
        })
        .collect();
    commands.entity(col).add_children(&inner);
    commands.entity(row).add_children(&[strip, col]);
    row
}

/// GitHub-style pipe table — bordered rounded container, `section_bg()` header,
/// alternating `row_even()` / `row_odd()` body rows, flex-sized columns.
fn table_block(
    commands: &mut Commands,
    fonts: &EmberFonts,
    header: &[String],
    rows: &[Vec<String>],
) -> Entity {
    let cols = header
        .len()
        .max(rows.iter().map(Vec::len).max().unwrap_or(0))
        .max(1);
    let table = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                margin: UiRect::vertical(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(rgb(border())),
            Name::new("md-table"),
        ))
        .id();

    let mut row_entities: Vec<Entity> = Vec::new();
    row_entities.push(table_row(commands, fonts, header, cols, section_bg(), text_primary()));
    for (i, cells) in rows.iter().enumerate() {
        let bg = if i % 2 == 0 { row_even() } else { row_odd() };
        row_entities.push(table_row(commands, fonts, cells, cols, bg, value_text()));
    }
    commands.entity(table).add_children(&row_entities);
    table
}

/// One table row: `cols` equally-flexed cells (missing cells render empty).
fn table_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    cells: &[String],
    cols: usize,
    bg: (u8, u8, u8),
    text: (u8, u8, u8),
) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                ..default()
            },
            BackgroundColor(rgb(bg)),
        ))
        .id();
    let kids: Vec<Entity> = (0..cols)
        .map(|i| {
            let cell = commands
                .spawn(Node {
                    flex_basis: Val::Px(0.0),
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(6.0)),
                    ..default()
                })
                .id();
            let content = cells.get(i).map(String::as_str).unwrap_or("");
            if !content.is_empty() {
                let inner = inline_line(commands, fonts, content, TABLE_SIZE, 0, None, text);
                commands.entity(cell).add_child(inner);
            }
            cell
        })
        .collect();
    commands.entity(row).add_children(&kids);
    row
}

/// `---` / `***` / `___` — a 1px divider line.
fn rule(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(1.0),
                margin: UiRect::vertical(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(divider())),
            Name::new("md-rule"),
        ))
        .id()
}

/// `![alt](url)` on its own line — muted alt text now, swapped for the real
/// image by [`markdown_images_sync`] once the download lands.
fn image_block(commands: &mut Commands, fonts: &EmberFonts, alt: &str, url: &str) -> Entity {
    let container = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                margin: UiRect::vertical(Val::Px(4.0)),
                ..default()
            },
            MarkdownImage {
                url: url.to_string(),
                done: false,
            },
            Name::new("md-image"),
        ))
        .id();
    let label = if alt.trim().is_empty() { url } else { alt };
    let alt_text = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            MarkdownImageAlt,
        ))
        .id();
    commands.entity(container).add_child(alt_text);
    container
}

// ── Inline rendering ─────────────────────────────────────────────────────────

enum Seg {
    Normal(String),
    Bold(String),
    Italic(String),
    Code(String),
    Link { text: String, url: String },
}

/// One wrapping line: optional list marker + inline spans (`code`, **bold**,
/// *italic*, [links](url)). `base` colors the plain-text runs.
fn inline_line(
    commands: &mut Commands,
    fonts: &EmberFonts,
    text: &str,
    size: f32,
    indent: usize,
    marker: Option<&str>,
    base: (u8, u8, u8),
) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            padding: UiRect::left(Val::Px(indent as f32 * 12.0)),
            ..default()
        })
        .id();
    if let Some(marker) = marker {
        let m = commands
            .spawn((
                Text::new(marker),
                ui_font(&fonts.ui, size),
                TextColor(rgb(text_muted())),
                Node {
                    flex_shrink: 0.0,
                    ..default()
                },
            ))
            .id();
        commands.entity(row).add_child(m);
    }

    let segs = parse_inline(text);

    // Links need their own `Interaction` node, and text spans can't carry one —
    // so lines with links render as a wrapping flex row of small Text chunks
    // (words) instead of one Text with spans.
    if segs.iter().any(|s| matches!(s, Seg::Link { .. })) {
        let flow = commands
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Baseline,
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(2.0),
                flex_shrink: 1.0,
                ..default()
            })
            .id();
        let kids = flow_entities(commands, fonts, segs, size, base);
        commands.entity(flow).add_children(&kids);
        commands.entity(row).add_child(flow);
        return row;
    }

    let body = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, size),
            TextColor(rgb(base)),
            Node {
                // Shrinkable so long text wraps inside the row instead of
                // pushing the panel wider.
                flex_shrink: 1.0,
                ..default()
            },
        ))
        .id();
    let spans: Vec<Entity> = segs
        .into_iter()
        .map(|seg| match seg {
            Seg::Normal(s) => commands
                .spawn((TextSpan::new(s), ui_font(&fonts.ui, size), TextColor(rgb(base))))
                .id(),
            Seg::Bold(s) => commands
                .spawn((TextSpan::new(s), ui_font(&fonts.ui, size), TextColor(rgb(text_primary()))))
                .id(),
            Seg::Italic(s) => commands
                .spawn((TextSpan::new(s), ui_font(&fonts.ui, size), TextColor(rgb(italic_color(base)))))
                .id(),
            Seg::Code(s) => commands
                .spawn((TextSpan::new(s), ui_font(&fonts.mono, size - 1.0), TextColor(rgb(C_INLINE_CODE))))
                .id(),
            // Unreachable (link lines take the flow path) — render as accent text.
            Seg::Link { text, .. } => commands
                .spawn((TextSpan::new(text), ui_font(&fonts.ui, size), TextColor(rgb(accent()))))
                .id(),
        })
        .collect();
    commands.entity(body).add_children(&spans);
    commands.entity(row).add_child(body);
    row
}

/// Word-level Text chunks for a line that contains links (so the link chunk can
/// carry `Interaction` + [`MarkdownLink`] while the whole line still wraps).
fn flow_entities(
    commands: &mut Commands,
    fonts: &EmberFonts,
    segs: Vec<Seg>,
    size: f32,
    base: (u8, u8, u8),
) -> Vec<Entity> {
    let mut out = Vec::new();
    let words = |commands: &mut Commands, s: &str, color: (u8, u8, u8), out: &mut Vec<Entity>| {
        for w in s.split_whitespace() {
            out.push(
                commands
                    .spawn((Text::new(w), ui_font(&fonts.ui, size), TextColor(rgb(color))))
                    .id(),
            );
        }
    };
    for seg in segs {
        match seg {
            Seg::Normal(s) => words(commands, &s, base, &mut out),
            Seg::Bold(s) => words(commands, &s, text_primary(), &mut out),
            Seg::Italic(s) => words(commands, &s, italic_color(base), &mut out),
            Seg::Code(s) => out.push(
                commands
                    .spawn((Text::new(s), ui_font(&fonts.mono, size - 1.0), TextColor(rgb(C_INLINE_CODE))))
                    .id(),
            ),
            Seg::Link { text, url } => out.push(
                commands
                    .spawn((
                        Text::new(text),
                        ui_font(&fonts.ui, size),
                        TextColor(rgb(accent())),
                        Interaction::default(),
                        MarkdownLink(url),
                    ))
                    .id(),
            ),
        }
    }
    out
}

/// Italic tint — the base color nudged toward the accent.
fn italic_color(base: (u8, u8, u8)) -> (u8, u8, u8) {
    let a = accent();
    let mix = |b: u8, t: u8| (b as f32 * 0.65 + t as f32 * 0.35) as u8;
    (mix(base.0, a.0), mix(base.1, a.1), mix(base.2, a.2))
}

fn spacer(commands: &mut Commands, height: f32) -> Entity {
    commands
        .spawn(Node {
            height: Val::Px(height),
            ..default()
        })
        .id()
}

// ── Line classification ──────────────────────────────────────────────────────

fn heading(line: &str) -> Option<(usize, &str)> {
    let hashes = line.bytes().take_while(|b| *b == b'#').count();
    if hashes == 0 || hashes > 4 {
        return None;
    }
    line[hashes..]
        .strip_prefix(' ')
        .map(|rest| (hashes - 1, rest))
}

fn numbered(line: &str) -> Option<(&str, &str)> {
    let (head, rest) = line.split_once(' ')?;
    let digits = head
        .strip_suffix('.')
        .or_else(|| head.strip_suffix(')'))?;
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
        .then_some((head, rest))
}

/// `---` / `***` / `___` (3+, spaces allowed: `- - -`).
fn is_hr(line: &str) -> bool {
    let s: Vec<char> = line.chars().filter(|c| !c.is_whitespace()).collect();
    s.len() >= 3 && (s.iter().all(|c| *c == '-') || s.iter().all(|c| *c == '*') || s.iter().all(|c| *c == '_'))
}

fn strip_quote(line: &str) -> &str {
    let rest = line.strip_prefix('>').unwrap_or(line);
    rest.strip_prefix(' ').unwrap_or(rest)
}

/// A whole line of the form `![alt](url)` (an optional `"title"` is dropped).
fn parse_image_line(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("![")?;
    let (alt, rest) = rest.split_once("](")?;
    let (raw, tail) = rest.split_once(')')?;
    if !tail.trim().is_empty() {
        return None; // trailing text — not a standalone image, render inline
    }
    let url = raw.split_whitespace().next()?.to_string();
    Some((alt.to_string(), url))
}

/// Cells of a `| a | b |` row (outer pipes optional).
fn split_cells(line: &str) -> Vec<String> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    t.split('|').map(|c| c.trim().to_string()).collect()
}

/// A `|---|:---:|` header separator row.
fn is_table_separator(line: &str) -> bool {
    line.starts_with('|')
        && line.contains('-')
        && line.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

// ── Inline tokenizer ─────────────────────────────────────────────────────────

/// Split a line into Normal / Bold / Italic / Code / Link segments. Unclosed
/// markers render literally — chat text shouldn't vanish because a backtick
/// was unbalanced.
fn parse_inline(text: &str) -> Vec<Seg> {
    let mut out = Vec::new();
    let mut normal = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let flush = |normal: &mut String, out: &mut Vec<Seg>| {
        if !normal.is_empty() {
            out.push(Seg::Normal(std::mem::take(normal)));
        }
    };
    while i < chars.len() {
        // `code`
        if chars[i] == '`' {
            if let Some(end) = chars[i + 1..].iter().position(|c| *c == '`') {
                flush(&mut normal, &mut out);
                out.push(Seg::Code(chars[i + 1..i + 1 + end].iter().collect()));
                i += end + 2;
                continue;
            }
        }
        // ![alt](url) inline — no room for a block image mid-text, so it
        // renders as a link to the image (the alt text, clickable).
        if chars[i] == '!' && chars.get(i + 1) == Some(&'[') {
            if let Some((alt, url, next)) = try_link(&chars, i + 1) {
                flush(&mut normal, &mut out);
                let text = if alt.trim().is_empty() { url.clone() } else { alt };
                out.push(Seg::Link { text, url });
                i = next;
                continue;
            }
        }
        // [text](url)
        if chars[i] == '[' {
            if let Some((text, url, next)) = try_link(&chars, i) {
                flush(&mut normal, &mut out);
                let text = if text.trim().is_empty() { url.clone() } else { text };
                out.push(Seg::Link { text, url });
                i = next;
                continue;
            }
        }
        // **bold** / __bold__
        let mut matched_bold = false;
        for pair in ['*', '_'] {
            if chars[i] == pair && chars.get(i + 1) == Some(&pair) {
                let rest = &chars[i + 2..];
                if let Some(end) = rest.windows(2).position(|w| w == [pair, pair]) {
                    if end > 0 {
                        flush(&mut normal, &mut out);
                        out.push(Seg::Bold(rest[..end].iter().collect()));
                        i += end + 4;
                        matched_bold = true;
                        break;
                    }
                }
            }
        }
        if matched_bold {
            continue;
        }
        // *italic* — opener must hug the text (`* spaced *` stays literal).
        if chars[i] == '*'
            && chars.get(i + 1).is_some_and(|c| !c.is_whitespace() && *c != '*')
        {
            if let Some(j) = chars[i + 1..].iter().position(|c| *c == '*').map(|p| p + i + 1) {
                if j > i + 1 && !chars[j - 1].is_whitespace() {
                    flush(&mut normal, &mut out);
                    out.push(Seg::Italic(chars[i + 1..j].iter().collect()));
                    i = j + 1;
                    continue;
                }
            }
        }
        // _italic_ — word-boundary rules so `snake_case_names` stay literal.
        if chars[i] == '_'
            && (i == 0 || !chars[i - 1].is_alphanumeric())
            && chars.get(i + 1).is_some_and(|c| !c.is_whitespace() && *c != '_')
        {
            if let Some(j) = chars[i + 1..].iter().position(|c| *c == '_').map(|p| p + i + 1) {
                let closes_word = chars.get(j + 1).is_none_or(|c| !c.is_alphanumeric());
                if j > i + 1 && !chars[j - 1].is_whitespace() && closes_word {
                    flush(&mut normal, &mut out);
                    out.push(Seg::Italic(chars[i + 1..j].iter().collect()));
                    i = j + 1;
                    continue;
                }
            }
        }
        normal.push(chars[i]);
        i += 1;
    }
    if !normal.is_empty() {
        out.push(Seg::Normal(normal));
    }
    out
}

/// Parse `[text](url)` starting at `chars[i] == '['`. Returns
/// `(text, url, index-after-`)`)`. A `"title"` after the url is dropped.
fn try_link(chars: &[char], i: usize) -> Option<(String, String, usize)> {
    let close = chars[i + 1..].iter().position(|c| *c == ']')? + i + 1;
    if chars.get(close + 1) != Some(&'(') {
        return None;
    }
    let end = chars[close + 2..].iter().position(|c| *c == ')')? + close + 2;
    let text: String = chars[i + 1..close].iter().collect();
    let raw: String = chars[close + 2..end].iter().collect();
    let url = raw.split_whitespace().next()?.to_string();
    Some((text, url, end + 1))
}

// ── Links (open in the system browser) ───────────────────────────────────────

/// A clickable `[text](url)` span. Clicking opens the (base-resolved) URL in
/// the system browser via [`markdown_link_click`].
#[derive(Component)]
pub struct MarkdownLink(pub String);

/// Base URL that relative markdown links/images (`/path`) resolve against.
#[derive(Resource)]
pub struct MarkdownBaseUrl(pub String);

impl Default for MarkdownBaseUrl {
    fn default() -> Self {
        Self("https://renzora.com".to_string())
    }
}

fn resolve_url(url: &str, base: &str) -> String {
    if url.starts_with('/') {
        format!("{}{}", base.trim_end_matches('/'), url)
    } else {
        url.to_string()
    }
}

/// Open clicked [`MarkdownLink`]s in the system browser.
pub(crate) fn markdown_link_click(
    base: Res<MarkdownBaseUrl>,
    q: Query<(&Interaction, &MarkdownLink), Changed<Interaction>>,
) {
    for (interaction, link) in &q {
        if *interaction == Interaction::Pressed {
            open_in_browser(&resolve_url(&link.0, &base.0));
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn open_in_browser(url: &str) {
    // Same pattern as the rest of the workspace (hub / shell / splash).
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

#[cfg(target_arch = "wasm32")]
fn open_in_browser(_url: &str) {}

// ── Images (background download → bevy Image assets) ────────────────────────

/// A `![alt](url)` block waiting for (or holding) its downloaded image.
#[derive(Component)]
pub struct MarkdownImage {
    url: String,
    done: bool,
}

/// Marks the muted alt-text child, despawned once the image arrives.
#[derive(Component)]
pub(crate) struct MarkdownImageAlt;

struct DownloadedImage {
    url: String,
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

/// Async URL → `Handle<Image>` cache for markdown images (same design as the
/// hub's thumbnail cache): `request` starts a background download,
/// [`markdown_images_sync`] registers finished images each frame.
#[derive(Resource)]
pub struct MarkdownImages {
    handles: HashMap<String, Handle<Image>>,
    in_flight: HashSet<String>,
    failed: HashSet<String>,
    tx: Sender<Result<DownloadedImage, String>>,
    rx: Receiver<Result<DownloadedImage, String>>,
}

impl Default for MarkdownImages {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Self {
            handles: HashMap::new(),
            in_flight: HashSet::new(),
            failed: HashSet::new(),
            tx,
            rx,
        }
    }
}

impl MarkdownImages {
    /// The loaded handle for `url`, or `None` if not ready / failed.
    pub fn get(&self, url: &str) -> Option<Handle<Image>> {
        self.handles.get(url).cloned()
    }

    /// Whether `url` failed to download or decode.
    pub fn failed(&self, url: &str) -> bool {
        self.failed.contains(url)
    }

    /// Start downloading `url` if not already loaded / in flight / failed.
    pub fn request(&mut self, url: &str) {
        if self.handles.contains_key(url) || self.in_flight.contains(url) || self.failed.contains(url) {
            return;
        }
        self.in_flight.insert(url.to_string());
        start_download(url.to_string(), self.tx.clone());
    }

    /// Drain finished downloads into `Image` assets.
    fn poll(&mut self, images: &mut Assets<Image>) {
        let mut done = Vec::new();
        while let Ok(res) = self.rx.try_recv() {
            done.push(res);
        }
        for res in done {
            match res {
                Ok(d) => {
                    self.in_flight.remove(&d.url);
                    let image = Image::new(
                        Extent3d { width: d.width, height: d.height, depth_or_array_layers: 1 },
                        TextureDimension::D2,
                        d.rgba,
                        TextureFormat::Rgba8UnormSrgb,
                        default(),
                    );
                    self.handles.insert(d.url, images.add(image));
                }
                Err(url) => {
                    self.in_flight.remove(&url);
                    self.failed.insert(url);
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn start_download(url: String, tx: Sender<Result<DownloadedImage, String>>) {
    use std::io::Read;
    std::thread::spawn(move || {
        let result = (|| -> Result<DownloadedImage, String> {
            let response = ureq::get(&url).call().map_err(|_| url.clone())?;
            let mut bytes = Vec::new();
            response
                .into_body()
                .into_reader()
                .take(10 * 1024 * 1024)
                .read_to_end(&mut bytes)
                .map_err(|_| url.clone())?;
            let img = image::load_from_memory(&bytes).map_err(|_| url.clone())?;
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            Ok(DownloadedImage { url: url.clone(), rgba: rgba.into_raw(), width, height })
        })();
        let _ = tx.send(result);
    });
}

#[cfg(target_arch = "wasm32")]
fn start_download(_url: String, _tx: Sender<Result<DownloadedImage, String>>) {}

/// Per-frame: drain finished downloads, then give every pending
/// [`MarkdownImage`] its `ImageNode` (or leave the muted alt text on failure).
pub(crate) fn markdown_images_sync(
    mut commands: Commands,
    mut cache: ResMut<MarkdownImages>,
    base: Res<MarkdownBaseUrl>,
    mut images: ResMut<Assets<Image>>,
    mut q: Query<(Entity, &mut MarkdownImage, Option<&Children>)>,
    alts: Query<(), With<MarkdownImageAlt>>,
) {
    cache.poll(&mut images);
    for (entity, mut md, children) in &mut q {
        if md.done {
            continue;
        }
        let url = resolve_url(&md.url, &base.0);
        if let Some(handle) = cache.get(&url) {
            md.done = true;
            if let Some(children) = children {
                for child in children.iter() {
                    if alts.get(child).is_ok() {
                        commands.entity(child).try_despawn();
                    }
                }
            }
            // Guarded attach: a chrome rebuild (e.g. a theme switch) can
            // despawn this markdown node in the same frame a download lands —
            // a plain `add_child` would then panic on the dead parent.
            commands.queue(move |world: &mut World| {
                if world.get_entity(entity).is_err() {
                    return;
                }
                let img = world
                    .spawn((
                        ImageNode::new(handle),
                        Node {
                            max_width: Val::Percent(100.0),
                            max_height: Val::Px(IMAGE_MAX_H),
                            ..default()
                        },
                    ))
                    .id();
                world.entity_mut(entity).add_child(img);
            });
        } else if cache.failed(&url) {
            md.done = true; // keep the muted alt text
        } else {
            cache.request(&url);
        }
    }
}
