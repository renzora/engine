//! The code-editor ECS systems: measuring, focus/click, keyboard, scroll,
//! re-rendering the visible rows, and the blinking caret. Everything that draws
//! or hit-tests goes through the [`super::layout`] visual-row model, so folding
//! and word wrap are handled uniformly.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::MouseWheel;
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::text::{FontSize, TextLayoutInfo};
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use crate::font::{icon_text, EmberFonts};
use crate::theme::*;

use super::edit::{self, bracket_match, has_selection, sel_range};
use super::highlight::tokenize;
use super::layout::{self, char_len};
use super::{
    mono, CodeEditor, CodeFoldToggle, CodeProbe, CodeViewport, Metrics, FOLD_COL_W, PAD, PROBE_LEN,
    TAB_WIDTH,
};

/// Recompute each editor's derived metrics; flag a re-render when they change.
pub(crate) fn code_metrics(mut editors: Query<&mut CodeEditor>) {
    for mut ed in &mut editors {
        if ed.recompute_metrics() {
            ed.dirty = true;
        }
    }
}

/// Measure the active mono font's real advance from a hidden probe (see the
/// module note in [`super`]). Scale-invariant and tightly guarded.
pub(crate) fn code_probe(
    fonts: Option<Res<EmberFonts>>,
    mut probes: Query<(&mut TextFont, &TextLayoutInfo, &CodeProbe)>,
    mut editors: Query<&mut CodeEditor>,
) {
    let Some(fonts) = fonts else {
        return;
    };
    for (mut tf, info, probe) in &mut probes {
        let Ok(mut ed) = editors.get_mut(probe.editor) else {
            continue;
        };
        let want_size = FontSize::Px(ed.font_size);
        if tf.font != fonts.mono || tf.font_size != want_size {
            tf.font = fonts.mono.clone();
            tf.font_size = want_size;
            continue;
        }
        let sf = if info.scale_factor > 0.0 { info.scale_factor } else { 1.0 };
        let em_physical = ed.font_size * sf;
        if em_physical <= 0.0 {
            continue;
        }
        let ratio = (info.size.x / PROBE_LEN) / em_physical;
        if (0.50..=0.72).contains(&ratio) && (ratio - ed.advance_ratio).abs() > 0.001 {
            ed.advance_ratio = ratio;
            ed.dirty = true;
        }
    }
}

/// Track the viewport size: how many visual rows fit, the width for full-width
/// overlays, and (when wrap is on) the wrap column count.
pub(crate) fn code_measure(
    viewports: Query<(&ComputedNode, &CodeViewport)>,
    mut editors: Query<&mut CodeEditor>,
) {
    for (cn, vp) in &viewports {
        let size = cn.size() * cn.inverse_scale_factor();
        if let Ok(mut ed) = editors.get_mut(vp.editor) {
            let vis = ((size.y / ed.line_h).floor() as usize).max(1);
            if ed.visible != vis {
                ed.visible = vis;
                ed.dirty = true;
            }
            if (ed.view_w - size.x).abs() > 0.5 {
                ed.view_w = size.x;
                ed.dirty = true;
            }
            // Wrap width: columns of text that fit between the gutter and the
            // right edge (leaving a little right margin). 0 means "don't wrap".
            let cols = if ed.wrap && ed.char_w > 0.0 {
                (((size.x - ed.gutter_w - PAD * 2.0) / ed.char_w).floor() as i32).max(1) as usize
            } else {
                0
            };
            if ed.wrap_cols != cols {
                ed.wrap_cols = cols;
                ed.dirty = true;
            }
        }
    }
}

/// Toggle a fold when its gutter chevron is clicked. Runs before the text-click
/// handler so a chevron press doesn't also reposition the caret.
pub(crate) fn code_fold_click(
    q: Query<(&Interaction, &CodeFoldToggle), Changed<Interaction>>,
    mut editors: Query<&mut CodeEditor>,
) {
    for (interaction, toggle) in &q {
        if *interaction == Interaction::Pressed {
            if let Ok(mut ed) = editors.get_mut(toggle.editor) {
                edit::toggle_fold(&mut ed, toggle.line);
            }
        }
    }
}

