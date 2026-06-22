//! Code editor — a monospace, syntax-highlighted, editable text view.
//!
//! Monospace makes caret/scroll math trivial: column → pixel is `col * char_w`.
//! Every metric (line height, gutter width, caret height, char advance) is
//! *derived from the live font size* ([`CodeEditor::font_size`]), so the editor
//! honors the zoom level (Ctrl +/-) and the Settings code-font size instead of a
//! hardcoded 13px. The per-font advance ratio is measured from the actual font
//! (see [`systems::code_probe`]) rather than assuming 0.6em, so non-default mono
//! fonts (Fira Code, Source Code Pro, …) still get pixel-correct carets.
//!
//! Monospace is intentional and matches every real code editor: Bevy's
//! `PositionedGlyph` exposes a glyph's pixel position but not its source
//! character/cluster index, so mapping an arbitrary glyph back to a column (what
//! proportional click/caret hit-testing needs) isn't available for our
//! multi-token text. Ligature mono fonts still work — a ligature keeps the
//! combined cell advance, so per-character column math stays exact.
//!
//! The doc is a `Vec<String>`; [`systems::code_input`] edits it,
//! [`systems::code_render`] rebuilds the visible lines (gutter + colored token
//! spans), and [`systems::code_caret`] positions the blinking cursor. Click to
//! place the cursor, wheel to scroll.
//!
//! Submodules: [`highlight`] (tokenizer), [`edit`] (document ops), [`systems`].

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::window::SystemCursorIcon;

use crate::theme::*;

mod binding;
mod edit;
pub(crate) mod highlight;
mod systems;

pub use binding::{bind_code, CodeBindingSpec};

/// Default code-font size (logical px) until the host pushes one via the
/// binding's `font_size` closure.
const DEFAULT_FONT_SIZE: f32 = 14.0;
/// Gutter line-number size relative to the code font (was 11/13).
const GUTTER_SIZE_RATIO: f32 = 11.0 / 13.0;
/// Line height relative to the code font (was 18/13 ≈ 1.385).
const LINE_H_RATIO: f32 = 18.0 / 13.0;
/// Caret height relative to the code font (was 16/13).
const CARET_H_RATIO: f32 = 16.0 / 13.0;
/// Fallback monospace advance (em) before [`systems::code_probe`] measures the
/// real one for the active font. 0.6 is exact for the bundled mono fonts.
const DEFAULT_ADVANCE: f32 = 0.6;
/// Horizontal padding between the gutter edge and the code text (logical px).
const PAD: f32 = 8.0;
/// Left+right breathing room added to the gutter around the digits (logical px).
const GUTTER_PAD: f32 = 14.0;
/// Columns per indent level — matches the Tab key's 4-space insert (see
/// [`edit`]). Used for indent guides.
pub(crate) const TAB_WIDTH: usize = 4;

/// Per-editor derived metrics, all logical px (except `font_size` which is the
/// source). Recomputed from [`CodeEditor::font_size`] / `advance_ratio` /
/// line-count by [`CodeEditor::recompute_metrics`].
#[derive(Clone, Copy)]
pub(crate) struct Metrics {
    pub font_size: f32,
    pub gutter_size: f32,
    pub line_h: f32,
    pub caret_h: f32,
    pub char_w: f32,
    pub gutter_w: f32,
    pub pad: f32,
}

/// Registers the code-editor systems.
pub(crate) struct CodeEditorPlugin;

impl Plugin for CodeEditorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                // Runs first so a doc switch reloads the buffer (and stores the
                // previous frame's edits) before input/render see it.
                binding::code_sync,
                // Recompute derived metrics (and measure the font advance) before
                // anything reads char_w / line_h this frame.
                systems::code_metrics,
                systems::code_probe,
                systems::code_measure,
                systems::code_pointer,
                systems::code_input,
                systems::code_scroll,
                systems::code_theme_watch,
                systems::code_render,
                systems::code_caret,
            ),
        );
    }
}

/// One colored run within a code line, produced by an injected [`Highlighter`].
/// `len` is the run length in **bytes** (the runs must exactly cover the line,
/// matching the host tokenizer's byte spans).
pub struct CodeToken {
    pub len: usize,
    pub color: Color,
}

/// A per-line tokenizer injected by the host crate (which owns the language
/// grammars). Given a line (no trailing `\n`) and the incoming cross-line state
/// (an opaque `u32`, e.g. `0` = normal, `1` = inside a block comment), it
/// returns the colored runs covering the line plus the outgoing state for the
/// next line. Lets ember stay language-agnostic without depending back on the
/// code-editor crate.
pub type Highlighter = Box<dyn Fn(&str, u32) -> (Vec<CodeToken>, u32) + Send + Sync>;

