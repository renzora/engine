//! The code-editor ECS systems: measuring, focus/click, keyboard, scroll,
//! re-rendering the visible lines, and the blinking caret.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::MouseWheel;
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::text::{FontSize, TextLayoutInfo};
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use crate::font::EmberFonts;
use crate::theme::*;

use super::edit::{bracket_match, edit, has_selection, sel_range};
use super::highlight::tokenize;
use super::{mono, CodeEditor, CodeProbe, CodeViewport, Metrics, PAD, PROBE_LEN, TAB_WIDTH};

/// Recompute each editor's derived metrics; flag a re-render when they change
/// (the zoom level moved, or the line count crossed a gutter digit boundary).
pub(crate) fn code_metrics(mut editors: Query<&mut CodeEditor>) {
    for mut ed in &mut editors {
        if ed.recompute_metrics() {
            ed.dirty = true;
        }
    }
}

/// Measure the active mono font's real advance from a hidden probe, so carets
/// land correctly for any monospace font — not only 0.6em ones. Keeps the
/// probe's font + size matched to the editor, then reads its laid-out width.
/// Scale-invariant and tightly guarded: a bad/early measurement is ignored and
/// the editor keeps the [`super::DEFAULT_ADVANCE`] fallback (no regression).
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
        // Keep the probe matched to the editor's font + size; remeasure next
        // frame once the layout reflects the change.
        let want_size = FontSize::Px(ed.font_size);
        if tf.font != fonts.mono || tf.font_size != want_size {
            tf.font = fonts.mono.clone();
            tf.font_size = want_size;
            continue;
        }
        // `info.size` is physical px; compute the em-ratio in a scale-invariant
        // way (physical advance / physical em) so the result is independent of
        // the window scale factor.
        let sf = if info.scale_factor > 0.0 { info.scale_factor } else { 1.0 };
        let em_physical = ed.font_size * sf;
        if em_physical <= 0.0 {
            continue;
        }
        let ratio = (info.size.x / PROBE_LEN) / em_physical;
        // Only accept values in a sane monospace band (rejects empty layouts,
        // fallback fonts, and any unit mismatch).
        if (0.50..=0.72).contains(&ratio) && (ratio - ed.advance_ratio).abs() > 0.001 {
            ed.advance_ratio = ratio;
            ed.dirty = true;
        }
    }
}

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
            // Full-width overlays (current-line highlight) need the viewport
            // width; re-render when it changes (panel resize).
            if (ed.view_w - size.x).abs() > 0.5 {
                ed.view_w = size.x;
                ed.dirty = true;
            }
        }
    }
}

/// Click to focus + place the cursor; drag (or shift-click) to select.
pub(crate) fn code_pointer(
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
        let hit = if just {
            *interaction == Interaction::Pressed
        } else {
            rcp.cursor_over
        };
        if hit {
            if let Some(nrm) = rcp.normalized {
                let size = cn.size() * cn.inverse_scale_factor();
                target = Some((
                    vp.editor,
                    Vec2::new((nrm.x + 0.5) * size.x, (nrm.y + 0.5) * size.y),
                ));
            }
            break;
        }
    }
    let Some((editor, local)) = target else {
        return;
    };
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
        let line = (ed.scroll + (local.y / ed.line_h) as usize).min(ed.text.len().saturating_sub(1));
        let col = (((local.x - ed.gutter_w - PAD) / ed.char_w).round().max(0.0)) as usize;
        ed.cursor_line = line;
        ed.cursor_col = col.min(ed.text[line].chars().count());
        if just && !shift {
            ed.anchor_line = ed.cursor_line;
            ed.anchor_col = ed.cursor_col;
        }
        ed.dirty = true;
    }
}

