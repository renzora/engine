//! Markdown view — renders a markdown subset into ember UI nodes.
//!
//! Built for chat/AI output: fenced code blocks (highlighted via the code
//! editor's tokenizer), `#`–`####` headings, bulleted and numbered lists
//! with indent levels, and inline `` `code` `` / `**bold**` spans. Anything
//! else renders as a wrapping paragraph. This is a one-shot builder — to
//! update, rebuild (e.g. from a keyed list row whose hash is the content).

use bevy::prelude::*;

use crate::font::{ui_font, EmberFonts};
use crate::theme::*;

use super::code_editor::highlight::tokenize;

const BODY_SIZE: f32 = 12.0;
const HEADING_SIZES: [f32; 4] = [16.0, 14.5, 13.5, 12.5];
/// Inline/code-span tint — warm amber, readable on dark panel backgrounds.
const C_INLINE_CODE: (u8, u8, u8) = (235, 200, 140);
const C_BOLD: (u8, u8, u8) = (240, 240, 250);

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
            ));
            continue;
        }

        // Bullet / numbered list item.
        if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| trimmed.strip_prefix("• "))
        {
            kids.push(inline_line(commands, fonts, rest, BODY_SIZE, indent, Some("•")));
            continue;
        }
        if let Some((marker, rest)) = numbered(trimmed) {
            kids.push(inline_line(commands, fonts, rest, BODY_SIZE, indent, Some(marker)));
            continue;
        }

        // Plain paragraph (indent preserved).
        kids.push(inline_line(commands, fonts, trimmed, BODY_SIZE, indent, None));
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

// ── Inline rendering ─────────────────────────────────────────────────────────

enum Seg {
    Normal(String),
    Bold(String),
    Code(String),
}

/// One wrapping line: optional list marker + inline spans (`code`, **bold**).
fn inline_line(
    commands: &mut Commands,
    fonts: &EmberFonts,
    text: &str,
    size: f32,
    indent: usize,
    marker: Option<&str>,
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

    let body = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, size),
            TextColor(rgb(text_primary())),
            Node {
                // Shrinkable so long text wraps inside the row instead of
                // pushing the panel wider.
                flex_shrink: 1.0,
                ..default()
            },
        ))
        .id();
    let spans: Vec<Entity> = parse_inline(text)
        .into_iter()
        .map(|seg| match seg {
            Seg::Normal(s) => commands
                .spawn((TextSpan::new(s), ui_font(&fonts.ui, size), TextColor(rgb(text_primary()))))
                .id(),
            Seg::Bold(s) => commands
                .spawn((TextSpan::new(s), ui_font(&fonts.ui, size), TextColor(rgb(C_BOLD))))
                .id(),
            Seg::Code(s) => commands
                .spawn((TextSpan::new(s), ui_font(&fonts.mono, size - 1.0), TextColor(rgb(C_INLINE_CODE))))
                .id(),
        })
        .collect();
    commands.entity(body).add_children(&spans);
    commands.entity(row).add_child(body);
    row
}

fn spacer(commands: &mut Commands, height: f32) -> Entity {
    commands
        .spawn(Node {
            height: Val::Px(height),
            ..default()
        })
        .id()
}

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

/// Split a line into Normal / Bold / Code segments. Unclosed markers render
/// literally — chat text shouldn't vanish because a backtick was unbalanced.
fn parse_inline(text: &str) -> Vec<Seg> {
    let mut out = Vec::new();
    let mut normal = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        // `code`
        if chars[i] == '`' {
            if let Some(end) = chars[i + 1..].iter().position(|c| *c == '`') {
                if !normal.is_empty() {
                    out.push(Seg::Normal(std::mem::take(&mut normal)));
                }
                out.push(Seg::Code(chars[i + 1..i + 1 + end].iter().collect()));
                i += end + 2;
                continue;
            }
        }
        // **bold**
        if chars[i] == '*' && chars.get(i + 1) == Some(&'*') {
            let rest = &chars[i + 2..];
            if let Some(end) = rest.windows(2).position(|w| w == ['*', '*']) {
                if !normal.is_empty() {
                    out.push(Seg::Normal(std::mem::take(&mut normal)));
                }
                out.push(Seg::Bold(rest[..end].iter().collect()));
                i += end + 4;
                continue;
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