/// Click to focus + place the cursor; double/triple-click select word/line;
/// drag (or shift-click) to select. Clicks in the gutter column are ignored
/// here (the fold chevrons own that strip).
pub(crate) fn code_pointer(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    viewports: Query<(&Interaction, &RelativeCursorPosition, &ComputedNode, &CodeViewport)>,
    mut editors: Query<(Entity, &mut CodeEditor)>,
) {
    if !mouse.pressed(MouseButton::Left) {
        return;
    }
    let just = mouse.just_pressed(MouseButton::Left);
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let mut target: Option<(Entity, Vec2)> = None;
    for (interaction, rcp, cn, vp) in &viewports {
        let hit = if just { *interaction == Interaction::Pressed } else { rcp.cursor_over };
        if hit {
            if let Some(nrm) = rcp.normalized {
                let size = cn.size() * cn.inverse_scale_factor();
                target = Some((vp.editor, Vec2::new((nrm.x + 0.5) * size.x, (nrm.y + 0.5) * size.y)));
            }
            break;
        }
    }
    let Some((editor, local)) = target else {
        return;
    };
    let now = time.elapsed_secs();
    for (e, mut ed) in &mut editors {
        if just {
            let focus = e == editor;
            if ed.focused != focus {
                ed.focused = focus;
            }
        }
        if e != editor || !ed.focused || ed.char_w <= 0.0 {
            continue;
        }
        // Gutter clicks belong to the fold chevrons, not the caret.
        if local.x < ed.gutter_w {
            continue;
        }
        let rows = ed.rows();
        let row_index = (ed.scroll + (local.y / ed.line_h) as usize).min(rows.len().saturating_sub(1));
        let x_col = (((local.x - ed.gutter_w - PAD) / ed.char_w).round().max(0.0)) as usize;
        let (line, raw_col) = layout::buffer_pos(&rows, row_index, x_col);
        let col = raw_col.min(char_len(&ed.text, line));

        if just {
            // Count consecutive clicks near the same spot for word/line select.
            let count = match ed.last_click {
                Some((t, ll, lc, c)) if now - t < 0.4 && ll == line && lc.abs_diff(col) <= 1 => {
                    (c % 3) + 1
                }
                _ => 1,
            };
            ed.last_click = Some((now, line, col, count));
            match count {
                2 => {
                    edit::select_word_at(&mut ed, line, col);
                    continue;
                }
                3 => {
                    edit::select_line_at(&mut ed, line);
                    continue;
                }
                _ => {}
            }
        }

        ed.cursor_line = line;
        ed.cursor_col = col;
        ed.goal_col = Some(col);
        if just && !shift {
            ed.anchor_line = ed.cursor_line;
            ed.anchor_col = ed.cursor_col;
        }
        ed.dirty = true;
    }
}