pub(crate) fn code_input(
    mut events: MessageReader<KeyboardInput>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut editors: Query<&mut CodeEditor>,
) {
    let keys: Vec<Key> = events
        .read()
        .filter(|e| e.state == ButtonState::Pressed)
        .map(|e| e.logical_key.clone())
        .collect();
    if keys.is_empty() {
        return;
    }
    // Don't type into the buffer while a Ctrl/Super chord is held (Ctrl+S save,
    // Ctrl+C/V, …) — those are shortcuts owned by the host, not text input.
    // AltGr (Ctrl+Alt) still produces characters, so allow it through.
    let alt = keyboard.pressed(KeyCode::AltLeft) || keyboard.pressed(KeyCode::AltRight);
    let cmd = keyboard.pressed(KeyCode::ControlLeft)
        || keyboard.pressed(KeyCode::ControlRight)
        || keyboard.pressed(KeyCode::SuperLeft)
        || keyboard.pressed(KeyCode::SuperRight);
    if cmd && !alt {
        return;
    }
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    for mut editor in &mut editors {
        if !editor.focused {
            continue;
        }
        let ed = &mut *editor;
        for key in &keys {
            edit(ed, key, shift);
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
                let max = ed.text.len().saturating_sub(1) as i32;
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
    mut editors: Query<&mut CodeEditor>,
    children: Query<&Children>,
) {
    let Some(fonts) = fonts else {
        return;
    };
    for mut ed in &mut editors {
        if !ed.dirty || ed.char_w <= 0.0 {
            continue;
        }
        ed.dirty = false;
        let m = ed.metrics();
        let body = ed.body;
        if let Ok(kids) = children.get(body) {
            for c in kids.iter() {
                commands.entity(c).despawn();
            }
        }
        let start = ed.scroll;
        let end = (start + ed.visible + 1).min(ed.text.len());

        // Underlay (added first → furthest back): current-line highlight, then
        // indent guides on top of it.
        let mut underlay: Vec<Entity> = Vec::new();
        if ed.cursor_line >= start && ed.cursor_line < end && ed.view_w > 0.0 {
            let rect = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px((ed.cursor_line - start) as f32 * m.line_h),
                        width: Val::Px(ed.view_w),
                        height: Val::Px(m.line_h),
                        ..default()
                    },
                    BackgroundColor(rgba(syntax_palette().current_line)),
                    bevy::ui::FocusPolicy::Pass,
                    Name::new("code-current-line"),
                ))
                .id();
            underlay.push(rect);
        }
        let guide_color = rgba(syntax_palette().indent_guide);
        for l in start..end {
            // Leading-whitespace columns (tab counts as TAB_WIDTH); each full
            // TAB_WIDTH is one indent level.
            let mut cols = 0usize;
            for c in ed.text[l].chars() {
                match c {
                    ' ' => cols += 1,
                    '\t' => cols += TAB_WIDTH,
                    _ => break,
                }
            }
            let level = cols / TAB_WIDTH;
            // Interior stops only (skip the gutter edge and the text start).
            for i in 1..level {
                let x = m.gutter_w + m.pad + (i * TAB_WIDTH) as f32 * m.char_w;
                let rect = commands
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(x),
                            top: Val::Px((l - start) as f32 * m.line_h),
                            width: Val::Px(1.0),
                            height: Val::Px(m.line_h),
                            ..default()
                        },
                        BackgroundColor(guide_color),
                        bevy::ui::FocusPolicy::Pass,
                        Name::new("code-indent-guide"),
                    ))
                    .id();
                underlay.push(rect);
            }
        }
        commands.entity(body).add_children(&underlay);

        // Selection highlight rects (added next → behind the text).
        let mut sel_rects: Vec<Entity> = Vec::new();
        if has_selection(&ed) {
            let ((sl, sc), (el, ec)) = sel_range(&ed);
            for l in sl.max(start)..=el.min(end.saturating_sub(1)) {
                let ca = if l == sl { sc } else { 0 };
                let cb = if l == el { ec } else { ed.text[l].chars().count() };
                let w = (cb.saturating_sub(ca) as f32 * m.char_w)
                    .max(if l < el { m.char_w * 0.5 } else { 0.0 });
                if w <= 0.0 {
                    continue;
                }
                let rect = commands
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(m.gutter_w + m.pad + ca as f32 * m.char_w),
                            top: Val::Px((l - start) as f32 * m.line_h),
                            width: Val::Px(w),
                            height: Val::Px(m.line_h),
                            ..default()
                        },
                        BackgroundColor(rgba(syntax_palette().selection)),
                        bevy::ui::FocusPolicy::Pass,
                        Name::new("code-selection"),
                    ))
                    .id();
                sel_rects.push(rect);
            }
        }
        commands.entity(body).add_children(&sel_rects);

        // Matching-bracket highlight (above the selection, below the text).
        if !has_selection(&ed) {
            if let Some((a, b)) = bracket_match(&ed) {
                let mut brackets: Vec<Entity> = Vec::new();
                for (bl, bc) in [a, b] {
                    if bl < start || bl >= end {
                        continue;
                    }
                    let rect = commands
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(m.gutter_w + m.pad + bc as f32 * m.char_w),
                                top: Val::Px((bl - start) as f32 * m.line_h),
                                width: Val::Px(m.char_w),
                                height: Val::Px(m.line_h),
                                ..default()
                            },
                            BackgroundColor(rgba(syntax_palette().bracket_match)),
                            bevy::ui::FocusPolicy::Pass,
                            Name::new("code-bracket-match"),
                        ))
                        .id();
                    brackets.push(rect);
                }
                commands.entity(body).add_children(&brackets);
            }
        }

        // Color the visible lines. With a host highlighter we thread the
        // cross-line state from the top of the file (cheap; only on `dirty`),
        // so block comments etc. resolve correctly when scrolled. Otherwise we
        // fall back to the built-in single-line Rust-ish tokenizer.
        let line_spans: Vec<Vec<(String, Color)>> = if let Some(hl) = ed.highlighter.as_ref() {
            let mut st = 0u32;
            for i in 0..start {
                st = hl(&ed.text[i], st).1;
            }
            (start..end)
                .map(|i| {
                    let (toks, ns) = hl(&ed.text[i], st);
                    st = ns;
                    let line = &ed.text[i];
                    let mut out: Vec<(String, Color)> = Vec::with_capacity(toks.len());
                    let mut off = 0usize;
                    for t in toks {
                        let end_b = (off + t.len).min(line.len());
                        if end_b > off && line.is_char_boundary(off) && line.is_char_boundary(end_b) {
                            out.push((line[off..end_b].to_string(), t.color));
                        }
                        off = end_b;
                    }
                    if off < line.len() && line.is_char_boundary(off) {
                        out.push((line[off..].to_string(), rgb(syntax_palette().normal)));
                    }
                    out
                })
                .collect()
        } else {
            (start..end)
                .map(|i| tokenize(&ed.text[i]).into_iter().map(|(s, c)| (s, rgb(c))).collect())
                .collect()
        };

        let rows: Vec<Entity> = line_spans
            .iter()
            .enumerate()
            .map(|(k, spans)| render_line(&mut commands, &fonts.mono, m, start + k + 1, spans))
            .collect();
        commands.entity(body).add_children(&rows);
    }
}

