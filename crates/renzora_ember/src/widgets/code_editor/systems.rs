//! The code-editor ECS systems: measuring, focus/click, keyboard, scroll,
//! re-rendering the visible rows, and the blinking caret. Everything that draws
//! or hit-tests goes through the [`super::layout`] visual-row model, so folding
//! and word wrap are handled uniformly.

use std::hash::{DefaultHasher, Hash, Hasher};

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
    mono, CodeEditor, CodeFoldToggle, CodeProbe, CodeScrollTrack, CodeViewport, Metrics,
    RenderedRow, FOLD_COL_W, PAD, PROBE_LEN, TAB_WIDTH,
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

/// Size and place the scrollbar thumb from the editor's scroll position.
///
/// The whole bar hides when the file fits, which is most files most of the time
/// — a full-height thumb that can never move is decoration, and it would sit
/// over the last column of every short file.
///
/// Positioned in the track's *free* space (`track − thumb`) rather than by
/// scrolling a proportion of the track: at the bottom of the file the thumb has
/// to end flush with the track, and scaling by `scroll/max` alone leaves it
/// short by its own height.
pub(crate) fn code_scrollbar_sync(
    tracks: Query<(Entity, &CodeScrollTrack, &ComputedNode, &Children)>,
    editors: Query<&CodeEditor>,
    mut nodes: Query<&mut Node>,
) {
    for (track_e, track, cn, kids) in &tracks {
        let Ok(ed) = editors.get(track.editor) else {
            continue;
        };
        let total = ed.rows().len();
        let visible = ed.visible.max(1);
        let show = total > visible;

        if let Ok(mut n) = nodes.get_mut(track_e) {
            let want = if show { Display::Flex } else { Display::None };
            if n.display != want {
                n.display = want;
            }
        }
        if !show {
            continue;
        }

        let track_h = cn.size().y * cn.inverse_scale_factor();
        // Floored at 24px so a very long file still leaves something grabbable
        // to look at rather than a one-pixel tick.
        let thumb_h = (track_h * visible as f32 / total as f32).max(24.0).min(track_h);
        let max_scroll = total.saturating_sub(visible).max(1) as f32;
        let t = (ed.scroll as f32 / max_scroll).clamp(0.0, 1.0);
        let top = (track_h - thumb_h) * t;

        for kid in kids.iter() {
            if let Ok(mut n) = nodes.get_mut(kid) {
                n.height = Val::Px(thumb_h);
                n.top = Val::Px(top);
            }
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

/// Rebuild the visible rows — **incrementally**.
///
/// This used to despawn every child of the body and respawn the lot on any
/// frame the editor was dirty. That is fine for a one-off repaint and ruinous
/// while a key is held: a typed or deleted character dirties the editor every
/// frame, so ~30 rows × (gutter + number + text + one entity per token span)
/// were destroyed and recreated 60 times a second, and bevy_ui had to re-run
/// taffy over the whole body and re-shape every line of text with it. It cost
/// enough to drag the editor to ~25 FPS while holding Delete (issue #84
/// follow-up).
///
/// An edit only changes *one* row, so each row now carries a hash of everything
/// it draws from ([`RowRender::sig`]) and a row whose hash is unchanged is left
/// completely alone — no despawn, no respawn, no relayout, no re-shape. The
/// per-row overlays (current-line highlight, indent guides, selection, bracket
/// match) moved inside the row entity for the same reason: as body-level
/// absolute overlays they had to be rebuilt whenever *any* row changed. Rows are
/// only ever appended to or popped from the tail, so the child order stays the
/// visual order without `insert_children` (which can panic when it reorders —
/// see the `place` bug worked around in `reactive.rs`).
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

        // If anything despawned our rows behind our back (a panel teardown),
        // drop the whole record and rebuild from scratch rather than issuing
        // commands against dead entities.
        if ed.rendered.iter().any(|r| commands.get_entity(r.entity).is_err()) {
            ed.rendered.clear();
            if let Ok(kids) = children.get(body) {
                for c in kids.iter() {
                    commands.entity(c).try_despawn();
                }
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

        // ---- What every row draws this frame, resolved but not yet spawned.
        let colors = RowColors {
            normal: rgb(sp.normal),
            line_number: rgb(sp.line_number),
            fold: rgb(sp.comment),
            guide: rgba(sp.indent_guide),
            current: rgba(sp.current_line),
            selection: rgba(sp.selection),
            bracket: rgba(sp.bracket_match),
        };
        // Shared inputs (zoom, theme, viewport width): a change here invalidates
        // every row at once, so they're folded into one value each row's hash
        // starts from instead of being repeated in it.
        let epoch = render_epoch(m, &colors, ed.view_w, ed.show_whitespace);

        let selection = has_selection(&ed).then(|| sel_range(&ed));
        let bracket = if selection.is_none() { bracket_match(&ed) } else { None };
        let show_ws = ed.show_whitespace;

        let mut plans: Vec<RowRender> = Vec::with_capacity(end - start);
        for row in &rows[start..end] {
            let slice = slice_spans(spans_of(row.line), row.start_col, row.end_col);

            // Indent guides for this line's leading whitespace (x offsets into
            // the row, which starts at the body's left edge).
            let mut cols = 0usize;
            for c in ed.text[row.line].chars() {
                match c {
                    ' ' => cols += 1,
                    '\t' => cols += TAB_WIDTH,
                    _ => break,
                }
            }
            let guides = (1..(cols / TAB_WIDTH))
                .map(|i| m.gutter_w + m.pad + (i * TAB_WIDTH) as f32 * m.char_w)
                .collect();

            // This row's slice of the selection, as (left, width).
            let mut sel_rect = None;
            if let Some(((sl, sc), (el, ec))) = selection {
                if row.line >= sl && row.line <= el {
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
                    if w > 0.0 {
                        sel_rect = Some((m.gutter_w + m.pad + (a - row.start_col) as f32 * m.char_w, w));
                    }
                }
            }

            // Matching-bracket cells that land on this row.
            let mut brackets: Vec<f32> = Vec::new();
            if let Some((a, b)) = bracket {
                for (bl, bc) in [a, b] {
                    if bl == row.line && bc >= row.start_col && bc < row.end_col {
                        brackets.push(m.gutter_w + m.pad + (bc - row.start_col) as f32 * m.char_w);
                    }
                }
            }

            plans.push(RowRender {
                number: if row.first { Some(row.line + 1) } else { None },
                line: row.line,
                foldable: row.first && edit::is_line_foldable(&ed, row.line),
                folded: edit::is_folded(&ed, row.line),
                fold_badge: row.fold_header,
                spans: slice,
                whitespace: show_ws
                    .then(|| whitespace_overlay(&ed.text[row.line], row.start_col, row.end_col)),
                current_line: row.line == ed.cursor_line && ed.view_w > 0.0,
                current_w: ed.view_w,
                guides,
                selection: sel_rect,
                brackets,
            });
        }

        // ---- Reconcile against what's already on screen.
        // Tail first: rows that scrolled out of existence.
        let keep = plans.len().min(ed.rendered.len());
        for gone in ed.rendered.split_off(keep) {
            if let Ok(mut e) = commands.get_entity(gone.entity) {
                e.try_despawn();
            }
        }
        for (k, plan) in plans.iter().enumerate() {
            let sig = plan.sig(epoch);
            match ed.rendered.get(k).copied() {
                // Already correct on screen — the whole point of this pass.
                Some(prev) if prev.sig == sig => {}
                Some(prev) => {
                    if let Ok(kids) = children.get(prev.entity) {
                        for c in kids.iter() {
                            commands.entity(c).try_despawn();
                        }
                    }
                    let kids = row_children(&mut commands, &fonts, m, colors, entity, plan);
                    commands.entity(prev.entity).insert(row_node(m)).replace_children(&kids);
                    ed.rendered[k].sig = sig;
                }
                None => {
                    let row = commands.spawn((row_node(m), Name::new("code-line"))).id();
                    let kids = row_children(&mut commands, &fonts, m, colors, entity, plan);
                    commands.entity(row).add_children(&kids);
                    commands.entity(body).add_children(&[row]);
                    ed.rendered.push(RenderedRow { entity: row, sig });
                }
            }
        }
    }
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

/// The palette colors a row draws with. Constant across a render pass (and
/// folded into [`render_epoch`]), so they're passed alongside the per-row plan
/// rather than stored in — and hashed by — every row.
#[derive(Clone, Copy)]
struct RowColors {
    normal: Color,
    line_number: Color,
    fold: Color,
    guide: Color,
    current: Color,
    selection: Color,
    bracket: Color,
}

/// Everything one visual row draws. Hashed by [`RowRender::sig`] so an unchanged
/// row can be skipped outright — see [`code_render`].
struct RowRender {
    number: Option<usize>,
    line: usize,
    foldable: bool,
    folded: bool,
    fold_badge: bool,
    spans: Vec<(String, Color)>,
    whitespace: Option<String>,
    /// Draw the full-width current-line highlight behind this row.
    current_line: bool,
    /// Width of that highlight (the viewport width).
    current_w: f32,
    /// x offsets of this row's indent guides.
    guides: Vec<f32>,
    /// This row's slice of the selection, as `(left, width)`.
    selection: Option<(f32, f32)>,
    /// x offsets of matching-bracket cells landing on this row.
    brackets: Vec<f32>,
}

fn hash_f32(f: f32, h: &mut DefaultHasher) {
    f.to_bits().hash(h);
}

fn hash_color(c: Color, h: &mut DefaultHasher) {
    let s = c.to_srgba();
    for ch in [s.red, s.green, s.blue, s.alpha] {
        hash_f32(ch, h);
    }
}

/// Hash of the inputs shared by every row (metrics, palette, viewport width).
/// Seeding each row's signature with this invalidates them all together when the
/// zoom or theme changes, without repeating those fields per row.
fn render_epoch(m: Metrics, c: &RowColors, view_w: f32, show_ws: bool) -> u64 {
    let mut h = DefaultHasher::new();
    for f in [m.font_size, m.gutter_size, m.line_h, m.caret_h, m.char_w, m.gutter_w, m.pad, view_w] {
        hash_f32(f, &mut h);
    }
    for col in [c.normal, c.line_number, c.fold, c.guide, c.current, c.selection, c.bracket] {
        hash_color(col, &mut h);
    }
    show_ws.hash(&mut h);
    h.finish()
}

impl RowRender {
    fn sig(&self, epoch: u64) -> u64 {
        let mut h = DefaultHasher::new();
        epoch.hash(&mut h);
        (self.number, self.line, self.foldable, self.folded, self.fold_badge).hash(&mut h);
        self.whitespace.hash(&mut h);
        (self.current_line, self.selection.is_some()).hash(&mut h);
        hash_f32(self.current_w, &mut h);
        for (text, color) in &self.spans {
            text.hash(&mut h);
            hash_color(*color, &mut h);
        }
        for x in self.guides.iter().chain(self.brackets.iter()) {
            hash_f32(*x, &mut h);
        }
        if let Some((left, w)) = self.selection {
            hash_f32(left, &mut h);
            hash_f32(w, &mut h);
        }
        h.finish()
    }
}

fn row_node(m: Metrics) -> Node {
    Node {
        flex_direction: FlexDirection::Row,
        height: Val::Px(m.line_h),
        align_items: AlignItems::Center,
        position_type: PositionType::Relative,
        ..default()
    }
}

/// Spawn one row's children, back to front: the absolutely-positioned overlays
/// (current line, indent guides, selection, bracket match, whitespace markers)
/// first so they paint behind the in-flow gutter and text. They belong to the
/// row rather than the body so a row that didn't change needs no work at all.
fn row_children(
    commands: &mut Commands,
    fonts: &EmberFonts,
    m: Metrics,
    colors: RowColors,
    editor: Entity,
    r: &RowRender,
) -> Vec<Entity> {
    let mut kids: Vec<Entity> = Vec::new();

    let overlay = |commands: &mut Commands, left: f32, width: f32, color: Color, name: &'static str| {
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(left),
                    top: Val::Px(0.0),
                    width: Val::Px(width),
                    height: Val::Px(m.line_h),
                    ..default()
                },
                BackgroundColor(color),
                bevy::ui::FocusPolicy::Pass,
                Name::new(name),
            ))
            .id()
    };

    if r.current_line {
        kids.push(overlay(commands, 0.0, r.current_w, colors.current, "code-current-line"));
    }
    for x in &r.guides {
        kids.push(overlay(commands, *x, 1.0, colors.guide, "code-indent-guide"));
    }
    if let Some((left, w)) = r.selection {
        kids.push(overlay(commands, left, w, colors.selection, "code-selection"));
    }
    for x in &r.brackets {
        kids.push(overlay(commands, *x, m.char_w, colors.bracket, "code-bracket-match"));
    }

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
                (colors.fold.to_srgba().red * 255.0) as u8,
                (colors.fold.to_srgba().green * 255.0) as u8,
                (colors.fold.to_srgba().blue * 255.0) as u8,
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
            .spawn((Text::new(format!("{num}")), mono(&fonts.mono, m.gutter_size), TextColor(colors.line_number)))
            .id();
        commands.entity(number_box).add_child(t);
    }
    commands.entity(gutter).add_children(&[chevron_slot, number_box]);

    // Whitespace markers sit under the text at the same x (monospace-aligned).
    if let Some(ws) = r.whitespace.as_ref().filter(|ws| !ws.trim().is_empty()) {
        kids.push(
            commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(m.gutter_w + m.pad),
                        top: Val::Px(0.0),
                        height: Val::Px(m.line_h),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    Text::new(ws.clone()),
                    mono(&fonts.mono, m.font_size),
                    TextColor(colors.guide),
                    TextLayout::no_wrap(),
                    bevy::ui::FocusPolicy::Pass,
                    Name::new("code-whitespace"),
                ))
                .id(),
        );
    }

    let line_text = commands
        .spawn((
            Text::new(""),
            mono(&fonts.mono, m.font_size),
            TextColor(colors.normal),
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
                .spawn((TextSpan::new("  \u{22EF}".to_string()), mono(&fonts.mono, m.font_size), TextColor(colors.fold)))
                .id(),
        );
    }
    commands.entity(line_text).add_children(&span_ents);
    kids.extend([gutter, line_text]);
    kids
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
        // Assign through `set_if_neq`-style guards: this runs every frame, and a
        // plain write marks the node changed, which makes bevy_ui re-run layout
        // on it (and its ancestors) 60 times a second for a caret that hasn't
        // moved. Only the blink phase should normally cost anything.
        let want = if ed.focused && on && on_screen && m.char_w > 0.0 {
            let x_col = ed.cursor_col.saturating_sub(rows[cr].start_col);
            Some((
                Val::Px(m.caret_h),
                Val::Px(m.gutter_w + m.pad + x_col as f32 * m.char_w),
                Val::Px((cr - ed.scroll) as f32 * m.line_h + (m.line_h - m.caret_h) / 2.0),
            ))
        } else {
            None
        };
        match want {
            Some((height, left, top)) => {
                if n.display != Display::Flex || n.height != height || n.left != left || n.top != top {
                    let n = &mut *n;
                    n.display = Display::Flex;
                    n.height = height;
                    n.left = left;
                    n.top = top;
                }
            }
            None => {
                if n.display != Display::None {
                    n.display = Display::None;
                }
            }
        }
    }
}

