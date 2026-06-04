//! Code editor — a monospace, syntax-highlighted, editable text view.
//!
//! Monospace (JetBrains Mono) makes caret/scroll math trivial: column → pixel is
//! `col * char_w` (char width measured once from a sample string). The doc is a
//! `Vec<String>`; [`systems::code_input`] edits it, [`systems::code_render`]
//! rebuilds the visible lines (gutter + colored token spans), and
//! [`systems::code_caret`] positions the blinking cursor. Click to place the
//! cursor, wheel to scroll.
//!
//! Submodules: [`highlight`] (tokenizer), [`edit`] (document ops), [`systems`].

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::window::SystemCursorIcon;

use crate::theme::*;

mod edit;
mod highlight;
mod systems;

const FONT_SIZE: f32 = 13.0;
const GUTTER_SIZE: f32 = 11.0;
const LINE_H: f32 = 18.0;
const GUTTER_W: f32 = 44.0;
const PAD: f32 = 8.0;
const CARET_H: f32 = 16.0;
/// JetBrains Mono advance is 600/1000 em — exact, no measurement needed.
const CHAR_W: f32 = FONT_SIZE * 0.6;

/// Registers the code-editor systems.
pub(crate) struct CodeEditorPlugin;

impl Plugin for CodeEditorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                systems::code_measure,
                systems::code_pointer,
                systems::code_input,
                systems::code_scroll,
                systems::code_render,
                systems::code_caret,
            ),
        );
    }
}

#[derive(Component)]
pub(crate) struct CodeEditor {
    text: Vec<String>,
    cursor_line: usize,
    cursor_col: usize,
    anchor_line: usize,
    anchor_col: usize,
    scroll: usize,
    char_w: f32,
    visible: usize,
    focused: bool,
    dirty: bool,
    body: Entity,
    caret: Entity,
}

#[derive(Component)]
pub(crate) struct CodeViewport {
    editor: Entity,
}

fn mono(font: &Handle<Font>, size: f32) -> TextFont {
    TextFont {
        font: font.clone(),
        font_size: size,
        ..default()
    }
}

/// Create a code editor showing `text` (highlighted as Rust-like source).
pub fn code_editor(commands: &mut Commands, text: &str) -> Entity {
    let lines: Vec<String> = if text.is_empty() {
        vec![String::new()]
    } else {
        text.lines().map(|l| l.to_string()).collect()
    };
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(260.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            BorderColor::all(rgb(border())),
            Name::new("code-editor"),
        ))
        .id();
    let viewport = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                ..default()
            },
            Interaction::default(),
            RelativeCursorPosition::default(),
            CodeViewport { editor: root },
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Text),
            Name::new("code-viewport"),
        ))
        .id();
    let body = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Name::new("code-body"),
        ))
        .id();
    let caret = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(2.0),
                height: Val::Px(CARET_H),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(accent())),
            bevy::ui::FocusPolicy::Pass,
            Name::new("code-caret"),
        ))
        .id();
    commands.entity(viewport).add_children(&[body, caret]);
    commands.entity(root).add_child(viewport);
    commands.entity(root).insert(CodeEditor {
        text: lines,
        cursor_line: 0,
        cursor_col: 0,
        anchor_line: 0,
        anchor_col: 0,
        scroll: 0,
        char_w: CHAR_W,
        visible: 1,
        focused: false,
        dirty: true,
        body,
        caret,
    });
    root
}