/// Keyboard: discrete chord commands (undo, clipboard, comment, line move…) plus
/// ordinary typing and caret motion.
pub(crate) fn code_input(
    mut events: MessageReader<KeyboardInput>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut editors: Query<&mut CodeEditor>,
) {
    let text_keys: Vec<Key> = events
        .read()
        .filter(|e| e.state == ButtonState::Pressed)
        .map(|e| e.logical_key.clone())
        .collect();

    let alt = keyboard.pressed(KeyCode::AltLeft) || keyboard.pressed(KeyCode::AltRight);
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let cmd = keyboard.pressed(KeyCode::ControlLeft)
        || keyboard.pressed(KeyCode::ControlRight)
        || keyboard.pressed(KeyCode::SuperLeft)
        || keyboard.pressed(KeyCode::SuperRight);

    // Any chord this frame? (avoids iterating events when nothing to do)
    let has_chords = cmd || alt;
    if text_keys.is_empty() && !has_chords {
        return;
    }

    for mut editor in &mut editors {
        if !editor.focused {
            continue;
        }
        let ed = &mut *editor;

        // --- discrete chord commands (physical keys) ---
        if cmd && keyboard.just_pressed(KeyCode::KeyZ) {
            if shift {
                edit::redo(ed);
            } else {
                edit::undo(ed);
            }
        }
        if cmd && keyboard.just_pressed(KeyCode::KeyY) {
            edit::redo(ed);
        }
        if cmd && keyboard.just_pressed(KeyCode::KeyA) {
            edit::select_all(ed);
        }
        if cmd && !shift && !alt && keyboard.just_pressed(KeyCode::KeyC) {
            edit::clipboard_copy(ed);
        }
        if cmd && !shift && !alt && keyboard.just_pressed(KeyCode::KeyX) {
            edit::clipboard_cut(ed);
        }
        if cmd && !shift && !alt && keyboard.just_pressed(KeyCode::KeyV) {
            edit::clipboard_paste(ed);
        }
        if cmd && keyboard.just_pressed(KeyCode::Slash) {
            if let Some(tok) = ed.line_comment.clone() {
                edit::toggle_comment(ed, &tok);
            }
        }
        if cmd && shift && keyboard.just_pressed(KeyCode::KeyK) {
            edit::delete_lines(ed);
        }
        if alt && !cmd && keyboard.just_pressed(KeyCode::ArrowUp) {
            if shift {
                edit::duplicate_lines(ed, true);
            } else {
                edit::move_lines(ed, true);
            }
        }
        if alt && !cmd && keyboard.just_pressed(KeyCode::ArrowDown) {
            if shift {
                edit::duplicate_lines(ed, false);
            } else {
                edit::move_lines(ed, false);
            }
        }

        // --- ordinary typing / caret motion ---
        for key in &text_keys {
            // Arrows while Alt is held are line move/duplicate, handled above.
            if alt && matches!(key, Key::ArrowUp | Key::ArrowDown | Key::ArrowLeft | Key::ArrowRight) {
                continue;
            }
            edit::edit(ed, key, shift, cmd);
        }
    }
}

pub(crate) fn code_scroll(
    mut wheel: MessageReader<MouseWheel>,
    viewports: Query<(&RelativeCursorPosition, &CodeViewport)>,
    mut editors: Query<&mut CodeEditor>,
) {
    let mut dy = 0.0;
    for ev in wheel.read() {
        dy += ev.y;
    }
    if dy == 0.0 {
        return;
    }
    for (rcp, vp) in &viewports {
        if rcp.cursor_over {
            if let Ok(mut ed) = editors.get_mut(vp.editor) {
                let max = ed.rows().len().saturating_sub(1) as i32;
                let new = (ed.scroll as i32 - (dy * 3.0) as i32).clamp(0, max) as usize;
                if new != ed.scroll {
                    ed.scroll = new;
                    ed.dirty = true;
                }
            }
            break;
        }
    }
}

