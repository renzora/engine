//! The code-editor ECS systems: measuring, focus/click, keyboard, scroll,
//! re-rendering the visible lines, and the blinking caret.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::MouseWheel;
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use crate::font::EmberFonts;
use crate::theme::*;

use super::edit::{edit, has_selection, sel_range};
use super::highlight::{tokenize, C_TEXT};
use super::{
    mono, CodeEditor, CodeViewport, CARET_H, FONT_SIZE, GUTTER_SIZE, GUTTER_W, LINE_H, PAD,
};

pub(crate) fn code_measure(
    viewports: Query<(&ComputedNode, &CodeViewport)>,
    mut editors: Query<&mut CodeEditor>,
) {
    for (cn, vp) in &viewports {
        let h = cn.size().y * cn.inverse_scale_factor();
        let vis = ((h / LINE_H).floor() as usize).max(1);
        if let Ok(mut ed) = editors.get_mut(vp.editor) {
            if ed.visible != vis {
                ed.visible = vis;
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
        let line = (ed.scroll + (local.y / LINE_H) as usize).min(ed.text.len().saturating_sub(1));
        let col = (((local.x - GUTTER_W - PAD) / ed.char_w).round().max(0.0)) as usize;
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
        let body = ed.body;
        if let Ok(kids) = children.get(body) {
            for c in kids.iter() {
                commands.entity(c).despawn();
            }
        }
        let start = ed.scroll;
        let end = (start + ed.visible + 1).min(ed.text.len());

        // Selection highlight rects (added first → behind the text).
        let mut sel_rects: Vec<Entity> = Vec::new();
        if has_selection(&ed) {
            let ((sl, sc), (el, ec)) = sel_range(&ed);
            for l in sl.max(start)..=el.min(end.saturating_sub(1)) {
                let ca = if l == sl { sc } else { 0 };
                let cb = if l == el { ec } else { ed.text[l].chars().count() };
                let w = (cb.saturating_sub(ca) as f32 * ed.char_w)
                    .max(if l < el { ed.char_w * 0.5 } else { 0.0 });
                if w <= 0.0 {
                    continue;
                }
                let rect = commands
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(GUTTER_W + PAD + ca as f32 * ed.char_w),
                            top: Val::Px((l - start) as f32 * LINE_H),
                            width: Val::Px(w),
                            height: Val::Px(LINE_H),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.31, 0.55, 1.0, 0.30)),
                        bevy::ui::FocusPolicy::Pass,
                        Name::new("code-selection"),
                    ))
                    .id();
                sel_rects.push(rect);
            }
        }
        commands.entity(body).add_children(&sel_rects);

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
                        out.push((line[off..].to_string(), rgb(C_TEXT)));
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
            .map(|(k, spans)| render_line(&mut commands, &fonts.mono, start + k + 1, spans))
            .collect();
        commands.entity(body).add_children(&rows);
    }
}

pub(crate) fn code_caret(time: Res<Time>, editors: Query<&CodeEditor>, mut nodes: Query<&mut Node>) {
    let on = (time.elapsed_secs() * 1.6).fract() < 0.5;
    for ed in &editors {
        let Ok(mut n) = nodes.get_mut(ed.caret) else {
            continue;
        };
        let visible = ed.cursor_line >= ed.scroll && ed.cursor_line < ed.scroll + ed.visible;
        if ed.focused && on && visible && ed.char_w > 0.0 {
            n.display = Display::Flex;
            n.left = Val::Px(GUTTER_W + PAD + ed.cursor_col as f32 * ed.char_w);
            n.top = Val::Px((ed.cursor_line - ed.scroll) as f32 * LINE_H + (LINE_H - CARET_H) / 2.0);
        } else {
            n.display = Display::None;
        }
    }
}

fn render_line(commands: &mut Commands, font: &bevy::text::FontSource, num: usize, spans: &[(String, Color)]) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                height: Val::Px(LINE_H),
                align_items: AlignItems::Center,
                ..default()
            },
            Name::new("code-line"),
        ))
        .id();
    let gutter = commands
        .spawn((Node {
            width: Val::Px(GUTTER_W),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::FlexEnd,
            padding: UiRect::right(Val::Px(8.0)),
            ..default()
        },))
        .with_children(|p| {
            p.spawn((
                Text::new(format!("{num}")),
                mono(font, GUTTER_SIZE),
                TextColor(rgb(placeholder())),
            ));
        })
        .id();
    let line_text = commands
        .spawn((
            Text::new(""),
            mono(font, FONT_SIZE),
            TextColor(rgb(C_TEXT)),
            TextLayout::no_wrap(),
            Node {
                padding: UiRect::left(Val::Px(PAD)),
                ..default()
            },
        ))
        .id();
    let span_ents: Vec<Entity> = spans
        .iter()
        .map(|(s, color)| {
            commands
                .spawn((TextSpan::new(s.clone()), mono(font, FONT_SIZE), TextColor(*color)))
                .id()
        })
        .collect();
    commands.entity(line_text).add_children(&span_ents);
    commands.entity(row).add_children(&[gutter, line_text]);
    row
}