/// Drive the editor's real systems headlessly so a key storm can be replayed in
/// a test. The widget spawns plain components (no layout/render needed), so an
/// `App` with `MinimalPlugins` + `InputPlugin` exercises input → edit → render
/// end-to-end — the path a held key actually takes.
#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::input::keyboard::{Key, KeyboardInput};
    use bevy::input::{ButtonState, InputPlugin};
    use bevy::text::FontSource;

    use super::super::CodeToken;

    /// The reporter's buffer (issue #84): a `function`/`end` block with the caret
    /// parked left of the body text.
    const SRC: &str = "\nfunction on_update(delta)\n    return 0.0 * delta\nend";

    fn fonts() -> EmberFonts {
        EmberFonts {
            ui: FontSource::SansSerif,
            phosphor: Handle::default(),
            mono: FontSource::Monospace,
            default_ui: FontSource::SansSerif,
            default_mono: FontSource::Monospace,
        }
    }

    fn press(logical: Key, code: KeyCode) -> KeyboardInput {
        KeyboardInput {
            key_code: code,
            logical_key: logical,
            state: ButtonState::Pressed,
            text: None,
            repeat: true,
            window: Entity::PLACEHOLDER,
        }
    }

    fn harness(text: &str) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, InputPlugin));
        app.insert_resource(fonts());
        app.add_systems(Update, (code_metrics, code_input, code_render, code_caret).chain());
        let owned = text.to_string();
        let editor = app
            .world_mut()
            .run_system_once(move |mut c: Commands| super::super::code_editor(&mut c, &owned))
            .unwrap();
        app.update();
        (app, editor)
    }

    /// Put the caret where the reporter had it and hold the key down.
    fn hold(app: &mut App, editor: Entity, logical: Key, code: KeyCode, presses_per_frame: usize) {
        {
            let mut ed = app.world_mut().get_mut::<CodeEditor>(editor).unwrap();
            ed.focused = true;
            ed.visible = 20;
            ed.cursor_line = 2;
            ed.cursor_col = 4;
            ed.anchor_line = 2;
            ed.anchor_col = 4;
        }
        for _ in 0..200 {
            for _ in 0..presses_per_frame {
                app.world_mut().write_message(press(logical.clone(), code));
            }
            app.update();
        }
    }

    /// The row entities under the body, each with its own children — the shape a
    /// repaint would churn.
    fn drawn(app: &mut App, editor: Entity) -> Vec<(Entity, Vec<Entity>)> {
        let body = app.world().get::<CodeEditor>(editor).unwrap().body;
        let world = app.world();
        let Some(rows) = world.get::<Children>(body) else {
            return Vec::new();
        };
        rows.iter()
            .map(|r| {
                let kids = world.get::<Children>(r).map(|c| c.iter().collect()).unwrap_or_default();
                (r, kids)
            })
            .collect()
    }

    #[test]
    fn holding_delete_does_not_panic() {
        let (mut app, editor) = harness(SRC);
        hold(&mut app, editor, Key::Delete, KeyCode::Delete, 1);
    }

    /// The FPS half of issue #84: a keystroke must not rebuild the whole view.
    /// Every row but the edited one keeps its exact entities, so bevy_ui has
    /// nothing to relayout and bevy_text nothing to re-shape for them.
    #[test]
    fn an_edit_rebuilds_only_the_edited_row() {
        let (mut app, editor) = harness(SRC);
        {
            let mut ed = app.world_mut().get_mut::<CodeEditor>(editor).unwrap();
            ed.focused = true;
            ed.visible = 20;
            ed.cursor_line = 2;
            ed.cursor_col = 4;
            ed.anchor_line = 2;
            ed.anchor_col = 4;
            // `code_measure` normally flags this; there's no layout here.
            ed.dirty = true;
        }
        app.update();
        let before = drawn(&mut app, editor);
        assert_eq!(before.len(), 4, "one row per line of SRC");

        app.world_mut().write_message(press(Key::Delete, KeyCode::Delete));
        app.update();
        let after = drawn(&mut app, editor);

        let row_ents: Vec<Entity> = before.iter().map(|(e, _)| *e).collect();
        assert_eq!(row_ents, after.iter().map(|(e, _)| *e).collect::<Vec<_>>(), "row entities must be reused");
        let rebuilt: Vec<usize> = before
            .iter()
            .zip(&after)
            .enumerate()
            .filter(|(_, ((_, a), (_, b)))| a != b)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(rebuilt, vec![2], "only the edited row's contents should be respawned");
    }

    /// A repaint that resolves to the same picture (theme tick, a stray dirty
    /// flag) must not touch a single entity.
    #[test]
    fn an_unchanged_repaint_touches_nothing() {
        let (mut app, editor) = harness(SRC);
        app.update();
        let before = drawn(&mut app, editor);
        app.world_mut().get_mut::<CodeEditor>(editor).unwrap().dirty = true;
        app.update();
        assert_eq!(before, drawn(&mut app, editor));
    }

    /// Scrolling reuses the row entities too — only their contents change.
    #[test]
    fn scrolling_reuses_the_row_entities() {
        let (mut app, editor) = harness("a\nb\nc\nd\ne\nf\ng\nh");
        {
            let mut ed = app.world_mut().get_mut::<CodeEditor>(editor).unwrap();
            ed.visible = 3;
            ed.dirty = true;
        }
        app.update();
        let before = drawn(&mut app, editor);
        {
            let mut ed = app.world_mut().get_mut::<CodeEditor>(editor).unwrap();
            ed.scroll = 3;
            ed.dirty = true;
        }
        app.update();
        let after = drawn(&mut app, editor);
        assert_eq!(before.len(), after.len());
        assert_eq!(
            before.iter().map(|(e, _)| *e).collect::<Vec<_>>(),
            after.iter().map(|(e, _)| *e).collect::<Vec<_>>(),
        );
    }

    #[test]
    fn holding_delete_with_repeats_per_frame_does_not_panic() {
        let (mut app, editor) = harness(SRC);
        hold(&mut app, editor, Key::Delete, KeyCode::Delete, 5);
    }

    #[test]
    fn holding_backspace_does_not_panic() {
        let (mut app, editor) = harness(SRC);
        hold(&mut app, editor, Key::Backspace, KeyCode::Backspace, 3);
    }

    #[test]
    fn holding_delete_while_wrapped_does_not_panic() {
        let (mut app, editor) = harness(SRC);
        {
            let mut ed = app.world_mut().get_mut::<CodeEditor>(editor).unwrap();
            ed.wrap = true;
            ed.wrap_cols = 6;
        }
        hold(&mut app, editor, Key::Delete, KeyCode::Delete, 2);
    }

    #[test]
    fn holding_delete_with_a_host_highlighter_does_not_panic() {
        let (mut app, editor) = harness(SRC);
        {
            let mut ed = app.world_mut().get_mut::<CodeEditor>(editor).unwrap();
            // Mimic the host tokenizer's contract: byte-length runs covering the
            // whole line (words vs the rest).
            ed.highlighter = Some(Box::new(|line: &str, st: u32| {
                let mut toks = Vec::new();
                let bytes = line.as_bytes();
                let mut i = 0;
                while i < bytes.len() {
                    let start = i;
                    let word = bytes[i].is_ascii_alphanumeric();
                    while i < bytes.len() && bytes[i].is_ascii_alphanumeric() == word {
                        i += 1;
                    }
                    toks.push(CodeToken { len: i - start, color: Color::WHITE });
                }
                (toks, st)
            }));
        }
        hold(&mut app, editor, Key::Delete, KeyCode::Delete, 2);
    }

    #[test]
    fn holding_delete_on_a_folded_buffer_does_not_panic() {
        let (mut app, editor) = harness(SRC);
        {
            let mut ed = app.world_mut().get_mut::<CodeEditor>(editor).unwrap();
            edit::toggle_fold(&mut ed, 1);
        }
        hold(&mut app, editor, Key::Delete, KeyCode::Delete, 2);
    }
}