pub(crate) fn code_render(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    mut editors: Query<(Entity, &mut CodeEditor)>,
    children: Query<&Children>,
) {
    let Some(fonts) = fonts else {
        return;
    };
    for (entity, mut ed) in &mut editors {
        if !ed.dirty || ed.char_w <= 0.0 {
            continue;
        }
        ed.dirty = false;
        let m = ed.metrics();
        let sp = syntax_palette();
        let body = ed.body;
        if let Ok(kids) = children.get(body) {
            for c in kids.iter() {
                commands.entity(c).despawn();
            }
        }

        let rows = ed.rows();
        let start = ed.scroll.min(rows.len().saturating_sub(1));
        let end = (start + ed.visible + 1).min(rows.len());
        if start >= end {
            continue;
        }

        // Tokenize each distinct visible buffer line once (wrap can show a line
        // across several rows). Thread the highlighter's cross-line state from
        // the top so block comments etc. resolve when scrolled.
        let lo = rows[start].line;
        let hi = rows[end - 1].line;
        let mut per_line: Vec<Vec<(String, Color)>> = Vec::with_capacity(hi - lo + 1);
        if let Some(hl) = ed.highlighter.as_ref() {
            let mut st = 0u32;
            for i in 0..lo {
                st = hl(&ed.text[i], st).1;
            }
            for i in lo..=hi {
                let (toks, ns) = hl(&ed.text[i], st);
                st = ns;
                let line = &ed.text[i];
                let mut out: Vec<(String, Color)> = Vec::with_capacity(toks.len());
                let mut off = 0usize;
                for t in toks {
                    let e = (off + t.len).min(line.len());
                    if e > off && line.is_char_boundary(off) && line.is_char_boundary(e) {
                        out.push((line[off..e].to_string(), t.color));
                    }
                    off = e;
                }
                if off < line.len() && line.is_char_boundary(off) {
                    out.push((line[off..].to_string(), rgb(sp.normal)));
                }
                per_line.push(out);
            }
        } else {
            for i in lo..=hi {
                per_line.push(tokenize(&ed.text[i]).into_iter().map(|(s, c)| (s, rgb(c))).collect());
            }
        }
        let spans_of = |line: usize| -> &Vec<(String, Color)> { &per_line[line - lo] };

        // ---- Underlay: current-line highlight + indent guides (furthest back).
        let mut underlay: Vec<Entity> = Vec::new();
        let guide_color = rgba(sp.indent_guide);
        for (k, row) in rows[start..end].iter().enumerate() {
            let y = k as f32 * m.line_h;
            if row.line == ed.cursor_line && ed.view_w > 0.0 {
                underlay.push(
                    commands
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(0.0),
                                top: Val::Px(y),
                                width: Val::Px(ed.view_w),
                                height: Val::Px(m.line_h),
                                ..default()
                            },
                            BackgroundColor(rgba(sp.current_line)),
                            bevy::ui::FocusPolicy::Pass,
                            Name::new("code-current-line"),
                        ))
                        .id(),
                );
            }
            // Indent guides for this line's leading whitespace.
            let mut cols = 0usize;
            for c in ed.text[row.line].chars() {
                match c {
                    ' ' => cols += 1,
                    '\t' => cols += TAB_WIDTH,
                    _ => break,
                }
            }
            for i in 1..(cols / TAB_WIDTH) {
                let x = m.gutter_w + m.pad + (i * TAB_WIDTH) as f32 * m.char_w;
                underlay.push(
                    commands
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(x),
                                top: Val::Px(y),
                                width: Val::Px(1.0),
                                height: Val::Px(m.line_h),
                                ..default()
                            },
                            BackgroundColor(guide_color),
                            bevy::ui::FocusPolicy::Pass,
                            Name::new("code-indent-guide"),
                        ))
                        .id(),
                );
            }
        }
        commands.entity(body).add_children(&underlay);

        // ---- Selection rects (behind the text), mapped onto visual rows.
        if has_selection(&ed) {
            let ((sl, sc), (el, ec)) = sel_range(&ed);
            let mut rects: Vec<Entity> = Vec::new();
            for (k, row) in rows[start..end].iter().enumerate() {
                if row.line < sl || row.line > el {
                    continue;
                }
                let full = char_len(&ed.text, row.line);
                let line_a = if row.line == sl { sc } else { 0 };
                let line_b = if row.line == el { ec } else { full };
                let a = line_a.max(row.start_col);
                let b = line_b.min(row.end_col);
                let mut w = (b.saturating_sub(a)) as f32 * m.char_w;
                // Show the selected newline as a sliver of trailing highlight.
                if row.line < el && row.end_col >= full {
                    w += m.char_w * 0.5;
                }
                if w <= 0.0 {
                    continue;
                }
                rects.push(
                    commands
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(m.gutter_w + m.pad + (a - row.start_col) as f32 * m.char_w),
                                top: Val::Px(k as f32 * m.line_h),
                                width: Val::Px(w),
                                height: Val::Px(m.line_h),
                                ..default()
                            },
                            BackgroundColor(rgba(sp.selection)),
                            bevy::ui::FocusPolicy::Pass,
                            Name::new("code-selection"),
                        ))
                        .id(),
                );
            }
            commands.entity(body).add_children(&rects);
        } else if let Some((a, b)) = bracket_match(&ed) {
            // ---- Matching-bracket highlight (only when there's no selection).
            let mut brackets: Vec<Entity> = Vec::new();
            for (bl, bc) in [a, b] {
                if let Some((k, x_col)) = locate_in_view(&rows, start, end, bl, bc) {
                    brackets.push(
                        commands
                            .spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(m.gutter_w + m.pad + x_col as f32 * m.char_w),
                                    top: Val::Px(k as f32 * m.line_h),
                                    width: Val::Px(m.char_w),
                                    height: Val::Px(m.line_h),
                                    ..default()
                                },
                                BackgroundColor(rgba(sp.bracket_match)),
                                bevy::ui::FocusPolicy::Pass,
                                Name::new("code-bracket-match"),
                            ))
                            .id(),
                    );
                }
            }
            commands.entity(body).add_children(&brackets);
        }

        // ---- The visible rows themselves (front).
        let show_ws = ed.show_whitespace;
        let mut row_ents: Vec<Entity> = Vec::with_capacity(end - start);
        for row in &rows[start..end] {
            let full_spans = spans_of(row.line);
            let slice = slice_spans(full_spans, row.start_col, row.end_col);
            let foldable = row.first && edit::is_line_foldable(&ed, row.line);
            let folded = edit::is_folded(&ed, row.line);
            let ws = if show_ws {
                Some(whitespace_overlay(&ed.text[row.line], row.start_col, row.end_col))
            } else {
                None
            };
            row_ents.push(render_row(
                &mut commands,
                &fonts,
                m,
                entity,
                RowRender {
                    number: if row.first { Some(row.line + 1) } else { None },
                    line: row.line,
                    foldable,
                    folded,
                    fold_badge: row.fold_header,
                    spans: &slice,
                    whitespace: ws,
                    normal: rgb(sp.normal),
                    line_number: rgb(sp.line_number),
                    fold_color: rgb(sp.comment),
                },
            ));
        }
        commands.entity(body).add_children(&row_ents);
    }
}