/// Repaint editors when the syntax palette changes. Live theme-color edits
/// update the palette but don't rebuild the panel, so without this a color
/// change wouldn't show until the next keystroke/scroll. Marks the visible lines
/// dirty (token/selection/gutter colors are respawned on render) and refreshes
/// the persistent caret fill, which is only set at spawn.
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
        let visible = ed.cursor_line >= ed.scroll && ed.cursor_line < ed.scroll + ed.visible;
        if ed.focused && on && visible && m.char_w > 0.0 {
            n.display = Display::Flex;
            n.height = Val::Px(m.caret_h);
            n.left = Val::Px(m.gutter_w + m.pad + ed.cursor_col as f32 * m.char_w);
            n.top = Val::Px((ed.cursor_line - ed.scroll) as f32 * m.line_h + (m.line_h - m.caret_h) / 2.0);
        } else {
            n.display = Display::None;
        }
    }
}

fn render_line(commands: &mut Commands, font: &bevy::text::FontSource, m: Metrics, num: usize, spans: &[(String, Color)]) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                height: Val::Px(m.line_h),
                align_items: AlignItems::Center,
                ..default()
            },
            Name::new("code-line"),
        ))
        .id();
    let gutter = commands
        .spawn((Node {
            width: Val::Px(m.gutter_w),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::FlexEnd,
            padding: UiRect::right(Val::Px(8.0)),
            ..default()
        },))
        .with_children(|p| {
            p.spawn((
                Text::new(format!("{num}")),
                mono(font, m.gutter_size),
                TextColor(rgb(syntax_palette().line_number)),
            ));
        })
        .id();
    let line_text = commands
        .spawn((
            Text::new(""),
            mono(font, m.font_size),
            TextColor(rgb(syntax_palette().normal)),
            TextLayout::no_wrap(),
            Node {
                padding: UiRect::left(Val::Px(m.pad)),
                ..default()
            },
        ))
        .id();
    let span_ents: Vec<Entity> = spans
        .iter()
        .map(|(s, color)| {
            commands
                .spawn((TextSpan::new(s.clone()), mono(font, m.font_size), TextColor(*color)))
                .id()
        })
        .collect();
    commands.entity(line_text).add_children(&span_ents);
    commands.entity(row).add_children(&[gutter, line_text]);
    row
}