#[derive(Component)]
pub(crate) struct CodeEditor {
    text: Vec<String>,
    cursor_line: usize,
    cursor_col: usize,
    anchor_line: usize,
    anchor_col: usize,
    scroll: usize,
    /// Live code-font size (logical px); driven by the host zoom / settings.
    font_size: f32,
    /// Measured monospace advance as a fraction of `font_size` (the probe
    /// updates this; [`DEFAULT_ADVANCE`] until then).
    advance_ratio: f32,
    /// Derived: `font_size * advance_ratio`.
    char_w: f32,
    /// Derived line height.
    line_h: f32,
    /// Derived gutter width (grows with the digit count).
    gutter_w: f32,
    /// Viewport width (logical px), tracked by `code_measure`; used to size
    /// full-width overlays like the current-line highlight.
    view_w: f32,
    visible: usize,
    focused: bool,
    dirty: bool,
    /// Set whenever the document text is mutated (not on caret moves). Watched
    /// by [`binding::code_sync`] to push the buffer back to the host store.
    content_dirty: bool,
    /// Identity of the currently loaded document; a change triggers a reload.
    last_key: Option<u64>,
    /// Host-injected per-line highlighter; falls back to the built-in Rust-ish
    /// tokenizer when `None`.
    highlighter: Option<Highlighter>,
    body: Entity,
    caret: Entity,
}

impl CodeEditor {
    /// Recompute the derived metrics from the live font size / advance / line
    /// count. Returns `true` if any visible-affecting metric changed (so callers
    /// can flag a re-render).
    fn recompute_metrics(&mut self) -> bool {
        let char_w = self.font_size * self.advance_ratio;
        let line_h = (self.font_size * LINE_H_RATIO).round().max(self.font_size + 2.0);
        let digits = self.text.len().max(1).to_string().len().max(2);
        let gutter_w = (digits as f32 * char_w + GUTTER_PAD).round();
        let changed = char_w != self.char_w || line_h != self.line_h || gutter_w != self.gutter_w;
        self.char_w = char_w;
        self.line_h = line_h;
        self.gutter_w = gutter_w;
        changed
    }

    fn metrics(&self) -> Metrics {
        Metrics {
            font_size: self.font_size,
            gutter_size: self.font_size * GUTTER_SIZE_RATIO,
            line_h: self.line_h,
            caret_h: self.font_size * CARET_H_RATIO,
            char_w: self.char_w,
            gutter_w: self.gutter_w,
            pad: PAD,
        }
    }
}

#[derive(Component)]
pub(crate) struct CodeViewport {
    editor: Entity,
}

/// A hidden, laid-out probe used to measure the active mono font's real advance
/// (see [`systems::code_probe`]). Holds the editor it feeds.
#[derive(Component)]
pub(crate) struct CodeProbe {
    pub editor: Entity,
}

fn mono(font: &bevy::text::FontSource, size: f32) -> TextFont {
    TextFont {
        font: font.clone(),
        font_size: bevy::text::FontSize::Px(size),
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
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Text),
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
                height: Val::Px(DEFAULT_FONT_SIZE * CARET_H_RATIO),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(syntax_palette().cursor)),
            bevy::ui::FocusPolicy::Pass,
            Name::new("code-caret"),
        ))
        .id();
    // Hidden probe: laid out (so Bevy computes its `TextLayoutInfo`) but not
    // visible. `code_probe` reads its width to derive the real font advance.
    let probe = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..default()
            },
            Text::new(PROBE_TEXT),
            // Seeded with the generic monospace family; `code_probe` swaps in the
            // editor's actual mono font (and size) before reading the advance.
            mono(&bevy::text::FontSource::Monospace, DEFAULT_FONT_SIZE),
            TextLayout::no_wrap(),
            Visibility::Hidden,
            bevy::ui::FocusPolicy::Pass,
            CodeProbe { editor: root },
            Name::new("code-probe"),
        ))
        .id();
    commands.entity(viewport).add_children(&[body, caret, probe]);
    commands.entity(root).add_child(viewport);
    let mut ed = CodeEditor {
        text: lines,
        cursor_line: 0,
        cursor_col: 0,
        anchor_line: 0,
        anchor_col: 0,
        scroll: 0,
        font_size: DEFAULT_FONT_SIZE,
        advance_ratio: DEFAULT_ADVANCE,
        char_w: DEFAULT_FONT_SIZE * DEFAULT_ADVANCE,
        line_h: (DEFAULT_FONT_SIZE * LINE_H_RATIO).round(),
        gutter_w: 0.0,
        view_w: 0.0,
        visible: 1,
        focused: false,
        dirty: true,
        content_dirty: false,
        last_key: None,
        highlighter: None,
        body,
        caret,
    };
    ed.recompute_metrics();
    commands.entity(root).insert(ed);
    root
}

/// Probe string: many `0`s so a divide gives a stable advance. Mono fonts make
/// every glyph the same width, so the digit choice is arbitrary.
const PROBE_TEXT: &str = "0000000000";
const PROBE_LEN: f32 = 10.0;