/// Find `(line, col)` among the visible rows, returning `(row_offset_from_start,
/// x_col_within_row)` when it's on screen.
fn locate_in_view(
    rows: &[layout::VisualRow],
    start: usize,
    end: usize,
    line: usize,
    col: usize,
) -> Option<(usize, usize)> {
    for (k, row) in rows[start..end].iter().enumerate() {
        if row.line == line && col >= row.start_col && col < row.end_col {
            return Some((k, col - row.start_col));
        }
    }
    None
}

/// Slice a line's colored spans to the character range `[a, b)` (for a wrapped
/// row that shows only part of the line).
fn slice_spans(spans: &[(String, Color)], a: usize, b: usize) -> Vec<(String, Color)> {
    if a == 0 && spans.iter().map(|(s, _)| s.chars().count()).sum::<usize>() <= b {
        return spans.to_vec(); // whole line — the common (no-wrap) case
    }
    let mut out = Vec::new();
    let mut off = 0usize;
    for (s, c) in spans {
        let len = s.chars().count();
        let seg_a = off;
        let seg_b = off + len;
        let ia = a.max(seg_a);
        let ib = b.min(seg_b);
        if ia < ib {
            let sub: String = s.chars().skip(ia - seg_a).take(ib - ia).collect();
            out.push((sub, *c));
        }
        off = seg_b;
        if off >= b {
            break;
        }
    }
    out
}

/// Build the whitespace-marker string for a row slice: spaces → `·`, tabs → `→`,
/// everything else → blank. Monospace makes this align exactly under the text.
fn whitespace_overlay(line: &str, a: usize, b: usize) -> String {
    line.chars()
        .skip(a)
        .take(b - a)
        .map(|c| match c {
            ' ' => '\u{00B7}',   // middle dot
            '\t' => '\u{2192}',  // rightwards arrow
            _ => ' ',
        })
        .collect()
}

struct RowRender<'a> {
    number: Option<usize>,
    line: usize,
    foldable: bool,
    folded: bool,
    fold_badge: bool,
    spans: &'a [(String, Color)],
    whitespace: Option<String>,
    normal: Color,
    line_number: Color,
    fold_color: Color,
}

fn render_row(commands: &mut Commands, fonts: &EmberFonts, m: Metrics, editor: Entity, r: RowRender<'_>) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                height: Val::Px(m.line_h),
                align_items: AlignItems::Center,
                position_type: PositionType::Relative,
                ..default()
            },
            Name::new("code-line"),
        ))
        .id();

    // Gutter = fold-chevron slot + right-aligned line number.
    let gutter = commands
        .spawn((Node {
            width: Val::Px(m.gutter_w),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            ..default()
        },))
        .id();
    let chevron_slot = commands
        .spawn((Node {
            width: Val::Px(FOLD_COL_W),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },))
        .id();
    if r.foldable {
        let icon = icon_text(
            commands,
            &fonts.phosphor,
            if r.folded { "caret-right" } else { "caret-down" },
            (
                (r.fold_color.to_srgba().red * 255.0) as u8,
                (r.fold_color.to_srgba().green * 255.0) as u8,
                (r.fold_color.to_srgba().blue * 255.0) as u8,
            ),
            m.gutter_size,
        );
        commands.entity(icon).insert((Interaction::default(), CodeFoldToggle { editor, line: r.line }));
        commands.entity(chevron_slot).add_child(icon);
    }
    let number_box = commands
        .spawn((Node {
            flex_grow: 1.0,
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::FlexEnd,
            padding: UiRect::right(Val::Px(6.0)),
            ..default()
        },))
        .id();
    if let Some(num) = r.number {
        let t = commands
            .spawn((Text::new(format!("{num}")), mono(&fonts.mono, m.gutter_size), TextColor(r.line_number)))
            .id();
        commands.entity(number_box).add_child(t);
    }
    commands.entity(gutter).add_children(&[chevron_slot, number_box]);

    // Whitespace overlay sits under the text at the same x (monospace-aligned).
    if let Some(ws) = r.whitespace {
        if !ws.trim().is_empty() {
            let overlay = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(m.gutter_w + m.pad),
                        top: Val::Px(0.0),
                        height: Val::Px(m.line_h),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    Text::new(ws),
                    mono(&fonts.mono, m.font_size),
                    TextColor(rgba(syntax_palette().indent_guide)),
                    TextLayout::no_wrap(),
                    bevy::ui::FocusPolicy::Pass,
                    Name::new("code-whitespace"),
                ))
                .id();
            commands.entity(row).add_child(overlay);
        }
    }

    let line_text = commands
        .spawn((
            Text::new(""),
            mono(&fonts.mono, m.font_size),
            TextColor(r.normal),
            TextLayout::no_wrap(),
            Node { padding: UiRect::left(Val::Px(m.pad)), ..default() },
        ))
        .id();
    let mut span_ents: Vec<Entity> = r
        .spans
        .iter()
        .map(|(s, color)| {
            commands.spawn((TextSpan::new(s.clone()), mono(&fonts.mono, m.font_size), TextColor(*color))).id()
        })
        .collect();
    // `⋯` badge marking a collapsed region.
    if r.fold_badge {
        span_ents.push(
            commands
                .spawn((TextSpan::new("  \u{22EF}".to_string()), mono(&fonts.mono, m.font_size), TextColor(r.fold_color)))
                .id(),
        );
    }
    commands.entity(line_text).add_children(&span_ents);
    commands.entity(row).add_children(&[gutter, line_text]);
    row
}

/// Repaint editors when the syntax palette changes (live theme edits).
pub(crate) fn code_theme_watch(
    mut last: Local<Option<SyntaxPalette>>,
    mut editors: Query<&mut CodeEditor>,
    mut bg: Query<&mut BackgroundColor>,
) {
    let sp = syntax_palette();
    if *last == Some(sp) {
        return;
    }
    *last = Some(sp);
    for mut ed in &mut editors {
        ed.dirty = true;
        if let Ok(mut c) = bg.get_mut(ed.caret) {
            c.0 = rgb(sp.cursor);
        }
    }
}

pub(crate) fn code_caret(time: Res<Time>, editors: Query<&CodeEditor>, mut nodes: Query<&mut Node>) {
    let on = (time.elapsed_secs() * 1.6).fract() < 0.5;
    for ed in &editors {
        let Ok(mut n) = nodes.get_mut(ed.caret) else {
            continue;
        };
        let m = ed.metrics();
        let rows = ed.rows();
        let cr = layout::row_of(&rows, ed.cursor_line, ed.cursor_col);
        let on_screen = cr >= ed.scroll && cr < ed.scroll + ed.visible;
        if ed.focused && on && on_screen && m.char_w > 0.0 {
            let x_col = ed.cursor_col.saturating_sub(rows[cr].start_col);
            n.display = Display::Flex;
            n.height = Val::Px(m.caret_h);
            n.left = Val::Px(m.gutter_w + m.pad + x_col as f32 * m.char_w);
            n.top = Val::Px((cr - ed.scroll) as f32 * m.line_h + (m.line_h - m.caret_h) / 2.0);
        } else {
            n.display = Display::None;
        }
    }
}
